//! Every other known moon: JPL's list of planetary satellites beyond the
//! 21 major moons, with their mean orbits and, where JPL has one, their
//! exact state at the snapshot epoch.
//!
//! `tools/fetch_small_moons.py` regenerates the data. See
//! `docs/physics/small-moons.md`.

use std::collections::HashMap;
use std::sync::OnceLock;

use worldline_core::constants::{DAY, JULIAN_YEAR, OBLIQUITY_J2000};
use worldline_core::mean_elements::{MeanElements, plane_frame};
use worldline_core::{DMat3, DVec3};

use crate::{ParseError, moon_systems, rotation_model, solar_system};

/// A small moon.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallMoon {
    /// IAU name, or provisional designation such as "S/2004 S 21".
    pub name: String,
    /// NAIF id.
    pub naif_id: u32,
    /// The planetary system's name in the solar-system snapshot.
    pub parent: String,
    /// GM in m³/s², where JPL has measured one.
    pub gm: Option<f64>,
    /// Mean radius in m, where the IAU or JPL publishes one.
    pub radius: Option<f64>,
    /// Position (m) and velocity (m/s) relative to the planet's center at
    /// the snapshot epoch, from JPL Horizons, if JPL has an ephemeris
    /// covering it.
    pub state: Option<(DVec3, DVec3)>,
    /// Whether it is a regular moon, orbiting near the planet's equator
    /// (JPL gives its elements relative to a Laplace or equatorial plane),
    /// rather than an irregular one on a distant, tilted orbit.
    pub regular: bool,
    /// The moon's approximate orbit, relative to the planet's center, with
    /// simulation time counted from the snapshot epoch: the ellipse through
    /// JPL's 2025 state turning at JPL's mean rates, or JPL's mean elements
    /// where there is no 2025 state.
    pub orbit: MeanElements,
}

/// One row of JPL's mean elements, in JPL's units.
#[derive(Debug, Clone, PartialEq)]
pub struct ElementsRow {
    /// Display name (see [`display_name`]).
    pub name: String,
    /// NAIF id.
    pub naif_id: u32,
    /// The planet ("Saturn"), or "Earth" for the Moon.
    pub parent: String,
    /// Reference plane: "ecliptic", "Laplace" or "equatorial".
    pub frame: String,
    /// Epoch of the elements, as a Julian Date (TDB).
    pub epoch_jd_tdb: f64,
    /// Semi-major axis, km.
    pub a_km: f64,
    /// Eccentricity.
    pub e: f64,
    /// Argument of periapsis, degrees.
    pub w_deg: f64,
    /// Mean anomaly, degrees.
    pub m_deg: f64,
    /// Inclination to the reference plane, degrees.
    pub i_deg: f64,
    /// Longitude of the ascending node, degrees.
    pub node_deg: f64,
    /// Sidereal period, days.
    pub period_days: f64,
    /// Periapsis precession period, years, if JPL gives one.
    pub apsis_period_yr: Option<f64>,
    /// Node precession period, years, if JPL gives one.
    pub node_period_yr: Option<f64>,
    /// Right ascension and declination (degrees, ICRF) of the Laplace
    /// plane's pole, for elements in the Laplace frame.
    pub laplace_pole_deg: Option<(f64, f64)>,
}

/// "S2004_S_21" or "S2023_U1" (JPL's spelling) as "S/2004 S 21"; other
/// names unchanged.
pub fn display_name(name: &str) -> String {
    let bytes = name.as_bytes();
    if bytes.len() > 6 && bytes[0] == b'S' && bytes[1..5].iter().all(u8::is_ascii_digit) {
        let rest: String = name[5..].chars().filter(|&c| c != '_').collect();
        let (planet, number) = rest.split_at(1);
        return format!("S/{} {planet} {number}", &name[1..5]);
    }
    name.to_string()
}

/// JPL's mean elements for every planetary satellite in its list, the
/// major moons and the Moon included.
pub fn moon_elements() -> &'static [ElementsRow] {
    static ROWS: OnceLock<Vec<ElementsRow>> = OnceLock::new();
    ROWS.get_or_init(|| {
        parse_elements(include_str!("../data/moon-elements.csv"))
            .expect("bundled moon elements are valid")
    })
}

/// How many satellites JPL's list holds, after merging duplicate entries
/// for one moon, as the data file records it.
pub fn satellites_in_jpls_list() -> usize {
    include_str!("../data/moon-elements.csv")
        .lines()
        .find_map(|l| l.strip_prefix("# satellites:"))
        .and_then(|l| l.split(',').nth(1))
        .and_then(|l| l.split_whitespace().next())
        .and_then(|n| n.parse().ok())
        .expect("the elements file records its satellite count")
}

/// Parses the mean-elements CSV format.
pub fn parse_elements(text: &str) -> Result<Vec<ElementsRow>, ParseError> {
    let mut rows = Vec::new();
    for (index, raw) in text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.starts_with('#'))
        .skip(1)
    {
        let line = index + 1;
        let f: Vec<&str> = raw.split(',').map(str::trim).collect();
        if f.len() != 17 {
            return Err(ParseError::new(
                line,
                format!("expected 17 fields, found {}", f.len()),
            ));
        }
        let optional = |i: usize| -> Result<Option<f64>, ParseError> {
            if f[i].is_empty() {
                return Ok(None);
            }
            f[i].parse::<f64>()
                .map(Some)
                .map_err(|_| ParseError::new(line, format!("`{}` is not a number", f[i])))
        };
        let number = |i: usize| {
            optional(i)?.ok_or_else(|| ParseError::new(line, format!("field {} is empty", i + 1)))
        };
        rows.push(ElementsRow {
            name: display_name(f[0]),
            naif_id: f[1]
                .parse()
                .map_err(|_| ParseError::new(line, format!("`{}` is not an id", f[1])))?,
            parent: f[2].to_string(),
            frame: f[4].to_string(),
            epoch_jd_tdb: number(5)?,
            a_km: number(6)?,
            e: number(7)?,
            w_deg: number(8)?,
            m_deg: number(9)?,
            i_deg: number(10)?,
            node_deg: number(11)?,
            period_days: number(12)?,
            apsis_period_yr: optional(13)?,
            node_period_yr: optional(14)?,
            laplace_pole_deg: optional(15)?.zip(optional(16)?),
        });
    }
    Ok(rows)
}

impl ElementsRow {
    /// The mean orbit, relative to the planet's center, with simulation
    /// time counted from Julian Date `start_jd`.
    pub fn mean_elements(&self, start_jd: f64) -> MeanElements {
        let frame = self.reference_frame();
        MeanElements {
            frame,
            epoch: (self.epoch_jd_tdb - start_jd) * DAY,
            a: self.a_km * 1e3,
            e: self.e,
            inclination: self.i_deg.to_radians(),
            node: self.node_deg.to_radians(),
            periapsis: self.w_deg.to_radians(),
            mean_anomaly: self.m_deg.to_radians(),
            period: self.period_days * DAY,
            periapsis_rate: 0.0,
            node_rate: 0.0,
        }
        .with_precession_periods(self.apsis_period(), self.node_period())
    }

    /// The moon's orbit through JPL's exact `position` and `velocity` (m,
    /// m/s, relative to the planet's center) at simulation time 0, turning
    /// at the mean rates JPL publishes: the osculating ellipse, carried
    /// round once per sidereal period, its periapsis and node precessing.
    /// Exact at the start, drifting slowly from there.
    pub fn anchored_elements(
        &self,
        position: DVec3,
        velocity: DVec3,
        planet_gm: f64,
    ) -> MeanElements {
        MeanElements {
            period: self.period_days * DAY,
            ..MeanElements::osculating(position, velocity, planet_gm, self.reference_frame(), 0.0)
        }
        .with_precession_periods(self.apsis_period(), self.node_period())
    }

    fn apsis_period(&self) -> Option<f64> {
        self.apsis_period_yr.map(|y| y * JULIAN_YEAR)
    }

    fn node_period(&self) -> Option<f64> {
        self.node_period_yr.map(|y| y * JULIAN_YEAR)
    }

    /// The frame the elements are measured in (see [`MeanElements::frame`]).
    fn reference_frame(&self) -> DMat3 {
        let (ra, dec) = match (self.frame.as_str(), self.laplace_pole_deg) {
            ("Laplace", Some(pole)) => pole,
            // The planet's equator, about its spin axis (the right-hand
            // pole), held at its J2000 direction.
            ("equatorial", _) => {
                let model = rotation_model(&self.parent).expect("planets have rotation models");
                if model.spin_rate() < 0.0 {
                    (model.pole_ra[0] + 180.0, -model.pole_dec[0])
                } else {
                    (model.pole_ra[0], model.pole_dec[0])
                }
            }
            // The J2000 ecliptic, whose node on the ICRF equator is the
            // equinox: the simulation's own frame.
            ("ecliptic", _) => return DMat3::IDENTITY,
            (frame, _) => panic!("{}: unknown frame `{frame}`", self.name),
        };
        plane_frame(ra.to_radians(), dec.to_radians(), OBLIQUITY_J2000)
    }
}

/// The NAIF id of a small moon, by its display name.
pub(crate) fn naif_id(name: &str) -> Option<u32> {
    static IDS: OnceLock<HashMap<String, u32>> = OnceLock::new();
    IDS.get_or_init(|| {
        moon_elements()
            .iter()
            .map(|r| (r.name.clone(), r.naif_id))
            .collect()
    })
    .get(name)
    .copied()
}

/// One row of the small-moons state files.
struct StateRow {
    naif_id: u32,
    gm: Option<f64>,
    radius: Option<f64>,
    /// Position and velocity relative to the planet's center.
    state: (DVec3, DVec3),
}

fn parse_states(text: &str) -> Vec<StateRow> {
    let optional = |s: &str| (!s.is_empty()).then(|| s.parse::<f64>().expect("published number"));
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            let n = |i: usize| f[i].parse::<f64>().expect("state component") * 1e3;
            StateRow {
                naif_id: f[1].parse().expect("NAIF id"),
                gm: optional(f[3]).map(|gm| gm * 1e9),
                radius: optional(f[4]).map(|r| r * 1e3),
                state: (DVec3::new(n(5), n(6), n(7)), DVec3::new(n(8), n(9), n(10))),
            }
        })
        .collect()
}

/// Every small moon, planet by planet, innermost first, at the snapshot
/// epoch (2025-01-01 00:00 TDB).
pub fn small_moons() -> Vec<SmallMoon> {
    let start_jd = solar_system().epoch_jd_tdb;
    let states: HashMap<u32, StateRow> =
        parse_states(include_str!("../data/small-moons-2025-01-01.csv"))
            .into_iter()
            .map(|r| (r.naif_id, r))
            .collect();
    let systems = moon_systems();
    let majors: Vec<&str> = systems
        .iter()
        .flat_map(|s| s.bodies.iter().map(|b| b.name.as_str()))
        .collect();
    moon_elements()
        .iter()
        .filter(|e| {
            systems.iter().any(|s| s.host == e.parent) && !majors.contains(&e.name.as_str())
        })
        .map(|e| {
            let row = states.get(&e.naif_id);
            // Most of JPL's elements are dated 2000, and a quarter century
            // of a rounded period loses track of where along its orbit the
            // moon is; an irregular moon's real ellipse also wanders far
            // from the average one. Where JPL has the moon's 2025 state,
            // start from that instead.
            let orbit = match row {
                Some(row) => {
                    let planet = systems
                        .iter()
                        .find(|s| s.host == e.parent)
                        .expect("parent system");
                    e.anchored_elements(row.state.0, row.state.1, planet.bodies[0].gm)
                }
                None => e.mean_elements(start_jd),
            };
            SmallMoon {
                name: e.name.clone(),
                naif_id: e.naif_id,
                parent: e.parent.clone(),
                gm: row.and_then(|r| r.gm),
                radius: row.and_then(|r| r.radius),
                state: row.map(|r| r.state),
                regular: e.frame != "ecliptic",
                orbit,
            }
        })
        .collect()
}

/// The small moons' states 30 days after the snapshot (2025-01-31 00:00
/// TDB), from JPL Horizons, relative to their planets' centers: the
/// reference for validation. Keyed by NAIF id.
pub fn small_moon_states_30_days_on() -> HashMap<u32, (DVec3, DVec3)> {
    parse_states(include_str!("../data/small-moons-2025-01-31.csv"))
        .into_iter()
        .map(|row| (row.naif_id, row.state))
        .collect()
}

/// A planet's oblateness as one JPL solution has it.
#[derive(Debug, Clone, PartialEq)]
pub struct SolutionZonal {
    /// The solution's name ("URA184").
    pub solution: String,
    /// Reference radius, in m.
    pub radius: f64,
    /// (degree, J_n) pairs, zeros left out.
    pub coefficients: Vec<(u32, f64)>,
}

/// A planet's oblateness as the JPL solution of its small moons has it.
pub fn small_moon_zonal_harmonics(planet: &str) -> Option<SolutionZonal> {
    let mut found: Option<SolutionZonal> = None;
    for line in include_str!("../data/small-moon-zonal.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
    {
        let f: Vec<&str> = line.split(',').collect();
        if f[0] != planet {
            continue;
        }
        let radius = f[2].parse::<f64>().expect("radius is a number") * 1e3;
        let degree: u32 = f[3].parse().expect("degree is an integer");
        let j: f64 = f[4].parse().expect("J is a number");
        let entry = found.get_or_insert_with(|| SolutionZonal {
            solution: f[1].to_string(),
            radius,
            coefficients: Vec::new(),
        });
        if j != 0.0 {
            entry.coefficients.push((degree, j));
        }
    }
    found
}

/// GM (m³/s²) of the small moons that orbit inside `planet`'s innermost
/// major moon and that JPL's integration of the major moons includes.
pub fn inner_moon_mass(planet: &str) -> f64 {
    include_str!("../data/inner-moon-masses.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').collect::<Vec<&str>>())
        .filter(|f| f[0] == planet)
        .map(|f| f[3].parse::<f64>().expect("GM is a number") * 1e9)
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provisional_designations_read_as_the_iau_writes_them() {
        assert_eq!(display_name("S2004_S_21"), "S/2004 S 21");
        assert_eq!(display_name("S2023_U1"), "S/2023 U 1");
        assert_eq!(display_name("Pan"), "Pan");
    }

    #[test]
    fn small_moons_load_with_their_planets() {
        let moons = small_moons();
        let pan = moons.iter().find(|m| m.name == "Pan").unwrap();
        assert_eq!((pan.naif_id, pan.parent.as_str()), (618, "Saturn"));
        // Pan circles inside the Encke Gap, 133,584 km from Saturn.
        let (r, _) = pan.state.unwrap();
        assert!(
            (r.length() / 133_584e3 - 1.0).abs() < 0.01,
            "Pan at {} km",
            r.length() / 1e3
        );
        // Ordered innermost first within each planet, by JPL's mean
        // distance (co-orbital moons trade places in their current orbits).
        let saturn: Vec<f64> = moons
            .iter()
            .filter(|m| m.parent == "Saturn")
            .map(|m| {
                moon_elements()
                    .iter()
                    .find(|e| e.naif_id == m.naif_id)
                    .unwrap()
                    .a_km
            })
            .collect();
        assert!(saturn.windows(2).all(|w| w[0] <= w[1]));
    }
}
