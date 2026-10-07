//! Planets' magnetic fields and the cavities they carve in the solar wind:
//! magnetospheres and radiation belts. See `docs/physics/magnetospheres.md`.

use glam::DVec3;

use crate::constants::MU_0;

/// A planet's internal magnetic field to first order: a dipole, given as
/// field models publish it, by its Schmidt-normalized Gauss coefficients of
/// degree 1 at a reference radius. The scalar potential is then
/// V = R (R/r)² (g₁⁰ cos θ + (g₁¹ cos φ + h₁¹ sin φ) sin θ), and B = −∇V.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dipole {
    /// g₁⁰, in T.
    pub g10: f64,
    /// g₁¹, in T.
    pub g11: f64,
    /// h₁¹, in T.
    pub h11: f64,
    /// The model's reference radius, in m.
    pub radius: f64,
    /// How far north of the planet's center the dipole sits, along the
    /// spin axis, in m (Mercury's is offset; the others are centered).
    pub north_offset: f64,
}

impl Dipole {
    /// The field strength on the magnetic equator at the reference radius
    /// (T): √(g₁⁰² + g₁¹² + h₁¹²).
    pub fn equatorial_field(&self) -> f64 {
        (self.g10 * self.g10 + self.g11 * self.g11 + self.h11 * self.h11).sqrt()
    }

    /// The direction of the dipole moment in the planet's body-fixed frame
    /// (x toward longitude 0, z along the spin axis): (g₁¹, h₁¹, g₁⁰)
    /// normalized. Earth's and Mercury's point south; the giant planets'
    /// north.
    pub fn moment_direction(&self) -> DVec3 {
        DVec3::new(self.g11, self.h11, self.g10).normalize()
    }

    /// The angle between the dipole's axis and the spin axis (rad).
    pub fn tilt(&self) -> f64 {
        (self.g10.abs() / self.equatorial_field()).acos()
    }

    /// The field's magnitude (T) at `position` (m, body-fixed, from the
    /// dipole's center): B₀ (R/r)³ √(1 + 3 cos² ϑ), ϑ from the moment.
    pub fn field_strength(&self, position: DVec3) -> f64 {
        let r = position.length();
        let cos = position.dot(self.moment_direction()) / r;
        self.equatorial_field() * (self.radius / r).powi(3) * (1.0 + 3.0 * cos * cos).sqrt()
    }
}

/// How much the magnetopause's currents strengthen a planet's field at the
/// nose: 2.44, from the self-consistent shape of the boundary (Mead & Beard
/// 1964, *J. Geophys. Res.* 69, 1169). A flat boundary would give exactly 2.
pub const FIELD_COMPRESSION: f64 = 2.44;

/// The share of the solar wind's dynamic pressure that reaches the
/// magnetopause's nose after the bow shock slows the flow: 0.88, for gas with
/// γ = 5/3 (Spreiter, Summers & Alksne 1966, *Planet. Space Sci.* 14, 223).
pub const STAGNATION_FACTOR: f64 = 0.88;

/// The magnetopause's distance at its nose (m from the dipole's center),
/// where the planet's compressed field pushes back as hard as the solar
/// wind (Chapman & Ferraro 1931):
///
/// K ρv² = (f B₀ (R/r)³)² / 2μ₀  ⇒  r = R (f² B₀² / 2μ₀Kρv²)^(1/6),
///
/// for a wind of flow pressure ρv² = `flow_pressure` (Pa).
pub fn standoff(dipole: &Dipole, flow_pressure: f64) -> f64 {
    let field = FIELD_COMPRESSION * dipole.equatorial_field();
    dipole.radius
        * (field * field / (2.0 * MU_0 * STAGNATION_FACTOR * flow_pressure)).powf(1.0 / 6.0)
}

/// Earth's magnetopause as Shue et al. measured it ("Magnetopause location
/// under extreme solar wind conditions", *J. Geophys. Res.* 103, 17691,
/// 1998), fitted to spacecraft crossings: its nose distance r₀ in Earth
/// radii and its flaring α, from the flow pressure (nPa, helium included)
/// and the field's north–south component Bz (nT):
///
/// r₀ = (10.22 + 1.29 tanh(0.184 (Bz + 8.14))) Dp^(−1/6.6),
/// α = (0.58 − 0.007 Bz)(1 + 0.024 ln Dp).
///
/// Their stated scatter about the observed crossings: 1.23 Earth radii.
pub fn shue_1998(flow_pressure_npa: f64, bz_nt: f64) -> (f64, f64) {
    let r0 = (10.22 + 1.29 * (0.184 * (bz_nt + 8.14)).tanh()) * flow_pressure_npa.powf(-1.0 / 6.6);
    let alpha = (0.58 - 0.007 * bz_nt) * (1.0 + 0.024 * flow_pressure_npa.ln());
    (r0, alpha)
}

/// The magnetopause's distance at angle `theta` (rad) from its nose, in
/// Shue et al.'s shape: r₀ (2 / (1 + cos θ))^α.
pub fn magnetopause_radius(nose: f64, alpha: f64, theta: f64) -> f64 {
    nose * (2.0 / (1.0 + theta.cos())).powf(alpha)
}

/// A point on the dipole field line that crosses the magnetic equator at
/// `l` times the reference radius, at magnetic latitude `latitude` (rad):
/// r = L R cos² λ. Returned in the dipole's frame, in the plane at magnetic
/// longitude `longitude` (rad), with z along the moment.
pub fn field_line_point(dipole: &Dipole, l: f64, latitude: f64, longitude: f64) -> DVec3 {
    let r = l * dipole.radius * latitude.cos().powi(2);
    DVec3::new(
        r * latitude.cos() * longitude.cos(),
        r * latitude.cos() * longitude.sin(),
        r * latitude.sin(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth() -> Dipole {
        // IGRF-14 at 2025.0.
        Dipole {
            g10: -29_350.0e-9,
            g11: -1_410.3e-9,
            h11: 4_545.5e-9,
            radius: 6_371.2e3,
            north_offset: 0.0,
        }
    }

    #[test]
    fn the_dipole_matches_its_potential() {
        // B = −∇V with V = R (R/r)² (g·r̂): check numerically at a few
        // points, by central differences of the potential.
        let d = earth();
        let g = DVec3::new(d.g11, d.h11, d.g10);
        let potential = |x: DVec3| d.radius.powi(3) * g.dot(x) / x.length().powi(3);
        for x in [
            DVec3::new(2.0, 0.5, 1.0),
            DVec3::new(-1.0, 3.0, -2.0),
            DVec3::new(0.0, 0.0, 4.0),
        ] {
            let x = x * d.radius;
            let h = 1e-4 * d.radius;
            let gradient = DVec3::new(
                potential(x + DVec3::X * h) - potential(x - DVec3::X * h),
                potential(x + DVec3::Y * h) - potential(x - DVec3::Y * h),
                potential(x + DVec3::Z * h) - potential(x - DVec3::Z * h),
            ) / (2.0 * h);
            let numeric = gradient.length();
            assert!((numeric / d.field_strength(x) - 1.0).abs() < 1e-6, "{x}");
        }
        // Earth's dipole: 29,733 nT, tilted 9.2°, its north geomagnetic
        // pole (where the moment points away from) at 80.8°N, 72.8°W.
        let pole = -d.moment_direction();
        println!(
            "B0 {:.0} nT, tilt {:.2}°, pole {:.1}°N {:.1}°E",
            d.equatorial_field() * 1e9,
            d.tilt().to_degrees(),
            pole.z.asin().to_degrees(),
            pole.y.atan2(pole.x).to_degrees()
        );
        assert!((d.equatorial_field() * 1e9 - 29_733.37).abs() < 0.01);
    }

    #[test]
    fn the_standoff_shrinks_as_the_sixth_root_of_pressure() {
        let d = earth();
        // Where the wind's pressure equals the compressed field's at the
        // surface, the magnetopause sits on the surface.
        let surface =
            (FIELD_COMPRESSION * d.equatorial_field()).powi(2) / (2.0 * MU_0 * STAGNATION_FACTOR);
        assert!((standoff(&d, surface) / d.radius - 1.0).abs() < 1e-12);
        let ratio = standoff(&d, 2e-9) / standoff(&d, 1e-9);
        assert!((ratio - 2f64.powf(-1.0 / 6.0)).abs() < 1e-12);
    }

    #[test]
    fn shues_shape_flares_from_the_nose() {
        let (r0, alpha) = shue_1998(2.0, 0.0);
        assert_eq!(magnetopause_radius(r0, alpha, 0.0), r0);
        let side = magnetopause_radius(r0, alpha, std::f64::consts::FRAC_PI_2);
        assert!((side / r0 - 2f64.powf(alpha)).abs() < 1e-12);
        // Field lines: r = L R at the equator, R at latitude acos(1/√L).
        let d = earth();
        let equator = field_line_point(&d, 4.0, 0.0, 0.0).length();
        let foot = field_line_point(&d, 4.0, (0.25f64).sqrt().acos(), 1.0).length();
        assert!((equator / (4.0 * d.radius) - 1.0).abs() < 1e-15);
        assert!((foot / d.radius - 1.0).abs() < 1e-12);
    }
}
