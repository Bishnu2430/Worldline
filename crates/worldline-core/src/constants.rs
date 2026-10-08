//! Physical and astronomical constants, in SI units.
//!
//! Each value cites its source. "Exact" means the value is fixed by the
//! definition of the SI or IAU units, so it has no uncertainty.

use std::f64::consts::PI;

/// Speed of light in vacuum, m/s. Exact (SI 2019).
pub const C: f64 = 299_792_458.0;

/// Vacuum magnetic permeability μ₀, N/A². CODATA 2018 (measured since the
/// SI's 2019 redefinition, no longer exactly 4π × 10⁻⁷).
pub const MU_0: f64 = 1.256_637_062_12e-6;

/// Newtonian constant of gravitation, m³ kg⁻¹ s⁻². CODATA 2018.
///
/// Known to only about 5 significant figures (22 ppm). Gravitational
/// parameters (GM) are measured far more precisely, so bodies store GM
/// instead of mass; see [`crate::Body`].
pub const G: f64 = 6.674_30e-11;

/// Planck constant, J s. Exact (SI 2019).
pub const H: f64 = 6.626_070_15e-34;

/// Reduced Planck constant ħ = h / 2π, J s.
pub const HBAR: f64 = H / (2.0 * PI);

/// Boltzmann constant, J/K. Exact (SI 2019).
pub const K_B: f64 = 1.380_649e-23;

/// Astronomical unit, m. Exact (IAU 2012 Resolution B2).
pub const AU: f64 = 149_597_870_700.0;

/// Parsec, m. Exact by definition: 648 000 / π astronomical units (IAU 2015 Resolution B2).
pub const PARSEC: f64 = 648_000.0 / PI * AU;

/// One day, s.
pub const DAY: f64 = 86_400.0;

/// Julian year (365.25 days), s. The standard year in astronomy.
pub const JULIAN_YEAR: f64 = 365.25 * DAY;

/// Light year, m: the distance light travels in one Julian year.
pub const LIGHT_YEAR: f64 = C * JULIAN_YEAR;

/// Nominal solar gravitational parameter GM☉, m³/s². IAU 2015 Resolution B3.
pub const GM_SUN: f64 = 1.327_124_4e20;

/// Nominal solar radius, m. IAU 2015 Resolution B3.
pub const SOLAR_RADIUS: f64 = 6.957e8;

/// Nominal solar luminosity, W. IAU 2015 Resolution B3: 4π (1 AU)² times the
/// nominal total solar irradiance, 1361 W/m², rounded to four digits.
pub const SOLAR_LUMINOSITY: f64 = 3.828e26;

/// Solar mass, kg, derived as GM☉ / G. Inherits G's 22 ppm uncertainty.
pub const SOLAR_MASS: f64 = GM_SUN / G;

/// The age of the universe, s: 13.787 ± 0.020 billion years (Planck
/// Collaboration 2020, *A&A* 641, A6, Table 2).
pub const AGE_OF_UNIVERSE: f64 = 13.787e9 * JULIAN_YEAR;

/// Julian Date of the J2000 epoch, 2000-01-01 12:00 TDB.
pub const J2000_JD: f64 = 2_451_545.0;

/// Obliquity of the ecliptic at J2000, rad: 84381.448″ (IAU 1976). The
/// tilt between the equatorial and ecliptic frames, as JPL Horizons uses.
pub const OBLIQUITY_J2000: f64 = 84_381.448 / 3600.0 * PI / 180.0;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, rel_tol: f64) {
        let rel = ((actual - expected) / expected).abs();
        assert!(
            rel < rel_tol,
            "{actual} vs {expected}: relative error {rel:e}"
        );
    }

    #[test]
    fn derived_units_match_textbook_values() {
        assert_close(PARSEC, 3.085_677_581e16, 1e-9);
        assert_close(LIGHT_YEAR, 9.460_730_472_580_8e15, 1e-12);
        assert_close(SOLAR_MASS, 1.988_4e30, 1e-4);
        assert_close(HBAR, 1.054_571_817e-34, 1e-9);
    }
}
