//! Sunlight: how bright it is, and how long it takes to arrive. See
//! `docs/physics/sun-reach.md`.

use glam::DVec3;

use crate::constants::{C, SOLAR_LUMINOSITY};

/// Sunlight's power per unit area (W/m²) facing the Sun at `distance` (m)
/// from it: the luminosity spread over a sphere, L / 4πr².
pub fn irradiance(distance: f64) -> f64 {
    SOLAR_LUMINOSITY / (4.0 * std::f64::consts::PI * distance * distance)
}

/// The extra time (s) light takes to pass a body of gravitational parameter
/// `mu` (m³/s²), because gravity slows clocks and stretches space near it:
/// the Shapiro delay for a straight path from `from` to `to`, positions
/// relative to the body (m). In general relativity (PPN γ = 1),
///
/// Δt = (2μ/c³) ln((r₁ + r₂ + d) / (r₁ + r₂ − d)),
///
/// with r₁, r₂ the distances from the body and d the path length (Shapiro,
/// "Fourth test of general relativity", *Phys. Rev. Lett.* 13, 789, 1964).
pub fn shapiro_delay(mu: f64, from: DVec3, to: DVec3) -> f64 {
    let (r1, r2, d) = (from.length(), to.length(), (to - from).length());
    2.0 * mu / (C * C * C) * ((r1 + r2 + d) / (r1 + r2 - d)).ln()
}

/// How long light takes to travel from `from` to `to` (m, relative to a
/// body of gravitational parameter `mu` that it passes): the distance over
/// c, plus the Shapiro delay.
pub fn light_time(mu: f64, from: DVec3, to: DVec3) -> f64 {
    (to - from).length() / C + shapiro_delay(mu, from, to)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, GM_SUN, SOLAR_RADIUS};

    #[test]
    fn sunlight_falls_off_as_the_square_of_distance() {
        let at_earth = irradiance(AU);
        println!("sunlight at 1 AU: {at_earth:.2} W/m²");
        // IAU's luminosity is 4π AU² × 1361 W/m² rounded to four digits,
        // so this lands within that rounding: half a unit in 3828.
        assert!((at_earth / 1361.0 - 1.0).abs() < 0.5 / 3828.0);
        assert!((irradiance(2.0 * AU) * 4.0 / at_earth - 1.0).abs() < 1e-15);
    }

    #[test]
    fn the_shapiro_delay_is_the_integral_of_gravitys_slowing_along_the_path() {
        // In the weak field, light is delayed by (2μ/c³) ∫ ds / r along its
        // path. Integrate that numerically along a path grazing the Sun and
        // compare with the closed form.
        let (from, to) = (
            DVec3::new(-AU, SOLAR_RADIUS, 0.0),
            DVec3::new(1.5 * AU, SOLAR_RADIUS, 0.0),
        );
        let steps = 2_000_000;
        let h = 1.0 / steps as f64;
        let integral: f64 = (0..steps)
            .map(|k| {
                let p = from.lerp(to, (k as f64 + 0.5) * h);
                h * (to - from).length() / p.length()
            })
            .sum();
        let numeric = 2.0 * GM_SUN / (C * C * C) * integral;
        let exact = shapiro_delay(GM_SUN, from, to);
        println!("grazing the Sun, one way: {:.1} µs", exact * 1e6);
        // The midpoint rule's error is about h²/24 times the curvature of
        // 1/r near the Sun, under 10⁻⁶ of the result here.
        assert!((numeric / exact - 1.0).abs() < 1e-6, "{numeric} vs {exact}");
    }
}
