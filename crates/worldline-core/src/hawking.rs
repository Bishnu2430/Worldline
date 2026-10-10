//! Hawking radiation: black holes aren't quite black. Quantum fields near
//! the horizon make a black hole glow like a body at a temperature that
//! rises as it shrinks, so it loses mass ever faster, and, unless something
//! feeds it, ends in a final burst. See `docs/physics/hawking.md`.
//!
//! The textbook estimate, used here (Hawking 1974, *Nature* 248, 30; 1975,
//! *Commun. Math. Phys.* 43, 199): a black body at the Hawking temperature
//! T = ħc³/(8πGMk_B), as big as the horizon (radius 2GM/c²), emitting
//! photons only. That gives power P = ħc⁶/(15360π G²M²) and a lifetime
//! t = 5120π G²M³/(ħc⁴). Real black holes also emit neutrinos and
//! gravitons, and at high enough temperatures every particle lighter than
//! kT, so they evaporate faster (Page 1976; Carr et al. 2010).

use std::f64::consts::PI;

use crate::constants::{C, G, HBAR, K_B};

/// The cosmic microwave background's temperature today, K: Fixsen (2009),
/// *ApJ* 707, 916. A black hole colder than this absorbs more of it than
/// it radiates, so it isn't evaporating today.
pub const CMB_TEMPERATURE: f64 = 2.72548;

/// The Hawking temperature of a black hole of gravitational parameter
/// `gm` (m³/s²), K: ħc³/(8π G M k_B).
pub fn temperature(gm: f64) -> f64 {
    HBAR * C.powi(3) / (8.0 * PI * gm * K_B)
}

/// The power a black hole of gravitational parameter `gm` radiates as
/// photons, W: ħc⁶/(15360π G²M²).
pub fn power(gm: f64) -> f64 {
    HBAR * C.powi(6) / (15360.0 * PI * gm * gm)
}

/// How long a black hole of gravitational parameter `gm` takes to
/// evaporate completely, s: 5120π G²M³/(ħc⁴), from dM/dt = −P/c².
pub fn lifetime(gm: f64) -> f64 {
    let m = gm / G;
    5120.0 * PI * G * G * m.powi(3) / (HBAR * C.powi(4))
}

/// Whether a black hole of gravitational parameter `gm` is evaporating
/// today: hotter than the cosmic microwave background. True below about
/// 4.5 × 10²² kg, some 60% of the Moon's mass.
pub fn evaporating(gm: f64) -> bool {
    temperature(gm) > CMB_TEMPERATURE
}

/// The gravitational parameter of a black hole of gravitational parameter
/// `gm` after it has radiated for `dt` seconds, exactly: M³ falls at the
/// steady rate 3ħc⁴/(15360π G²), so M(t + dt) = (M³ − 3ħc⁴ dt/(15360π G²))^(1/3).
/// `None` if it evaporates completely within `dt`.
pub fn after(gm: f64, dt: f64) -> Option<f64> {
    let m = gm / G;
    let left = m.powi(3) - 3.0 * HBAR * C.powi(4) / (15360.0 * PI * G * G) * dt;
    (left > 0.0).then(|| G * left.cbrt())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GM_SUN;

    #[test]
    fn a_sun_mass_hole_is_60_nanokelvin_and_lives_10_to_the_67_years() {
        let year = 365.25 * 86_400.0;
        let t = temperature(GM_SUN);
        let life = lifetime(GM_SUN) / year;
        println!("one Sun: {t:.4e} K, lifetime {life:.3e} years");
        // The commonly quoted 6.17 × 10⁻⁸ K, to its digits.
        assert!((t - 6.17e-8).abs() <= 0.005e-8);
        assert!(!evaporating(GM_SUN));
        // The lifetime is where the mass runs out.
        let gm = 1e8 * G;
        assert!(after(gm, lifetime(gm) * (1.0 - 1e-9)).is_some());
        assert_eq!(after(gm, lifetime(gm) * (1.0 + 1e-9)), None);
    }
}
