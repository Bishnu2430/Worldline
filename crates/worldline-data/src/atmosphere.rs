//! Atmospheres, as needed to draw them: how thick they are and how they
//! scatter sunlight.

/// A clear (Rayleigh-scattering) atmosphere whose density falls off
/// exponentially with height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atmosphere {
    /// Height of the top of the atmosphere above the surface, in m.
    pub height: f64,
    /// Height over which the density falls by a factor e, in m.
    pub scale_height: f64,
    /// Rayleigh scattering coefficients at the surface, per m, for red
    /// (680 nm), green (550 nm) and blue (440 nm) light.
    pub rayleigh: [f64; 3],
}

/// The atmosphere of a body, if Worldline models it.
///
/// Earth: scattering coefficients and scale height from Bruneton & Neyret
/// (2008), "Precomputed Atmospheric Scattering", Computer Graphics Forum
/// 27(4), the standard values for the sea-level air of a clear sky.
pub fn atmosphere(name: &str) -> Option<Atmosphere> {
    match name {
        "Earth" => Some(Atmosphere {
            height: 60e3,
            scale_height: 8e3,
            rayleigh: [5.8e-6, 13.5e-6, 33.1e-6],
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rayleigh_scattering_goes_as_inverse_fourth_power_of_wavelength() {
        // Why the sky is blue: scattering ∝ 1/λ⁴, so blue (440 nm) scatters
        // (680/440)⁴ ≈ 5.7 times more than red (680 nm).
        let [red, green, blue] = atmosphere("Earth").unwrap().rayleigh;
        let law = |a: f64, b: f64| (a / b).powi(4);
        assert!((blue / red / law(680.0, 440.0) - 1.0).abs() < 0.01);
        assert!((green / red / law(680.0, 550.0) - 1.0).abs() < 0.01);
    }
}
