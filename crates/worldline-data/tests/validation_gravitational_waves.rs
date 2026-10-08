//! Validation: the Hulse–Taylor binary pulsar loses energy to
//! gravitational waves as measured.
//!
//! The pulsar and its companion, two neutron stars of the catalog's
//! published masses, are simulated with Worldline's post-Newtonian gravity,
//! radiation reaction included, on the measured orbit. The time of every
//! periastron passage is recorded, as pulsar timing records them, and a
//! parabola through them gives how fast the period shrinks.
//!
//! `cargo test --release -p worldline-data --test validation_gravitational_waves -- --nocapture`

use std::f64::consts::TAU;

use worldline_core::constants::{C, GM_SUN, JULIAN_YEAR};
use worldline_core::gravitational_waves::{merger_time, period_derivative};
use worldline_core::gravity::{Gravity, PostNewtonian};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::orbit::two_body_system;
use worldline_core::{DVec3, System};
use worldline_data::{binary_orbits, notable_object};

/// Relative position and velocity of the pair.
fn relative(system: &System) -> (DVec3, DVec3) {
    let (a, b) = (&system.bodies[0], &system.bodies[1]);
    (b.position - a.position, b.velocity - a.velocity)
}

/// A closest or farthest point of the orbit: when, and how far apart.
struct Turn {
    time: f64,
    distance: f64,
    closest: bool,
}

/// Runs the pair until it has passed periastron `orbits` times, and returns
/// every periastron and apastron passage: the moments r·v changes sign
/// (from − to + at periastron), each found to a tiny fraction of a
/// microsecond by Newton's method on a copy of the pair.
fn turns(mut system: System, gravity: &dyn Gravity, period: f64, orbits: usize) -> Vec<Turn> {
    let mut ias = Ias15::new();
    let dt = period / 32.0;
    let mut out: Vec<Turn> = Vec::new();
    let radial = |s: &System| {
        let (r, v) = relative(s);
        r.dot(v)
    };
    while out.iter().filter(|t| t.closest).count() < orbits {
        let before = system.clone();
        let was = radial(&system);
        advance(&mut system, gravity, &mut ias, dt);
        if (was < 0.0) != (radial(&system) < 0.0) {
            // Newton's method from the sample before: d(r·v)/dt = v² + r·a.
            let mut tau = 0.0;
            let mut probe = before.clone();
            for _ in 0..6 {
                probe = before.clone();
                if tau > 0.0 {
                    advance(&mut probe, gravity, &mut Ias15::new(), tau);
                }
                let (r, v) = relative(&probe);
                let gm = probe.bodies[0].gm + probe.bodies[1].gm;
                let a = -gm * r / r.length().powi(3);
                tau -= r.dot(v) / (v.length_squared() + r.dot(a));
            }
            out.push(Turn {
                time: before.time() + tau,
                distance: relative(&probe).0.length(),
                closest: was < 0.0,
            });
        }
    }
    out
}

/// The times of the periastron passages.
fn periastron_times(turns: &[Turn]) -> Vec<f64> {
    turns.iter().filter(|t| t.closest).map(|t| t.time).collect()
}

/// The orbit's period, and its radial eccentricity (r_far − r_close) /
/// (r_far + r_close), from its first passages.
fn period_and_eccentricity(turns: &[Turn]) -> (f64, f64) {
    let times = periastron_times(turns);
    let period = (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64;
    let close = turns.iter().find(|t| t.closest).expect("a periastron");
    let far = turns.iter().find(|t| !t.closest).expect("an apastron");
    let (close, far) = (close.distance, far.distance);
    (period, (far - close) / (far + close))
}

/// Least-squares parabola t = c₀ + c₁ k + c₂ k² through the passage times.
fn parabola(times: &[f64]) -> [f64; 3] {
    // Centered and scaled for a well-conditioned fit.
    let n = times.len() as f64;
    let mid = (n - 1.0) / 2.0;
    let mut m = [[0.0f64; 3]; 3];
    let mut b = [0.0f64; 3];
    for (k, &t) in times.iter().enumerate() {
        let s = (k as f64 - mid) / mid;
        let basis = [1.0, s, s * s];
        for i in 0..3 {
            b[i] += basis[i] * (t - times[0]);
            for j in 0..3 {
                m[i][j] += basis[i] * basis[j];
            }
        }
    }
    // Gaussian elimination.
    for col in 0..3 {
        for row in col + 1..3 {
            let f = m[row][col] / m[col][col];
            let pivot = m[col];
            for (target, source) in m[row].iter_mut().zip(pivot).skip(col) {
                *target -= f * source;
            }
            b[row] -= f * b[col];
        }
    }
    let mut x = [0.0; 3];
    for row in (0..3).rev() {
        let rest: f64 = (row + 1..3).map(|j| m[row][j] * x[j]).sum();
        x[row] = (b[row] - rest) / m[row][row];
    }
    // Back to t = c₀ + c₁ k + c₂ k² in the unscaled count k.
    let (a0, a1, a2) = (x[0], x[1] / mid, x[2] / (mid * mid));
    [
        times[0] + a0 - a1 * mid + a2 * mid * mid,
        a1 - 2.0 * a2 * mid,
        a2,
    ]
}

/// How far the simulated dP/dt may sit from Weisberg & Huang's prediction,
/// as a fraction: the rounding of the catalog's masses, plus the
/// prediction's own uncertainty.
fn allowed_against_prediction(predicted: f64, sigma: f64) -> f64 {
    let rounding = 0.0005 / 1.438 + 0.0005 / 1.390 + 0.001 / 2.828 / 3.0;
    rounding + (sigma / predicted).abs()
}

#[test]
fn the_hulse_taylor_orbit_shrinks_as_measured() {
    let pulsar = notable_object("PSR B1913+16").expect("in the catalog");
    let companion = notable_object("PSR B1913+16 companion").expect("in the catalog");
    let orbit = binary_orbits()
        .into_iter()
        .find(|o| o.system == "PSR B1913+16")
        .expect("the orbit is in the catalog");
    let (gm1, gm2) = (pulsar.gm(), companion.gm());
    let gm = gm1 + gm2;
    let e = orbit.eccentricity;
    let gravity = PostNewtonian::default();
    let pair = |a: f64, e: f64| two_body_system(pulsar.body(), companion.body(), a, e);

    // Kepler's laws give the orbit's size and shape, but relativity changes
    // an orbit started from them: its period comes out 1.5 × 10⁻⁴ longer and
    // its eccentricity 3.6 × 10⁻⁵ larger. Adjust both until the simulated
    // orbit has the measured period and eccentricity.
    let (mut a, mut start_e) = ((gm * (orbit.period / TAU).powi(2)).cbrt(), e);
    for _ in 0..3 {
        let first = turns(pair(a, start_e), &gravity, orbit.period, 3);
        let (period, simulated_e) = period_and_eccentricity(&first);
        a *= (orbit.period / period).powf(2.0 / 3.0);
        start_e += e - simulated_e;
    }

    // A thousand orbits, 0.88 years: the shrinking adds up to a delay of
    // ½ P (dP/dt) N² ≈ 34 ms by the last passage.
    let orbits = 1000;
    let all = turns(pair(a, start_e), &gravity, orbit.period, orbits);
    let (_, simulated_e) = period_and_eccentricity(&all[..6]);
    let [_, period, curvature] = parabola(&periastron_times(&all));
    let simulated = 2.0 * curvature / period;

    let formula = period_derivative(gm1, gm2, period, e);
    let (measured, sigma) = orbit.period_derivative;
    let (predicted, predicted_sigma) = orbit.period_derivative_gr;
    // The pair's field strength at periastron, GM/(r c²): the size of the
    // next post-Newtonian order, relative to what's kept.
    let strength = gm / (a * (1.0 - e) * C * C);
    println!(
        "Hulse–Taylor over {orbits} orbits (period {:.6} h, eccentricity {simulated_e:.7}):",
        period / 3600.0
    );
    println!("  simulated dP/dt  {:.5e}", simulated);
    println!(
        "  Peters & Mathews {:.5e} (same masses and orbit): {:.1e} apart (bound {:.1e})",
        formula,
        (simulated / formula - 1.0).abs(),
        strength.sqrt()
    );
    println!(
        "  general relativity's prediction, Weisberg & Huang 2016: {:.5e} ± {:.5e}: {:.1e} apart (bound {:.1e})",
        predicted,
        predicted_sigma,
        (simulated / predicted - 1.0).abs(),
        allowed_against_prediction(predicted, predicted_sigma)
    );
    println!(
        "  measured, corrected for the galaxy's pull: {:.3e} ± {:.3e} ({:+.1} σ from the simulation)",
        measured,
        sigma,
        (simulated - measured) / sigma
    );
    println!(
        "  the pair merges in {:.0} million years",
        merger_time(gm1, gm2, period, e) / JULIAN_YEAR / 1e6
    );

    // The simulated orbit is the measured one.
    assert!((period / orbit.period - 1.0).abs() < 1e-6);
    assert!((simulated_e - e).abs() < 1e-6);

    // 1. The textbook formula, from the same masses and orbit: what
    //    differs is the next post-Newtonian order, about GM/(rc²) of it
    //    (5 × 10⁻⁶); a mistake would be of order one. The test allows the
    //    midpoint, √(GM/rc²).
    assert!((simulated / formula - 1.0).abs() < strength.sqrt());

    // 2. General relativity's prediction from the measured masses (to
    //    0.00005 of 2.40263): the catalog keeps the masses to 0.001 Suns,
    //    and dP/dt goes as m₁ m₂ (m₁+m₂)^(−1/3), so rounding them can shift
    //    it by up to 0.0005/1.438 + 0.0005/1.390 + (1/3)(0.001/2.828).
    assert!((pulsar.gm() / GM_SUN - 1.438).abs() < 1e-12);
    assert!(
        (simulated / predicted - 1.0).abs()
            < allowed_against_prediction(predicted, predicted_sigma)
    );

    // 3. The roadmap: the period shrinks at −2.40 × 10⁻¹² s/s.
    assert!((simulated - -2.40e-12).abs() < 0.005e-12);
}

#[test]
fn without_radiation_reaction_the_period_holds() {
    // The same pair with the conservative equations alone: nothing drains
    // its energy, so the period must hold. Whatever drift the integrator and
    // rounding leave must be smaller than the precision the check above
    // demands, or they could pass for physics.
    let pulsar = notable_object("PSR B1913+16").expect("in the catalog");
    let companion = notable_object("PSR B1913+16 companion").expect("in the catalog");
    let orbit = &binary_orbits()[0];
    let gm = pulsar.gm() + companion.gm();
    let a = (gm * (orbit.period / TAU).powi(2)).cbrt();
    let system = two_body_system(pulsar.body(), companion.body(), a, orbit.eccentricity);
    let all = turns(system, &PostNewtonian::conservative(), orbit.period, 1000);
    let [_, period, curvature] = parabola(&periastron_times(&all));
    let drift = 2.0 * curvature / period;
    let (predicted, sigma) = orbit.period_derivative_gr;
    let bound = allowed_against_prediction(predicted, sigma) * predicted.abs();
    println!("without radiation reaction: dP/dt = {drift:.2e} (bound {bound:.1e})");
    assert!(drift.abs() < bound);
}
