//! Validation: a flattened planet makes its moons' orbits precess.
//!
//! The extra pull of a planet's equatorial bulge (J2) turns a tilted orbit
//! like a top: the line where the orbit crosses the equator (the node)
//! slides backwards at
//!
//! dΩ/dt = −(3/2) n J2 (R/a)² cos i
//!
//! to first order in J2 (e.g. Murray & Dermott, Solar System Dynamics,
//! eq. 6.249). Simulate an Io-like moon around a Jupiter-like planet and
//! compare.

use std::f64::consts::PI;

use worldline_core::constants::DAY;
use worldline_core::gravity::{Newtonian, ZonalField};
use worldline_core::hierarchy::{Hierarchy, MoonSystem};
use worldline_core::orbit::periapsis_state;
use worldline_core::{Body, DMat3, DVec3, System};

#[test]
fn j2_makes_the_orbit_node_regress_at_the_theoretical_rate() {
    let (mu, j2, radius) = (1.267e17, 0.0146965, 71_492e3);
    let (a, inclination) = (4.217e8, 10f64.to_radians());

    // A lone planetary system (no Sun), with a test-particle moon on a
    // circular orbit tilted 10° to the planet's equator (the x–y plane).
    let top = System::new(vec![Body::new("system", mu, radius)]);
    let (r, v) = periapsis_state(a, 0.0, mu);
    let tilt = DMat3::from_rotation_x(inclination);
    let bodies = vec![
        Body::new("planet", mu, radius),
        Body::new("moon", 0.0, 1.8e6).at(tilt * r).moving(tilt * v),
    ];
    let zonal = ZonalField {
        radius,
        coefficients: vec![(2, j2)],
        pole: DVec3::Z,
    };
    let mut h = Hierarchy::new(
        top,
        Box::new(Newtonian),
        vec![MoonSystem::new(0, bodies).with_zonal(zonal)],
    );

    // Track the node's longitude Ω every 2 hours for 10 days.
    let node = |h: &Hierarchy| {
        let s = &h.moon_systems[0].system;
        let (r, v) = (
            s.bodies[1].position - s.bodies[0].position,
            s.bodies[1].velocity - s.bodies[0].velocity,
        );
        let n = DVec3::Z.cross(r.cross(v));
        n.y.atan2(n.x)
    };
    let mut samples = Vec::new();
    for _ in 0..=120 {
        samples.push((h.time(), node(&h)));
        h.advance(2.0 * 3600.0);
    }
    // Least-squares slope of Ω(t): averages out the twice-per-orbit wobble.
    let n = samples.len() as f64;
    let t_mean = samples.iter().map(|s| s.0).sum::<f64>() / n;
    let o_mean = samples.iter().map(|s| s.1).sum::<f64>() / n;
    let slope = samples
        .iter()
        .map(|s| (s.0 - t_mean) * (s.1 - o_mean))
        .sum::<f64>()
        / samples.iter().map(|s| (s.0 - t_mean).powi(2)).sum::<f64>();

    let mean_motion = (mu / a.powi(3)).sqrt();
    let theory = -1.5 * mean_motion * j2 * (radius / a).powi(2) * inclination.cos();
    let per_day = |rate: f64| rate * DAY * 180.0 / PI;
    println!(
        "node regression: simulated {:.5}°/day, theory {:.5}°/day",
        per_day(slope),
        per_day(theory)
    );
    // The formula is first order in J2. What it leaves out (second-order
    // terms, and the difference between instantaneous and averaged orbital
    // elements) amounts to a few tenths of a percent here. Allow 1%.
    assert!((slope / theory - 1.0).abs() < 0.01);
}
