//! Validation: magnetopauses from pressure balance, against spacecraft
//! measurements.

use worldline_core::constants::AU;
use worldline_core::magnetosphere::{shue_1998, standoff};
use worldline_data::{planetary_fields, solar_system, solar_wind_2025};

/// 2025's average flow pressure of the solar wind at Earth, in Pa.
fn mean_flow_pressure() -> f64 {
    let pressures: Vec<f64> = solar_wind_2025()
        .iter()
        .filter_map(|h| h.flow_pressure)
        .collect();
    pressures.iter().sum::<f64>() / pressures.len() as f64
}

#[test]
fn earths_magnetopause_sits_about_10_earth_radii_sunward() {
    let earth = planetary_fields()
        .into_iter()
        .find(|f| f.planet == "Earth")
        .unwrap()
        .dipole;
    // The roadmap's figure, from 2025's average wind: "about 10", to the
    // nearest Earth radius.
    let mean = mean_flow_pressure();
    let at_mean = standoff(&earth, mean) / earth.radius;
    println!(
        "2025 average flow pressure {:.2} nPa: magnetopause {at_mean:.2} Earth radii sunward",
        mean * 1e9
    );
    assert!((at_mean - 10.0).abs() < 0.5);

    // Hour by hour, against Shue et al.'s fit to spacecraft crossings of
    // the magnetopause, given the same measured wind. Their model scatters
    // about the real crossings by 1.23 Earth radii (standard deviation);
    // the physics must agree with it at least that well.
    //
    // Shue's model takes the field's Bz in GSM coordinates; these data are
    // GSE, a rotation about the Sun–Earth line of up to ~35° apart. At
    // Shue's slope near Bz = 0 (0.04 Earth radii per nT), that shifts its
    // standoff by tenths of an Earth radius at most.
    let mut differences = Vec::new();
    for hour in solar_wind_2025() {
        let (Some(p), Some(b)) = (hour.flow_pressure, hour.field) else {
            continue;
        };
        let physics = standoff(&earth, p) / earth.radius;
        let (measured, _) = shue_1998(p * 1e9, b.z * 1e9);
        differences.push(physics - measured);
    }
    let n = differences.len() as f64;
    let rms = (differences.iter().map(|d| d * d).sum::<f64>() / n).sqrt();
    let mean_difference = differences.iter().sum::<f64>() / n;
    let within_1 = differences.iter().filter(|d| d.abs() <= 1.0).count() as f64 / n;
    println!(
        "{} hours: physics − Shue {mean_difference:+.2} on average, RMS {rms:.2} Earth radii; within ±1: {:.0}%",
        differences.len(),
        within_1 * 100.0
    );
    assert!(rms <= 1.23);
}

#[test]
fn the_planets_magnetopauses_compared_with_spacecraft() {
    // Pressure balance with each planet's dipole, in 2025's average wind
    // thinned out as 1/r². Observed: Mercury, MESSENGER (Winslow et al.
    // 2013); Jupiter, all flybys and orbiters (Joy et al. 2002, two common
    // states); Saturn, Cassini (Achilleos et al. 2008, two common states);
    // Uranus and Neptune, Voyager 2's single crossings. Printed, not
    // checked: Jupiter's and Saturn's magnetospheres are inflated by their
    // own plasma (from Io and Enceladus), which pure pressure balance with
    // a dipole leaves out.
    let observed = [
        ("Mercury", "1.45"),
        ("Earth", "~10–11"),
        ("Jupiter", "63 or 92"),
        ("Saturn", "22 or 27"),
        ("Uranus", "18.3"),
        ("Neptune", "26.5"),
    ];
    let snapshot = solar_system();
    let sun = snapshot.bodies[0].position;
    let mean = mean_flow_pressure();
    println!(
        "{:<8} {:>12} {:>8} {:>14} {:>12}",
        "planet", "field (nT)", "tilt", "model (radii)", "observed"
    );
    for field in planetary_fields() {
        let planet = snapshot
            .bodies
            .iter()
            .find(|b| b.name == field.planet)
            .unwrap();
        let r = (planet.position - sun).length();
        let pressure = mean * (AU / r).powi(2);
        let distance = standoff(&field.dipole, pressure) / field.dipole.radius;
        let seen = observed.iter().find(|o| o.0 == field.planet).unwrap().1;
        println!(
            "{:<8} {:>12.0} {:>7.1}° {:>14.1} {:>12}",
            field.planet,
            field.dipole.equatorial_field() * 1e9,
            field.dipole.tilt().to_degrees(),
            distance,
            seen
        );
    }
}
