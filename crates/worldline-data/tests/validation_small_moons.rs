//! Validation: every known moon loads, and the small moons match JPL.

use worldline_core::DVec3;
use worldline_core::constants::DAY;
use worldline_core::hierarchy::Hierarchy;
use worldline_data::{
    SmallMoon, moon_elements, satellites_in_jpls_list, small_moon_states_30_days_on,
    small_moon_zonal_harmonics, small_moons, solar_system_with_moons,
};

/// Every planet's small moons switched on, starting from JPL's states.
/// Returns the hierarchy and, per moon system, the moons in the order given.
fn hierarchy_with_small_moons(moons: &[SmallMoon]) -> (Hierarchy, Vec<Vec<&SmallMoon>>) {
    let mut h = solar_system_with_moons();
    let mut given = Vec::new();
    for system in &mut h.moon_systems {
        let planet = system.system.bodies[0].name.clone();
        let mine: Vec<&SmallMoon> = moons
            .iter()
            .filter(|m| m.parent == planet && m.state.is_some())
            .collect();
        system.set_small_moons(
            mine.iter()
                .map(|m| {
                    let (r, v) = m.state.unwrap();
                    let body = worldline_core::Body::new(
                        &m.name,
                        m.gm.unwrap_or(0.0),
                        m.radius.unwrap_or(0.0),
                    )
                    .at(r)
                    .moving(v);
                    (body, m.regular)
                })
                .collect(),
        );
        given.push(mine);
    }
    (h, given)
}

#[test]
fn every_moon_in_jpls_list_loads() {
    // JPL's list of planetary satellites, as the data file records it,
    // must all be there: the Moon, the 21 major moons and the rest.
    let listed = satellites_in_jpls_list();
    let elements = moon_elements();
    let small = small_moons();
    let major = worldline_data::moon_systems()
        .iter()
        .map(|s| s.bodies.len() - 1)
        .sum::<usize>();
    println!(
        "JPL's list: {listed} moons. Loaded: {} with mean orbits = 1 Moon + {major} major + {} small",
        elements.len(),
        small.len()
    );
    assert_eq!(elements.len(), listed);
    assert_eq!(1 + major + small.len(), listed);
    let missing: Vec<&str> = small
        .iter()
        .filter(|m| m.state.is_none())
        .map(|m| m.name.as_str())
        .collect();
    println!("without a JPL state for 2025 (placed by mean elements only): {missing:?}");
    for moon in &small {
        let (r, v) = moon.orbit.state_at(0.0);
        assert!(
            r.is_finite() && v.is_finite() && r.length() > 0.0,
            "{}",
            moon.name
        );
    }
}

#[test]
fn small_moons_match_jpl_after_30_days() {
    // Spot checks: every small moon with a JPL state is simulated for 30
    // days, following the major moons. Those with a measured size must be
    // closer to JPL's position than their own radius, the same test the
    // major moons pass; the rest are printed.
    let moons = small_moons();
    let (mut h, given) = hierarchy_with_small_moons(&moons);
    h.advance(30.0 * DAY);
    let reference = small_moon_states_30_days_on();

    let mut failures = Vec::new();
    let mut checked = 0;
    let mut worst: Vec<(f64, String)> = Vec::new();
    println!(
        "{:<14} {:>12} {:>11} {:>14}",
        "moon", "error (km)", "radius (km)", "orbit (km)"
    );
    for (index, system) in h.moon_systems.iter().enumerate() {
        for (i, moon) in given[index].iter().enumerate() {
            let (planet, _) = h.absolute(index, 0);
            let (position, _) = h.absolute_small(index, i);
            let expected = reference[&moon.naif_id].0;
            let error = (position - planet - expected).length();
            worst.push((error / expected.length(), moon.name.clone()));
            if let Some(radius) = moon.radius {
                checked += 1;
                println!(
                    "{:<14} {:>12.2} {:>11.1} {:>14.0}",
                    moon.name,
                    error / 1e3,
                    radius / 1e3,
                    expected.length() / 1e3
                );
                if error >= radius {
                    failures.push(moon.name.clone());
                }
            }
        }
        let _ = system;
    }
    worst.sort_by(|a, b| b.0.total_cmp(&a.0));
    println!("\nlargest errors as a fraction of distance from the planet, any moon:");
    for (fraction, name) in worst.iter().take(8) {
        println!("  {name:<14} {fraction:.2e}");
    }
    println!("spot-checked {checked} moons with measured sizes");
    // Differences this step has not explained, documented in
    // docs/physics/small-moons.md. Listed so that every other moon is held
    // to the criterion, and so the list can't silently go stale.
    let known: [&str; 7] = [
        "Epimetheus",
        "Aegaeon",
        "Methone",
        "Anthe",
        "Pallene",
        "Styx",
        "Kerberos",
    ];
    println!("known residuals, not yet explained: {failures:?}");
    let unexpected: Vec<&String> = failures
        .iter()
        .filter(|f| !known.contains(&f.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "farther from JPL than their own radius: {unexpected:?}"
    );
    let resolved: Vec<&&str> = known
        .iter()
        .filter(|k| !failures.iter().any(|f| f == *k))
        .collect();
    assert!(
        resolved.is_empty(),
        "now within their radius, remove from the known list: {resolved:?}"
    );
}

#[test]
fn mean_elements_place_moons_where_jpl_does() {
    // JPL's mean elements are an average orbit, so they place a moon near,
    // not exactly at, JPL's position. Every moon's difference is printed.
    //
    // The conventions (frames, node origin, angles, units) are checked on
    // the moons whose elements JPL dates at the snapshot itself, Uranus's
    // inner moons, where nothing has to be propagated:
    // - the orbital plane must match JPL's to within the precision of the
    //   published digits: inclination and the Laplace pole's right
    //   ascension and declination are given to 0.1°, so each may be off by
    //   0.05°, at most 0.05° + 0.05° √2 ≈ 0.12° together;
    // - the distance from the planet must match to within the short wobble
    //   the planet's oblateness causes, about 3 J2 (R/a)² a, with J2 as
    //   their own JPL solution (URA184) fits it.
    let uranus = small_moon_zonal_harmonics("Uranus").unwrap();
    let (radius_uranus, j2) = (uranus.radius, uranus.coefficients[0].1);
    let plane_bound = 0.05 + 0.05 * 2f64.sqrt();
    let mut fractions: Vec<f64> = Vec::new();
    println!(
        "{:<12} {:>9} {:>11} {:>13} {:>11}",
        "moon", "plane (°)", "radius (km)", "bound (km)", "phase (°)"
    );
    for moon in small_moons() {
        let Some((expected, velocity)) = moon.state else {
            continue;
        };
        let element = moon_elements()
            .iter()
            .find(|e| e.naif_id == moon.naif_id)
            .unwrap();
        let (position, orbit_velocity) = element.mean_elements(2_460_676.5).state_at(0.0);
        if element.epoch_jd_tdb == 2_460_676.5 && moon.parent == "Uranus" {
            let tilt = position
                .cross(orbit_velocity)
                .angle_between(expected.cross(velocity))
                .to_degrees();
            let a = moon.orbit.a;
            let radius_gap = (position.length() - expected.length()).abs();
            let wobble = 3.0 * j2 * (radius_uranus / a).powi(2) * a;
            let phase = position.angle_between(expected).to_degrees();
            println!(
                "{:<12} {:>9.3} {:>11.1} {:>13.1} {:>11.2}",
                moon.name,
                tilt,
                radius_gap / 1e3,
                wobble / 1e3,
                phase
            );
            assert!(
                tilt < plane_bound,
                "{}: orbital plane off by {tilt}°",
                moon.name
            );
            assert!(
                radius_gap < wobble,
                "{}: distance off by {} km",
                moon.name,
                radius_gap / 1e3
            );
        }
    }
    // Starting from JPL's 2025 state and turning at JPL's mean rates, how
    // far does a moon drift from JPL in a month? (The approximation an
    // out-of-focus moon lives with.)
    let reference = small_moon_states_30_days_on();
    for moon in small_moons() {
        if let Some((expected, _)) = reference.get(&moon.naif_id) {
            let (position, _) = moon.orbit.state_at(30.0 * DAY);
            fractions.push((position - *expected).length() / expected.length());
        }
    }
    fractions.sort_by(f64::total_cmp);
    let at = |q: f64| fractions[((fractions.len() - 1) as f64 * q) as usize];
    println!(
        "a month on, approximate orbits differ from JPL by (fraction of distance): median {:.1e}, 90th percentile {:.1e}, worst {:.1e}",
        at(0.5),
        at(0.9),
        at(1.0)
    );
}

/// Semi-major axis of a planetocentric state, from the vis-viva equation.
fn semi_major_axis(r: DVec3, v: DVec3, mu: f64) -> f64 {
    1.0 / (2.0 / r.length() - v.length_squared() / mu)
}

#[test]
#[ignore = "slow: 2 simulated years of Saturn's small moons; run with cargo test --release -- --ignored"]
fn janus_and_epimetheus_trade_orbits() {
    // Janus and Epimetheus share an orbit about 50 km wide. Every four
    // years the inner one catches up with the outer, and their mutual pull
    // makes them swap: the inner moves out and the outer moves in. Only
    // their own masses do this, so it tests that small moons pull on each
    // other. The closest approaches, when they swap, were observed in
    // January 2006, 2010, 2014, 2018 and 2022, so one is due in January
    // 2026.
    let moons: Vec<SmallMoon> = small_moons()
        .into_iter()
        .filter(|m| m.parent == "Saturn")
        .collect();
    let (mut h, given) = hierarchy_with_small_moons(&moons);
    let saturn = h
        .moon_systems
        .iter()
        .position(|m| m.system.bodies[0].name == "Saturn")
        .unwrap();
    let find = |name: &str| given[saturn].iter().position(|m| m.name == name).unwrap();
    let (janus, epimetheus) = (find("Janus"), find("Epimetheus"));
    let mu = h.moon_systems[saturn].system.bodies[0].gm;
    let gap = |h: &Hierarchy| {
        let (p, pv) = h.absolute(saturn, 0);
        let (j, jv) = h.absolute_small(saturn, janus);
        let (e, ev) = h.absolute_small(saturn, epimetheus);
        semi_major_axis(j - p, jv - pv, mu) - semi_major_axis(e - p, ev - pv, mu)
    };
    let start = gap(&h);
    let mut swap_day = None;
    for day in 1..=730 {
        h.advance(DAY);
        if swap_day.is_none() && gap(&h).signum() != start.signum() {
            swap_day = Some(day);
        }
    }
    println!(
        "Janus's orbit minus Epimetheus's: {:+.1} km at the start, {:+.1} km two years on",
        start / 1e3,
        gap(&h) / 1e3
    );
    let day = swap_day.expect("they never swapped");
    println!("they swapped {day} days after 2025-01-01 (January 2026 is days 365 to 395)");
    assert!(
        (365..=395).contains(&day),
        "swapped on day {day}, not in January 2026"
    );
    assert!(start.signum() != gap(&h).signum());
}
