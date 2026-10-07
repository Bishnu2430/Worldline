//! Carrying a body along its two-body orbit for any length of time, on any
//! kind of orbit: ellipse, parabola or hyperbola.
//!
//! Universal variables (Danby 1988, *Fundamentals of Celestial Mechanics*,
//! §6.9; Battin 1999, *An Introduction to the Mathematics and Methods of
//! Astrodynamics*, §4.5) solve one form of Kepler's equation for all three
//! kinds, so a body can be carried across the switch from bound to unbound
//! without special cases. This is the "drift" of the particle swarm's
//! integrator (see `swarm.rs`).

use std::f64::consts::TAU;

use glam::DVec3;

/// Stumpff's functions c₂(z) and c₃(z): (1 − cos √z)/z and
/// (√z − sin √z)/√z³, continued to z ≤ 0 with cosh and sinh, and by their
/// series near zero, where the closed forms lose precision.
fn stumpff(z: f64) -> (f64, f64) {
    if z.abs() < 1e-3 {
        // c₂ = Σ (−z)^k/(2k+2)!, c₃ = Σ (−z)^k/(2k+3)!: four terms reach
        // rounding for |z| < 10⁻³.
        let c2 = 1.0 / 2.0 - z / 24.0 + z * z / 720.0 - z * z * z / 40_320.0;
        let c3 = 1.0 / 6.0 - z / 120.0 + z * z / 5040.0 - z * z * z / 362_880.0;
        (c2, c3)
    } else if z > 0.0 {
        let s = z.sqrt();
        ((1.0 - s.cos()) / z, (s - s.sin()) / (s * z))
    } else {
        let s = (-z).sqrt();
        ((s.cosh() - 1.0) / -z, (s.sinh() - s) / (s * -z))
    }
}

/// What happened along a drift.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drift {
    /// Position relative to the center at the end, in m.
    pub position: DVec3,
    /// Velocity relative to the center at the end, in m/s.
    pub velocity: DVec3,
    /// The closest the body came to the center during the drift, in m.
    pub closest: f64,
}

/// Carries a body at `position` and `velocity` relative to a center of
/// gravitational parameter `mu` along its two-body orbit for `dt` seconds
/// (negative to go back), and reports how close it came to the center.
pub fn drift(position: DVec3, velocity: DVec3, mu: f64, dt: f64) -> Drift {
    let r0 = position.length();
    let sqrt_mu = mu.sqrt();
    // σ₀ = r·v/√μ, and α = 1/a: positive for an ellipse, zero for a
    // parabola, negative for a hyperbola.
    let sigma = position.dot(velocity) / sqrt_mu;
    let alpha = 2.0 / r0 - velocity.length_squared() / mu;
    // Whether the drift passes periapsis. On an ellipse, from the mean
    // anomaly M (E − e sin E, with e cos E = 1 − α r₀ and
    // e sin E = σ₀ √α): it does if the drift lasts longer than the time to
    // the next periapsis (or since the last, going back). Whole turns of an
    // ellipse change nothing else, so they are dropped.
    let full = dt;
    let (dt, ellipse_passes) = if alpha > 0.0 {
        let period = TAU / (sqrt_mu * alpha.powf(1.5));
        let e_cos = 1.0 - alpha * r0;
        let e_sin = sigma * alpha.sqrt();
        let anomaly = e_sin.atan2(e_cos).rem_euclid(TAU);
        let mean = (anomaly - e_sin).rem_euclid(TAU);
        let passes = if full.abs() >= period {
            true
        } else if full >= 0.0 {
            full >= (TAU - mean) / TAU * period
        } else {
            -full >= mean / TAU * period
        };
        (dt - period * (dt / period).round(), Some(passes))
    } else {
        (dt, None)
    };
    // Kepler's equation in the universal variable χ:
    // √μ Δt = σ₀ χ² c₂ + (1 − α r₀) χ³ c₃ + r₀ χ, whose derivative in χ is
    // the distance r. Solved by Laguerre's method (Conway 1986), which
    // converges from any start on such equations.
    let kepler = |chi: f64| {
        let z = alpha * chi * chi;
        let (c2, c3) = stumpff(z);
        let f = sigma * chi * chi * c2 + (1.0 - alpha * r0) * chi.powi(3) * c3 + r0 * chi
            - sqrt_mu * dt;
        let r = sigma * chi * (1.0 - z * c3) + (1.0 - alpha * r0) * chi * chi * c2 + r0;
        let dr = sigma * (1.0 - z * c2) + (1.0 - alpha * r0) * chi * (1.0 - z * c3);
        (f, r, dr, c2, c3)
    };
    let mut chi = if alpha > 0.0 {
        sqrt_mu * dt * alpha
    } else {
        sqrt_mu * dt / r0
    };
    const N: f64 = 5.0;
    for _ in 0..60 {
        let (f, r, dr, _, _) = kepler(chi);
        let root = ((N - 1.0) * (N - 1.0) * r * r - N * (N - 1.0) * f * dr)
            .abs()
            .sqrt();
        let step = N * f / (r + r.signum() * root);
        chi -= step;
        if step.abs() <= 1e-15 * chi.abs().max(1e-300) {
            break;
        }
    }
    let (_, _, _, c2, c3) = kepler(chi);
    let z = alpha * chi * chi;
    // The Lagrange coefficients f, g and their rates.
    let f = 1.0 - chi * chi * c2 / r0;
    let g = dt - chi.powi(3) * c3 / sqrt_mu;
    let new_position = position * f + velocity * g;
    let distance = new_position.length();
    let f_dot = sqrt_mu / (distance * r0) * (z * c3 - 1.0) * chi;
    let g_dot = 1.0 - chi * chi * c2 / distance;
    // Closest approach: periapsis, if the drift passed it, otherwise one
    // of the ends. Off an ellipse it passes periapsis if it was coming in
    // and ends going out.
    let mut closest = r0.min(distance);
    let new_velocity = position * f_dot + velocity * g_dot;
    let passes = ellipse_passes.unwrap_or_else(|| {
        sigma * full.signum() < 0.0 && new_position.dot(new_velocity) * full.signum() > 0.0
    });
    if passes {
        let h = position.cross(velocity).length_squared();
        let e = (1.0 - alpha * h / mu).max(0.0).sqrt();
        closest = closest.min(h / mu / (1.0 + e));
    }
    Drift {
        position: new_position,
        velocity: new_velocity,
        closest,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, DAY, GM_SUN};
    use crate::orbit::{Elements, KeplerOrbit, periapsis_state};

    #[test]
    fn an_ellipse_matches_keplers_equation() {
        // An eccentric asteroid orbit, carried 1,000 days in one drift,
        // against the elements-based solution.
        let elements = Elements {
            a: 2.7 * AU,
            e: 0.4,
            inclination: 0.3,
            node: 1.1,
            periapsis: 2.0,
            mean_anomaly: 0.7,
            epoch: 0.0,
        };
        let orbit = KeplerOrbit::new(&elements, GM_SUN);
        let (r, v) = orbit.state_at(0.0);
        for days in [0.5, 37.0, 400.0, 1000.0, -250.0, 1e5] {
            let d = drift(r, v, GM_SUN, days * DAY);
            let (expected, expected_v) = orbit.state_at(days * DAY);
            let error = (d.position - expected).length() / expected.length();
            assert!(error < 1e-10, "{days} days: {error:e}");
            assert!((d.velocity - expected_v).length() < 1e-9 * expected_v.length());
        }
    }

    #[test]
    fn a_hyperbola_keeps_its_energy_and_angular_momentum() {
        // An interstellar visitor at 1 AU, moving at 60 km/s: unbound.
        let (r, v) = (DVec3::new(AU, 0.0, 0.0), DVec3::new(-2e4, 5.6e4, 1e4));
        let energy = |r: DVec3, v: DVec3| 0.5 * v.length_squared() - GM_SUN / r.length();
        for days in [1.0, 30.0, 365.0, -100.0] {
            let d = drift(r, v, GM_SUN, days * DAY);
            let e0 = energy(r, v);
            assert!((energy(d.position, d.velocity) - e0).abs() < 1e-10 * e0.abs());
            let h = r.cross(v);
            assert!((d.position.cross(d.velocity) - h).length() < 1e-10 * h.length());
            // Back again.
            let back = drift(d.position, d.velocity, GM_SUN, -days * DAY);
            assert!((back.position - r).length() < 1e-8 * r.length());
        }
    }

    #[test]
    fn the_closest_approach_is_found_between_the_ends() {
        // From apoapsis of an ellipse with periapsis 0.1 AU, a drift of
        // a whole period passes periapsis; one of a quarter period doesn't.
        let (a, e) = (1.0 * AU, 0.9);
        let (rp, vp) = periapsis_state(a, e, GM_SUN);
        let period = TAU * (a.powi(3) / GM_SUN).sqrt();
        let start = drift(rp, vp, GM_SUN, 0.5 * period);
        let through = drift(start.position, start.velocity, GM_SUN, 0.9 * period);
        assert!((through.closest / (a * (1.0 - e)) - 1.0).abs() < 1e-9);
        let short = drift(start.position, start.velocity, GM_SUN, 0.25 * period);
        assert!(short.closest > 0.5 * a);
    }
}
