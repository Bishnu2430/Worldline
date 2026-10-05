use glam::DVec3;

use super::Gravity;
use crate::Body;

/// Newton's law of universal gravitation for point masses:
///
/// a_i = Σ_{j≠i} μ_j (r_j − r_i) / |r_j − r_i|³
///
/// Valid when gravity is weak (GM / rc² ≪ 1) and motion is slow (v ≪ c).
/// Source: Newton, *Principia* (1687). See `docs/physics/newtonian-gravity.md`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Newtonian;

impl Gravity for Newtonian {
    fn name(&self) -> &'static str {
        "Newtonian"
    }

    fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
        assert_eq!(
            bodies.len(),
            out.len(),
            "need one acceleration slot per body"
        );
        out.fill(DVec3::ZERO);
        // Visit each pair once and apply equal and opposite pulls (Newton's
        // third law), so total momentum is conserved to rounding error.
        for (i, bi) in bodies.iter().enumerate() {
            for (j, bj) in bodies.iter().enumerate().skip(i + 1) {
                let d = bj.position - bi.position;
                let r2 = d.length_squared();
                debug_assert!(r2 > 0.0, "{} and {} share a position", bi.name, bj.name);
                let inv_r3 = 1.0 / (r2 * r2.sqrt());
                out[i] += d * (bj.gm * inv_r3);
                out[j] -= d * (bi.gm * inv_r3);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_pair_follows_inverse_square_law() {
        let bodies = [
            Body::new("a", 4.0e14, 1.0),
            Body::new("b", 1.0e10, 1.0).at(DVec3::new(2.0e7, 0.0, 0.0)),
        ];
        let mut acc = [DVec3::ZERO; 2];
        Newtonian.accelerations(0.0, &bodies, &mut acc);
        // b is pulled toward a with magnitude μ_a / r².
        let expected = 4.0e14 / (2.0e7_f64).powi(2);
        assert!((acc[1].x + expected).abs() / expected < 1e-15);
        assert_eq!((acc[1].y, acc[1].z), (0.0, 0.0));
        // a is pulled toward b with magnitude μ_b / r².
        assert!(acc[0].x > 0.0);
    }

    #[test]
    fn total_force_is_zero() {
        // Newton's third law: Σ m_i a_i = 0, i.e. Σ μ_i a_i = 0.
        let bodies = [
            Body::new("a", 1.3e20, 1.0),
            Body::new("b", 4.0e14, 1.0).at(DVec3::new(1.5e11, 0.0, 0.0)),
            Body::new("c", 1.3e17, 1.0).at(DVec3::new(-3.0e11, 7.0e11, 1.0e10)),
            Body::new("d", 3.8e16, 1.0).at(DVec3::new(9.0e11, -1.0e12, -2.0e10)),
        ];
        let mut acc = [DVec3::ZERO; 4];
        Newtonian.accelerations(0.0, &bodies, &mut acc);
        let net: DVec3 = bodies.iter().zip(&acc).map(|(b, a)| b.gm * *a).sum();
        let scale: f64 = bodies
            .iter()
            .zip(&acc)
            .map(|(b, a)| b.gm * a.length())
            .sum();
        assert!(net.length() / scale < 1e-15);
    }
}
