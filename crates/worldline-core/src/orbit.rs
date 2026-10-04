//! Two-body (Keplerian) orbit formulas.
//!
//! These are exact solutions of Newtonian gravity for two bodies. They set
//! up scenarios and provide the answers the simulation must reproduce.

use std::f64::consts::TAU;

use glam::DVec3;

use crate::{Body, System};

/// Orbital period from Kepler's third law, T = 2π √(a³ / μ), in s.
///
/// `a` is the semi-major axis of the relative orbit, in m, and `mu` is the
/// combined gravitational parameter μ₁ + μ₂, in m³/s².
pub fn kepler_period(a: f64, mu: f64) -> f64 {
    TAU * (a.powi(3) / mu).sqrt()
}

/// Relative position (m) and velocity (m/s) at periapsis, the closest
/// approach, for a bound orbit with semi-major axis `a` (m), eccentricity
/// `e` (0 ≤ e < 1) and combined gravitational parameter `mu` (m³/s²).
///
/// The orbit lies in the x–y plane with periapsis on the +x axis, moving
/// counterclockwise seen from +z. The speed comes from the vis-viva equation.
pub fn periapsis_state(a: f64, e: f64, mu: f64) -> (DVec3, DVec3) {
    assert!(a > 0.0, "semi-major axis must be positive");
    assert!(
        (0.0..1.0).contains(&e),
        "only bound orbits (0 ≤ e < 1) are supported"
    );
    let r = a * (1.0 - e);
    let v = (mu * (1.0 + e) / r).sqrt();
    (DVec3::new(r, 0.0, 0.0), DVec3::new(0.0, v, 0.0))
}

/// Builds an isolated two-body system in its barycentric frame, with the
/// bodies at periapsis of a bound orbit with semi-major axis `a` (m) and
/// eccentricity `e`.
pub fn two_body_system(primary: Body, secondary: Body, a: f64, e: f64) -> System {
    let (r, v) = periapsis_state(a, e, primary.gm + secondary.gm);
    let mut system = System::new(vec![
        primary.at(DVec3::ZERO).moving(DVec3::ZERO),
        secondary.at(r).moving(v),
    ]);
    system.move_to_barycentric_frame();
    system
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, DAY, GM_SUN};

    #[test]
    fn one_au_around_the_sun_takes_a_gaussian_year() {
        // A massless body 1 AU from the Sun orbits in 365.2568983 days,
        // the "Gaussian year" astronomers used to define the AU until 2012.
        let days = kepler_period(AU, GM_SUN) / DAY;
        assert!((days - 365.256_898_3).abs() < 1e-3, "got {days} days");
    }

    #[test]
    fn periapsis_speed_satisfies_vis_viva() {
        let (a, e, mu) = (2.0e11, 0.4, 1.0e20);
        let (r, v) = periapsis_state(a, e, mu);
        let vis_viva = mu * (2.0 / r.length() - 1.0 / a);
        assert!((v.length_squared() - vis_viva).abs() / vis_viva < 1e-14);
    }
}
