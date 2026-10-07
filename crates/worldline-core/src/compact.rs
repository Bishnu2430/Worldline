//! Compact objects: black holes, neutron stars and white dwarfs, where
//! gravity at the surface is strong. See `docs/physics/compact-objects.md`.

use crate::Body;
use crate::constants::C;

/// Buchdahl's limit: no static body of fluid (with density that doesn't
/// increase outward) can be smaller than 9/4 GM/c² in general relativity
/// (Buchdahl 1959, *Phys. Rev.* 116, 1027). Anything more compact must
/// collapse, so a body smaller than this is a black hole.
pub const BUCHDAHL_RADII: f64 = 9.0 / 4.0;

/// The gravitational radius GM/c², in m: the length that sets every scale
/// near a compact object.
pub fn gravitational_radius(gm: f64) -> f64 {
    gm / (C * C)
}

/// The Schwarzschild radius 2GM/c², in m: the event horizon of a black
/// hole that doesn't spin.
pub fn schwarzschild_radius(gm: f64) -> f64 {
    2.0 * gravitational_radius(gm)
}

/// The event horizon of a spinning (Kerr) black hole, in m:
/// r₊ = GM/c² (1 + √(1 − a²)), where `spin` a = cJ/(GM²) runs from 0 (not
/// spinning: 2GM/c²) to 1 (spinning as fast as possible: GM/c²).
pub fn horizon_radius(gm: f64, spin: f64) -> f64 {
    assert!((0.0..=1.0).contains(&spin), "spin must be between 0 and 1");
    gravitational_radius(gm) * (1.0 + (1.0 - spin * spin).sqrt())
}

/// The radius of a non-spinning black hole's shadow, √27 GM/c², in m: how
/// big the dark disk looks from far away, because light passing closer
/// than this falls in (Synge 1966). Spin changes it by at most a few
/// percent.
pub fn shadow_radius(gm: f64) -> f64 {
    27f64.sqrt() * gravitational_radius(gm)
}

/// Whether a body is a black hole: smaller than Buchdahl's limit, which no
/// star can be.
pub fn is_black_hole(body: &Body) -> bool {
    body.gm > 0.0 && body.radius < BUCHDAHL_RADII * gravitational_radius(body.gm)
}

/// The gravitational redshift of light leaving the surface of a static,
/// spherical body of radius `r`, as a velocity c·z in m/s:
/// 1 + z = 1/√(1 − 2GM/(rc²)) (the Schwarzschild solution).
pub fn gravitational_redshift(gm: f64, r: f64) -> f64 {
    C * (1.0 / (1.0 - 2.0 * gm / (r * C * C)).sqrt() - 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, GM_SUN, SOLAR_RADIUS};

    #[test]
    fn horizons_run_from_two_to_one_gravitational_radii() {
        let gm = GM_SUN;
        // The Sun's Schwarzschild radius: 2.953 km.
        assert!((schwarzschild_radius(gm) - 2953.25).abs() < 0.01);
        assert_eq!(horizon_radius(gm, 0.0), schwarzschild_radius(gm));
        assert_eq!(horizon_radius(gm, 1.0), gravitational_radius(gm));
        // Halfway in spin is not halfway in size: 1 + √0.75.
        let half = horizon_radius(gm, 0.5) / gravitational_radius(gm);
        assert!((half - 1.866_025_403_784_438_6).abs() < 1e-15);
        // A hole of 4.3 million Suns, like Sagittarius A*, is 0.085 AU
        // across its horizon's radius: well inside Mercury's orbit.
        let sgr = schwarzschild_radius(4.297e6 * gm) / AU;
        assert!((sgr - 0.0848).abs() < 0.0001, "{sgr}");
    }

    #[test]
    fn only_bodies_inside_buchdahls_limit_are_black_holes() {
        let hole = Body::new("Hole", 10.0 * GM_SUN, schwarzschild_radius(10.0 * GM_SUN));
        assert!(is_black_hole(&hole));
        // A spinning hole is smaller still.
        let spinning = Body::new("Kerr", 10.0 * GM_SUN, horizon_radius(10.0 * GM_SUN, 0.9));
        assert!(is_black_hole(&spinning));
        // A neutron star of 1.4 Suns and 12 km is 2.9 times its
        // gravitational radius: compact, but not a black hole.
        let star = Body::new("Neutron star", 1.4 * GM_SUN, 1.2e4);
        assert!(!is_black_hole(&star));
        assert!(!is_black_hole(&Body::new("Sun", GM_SUN, SOLAR_RADIUS)));
        // Massless bodies never are.
        assert!(!is_black_hole(&Body::new("Dust", 0.0, 0.0)));
    }

    #[test]
    fn redshift_matches_its_weak_field_limit() {
        // Far from a horizon, cz ≈ GM/(rc): the Sun's surface gives
        // 636.3 m/s.
        let sun = gravitational_redshift(GM_SUN, SOLAR_RADIUS);
        assert!((sun - GM_SUN / (SOLAR_RADIUS * C)).abs() < 1e-3 * sun);
        assert!((sun - 636.3).abs() < 0.1, "{sun}");
    }
}
