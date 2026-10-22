//! Validation: the galactic center.
//!
//! Sagittarius A* must sit where it is on the sky, and the star S2 must go
//! around it as observed: every 16 years, its long axis turned 12′ per
//! orbit by relativity. The GRAVITY Collaboration measured that turning in
//! 2020 and, with three more stars, in 2022: f_SP = 0.997 ± 0.144 times
//! general relativity's prediction (A&A 657, L12).

use std::f64::consts::{PI, TAU};

use worldline_core::DMat3;
use worldline_core::DVec3;
use worldline_core::constants::{AU, C, JULIAN_YEAR, OBLIQUITY_J2000};
use worldline_data::{
    equatorial_to_galactic, galactic_center, s_stars, sgr_a_star_direction, year_to_time,
};

#[test]
fn sagittarius_a_star_is_where_it_is_on_the_sky() {
    // Its direction, turned back from the ecliptic to the equator and then
    // to galactic coordinates (the Hipparcos catalogue's rotation, ESA
    // 1997, vol. 1, §1.5.3), must match SIMBAD's l = 359.94423568°,
    // b = −0.04616002°, computed from an earlier radio position (Petrov et
    // al. 2011) that differs from ours by under 0.05″: so within 0.0001°.
    let equatorial = DMat3::from_rotation_x(OBLIQUITY_J2000) * sgr_a_star_direction();
    let galactic = equatorial_to_galactic() * equatorial;
    let l = galactic.y.atan2(galactic.x).to_degrees().rem_euclid(360.0);
    let b = galactic.z.asin().to_degrees();
    println!("Sagittarius A*: l = {l:.8}°, b = {b:.8}° (SIMBAD: 359.94423568°, −0.04616002°)");
    assert!((l - 359.944_235_68).abs() < 1e-4);
    assert!((b + 0.046_160_02).abs() < 1e-4);
}

#[test]
fn s2_circles_sagittarius_a_star_and_turns_as_gravity_measured() {
    let mut gc = galactic_center();
    let s2 = s_stars().into_iter().find(|s| s.name == "S2").unwrap();
    let gm = gc.hierarchy.top.bodies[0].gm;
    let (a, e) = (s2.a(), s2.e);
    let period = TAU * (a.powi(3) / gm).sqrt();
    let index = (0..gc.hierarchy.follower_count())
        .find(|&i| gc.hierarchy.follower(i).name == "S2")
        .unwrap();
    // The direction of the orbit's closest point (the Laplace–Runge–Lenz
    // vector), measured in the orbit's plane as it starts.
    let state = |gc: &worldline_data::GalacticCenter| {
        let b = gc.hierarchy.follower(index);
        (b.position, b.velocity)
    };
    let lrl = |(r, v): (DVec3, DVec3)| v.cross(r.cross(v)) - gm * r.normalize();
    let start = state(&gc);
    let p = lrl(start).normalize();
    let q = start.0.cross(start.1).normalize().cross(p);
    let angle = |s: (DVec3, DVec3)| {
        let a = lrl(s);
        a.dot(q).atan2(a.dot(p))
    };
    // Ten orbits, 400 samples an orbit.
    let orbits = 10.0;
    let dt = period / 400.0;
    let mut points = vec![(0.0, angle(start))];
    let (mut closest, mut fastest) = (f64::MAX, 0.0f64);
    let mut perihelia = Vec::new();
    let mut last = start;
    while gc.hierarchy.time() < orbits * period {
        gc.hierarchy.advance(dt);
        let s = state(&gc);
        let r = s.0.length();
        closest = closest.min(r);
        fastest = fastest.max(s.1.length());
        if last.0.dot(last.1) < 0.0 && s.0.dot(s.1) >= 0.0 {
            perihelia.push(gc.hierarchy.time());
        }
        last = s;
        points.push((gc.hierarchy.time(), angle(s)));
    }
    let n = points.len() as f64;
    let (tm, am) = (
        points.iter().map(|x| x.0).sum::<f64>() / n,
        points.iter().map(|x| x.1).sum::<f64>() / n,
    );
    let (num, den) = points.iter().fold((0.0, 0.0), |(num, den), (t, x)| {
        (num + (t - tm) * (x - am), den + (t - tm) * (t - tm))
    });
    let per_orbit = num / den * period;
    // General relativity's first-order prediction: 6πGM/(c²a(1 − e²)).
    let predicted = 6.0 * PI * gm / (C * C * a * (1.0 - e * e));
    let f = per_orbit / predicted;
    let arcmin = 180.0 / PI * 60.0;
    let year = |t: f64| 2025.0 + t / JULIAN_YEAR;
    println!(
        "S2: period {:.3} yr; next pericenters {}",
        period / JULIAN_YEAR,
        perihelia
            .iter()
            .take(2)
            .map(|t| format!("{:.2}", year(*t)))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "pericenter {:.1} AU at {:.0} km/s (GRAVITY: about 120 AU, about 7700 km/s)",
        closest / AU,
        fastest / 1e3
    );
    println!(
        "turning: {:.2}′ per orbit simulated, {:.2}′ predicted: f = {f:.4} (GRAVITY measured 0.997 ± 0.144)",
        per_orbit * arcmin,
        predicted * arcmin
    );
    // The next pericenter comes one Keplerian period after 2018.3789,
    // give or take what first-order relativity changes in the timing: at
    // most its strength at pericenter, GM/(r c²) = 3.5 × 10⁻⁴, of a period.
    let expected = year_to_time(s2.t_peri) + period;
    let allowed = gm / (C * C * a * (1.0 - e)) * period;
    println!(
        "next pericenter {:.2} days from one Keplerian period after 2018.3789 (allowed {:.2})",
        (perihelia[0] - expected) / 86_400.0,
        allowed / 86_400.0
    );
    assert!((perihelia[0] - expected).abs() < allowed);
    // Its turning as GRAVITY measured it, within their 1σ.
    assert!((f - 0.997).abs() < 0.144);
}

#[test]
fn the_solar_system_goes_around_the_galaxy() {
    use worldline_core::constants::PARSEC;
    use worldline_core::milky_way::{GalacticOrbit, KILOPARSEC, milky_way};
    use worldline_data::{galactic_frame, galactic_north_pole, sun_motion};

    // Where the measurements put the solar system: Sagittarius A* 8,277 pc
    // away in its radio direction, the Sun 20.8 pc above the midplane.
    let frame = galactic_frame();
    let turn = frame.to_galactic;
    let (x, v) = (frame.position, frame.velocity);
    println!(
        "the solar system, from Sagittarius A*: x {:.4} kpc, z {:.2} pc; moving {:.2} km/s toward it, {:.2} km/s the way the galaxy turns, {:.2} km/s north",
        x.x / KILOPARSEC,
        x.z / PARSEC,
        v.x / 1e3,
        v.y / 1e3,
        v.z / 1e3
    );
    assert!(
        ((turn * turn.transpose()) - DMat3::IDENTITY)
            .abs()
            .to_cols_array()
            .iter()
            .all(|e| *e < 1e-15)
    );
    assert!((x.length() / (8277.0 * PARSEC) - 1.0).abs() < 1e-14);
    assert!((x.z / PARSEC - 20.8).abs() < 1e-9 && x.y.abs() < 1e-6 * PARSEC);

    // Back the other way, Sagittarius A* must move on the sky exactly as
    // measured: −6.411 and −0.219 mas/yr, and 11.1 km/s toward us.
    let toward = sgr_a_star_direction();
    let pole = galactic_north_pole();
    let along = pole.cross(toward).normalize();
    let north = toward.cross(along);
    let seen = turn.transpose() * -v;
    let d = x.length();
    let mas_per_year = (1e-3 / 3600.0_f64).to_radians() / JULIAN_YEAR;
    let m = sun_motion();
    let (mu_l, mu_b) = (seen.dot(along) / d, seen.dot(north) / d);
    println!(
        "Sagittarius A* seen from the Sun: {:.4} mas/yr along the plane, {:.4} north, {:.3} km/s along the line of sight",
        mu_l / mas_per_year,
        mu_b / mas_per_year,
        seen.dot(toward) / 1e3
    );
    assert!((mu_l / m.mu_l - 1.0).abs() < 1e-12 && (mu_b / m.mu_b - 1.0).abs() < 1e-12);
    assert!((seen.dot(toward) + m.u).abs() < 1e-9);

    // Its orbit in McMillan's (2017) model, over two billion years.
    let potential = milky_way();
    let orbit = GalacticOrbit::new(potential, x, v);
    let r = x.truncate().length();
    let circular = potential.circular_speed(r);
    let myr = 1e6 * JULIAN_YEAR;
    let s = orbit.survey(2e3 * myr, 0.5 * myr);
    println!(
        "a circular orbit there goes {:.1} km/s; the Sun {:.1} km/s faster along it",
        circular / 1e3,
        (v.y - circular) / 1e3
    );
    println!(
        "its orbit: {:.2} to {:.2} kpc from the center, up to {:.0} pc from the midplane; around every {:.1} Myr, in and out every {:.1} Myr, through the plane every {:.1} Myr",
        s.r_min / KILOPARSEC,
        s.r_max / KILOPARSEC,
        s.z_max / PARSEC,
        s.azimuthal_period / myr,
        s.radial_period.unwrap() / myr,
        s.vertical_period.unwrap() / myr
    );
    assert!(orbit.energy() < 0.0);
}
