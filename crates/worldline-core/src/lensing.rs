//! Light bent by a non-spinning black hole: rays (null geodesics) of
//! Schwarzschild spacetime.
//!
//! A ray stays in one plane, where u = 1/r obeys
//!
//! d²u/dφ² + u = 3GMu²/c²
//!
//! (Binet's equation for light; e.g. Misner, Thorne & Wheeler 1973, §25.6).
//! Rays aimed closer than √27 GM/c² fall in: that is the shadow's radius
//! (Synge 1966). A ray passing at impact parameter b is turned by an angle
//! given exactly by an elliptic integral (Darwin 1959), about 4GM/(c²b)
//! far out (Einstein 1915).
//!
//! What the camera sees is worked out here, in double precision: for each
//! angle from the direction to the hole at which the camera looks, where
//! on the sky that ray ends up. The GPU only looks that up (see
//! [`BendTable`]).
//!
//! See `docs/physics/lensing.md`.

use std::f64::consts::PI;

use crate::constants::C;

/// The radius of a non-spinning hole's shadow seen from far away, its
/// critical impact parameter: √27 GM/c², in m.
pub fn critical_impact(gm: f64) -> f64 {
    27f64.sqrt() * gm / (C * C)
}

/// The radius of the photon sphere, where light can circle: 3GM/c², in m.
pub fn photon_sphere(gm: f64) -> f64 {
    3.0 * gm / (C * C)
}

/// The angular radius of the shadow, in rad, for an observer at rest at
/// `distance` (m) from the hole: sin θ = √27 (GM/c²r) √(1 − 2GM/c²r),
/// past 90° inside the photon sphere (Synge 1966).
pub fn shadow_angle(gm: f64, distance: f64) -> f64 {
    let x = gm / (C * C * distance);
    let sine = (27f64.sqrt() * x * (1.0 - 2.0 * x).max(0.0).sqrt()).min(1.0);
    if distance >= photon_sphere(gm) {
        sine.asin()
    } else {
        PI - sine.asin()
    }
}

/// Carlson's symmetric elliptic integral of the first kind,
/// R_F(x, y, z) = ½∫₀^∞ dt/√((t + x)(t + y)(t + z)), by duplication
/// (Carlson 1995, *Numer. Algorithms* 10, 13).
pub fn carlson_rf(x: f64, y: f64, z: f64) -> f64 {
    let (mut x, mut y, mut z) = (x, y, z);
    loop {
        let mean = (x + y + z) / 3.0;
        let (dx, dy, dz) = (1.0 - x / mean, 1.0 - y / mean, 1.0 - z / mean);
        if dx.abs().max(dy.abs()).max(dz.abs()) < 1e-4 {
            // The series to fifth order: its error is below 10⁻²⁰.
            let e2 = dx * dy - dz * dz;
            let e3 = dx * dy * dz;
            return (1.0 - e2 / 10.0 + e3 / 14.0 + e2 * e2 / 24.0 - 3.0 * e2 * e3 / 44.0)
                / mean.sqrt();
        }
        let (sx, sy, sz) = (x.sqrt(), y.sqrt(), z.sqrt());
        let lambda = sx * sy + sx * sz + sy * sz;
        (x, y, z) = (
            0.25 * (x + lambda),
            0.25 * (y + lambda),
            0.25 * (z + lambda),
        );
    }
}

/// The three roots of 2u³ − u² + 1/b² (u in units of c²/GM, b in GM/c²),
/// for b above √27, smallest first: one negative, then the turning point
/// of a ray passing at b, then one beyond the photon sphere.
fn roots(b: f64) -> [f64; 3] {
    // acos(1 − 54/b²) as 2 asin(√27/b): exact, and without the rounding
    // of a cosine near 1 far from the hole.
    let theta = 2.0 * (27f64.sqrt() / b).min(1.0).asin() / 3.0;
    let root = |k: f64| {
        let mut u = (1.0 + 2.0 * (theta - 2.0 * PI * k / 3.0).cos()) / 6.0;
        // Far out, the two small roots come from 1 + 2cos(…) cancelling:
        // Newton's method on the cubic itself gives them back to full
        // precision. Near √27 two roots merge and the formula needs none.
        if b > 50.0 {
            for _ in 0..3 {
                u -= ((2.0 * u - 1.0) * u * u + 1.0 / (b * b)) / (2.0 * u * (3.0 * u - 1.0));
            }
        }
        u
    };
    [root(2.0), root(1.0), root(0.0)]
}

/// ∫_y^x du/√(1/b² − u² + 2u³), for 0 ≤ y < x up to the turning point,
/// in units of GM/c², by Carlson's formula for three linear factors:
/// ∫_y^x Π(a_i + b_i t)^(−1/2) dt = 2R_F(U₁₂², U₁₃², U₂₃²) (Carlson 1988,
/// *Math. Comp.* 51, 267).
fn sweep(b: f64, y: f64, x: f64) -> f64 {
    let [low, turn, high] = roots(b);
    // The factors u − low, turn − u, high − u, at both ends.
    let at = |u: f64| [(u - low).max(0.0), (turn - u).max(0.0), (high - u).max(0.0)].map(f64::sqrt);
    let (xs, ys) = (at(x), at(y));
    let u =
        |i: usize, j: usize, k: usize| (xs[i] * xs[j] * ys[k] + ys[i] * ys[j] * xs[k]) / (x - y);
    let (u12, u13, u23) = (u(0, 1, 2), u(0, 2, 1), u(1, 2, 0));
    // G(u) = 2(u − low)(turn − u)(high − u).
    2.0 * carlson_rf(u12 * u12, u13 * u13, u23 * u23) / 2f64.sqrt()
}

/// How far a ray from far away, passing a hole of `gm` at impact parameter
/// `b` (m), is turned, in rad, if it isn't captured: exactly, 2∫₀^u_t du/√G − π
/// (Darwin 1959), or `None` for b at or below √27 GM/c².
pub fn deflection(gm: f64, b: f64) -> Option<f64> {
    let m = gm / (C * C);
    let b = b / m;
    if b <= 27f64.sqrt() {
        return None;
    }
    let turn = roots(b)[1];
    Some(2.0 * sweep(b, 0.0, turn) - PI)
}

/// Where a ray goes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ray {
    /// It falls into the hole.
    Captured,
    /// It escapes, having swept this angle around the hole (rad) from where
    /// it started, measured from the hole: a straight ray sweeps π − ψ.
    Escapes(f64),
}

/// Traces a ray backward from an observer at rest `distance` (m) from a
/// hole of `gm`, looking at angle `psi` (rad) from the direction to the
/// hole, by fourth-order Runge–Kutta on Binet's equation in steps of
/// `step` rad: its error is of order step⁴ per radian swept.
///
/// The observer's own angle sets the impact parameter, b = r sin ψ /
/// √(1 − 2GM/c²r), so the shadow's edge falls where Synge's formula puts
/// it.
pub fn trace(gm: f64, distance: f64, psi: f64, step: f64) -> Ray {
    let m = gm / (C * C);
    let r = distance / m;
    if r <= 2.0 {
        return Ray::Captured;
    }
    let u0 = 1.0 / r;
    // du/dφ = u √(1 − 2u) cot ψ: positive while the ray closes in.
    let (sine, cosine) = psi.sin_cos();
    if sine.abs() < 1e-300 {
        return if cosine > 0.0 {
            Ray::Captured
        } else {
            Ray::Escapes(0.0)
        };
    }
    let w0 = u0 * (1.0 - 2.0 * u0).sqrt() * cosine / sine;
    let field = |u: f64| 3.0 * u * u - u;
    let (mut u, mut w, mut phi) = (u0, w0, 0.0);
    // A ray inside the photon sphere moving inward can't come back out.
    while phi < 40.0 * PI {
        if u >= 0.5 || (u > 1.0 / 3.0 && w > 0.0) {
            return Ray::Captured;
        }
        let k1 = (w, field(u));
        let k2 = (w + 0.5 * step * k1.1, field(u + 0.5 * step * k1.0));
        let k3 = (w + 0.5 * step * k2.1, field(u + 0.5 * step * k2.0));
        let k4 = (w + step * k3.1, field(u + step * k3.0));
        let next_u = u + step / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
        let next_w = w + step / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
        if next_u <= 0.0 {
            // Out to infinity within this step: where the cubic through
            // both ends reaches u = 0.
            let at = |s: f64| {
                let (s2, s3) = (s * s, s * s * s);
                u * (2.0 * s3 - 3.0 * s2 + 1.0)
                    + w * step * (s3 - 2.0 * s2 + s)
                    + next_u * (3.0 * s2 - 2.0 * s3)
                    + next_w * step * (s3 - s2)
            };
            let (mut lo, mut hi) = (0.0, 1.0);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if at(mid) > 0.0 {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            return Ray::Escapes(phi + 0.5 * (lo + hi) * step);
        }
        (u, w, phi) = (next_u, next_w, phi + step);
    }
    // Still circling after twenty turns: on the photon sphere, as good as
    // captured.
    Ray::Captured
}

/// For an observer at one distance from a hole: how much each ray is bent,
/// sampled for the GPU to look up. Rays at angle ψ from the direction to
/// the hole, inside the shadow's edge ψ_c, are captured; beyond it, a ray
/// ends up swept π − ψ + δ(ψ) around the hole, δ its bending. δ grows
/// without bound toward the edge, like −ln(ψ − ψ_c), so it is sampled
/// evenly in x = ln((ψ − ψ_c)/(π − ψ_c)), from `X_MIN` to 0.
#[derive(Debug, Clone, PartialEq)]
pub struct BendTable {
    /// The shadow's edge ψ_c, in rad.
    pub edge: f64,
    /// δ at evenly spaced x from `X_MIN` to 0, in rad.
    pub bending: Vec<f64>,
}

impl BendTable {
    /// The smallest x sampled: within 10⁻⁷ of the way from the edge to the
    /// far side, closer than any pixel.
    pub const X_MIN: f64 = -16.0;
    /// How many samples: linear interpolation between them is good to
    /// 10⁻⁵ rad (see `docs/physics/lensing.md`).
    pub const SAMPLES: usize = 2048;
    /// The Runge–Kutta step for the table, in rad: its error, of order
    /// step⁴ per radian swept, is under 10⁻⁶ rad.
    pub const STEP: f64 = 0.02;

    /// The table for an observer at rest `distance` (m) from a hole of
    /// `gm`.
    pub fn new(gm: f64, distance: f64) -> Self {
        let edge = shadow_angle(gm, distance);
        let span = PI - edge;
        let sample = |i: usize| {
            let x = Self::X_MIN * (1.0 - i as f64 / (Self::SAMPLES - 1) as f64);
            let psi = edge + span * x.exp();
            match trace(gm, distance, psi, Self::STEP) {
                Ray::Escapes(swept) => swept - (PI - psi),
                // Just outside the edge a ray can circle more than our
                // tracing follows: keep the last bending found.
                Ray::Captured => f64::NAN,
            }
        };
        // The rays are independent: shared out among the cores.
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk = Self::SAMPLES.div_ceil(threads);
        let mut bending = Vec::with_capacity(Self::SAMPLES);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..Self::SAMPLES)
                .step_by(chunk)
                .map(|first| {
                    let sample = &sample;
                    scope.spawn(move || {
                        (first..(first + chunk).min(Self::SAMPLES))
                            .map(sample)
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for handle in handles {
                bending.extend(handle.join().expect("a tracing thread finished"));
            }
        });
        // Fill any samples traced as captured from their outer neighbor.
        for i in (0..bending.len().saturating_sub(1)).rev() {
            if bending[i].is_nan() {
                bending[i] = bending[i + 1];
            }
        }
        Self { edge, bending }
    }

    /// The bending at angle `psi` (rad) from the direction to the hole, by
    /// linear interpolation as the GPU does it; `None` inside the shadow.
    pub fn bending(&self, psi: f64) -> Option<f64> {
        if psi <= self.edge {
            return None;
        }
        let x = ((psi - self.edge) / (PI - self.edge)).ln().max(Self::X_MIN);
        let f = (1.0 - x / Self::X_MIN) * (Self::SAMPLES - 1) as f64;
        let i = (f.floor() as usize).min(Self::SAMPLES - 2);
        let t = f - i as f64;
        Some(self.bending[i] * (1.0 - t) + self.bending[i + 1] * t)
    }
}

/// The largest impact parameter light leaving the surface of a body of
/// `gm` and `radius` (m) can have, in m: a ray grazing it, R/√(1 − 2GM/c²R).
/// Seen from far away, the body looks this big: its own gravity bends light
/// from behind it into view. Only for a body bigger than its photon sphere.
pub fn grazing_impact(gm: f64, radius: f64) -> f64 {
    radius / (1.0 - 2.0 * gm / (C * C * radius)).sqrt()
}

/// How far round a body of `gm` and `radius` (m), bigger than its photon
/// sphere, a ray from its surface with impact parameter `b` (m) carries
/// light on its way to an observer `observer` m from its center
/// (`f64::INFINITY` for far away): the angle ψ at the center between where
/// it left the surface and the observer, in rad. `None` for b past the
/// grazing ray's.
///
/// Exactly ψ = ∫ du/√(1/b² − u² + 2u³) from the observer's u to the
/// surface's, u = GM/c²r and b in GM/c² (Pechenick, Ftaclas & Cohen 1983);
/// here by Gauss–Legendre quadrature, after substitutions that leave the
/// integrand smooth even for the grazing ray, whose 1/√ blows up at the
/// surface. Without gravity it's asin(b/R), half the body seen; a neutron
/// star shows more than half (see `docs/physics/lensing.md`).
pub fn surface_angle(gm: f64, radius: f64, observer: f64, b: f64) -> Option<f64> {
    let m = gm / (C * C);
    let (surface, far, b) = (m / radius, m / observer, b / m);
    let grazing = grazing_impact(gm, radius) / m;
    if b > grazing * (1.0 + 1e-12) || far >= surface {
        return None;
    }
    if b <= 0.0 {
        return Some(0.0);
    }
    // u = surface − Δs²: G(u) = G(surface) + Δs² q(u), with q(u) = (u +
    // surface) − 2(u² + u·surface + surface²), positive outside the photon
    // sphere. G(surface) = 1/b² − (surface² − 2 surface³) = 1/b² −
    // 1/grazing², written so, it's exactly 0 for the grazing ray rather than
    // what's left of cancelling terms. Then s = σ², which spreads the nodes
    // where G(surface) is small but not zero.
    let span = surface - far;
    let at_surface = ((grazing * grazing - b * b) / (b * b * grazing * grazing)).max(0.0);
    let integrand = |sigma: f64| {
        let s = sigma * sigma;
        let u = surface - span * s * s;
        let q = (u + surface) - 2.0 * (u * u + u * surface + surface * surface);
        // dψ = du/√G, du = 2Δs ds, ds = 2σ dσ.
        4.0 * span * s * sigma / (at_surface + span * s * s * q).sqrt()
    };
    Some(
        gauss_legendre()
            .iter()
            .map(|&(x, w)| 0.5 * w * integrand(0.5 * (x + 1.0)))
            .sum(),
    )
}

/// The angle from the direction to a body's center at which an observer at
/// rest `observer` m from it (of `gm`) sees a ray of impact parameter `b`
/// (m) arrive: sin δ = (b/r)√(1 − 2GM/c²r), as `trace` takes it.
pub fn arrival_angle(gm: f64, observer: f64, b: f64) -> f64 {
    ((b / observer) * (1.0 - 2.0 * gm / (C * C * observer)).sqrt())
        .min(1.0)
        .asin()
}

/// 64-point Gauss–Legendre nodes and weights on [−1, 1], by Newton's
/// method on the Legendre polynomial (e.g. Press et al. 2007, §4.6):
/// exact for polynomials up to degree 127.
fn gauss_legendre() -> &'static [(f64, f64)] {
    static NODES: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();
    NODES.get_or_init(|| {
        const N: usize = 64;
        (0..N)
            .map(|i| {
                let mut x = (PI * (i as f64 + 0.75) / (N as f64 + 0.5)).cos();
                let mut derivative = 0.0;
                for _ in 0..100 {
                    let (mut p0, mut p1) = (1.0, x);
                    for k in 2..=N {
                        let k = k as f64;
                        (p0, p1) = (p1, ((2.0 * k - 1.0) * x * p1 - (k - 1.0) * p0) / k);
                    }
                    derivative = N as f64 * (x * p1 - p0) / (x * x - 1.0);
                    let step = p1 / derivative;
                    x -= step;
                    if step.abs() < 1e-16 {
                        break;
                    }
                }
                (x, 2.0 / ((1.0 - x * x) * derivative * derivative))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauss_legendre_integrates_polynomials_exactly() {
        let nodes = gauss_legendre();
        let total: f64 = nodes.iter().map(|&(_, w)| w).sum();
        assert!((total - 2.0).abs() < 1e-13);
        // ∫ x¹²⁶ dx over [−1, 1] = 2/127.
        let high: f64 = nodes.iter().map(|&(x, w)| w * x.powi(126)).sum();
        assert!((high - 2.0 / 127.0).abs() < 1e-14);
    }

    #[test]
    fn a_grazing_ray_from_the_surface_is_half_of_darwins_ray() {
        // A ray from far away whose closest approach is the surface,
        // impact parameter R/√(1 − 2GM/c²R), is turned by Darwin's exact
        // 2∫du/√G − π: the surface's ψ, out and back, less the π a straight
        // line sweeps. From a little outside the photon sphere to far out.
        // The two agree to rounding: 10⁻¹² rad.
        let gm = 1.0e20;
        let m = gm / (C * C);
        for r in [3.2, 4.0, 5.8, 10.0, 100.0, 1e4] {
            let radius = r * m;
            let b = grazing_impact(gm, radius);
            let psi = surface_angle(gm, radius, f64::INFINITY, b).expect("the grazing ray");
            let darwin = deflection(gm, b).expect("not captured");
            println!(
                "R = {r} GM/c²: 2ψ − π = {:.15}, Darwin {darwin:.15}",
                2.0 * psi - PI
            );
            assert!((2.0 * psi - PI - darwin).abs() < 1e-12, "R = {r}");
        }
    }

    #[test]
    fn without_gravity_half_the_body_shows() {
        // A body far larger than its gravity's reach: ψ = asin(b/R), and
        // the grazing ray reaches 90°.
        let (gm, radius) = (1.0, 1.0e6);
        for b in [0.0, 0.3e6, 0.7e6, 1.0e6] {
            let psi = surface_angle(gm, radius, f64::INFINITY, b).expect("on the surface");
            assert!((psi - (b / radius).asin()).abs() < 1e-9, "b = {b}");
        }
        assert!(surface_angle(gm, radius, f64::INFINITY, 1.01e6).is_none());
    }

    #[test]
    fn a_neutron_star_shows_more_than_half_of_itself() {
        // The Hulse–Taylor pulsar, 1.438 Suns in 11.9 km, 5.6 GM/c²: its
        // grazing ray left the surface 122.4° round from the line of sight,
        // so 76.8% of its surface shows, and it looks 1.247 times its size.
        // Seen from 100 km, a little less of it.
        let gm = 1.438 * crate::constants::GM_SUN;
        let radius = 11.9e3;
        let b = grazing_impact(gm, radius);
        let far = surface_angle(gm, radius, f64::INFINITY, b).expect("grazing");
        let near = surface_angle(gm, radius, 100e3, b).expect("grazing");
        let shown = (1.0 - far.cos()) / 2.0;
        println!(
            "looks {:.4} times its size; grazing ray from {:.2}° ({:.2}° seen from 100 km): {:.1}% of the surface",
            b / radius,
            far.to_degrees(),
            near.to_degrees(),
            100.0 * shown
        );
        assert!((b / radius - 1.247).abs() < 5e-4);
        assert!((far.to_degrees() - 122.43).abs() < 5e-3);
        assert!(near < far);
        // More light bent round the closer the star is to its photon
        // sphere: monotonic in b.
        let mut last = -1.0;
        for k in 0..=50 {
            let psi = surface_angle(gm, radius, f64::INFINITY, b * f64::from(k) / 50.0).unwrap();
            assert!(psi > last);
            last = psi;
        }
    }

    #[test]
    fn the_angle_a_ray_arrives_at_matches_synges_shadow_edge() {
        // For the ray grazing the photon sphere, b = √27 GM/c², the
        // arrival angle is the shadow's: Synge's formula, as
        // `shadow_angle` has it.
        let gm = 1.0e20;
        let distance = 40.0 * gm / (C * C);
        let at = arrival_angle(gm, distance, critical_impact(gm));
        assert!((at - shadow_angle(gm, distance)).abs() < 1e-14);
    }
    use crate::constants::GM_SUN;

    #[test]
    fn carlson_matches_his_test_values() {
        // Carlson (1995), section 3: R_F(1, 2, 0) = 1.3110287771461 and
        // R_F(2, 3, 4) = 0.58408284167715.
        assert!((carlson_rf(1.0, 2.0, 0.0) - 1.311_028_777_146_1).abs() < 1e-13);
        assert!((carlson_rf(2.0, 3.0, 4.0) - 0.584_082_841_677_15).abs() < 1e-13);
    }

    #[test]
    fn the_roots_meet_at_the_photon_sphere() {
        // At b = √27 the turning point and the root beyond merge at u = 1/3.
        let [low, turn, high] = roots(27f64.sqrt());
        assert!((turn - 1.0 / 3.0).abs() < 1e-7 && (high - 1.0 / 3.0).abs() < 1e-7);
        assert!((low + 1.0 / 6.0).abs() < 1e-12);
        // Far out, the turning point is about 1/b.
        let [_, turn, _] = roots(1e6);
        assert!((turn * 1e6 - 1.0).abs() < 1e-5);
    }

    #[test]
    fn the_shadow_seen_from_afar_is_root_27() {
        let gm = 10.0 * GM_SUN;
        let far = 1e12 * critical_impact(gm);
        assert!((shadow_angle(gm, far) * far / critical_impact(gm) - 1.0).abs() < 1e-11);
        // On the photon sphere the edge is at 90°.
        assert!((shadow_angle(gm, photon_sphere(gm)) - PI / 2.0).abs() < 1e-7);
    }
}
