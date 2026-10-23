//! Validation: the numbers behind the view's overlays (step 3.3), against
//! hand calculations: how fast clocks run, how tides stretch, the shape of
//! space around a body, and gravitational waves' wavelength and strength.

use std::f64::consts::PI;

use worldline_core::compact::{horizon_radius, schwarzschild_radius};
use worldline_core::constants::{AU, C, GM_SUN, PARSEC, SOLAR_RADIUS};
use worldline_core::fields::{
    clock_slowdown, embedding_height, principal_axes, ruler_excess, tidal_tensor,
};
use worldline_core::gravitational_waves::{
    chirp_gm, frequency_then, lookback, merger_time, strain, strain_amplitude, time_to_merge,
};
use worldline_core::gravity::circular_pair;
use worldline_core::{Body, DVec3};

/// Earth's gravitational parameter, m³/s² (IERS Conventions 2010), and
/// equatorial radius, m (IAU 2015 nominal).
const GM_EARTH: f64 = 3.986_004_418e14;
const EARTH_RADIUS: f64 = 6.3781e6;
/// The Moon's gravitational parameter, m³/s² (JPL DE440), and its mean
/// distance, m.
const GM_MOON: f64 = 4.902_800_118e12;
const MOON_DISTANCE: f64 = 3.844e8;

#[test]
fn clocks_run_slow_as_worked_out_by_hand() {
    let earth = Body::new("Earth", GM_EARTH, EARTH_RADIUS);
    let at = |r: f64| DVec3::new(r, 0.0, 0.0);
    // GPS: a clock on the satellites' orbit (semi-major axis 26,559.7 km)
    // gains on one at the equator, from gravity alone, GM/c² (1/R − 1/r)
    // a second: 45.7 µs a day, the figure GPS is built around. What the
    // first order leaves out, (GM/Rc²)², is 5 × 10⁻¹⁹.
    let ground = clock_slowdown(std::slice::from_ref(&earth), at(EARTH_RADIUS)).unwrap();
    let orbit = clock_slowdown(std::slice::from_ref(&earth), at(2.65597e7)).unwrap();
    let hand = GM_EARTH / (C * C) * (1.0 / EARTH_RADIUS - 1.0 / 2.65597e7);
    let gain = (ground - orbit) * 86_400.0;
    println!(
        "GPS clocks gain {:.2} µs a day from gravity (by hand {:.2})",
        gain * 1e6,
        hand * 86_400.0 * 1e6
    );
    assert!(((ground - orbit) - hand).abs() < 5e-19);
    // The Sun adds GM☉/(AU c²) = 9.87 × 10⁻⁹ at Earth: together 1.06 × 10⁻⁸.
    let sun = Body::new("Sun", GM_SUN, SOLAR_RADIUS);
    let earth_at_1au = Body {
        position: at(AU),
        ..earth.clone()
    };
    let both = clock_slowdown(&[sun, earth_at_1au], at(AU + EARTH_RADIUS)).unwrap();
    let hand = GM_SUN / ((AU + EARTH_RADIUS) * C * C) + GM_EARTH / (EARTH_RADIUS * C * C);
    println!(
        "on Earth, with the Sun: slow by {both:.4e} ({:.3} ms a day; by hand {hand:.4e})",
        both * 86_400e3
    );
    assert!((both - hand).abs() < 1e-15);
    // Twice the Schwarzschild radius from a black hole: √(1 − 1/2).
    let hole = Body::new("hole", 10.0 * GM_SUN, schwarzschild_radius(10.0 * GM_SUN));
    let rs = schwarzschild_radius(hole.gm);
    let there = clock_slowdown(std::slice::from_ref(&hole), at(2.0 * rs)).unwrap();
    println!(
        "at 2 r_s from a hole: clocks run at {:.10} (√½ = {:.10})",
        1.0 - there,
        0.5f64.sqrt()
    );
    assert!((1.0 - there - 0.5f64.sqrt()).abs() < 1e-15);
    // No clock stays at rest inside the horizon, or inside a spinning
    // hole's ergosphere: at a = 0.9, the equator out to 2 GM/c².
    assert!(clock_slowdown(std::slice::from_ref(&hole), at(0.9 * rs)).is_none());
    let m = 10.0 * GM_SUN / (C * C);
    let spinning = Body::new(
        "spinning",
        10.0 * GM_SUN,
        horizon_radius(10.0 * GM_SUN, 0.9),
    );
    assert!(clock_slowdown(std::slice::from_ref(&spinning), at(1.9 * m)).is_none());
    // Above its pole at r = 1.9 GM/c², Kerr's g_tt: √(1 − 2r/(r² + a²)).
    let pole = clock_slowdown(
        std::slice::from_ref(&spinning),
        DVec3::new(0.0, 0.0, 1.9 * m),
    )
    .unwrap();
    let hand = (1.0 - 3.8 / (1.9f64 * 1.9 + 0.81)).sqrt();
    println!(
        "above a spinning hole's pole at 1.9 GM/c²: {:.10} (by hand {hand:.10})",
        1.0 - pole
    );
    assert!((1.0 - pole - hand).abs() < 1e-12);
}

#[test]
fn tides_stretch_as_worked_out_by_hand() {
    // The Moon's tide at Earth's surface: 2GM R/d³ along the line to the
    // Moon, 1.10 × 10⁻⁶ m/s², and half that squeezing across; the Sun's,
    // 0.505 × 10⁻⁶: the Moon's is 2.18 times the Sun's.
    let moon = Body {
        position: DVec3::new(MOON_DISTANCE, 0.0, 0.0),
        ..Body::new("Moon", GM_MOON, 1.7374e6)
    };
    let sun = Body {
        position: DVec3::new(0.0, AU, 0.0),
        ..Body::new("Sun", GM_SUN, SOLAR_RADIUS)
    };
    let earth = Body::new("Earth", GM_EARTH, EARTH_RADIUS);
    let tide = |body: &Body| {
        let axes = principal_axes(tidal_tensor(
            &[earth.clone(), body.clone()],
            DVec3::ZERO,
            Some(0),
        ));
        (
            axes[0].0 * EARTH_RADIUS,
            axes[0].1,
            axes[2].0 * EARTH_RADIUS,
        )
    };
    let (stretch, along, squeeze) = tide(&moon);
    let hand = 2.0 * GM_MOON * EARTH_RADIUS / MOON_DISTANCE.powi(3);
    println!(
        "the Moon's tide: {stretch:.4e} m/s² along the line to it (by hand {hand:.4e}), {squeeze:.4e} across"
    );
    assert!((stretch / hand - 1.0).abs() < 1e-13 && (squeeze / (-0.5 * hand) - 1.0).abs() < 1e-13);
    assert!(along.x.abs() > 1.0 - 1e-13);
    let (solar, along, _) = tide(&sun);
    let hand = 2.0 * GM_SUN * EARTH_RADIUS / AU.powi(3);
    println!(
        "the Sun's: {solar:.4e} m/s² (by hand {hand:.4e}); the Moon's is {:.2} times it",
        stretch / solar
    );
    assert!((solar / hand - 1.0).abs() < 1e-13 && along.y.abs() > 1.0 - 1e-13);
    // Spaghettification: 1,000 km from a hole of 10 Suns, a body 2 m long
    // is pulled apart by 2GM·2 m/r³ = 5,300 m/s², 540 g.
    let hole = Body::new("hole", 10.0 * GM_SUN, schwarzschild_radius(10.0 * GM_SUN));
    let axes = principal_axes(tidal_tensor(
        std::slice::from_ref(&hole),
        DVec3::new(1e6, 0.0, 0.0),
        None,
    ));
    let pull = axes[0].0 * 2.0;
    println!(
        "2 m long, 1,000 km from a hole of 10 Suns: pulled apart by {pull:.0} m/s², {:.0} g",
        pull / 9.80665
    );
    assert!((pull / (4.0 * 10.0 * GM_SUN / 1e18) - 1.0).abs() < 1e-13);
}

#[test]
fn space_around_a_body_is_shaped_as_worked_out_by_hand() {
    // A black hole: Flamm's paraboloid stands 2r_s high at r = 2r_s, and a
    // ruler from the horizon to there is r_s(√2 + ln(1 + √2)) = 2.2956 r_s
    // long, not r_s: 1.2956 r_s more.
    let gm = 10.0 * GM_SUN;
    let rs = schwarzschild_radius(gm);
    let height = embedding_height(gm, rs, 2.0 * rs) / rs;
    let excess = ruler_excess(gm, rs, 2.0 * rs) / rs;
    let hand = 2f64.sqrt() + (1.0 + 2f64.sqrt()).ln() - 1.0;
    println!(
        "a hole: height {height:.12} r_s at 2 r_s; the ruler {excess:.12} r_s longer (by hand {hand:.12})"
    );
    assert!((height - 2.0).abs() < 1e-12 && (excess - hand).abs() < 1e-12);
    // Earth (r_s = 8.87 mm): from its surface to twice its radius a ruler
    // is longer by about (r_s/2) ln 2 = 3.07 mm; its center sits √(r_s R)/2
    // = 119 m below its surface, and twice its radius out stands
    // 2√(r_s R)(√2 − 1) = 197 m above it. What these first-order forms leave
    // out is of relative size r_s/R = 1.4 × 10⁻⁹.
    let rs = schwarzschild_radius(GM_EARTH);
    let r = EARTH_RADIUS;
    let excess = ruler_excess(GM_EARTH, r, 2.0 * r);
    let below = embedding_height(GM_EARTH, r, r) - embedding_height(GM_EARTH, r, 0.0);
    let above = embedding_height(GM_EARTH, r, 2.0 * r) - embedding_height(GM_EARTH, r, r);
    let hands = [
        0.5 * rs * 2f64.ln(),
        0.5 * (rs * r).sqrt(),
        2.0 * (rs * r).sqrt() * (2f64.sqrt() - 1.0),
    ];
    println!(
        "Earth: the ruler {:.4} mm longer (by hand {:.4}); its center {below:.3} m below its surface (by hand {:.3}); twice its radius out {above:.3} m above (by hand {:.3})",
        excess * 1e3,
        hands[0] * 1e3,
        hands[1],
        hands[2]
    );
    for (value, hand) in [excess, below, above].iter().zip(hands) {
        assert!((value / hand - 1.0).abs() < 1e-8);
    }
}

#[test]
fn gravitational_waves_are_as_worked_out_by_hand() {
    // Two holes of 36 and 29 Suns (GW150914's), sending waves at 100 Hz.
    let (gm1, gm2) = (36.0 * GM_SUN, 29.0 * GM_SUN);
    let chirp = chirp_gm(gm1, gm2);
    println!("chirp mass {:.2} Suns", chirp / GM_SUN);
    // Leading order's time to merge is Peters' for a circular orbit, whose
    // period is twice the waves'.
    let tau = time_to_merge(chirp, 100.0);
    let peters = merger_time(gm1, gm2, 2.0 / 100.0, 0.0);
    println!(
        "at 100 Hz they merge in {:.4} s (Peters: {:.4} s); the waves are {:.0} km long",
        tau,
        peters,
        C / 100.0 / 1e3
    );
    assert!((tau / peters - 1.0).abs() < 1e-12);
    // The orbit's phase back in time: half a turn back takes about π/(πf),
    // a little longer since the orbit was slower; and the orbital
    // frequency, πf(t), adds up over that time to the phase.
    let back = lookback(chirp, 100.0, PI);
    let rule = worldline_core::milky_way::gauss_legendre(32);
    let turned: f64 = rule
        .iter()
        .map(|&(t, w)| {
            let ago = 0.5 * back * (t + 1.0);
            0.5 * back * w * PI * frequency_then(chirp, 100.0, ago)
        })
        .sum();
    println!(
        "half a turn back: {:.6} ms ago (π/(πf) = {:.6} ms); the orbit turned {turned:.12} rad since",
        back * 1e3,
        10.0
    );
    assert!((turned - PI).abs() < 1e-12 && back > 0.01);
    // A slow pair keeps its digits: half a turn of a 10-hour orbit.
    let slow = lookback(chirp, 2.0 / 36_000.0, PI);
    assert!((slow / 18_000.0 - 1.0).abs() < 1e-9);
    // The strain from 410 Mpc, face on: (4/R)(G𝓜/c²)^(5/3)(πf/c)^(2/3).
    let (plus, cross) = strain_amplitude(chirp, 100.0, 410e6 * PARSEC, 0.0);
    let hand = 4.0 / (410e6 * PARSEC)
        * (chirp / (C * C)).powf(5.0 / 3.0)
        * (PI * 100.0 / C).powf(2.0 / 3.0);
    println!("from 410 Mpc, face on: h = {plus:.3e} (by hand {hand:.3e})");
    assert!((plus - hand).abs() < 1e-15 * hand && (cross - hand).abs() < 1e-15 * hand);
    // And against the quadrupole formula with the bodies themselves, for a
    // wide pair at 10⁻³ Hz. They differ by the first order the leading
    // one leaves out, GM/rc² times a number of order 1: twice GM/rc² is
    // allowed. Face on, h₊² + h×² is the amplitude squared at every
    // instant; in the orbit's plane, h₊ peaks at half it.
    let (a, b) = circular_pair(Body::new("a", gm1, 1.0), Body::new("b", gm2, 1.0), 1e-3);
    let r = (a.position - b.position).length();
    let compact = (gm1 + gm2) / (r * C * C);
    let distance = 1e20;
    let face = strain(&[a.clone(), b.clone()], DVec3::Z, distance);
    let edge = strain(&[a.clone(), b.clone()], DVec3::Y, distance);
    let (amplitude, _) = strain_amplitude(chirp, 1e-3, distance, 0.0);
    let (in_plane, _) = strain_amplitude(chirp, 1e-3, distance, PI / 2.0);
    let face_off = face.plus.hypot(face.cross) / amplitude - 1.0;
    let edge_off = edge.plus.abs() / in_plane - 1.0;
    println!(
        "against the bodies' quadrupole: face on {face_off:.1e}, edge on {edge_off:.1e} (GM/rc² = {compact:.1e})"
    );
    assert!(face_off.abs() < 2.0 * compact && edge_off.abs() < 2.0 * compact);
}
