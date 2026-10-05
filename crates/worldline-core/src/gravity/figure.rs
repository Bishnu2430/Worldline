use glam::DVec3;

/// The shape of a tidally locked moon, as it affects its pull on its planet.
///
/// A moon that always shows its planet the same face is stretched by the
/// planet's tide into a slight egg shape, its long axis pointing at the
/// planet. Seen from the planet, the moon's degree-2 potential along that
/// axis (latitude 0, longitude 0 in the moon's body frame) is
///
/// U = (G m / r) (R / r)² (J2 / 2 + 3 C22)
///
/// beyond the point-mass term (P₂(0) = −½ and P₂₂(0) = 3 in the expansion
/// used by [`super::TesseralField`]). That pulls planet and moon together a
/// little harder than point masses would: with q = J2/2 + 3 C22, the moon
/// accelerates toward the planet by an extra 3 G M R² q / r⁴ and the planet
/// toward the moon by 3 G m R² q / r⁴.
///
/// The long axis really points toward the empty focus of the orbit and
/// rocks slightly (physical libration). For these moons that tilts it by
/// at most about 2e ≈ 2° (e is the orbit's eccentricity), changing the pull
/// by under 0.1%. See `docs/physics/moons.md`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SynchronousFigure {
    /// Reference radius the coefficients are defined with, in m.
    pub radius: f64,
    /// The moon's oblateness J2 (unnormalized).
    pub j2: f64,
    /// The moon's equatorial ellipticity C22 (unnormalized).
    pub c22: f64,
}

impl SynchronousFigure {
    /// The extra acceleration of the moon (m/s²) at `r` (m) from a planet
    /// with gravitational parameter `planet_gm` (m³/s²). The planet feels
    /// the opposite pull scaled by the moon's GM over the planet's.
    pub fn acceleration(&self, planet_gm: f64, r: DVec3) -> DVec3 {
        let q = self.j2 / 2.0 + 3.0 * self.c22;
        let distance = r.length();
        -r * (3.0 * planet_gm * self.radius * self.radius * q / distance.powi(5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gravity::TesseralField;
    use glam::DMat3;

    #[test]
    fn matches_the_degree_two_field_along_the_long_axis() {
        // A point on the moon's long axis feels the moon's degree-2 field
        // as a radial pull. By symmetry, the moon feels the same extra pull
        // from a point mass there, scaled by the point's GM.
        let io = SynchronousFigure {
            radius: 1_829.4e3,
            j2: 1.8236e-3,
            c22: 5.497e-4,
        };
        let field = TesseralField {
            radius: io.radius,
            coefficients: vec![(2, 0, -io.j2, 0.0), (2, 2, io.c22, 0.0)],
            frame: DMat3::IDENTITY,
            spin_rate: 0.0,
            epoch: 0.0,
        };
        let (gm_io, gm_jupiter) = (5.96e12, 1.267e17);
        let r = 4.218e8;
        // A planet a distance r out along the moon's long axis is pulled
        // back toward the moon (per unit of the moon's GM) exactly as the
        // moon, a distance r out from the planet, is pulled toward the
        // planet (per unit of the planet's GM).
        let on_planet = field.body_fixed_acceleration(gm_io, DVec3::X * r) / gm_io;
        let on_moon = io.acceleration(gm_jupiter, DVec3::X * r) / gm_jupiter;
        assert!((on_planet - on_moon).length() < 1e-12 * on_moon.length());
        assert!(on_moon.x < 0.0, "the moon is pulled toward the planet");
    }
}
