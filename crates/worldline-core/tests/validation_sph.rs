//! Validation: a self-gravitating fluid in SPH (step 4.1).
//!
//! The roadmap's check: a star or planet left alone stays in equilibrium.
//! Jupiter is close to an n = 1 polytrope (P = Kρ²), whose structure is
//! known exactly (Lane–Emden: ρ = ρ_c sin ξ/ξ, ξ = πr/R). Built from SPH
//! particles, let settle, then left alone for ten dynamical times
//! (√(R³/GM), 27 minutes for Jupiter), it must neither collapse nor fly
//! apart, keep its energy, and have the polytrope's structure, to what SPH
//! can resolve: its smoothing length, h.

use worldline_core::constants::G;
use worldline_core::sph::{Fluid, polytrope, polytrope_mass};

/// Jupiter's mass, kg, and mean radius, m (IAU 2015 nominal: GM =
/// 1.266 865 3 × 10¹⁷ m³/s²; R = 69,911 km).
const MASS: f64 = 1.266_865_3e17 / G;
const RADIUS: f64 = 6.9911e7;

fn dynamical_time() -> f64 {
    (RADIUS.powi(3) / (G * MASS)).sqrt()
}

/// Jupiter as about `count` particles, let settle for five dynamical
/// times at the polytrope's entropy, its motion damped with time constant
/// 0.3 of one.
fn settled(count: usize) -> Fluid {
    let (mut fluid, k) = polytrope(MASS, RADIUS, count);
    fluid.settle(k, 5.0 * dynamical_time(), 0.3 * dynamical_time());
    fluid
}

/// The radius holding half the mass of an n = 1 polytrope, as a fraction
/// of R: where (sin ξ − ξ cos ξ)/π = ½, by halving.
fn half_mass_radius() -> f64 {
    let (mut lo, mut hi) = (0.0, RADIUS);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if polytrope_mass(RADIUS, mid) < 0.5 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi) / RADIUS
}

/// The mean smoothing length, as a fraction of R.
fn mean_h(fluid: &Fluid) -> f64 {
    fluid.particles.iter().map(|p| p.h).sum::<f64>() / fluid.particles.len() as f64 / RADIUS
}

#[test]
fn a_planet_left_alone_stays_in_equilibrium() {
    // About 1,000 particles. SPH places mass no finer than its smoothing
    // length, h, and its particles jiggle around their places by a
    // fraction of it. So: over ten dynamical times the half-mass radius
    // must stay within h/2 of where it starts, and the kinetic energy
    // under that of an oscillation that size, (h/2R)² of the binding
    // energy. Without pressure it would fall in within one (free fall
    // takes 1.1); with too much it would fly apart as fast.
    let mut fluid = settled(1000);
    let t_dyn = dynamical_time();
    let h = mean_h(&fluid);
    let start = fluid.mass_radius(0.5) / RADIUS;
    let energy = fluid.energy();
    let (mut moved, mut stirred, mut steps): (f64, f64, usize) = (0.0, 0.0, 0);
    let begun = fluid.time;
    while fluid.time - begun < 10.0 * t_dyn {
        fluid.step();
        steps += 1;
        moved = moved.max((fluid.mass_radius(0.5) / RADIUS - start).abs());
        stirred = stirred.max(fluid.kinetic() / fluid.gravitational().abs());
    }
    let drift = (fluid.energy() - energy) / energy.abs();
    let step = 10.0 / steps as f64;
    println!(
        "{} particles, mean h = {h:.3} R: over 10 dynamical times ({steps} steps), the half-mass radius moved at most {moved:.4} R ({:.2} h) from {start:.4} R; kinetic energy at most {stirred:.1e} of the binding energy (limit {:.1e}); energy kept to {drift:.1e} (limit (Δt/t_dyn)² = {:.1e})",
        fluid.particles.len(),
        moved / h,
        (0.5 * h).powi(2),
        step * step
    );
    assert!(moved < 0.5 * h);
    assert!(stirred < (0.5 * h).powi(2));
    // Leapfrog keeps energy to its steps' error, of order (Δt/t_dyn)².
    assert!(drift.abs() < step * step);
    // The polytrope's structure, to SPH's resolution: its half-mass radius
    // within a smoothing length of Lane–Emden's, 0.6062 R.
    let exact = half_mass_radius();
    println!("its half-mass radius {start:.4} R against Lane–Emden's {exact:.4} R");
    assert!((start - exact).abs() < h);
}

#[test]
#[ignore = "slow: 4,000 particles, about a minute"]
fn with_more_particles_it_comes_closer_to_lane_and_emdens_structure() {
    // SPH's error is of order h², and h ∝ N^(−1/3): four times the
    // particles should bring the half-mass radius at least 0.4 times as
    // close to Lane–Emden's.
    let exact = half_mass_radius();
    let errors: Vec<(usize, f64, f64)> = [1000, 4000]
        .iter()
        .map(|&n| {
            let fluid = settled(n);
            let r = fluid.mass_radius(0.5) / RADIUS;
            (fluid.particles.len(), r, (r - exact).abs())
        })
        .collect();
    for (n, r, error) in &errors {
        println!(
            "{n} particles: half-mass radius {r:.4} R, {error:.4} R off Lane–Emden's {exact:.4} R"
        );
    }
    assert!(errors[1].2 < 0.4 * errors[0].2);
}
