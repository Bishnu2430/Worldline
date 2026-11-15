//! The nearest and brightest stars (step 7.1), at their measured places and
//! motions: Gaia DR3 where it measures a star well, Hipparcos otherwise, and
//! α Centauri and Sirius by their published orbits.
//!
//! `tools/fetch_neighbors.py` regenerates the data. See
//! `docs/physics/neighbors.md`.

use std::f64::consts::TAU;

use worldline_core::constants::{AU, GM_SUN, JULIAN_YEAR, OBLIQUITY_J2000, PARSEC, SOLAR_RADIUS};
use worldline_core::mean_elements::solve_kepler;
use worldline_core::{Body, DMat3, DVec3};

use crate::notable::{ObjectKind, notable_object};

/// J2000.0, as a Julian Date.
const J2000: f64 = 2_451_545.0;

/// A milliarcsecond, in radians.
const MAS: f64 = TAU / (360.0 * 3600.0 * 1000.0);

/// A star from the catalogue, as measured.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbor {
    /// The star, placed at the moment asked for, in the simulation's frame
    /// (ecliptic and equinox of J2000), from the solar system's barycenter.
    pub body: Body,
    /// What kind of star it is.
    pub kind: ObjectKind,
    /// Where its place and motion come from: "Gaia DR3 …", "Hipparcos …",
    /// or its pair's orbit.
    pub catalog: String,
    /// For a star of a pair, whose motion the parallax, proper motion and
    /// radial velocity are: the pair's barycenter's, or its primary's.
    pub motion_of: Option<String>,
    /// The Julian year its place and motion were measured for.
    pub epoch: f64,
    /// Parallax, mas.
    pub parallax: f64,
    /// Proper motion in right ascension (times cos δ) and declination,
    /// mas/yr.
    pub proper_motion: (f64, f64),
    /// Their uncertainties, mas/yr, if given.
    pub proper_motion_error: Option<(f64, f64)>,
    /// Radial velocity, m/s.
    pub radial_velocity: f64,
    /// Where its mass and radius were published.
    pub source: String,
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

/// A file's rows, by its header's names.
fn rows(text: &str) -> Vec<std::collections::HashMap<String, String>> {
    let mut lines = text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty());
    let header = fields(lines.next().expect("a header"));
    lines
        .map(|l| header.iter().cloned().zip(fields(l)).collect())
        .collect()
}

fn number(row: &std::collections::HashMap<String, String>, key: &str) -> f64 {
    row[key].parse().expect("a published number")
}

/// North, east and away from the solar system, at right ascension `ra` and
/// declination `dec` (rad), in equatorial axes.
pub fn sky_axes(ra: f64, dec: f64) -> (DVec3, DVec3, DVec3) {
    let (sa, ca) = ra.sin_cos();
    let (sd, cd) = dec.sin_cos();
    (
        DVec3::new(-sd * ca, -sd * sa, cd),
        DVec3::new(-sa, ca, 0.0),
        DVec3::new(cd * ca, cd * sa, sd),
    )
}

/// Equatorial (ICRS) to the simulation's frame, the ecliptic and equinox of
/// J2000.
pub fn equatorial_to_ecliptic() -> DMat3 {
    DMat3::from_rotation_x(-OBLIQUITY_J2000)
}

/// The place and velocity a catalogue gives, in equatorial axes from the
/// solar system's barycenter (m, m/s): `ra`, `dec` in degrees, `parallax`
/// in mas, proper motion in mas/yr, radial velocity in m/s.
pub fn catalog_state(
    ra: f64,
    dec: f64,
    parallax: f64,
    proper_motion: (f64, f64),
    radial_velocity: f64,
) -> (DVec3, DVec3) {
    let (north, east, away) = sky_axes(ra.to_radians(), dec.to_radians());
    let distance = 1000.0 / parallax * PARSEC;
    let across = (east * proper_motion.0 + north * proper_motion.1) * (MAS / JULIAN_YEAR);
    (away * distance, across * distance + away * radial_velocity)
}

/// A visual binary's orbit: B around A, as seen on the sky.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualOrbit {
    /// Period, Julian years.
    pub period: f64,
    /// When it last passed periastron, Julian year.
    pub periastron: f64,
    /// Semi-major axis, arcsec.
    pub semi_major: f64,
    /// Eccentricity.
    pub eccentricity: f64,
    /// Inclination, deg.
    pub inclination: f64,
    /// The ascending node's position angle, deg.
    pub node: f64,
    /// The argument of periastron, deg.
    pub omega: f64,
    /// Parallax, mas.
    pub parallax: f64,
}

impl VisualOrbit {
    /// B's place and velocity relative to A at Julian year `year` (m,
    /// m/s), along north, east and away from the solar system. The node's
    /// side is taken as the one where B recedes, so "away" is +z of Heintz's
    /// (1978) convention.
    pub fn relative(&self, year: f64) -> (f64, DVec3, DVec3) {
        let a = self.semi_major / (self.parallax / 1000.0) * AU;
        let e = self.eccentricity;
        let n = TAU / (self.period * JULIAN_YEAR);
        let anomaly = solve_kepler(TAU * ((year - self.periastron) / self.period).fract(), e);
        let (i, node, omega) = (
            self.inclination.to_radians(),
            self.node.to_radians(),
            self.omega.to_radians(),
        );
        let (si, ci) = i.sin_cos();
        let (sn, cn) = node.sin_cos();
        let (sw, cw) = omega.sin_cos();
        let p = DVec3::new(cw * cn - sw * sn * ci, cw * sn + sw * cn * ci, sw * si);
        let q = DVec3::new(-sw * cn - cw * sn * ci, -sw * sn + cw * cn * ci, cw * si);
        let (se, ce) = anomaly.sin_cos();
        let root = (1.0 - e * e).sqrt();
        let position = p * (a * (ce - e)) + q * (a * root * se);
        let velocity = (p * -se + q * (root * ce)) * (n * a / (1.0 - e * ce));
        (a, position, velocity)
    }
}

/// The Julian year of Julian Date `jd`.
pub fn julian_year(jd: f64) -> f64 {
    2000.0 + (jd - J2000) / 365.25
}

/// Every star of the catalogue at Julian Date `jd_tdb`: each carried in a
/// straight line from where it was measured, at its epoch, and each pair's
/// stars placed on their orbit around their barycenter, which moves in a
/// straight line.
pub fn neighbors(jd_tdb: f64) -> Vec<Neighbor> {
    let now = julian_year(jd_tdb);
    let tilt = equatorial_to_ecliptic();
    let mut stars = Vec::new();
    for row in rows(include_str!("../data/neighbors.csv")) {
        let epoch = number(&row, "epoch");
        let proper_motion = (number(&row, "pmra"), number(&row, "pmdec"));
        let speed = number(&row, "rv") * 1e3;
        let (position, velocity) = catalog_state(
            number(&row, "ra"),
            number(&row, "dec"),
            number(&row, "parallax"),
            proper_motion,
            speed,
        );
        let position = position + velocity * ((now - epoch) * JULIAN_YEAR);
        let body = Body::new(
            row["name"].clone(),
            number(&row, "mass") * GM_SUN,
            number(&row, "radius") * SOLAR_RADIUS,
        )
        .at(tilt * position)
        .moving(tilt * velocity);
        let error = |key: &str| row.get(key).and_then(|v| v.parse::<f64>().ok());
        stars.push(Neighbor {
            body,
            kind: ObjectKind::Star,
            catalog: row["catalog"].clone(),
            motion_of: None,
            epoch,
            parallax: number(&row, "parallax"),
            proper_motion,
            proper_motion_error: error("pmra_error").zip(error("pmdec_error")),
            radial_velocity: speed,
            source: row["source"].clone(),
        });
    }
    for row in rows(include_str!("../data/neighbor-binaries.csv")) {
        let primary = notable_object(&row["primary"]).expect("the primary is in the catalog");
        let secondary = notable_object(&row["secondary"]).expect("the secondary is in the catalog");
        let (ma, mb) = (primary.mass.value, secondary.mass.value);
        // B's share of the mass: A sits that far along from the barycenter
        // toward the other side.
        let share = mb / (ma + mb);
        let orbit = VisualOrbit {
            period: number(&row, "period"),
            periastron: number(&row, "periastron"),
            semi_major: number(&row, "a"),
            eccentricity: number(&row, "e"),
            inclination: number(&row, "i"),
            node: number(&row, "node"),
            omega: number(&row, "omega"),
            parallax: number(&row, "parallax"),
        };
        let epoch = number(&row, "epoch");
        let (ra, dec) = (number(&row, "ra"), number(&row, "dec"));
        let proper_motion = (number(&row, "pmra"), number(&row, "pmdec"));
        let speed = number(&row, "rv") * 1e3;
        let (position, velocity) = catalog_state(ra, dec, orbit.parallax, proper_motion, speed);
        let (north, east, away) = sky_axes(ra.to_radians(), dec.to_radians());
        let sky = DMat3::from_cols(north, east, away);
        // Where the barycenter was at the epoch: given, or the primary's
        // place and motion less its own share of the orbit.
        let (center, drift) = if row["refers_to"] == "primary" {
            let (_, r, v) = orbit.relative(epoch);
            (position + sky * r * share, velocity + sky * v * share)
        } else {
            (position, velocity)
        };
        // The stars' speeds follow from their masses, Kepler's third law
        // with the orbit's size, so that what pulls them and how they move
        // agree. The published period agrees with them to its last digits.
        let (a, r, v) = orbit.relative(now);
        let mean_motion = TAU / (orbit.period * JULIAN_YEAR);
        let v = v * ((primary.gm() + secondary.gm()) / a.powi(3)).sqrt() / mean_motion;
        let center = center + drift * ((now - epoch) * JULIAN_YEAR);
        for (object, side) in [(&primary, -share), (&secondary, 1.0 - share)] {
            let body = Body::new(object.name.clone(), object.gm(), object.radius.value)
                .at(tilt * (center + sky * r * side))
                .moving(tilt * (drift + sky * v * side));
            stars.push(Neighbor {
                body,
                kind: object.kind,
                catalog: format!("the orbit of {} and {}", primary.name, secondary.name),
                motion_of: Some(if row["refers_to"] == "barycenter" {
                    "the pair's barycenter".to_string()
                } else {
                    primary.name.clone()
                }),
                epoch,
                parallax: orbit.parallax,
                proper_motion,
                proper_motion_error: None,
                radial_velocity: speed,
                source: row["source"].clone(),
            });
        }
    }
    stars
}

/// The visual orbit of the pair whose primary is `primary`, if it's one.
pub fn neighbor_orbit(primary: &str) -> Option<VisualOrbit> {
    rows(include_str!("../data/neighbor-binaries.csv"))
        .into_iter()
        .find(|row| row["primary"] == primary)
        .map(|row| VisualOrbit {
            period: number(&row, "period"),
            periastron: number(&row, "periastron"),
            semi_major: number(&row, "a"),
            eccentricity: number(&row, "e"),
            inclination: number(&row, "i"),
            node: number(&row, "node"),
            omega: number(&row, "omega"),
            parallax: number(&row, "parallax"),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalog_state_gives_back_its_proper_motion() {
        // Barnard's Star's, from its place and velocity: the motion across
        // the line of sight over the distance.
        let (position, velocity) = catalog_state(
            269.448_502_5,
            4.739_420_05,
            546.975_9,
            (-801.551, 10_362.394),
            -110_468.0,
        );
        let distance = position.length();
        assert!((distance / (1000.0 / 546.975_9 * PARSEC) - 1.0).abs() < 1e-14);
        let away = position / distance;
        let across = velocity - away * velocity.dot(away);
        let rate = across.length() / distance / MAS * JULIAN_YEAR;
        let wanted = (801.551f64.powi(2) + 10_362.394f64.powi(2)).sqrt();
        assert!((rate / wanted - 1.0).abs() < 1e-12, "{rate} {wanted}");
        assert!((velocity.dot(away) + 110_468.0).abs() < 1e-6);
    }

    #[test]
    fn an_orbit_goes_round_in_its_period_and_keeps_its_size() {
        let orbit = neighbor_orbit("Alpha Centauri A").expect("in the data");
        let (a, start, _) = orbit.relative(2020.0);
        let (_, later, _) = orbit.relative(2020.0 + orbit.period);
        assert!((start - later).length() < 1e-6 * a);
        // At periastron, a(1 − e).
        let (_, closest, _) = orbit.relative(orbit.periastron);
        assert!((closest.length() / (a * (1.0 - orbit.eccentricity)) - 1.0).abs() < 1e-12);
        // Its velocity is its place's slope.
        let dt = 1e-6;
        let (_, before, _) = orbit.relative(2020.0 - dt);
        let (_, after, v) = orbit.relative(2020.0 + dt);
        let slope = (after - before) / (2.0 * dt * JULIAN_YEAR);
        let (_, _, v0) = orbit.relative(2020.0);
        assert!((slope - v0).length() < 1e-6 * v.length());
    }

    #[test]
    fn every_star_is_placed() {
        let stars = neighbors(J2000 + 25.0 * 365.25);
        assert_eq!(stars.len(), 19);
        for s in &stars {
            let d = s.body.position.length() / PARSEC;
            assert!(d > 1.0 && d < 400.0, "{}: {d} pc", s.body.name);
            assert!(s.body.gm > 0.0 && s.body.radius > 0.0);
        }
    }
}
