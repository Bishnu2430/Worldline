//! Validation: a neutron-star binary's gravitational-wave chirp sweeps up
//! in frequency as theory says.
//!
//! Two neutron stars (1.4 and 1.3 Suns) spiral in under Worldline's
//! post-Newtonian gravity, radiation reaction included, from a
//! gravitational-wave frequency of about 20 Hz to about 40 Hz: 140 seconds
//! and 3,700 wave cycles. Their waveform is computed as an observer 40 Mpc
//! away, face-on, would see it, and its frequency f and its rate of change
//! are read off the wave's zero crossings, as one would from a detector's
//! data.
//!
//! `cargo test --release -p worldline-core --test validation_chirp -- --nocapture`

use std::f64::consts::{PI, TAU};

use worldline_core::constants::{C, GM_SUN, PARSEC};
use worldline_core::gravitational_waves::strain;
use worldline_core::gravity::{PostNewtonian, relative_acceleration};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::{Body, DVec3, System};

/// A pair on a circular orbit of separation `r` in the x–y plane, in its
/// center-of-mass frame (to first post-Newtonian order), already falling
/// inward at the leading-order rate ṙ = −(64/5) G³m³ν/(r³c⁵) so the orbit
/// stays circular as it shrinks.
fn circular_pair(gm1: f64, gm2: f64, r: f64) -> System {
    let gm = gm1 + gm2;
    let (x1, x2) = (gm1 / gm, gm2 / gm);
    let nu = x1 * x2;
    let x = DVec3::new(r, 0.0, 0.0);
    // The circular speed under the conservative equations: v² = r |a·n|.
    let mut speed = (gm / r).sqrt();
    for _ in 0..20 {
        let a = relative_acceleration(gm, nu, x, DVec3::new(0.0, speed, 0.0));
        let inward = -(a.newtonian + a.first + a.second).x;
        speed = (r * inward).sqrt();
    }
    let rdot = -64.0 / 5.0 * gm.powi(3) * nu / (r.powi(3) * C.powi(5));
    let v = DVec3::new(rdot, speed, 0.0);
    let shift = nu * (x1 - x2) / (C * C) * (0.5 * v.length_squared() - 0.5 * gm / r) * v;
    System::new(vec![
        Body::new("one", gm1, 1.0).at(x * x2).moving(v * x2 + shift),
        Body::new("two", gm2, 1.0)
            .at(-x * x1)
            .moving(-v * x1 + shift),
    ])
}

/// The wave's frequency f and its rate of change, from a least-squares
/// parabola through the frequencies of consecutive half-cycles, around the
/// middle of `half_cycles` (time, frequency) pairs.
fn sweep(half_cycles: &[(f64, f64)]) -> (f64, f64) {
    let mid = half_cycles[half_cycles.len() / 2].0;
    let mut m = [[0.0f64; 3]; 3];
    let mut b = [0.0f64; 3];
    for &(t, f) in half_cycles {
        let s = t - mid;
        let basis = [1.0, s, s * s];
        for i in 0..3 {
            b[i] += basis[i] * f;
            for j in 0..3 {
                m[i][j] += basis[i] * basis[j];
            }
        }
    }
    for col in 0..3 {
        for row in col + 1..3 {
            let k = m[row][col] / m[col][col];
            let pivot = m[col];
            for (target, source) in m[row].iter_mut().zip(pivot).skip(col) {
                *target -= k * source;
            }
            b[row] -= k * b[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let rest: f64 = (row + 1..3).map(|j| m[row][j] * x[j]).sum();
        x[row] = (b[row] - rest) / m[row][row];
    }
    (x[0], x[1])
}

#[test]
fn the_chirp_sweeps_up_as_theory_says() {
    let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
    let gm = gm1 + gm2;
    let nu = gm1 * gm2 / (gm * gm);
    // The chirp mass, the combination the sweep depends on: G𝓜 =
    // (Gm₁ Gm₂)^(3/5) / (Gm)^(1/5).
    let chirp = (gm1 * gm2).powf(0.6) / gm.powf(0.2);
    let distance = 40e6 * PARSEC;

    // Start where the wave is at 20 Hz: orbital frequency 10 Hz.
    let start_omega = PI * 20.0;
    let r = (gm / (start_omega * start_omega)).cbrt();
    let mut system = circular_pair(gm1, gm2, r);
    let gravity = PostNewtonian::default();
    let mut ias = Ias15::new();

    // Sample h₊ 64 times an orbit until the wave passes 40 Hz, and time
    // its zero crossings (linear interpolation is exact to third order
    // there, where the wave's curvature vanishes).
    let mut crossings: Vec<f64> = Vec::new();
    let mut amplitude: Vec<(f64, f64)> = Vec::new();
    let mut previous = (0.0, strain(&system.bodies, DVec3::Z, distance).plus);
    loop {
        let (a, b) = (&system.bodies[0], &system.bodies[1]);
        let x = a.position - b.position;
        let v = a.velocity - b.velocity;
        let omega = x.cross(v).length() / x.length_squared();
        if omega / PI > 40.0 {
            break;
        }
        advance(&mut system, &gravity, &mut ias, TAU / omega / 64.0);
        let h = strain(&system.bodies, DVec3::Z, distance);
        let now = (system.time(), h.plus);
        if (previous.1 < 0.0) != (now.1 < 0.0) {
            crossings.push(previous.0 - previous.1 * (now.0 - previous.0) / (now.1 - previous.1));
        }
        amplitude.push((now.0, h.plus.hypot(h.cross)));
        previous = now;
    }
    let half_cycles: Vec<(f64, f64)> = crossings
        .windows(2)
        .map(|w| (0.5 * (w[0] + w[1]), 0.5 / (w[1] - w[0])))
        .collect();
    println!(
        "{} wave cycles over {:.1} s, from {:.2} to {:.2} Hz",
        half_cycles.len() / 2,
        system.time(),
        half_cycles[0].1,
        half_cycles[half_cycles.len() - 1].1
    );

    // At five points along the way: the measured sweep against the
    // formulas, in windows of 400 half-cycles.
    let window = 400;
    for k in 0..5 {
        let at = window / 2 + k * (half_cycles.len() - window) / 4;
        let (f, fdot) = sweep(&half_cycles[at - window / 2..at + window / 2]);
        // x = (π G m f / c³)^(2/3), the post-Newtonian parameter, about
        // GM/(rc²).
        let x = (PI * gm * f / C.powi(3)).powf(2.0 / 3.0);
        // The chirp formula (leading order) and its first correction,
        // −(743/336 + 11ν/4) x, from Blanchet (2024)'s flux and energy for
        // circular orbits; and general relativity's next term, the tail,
        // +4π x^(3/2), which local equations of motion don't carry.
        let leading = 96.0 / 5.0
            * PI.powf(8.0 / 3.0)
            * (chirp / C.powi(3)).powf(5.0 / 3.0)
            * f.powf(11.0 / 3.0);
        let corrected = leading * (1.0 - (743.0 / 336.0 + 11.0 / 4.0 * nu) * x);
        let with_tail = corrected + leading * 4.0 * PI * x.powf(1.5);
        let miss = (fdot / leading - 1.0).abs();
        let corrected_miss = (fdot / corrected - 1.0).abs();
        // The face-on amplitude, 4 (G𝓜)^(5/3) (πf)^(2/3) / (c⁴ R).
        let expected_h =
            4.0 * chirp.powf(5.0 / 3.0) * (PI * f).powf(2.0 / 3.0) / (C.powi(4) * distance);
        let t = half_cycles[at].0;
        let h = amplitude
            .iter()
            .min_by(|p, q| (p.0 - t).abs().total_cmp(&(q.0 - t).abs()))
            .expect("samples")
            .1;
        let h_miss = (h / expected_h - 1.0).abs();
        println!(
            "f = {f:.3} Hz (x = {x:.4}): df/dt = {fdot:.5} Hz/s; chirp formula {leading:.5} (off by {miss:.1e} = {:.1} x, bound {:.1e}); with its 1PN term {corrected:.5} (off by {corrected_miss:.1e} = {:.0} x², bound {x:.1e}); general relativity with the tail {with_tail:.5}; amplitude {h:.3e} vs {expected_h:.3e} (off by {h_miss:.1e})",
            miss / x,
            x.sqrt(),
            corrected_miss / (x * x)
        );
        // 1. The chirp formula holds to its next order, x: allowed √x.
        assert!(miss < x.sqrt());
        // 2. With the 1PN term (which the 3.5PN reaction terms produce),
        //    what's left must be of second order: less than x. Without the
        //    3.5PN terms the sweep runs 6.7x fast instead of 3.2x slow
        //    (measured at 20 Hz), missing this formula by 10x, and the test
        //    fails. What's left is about 25x², from the 4.5PN reaction the
        //    simulation doesn't carry, as the 3.5PN one carried 10x.
        assert!(corrected_miss < x);
        // 3. The amplitude, to its next order: allowed √x.
        assert!(h_miss < x.sqrt());
    }
}
