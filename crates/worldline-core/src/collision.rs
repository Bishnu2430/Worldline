//! Collisions: bodies that touch merge, keeping their momentum. See
//! `docs/physics/collisions.md`.

use glam::DVec3;

use crate::Body;

/// Two bodies merged as one, perfectly inelastically: the masses add, the
/// merged body sits at their center of mass and moves with their combined
/// momentum (so momentum is conserved exactly), and its volume is the sum
/// of theirs (as if they had the same density). The kinetic energy of
/// their relative motion, ½ μ v² with μ = m₁m₂/(m₁ + m₂), goes into heat.
/// The merged body keeps the survivor's name.
pub fn merge(survivor: &Body, absorbed: &Body) -> Body {
    let gm = survivor.gm + absorbed.gm;
    let (position, velocity) = if gm > 0.0 {
        (
            (survivor.position * survivor.gm + absorbed.position * absorbed.gm) / gm,
            (survivor.velocity * survivor.gm + absorbed.velocity * absorbed.gm) / gm,
        )
    } else {
        (survivor.position, survivor.velocity)
    };
    Body {
        name: survivor.name.clone(),
        gm,
        radius: (survivor.radius.powi(3) + absorbed.radius.powi(3)).cbrt(),
        position,
        velocity,
    }
}

/// Where a body is along a step, by cubic Hermite interpolation between
/// its position and velocity at the start and end, `h` seconds apart, at
/// fraction `s` of the step: (position, velocity).
fn hermite(start: (DVec3, DVec3), end: (DVec3, DVec3), s: f64, h: f64) -> (DVec3, DVec3) {
    let (s2, s3) = (s * s, s * s * s);
    let position = start.0 * (2.0 * s3 - 3.0 * s2 + 1.0)
        + start.1 * (h * (s3 - 2.0 * s2 + s))
        + end.0 * (-2.0 * s3 + 3.0 * s2)
        + end.1 * (h * (s3 - s2));
    let velocity = (start.0 * (6.0 * s2 - 6.0 * s) + end.0 * (6.0 * s - 6.0 * s2)) / h
        + start.1 * (3.0 * s2 - 4.0 * s + 1.0)
        + end.1 * (3.0 * s2 - 2.0 * s);
    (position, velocity)
}

/// Whether two bodies touch during a step of `h` seconds, given their
/// relative position and velocity at its start and end: the first moment
/// their distance is within `reach` (the sum of their radii), as the
/// fraction of the step, and their relative speed then. Checking only the
/// ends of steps would miss a fast body that passes right through another
/// between them.
pub fn contact(
    start: (DVec3, DVec3),
    end: (DVec3, DVec3),
    h: f64,
    reach: f64,
) -> Option<(f64, f64)> {
    const SAMPLES: usize = 64;
    // The curve lies within the convex hull of its Bézier control points,
    // so within any ball that holds them: if that ball stays out of reach,
    // so does the curve, and the sampling can be skipped.
    let controls = [
        start.0,
        start.0 + start.1 * (h / 3.0),
        end.0 - end.1 * (h / 3.0),
        end.0,
    ];
    let center = controls.iter().sum::<DVec3>() / 4.0;
    let radius = controls
        .iter()
        .map(|c| (*c - center).length())
        .fold(0.0, f64::max);
    if center.length() - radius > reach {
        return None;
    }
    let distance = |s: f64| hermite(start, end, s, h).0.length();
    let samples: Vec<f64> = (0..=SAMPLES).map(|k| k as f64 / SAMPLES as f64).collect();
    // The first sample within reach, if any; otherwise the closest point,
    // refined between its neighboring samples (golden-section search), in
    // case a grazing contact falls between samples.
    let first = match samples.iter().position(|&s| distance(s) <= reach) {
        Some(k) => samples[k],
        None => {
            let nearest = samples
                .iter()
                .copied()
                .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))?;
            let step = 1.0 / SAMPLES as f64;
            let (mut low, mut high) = ((nearest - step).max(0.0), (nearest + step).min(1.0));
            let ratio = (5f64.sqrt() - 1.0) / 2.0;
            for _ in 0..60 {
                let (a, b) = (high - ratio * (high - low), low + ratio * (high - low));
                if distance(a) < distance(b) {
                    high = b;
                } else {
                    low = a;
                }
            }
            let closest = 0.5 * (low + high);
            if distance(closest) > reach {
                return None;
            }
            closest
        }
    };
    // Narrow down the moment of contact by bisection, from the last point
    // known to be out of reach (none, if it starts within it).
    let mut high = first;
    if distance(0.0) > reach {
        let mut low = samples
            .iter()
            .copied()
            .take_while(|&s| s < first)
            .last()
            .unwrap_or(0.0);
        for _ in 0..50 {
            let mid = 0.5 * (low + high);
            if distance(mid) <= reach {
                high = mid;
            } else {
                low = mid;
            }
        }
    } else {
        high = 0.0;
    }
    Some((high, hermite(start, end, high, h).1.length()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_keeps_mass_momentum_and_volume() {
        let a = Body::new("A", 3.0e14, 6.0e6)
            .at(DVec3::new(1.0e8, 0.0, 0.0))
            .moving(DVec3::new(0.0, 3.0e4, 0.0));
        let b = Body::new("B", 1.0e14, 4.0e6)
            .at(DVec3::new(1.0e8 + 9.0e6, 0.0, 0.0))
            .moving(DVec3::new(-1.0e4, 2.0e4, 5.0e3));
        let m = merge(&a, &b);
        assert_eq!(m.name, "A");
        assert_eq!(m.gm, 4.0e14);
        let momentum = |bodies: &[&Body]| bodies.iter().map(|x| x.velocity * x.gm).sum::<DVec3>();
        let before = momentum(&[&a, &b]);
        assert!((momentum(&[&m]) - before).length() <= 1e-15 * before.length());
        assert!(
            (m.radius.powi(3) - (a.radius.powi(3) + b.radius.powi(3))).abs()
                < 1e-9 * m.radius.powi(3)
        );
        // The kinetic energy lost is that of the relative motion, ½ μ v².
        let kinetic = |x: &Body| 0.5 * x.gm * x.velocity.length_squared();
        let lost = kinetic(&a) + kinetic(&b) - kinetic(&m);
        let mu = a.gm * b.gm / (a.gm + b.gm);
        let relative = 0.5 * mu * (a.velocity - b.velocity).length_squared();
        assert!((lost / relative - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_bezier_hull_bounds_the_curve() {
        // The quick rejection must never reject a curve that comes within
        // reach: check the hull bound against dense sampling on curved
        // paths.
        let h = 1e4;
        for k in 0..200 {
            let a = k as f64 * 0.37;
            let start = (
                DVec3::new(a.cos(), a.sin(), 0.3) * 3e8,
                DVec3::new(-a.sin(), a.cos(), 0.1) * 6e4,
            );
            let end = (
                DVec3::new((a + 1.0).cos(), (a + 1.0).sin(), -0.2) * 2e8,
                DVec3::new(a.sin(), -a.cos(), 0.0) * 5e4,
            );
            let nearest = (0..=10_000)
                .map(|i| hermite(start, end, i as f64 / 1e4, h).0.length())
                .fold(f64::MAX, f64::min);
            assert!(
                contact(start, end, h, nearest * 1.001).is_some(),
                "case {k}"
            );
        }
    }

    #[test]
    fn a_pass_between_samples_is_caught() {
        // A body crossing straight through another within one step: at
        // both ends they are 10⁹ m apart, but halfway they coincide. It
        // touches (within 10⁷ m) when it is 10⁷ m short of the middle.
        let (v, h) = (DVec3::new(2.0e5, 0.0, 0.0), 1.0e4);
        let start = (DVec3::new(-1.0e9, 0.0, 0.0), v);
        let end = (DVec3::new(1.0e9, 0.0, 0.0), v);
        let (at, speed) = contact(start, end, h, 1.0e7).expect("they touch");
        assert!((at - 0.495).abs() < 1e-9, "{at}");
        assert!((speed - 2.0e5).abs() < 1e-6);
        // A near miss stays a miss: 3 × 10⁷ m off to the side.
        let side = DVec3::new(0.0, 3.0e7, 0.0);
        assert!(contact((start.0 + side, v), (end.0 + side, v), h, 1.0e7).is_none());
    }
}
