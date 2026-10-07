//! The solar wind and the bubble it blows in the interstellar gas: the
//! Parker spiral and the heliosphere's boundaries. See
//! `docs/physics/sun-reach.md`.

use glam::DVec3;

use crate::constants::{AU, SOLAR_RADIUS};

/// The solar wind as Parker described it ("Dynamics of the interplanetary
/// gas and magnetic fields", *ApJ* 128, 664, 1958): gas streaming straight
/// out from the Sun at a steady speed, dragging the Sun's magnetic field
/// with it. The Sun turns beneath the outflow, so each field line winds
/// into an Archimedean spiral, like the water from a turning garden
/// sprinkler, lagging behind the rotation by Ω(r − r₀)/v. It crosses each
/// distance r at an angle ψ to the radial direction with
///
/// tan ψ = Ω r sin θ / v,
///
/// θ being the colatitude from the Sun's spin axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParkerSpiral {
    /// The wind's speed, in m/s.
    pub speed: f64,
    /// The Sun's sidereal rotation rate, in rad/s.
    pub rotation_rate: f64,
    /// The protons per m³ at 1 AU. The wind spreads over spheres, so the
    /// density falls as 1/r².
    pub density_at_1au: f64,
}

impl ParkerSpiral {
    /// Where the field lines start their spiral: the "source surface" at
    /// 2.5 solar radii, beyond which the wind, not the Sun's own field,
    /// shapes the field (Altschuler & Newkirk 1969; Schatten, Wilcox &
    /// Ness 1969).
    pub const SOURCE_SURFACE: f64 = 2.5 * SOLAR_RADIUS;

    /// The angle (rad) between the field and the radial direction at
    /// `distance` (m) from the Sun, at colatitude with sine
    /// `sin_colatitude`.
    pub fn angle(&self, distance: f64, sin_colatitude: f64) -> f64 {
        (self.rotation_rate * distance * sin_colatitude / self.speed).atan()
    }

    /// How far (rad) the field line at `distance` (m) lags behind the
    /// longitude of its root on the source surface.
    pub fn lag(&self, distance: f64) -> f64 {
        self.rotation_rate * (distance - Self::SOURCE_SURFACE).max(0.0) / self.speed
    }

    /// Protons per m³ at `distance` (m) from the Sun.
    pub fn density(&self, distance: f64) -> f64 {
        self.density_at_1au * (AU / distance).powi(2)
    }
}

/// The heliosphere: the bubble the solar wind blows in the interstellar
/// gas, with its two boundaries. At the **termination shock** the wind
/// drops abruptly from supersonic to subsonic; at the **heliopause** it
/// meets the interstellar gas. Each has been measured at just two points,
/// where Voyager 1 and 2 crossed it.
///
/// Between those points its shape is drawn from the simplest flow model:
/// a source blowing into a uniform stream, whose dividing surface is a
/// Rankine half-body,
///
/// r(θ) = r₀ √(2 / (1 + cos θ)),
///
/// θ being the angle from the nose (the direction the interstellar wind
/// comes from). It opens downstream into an endless tail. The real
/// heliosphere is squeezed asymmetrically by the interstellar magnetic
/// field, and its tail has never been measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heliosphere {
    /// Toward the nose: where the interstellar wind comes from.
    pub nose: DVec3,
    /// The termination shock's distance at the nose, in m.
    pub termination_shock: f64,
    /// The heliopause's distance at the nose, in m.
    pub heliopause: f64,
}

impl Heliosphere {
    /// The shape factor r(θ)/r₀ in direction `direction`, or infinity
    /// straight down the tail.
    pub fn shape(&self, direction: DVec3) -> f64 {
        let cos = direction.normalize().dot(self.nose);
        (2.0 / (1.0 + cos)).sqrt()
    }

    /// The nose distance (m) of a Rankine half-body that best fits
    /// `crossings` (positions relative to the Sun, m): the geometric mean
    /// of each crossing's distance divided by its shape factor, so that the
    /// misfits, as ratios, balance.
    pub fn fit(nose: DVec3, crossings: &[DVec3]) -> f64 {
        let shell = Self {
            nose,
            termination_shock: 0.0,
            heliopause: 0.0,
        };
        let mean_log = crossings
            .iter()
            .map(|c| (c.length() / shell.shape(*c)).ln())
            .sum::<f64>()
            / crossings.len() as f64;
        mean_log.exp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_spiral_crosses_1_au_at_45_degrees_for_a_typical_wind() {
        // At 400 km/s and the Sun's 25.38-day sidereal rotation,
        // tan ψ = Ω r / v = 1.07 at 1 AU in the Sun's equatorial plane.
        let wind = ParkerSpiral {
            speed: 400e3,
            rotation_rate: std::f64::consts::TAU / (25.38 * 86_400.0),
            density_at_1au: 5e6,
        };
        let angle = wind.angle(AU, 1.0).to_degrees();
        let expected = (wind.rotation_rate * AU / 400e3).atan().to_degrees();
        println!("Parker angle at 1 AU for 400 km/s: {angle:.2}°");
        assert_eq!(angle, expected);
        // Over its first turn, a field line reaches ν = 2πv/Ω farther out.
        let turn = std::f64::consts::TAU * wind.speed / wind.rotation_rate;
        assert!(
            (wind.lag(ParkerSpiral::SOURCE_SURFACE + turn) - std::f64::consts::TAU).abs() < 1e-12
        );
        assert!((wind.density(2.0 * AU) * 4.0 / wind.density_at_1au - 1.0).abs() < 1e-15);
    }

    #[test]
    fn the_rankine_shape_is_where_the_two_flows_part() {
        // A source of strength m in a uniform stream U: the flow
        // u = −U n̂ + m r̂ / 4πr² stops at the nose, r₀² = m / 4πU, and the
        // surface r(θ) = r₀ √(2 / (1 + cos θ)) is the streamline that
        // divides the two flows: nowhere does the flow cross it.
        let nose = DVec3::new(0.3, -0.5, 0.8).normalize();
        let shell = Heliosphere {
            nose,
            termination_shock: 0.0,
            heliopause: 0.0,
        };
        let (u, r0) = (1.0, 2.0);
        let m = 4.0 * std::f64::consts::PI * u * r0 * r0;
        let side = nose.any_orthonormal_vector();
        for degrees in [0.0_f64, 30.0, 60.0, 90.0, 120.0, 150.0] {
            let t = degrees.to_radians();
            let direction = nose * t.cos() + side * t.sin();
            let x = direction * r0 * shell.shape(direction);
            let flow =
                -nose * u + x.normalize() * m / (4.0 * std::f64::consts::PI * x.length_squared());
            // The surface F = |x|² + |x|(x·n̂) − 2r₀² = 0, and its normal.
            let normal = 2.0 * x + x.normalize() * x.dot(nose) + nose * x.length();
            // Relative to the stream's speed (the flow stops at the nose).
            let crossing = flow.dot(normal.normalize()) / u;
            assert!(crossing.abs() < 1e-12, "{degrees}°: {crossing}");
        }
        assert_eq!(shell.shape(nose), 1.0);
        assert!((shell.shape(side) - 2f64.sqrt()).abs() < 1e-15);
    }

    #[test]
    fn a_fit_through_one_crossing_passes_through_it() {
        let nose = DVec3::Y;
        let crossing = DVec3::new(30.0, 90.0, 40.0) * AU;
        let shell = Heliosphere {
            nose,
            termination_shock: Heliosphere::fit(nose, &[crossing]),
            heliopause: 0.0,
        };
        let r = shell.termination_shock * shell.shape(crossing);
        assert!((r / crossing.length() - 1.0).abs() < 1e-15);
    }
}
