//! Gaia's stars near the Sun, as the galaxy's swarm holds them (step 7.2).
//!
//! Run with `cargo test -p worldline-data --test validation_gaia_nearby_stars
//! -- --nocapture` to see the numbers.

use std::f64::consts::TAU;

use worldline_core::DVec3;
use worldline_core::constants::JULIAN_YEAR;
use worldline_data::{
    GAIA_DR3_EPOCH, equatorial_to_ecliptic, equatorial_to_galactic, gaia_nearby_stars, gaia_swarm,
    gaia_swarm_stars, galactic_frame, sky_axes, sun_motion,
};

/// A milliarcsecond, in radians.
const MAS: f64 = TAU / (360.0 * 3600.0 * 1000.0);

fn julian_date(year: f64) -> f64 {
    2_451_545.0 + (year - 2000.0) * 365.25
}

/// Each star, placed in the galaxy as the app places it, then measured back
/// from the solar system's barycenter at Gaia's epoch: its direction,
/// parallax, proper motion and radial velocity must come back as Gaia
/// published them, to double precision's rounding. Each step rounds to
/// about 10⁻¹⁶ of the largest number in it, the galaxy-centered place
/// (8 kpc) or velocity (250 km/s); the limit allows 10⁴ such roundings,
/// 10⁻¹² of those.
#[test]
fn each_is_where_gaia_measured_it() {
    let frame = galactic_frame();
    let jd = julian_date(GAIA_DR3_EPOCH);
    let stars = gaia_swarm_stars();
    let placed = gaia_swarm(jd, frame.to_galactic, (frame.position, frame.velocity));
    let to_equatorial = equatorial_to_ecliptic().transpose();
    let back = frame.to_galactic.transpose();
    let mut worst = 0.0_f64;
    for (s, p) in stars.iter().zip(&placed) {
        let x = to_equatorial * (back * (p.position - frame.position));
        let v = to_equatorial * (back * (p.velocity - frame.velocity));
        let distance = x.length();
        let ra = x.y.atan2(x.x).rem_euclid(TAU);
        let dec = (x.z / distance).asin();
        let (north, east, away) = sky_axes(ra, dec);
        let per = JULIAN_YEAR / (MAS * distance);
        let far = 1e-12 * (p.position.length() + frame.position.length());
        let fast = 1e-12 * (p.velocity.length() + frame.velocity.length());
        let measured = sky_axes(s.ra.to_radians(), s.dec.to_radians()).2;
        let offs = [
            (away - measured).length() * distance / far,
            (1000.0 / s.parallax * worldline_core::constants::PARSEC - distance).abs() / far,
            (v.dot(east) * per - s.proper_motion.0).abs() / (fast * per),
            (v.dot(north) * per - s.proper_motion.1).abs() / (fast * per),
            (v.dot(away) - s.radial_velocity).abs() / fast,
        ];
        for off in offs {
            assert!(off <= 1.0, "Gaia DR3 {}: {offs:?}", s.source_id);
            worst = worst.max(off);
        }
    }
    let all = gaia_nearby_stars().len();
    worldline_report::margins(
        "gaia-placed",
        "Gaia's stars within 50 pc, placed in the galaxy and measured back as Gaia measured them",
    )
    .check(
        &format!("all {} stars: place, distance, motions", stars.len()),
        worst,
        1.0,
        format!(
            "the worst off by {worst:.3} of what rounding allows (10⁻¹² of the places and speeds)"
        ),
    )
    .save(worldline_report::here!());
    println!(
        "{} of Gaia DR3's {all} stars within 50 pc with full motions join the swarm; {} are \
         already bodies of their own (step 7.1)",
        stars.len(),
        all - stars.len()
    );
    println!(
        "measured back from the barycenter at 2016.0: the worst off by {worst:.3} of what \
         rounding allows"
    );
}

/// On average the Sun's neighbors move as the Sun's own measured motion
/// against them says. Schönrich, Binney & Dehnen (2010, MNRAS 403, 1829)
/// found the Sun moving 11.1 (+0.69/−0.75, systematic 1.0) km/s toward the
/// galaxy's center and 7.25 (+0.37/−0.36, systematic 0.5) km/s toward its
/// north pole, relative to the stars around it. So Gaia's stars, turned
/// into galactic axes by the rotation the galaxy's frame is built on, must
/// on average move −11.1 and −7.25 km/s that way relative to the
/// barycenter.
///
/// The limit is three standard deviations of the difference: the
/// published statistical and systematic errors (the larger side) and the
/// mean's own, σ/√N, together in quadrature. Along the galaxy's turning
/// they lag the Sun by an amount that depends on their ages (asymmetric
/// drift), so that direction isn't checked.
#[test]
fn they_move_as_the_suns_motion_against_its_neighbors_says() {
    let turn = equatorial_to_galactic();
    let velocities: Vec<DVec3> = gaia_swarm_stars()
        .iter()
        .map(|s| turn * s.measured_state().1 / 1e3)
        .collect();
    let n = velocities.len() as f64;
    let mean = velocities.iter().fold(DVec3::ZERO, |a, v| a + *v) / n;
    let spread = velocities
        .iter()
        .fold(DVec3::ZERO, |a, v| a + (*v - mean) * (*v - mean))
        / (n - 1.0);
    let sun_u = sun_motion().u / 1e3;
    let checks = [
        (
            "toward the galaxy's center (U)",
            mean.x,
            -sun_u,
            0.75,
            1.0,
            spread.x,
        ),
        (
            "toward its north pole (W)",
            mean.z,
            -7.25,
            0.37,
            0.5,
            spread.z,
        ),
    ];
    let mut figure = worldline_report::margins(
        "gaia-motions",
        "Gaia's neighbors move on average as the Sun's motion against them says (Schönrich et al. 2010)",
    );
    for (what, got, wanted, statistical, systematic, variance) in checks {
        let of_mean = (variance / n).sqrt();
        let sigma =
            (statistical * statistical + systematic * systematic + of_mean * of_mean).sqrt();
        let off = (got - wanted).abs();
        println!(
            "{what}: {got:.2} ± {of_mean:.2} km/s, against {wanted:.2}; off by {off:.2}, \
             {:.2} standard deviations (limit 3)",
            off / sigma
        );
        figure = figure.check(
            what,
            off,
            3.0 * sigma,
            format!(
                "{got:.2} ± {of_mean:.2} km/s against {wanted:.2}: {:.2} standard deviations (limit 3)",
                off / sigma
            ),
        );
        assert!(off < 3.0 * sigma, "{what}");
    }
    figure.save(worldline_report::here!());
    println!(
        "along the galaxy's turning (V, not checked): {:.2} km/s, against the Sun's 12.24 \
         plus the stars' lag",
        mean.y
    );
}
