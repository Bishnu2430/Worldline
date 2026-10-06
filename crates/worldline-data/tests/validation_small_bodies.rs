//! Validation: dwarf planets, asteroids and comets against NASA JPL.

use worldline_core::constants::{AU, DAY};
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::hierarchy::Hierarchy;
use worldline_core::integrator::{Ias15, advance};
use worldline_core::{DVec3, System};
use worldline_data::{
    Snapshot, full_solar_system, jpl_halley_perihelion_jd, small_bodies, small_bodies_one_year_on,
    solar_system,
};

/// One year of the planets with relativistic gravity, with or without the
/// massive asteroids and dwarf planets, and each planet's error (km)
/// against JPL.
fn planet_errors(with_asteroids: bool) -> Vec<(String, f64)> {
    let start = solar_system();
    let end = Snapshot::parse(include_str!("../data/solar-system-2026-01-01T06.csv"))
        .expect("valid reference");
    let mut system: System = start.system();
    if with_asteroids {
        system.bodies.extend(
            small_bodies()
                .iter()
                .filter(|b| b.gm.is_some())
                .map(|b| b.body()),
        );
    }
    advance(
        &mut system,
        &EinsteinInfeldHoffmann,
        &mut Ias15::new(),
        (end.epoch_jd_tdb - start.epoch_jd_tdb) * DAY,
    );
    end.bodies
        .iter()
        .zip(&system.bodies)
        .map(|(expected, simulated)| {
            assert_eq!(expected.name, simulated.name);
            (
                expected.name.clone(),
                (simulated.position - expected.position).length() / 1e3,
            )
        })
        .collect()
}

#[test]
fn the_asteroids_mass_shrinks_mars_error() {
    // JPL's planetary ephemeris includes the pull of the heaviest
    // asteroids, and they tug hardest on Mars, the planet closest to the
    // asteroid belt. Adding them (with DE440's GMs) must bring Mars closer
    // to JPL after a year.
    let without = planet_errors(false);
    let with = planet_errors(true);
    println!(
        "{:<8} {:>21} {:>18}",
        "body", "without asteroids (km)", "with asteroids (km)"
    );
    for ((name, a), (_, b)) in without.iter().zip(&with) {
        println!("{name:<8} {a:>21.3} {b:>18.3}");
    }
    let mars = |errors: &[(String, f64)]| errors.iter().find(|e| e.0 == "Mars").unwrap().1;
    assert!(
        mars(&with) < mars(&without),
        "Mars: {} km with, {} km without",
        mars(&with),
        mars(&without)
    );
}

/// Every small body's position in a hierarchy, by name.
fn positions(h: &Hierarchy) -> Vec<(String, DVec3)> {
    let mut all: Vec<(String, DVec3)> = h
        .top
        .bodies
        .iter()
        .map(|b| (b.name.clone(), b.position))
        .collect();
    all.extend(
        (0..h.follower_count()).map(|i| (h.follower(i).name.clone(), h.follower(i).position)),
    );
    all
}

#[test]
fn small_bodies_match_jpl_after_one_year() {
    // The same standard as the planets in step 1.3: within 1 part in
    // 10,000 of its distance from the Sun after a year.
    let mut h = full_solar_system();
    h.advance(365.25 * DAY);
    let reference = small_bodies_one_year_on();
    let sun = h.top.bodies[0].position;
    let mut failures = Vec::new();
    println!(
        "{:<22} {:>12} {:>12} {:>12}",
        "body", "error (km)", "Sun (AU)", "fraction"
    );
    for (name, position) in positions(&h) {
        let Some((expected, _)) = reference.get(&name) else {
            continue;
        };
        let error = (position - *expected).length();
        let distance = (*expected - sun).length();
        println!(
            "{name:<22} {:>12.2} {:>12.3} {:>12.1e}",
            error / 1e3,
            distance / AU,
            error / distance
        );
        if error >= 1e-4 * distance {
            failures.push(name);
        }
    }
    assert!(
        failures.is_empty(),
        "farther than 1 part in 10,000: {failures:?}"
    );
}

#[test]
#[ignore = "slow: 37 simulated years; run with cargo test --release -- --ignored"]
fn halleys_comet_returns_in_july_2061() {
    // Halley's Comet was last at perihelion in February 1986 and comes back
    // every 75 or 76 years. From its January 2025 state, near aphelion, the
    // simulation must bring it back to the Sun in July 2061.
    let mut h = full_solar_system();
    // The moons don't matter to Halley; leave them out to save time.
    h.moon_systems.clear();
    let halley = (0..h.follower_count())
        .find(|&i| h.follower(i).name == "Halley")
        .unwrap();
    let start_jd = solar_system().epoch_jd_tdb;
    let distance =
        |h: &Hierarchy| (h.follower(halley).position - h.top.bodies[0].position).length();
    // Run to mid-2061, then track the distance hourly through the summer.
    h.advance((2_473_985.5 - start_jd) * DAY);
    let (mut closest, mut when) = (f64::MAX, 0.0);
    for _ in 0..(120 * 24) {
        h.advance(3600.0);
        let d = distance(&h);
        if d < closest {
            (closest, when) = (d, start_jd + h.time() / DAY);
        }
    }
    let jpl = jpl_halley_perihelion_jd();
    println!(
        "perihelion: JD {when:.3} (JPL predicts JD {jpl:.3}, {:+.2} days), {:.4} AU from the Sun",
        when - jpl,
        closest / AU
    );
    // July 2061, from 1 July 0h to 1 August 0h TDB.
    let (july_1, august_1) = (2_474_006.5, 2_474_037.5);
    assert!(
        (july_1..august_1).contains(&when),
        "perihelion at JD {when}, not in July 2061"
    );
}
