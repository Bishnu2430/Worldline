//! Validation: bodies near a black hole move as general relativity says,
//! in its exact (Kerr) spacetime.
//!
//! The roadmap's check: the innermost stable circular orbit of a hole that
//! doesn't spin is at 6 GM/c². Found the way nature finds it: circular
//! orbits just outside it, nudged, keep circling; just inside it, they
//! spiral in and plunge into the hole. Then the same for a hole spinning at
//! 0.9, with and against its spin, against Bardeen, Press & Teukolsky's
//! formula. And what any free body must keep (its energy and its angular
//! momentum about the spin axis), and the circular orbits' periods.
//!
//! `cargo test --release -p worldline-core --test validation_kerr -- --nocapture`

use std::f64::consts::TAU;

use worldline_core::constants::{C, GM_SUN};
use worldline_core::gravity::Gravity;
use worldline_core::gravity::kerr::{
    boyer_lindquist_radius, circular_orbit, energy_and_angular_momentum, geodesic_acceleration,
    innermost_stable_orbit,
};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::{Body, DVec3, System};

/// A test body around a black hole fixed at the origin.
struct Around {
    gm: f64,
    spin: f64,
}

impl Gravity for Around {
    fn name(&self) -> &'static str {
        "Kerr geodesic"
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
        for (b, a) in bodies.iter().zip(out.iter_mut()) {
            *a = geodesic_acceleration(self.gm, self.spin, b.position, b.velocity);
        }
    }
}

/// A hole of 10 Suns.
const GM: f64 = 10.0 * GM_SUN;

fn m() -> f64 {
    GM / (C * C)
}

/// Runs a body from `position` and `velocity` for `orbits` turns of
/// period `period`, 200 steps a turn; returns its smallest and largest
/// Boyer–Lindquist radius, in units of GM/c², and whether it fell within
/// 2 GM/c² (inside the horizon of a hole that doesn't spin, and well
/// inside every innermost stable orbit).
fn run(spin: f64, position: DVec3, velocity: DVec3, period: f64, orbits: f64) -> (f64, f64, bool) {
    let gravity = Around { gm: GM, spin };
    let mut system = System::new(vec![
        Body::new("test", 0.0, 0.0).at(position).moving(velocity),
    ]);
    let mut ias = Ias15::new();
    let (mut low, mut high) = (f64::INFINITY, 0.0f64);
    for _ in 0..(orbits * 200.0) as usize {
        advance(&mut system, &gravity, &mut ias, period / 200.0);
        let r = boyer_lindquist_radius(GM, spin, system.bodies[0].position) / m();
        low = low.min(r);
        high = high.max(r);
        if r < 2.0 {
            return (low, high, true);
        }
    }
    (low, high, false)
}

/// Whether a circular orbit at `r` (units of GM/c²), its speed nudged by
/// one part in 10⁶, survives 300 turns.
fn survives(spin: f64, r: f64, prograde: bool) -> bool {
    let (x, v) = circular_orbit(GM, spin, r * m(), prograde);
    let period = TAU * r * m() / v.length();
    let (low, high, fell) = run(spin, x, v * (1.0 - 1e-6), period, 300.0);
    println!(
        "  spin {spin}, {} orbit at r = {r:.4} GM/c²: {}, r from {low:.4} to {high:.4}",
        if prograde { "prograde" } else { "retrograde" },
        if fell { "plunged" } else { "kept circling" },
    );
    !fell
}

#[test]
fn the_innermost_stable_orbit_is_where_theory_puts_it() {
    // A nudge δ = 10⁻⁶ to the speed. A fraction ε outside the innermost
    // stable orbit, the orbit then swings in and out by about δ/ε of its
    // radius; ε inside, the swing grows by e every 1/(2π√ε) turns (for a
    // hole that doesn't spin), so ln(ε/δ) e-folds take it into the hole
    // within about 20 turns. With ε = 0.5%, well above √δ = 0.1%, the
    // swing outside (2 × 10⁻⁴) stays well clear of the orbit, and 300
    // turns are ample inside.
    let bracket = 0.005;
    for (spin, prograde) in [(0.0, true), (0.9, true), (0.9, false)] {
        let isco = innermost_stable_orbit(spin, prograde);
        println!(
            "spin {spin}, {}: Bardeen, Press & Teukolsky put it at {isco:.4} GM/c²",
            if prograde { "prograde" } else { "retrograde" }
        );
        assert!(survives(spin, isco * (1.0 + bracket), prograde));
        assert!(!survives(spin, isco * (1.0 - bracket), prograde));
    }
    // The roadmap's number: 6 GM/c² for a hole that doesn't spin.
    assert_eq!(innermost_stable_orbit(0.0, true), 6.0);
}

#[test]
fn free_bodies_keep_their_energy_and_angular_momentum() {
    // A bound, eccentric, inclined orbit around a hole spinning at 0.9,
    // between about 8 and 13 GM/c²: every part of the metric matters.
    // Energy and angular momentum about the spin axis must hold to the
    // integrator's precision.
    let spin = 0.9;
    let x = DVec3::new(8.0 * m(), 0.0, 1.0 * m());
    let v = DVec3::new(0.0, 0.36 * C, 0.06 * C);
    let (e0, l0) = energy_and_angular_momentum(GM, spin, x, v);
    assert!(e0 < 1.0, "bound");
    let period = TAU * (10.0 * m()).powf(1.5) / GM.sqrt();
    let gravity = Around { gm: GM, spin };
    let mut system = System::new(vec![Body::new("test", 0.0, 0.0).at(x).moving(v)]);
    let mut ias = Ias15::new();
    let (mut de, mut dl): (f64, f64) = (0.0, 0.0);
    let (mut low, mut high) = (f64::INFINITY, 0.0f64);
    for _ in 0..20 * 200 {
        advance(&mut system, &gravity, &mut ias, period / 200.0);
        let b = &system.bodies[0];
        let (e, l) = energy_and_angular_momentum(GM, spin, b.position, b.velocity);
        de = de.max((e / e0 - 1.0).abs());
        dl = dl.max((l / l0 - 1.0).abs());
        let r = boyer_lindquist_radius(GM, spin, b.position) / m();
        low = low.min(r);
        high = high.max(r);
    }
    println!(
        "E = {e0:.6}, L = {:.4} GM/c over 20 orbits (r {low:.2} to {high:.2} GM/c²): energy kept to {de:.1e}, angular momentum to {dl:.1e}",
        l0 / (GM / C)
    );
    assert!(de < 1e-9 && dl < 1e-9);
}

#[test]
fn circular_orbits_turn_at_their_exact_rate() {
    // Ω = 1/(r^(3/2) ± a) in units of GM/c² and c (Bardeen, Press &
    // Teukolsky 1972): after 10 turns the body must be back where it
    // started, to the integrator's precision.
    for (spin, prograde, r) in [(0.0, true, 10.0), (0.9, true, 4.0), (0.9, false, 12.0)] {
        let (x, v) = circular_orbit(GM, spin, r * m(), prograde);
        let gravity = Around { gm: GM, spin };
        let mut system = System::new(vec![Body::new("test", 0.0, 0.0).at(x).moving(v)]);
        let mut ias = Ias15::new();
        let sign = if prograde { 1.0 } else { -1.0 };
        let omega = sign / (r.powf(1.5) + sign * spin) * C / m();
        advance(&mut system, &gravity, &mut ias, 10.0 * TAU / omega.abs());
        let miss = (system.bodies[0].position - x).length() / x.length();
        println!(
            "spin {spin}, r = {r} GM/c²: back within {miss:.1e} of where it started after 10 turns"
        );
        assert!(miss < 1e-8);
    }
}

#[test]
fn far_away_it_is_newtons_gravity() {
    // At 10⁶ GM/c², the relativistic corrections are of order GM/rc² =
    // 10⁻⁶ of the pull; a mistake in Newton's part would show at order 1.
    // The test passes below the midpoint, √(GM/rc²) = 10⁻³.
    let x = DVec3::new(6e5, 8e5, 0.0) * m();
    let v = DVec3::new(0.0, 0.0, 0.0);
    let newton = -GM * x / x.length().powi(3);
    let kerr = geodesic_acceleration(GM, 0.9, x, v);
    let miss = (kerr - newton).length() / newton.length();
    println!("at 10⁶ GM/c²: the pull is Newton's to {miss:.1e}");
    assert!(miss < 1e-3);
}
