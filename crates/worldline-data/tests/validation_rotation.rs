//! Validation: rotation models against known facts about the solar system.
//!
//! The IAU models give each body's pole and spin. Combined with the
//! simulated orbits, they must reproduce:
//! - the axial tilts NASA publishes,
//! - Mercury's 3:2 spin–orbit resonance,
//! - the Moon keeping one face toward Earth, with its slight rocking
//!   (libration).

use worldline_core::constants::{DAY, GM_SUN};
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::integrator::{Ias15, advance};
use worldline_core::orbit::kepler_period;
use worldline_core::{Body, DVec3};
use worldline_data::{rotation_model, solar_system};

/// The angle between a body's spin and its orbit's angular momentum.
fn obliquity(body: &Body, center: &Body, jd: f64) -> f64 {
    let r = body.position - center.position;
    let v = body.velocity - center.velocity;
    let orbit_normal = r.cross(v).normalize();
    let spin = rotation_model(&body.name).unwrap().spin_axis(jd);
    spin.angle_between(orbit_normal).to_degrees()
}

#[test]
fn axial_tilts_match_nasa() {
    // "Obliquity to orbit" from the NASA Planetary Fact Sheet, which gives
    // one decimal place (more for Mercury). Above 90° means retrograde spin.
    let published = [
        ("Mercury", 0.034),
        ("Venus", 177.4),
        ("Earth", 23.4),
        ("Mars", 25.2),
        ("Jupiter", 3.1),
        ("Saturn", 26.7),
        ("Uranus", 97.8),
        ("Neptune", 28.3),
    ];
    let snapshot = solar_system();
    let jd = snapshot.epoch_jd_tdb;
    let sun = snapshot.body("Sun").unwrap();
    for (name, expected) in published {
        let tilt = obliquity(snapshot.body(name).unwrap(), sun, jd);
        println!("{name:<8} tilt {tilt:7.3}°   NASA {expected}°");
        assert!(
            (tilt - expected).abs() < 0.1,
            "{name}: {tilt}° vs {expected}°"
        );
    }
}

#[test]
fn moon_follows_cassinis_laws() {
    // The Moon's equator is tilted 1.54° to the ecliptic and its orbit 5.14°
    // on average, on opposite sides of the ecliptic pole (Cassini's laws).
    // Both axes precess together every 18.6 years. The Sun makes the
    // orbit's inclination swing between 4.99° and 5.30° every 173 days, so
    // the tilt between spin and orbit is 6.53°–6.84° at any moment.
    let snapshot = solar_system();
    let jd = snapshot.epoch_jd_tdb;
    let (moon, earth) = (
        snapshot.body("Moon").unwrap(),
        snapshot.body("Earth").unwrap(),
    );
    let tilt = obliquity(moon, earth, jd);

    let spin = rotation_model("Moon").unwrap().spin_axis(jd);
    let orbit = (moon.position - earth.position)
        .cross(moon.velocity - earth.velocity)
        .normalize();
    let ecliptic_direction = |v: DVec3| DVec3::new(v.x, v.y, 0.0).normalize();
    let apart = ecliptic_direction(spin)
        .angle_between(ecliptic_direction(orbit))
        .to_degrees();
    println!(
        "Moon: spin tilted {:.3}° from the ecliptic pole, orbit {:.3}°, {tilt:.3}° from each other; \
         their directions are {apart:.1}° apart around the pole",
        spin.angle_between(DVec3::Z).to_degrees(),
        orbit.angle_between(DVec3::Z).to_degrees(),
    );
    assert!((6.5..6.9).contains(&tilt), "tilt {tilt}°");
    // Opposite sides of the pole. The orbit's node wobbles by a few degrees
    // over a month, so allow 5°.
    assert!((apart - 180.0).abs() < 5.0, "spin and orbit {apart}° apart");
}

#[test]
fn mercury_spins_three_times_every_two_orbits() {
    let snapshot = solar_system();
    let (mercury, sun) = (
        snapshot.body("Mercury").unwrap(),
        snapshot.body("Sun").unwrap(),
    );
    // Semi-major axis from the vis-viva equation, then Kepler's third law.
    let mu = GM_SUN + mercury.gm;
    let r = (mercury.position - sun.position).length();
    let v = (mercury.velocity - sun.velocity).length();
    let a = 1.0 / (2.0 / r - v * v / mu);
    let orbit = kepler_period(a, mu);
    let spin = rotation_model("Mercury").unwrap().sidereal_period();
    let ratio = orbit / spin;
    println!(
        "Mercury: orbit {:.3} days, spin {:.3} days, ratio {ratio:.5}",
        orbit / DAY,
        spin / DAY
    );
    assert!((ratio - 1.5).abs() < 1e-3);
}

#[test]
fn earth_turns_once_per_sidereal_day() {
    let period = rotation_model("Earth").unwrap().sidereal_period();
    println!("Earth's sidereal day: {period:.2} s");
    // 23 h 56 min 4.1 s.
    assert!((period - 86_164.1).abs() < 0.1);
}

#[test]
fn moon_keeps_one_face_toward_earth() {
    // Simulate a month and track where Earth sits in the Moon's sky,
    // in the Moon's own body-fixed longitude and latitude.
    let snapshot = solar_system();
    let model = rotation_model("Moon").unwrap();
    let mut system = snapshot.system();
    let (earth, moon) = (3, 4);
    let mut ias = Ias15::new();
    let (mut lon_range, mut lat_range) = ((f64::MAX, f64::MIN), (f64::MAX, f64::MIN));
    for _ in 0..=(28 * 4) {
        let jd = snapshot.epoch_jd_tdb + system.time() / DAY;
        let to_earth: DVec3 = system.bodies[earth].position - system.bodies[moon].position;
        let local = model.orientation(jd).body_to_ecliptic.transpose() * to_earth.normalize();
        let lon = local.y.atan2(local.x).to_degrees();
        let lat = local.z.asin().to_degrees();
        lon_range = (lon_range.0.min(lon), lon_range.1.max(lon));
        lat_range = (lat_range.0.min(lat), lat_range.1.max(lat));
        advance(&mut system, &EinsteinInfeldHoffmann, &mut ias, DAY / 4.0);
    }
    println!(
        "Earth in the Moon's sky over a month: longitude {:+.2}° to {:+.2}°, latitude {:+.2}° to {:+.2}°",
        lon_range.0, lon_range.1, lat_range.0, lat_range.1
    );
    // Earth stays near the center of the Moon's near side (0°, 0°). Libration
    // rocks it by at most about 7.9° in longitude and 6.7° in latitude.
    assert!(lon_range.0 > -8.5 && lon_range.1 < 8.5);
    assert!(lat_range.0 > -7.5 && lat_range.1 < 7.5);
    // And the rocking is really there: the Moon doesn't simply stare.
    assert!(lon_range.1 - lon_range.0 > 2.0);
}

#[test]
fn sun_is_overhead_in_the_right_place_on_new_years_day() {
    // Where on Earth is the Sun straight overhead at 2025-01-01 00:00 TDB?
    // Combines Earth's IAU rotation with the simulated Sun–Earth geometry,
    // and fixes where day and night fall on Earth's globe.
    //
    // Expected: latitude = the Sun's declination on January 1, about −23.0°
    // (southern summer). Longitude: midnight at Greenwich puts the noon Sun
    // over the date line, shifted by the equation of time (−3.1 minutes →
    // −0.8°) and by TDB running 69 s ahead of UTC (+0.3°): about −179.0°.
    let snapshot = solar_system();
    let jd = snapshot.epoch_jd_tdb;
    let (earth, sun) = (
        snapshot.body("Earth").unwrap(),
        snapshot.body("Sun").unwrap(),
    );
    let toward_sun = (sun.position - earth.position).normalize();
    let model = rotation_model("Earth").unwrap();
    let local = model.orientation(jd).body_to_ecliptic.transpose() * toward_sun;
    let latitude = local.z.asin().to_degrees();
    let longitude = local.y.atan2(local.x).to_degrees();
    println!("Sun overhead at latitude {latitude:.2}°, longitude {longitude:.2}°");
    assert!((latitude - -23.0).abs() < 0.2, "latitude {latitude}°");
    // The IAU Earth model is a simple one (see docs/physics/rotation.md),
    // good to a few tenths of a degree.
    assert!((longitude - -179.0).abs() < 0.5, "longitude {longitude}°");
}
