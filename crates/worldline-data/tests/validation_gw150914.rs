//! Validation: a replay of GW150914, the first black-hole merger LIGO
//! heard, leaves the black hole LIGO measured.
//!
//! The catalog's two holes (35.6 and 30.6 Suns, GWTC-1) start on the
//! circular orbit whose gravitational wave is at 20 Hz and spiral in under
//! Worldline's post-Newtonian gravity, radiation reaction included, inside
//! the same hierarchy the app runs. Where the post-Newtonian description
//! gives out, a few orbits before the real merger, they merge as
//! numerical relativity's fits say. The final hole's mass and spin must
//! fall within the 90% credible intervals LIGO and Virgo published.
//!
//! `cargo test --release -p worldline-data --test validation_gw150914 -- --nocapture`

use std::f64::consts::PI;
use worldline_core::compact::{horizon_spin, is_black_hole};
use worldline_core::constants::{C, GM_SUN};
use worldline_core::gravity::{PostNewtonian, circular_pair};
use worldline_core::hierarchy::Hierarchy;
use worldline_core::merger::ringdown;

use worldline_core::System;
use worldline_data::notable_object;

#[test]
fn a_gw150914_replay_leaves_the_hole_ligo_measured() {
    let find = |name: &str| notable_object(name).expect("in the catalog");
    let (heavier, lighter) = (find("GW150914 heavier hole"), find("GW150914 lighter hole"));
    let measured = find("GW150914 final hole");
    let (a, b) = circular_pair(heavier.body(), lighter.body(), 20.0);
    let gm = a.gm + b.gm;
    let mut h = Hierarchy::new(
        System::new(vec![a, b]),
        Box::new(PostNewtonian::default()),
        Vec::new(),
    );
    // The pair's separation and wave frequency at the last step before
    // the merger.
    let mut last = (0.0, 0.0);
    while h.top.bodies.len() == 2 && h.time() < 60.0 {
        let (p, q) = (&h.top.bodies[0], &h.top.bodies[1]);
        let (x, v) = (p.position - q.position, p.velocity - q.velocity);
        last = (x.length(), x.cross(v).length() / x.length_squared() / PI);
        h.step(1e-3);
    }
    let collisions = h.take_collisions();
    assert_eq!(collisions.len(), 1, "the holes merged");
    let merger = collisions[0]
        .merger
        .expect("as black holes that spiraled in");
    let hole = &h.top.bodies[0];
    let mass = hole.gm / GM_SUN;
    let spin = horizon_spin(hole.gm, hole.radius);
    let tone = ringdown(hole.gm, spin);
    println!(
        "merged {:.2} s after the wave was at 20 Hz, where it was at {:.1} Hz and GM/rc² = {:.3}",
        collisions[0].time,
        last.1,
        gm / (last.0 * C * C)
    );
    println!(
        "final hole: {mass:.2} Suns, spin {spin:.3}; {:.2} Suns radiated; kick {:.1} km/s; rings down at {:.0} Hz, fading in {:.2} ms",
        merger.radiated_gm / GM_SUN,
        merger.kick.length() / 1e3,
        tone.frequency,
        tone.damping * 1e3
    );
    assert!(is_black_hole(hole));
    assert!((hole.gm + merger.radiated_gm - gm).abs() <= 1e-15 * gm);
    // The pair's center of mass started at rest, to second order. On top
    // of its kick the hole moves with the center of mass, which by the
    // merger has wandered at third order: printed, not tested (the unit
    // test of the center of mass checks that order).
    let wander = (hole.velocity - merger.kick).length();
    println!(
        "the hole moves at {:.1} km/s: its kick plus the center of mass's third-order wander, {:.1} km/s",
        hole.velocity.length() / 1e3,
        wander / 1e3
    );

    // LIGO and Virgo's first measurement (Abbott et al. 2016, Phys. Rev.
    // Lett. 116, 061102), which the roadmap quotes: 62 ± 4 Suns, spin
    // 0.67 +0.05/−0.07, 3.0 ± 0.5 Suns radiated (90% credible).
    println!("2016 discovery paper: 62 ± 4 Suns, spin 0.67 +0.05/−0.07, 3.0 ± 0.5 Suns radiated");
    assert!((mass - 62.0).abs() <= 4.0);
    assert!((0.67 - 0.07..=0.67 + 0.05).contains(&spin));
    assert!((merger.radiated_gm / GM_SUN - 3.0).abs() <= 0.5);

    // The catalog's GWTC-1 values (Abbott et al. 2019, Phys. Rev. X 9,
    // 031040), from which the two holes' masses come: 63.1 +3.4/−3.0 Suns,
    // spin 0.69 +0.05/−0.04.
    let (plus, minus) = measured.mass.plus_minus.expect("an interval");
    let published = measured.mass.value;
    let (published_spin, _) = measured.spin.expect("a spin");
    println!("GWTC-1: {published} +{plus}/−{minus} Suns, spin {published_spin} +0.05/−0.04");
    assert!((published - minus..=published + plus).contains(&mass));
    assert!((published_spin - 0.04..=published_spin + 0.05).contains(&spin));
}
