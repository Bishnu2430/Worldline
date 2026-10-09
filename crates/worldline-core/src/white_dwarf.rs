//! White dwarfs: stars held up not by heat but by the pressure of
//! degenerate electrons, packed so tightly that quantum mechanics keeps
//! them apart. See `docs/physics/white-dwarfs.md`.
//!
//! The pressure of a cold, ideal electron gas, as a function of its
//! density, is exact (Chandrasekhar 1935). Heavier white dwarfs are denser,
//! so more of their electrons move near light speed, which makes the gas
//! softer; past a certain mass, no density holds the star up. That mass is
//! Chandrasekhar's limit (Chandrasekhar 1931, *ApJ* 74, 81).
//!
//! "Ideal" here means as Chandrasekhar computed it: electrons at zero
//! temperature, no electrostatic (Coulomb) corrections, Newtonian gravity,
//! and no electron captures by nuclei.

use std::f64::consts::PI;

use crate::constants::{ATOMIC_MASS_UNIT, C, ELECTRON_MASS, G, HBAR};

/// Electrons per unit mass of carbon or oxygen: μₑ = A/Z = 2 atomic mass
/// units per electron.
pub const CARBON_OXYGEN: f64 = 2.0;

/// The solution of an equation of the Lane–Emden type,
/// (1/η²) d/dη(η² dφ/dη) = −f(φ), φ(0) = 1, φ′(0) = 0, out to where φ
/// falls to `surface`: (η₁, −η₁² φ′(η₁)), the star's radius and mass in
/// the equation's units.
fn solve(f: impl Fn(f64) -> f64, surface: f64) -> (f64, f64) {
    // The state is (φ, χ) with χ = η² φ′: φ′ = χ/η², χ′ = −η² f(φ).
    let rate = |eta: f64, s: [f64; 2]| [s[1] / (eta * eta), -eta * eta * f(s[0].max(surface))];
    let rk4 = |eta: f64, s: [f64; 2], h: f64| {
        let add = |s: [f64; 2], k: [f64; 2], c: f64| [s[0] + c * k[0], s[1] + c * k[1]];
        let k1 = rate(eta, s);
        let k2 = rate(eta + h / 2.0, add(s, k1, h / 2.0));
        let k3 = rate(eta + h / 2.0, add(s, k2, h / 2.0));
        let k4 = rate(eta + h, add(s, k3, h));
        [
            s[0] + h / 6.0 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]),
            s[1] + h / 6.0 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]),
        ]
    };
    // Start just off the center with the series φ = 1 − f(1) η²/6.
    let f0 = f(1.0);
    let mut eta = 1e-4;
    let mut s = [1.0 - f0 * eta * eta / 6.0, -f0 * eta.powi(3) / 3.0];
    let mut h = 1e-3;
    let tolerance = 1e-13;
    loop {
        // Step doubling: one step of h against two of h/2.
        let whole = rk4(eta, s, h);
        let half = rk4(eta + h / 2.0, rk4(eta, s, h / 2.0), h / 2.0);
        let error = (half[0] - whole[0]).abs() + (half[1] - whole[1]).abs() / (1.0 + s[1].abs());
        if error > tolerance && h > 1e-9 {
            h /= 2.0;
            continue;
        }
        if half[0] <= surface {
            // The surface lies within this step: find it by Newton's
            // method on the step's length.
            let mut step = h / 2.0;
            for _ in 0..50 {
                let end = rk4(eta, s, step);
                let slope = end[1] / ((eta + step) * (eta + step));
                let correction = (end[0] - surface) / slope;
                step -= correction;
                if correction.abs() < 1e-15 * (eta + step) {
                    break;
                }
            }
            let end = rk4(eta, s, step);
            return (eta + step, -end[1]);
        }
        eta += h;
        s = half;
        if error < tolerance / 64.0 {
            h *= 2.0;
        }
    }
}

/// A polytrope's surface and mass in Lane–Emden units: the first zero ξ₁
/// of θ, and −ξ₁² θ′(ξ₁), for (1/ξ²) d/dξ(ξ² dθ/dξ) = −θⁿ (Emden 1907;
/// Chandrasekhar 1939, *An Introduction to the Study of Stellar
/// Structure*, chapter IV). For n = 3, a fully relativistic degenerate
/// gas, ξ₁ = 6.89685 and −ξ₁²θ′ = 2.01824; for n = 3/2, a non-relativistic
/// one, 3.65375 and 2.71406.
pub fn lane_emden(n: f64) -> (f64, f64) {
    solve(|theta| theta.max(0.0).powf(n), 0.0)
}

/// The scales of a white dwarf of μₑ atomic mass units per electron:
/// k = μₑ mᵤ/(mₑc²), in 1/(m²/s²), which turns the electrons' energy into
/// gravitational potential, and ρ₀ = μₑ mᵤ/(3π² λₑ³), the density at which
/// the electrons' Fermi momentum is mₑc (λₑ = ħ/(mₑc)), in kg/m³.
fn scales(mu_e: f64) -> (f64, f64) {
    let lambda = HBAR / (ELECTRON_MASS * C);
    let k = mu_e * ATOMIC_MASS_UNIT / (ELECTRON_MASS * C * C);
    let rho0 = mu_e * ATOMIC_MASS_UNIT / (3.0 * PI * PI * lambda.powi(3));
    (k, rho0)
}

/// Chandrasekhar's limit, the most a cold, ideal white dwarf of μₑ atomic
/// mass units per electron can weigh, as a gravitational parameter G M
/// (m³/s²): G (√(3π)/2) ω₃ (ħc/G)^(3/2)/(μₑ mᵤ)², where ω₃ = −ξ₁²θ′(ξ₁)
/// for the n = 3 polytrope, which the star becomes as its density grows
/// without bound. 1.456 Suns for carbon and oxygen (μₑ = 2).
pub fn chandrasekhar_limit(mu_e: f64) -> f64 {
    let (_, omega) = lane_emden(3.0);
    G * (3.0 * PI).sqrt() / 2.0 * omega * (HBAR * C / G).powf(1.5)
        / (mu_e * ATOMIC_MASS_UNIT).powi(2)
}

/// A cold, ideal white dwarf.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WhiteDwarf {
    /// Its gravitational parameter G M, m³/s².
    pub gm: f64,
    /// Its radius, m.
    pub radius: f64,
    /// Its density at the center, kg/m³.
    pub central_density: f64,
}

/// The cold, ideal white dwarf of μₑ atomic mass units per electron with
/// central density `central_density` (kg/m³): Chandrasekhar's (1939,
/// chapter XI) equation for a star of degenerate electrons in hydrostatic
/// equilibrium,
///
/// (1/η²) d/dη(η² dφ/dη) = −(φ² − 1/y₀²)^(3/2),
///
/// where y = √(1 + x²) is the electrons' chemical potential in units of
/// mₑc² (x their Fermi momentum over mₑc), y₀ its central value, φ = y/y₀,
/// and r = η / (y₀ √(4π k G ρ₀)). The surface is where y = 1. Its mass is
/// −η₁² φ′(η₁) / (k G √(4π k G ρ₀)).
pub fn white_dwarf(central_density: f64, mu_e: f64) -> WhiteDwarf {
    let (k, rho0) = scales(mu_e);
    let y0 = (1.0 + (central_density / rho0).powf(2.0 / 3.0)).sqrt();
    let floor = 1.0 / (y0 * y0);
    let (eta, minus_chi) = solve(|phi| (phi * phi - floor).max(0.0).powf(1.5), 1.0 / y0);
    let unit = (4.0 * PI * k * G * rho0).sqrt();
    WhiteDwarf {
        gm: G * minus_chi / (k * G * unit),
        radius: eta / (y0 * unit),
        central_density,
    }
}

/// The radius (m) of the cold, ideal white dwarf of μₑ atomic mass units
/// per electron and gravitational parameter `gm`, if it can exist: lighter
/// than [`chandrasekhar_limit`]. Heavier white dwarfs are smaller. Found by
/// bisection on the central density, along which the mass rises
/// steadily.
pub fn radius_for(gm: f64, mu_e: f64) -> Option<f64> {
    if gm <= 0.0 || gm >= chandrasekhar_limit(mu_e) {
        return None;
    }
    let (mut low, mut high) = (1e-3f64.ln(), 1e20f64.ln());
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if white_dwarf(middle.exp(), mu_e).gm < gm {
            low = middle;
        } else {
            high = middle;
        }
        if high - low < 1e-13 {
            break;
        }
    }
    Some(white_dwarf((0.5 * (low + high)).exp(), mu_e).radius)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{GM_SUN, SOLAR_RADIUS};

    #[test]
    fn the_lane_emden_constants_come_out() {
        // Exact for n = 0 (√6, 2√6) and n = 1 (π, π); for n = 3/2 and 3,
        // Chandrasekhar's (1939) table to 5 decimals, so within half a unit
        // of the last.
        for (n, xi, omega) in [
            (0.0, 6f64.sqrt(), 2.0 * 6f64.sqrt()),
            (1.0, PI, PI),
            (1.5, 3.65375, 2.71406),
            (3.0, 6.89685, 2.01824),
        ] {
            let (x, w) = lane_emden(n);
            println!("n = {n}: ξ₁ = {x:.6}, −ξ₁²θ′ = {w:.6}");
            assert!((x - xi).abs() <= 5e-6 && (w - omega).abs() <= 5e-6);
        }
    }

    #[test]
    fn heavier_white_dwarfs_are_smaller_and_none_passes_the_limit() {
        let limit = chandrasekhar_limit(CARBON_OXYGEN);
        let small = radius_for(0.6 * GM_SUN, CARBON_OXYGEN).unwrap();
        let big = radius_for(1.2 * GM_SUN, CARBON_OXYGEN).unwrap();
        println!(
            "0.6 Suns: {:.0} km; 1.2 Suns: {:.0} km; limit {:.4} Suns",
            small / 1e3,
            big / 1e3,
            limit / GM_SUN
        );
        assert!(big < small && small < 0.02 * SOLAR_RADIUS);
        assert_eq!(radius_for(limit, CARBON_OXYGEN), None);
        assert_eq!(radius_for(1.5 * GM_SUN, CARBON_OXYGEN), None);
    }
}
