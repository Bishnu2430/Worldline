//! Validation: Newtonian two-body orbits against Kepler's laws and the
//! conservation laws.
//!
//! Kepler's third law, T = 2π √(a³ / GM), is an exact consequence of
//! Newtonian gravity. If the gravity model and integrator are right, the
//! simulated orbital periods must match it. Energy, momentum and angular
//! momentum of an isolated system must stay constant.

use std::f64::consts::TAU;

use worldline_core::constants::{AU, DAY, GM_SUN, JULIAN_YEAR, SOLAR_RADIUS};
use worldline_core::diagnostics::{angular_momentum, linear_momentum, newtonian_energy};
use worldline_core::gravity::Newtonian;
use worldline_core::integrator::{Integrator, Leapfrog};
use worldline_core::orbit::{kepler_period, two_body_system};
use worldline_core::{Body, DVec3, System};

fn sun() -> Body {
    Body::new("Sun", GM_SUN, SOLAR_RADIUS)
}

/// Integrates a two-body system until the relative position vector has
/// swept a full turn, and returns the time that took.
fn measure_period(system: &mut System, dt: f64) -> f64 {
    let mut leapfrog = Leapfrog::new(dt);
    let relative = |s: &System| s.bodies[1].position - s.bodies[0].position;
    let mut swept = 0.0;
    let mut previous = relative(system);
    loop {
        let t0 = system.time();
        leapfrog.step(system, &Newtonian, f64::INFINITY);
        let current = relative(system);
        // Signed angle turned during this step, in the orbital (x–y) plane.
        let turned = previous.cross(current).z.atan2(previous.dot(current));
        if swept + turned >= TAU {
            // The full turn completed inside this step; interpolate when.
            return t0 + (TAU - swept) / turned * dt;
        }
        swept += turned;
        previous = current;
    }
}

#[test]
fn orbital_periods_obey_keplers_third_law() {
    // (name, semi-major axis, eccentricity, GM of the orbiting body)
    let orbits = [
        ("Mercury-like", 0.387_098 * AU, 0.205_630, 2.2032e13),
        ("Earth-like", 1.0 * AU, 0.016_7, 4.035_03e14),
        ("Jupiter-like", 5.2026 * AU, 0.048_4, 1.267_127_64e17),
        ("Comet-like", 3.0 * AU, 0.7, 1.0e9),
    ];
    for (name, a, e, gm) in orbits {
        let expected = kepler_period(a, GM_SUN + gm);
        let mut system = two_body_system(sun(), Body::new(name, gm, 1.0), a, e);
        let measured = measure_period(&mut system, expected / 50_000.0);
        let rel_error = (measured - expected).abs() / expected;
        println!(
            "{name:>13}: expected {:.6} days, simulated {:.6} days, relative error {rel_error:.1e}",
            expected / DAY,
            measured / DAY,
        );
        assert!(rel_error < 1e-6, "{name}: period off by {rel_error:e}");
    }
}

/// Integrates an eccentric (e = 0.5) orbit for `orbits` orbits and returns
/// the largest relative energy error seen during the first `window` orbits
/// and during the last `window` orbits.
fn energy_error_bands(steps_per_orbit: usize, orbits: usize, window: usize) -> (f64, f64) {
    let a = AU;
    let mut system = two_body_system(sun(), Body::new("planet", 4.0e14, 1.0), a, 0.5);
    let period = kepler_period(a, system.total_gm());
    let mut leapfrog = Leapfrog::new(period / steps_per_orbit as f64);
    let e0 = newtonian_energy(&system);

    let (mut early, mut late): (f64, f64) = (0.0, 0.0);
    for orbit in 0..orbits {
        for _ in 0..steps_per_orbit {
            leapfrog.step(&mut system, &Newtonian, f64::INFINITY);
            let error = ((newtonian_energy(&system) - e0) / e0).abs();
            if orbit < window {
                early = early.max(error);
            }
            if orbit >= orbits - window {
                late = late.max(error);
            }
        }
    }
    (early, late)
}

#[test]
fn energy_error_stays_bounded_without_drifting() {
    // A symplectic integrator's energy error oscillates within a band but
    // never grows. Compare the band early on with the band 1,900 orbits later.
    let (early, late) = energy_error_bands(1_000, 2_000, 100);
    println!("max relative energy error: orbits 1–100 {early:.3e}, orbits 1901–2000 {late:.3e}");
    assert!(
        late < 1.5 * early,
        "energy is drifting: {early:e} → {late:e}"
    );
}

#[test]
fn energy_error_shrinks_fourfold_when_step_halves() {
    // Leapfrog is second-order accurate: its error scales as (step size)².
    let (coarse, _) = energy_error_bands(1_000, 100, 100);
    let (fine, _) = energy_error_bands(2_000, 100, 100);
    let ratio = coarse / fine;
    println!("energy error {coarse:.3e} at 1000 steps/orbit, {fine:.3e} at 2000: ratio {ratio:.3}");
    assert!(
        (3.6..4.4).contains(&ratio),
        "expected a ratio near 4, got {ratio}"
    );
}

#[test]
fn momentum_and_angular_momentum_are_conserved() {
    // Sun, Earth and Jupiter on circular orbits, run for a century.
    let circular = |name: &str, gm: f64, r: f64| {
        let v = (GM_SUN / r).sqrt();
        Body::new(name, gm, 1.0)
            .at(DVec3::new(r, 0.0, 0.0))
            .moving(DVec3::new(0.0, v, 0.0))
    };
    let mut system = System::new(vec![
        sun(),
        circular("Earth", 4.035_03e14, AU),
        circular("Jupiter", 1.267_127_64e17, 5.2026 * AU),
    ]);
    system.move_to_barycentric_frame();

    let p0 = linear_momentum(&system);
    let l0 = angular_momentum(&system);
    // Momentum starts at zero, so measure its change against the size of
    // the individual bodies' momenta.
    let momentum_scale: f64 = system
        .bodies
        .iter()
        .map(|b| b.mass() * b.velocity.length())
        .sum();

    let mut leapfrog = Leapfrog::new(DAY);
    worldline_core::integrator::advance(
        &mut system,
        &Newtonian,
        &mut leapfrog,
        100.0 * JULIAN_YEAR,
    );

    let dp = (linear_momentum(&system) - p0).length() / momentum_scale;
    let dl = (angular_momentum(&system) - l0).length() / l0.length();
    println!("after 100 years: momentum change {dp:.1e}, angular momentum change {dl:.1e}");
    assert!(dp < 1e-12, "momentum changed by {dp:e}");
    assert!(dl < 1e-12, "angular momentum changed by {dl:e}");
}
