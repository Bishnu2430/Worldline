//! Validation: Sagittarius A* spirals into TON 618.
//!
//! An extreme-mass-ratio inspiral: Sagittarius A* (4.3 million Suns) on a
//! circular orbit around TON 618 (66 billion Suns, 15,000 times heavier),
//! nine times TON 618's GM/c² out. It moves in TON 618's exact spacetime
//! (Kerr's, here without spin, since TON 618's isn't measured), losing
//! energy to gravitational waves, inside the hierarchy the app runs.
//!
//! What it must do, from that model's own equations: shrink at the
//! adiabatic rate, dr/dt = −(64/5) ν (GM/rc²)³ c (1 − 2GM/rc²)/(1 − 6GM/rc²),
//! the radiated power against the energy of circular orbits in the hole's
//! spacetime; then, at the innermost stable orbit (6 GM/c²) where that
//! rate runs away, plunge, and merge at the horizon.
//!
//! `cargo test --release -p worldline-data --test validation_emri -- --nocapture`

use std::f64::consts::TAU;

use worldline_core::System;
use worldline_core::constants::C;
use worldline_core::gravity::Relativistic;
use worldline_core::gravity::kerr::{circular_orbit, plunge_time};
use worldline_core::hierarchy::Hierarchy;
use worldline_data::notable_object;

#[test]
fn sagittarius_a_star_spirals_into_ton_618() {
    let find = |name: &str| notable_object(name).expect("in the catalog");
    let (ton, sgr) = (find("TON 618").body(), find("Sagittarius A*").body());
    let gm = ton.gm + sgr.gm;
    let nu = ton.gm * sgr.gm / (gm * gm);
    let m = gm / (C * C);
    let unit_time = gm / C.powi(3);
    // On the circular orbit at 9 GM/c², around the pair's center of mass.
    let (x, v) = circular_orbit(gm, 0.0, 9.0 * m, true);
    let (to_sgr, to_ton) = (ton.gm / gm, sgr.gm / gm);
    let mut h = Hierarchy::new(
        System::new(vec![
            ton.at(-x * to_ton).moving(-v * to_ton),
            sgr.at(x * to_sgr).moving(v * to_sgr),
        ]),
        Box::new(Relativistic::default()),
        Vec::new(),
    );
    // Step a thirtieth of an orbit at a time; note when the separation
    // first falls below each radius, interpolating within the step.
    let marks = [8.5, 6.5, 6.0];
    let mut crossed = [f64::NAN; 3];
    let mut last = (0.0, 9.0);
    let mut orbits = 0.0;
    let mut closest: f64 = 9.0;
    while h.top.bodies.len() == 2 {
        let r = (h.top.bodies[1].position - h.top.bodies[0].position).length() / m;
        let t = h.time();
        for (k, &mark) in marks.iter().enumerate() {
            if crossed[k].is_nan() && r < mark {
                crossed[k] = last.0 + (t - last.0) * (last.1 - mark) / (last.1 - r);
            }
        }
        closest = closest.min(r);
        last = (t, r);
        let period = TAU * r.max(2.0).powf(1.5) * unit_time;
        h.step(period / 30.0);
        orbits += 1.0 / 30.0;
        assert!(orbits < 10_000.0, "still circling at {r:.3} GM/c²");
    }
    let merged = h.time();
    let collisions = h.take_collisions();
    assert_eq!(collisions.len(), 1);
    assert_eq!(collisions[0].survivor, "TON 618");

    let measured = (crossed[1] - crossed[0]) / unit_time;
    // The adiabatic time from 8.5 to 6.5 GM/c² (see `plunge_time`).
    let to_plunge = |x: f64| plunge_time(gm, nu, x * m).expect("outside 6 GM/c²");
    let expected = (to_plunge(8.5) - to_plunge(6.5)) / unit_time;
    let miss = (measured / expected - 1.0).abs();
    let year = 365.25 * 86_400.0;
    println!(
        "ν = {nu:.3e}; GM/c³ = {:.3} days. From 8.5 to 6.5 GM/c²: {measured:.5e} GM/c³ ({:.0} years); the adiabatic rate gives {expected:.5e} (off by {miss:.1e}, bound √ν = {:.1e})",
        unit_time / 86_400.0,
        measured * unit_time / year,
        nu.sqrt()
    );
    let isco_period = TAU * 6f64.powf(1.5);
    println!(
        "from the innermost stable orbit, 6 GM/c², to the merger: {:.2} orbits there ({:.1} years); {orbits:.0} orbits in all, {:.0} years",
        (merged - crossed[2]) / unit_time / isco_period,
        (merged - crossed[2]) / year,
        merged / year
    );
    // 1. The inspiral follows the adiabatic rate, up to corrections of
    //    order ν (the orbit isn't quite circular as it shrinks); a mistake
    //    in the spacetime or the radiation reaction would show at order 1.
    //    Allowed: the midpoint, √ν.
    assert!(miss < nu.sqrt());
    // 2. It plunges where the rate runs away: from the innermost stable
    //    orbit it reaches the horizon within the transition Ori & Thorne
    //    (2000, Phys. Rev. D 62, 124022) found, about ν^(−1/5) orbits there
    //    (7 here). Were the orbit still stable, it would shrink at the
    //    adiabatic pace, about ν^(−1) orbits. Allowed: the midpoint,
    //    ν^(−3/5) orbits (320 here).
    assert!((merged - crossed[2]) / unit_time / isco_period < nu.powf(-0.6));
}
