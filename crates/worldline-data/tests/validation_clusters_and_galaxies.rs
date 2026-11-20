//! Step 7.3's check: every cluster, the Orion Nebula and every satellite
//! galaxy at its published distance and place.
//!
//! Run with `cargo test -p worldline-data --test
//! validation_clusters_and_galaxies -- --nocapture` to see the numbers.

use std::f64::consts::TAU;

use worldline_core::constants::{JULIAN_YEAR, PARSEC};
use worldline_core::{DMat3, DVec3};
use worldline_data::{
    DeepSkyKind, DeepSkyObject, deep_sky_objects, equatorial_to_ecliptic, equatorial_to_galactic,
    galactic_frame, sky_axes,
};

/// A milliarcsecond, in radians.
const MAS: f64 = TAU / (360.0 * 3600.0 * 1000.0);

/// J2000.0 as a Julian Date, and the Julian Date of a Julian year.
fn jd(year: f64) -> f64 {
    2_451_545.0 + (year - 2000.0) * 365.25
}

/// What an observer at the solar system's barycenter would measure of a
/// place and velocity there, in equatorial axes: right ascension and
/// declination (rad), distance (m), proper motion (mas/yr) and radial
/// velocity (m/s).
fn observe(x: DVec3, v: DVec3) -> (f64, f64, f64, (f64, f64), f64) {
    let distance = x.length();
    let ra = x.y.atan2(x.x).rem_euclid(TAU);
    let dec = (x.z / distance).asin();
    let (north, east, away) = sky_axes(ra, dec);
    let per = JULIAN_YEAR / (MAS * distance);
    (
        ra,
        dec,
        distance,
        (v.dot(east) * per, v.dot(north) * per),
        v.dot(away),
    )
}

/// Its distance in the unit its catalogue uses.
fn in_published_unit(o: &DeepSkyObject, meters: f64) -> f64 {
    let pc = meters / PARSEC;
    match o.distance_published.rsplit(' ').next() {
        Some("kpc") => pc / 1e3,
        Some("pc") => pc,
        Some("mag") => 5.0 * (pc / 10.0).log10(),
        unit => panic!("unknown unit {unit:?}"),
    }
}

/// Half a unit in the last place of a number as printed.
fn half_last_digit(printed: &str) -> f64 {
    let mantissa = printed.split(['e', 'E']).next().unwrap_or(printed);
    let exponent: i32 = printed
        .split(['e', 'E'])
        .nth(1)
        .map_or(0, |e| e.parse().expect("an exponent"));
    let decimals = mantissa.split('.').nth(1).map_or(0, str::len) as i32;
    0.5 * 10f64.powi(exponent - decimals)
}

/// The roadmap's check. Each object is placed as the app places it:
/// carried from its catalogue's direction, distance and motion into the
/// galaxy's frame, around the galaxy's center. Turned back into what an
/// observer at the solar system's barycenter would measure at its epoch,
/// every number must come back as published.
///
/// The only error allowed is double precision's rounding: each step
/// rounds to about 10⁻¹⁶ of the largest number in it, the galaxy-centered
/// place (kiloparsecs) or velocity (hundreds of km/s). The limit allows 10⁴
/// such roundings, 10⁻¹² of those.
#[test]
fn each_is_at_its_published_distance_and_place() {
    let frame = galactic_frame();
    let back = frame.to_galactic.transpose();
    let to_equatorial = equatorial_to_ecliptic().transpose();
    let objects = deep_sky_objects();
    let (mut worst_distance, mut worst_angle, mut worst_pm, mut worst_rv) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    let mut worst_name = String::new();
    for o in &objects {
        let (x, v) = o.galactocentric(
            jd(o.epoch),
            frame.to_galactic,
            (frame.position, frame.velocity),
        );
        // Back out of the galaxy's frame, to the barycenter's equatorial
        // axes.
        let seen = to_equatorial * (back * (x - frame.position));
        let moving = to_equatorial * (back * (v - frame.velocity));
        let (ra, dec, distance, (pmra, pmdec), rv) = observe(seen, moving);
        let scale = 1e-12 * (x.length() + frame.position.length());
        let speed_scale = 1e-12 * (v.length() + frame.velocity.length());
        // Distance, in its published unit: the change in it that rounding
        // can make.
        let published = in_published_unit(o, o.distance);
        let got = in_published_unit(o, distance);
        let allowed = (in_published_unit(o, o.distance + scale) - published).abs();
        let distance_off = (got - published).abs() / allowed;
        let angle_off =
            (DVec3::new(ra.cos() * dec.cos(), ra.sin() * dec.cos(), dec.sin()) - sky(o)).length()
                / (scale / o.distance);
        let pm_allowed = speed_scale * JULIAN_YEAR / (MAS * o.distance);
        let pm_off = (pmra - o.proper_motion.0)
            .abs()
            .max((pmdec - o.proper_motion.1).abs())
            / pm_allowed;
        let rv_off = (rv - o.radial_velocity).abs() / speed_scale;
        for off in [distance_off, angle_off, pm_off, rv_off] {
            assert!(
                off <= 1.0,
                "{}: {off:.2} of what rounding allows ({distance_off:.2e} distance, \
                 {angle_off:.2e} place, {pm_off:.2e} proper motion, {rv_off:.2e} radial velocity)",
                o.name
            );
        }
        if distance_off.max(angle_off).max(pm_off).max(rv_off)
            > worst_distance.max(worst_angle).max(worst_pm).max(worst_rv)
        {
            worst_name = o.name.clone();
        }
        worst_distance = worst_distance.max(distance_off);
        worst_angle = worst_angle.max(angle_off);
        worst_pm = worst_pm.max(pm_off);
        worst_rv = worst_rv.max(rv_off);
    }
    println!(
        "{} objects placed in the galaxy and measured back from the barycenter at their epochs:",
        objects.len()
    );
    println!(
        "worst, as a fraction of what rounding allows (10⁻¹² of the galaxy-centered place or \
         velocity): distance {worst_distance:.3}, place {worst_angle:.3}, proper motion \
         {worst_pm:.3}, radial velocity {worst_rv:.3} ({worst_name})"
    );
    worldline_report::margins(
        "deep-sky-placed",
        "Clusters, nebulae and satellite galaxies placed in the galaxy and measured back as published",
    )
    .check(
        "distances",
        worst_distance,
        1.0,
        format!("the worst {worst_distance:.3} of what rounding allows (10⁻¹² of the galaxy-centered place)"),
    )
    .check("places on the sky", worst_angle, 1.0, format!("the worst {worst_angle:.3}"))
    .check("proper motions", worst_pm, 1.0, format!("the worst {worst_pm:.3}"))
    .check("radial velocities", worst_rv, 1.0, format!("the worst {worst_rv:.3}"))
    .save(worldline_report::here!());
    println!();
    println!("Some of them, as placed and as published:");
    for name in [
        "Hyades",
        "Pleiades",
        "Orion Nebula",
        "M4",
        "47 Tucanae",
        "Omega Centauri",
        "M13",
        "Large Magellanic Cloud",
        "Small Magellanic Cloud",
    ] {
        let o = objects.iter().find(|o| o.name == name).expect(name);
        let (x, _) = o.galactocentric(
            jd(o.epoch),
            frame.to_galactic,
            (frame.position, frame.velocity),
        );
        let seen = to_equatorial * (back * (x - frame.position));
        let (ra, dec, distance, _, _) = observe(seen, DVec3::ZERO);
        println!(
            "  {name:<24} {:>13.8}° {:>13.8}°  {:>12.6} kpc   published: {} {}, {}",
            ra.to_degrees(),
            dec.to_degrees(),
            distance / (1e3 * PARSEC),
            o.printed.ra,
            o.printed.dec,
            o.distance_published,
        );
    }
}

/// Its published direction, a unit vector in equatorial axes.
fn sky(o: &DeepSkyObject) -> DVec3 {
    sky_axes(o.ra, o.dec).2
}

/// Each object's galactic longitude and latitude, from its right ascension
/// and declination by the Hipparcos catalogue's rotation (the one the
/// galaxy's frame is built on), against those its catalogue prints, worked
/// out independently.
///
/// The limit is half a unit in the last digit printed, in each, plus what
/// two correct conversions can differ by. The galactic pole and the zero of
/// longitude are defined by three angles given to 10⁻⁵ degrees (ESA 1997,
/// vol. 1, §1.5.3), half a unit of which is 0.018″; and some conversions
/// apply them in the axes of the J2000 equator and equinox rather than the
/// ICRS's (astropy's, through FK5), which the IAU 2000 frame bias turns by
/// √(16.617² + 6.819² + 14.6²) = 23.2 mas (IERS Conventions 2010, ch. 5).
/// Together, under 0.08″: the limit allows 0.1″.
#[test]
fn their_galactic_coordinates_are_the_catalogues() {
    let turn: DMat3 = equatorial_to_galactic();
    let frames = 0.1 / 3600.0;
    let mut checked = 0;
    let mut worst: Vec<(DeepSkyKind, f64, f64, String)> = Vec::new();
    for o in deep_sky_objects() {
        let Some((l, b)) = o.galactic else {
            continue;
        };
        let g = turn * sky(&o);
        let our_l = g.y.atan2(g.x).to_degrees().rem_euclid(360.0);
        let our_b = g.z.asin().to_degrees();
        let off_l = ((our_l - l + 180.0).rem_euclid(360.0) - 180.0).abs() * b.to_radians().cos();
        let off_b = (our_b - b).abs();
        let allowed_l = half_last_digit(&o.printed.l) * b.to_radians().cos() + frames;
        let allowed_b = half_last_digit(&o.printed.b) + frames;
        let fraction = (off_l / allowed_l).max(off_b / allowed_b);
        assert!(
            fraction <= 1.0,
            "{}: l {our_l:.7}° against {l}°, b {our_b:.7}° against {b}°",
            o.name
        );
        checked += 1;
        match worst.iter_mut().find(|w| w.0 == o.kind) {
            Some(w) if w.1 >= fraction => {}
            Some(w) => *w = (o.kind, fraction, off_l.max(off_b) * 3600.0, o.name.clone()),
            None => worst.push((o.kind, fraction, off_l.max(off_b) * 3600.0, o.name.clone())),
        }
    }
    println!("{checked} objects' galactic longitude and latitude against their catalogues':");
    let mut figure = worldline_report::margins(
        "deep-sky-galactic",
        "Clusters, nebulae and satellite galaxies: galactic coordinates as their catalogues print them",
    );
    for (kind, fraction, arcsec, name) in worst {
        println!(
            "  {}: worst {name}, off by {arcsec:.4}″, {fraction:.2} of what's allowed",
            kind.plural()
        );
        figure = figure.check(
            &format!("{}: the worst, {name}", kind.plural()),
            fraction,
            1.0,
            format!(
                "{arcsec:.4}″ off (limit: half the last digit printed, and 0.1″ for the frames)"
            ),
        );
    }
    figure.save(worldline_report::here!());
}
