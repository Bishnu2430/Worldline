//! Validation: the IAS15 integrator on Newtonian orbits.
//!
//! IAS15 should keep errors at the level of double-precision rounding:
//! energy errors that only random-walk (Brouwer's law), orbits that close
//! after many periods even when highly eccentric, and step sizes that adapt
//! to how fast the orbit is changing.

use worldline_core::constants::{AU, GM_SUN, SOLAR_RADIUS};
use worldline_core::diagnostics::newtonian_energy;
use worldline_core::gravity::Newtonian;
use worldline_core::integrator::{Ias15, Integrator, advance};
use worldline_core::orbit::{kepler_period, two_body_system};
use worldline_core::{Body, System};

fn orbit(e: f64) -> System {
    two_body_system(
        Body::new("Sun", GM_SUN, SOLAR_RADIUS),
        Body::new("planet", 4.0e14, 1.0),
        AU,
        e,
    )
}

fn relative_position(system: &System) -> worldline_core::DVec3 {
    system.bodies[1].position - system.bodies[0].position
}

#[test]
fn energy_error_random_walks_at_rounding_level() {
    let mut system = orbit(0.5);
    let period = kepler_period(AU, system.total_gm());
    let end = 10_000.0 * period;
    let e0 = newtonian_energy(&system);
    let mut ias = Ias15::new();
    let (mut steps, mut max_error) = (0.0_f64, 0.0_f64);
    while system.time() < end {
        let remaining = end - system.time();
        ias.step(&mut system, &Newtonian, remaining);
        steps += 1.0;
        max_error = max_error.max(((newtonian_energy(&system) - e0) / e0).abs());
    }
    let bound = 10.0 * f64::EPSILON * steps.sqrt();
    println!(
        "10,000 orbits in {steps} steps ({:.0} per orbit): max energy error {max_error:.1e}, rounding bound {bound:.1e}",
        steps / 10_000.0
    );
    assert!(
        max_error < bound,
        "energy error {max_error:e} exceeds {bound:e}"
    );
}

#[test]
fn eccentric_orbits_close_after_100_periods() {
    // A Newtonian two-body orbit is exactly periodic. Step 1.4 must detect
    // relativity shifting Mercury's perihelion by ~2 × 10⁻⁵ AU over 100
    // orbits, so integrator error must be far below that: require 10⁻⁹ AU.
    for e in [0.0167, 0.5, 0.9, 0.99] {
        let mut system = orbit(e);
        let start = relative_position(&system);
        let period = kepler_period(AU, system.total_gm());
        let mut ias = Ias15::new();
        advance(&mut system, &Newtonian, &mut ias, 100.0 * period);
        let miss = (relative_position(&system) - start).length() / AU;
        println!("e = {e:<6}: after 100 orbits, returns to periapsis within {miss:.1e} AU");
        assert!(miss < 1e-9, "e = {e}: missed by {miss:e} AU");
    }
}

#[test]
fn step_size_follows_orbital_speed() {
    let e = 0.9;
    let mut system = orbit(e);
    let period = kepler_period(AU, system.total_gm());
    let mut ias = Ias15::new();
    // Let the step size settle, then record one full orbit: each step's
    // size and the distance between the bodies at the time.
    advance(&mut system, &Newtonian, &mut ias, period);
    let end = system.time() + period;
    let mut steps: Vec<(f64, f64)> = Vec::new();
    while system.time() < end {
        let remaining = end - system.time();
        let r = relative_position(&system).length();
        let dt = ias.step(&mut system, &Newtonian, remaining);
        if dt < remaining {
            steps.push((dt, r));
        }
    }
    let (shortest, r_at_shortest) = steps
        .iter()
        .copied()
        .fold((f64::INFINITY, 0.0), |m, s| if s.0 < m.0 { s } else { m });
    let (longest, r_at_longest) = steps
        .iter()
        .copied()
        .fold((0.0, 0.0), |m, s| if s.0 > m.0 { s } else { m });
    let (r_peri, r_apo) = (AU * (1.0 - e), AU * (1.0 + e));
    println!(
        "e = {e}: {} steps per orbit; shortest {:.2} h at r = {:.2} r_peri, longest {:.1} h at r = {:.2} r_apo ({:.0}× longer)",
        steps.len(),
        shortest / 3600.0,
        r_at_shortest / r_peri,
        longest / 3600.0,
        r_at_longest / r_apo,
        longest / shortest,
    );
    // Short steps where the orbit changes fast (near the Sun), long steps
    // where it changes slowly (far away).
    assert!(
        r_at_shortest < 1.5 * r_peri,
        "shortest step not near periapsis"
    );
    assert!(r_at_longest > 0.8 * r_apo, "longest step not near apoapsis");
    assert!(longest / shortest > 10.0, "step size barely adapts");
}
