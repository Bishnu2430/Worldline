//! Major moons: their states, their planets' oblateness, and the solar
//! system assembled as a hierarchy of moon systems.
//!
//! `tools/fetch_moons.py` regenerates the data. See `docs/physics/moons.md`.

use std::collections::HashMap;

use worldline_core::constants::DAY;
use worldline_core::gravity::{
    EinsteinInfeldHoffmann, SynchronousFigure, TesseralField, TesseralTerm, ZonalField,
};
use worldline_core::hierarchy::{Hierarchy, MoonSystem};
use worldline_core::{Body, DVec3};

use crate::{ParseError, rotation_model, solar_system};

/// A planet and its major moons, in the solar-system snapshot's frame.
#[derive(Debug, Clone, PartialEq)]
pub struct MoonSystemData {
    /// The planetary system's name in the solar-system snapshot ("Jupiter").
    pub host: String,
    /// The planet's center first, then its moons, in SI units.
    pub bodies: Vec<Body>,
}

/// Parses the moons CSV format: the snapshot format plus a `parent` column
/// naming each body's planetary system. Returns the epoch (JD, TDB) and the
/// systems in file order.
pub fn parse_moons(text: &str) -> Result<(f64, Vec<MoonSystemData>), ParseError> {
    let mut epoch = None;
    let mut systems: Vec<MoonSystemData> = Vec::new();
    let mut header_seen = false;
    for (index, raw) in text.lines().enumerate() {
        let line_number = index + 1;
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            if let Some(value) = comment.trim().strip_prefix("epoch_jd_tdb:") {
                epoch = value.trim().parse::<f64>().ok();
            }
            continue;
        }
        if !header_seen {
            header_seen = true;
            continue;
        }
        let fields: Vec<&str> = line.split(',').map(str::trim).collect();
        if fields.len() != 11 {
            return Err(ParseError::new(
                line_number,
                format!("expected 11 fields, found {}", fields.len()),
            ));
        }
        let number = |i: usize| {
            fields[i]
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| {
                    ParseError::new(line_number, format!("`{}` is not a number", fields[i]))
                })
        };
        let km = 1e3;
        let body = Body::new(fields[0], number(3)? * 1e9, number(4)? * km)
            .at(DVec3::new(number(5)?, number(6)?, number(7)?) * km)
            .moving(DVec3::new(number(8)?, number(9)?, number(10)?) * km);
        let host = fields[2];
        match systems.iter_mut().find(|s| s.host == host) {
            Some(system) => system.bodies.push(body),
            None => systems.push(MoonSystemData {
                host: host.to_string(),
                bodies: vec![body],
            }),
        }
    }
    let epoch = epoch.ok_or_else(|| ParseError::new(0, "missing `# epoch_jd_tdb:` line"))?;
    Ok((epoch, systems))
}

/// The major moons at 2025-01-01 00:00 TDB, with their planets' centers.
pub fn moon_systems() -> Vec<MoonSystemData> {
    parse_moons(include_str!("../data/moons-2025-01-01.csv"))
        .expect("bundled moon data is valid")
        .1
}

/// A planet's zonal gravity harmonics: reference radius (m) and (degree,
/// J_n) pairs, from the constants of JPL's satellite ephemerides.
pub fn zonal_harmonics(planet: &str) -> Option<(f64, Vec<(u32, f64)>)> {
    let mut radius = None;
    let mut terms = Vec::new();
    for line in include_str!("../data/zonal-harmonics.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
    {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0] != planet {
            continue;
        }
        radius = fields[1].parse::<f64>().ok().map(|r| r * 1e3);
        let degree: u32 = fields[2].parse().expect("degree is an integer");
        let j: f64 = fields[3].parse().expect("J is a number");
        if j != 0.0 {
            terms.push((degree, j));
        }
    }
    Some((radius?, terms))
}

/// A planet's tesseral gravity harmonics: reference radius (m) and (degree,
/// order, C_nm, S_nm), from the constants of JPL's satellite ephemerides.
pub fn tesseral_harmonics(planet: &str) -> Option<(f64, Vec<TesseralTerm>)> {
    let mut radius = None;
    let mut terms = Vec::new();
    for line in include_str!("../data/tesseral-harmonics.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
    {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0] != planet {
            continue;
        }
        radius = fields[1].parse::<f64>().ok().map(|r| r * 1e3);
        let number = |i: usize| fields[i].parse::<f64>().expect("coefficient is a number");
        let degree: u32 = fields[2].parse().expect("degree is an integer");
        let order: u32 = fields[3].parse().expect("order is an integer");
        terms.push((degree, order, number(4), number(5)));
    }
    Some((radius?, terms))
}

/// The shape of a tidally locked moon, from the constants of JPL's
/// satellite ephemerides, if JPL models it.
pub fn moon_figure(name: &str) -> Option<SynchronousFigure> {
    include_str!("../data/moon-figures.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').collect::<Vec<&str>>())
        .find(|fields| fields[0] == name)
        .map(|fields| {
            let number = |i: usize| {
                fields[i]
                    .parse::<f64>()
                    .expect("figure constant is a number")
            };
            SynchronousFigure {
                radius: number(2) * 1e3,
                j2: number(3),
                c22: number(4),
            }
        })
}

/// The total mass of a planet's rings as JPL's satellite ephemerides
/// include it, as GM in m³/s² (zero for planets without).
pub fn ring_mass(planet: &str) -> f64 {
    include_str!("../data/ring-masses.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').collect::<Vec<&str>>())
        .filter(|fields| fields[0] == planet)
        .map(|fields| fields[2].parse::<f64>().expect("ring GM is a number") * 1e9)
        .sum()
}

/// The solar system with its major moons, ready to simulate: the top level
/// from the 2025-01-01 snapshot with relativistic gravity, and each planet
/// with major moons as its own moon system with the planet's gravity field.
pub fn solar_system_with_moons() -> Hierarchy {
    let snapshot = solar_system();
    let top = snapshot.system();
    let index: HashMap<&str, usize> = top
        .bodies
        .iter()
        .enumerate()
        .map(|(i, b)| (b.name.as_str(), i))
        .collect();
    let moon_systems = moon_systems()
        .into_iter()
        .map(|mut data| {
            // The rings circle far inside the moons, so to the moons they
            // pull like extra mass at the planet's center (a ring of radius
            // ρ pulls a moon at distance a harder by a factor of about
            // 1 + ¾(ρ/a)², a few percent of an already small pull).
            data.bodies[0].gm += ring_mass(&data.host);
            let figures: Vec<(usize, SynchronousFigure)> = data
                .bodies
                .iter()
                .enumerate()
                .skip(1)
                .filter_map(|(i, moon)| Some((i, moon_figure(&moon.name)?)))
                .collect();
            let mut moons = MoonSystem::new(index[data.host.as_str()], data.bodies)
                .with_inner_mass(crate::inner_moon_mass(&data.host));
            for (i, figure) in figures {
                moons = moons.with_figure(i, figure);
            }
            let Some(rotation) = rotation_model(&data.host) else {
                return moons;
            };
            // The planet's orientation at the epoch. Its pole drifts by
            // hundredths of a degree per century, so it is held fixed.
            let frame = rotation.orientation(snapshot.epoch_jd_tdb).body_to_ecliptic;
            if let Some((radius, coefficients)) =
                zonal_harmonics(&data.host).filter(|z| !z.1.is_empty())
            {
                moons = moons.with_zonal(ZonalField {
                    radius,
                    coefficients,
                    pole: frame.z_axis,
                });
            }
            // The small moons' JPL solution may fit the planet's oblateness
            // differently; they feel the field their own solution has.
            if let Some(zonal) =
                crate::small_moon_zonal_harmonics(&data.host).filter(|z| !z.coefficients.is_empty())
            {
                moons.small_moon_zonal = Some(ZonalField {
                    radius: zonal.radius,
                    coefficients: zonal.coefficients,
                    pole: frame.z_axis,
                });
            }
            if let Some((radius, coefficients)) =
                tesseral_harmonics(&data.host).filter(|t| !t.1.is_empty())
            {
                moons = moons.with_tesseral(TesseralField {
                    radius,
                    coefficients,
                    frame,
                    spin_rate: rotation.spin_rate().to_radians() / DAY,
                    epoch: top.time(),
                });
            }
            moons
        })
        .collect();
    Hierarchy::new(top, Box::new(EinsteinInfeldHoffmann), moon_systems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moon_systems_load() {
        let systems = moon_systems();
        let names: Vec<&str> = systems.iter().map(|s| s.host.as_str()).collect();
        assert_eq!(
            names,
            ["Mars", "Jupiter", "Saturn", "Uranus", "Neptune", "Pluto"]
        );
        let jupiter = &systems[1];
        let moons: Vec<&str> = jupiter.bodies.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(moons, ["Jupiter", "Io", "Europa", "Ganymede", "Callisto"]);
        assert_eq!(
            systems.iter().map(|s| s.bodies.len() - 1).sum::<usize>(),
            21
        );
    }

    #[test]
    fn zonal_harmonics_load() {
        let (radius, terms) = zonal_harmonics("Saturn").unwrap();
        assert_eq!(radius, 60_330e3);
        assert_eq!(terms[0], (2, 1.629_061_510_215_236e-2));
        // Pluto's file gives J2 = 0: no terms.
        assert!(zonal_harmonics("Pluto").unwrap().1.is_empty());
        // Of these planets, only Mars's lumps matter to its moons.
        let (radius, terms) = tesseral_harmonics("Mars").unwrap();
        assert_eq!(radius, 3_396e3);
        assert_eq!(terms.len(), 20);
        assert_eq!(
            terms[1],
            (2, 2, -5.463_026_057_285_782e-5, 3.159_047_860_342_841e-5)
        );
        assert!(tesseral_harmonics("Jupiter").is_none());
        let io = moon_figure("Io").unwrap();
        assert_eq!((io.radius, io.j2, io.c22), (1_829.4e3, 1.8236e-3, 5.497e-4));
        assert!(moon_figure("Titan").is_none());
    }

    #[test]
    fn moon_solutions_agree_with_the_planetary_ephemeris_on_mass() {
        // Each moon system's GMs come from JPL's satellite solution, the
        // host's from DE440. They agree closely; Pluto's differ most
        // (975.43 vs 975.5 km³/s², 7 parts in 100,000). The bound is the
        // one the hierarchy enforces.
        let h = solar_system_with_moons();
        for moons in &h.moon_systems {
            let host = &h.top.bodies[moons.host];
            let ratio = moons.system.total_gm() / host.gm;
            println!("{}: moon solution / DE440 = {ratio:.9}", host.name);
            assert!((ratio - 1.0).abs() < 1e-4, "{}", host.name);
        }
    }
}
