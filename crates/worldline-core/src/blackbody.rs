//! The color of a blackbody (step 8.5b): what a star of a given
//! temperature looks like, if its light were a blackbody's.
//!
//! Planck's law gives the light a blackbody gives off at each wavelength.
//! Weighted by the CIE 1931 colour-matching functions (the 2° observer,
//! CIE 018:2019) and summed over wavelength, it gives the color's CIE XYZ,
//! the standard way to measure a color as the eye sees it. A screen shows
//! sRGB (IEC 61966-2-1:1999), reached from XYZ by a fixed matrix.
//!
//! What's drawn is the color's hue, not its brightness: the color is scaled
//! so its brightest channel is full. A star's real light isn't a perfect
//! blackbody's (its spectral lines and absorption make it a little
//! different), and a color outside what a screen can show (a blackbody
//! cooler than about 1,900 K, redder than the screen's red) is clipped to
//! it. See `docs/physics/star-colors.md`.

use crate::DVec3;
use crate::constants::C;

/// Planck's constant, J s: exact in the SI since 2019.
pub const PLANCK: f64 = 6.626_070_15e-34;
/// Boltzmann's constant, J/K: exact in the SI since 2019.
pub const BOLTZMANN: f64 = 1.380_649e-23;

/// The second radiation constant hc/k, m K: about 1.4388 × 10⁻².
pub fn second_radiation_constant() -> f64 {
    PLANCK * C / BOLTZMANN
}

/// Planck's law at wavelength `lambda` (m) and temperature `t` (K), with
/// second radiation constant `c2`, up to a constant factor: λ⁻⁵ / (e^(c2/λT)
/// − 1). The factor, 2hc², doesn't change a color.
pub fn planck(lambda: f64, t: f64, c2: f64) -> f64 {
    lambda.powi(-5) / (c2 / (lambda * t)).exp_m1()
}

/// The CIE XYZ of light whose spectrum is `spectrum` (of wavelength, m), up
/// to a constant factor: the spectrum weighted by the colour-matching
/// functions `table` (rows of λ in nm, x̄, ȳ, z̄, at equal steps) and summed,
/// as CIE 015:2018 sums colors.
pub fn tristimulus(table: &[[f64; 4]], spectrum: impl Fn(f64) -> f64) -> DVec3 {
    table
        .iter()
        .map(|&[nm, x, y, z]| DVec3::new(x, y, z) * spectrum(nm * 1e-9))
        .sum()
}

/// A blackbody's CIE XYZ at temperature `t` (K), up to a constant factor.
pub fn blackbody(table: &[[f64; 4]], t: f64) -> DVec3 {
    let c2 = second_radiation_constant();
    tristimulus(table, |lambda| planck(lambda, t, c2))
}

/// A color's chromaticity (x, y): its share of X and of Y.
pub fn chromaticity(xyz: DVec3) -> (f64, f64) {
    let sum = xyz.x + xyz.y + xyz.z;
    (xyz.x / sum, xyz.y / sum)
}

/// Linear sRGB from CIE XYZ: IEC 61966-2-1:1999's matrix, for its white,
/// D65.
pub fn linear_srgb(xyz: DVec3) -> DVec3 {
    DVec3::new(
        3.2406 * xyz.x - 1.5372 * xyz.y - 0.4986 * xyz.z,
        -0.9689 * xyz.x + 1.8758 * xyz.y + 0.0415 * xyz.z,
        0.0557 * xyz.x - 0.2040 * xyz.y + 1.0570 * xyz.z,
    )
}

/// sRGB's transfer curve: a linear channel (0 to 1) as a screen encodes
/// it (IEC 61966-2-1:1999).
pub fn encode(linear: f64) -> f64 {
    if linear <= 0.003_130_8 {
        12.92 * linear
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    }
}

/// The color to draw for CIE XYZ `xyz`, as encoded sRGB channels (0 to 1):
/// its hue at full brightness, its brightest linear channel scaled to 1,
/// and any channel the screen can't show (below 0) clipped to 0.
pub fn display(xyz: DVec3) -> [f64; 3] {
    let rgb = linear_srgb(xyz).max(DVec3::ZERO);
    let peak = rgb.max_element();
    if peak <= 0.0 {
        return [0.0; 3];
    }
    let rgb = rgb / peak;
    [encode(rgb.x), encode(rgb.y), encode(rgb.z)]
}

/// Light from the surface of a body of compactness r_s/R climbs out of its
/// gravity, losing energy: seen far away, every wavelength is longer by
/// 1/√(1 − r_s/R), and a blackbody looks as one cooler by √(1 − r_s/R)
/// (Schwarzschild; e.g. Misner, Thorne & Wheeler 1973, §25.4).
pub fn seen_far_away(t: f64, compactness: f64) -> f64 {
    t * (1.0 - compactness).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_second_radiation_constant_is_codatas() {
        // CODATA 2018: c₂ = 1.438 776 877... × 10⁻² m K, exact.
        assert!((second_radiation_constant() / 1.438_776_877e-2 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn d65_white_is_the_screens_white() {
        // D65's XYZ (Y = 1), as IEC 61966-2-1 gives its white point, maps
        // to equal channels. The matrix's entries are printed to four
        // places, rounding each by up to 5 × 10⁻⁵; summed over X + Y + Z =
        // 3.04 that's 1.6 × 10⁻⁴ at most.
        let rgb = linear_srgb(DVec3::new(0.9505, 1.0, 1.0890));
        for channel in rgb.to_array() {
            assert!((channel - 1.0).abs() < 1.6e-4, "{rgb:?}");
        }
        assert_eq!(
            display(DVec3::new(0.9505, 1.0, 1.0890)).map(|c| c > 0.9999),
            [true; 3]
        );
    }

    #[test]
    fn the_transfer_curve_meets_itself() {
        // Its two pieces agree where they join, to the standard's rounding.
        let below = 12.92 * 0.003_130_8;
        let above = 1.055 * 0.003_130_8f64.powf(1.0 / 2.4) - 0.055;
        assert!((below - above).abs() < 1e-7);
        assert_eq!(encode(0.0), 0.0);
        assert!((encode(1.0) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn climbing_out_cools_the_light() {
        assert_eq!(seen_far_away(10_000.0, 0.0), 10_000.0);
        assert!((seen_far_away(10_000.0, 0.75) - 5_000.0).abs() < 1e-9);
    }
}
