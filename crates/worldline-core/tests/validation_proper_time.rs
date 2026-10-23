//! Validation: how fast moving clocks run (step 3.4), against hand
//! calculations and Bardeen, Press & Teukolsky's (1972) circular orbits.
//!
//! The roadmap's check: a clock near a black hole visibly runs slow by the
//! predicted factor. A clock on a circular orbit at r around a hole that
//! doesn't spin runs at √(1 − 3GM/rc²) of a far-away clock's rate; carried
//! round by the engine's own equations of motion, it keeps doing so.

use std::f64::consts::TAU;

use worldline_core::constants::{C, GM_SUN};
use worldline_core::fields::{clock_slowdown, kerr_clock_slowdown, moving_clock_slowdown};
use worldline_core::gravity::Gravity;
use worldline_core::gravity::kerr::{circular_orbit, geodesic_acceleration};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::kerr_light::circular;
use worldline_core::{Body, DVec3, System};

/// A hole of 10 Suns, and its GM/c².
const GM: f64 = 10.0 * GM_SUN;

fn m() -> f64 {
    GM / (C * C)
}

#[test]
fn a_clock_on_a_circular_orbit_runs_at_bardeen_press_and_teukolskys_rate() {
    // dτ/dt = 1/u^t, u^t = (r^(3/2) + a)/(r^(3/4) √(r^(3/2) − 3r^(1/2) + 2a)):
    // without spin, √(1 − 3/r). The clock's rate from the metric itself,
    // for the orbit's position and velocity in Kerr–Schild coordinates,
    // must agree to rounding.
    for (a, r) in [
        (0.0, 6.0),
        (0.0, 10.0),
        (0.9, 4.0),
        (0.9, 10.0),
        (0.99, 2.0),
    ] {
        let (x, v) = circular_orbit(GM, a, r * m(), true);
        let rate = 1.0 - kerr_clock_slowdown(GM, a, x, v).expect("a clock can move so");
        let (_, ut) = circular(a, r);
        println!(
            "spin {a}, r = {r} GM/c²: runs at {rate:.12} (1/u^t = {:.12})",
            1.0 / ut
        );
        assert!((rate * ut - 1.0).abs() < 1e-14);
    }
    // At r = 6 GM/c², the innermost stable orbit: √½.
    let (x, v) = circular_orbit(GM, 0.0, 6.0 * m(), true);
    let rate = 1.0 - kerr_clock_slowdown(GM, 0.0, x, v).unwrap();
    assert!((rate - 0.5f64.sqrt()).abs() < 1e-14);
    // At rest it is the clock-rate overlay's static clock, √(1 − 2GM/rc²).
    let hole = Body::new("hole", GM, 2.0 * m());
    let at = DVec3::new(10.0 * m(), 0.0, 0.0);
    let resting = kerr_clock_slowdown(GM, 0.0, at, DVec3::ZERO).unwrap();
    assert!((resting - clock_slowdown(&[hole], at).unwrap()).abs() < 1e-15);
}

/// A test body around a black hole fixed at the origin, as the engine
/// moves bodies a hole holds.
struct Around;

impl Gravity for Around {
    fn name(&self) -> &'static str {
        "Kerr geodesic"
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
        for (b, a) in bodies.iter().zip(out.iter_mut()) {
            *a = geodesic_acceleration(GM, 0.0, b.position, b.velocity);
        }
    }
}

#[test]
fn carried_round_its_orbit_the_clock_keeps_that_rate() {
    // The roadmap's check. A clock on the circular orbit at 10 GM/c²,
    // carried 20 times round by the engine's geodesic equation, against a
    // far-away clock: the time it shows, added up step by step (trapezoid
    // rule, 200 steps a turn), over the time that passed. Predicted:
    // √(1 − 3/10) = 0.836660. The orbit keeps its radius to the
    // integrator's precision, and the rate changes with radius only as
    // d(√(1 − 3/r))/d ln r ≈ 0.27, so 10⁻¹⁰ is ample.
    let r = 10.0;
    let (x, v) = circular_orbit(GM, 0.0, r * m(), true);
    let period = TAU * r * m() / v.length();
    let mut system = System::new(vec![Body::new("clock", 0.0, 0.0).at(x).moving(v)]);
    let mut ias = Ias15::new();
    let rate = |s: &System| {
        1.0 - kerr_clock_slowdown(GM, 0.0, s.bodies[0].position, s.bodies[0].velocity).unwrap()
    };
    let (mut shown, mut before) = (0.0, rate(&system));
    let step = period / 200.0;
    for _ in 0..4000 {
        advance(&mut system, &Around, &mut ias, step);
        let after = rate(&system);
        shown += 0.5 * (before + after) * step;
        before = after;
    }
    let factor = shown / (4000.0 * step);
    let predicted = (1.0 - 3.0 / r).sqrt();
    println!(
        "after 20 turns at 10 GM/c² ({:.4} s of far-away time), the clock shows {:.4} s: {factor:.12} of it (√(1 − 3/r) = {predicted:.12})",
        4000.0 * step,
        shown
    );
    assert!((factor - predicted).abs() < 1e-10);
}

#[test]
fn gps_clocks_run_fast_as_worked_out_by_hand() {
    // A GPS satellite's clock (26,559.7 km from Earth's center, moving at
    // √(GM/r)) against one on the equator, carried round at 465.1 m/s by
    // Earth's turning: gravity makes it gain GM/c²(1/R − 1/r), its speed
    // lose (v_s² − v_g²)/2c². By hand: 45.65 − 7.21 + 0.10 = 38.54 µs a
    // day. (GPS corrects for 38.6: Earth's oblateness adds the rest, left
    // out here.) First order, to (GM/Rc²)² = 5 × 10⁻¹⁹.
    let (gm, radius, orbit) = (3.986_004_418e14, 6.3781e6, 2.65597e7);
    let earth = Body::new("Earth", gm, radius);
    let satellite = (gm / orbit).sqrt();
    let up = moving_clock_slowdown(
        std::slice::from_ref(&earth),
        DVec3::new(orbit, 0.0, 0.0),
        DVec3::new(0.0, satellite, 0.0),
        None,
    )
    .unwrap();
    let ground = moving_clock_slowdown(
        std::slice::from_ref(&earth),
        DVec3::new(radius, 0.0, 0.0),
        DVec3::new(0.0, 465.1, 0.0),
        None,
    )
    .unwrap();
    let hand = gm / (C * C) * (1.0 / radius - 1.0 / orbit)
        - (satellite.powi(2) - 465.1f64.powi(2)) / (2.0 * C * C);
    println!(
        "a GPS clock gains {:.2} µs a day on one on the equator (by hand {:.2})",
        (ground - up) * 86_400e6,
        hand * 86_400e6
    );
    assert!(((ground - up) - hand).abs() < 5e-19);
}
