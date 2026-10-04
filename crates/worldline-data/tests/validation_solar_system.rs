//! Validation: one year of the real solar system against NASA JPL.
//!
//! Start from JPL's state of the solar system on 2025-01-01, simulate one
//! Julian year, and compare every body with where JPL says it was on
//! 2026-01-01 06:00 TDB.
//!
//! JPL's ephemeris includes general relativity, asteroids and the Sun's
//! oblateness; this simulation is Newtonian point masses only. The
//! leftover errors are largest for the inner planets, where relativity is
//! strongest. Step 1.4 adds relativity and should shrink them.

use worldline_core::constants::{AU, DAY};
use worldline_core::gravity::Newtonian;
use worldline_core::integrator::{Ias15, advance};
use worldline_data::{Snapshot, solar_system};

fn reference() -> Snapshot {
    Snapshot::parse(include_str!("../data/solar-system-2026-01-01T06.csv"))
        .expect("valid reference data")
}

#[test]
fn one_year_matches_jpl() {
    let start = solar_system();
    let end = reference();
    let mut system = start.system();
    let mut ias = Ias15::new();
    let duration = (end.epoch_jd_tdb - start.epoch_jd_tdb) * DAY;
    advance(&mut system, &Newtonian, &mut ias, duration);

    println!(
        "{:<8} {:>11} {:>22}",
        "body", "error (km)", "error / distance (ppm)"
    );
    for (simulated, expected) in system.bodies.iter().zip(&end.bodies) {
        assert_eq!(simulated.name, expected.name);
        let error = (simulated.position - expected.position).length();
        // Distance from the solar system's barycenter, the origin of JPL's frame.
        let relative = error / expected.position.length();
        println!(
            "{:<8} {:>11.1} {:>22.3}",
            simulated.name,
            error / 1e3,
            relative * 1e6
        );
        // Every body within 10 parts per million of its distance.
        assert!(
            relative < 1e-5,
            "{} off by {:.0} km",
            simulated.name,
            error / 1e3
        );
    }

    // The roadmap's criterion: Earth within 1 part in 10,000 of an AU.
    let earth_error = (system.bodies[3].position - end.bodies[3].position).length();
    assert!(
        earth_error < 1e-4 * AU,
        "Earth off by {:.0} km",
        earth_error / 1e3
    );
}
