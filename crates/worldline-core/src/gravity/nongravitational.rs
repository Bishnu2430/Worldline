use glam::{DMat3, DVec3};

use crate::constants::{AU, DAY};
use crate::mean_elements::MeanElements;

/// A small body's non-gravitational acceleration, as JPL models it: a
/// comet's outgassing, or the Yarkovsky drift of an asteroid warmed by
/// sunlight.
///
/// Marsden, Sekanina & Yeomans (1973, *Astronomical Journal* 78, 211):
///
/// a = g(r) (A1 r̂ + A2 t̂ + A3 n̂),  g(r) = α (r/r₀)^−m (1 + (r/r₀)^n)^−k
///
/// with r̂ pointing away from the Sun, t̂ along the motion in the orbit's
/// plane, and n̂ along the orbit's angular momentum. The default g(r)
/// follows water ice sublimating (α = 0.1112620426, r₀ = 2.808 AU,
/// m = 2.15, n = 5.093, k = 4.6142, which make g(1 AU) = 1); JPL fits other
/// curves where they suit a body better, such as g = (1 AU / r)² for the
/// Yarkovsky effect.
///
/// A comet often outgasses hardest some days after perihelion. Following
/// Yeomans & Chodas (1989), g is then evaluated at the distance the body
/// had a time Δt earlier, along its two-body orbit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NonGravitational {
    /// Radial, transverse and normal strengths A1, A2, A3, in m/s² (at
    /// g = 1).
    pub strengths: DVec3,
    /// The delay Δt, in s.
    pub delay: f64,
    /// α, the normalizing factor.
    pub alpha: f64,
    /// r₀, the normalizing distance, in m.
    pub r0: f64,
    /// The exponents m, n and k.
    pub exponents: [f64; 3],
}

impl NonGravitational {
    /// The water-ice model with strengths in JPL's units (AU/day²).
    pub fn water_ice(a1: f64, a2: f64, a3: f64) -> Self {
        Self {
            strengths: DVec3::new(a1, a2, a3) * (AU / (DAY * DAY)),
            delay: 0.0,
            alpha: 0.111_262_042_6,
            r0: 2.808 * AU,
            exponents: [2.15, 5.093, 4.6142],
        }
    }

    /// g(r) at distance `r` (m) from the Sun.
    pub fn g(&self, r: f64) -> f64 {
        let [m, n, k] = self.exponents;
        let x = r / self.r0;
        self.alpha * x.powf(-m) * (1.0 + x.powf(n)).powf(-k)
    }

    /// The acceleration (m/s²) of a body at heliocentric position `r` (m)
    /// with heliocentric velocity `v` (m/s), around a Sun of
    /// gravitational parameter `mu_sun` (m³/s²).
    pub fn acceleration(&self, r: DVec3, v: DVec3, mu_sun: f64) -> DVec3 {
        let distance = if self.delay == 0.0 {
            r.length()
        } else {
            let orbit = MeanElements::osculating(r, v, mu_sun, DMat3::IDENTITY, 0.0);
            if orbit.e < 1.0 {
                orbit.state_at(-self.delay).0.length()
            } else {
                hyperbolic_distance(r, v, mu_sun, -self.delay)
            }
        };
        let radial = r.normalize();
        let normal = r.cross(v).normalize();
        let transverse = normal.cross(radial);
        let s = self.strengths;
        (radial * s.x + transverse * s.y + normal * s.z) * self.g(distance)
    }
}

/// The distance from the Sun a time `dt` later (earlier if negative) along
/// an unbound two-body orbit through `r`, `v`.
///
/// On a hyperbola, r = |a| (e cosh H − 1) and r·v = √(μ|a|) e sinh H, and
/// the hyperbolic anomaly H advances by Kepler's equation
/// e sinh H − H = n t, with n = √(μ/|a|³) (Murray & Dermott 1999, §2.5).
fn hyperbolic_distance(r: DVec3, v: DVec3, mu: f64, dt: f64) -> f64 {
    let distance = r.length();
    let a = -1.0 / (2.0 / distance - v.length_squared() / mu).abs();
    let e_cosh = 1.0 - distance / a;
    let e_sinh = r.dot(v) / (-mu * a).sqrt();
    let e = (e_cosh * e_cosh - e_sinh * e_sinh).sqrt();
    let target = e_sinh - (e_sinh / e).asinh() + (mu / -(a * a * a)).sqrt() * dt;
    // Newton's method; e sinh H − H is convex for H > 0, so it converges
    // from a start on the far side of the root.
    let mut h = (target / e).asinh() + target.signum();
    for _ in 0..100 {
        let step = (e * h.sinh() - h - target) / (e * h.cosh() - 1.0);
        h -= step;
        if step.abs() <= 1e-15 * h.abs().max(1.0) {
            break;
        }
    }
    -a * (e * h.cosh() - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GM_SUN;

    #[test]
    fn the_water_ice_curve_is_one_at_one_au() {
        // α is chosen so that g(1 AU) = 1; the published constants carry
        // 4 to 5 significant figures, so it lands within 10⁻⁴ of 1.
        let model = NonGravitational::water_ice(1e-9, 0.0, 0.0);
        let g = model.g(AU);
        println!("g(1 AU) = {g:.6}");
        assert!((g - 1.0).abs() < 1e-4);
        // Beyond about 3 AU water ice stops sublimating: g falls steeply.
        assert!(model.g(5.0 * AU) < 1e-2 * model.g(2.0 * AU));
    }

    #[test]
    fn the_components_point_the_right_ways() {
        // On a prograde circle in the ecliptic, A1 pushes away from the
        // Sun, A2 along the motion and A3 north.
        let r = DVec3::new(AU, 0.0, 0.0);
        let v = DVec3::new(0.0, 29_780.0, 0.0);
        let unit = AU / (DAY * DAY);
        for (strengths, expected) in [
            ((1e-9, 0.0, 0.0), DVec3::X),
            ((0.0, 1e-9, 0.0), DVec3::Y),
            ((0.0, 0.0, 1e-9), DVec3::Z),
        ] {
            let model = NonGravitational::water_ice(strengths.0, strengths.1, strengths.2);
            let a = model.acceleration(r, v, GM_SUN);
            assert!((a.normalize() - expected).length() < 1e-12, "{a}");
            assert!((a.length() / (1e-9 * unit * model.g(AU)) - 1.0).abs() < 1e-12);
        }
    }

    #[test]
    fn a_delay_uses_the_distance_from_earlier_in_the_orbit() {
        // Leaving perihelion, the distance some days earlier was smaller,
        // so a delayed model pushes harder than an undelayed one.
        let (rp, vp) = crate::orbit::periapsis_state(2.0 * AU, 0.5, GM_SUN);
        let mut orbit = MeanElements::osculating(rp, vp, GM_SUN, DMat3::IDENTITY, 0.0);
        orbit.period = std::f64::consts::TAU * (orbit.a.powi(3) / GM_SUN).sqrt();
        let (r, v) = orbit.state_at(20.0 * DAY);
        let plain = NonGravitational::water_ice(1e-9, 0.0, 0.0);
        let delayed = NonGravitational {
            delay: 10.0 * DAY,
            ..plain
        };
        assert!(
            delayed.acceleration(r, v, GM_SUN).length() > plain.acceleration(r, v, GM_SUN).length()
        );
    }

    #[test]
    fn an_unbound_orbit_is_followed_back_along_its_hyperbola() {
        // An interstellar comet like 3I/ATLAS (perihelion 1.36 AU,
        // e = 6.14): integrate 10 days on from perihelion, then step back
        // 10 days analytically. Both are exact up to rounding, and IAS15
        // keeps its error near 10⁻¹² of the orbit's size (integrators.md),
        // so they must agree to 10⁻¹⁰.
        use crate::gravity::Newtonian;
        use crate::integrator::{Ias15, advance};
        use crate::{Body, System};
        let (q, e) = (1.36 * AU, 6.14);
        let speed = (GM_SUN * (1.0 + e) / q).sqrt();
        let mut system = System::new(vec![
            Body::new("Sun", GM_SUN, 0.0),
            Body::new("comet", 0.0, 0.0)
                .at(DVec3::new(q, 0.0, 0.0))
                .moving(DVec3::new(0.0, speed, 0.0)),
        ]);
        let dt = 10.0 * DAY;
        advance(&mut system, &Newtonian, &mut Ias15::new(), dt);
        let (r, v) = (system.bodies[1].position, system.bodies[1].velocity);
        let back = hyperbolic_distance(r, v, GM_SUN, -dt);
        let forward = hyperbolic_distance(DVec3::new(q, 0.0, 0.0), DVec3::Y * speed, GM_SUN, dt);
        let straight = (r - v * dt).length();
        println!(
            "back to perihelion: {:.3e}; forward: {:.3e}; a straight line misses by {:.3e}",
            back / q - 1.0,
            forward / r.length() - 1.0,
            straight / q - 1.0
        );
        assert!((back / q - 1.0).abs() < 1e-10);
        assert!((forward / r.length() - 1.0).abs() < 1e-10);
    }
}
