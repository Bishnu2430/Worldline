//! Star clusters, the Orion Nebula and the Milky Way's satellite galaxies
//! (step 7.3), at their published places, distances and motions.
//!
//! `tools/fetch_clusters_and_galaxies.py` regenerates the data. See
//! `docs/physics/clusters-and-galaxies.md`.

use std::f64::consts::TAU;

use worldline_core::constants::{JULIAN_YEAR, PARSEC};
use worldline_core::{DMat3, DVec3};

use crate::neighbors::{equatorial_to_ecliptic, julian_year, sky_axes};

/// A milliarcsecond, in radians.
const MAS: f64 = TAU / (360.0 * 3600.0 * 1000.0);

/// What kind of object it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeepSkyKind {
    /// An old, dense ball of stars in the galaxy's halo or bulge.
    GlobularCluster,
    /// A young, loose cluster in the disk.
    OpenCluster,
    /// A cloud of gas lit by the stars in it.
    Nebula,
    /// A small galaxy going around the Milky Way.
    SatelliteGalaxy,
}

impl DeepSkyKind {
    /// Its name, as the data writes it.
    pub fn name(self) -> &'static str {
        match self {
            Self::GlobularCluster => "globular cluster",
            Self::OpenCluster => "open cluster",
            Self::Nebula => "nebula",
            Self::SatelliteGalaxy => "satellite galaxy",
        }
    }

    /// More than one of them: "globular clusters".
    pub fn plural(self) -> &'static str {
        match self {
            Self::GlobularCluster => "globular clusters",
            Self::OpenCluster => "open clusters",
            Self::Nebula => "nebulae",
            Self::SatelliteGalaxy => "satellite galaxies",
        }
    }

    /// The four, in the order the app lists them.
    pub const ALL: [Self; 4] = [
        Self::OpenCluster,
        Self::Nebula,
        Self::GlobularCluster,
        Self::SatelliteGalaxy,
    ];
}

/// A cluster, nebula or galaxy, as its catalogue publishes it.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepSkyObject {
    /// The name it's best known by: "Pleiades", "M13", "NGC 6397".
    pub name: String,
    /// Its other designations, if any.
    pub also: String,
    /// What kind of object it is.
    pub kind: DeepSkyKind,
    /// Right ascension (ICRS), rad.
    pub ra: f64,
    /// Declination (ICRS), rad.
    pub dec: f64,
    /// The Julian year its place and motion are given for.
    pub epoch: f64,
    /// Distance from the solar system's barycenter, m.
    pub distance: f64,
    /// The distance as published, with its error and unit: "4.52 ± 0.03
    /// kpc", or a distance modulus, "18.477 ± 0.024 mag".
    pub distance_published: String,
    /// Proper motion in right ascension (times cos δ) and declination,
    /// mas/yr.
    pub proper_motion: (f64, f64),
    /// Their errors, as published.
    pub proper_motion_error: (String, String),
    /// Radial velocity, m/s, positive away.
    pub radial_velocity: f64,
    /// Its error, km/s, as published.
    pub radial_velocity_error: String,
    /// Its size, m: a globular cluster's projected half-light radius, an
    /// open cluster's radius holding half its members, a galaxy's
    /// half-light radius along its major axis.
    pub radius: Option<f64>,
    /// The size as published: "4.03 pc", "192.9 arcmin".
    pub radius_published: Option<String>,
    /// Mass, Suns, where its catalogue gives one.
    pub mass: Option<f64>,
    /// Its error, Suns, as published.
    pub mass_error: Option<String>,
    /// Galactic longitude and latitude as its catalogue gives them, degrees.
    pub galactic: Option<(f64, f64)>,
    /// Its digits as published, for checking against: right ascension and
    /// declination as written, and its galactic coordinates as written.
    pub printed: Printed,
    /// Where each measurement comes from.
    pub source: String,
}

/// An object's coordinates as its catalogue writes them.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Printed {
    /// Right ascension: degrees, or h m s.
    pub ra: String,
    /// Declination: degrees, or d m s.
    pub dec: String,
    /// Distance, in its catalogue's unit.
    pub distance: String,
    /// Galactic longitude, degrees, if given.
    pub l: String,
    /// Galactic latitude, degrees, if given.
    pub b: String,
}

/// Splits a CSV line, where quoted fields may hold commas.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(String::new()),
            _ => out.last_mut().expect("a field").push(c),
        }
    }
    out
}

/// An angle in degrees, written in degrees or as "h m s" (`hours`) or
/// "d m s".
fn degrees(text: &str, hours: bool) -> f64 {
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() == 1 {
        return parts[0].parse().expect("a published angle");
    }
    let n = |i: usize| parts[i].parse::<f64>().expect("a published angle");
    // The sign belongs to the whole angle: −05° 23′ 40.1″.
    let sign = if parts[0].starts_with('-') { -1.0 } else { 1.0 };
    let value = n(0).abs() + n(1) / 60.0 + n(2) / 3600.0;
    sign * value * if hours { 15.0 } else { 1.0 }
}

/// A distance as published, in m: kpc, pc, or a distance modulus in mag,
/// m − M = 5 log₁₀(d/10 pc).
fn meters(value: f64, unit: &str) -> f64 {
    match unit {
        "kpc" => value * 1e3 * PARSEC,
        "pc" => value * PARSEC,
        "mag" => 10f64.powf(value / 5.0 + 1.0) * PARSEC,
        _ => panic!("unknown distance unit `{unit}`"),
    }
}

/// "0.03" as "± 0.03"; "-6/+7" as "−6/+7".
fn error_text(error: &str) -> String {
    if error.contains('/') {
        error.replace('-', "−")
    } else {
        format!("± {error}")
    }
}

/// Every cluster, nebula and galaxy in `clusters-and-galaxies.csv`.
pub fn deep_sky_objects() -> Vec<DeepSkyObject> {
    let text = include_str!("../data/clusters-and-galaxies.csv");
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty());
    let header = fields(lines.next().expect("a header"));
    let column = |name: &str| {
        header
            .iter()
            .position(|h| h == name)
            .unwrap_or_else(|| panic!("no column `{name}`"))
    };
    let at = |row: &[String], name: &str| row[column(name)].clone();
    let number = |row: &[String], name: &str| -> f64 {
        at(row, name)
            .parse()
            .unwrap_or_else(|_| panic!("`{name}` isn't a number"))
    };
    let optional = |row: &[String], name: &str| -> Option<f64> {
        let text = at(row, name);
        (!text.is_empty()).then(|| text.parse().expect("a published number"))
    };
    lines
        .map(|line| {
            let row = fields(line);
            let kind = match at(&row, "kind").as_str() {
                "globular cluster" => DeepSkyKind::GlobularCluster,
                "open cluster" => DeepSkyKind::OpenCluster,
                "nebula" => DeepSkyKind::Nebula,
                "satellite galaxy" => DeepSkyKind::SatelliteGalaxy,
                other => panic!("unknown kind `{other}`"),
            };
            let distance_unit = at(&row, "distance_unit");
            let distance = meters(number(&row, "distance"), &distance_unit);
            // A size in arcminutes is a size across the sky: times the
            // distance.
            let radius = optional(&row, "radius").map(|r| match at(&row, "radius_unit").as_str() {
                "pc" => r * PARSEC,
                "arcmin" => (r / 60.0).to_radians() * distance,
                unit => panic!("unknown radius unit `{unit}`"),
            });
            let galactic = optional(&row, "l").zip(optional(&row, "b"));
            let mass_error = at(&row, "mass_error");
            DeepSkyObject {
                name: at(&row, "name"),
                also: at(&row, "also"),
                kind,
                ra: degrees(&at(&row, "ra"), true).to_radians(),
                dec: degrees(&at(&row, "dec"), false).to_radians(),
                epoch: number(&row, "epoch"),
                distance,
                distance_published: format!(
                    "{} {} {distance_unit}",
                    at(&row, "distance"),
                    error_text(&at(&row, "distance_error"))
                ),
                proper_motion: (number(&row, "pmra"), number(&row, "pmdec")),
                proper_motion_error: (at(&row, "pmra_error"), at(&row, "pmdec_error")),
                radial_velocity: number(&row, "rv") * 1e3,
                radial_velocity_error: at(&row, "rv_error"),
                radius,
                radius_published: radius
                    .map(|_| format!("{} {}", at(&row, "radius"), at(&row, "radius_unit"))),
                mass: optional(&row, "mass"),
                mass_error: (!mass_error.is_empty()).then_some(mass_error),
                galactic,
                printed: Printed {
                    ra: at(&row, "ra"),
                    dec: at(&row, "dec"),
                    distance: at(&row, "distance"),
                    l: at(&row, "l"),
                    b: at(&row, "b"),
                },
                source: at(&row, "source"),
            }
        })
        .collect()
}

impl DeepSkyObject {
    /// Where it is and how it moves at its epoch, in equatorial axes from
    /// the solar system's barycenter (m, m/s): its direction and distance,
    /// its motion across the sky times its distance, and its radial
    /// velocity along the line of sight.
    pub fn measured_state(&self) -> (DVec3, DVec3) {
        let (north, east, away) = sky_axes(self.ra, self.dec);
        let (pmra, pmdec) = self.proper_motion;
        let across = (east * pmra + north * pmdec) * (MAS / JULIAN_YEAR);
        (
            away * self.distance,
            across * self.distance + away * self.radial_velocity,
        )
    }

    /// Where it is at Julian Date `jd_tdb`, in the simulation's frame (the
    /// ecliptic and equinox of J2000) from the solar system's barycenter:
    /// carried in a straight line from its epoch. Over the decade between,
    /// the galaxy's pull, under 3 × 10⁻⁹ m/s² even half a kiloparsec from
    /// its center, bends its path by under 2 × 10⁵ km: under 10⁻¹² of its
    /// distance, which is at least 7 kpc there.
    pub fn at(&self, jd_tdb: f64) -> (DVec3, DVec3) {
        let (position, velocity) = self.measured_state();
        let since = (julian_year(jd_tdb) - self.epoch) * JULIAN_YEAR;
        let turn = equatorial_to_ecliptic();
        (turn * (position + velocity * since), turn * velocity)
    }

    /// Where it is relative to the galaxy's center at Julian Date `jd_tdb`,
    /// galactocentric (see [`crate::GalacticFrame`]), given the solar
    /// system's barycenter's place and motion there then and the turn from
    /// the simulation's axes.
    pub fn galactocentric(
        &self,
        jd_tdb: f64,
        to_galactic: DMat3,
        barycenter: (DVec3, DVec3),
    ) -> (DVec3, DVec3) {
        let (position, velocity) = self.at(jd_tdb);
        (
            barycenter.0 + to_galactic * position,
            barycenter.1 + to_galactic * velocity,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn angles_and_distances_read_as_written() {
        assert!((degrees("05 35 15.68", true) - 83.815_333_333_333_33).abs() < 1e-12);
        assert!((degrees("-05 23 40.1", false) + 5.394_472_222_222_222).abs() < 1e-12);
        assert_eq!(degrees("-72.081306", false), -72.081306);
        // The LMC's distance modulus is Pietrzyński et al.'s 49.59 kpc.
        assert!((meters(18.477, "mag") / (1e3 * PARSEC) - 49.59).abs() < 0.005);
        assert_eq!(meters(4.52, "kpc"), 4.52e3 * PARSEC);
    }

    #[test]
    fn every_kind_is_there_and_names_are_unique() {
        let objects = deep_sky_objects();
        for kind in DeepSkyKind::ALL {
            assert!(objects.iter().any(|o| o.kind == kind), "{}", kind.name());
        }
        let mut names: Vec<&str> = objects.iter().map(|o| o.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), objects.len());
        for name in [
            "Pleiades",
            "Orion Nebula",
            "Omega Centauri",
            "47 Tucanae",
            "Large Magellanic Cloud",
            "Small Magellanic Cloud",
        ] {
            assert!(objects.iter().any(|o| o.name == name), "{name}");
        }
    }

    #[test]
    fn a_motion_across_the_sky_and_away_is_where_the_velocity_points() {
        // Straight away at 10 km/s, and nothing across: the velocity is
        // along the line of sight.
        let mut o = deep_sky_objects().remove(0);
        o.proper_motion = (0.0, 0.0);
        o.radial_velocity = 1e4;
        let (x, v) = o.measured_state();
        assert!((v.normalize() - x.normalize()).length() < 1e-15);
        assert!((v.length() - 1e4).abs() < 1e-9);
    }
}
