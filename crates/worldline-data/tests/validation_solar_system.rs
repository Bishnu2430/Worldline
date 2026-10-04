//! Validation: one year of the real solar system against NASA JPL.
//!
//! Start from JPL's state of the solar system on 2025-01-01, simulate one
//! Julian year, and compare every body with where JPL says it was on
//! 2026-01-01 06:00 TDB. Run it twice: with Newtonian gravity, and with
//! relativistic (Einstein–Infeld–Hoffmann) gravity, which JPL also uses.
//!
//! JPL's ephemeris also includes asteroids, the Sun's oblateness and
//! Earth–Moon tides, which Worldline doesn't model yet.

use worldline_core::constants::{AU, DAY};
use worldline_core::gravity::{EinsteinInfeldHoffmann, Gravity, Newtonian};
use worldline_core::integrator::{Ias15, advance};
use worldline_data::{Snapshot, solar_system};

fn reference() -> Snapshot {
    Snapshot::parse(include_str!("../data/solar-system-2026-01-01T06.csv"))
        .expect("valid reference data")
}

/// Simulates one year from JPL's 2025 state and returns each body's name,
/// position error (m) and distance from the solar system barycenter (m).
fn one_year_errors(gravity: &dyn Gravity) -> Vec<(String, f64, f64)> {
    let start = solar_system();
    let end = reference();
    let mut system = start.system();
    let mut ias = Ias15::new();
    let duration = (end.epoch_jd_tdb - start.epoch_jd_tdb) * DAY;
    advance(&mut system, gravity, &mut ias, duration);
    system
        .bodies
        .iter()
        .zip(&end.bodies)
        .map(|(simulated, expected)| {
            assert_eq!(simulated.name, expected.name);
            let error = (simulated.position - expected.position).length();
            (simulated.name.clone(), error, expected.position.length())
        })
        .collect()
}

#[test]
fn one_year_matches_jpl() {
    let newtonian = one_year_errors(&Newtonian);
    let relativistic = one_year_errors(&EinsteinInfeldHoffmann);

    println!(
        "{:<8} {:>15} {:>17} {:>13}",
        "body", "Newtonian (km)", "relativistic (km)", "improvement"
    );
    for ((name, newton_error, distance), (_, einstein_error, _)) in
        newtonian.iter().zip(&relativistic)
    {
        println!(
            "{:<8} {:>15.2} {:>17.2} {:>12.0}×",
            name,
            newton_error / 1e3,
            einstein_error / 1e3,
            newton_error / einstein_error
        );
        // Both models: every body within 10 parts per million of its distance.
        for error in [newton_error, einstein_error] {
            assert!(
                error / distance < 1e-5,
                "{name} off by {:.0} km",
                error / 1e3
            );
        }
    }

    // Step 1.3's prediction: relativity was the main physics missing for
    // the inner planets, so adding it must cut their errors at least 10×.
    for planet in ["Mercury", "Venus", "Earth", "Mars"] {
        let error = |errors: &[(String, f64, f64)]| {
            errors.iter().find(|(name, ..)| name == planet).unwrap().1
        };
        let improvement = error(&newtonian) / error(&relativistic);
        assert!(
            improvement > 10.0,
            "{planet} improved only {improvement:.1}×"
        );
    }

    // The roadmap's criterion: Earth within 1 part in 10,000 of an AU.
    let earth_error = relativistic[3].1;
    assert!(
        earth_error < 1e-4 * AU,
        "Earth off by {:.0} km",
        earth_error / 1e3
    );
}
