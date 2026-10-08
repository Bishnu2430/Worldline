//! The galactic center: Sagittarius A* where it is on the sky, 8,277 pc
//! away, and the stars that orbit it (the S-stars), from their published
//! orbits.
//!
//! `tools/write_galactic_center.py` regenerates the data. See
//! `docs/physics/galactic-center.md`.

use std::f64::consts::TAU;

use worldline_core::constants::{AU, JULIAN_YEAR, OBLIQUITY_J2000, PARSEC};
use worldline_core::gravity::{EinsteinInfeldHoffmann, PostNewtonian};
use worldline_core::hierarchy::Hierarchy;
use worldline_core::mean_elements::solve_kepler;
use worldline_core::{Body, DMat3, DVec3, System};

/// A star's published orbit around Sagittarius A*.
#[derive(Debug, Clone, PartialEq)]
pub struct SStar {
    /// Its name ("S2", "R34").
    pub name: String,
    /// Semi-major axis, in arcseconds.
    pub a_arcsec: f64,
    /// Eccentricity.
    pub e: f64,
    /// Inclination, in rad.
    pub inclination: f64,
    /// Position angle of the ascending node, east of north, in rad.
    pub node: f64,
    /// Argument of pericenter, in rad.
    pub periapsis: f64,
    /// When it passed pericenter, as a Julian-epoch year.
    pub t_peri: f64,
    /// Its period, in years, if published; else from Kepler's law.
    pub period: Option<f64>,
    /// The year its elements are osculating at, for fits that include
    /// relativity (GRAVITY's); `None` for Keplerian fits.
    pub osculating: Option<f64>,
    /// The distance its paper converts angles with, in pc.
    pub r0_pc: f64,
    /// 'e' for an early-type (young, hot) star, 'l' for a late-type
    /// (cool giant) one, None if unknown.
    pub spectral_type: Option<char>,
    /// Where its orbit was published.
    pub source: String,
}

impl SStar {
    /// Semi-major axis, in m.
    pub fn a(&self) -> f64 {
        self.a_arcsec * self.r0_pc * AU
    }
}

/// Splits a CSV line whose last field is quoted and may contain commas.
fn fields(line: &str, count: usize) -> Vec<String> {
    line.splitn(count, ',')
        .map(|f| f.trim_matches('"').to_string())
        .collect()
}

/// The 39 stars with published orbits.
pub fn s_stars() -> Vec<SStar> {
    include_str!("../data/s-stars.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f = fields(line, 13);
            let number = |i: usize| f[i].parse::<f64>().expect("published number");
            let optional = |i: usize| (!f[i].is_empty()).then(|| number(i));
            SStar {
                name: f[0].clone(),
                a_arcsec: number(1),
                e: number(2),
                inclination: number(3).to_radians(),
                node: number(4).to_radians(),
                periapsis: number(5).to_radians(),
                t_peri: number(6),
                period: optional(7),
                osculating: optional(8),
                r0_pc: number(9),
                spectral_type: f[10].chars().next(),
                source: f[12].clone(),
            }
        })
        .collect()
}

/// Sagittarius A*'s right ascension and declination (ICRF3), in rad.
pub fn sgr_a_star_sky_position() -> (f64, f64) {
    let line = include_str!("../data/sgr-a-star-position.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .nth(1)
        .expect("a position");
    let f = fields(line, 7);
    let n = |i: usize| f[i].parse::<f64>().expect("published number");
    let ra = (n(0) + n(1) / 60.0 + n(2) / 3600.0) * 15.0;
    // The sign belongs to the whole declination: −29° 00′ 28″.
    let sign = if f[3].starts_with('-') { -1.0 } else { 1.0 };
    let dec = sign * (n(3).abs() + n(4) / 60.0 + n(5) / 3600.0);
    (ra.to_radians(), dec.to_radians())
}

/// Equatorial (ICRF) to the simulation's frame, the ecliptic and equinox
/// of J2000: a turn about the x axis by the obliquity.
fn to_ecliptic(v: DVec3) -> DVec3 {
    DMat3::from_rotation_x(-OBLIQUITY_J2000) * v
}

/// The sky's directions at Sagittarius A*, in the simulation's frame:
/// north, east, and away from us along the line of sight.
fn sky_frame() -> (DVec3, DVec3, DVec3) {
    let (ra, dec) = sgr_a_star_sky_position();
    let (sa, ca) = ra.sin_cos();
    let (sd, cd) = dec.sin_cos();
    let away = DVec3::new(cd * ca, cd * sa, sd);
    let east = DVec3::new(-sa, ca, 0.0);
    let north = DVec3::new(-sd * ca, -sd * sa, cd);
    (to_ecliptic(north), to_ecliptic(east), to_ecliptic(away))
}

/// Sagittarius A*'s direction from the Sun, in the simulation's frame.
pub fn sgr_a_star_direction() -> DVec3 {
    sky_frame().2
}

/// Simulation time (s after 2025-01-01 00:00 TDB) of a Julian-epoch year
/// such as 2018.3789: J2000.0 is JD 2451545.0, and a Julian year 365.25
/// days.
pub fn year_to_time(year: f64) -> f64 {
    let jd = 2_451_545.0 + (year - 2000.0) * 365.25;
    (jd - crate::solar_system().epoch_jd_tdb) * 86_400.0
}

/// The state of `star` relative to Sagittarius A* (gravitational
/// parameter `gm`) at simulation time `time`, on its two-body orbit, in
/// the simulation's frame. The Thiele–Innes constants turn the orbit into
/// north, east and line-of-sight offsets (Wright & Howard 2009, *ApJS*
/// 182, 205).
pub fn kepler_state(star: &SStar, gm: f64, time: f64) -> (DVec3, DVec3) {
    let a = star.a();
    let period = star
        .period
        .map_or_else(|| TAU * (a.powi(3) / gm).sqrt(), |p| p * JULIAN_YEAR);
    let n = TAU / period;
    let mean = (n * (time - year_to_time(star.t_peri))).rem_euclid(TAU);
    let e = star.e;
    let big_e = solve_kepler(mean, e);
    let (se, ce) = big_e.sin_cos();
    let root = (1.0 - e * e).sqrt();
    let (x, y) = (ce - e, root * se);
    let rate = n / (1.0 - e * ce);
    let (dx, dy) = (-se * rate, root * ce * rate);
    let (so, co) = star.periapsis.sin_cos();
    let (sn, cn) = star.node.sin_cos();
    let (si, ci) = star.inclination.sin_cos();
    let thiele = [
        // (north, east, away), each as the coefficients of x and y.
        (a * (co * cn - so * sn * ci), a * (-so * cn - co * sn * ci)),
        (a * (co * sn + so * cn * ci), a * (-so * sn + co * cn * ci)),
        (a * so * si, a * co * si),
    ];
    let (north, east, away) = sky_frame();
    let axes = [north, east, away];
    let mut position = DVec3::ZERO;
    let mut velocity = DVec3::ZERO;
    for ((cx, cy), axis) in thiele.iter().zip(axes) {
        position += axis * (cx * x + cy * y);
        velocity += axis * (cx * dx + cy * dy);
    }
    (position, velocity)
}

/// The galactic center as a region of its own: Sagittarius A*, still, at
/// its local origin, and the S-stars following it.
pub struct GalacticCenter {
    /// Where Sagittarius A* is, relative to the solar system's
    /// barycenter, in the simulation's frame, in m: 8,277 pc along its
    /// direction on the sky.
    pub origin: DVec3,
    /// Sagittarius A* and its stars, around the local origin.
    pub hierarchy: Hierarchy,
}

/// Sagittarius A* as the region's central body, at rest at its origin.
fn sgr_a_star() -> Body {
    crate::notable_object("Sagittarius A*")
        .expect("Sagittarius A* is in the catalog")
        .body()
}

/// The galactic center at the snapshot (2025-01-01). Stars from Keplerian
/// fits are placed on their ellipses. GRAVITY's elements include
/// relativity and are osculating at an earlier epoch (2010.35 for S2):
/// those stars are placed there and run forward to the snapshot with the
/// same gravity the region uses.
pub fn galactic_center() -> GalacticCenter {
    let center = sgr_a_star();
    let gm = center.gm;
    let catalog = crate::notable_object("Sagittarius A*").expect("in the catalog");
    let distance = match catalog.distance {
        Some(crate::Distance::Meters(d)) => d,
        _ => 8277.0 * PARSEC,
    };
    let stars = s_stars();
    let followers = stars
        .iter()
        .map(|star| {
            let (position, velocity) = match star.osculating {
                None => kepler_state(star, gm, 0.0),
                Some(year) => {
                    let start = year_to_time(year);
                    let (r, v) = kepler_state(star, gm, start);
                    let mut run = Hierarchy::new(
                        System::new(vec![center.clone()]),
                        Box::new(EinsteinInfeldHoffmann),
                        Vec::new(),
                    );
                    run.set_time(start);
                    run.add_followers(vec![(
                        Body::new(star.name.clone(), 0.0, 0.0).at(r).moving(v),
                        None,
                    )]);
                    run.advance(-start);
                    let b = run.follower(0);
                    (b.position, b.velocity)
                }
            };
            (
                Body::new(star.name.clone(), 0.0, 0.0)
                    .at(position)
                    .moving(velocity),
                None,
            )
        })
        .collect();
    // The stars follow Sagittarius A* (FollowerGravity); bodies dropped into
    // the region pull on each other with the same gravity as at home.
    let mut hierarchy = Hierarchy::new(
        System::new(vec![center]),
        Box::new(PostNewtonian::default()),
        Vec::new(),
    );
    hierarchy.add_followers(followers);
    GalacticCenter {
        origin: sgr_a_star_direction() * distance,
        hierarchy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stars_and_position_load() {
        let stars = s_stars();
        assert_eq!(stars.len(), 39);
        let s2 = &stars[0];
        assert_eq!(s2.name, "S2");
        // 0.12495″ at 8277 pc: 1034 AU.
        assert!((s2.a() / AU - 1034.2).abs() < 0.1, "{}", s2.a() / AU);
        let (ra, dec) = sgr_a_star_sky_position();
        assert!((ra.to_degrees() - 266.416_808_5).abs() < 1e-6);
        assert!((dec.to_degrees() + 29.007_837_8).abs() < 1e-6);
        // The epoch of S2's pericenter, 2018.3789, is 6.6 years before the
        // snapshot.
        assert!((year_to_time(2018.3789) / JULIAN_YEAR + 6.6218).abs() < 1e-3);
    }
}
