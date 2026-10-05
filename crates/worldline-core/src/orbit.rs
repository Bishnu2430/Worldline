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

/// Points around the osculating orbit: the ellipse a body at relative
/// position `r` (m) and velocity `v` (m/s) would trace if only the central
/// gravity `mu` (m³/s²) acted on it. Returns `count` + 1 points covering
/// one full turn, ending at `r` itself, or `None` if the orbit isn't bound.
///
/// From the eccentricity vector e = ((v² − μ/r) r − (r·v) v)/μ, which
/// points at periapsis, and the conic r(ν) = p / (1 + e cos ν) with
/// p = h²/μ (Murray & Dermott, *Solar System Dynamics*, 1999, §2.4–2.5).
pub fn osculating_orbit(r: DVec3, v: DVec3, mu: f64, count: usize) -> Option<Vec<DVec3>> {
    let h = r.cross(v);
    let e_vec = ((v.length_squared() - mu / r.length()) * r - r.dot(v) * v) / mu;
    let e = e_vec.length();
    if e >= 1.0 || h.length_squared() == 0.0 || count == 0 {
        return None;
    }
    let normal = h.normalize();
    // Toward periapsis; any direction in the plane will do for a circle.
    let p_axis = if e > 1e-12 { e_vec / e } else { r.normalize() };
    let q_axis = normal.cross(p_axis);
    let semi_latus = h.length_squared() / mu;
    let now = r.dot(q_axis).atan2(r.dot(p_axis));
    Some(
        (0..=count)
            .map(|k| {
                let nu = now - TAU * (1.0 - k as f64 / count as f64);
                let (sin, cos) = nu.sin_cos();
                (p_axis * cos + q_axis * sin) * (semi_latus / (1.0 + e * cos))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, DAY, GM_SUN};

    #[test]
    fn the_osculating_orbit_passes_through_both_ends_of_the_ellipse() {
        // Start at periapsis of an ellipse: the drawn orbit must end at the
        // body, reach apoapsis a(1 + e) halfway round, and stay in the plane.
        let (a, e, mu) = (4.2e8, 0.3, 1.27e17);
        let (r, v) = periapsis_state(a, e, mu);
        let points = osculating_orbit(r, v, mu, 360).unwrap();
        assert_eq!(points.len(), 361);
        assert!((points[360] - r).length() < 1e-6 * a);
        assert!((points[0] - r).length() < 1e-6 * a);
        assert!((points[180].length() - a * (1.0 + e)).abs() < 1e-6 * a);
        assert!(points.iter().all(|p| p.z.abs() < 1e-6 * a));
        // An escaping body has no ellipse.
        assert!(osculating_orbit(r, v * 1.5, mu, 360).is_none());
    }

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
