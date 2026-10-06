//! Dwarf planets, major asteroids and comets, from JPL's small-body
//! solutions.
//!
//! `tools/fetch_small_bodies.py` regenerates the data. See
//! `docs/physics/small-bodies.md`.

use std::collections::HashMap;

use worldline_core::constants::{AU, DAY};
use worldline_core::gravity::NonGravitational;
use worldline_core::hierarchy::Hierarchy;
use worldline_core::{Body, DVec3};

/// What sort of small body it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmallBodyKind {
    /// Ceres, Eris, Haumea, Makemake.
    DwarfPlanet,
    /// Large bodies beyond Neptune that the IAU hasn't classed as dwarf
    /// planets.
    TransNeptunian,
    /// An asteroid.
    Asteroid,
    /// A comet.
    Comet,
    /// A visitor from another star, on an unbound orbit.
    Interstellar,
}

/// A small body.
#[derive(Debug, Clone, PartialEq)]
pub struct SmallBody {
    /// Its name.
    pub name: String,
    /// What sort of body it is.
    pub kind: SmallBodyKind,
    /// NAIF id (2,000,000 plus the number for asteroids).
    pub naif_id: u32,
    /// GM in m³/s², for the bodies heavy enough that JPL's planetary
    /// ephemeris DE440 includes their pull. The rest are treated as
    /// massless.
    pub gm: Option<f64>,
    /// Mean radius in m, where published.
    pub radius: Option<f64>,
    /// Position (m) and velocity (m/s) relative to the solar system
    /// barycenter.
    pub state: (DVec3, DVec3),
    /// Its non-gravitational force model, if JPL fitted one.
    pub forces: Option<NonGravitational>,
}

impl SmallBody {
    /// The body as the simulation sees it (massless where no GM is known,
    /// a point if no size is known).
    pub fn body(&self) -> Body {
        Body::new(
            &self.name,
            self.gm.unwrap_or(0.0),
            self.radius.unwrap_or(0.0),
        )
        .at(self.state.0)
        .moving(self.state.1)
    }
}

fn parse(text: &str) -> Vec<SmallBody> {
    let optional = |s: &str| (!s.is_empty()).then(|| s.parse::<f64>().expect("published number"));
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            let km = |i: usize| f[i].parse::<f64>().expect("state component") * 1e3;
            let kind = match f[1] {
                "dwarf planet" => SmallBodyKind::DwarfPlanet,
                "trans-Neptunian object" => SmallBodyKind::TransNeptunian,
                "asteroid" => SmallBodyKind::Asteroid,
                "comet" => SmallBodyKind::Comet,
                "interstellar object" => SmallBodyKind::Interstellar,
                other => panic!("unknown kind `{other}`"),
            };
            let model: Vec<Option<f64>> = (11..20).map(|i| optional(f[i])).collect();
            let forces = model[0].map(|_| {
                let unit = AU / (DAY * DAY);
                let value = |i: usize| model[i].unwrap_or(0.0);
                NonGravitational {
                    strengths: DVec3::new(value(0), value(1), value(2)) * unit,
                    delay: value(3) * DAY,
                    alpha: value(4),
                    exponents: [value(5), value(6), value(7)],
                    r0: value(8) * AU,
                }
            });
            SmallBody {
                name: f[0].to_string(),
                kind,
                naif_id: f[2].parse().expect("NAIF id"),
                gm: optional(f[3]).map(|gm| gm * 1e9),
                radius: optional(f[4]).map(|r| r * 1e3),
                state: (
                    DVec3::new(km(5), km(6), km(7)),
                    DVec3::new(km(8), km(9), km(10)),
                ),
                forces,
            }
        })
        .collect()
}

/// The dwarf planets, major asteroids and comets at 2025-01-01 00:00 TDB.
pub fn small_bodies() -> Vec<SmallBody> {
    parse(include_str!("../data/small-bodies-2025-01-01.csv"))
}

/// Their states one Julian year later (2026-01-01 06:00 TDB), from JPL
/// Horizons: the reference for validation. Keyed by name.
pub fn small_bodies_one_year_on() -> HashMap<String, (DVec3, DVec3)> {
    parse(include_str!("../data/small-bodies-2026-01-01T06.csv"))
        .into_iter()
        .map(|b| (b.name, b.state))
        .collect()
}

/// JPL's prediction of the time of Halley's Comet's 2061 perihelion, as a
/// Julian Date (TDB).
pub fn jpl_halley_perihelion_jd() -> f64 {
    include_str!("../data/small-bodies-2025-01-01.csv")
        .lines()
        .find_map(|l| l.strip_prefix("# jpl_halley_perihelion_jd_tdb:"))
        .and_then(|v| v.trim().parse().ok())
        .expect("the data file records JPL's prediction")
}

/// The whole solar system: the Sun and planets, the dwarf planets and
/// asteroids massive enough to pull on them (at the top level, with
/// relativistic gravity), every planet's moons, and the massless small
/// bodies following along with their non-gravitational forces.
pub fn full_solar_system() -> Hierarchy {
    let bodies = small_bodies();
    let (massive, massless): (Vec<&SmallBody>, Vec<&SmallBody>) =
        bodies.iter().partition(|b| b.gm.is_some());
    let mut top = crate::solar_system().system();
    top.bodies.extend(massive.iter().map(|b| b.body()));
    let mut hierarchy = crate::moons::hierarchy_with_moons(top);
    hierarchy.add_followers(massless.iter().map(|b| (b.body(), b.forces)).collect());
    hierarchy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_bodies_load() {
        let bodies = small_bodies();
        let halley = bodies.iter().find(|b| b.name == "Halley").unwrap();
        assert_eq!(
            (halley.kind, halley.naif_id),
            (SmallBodyKind::Comet, 1_000_036)
        );
        // Halley's outgassing: JPL's water-ice model, pushing outward.
        let forces = halley.forces.unwrap();
        assert!(forces.strengths.x > 0.0 && forces.r0 == 2.808 * AU);
        // Ceres pulls (DE440's GM), Bennu doesn't.
        let gm = |name: &str| bodies.iter().find(|b| b.name == name).unwrap().gm;
        assert_eq!(gm("Ceres"), Some(62.628_888_644_409_933e9));
        assert_eq!(gm("Bennu"), None);
        // JPL predicts Halley's 2061 perihelion on 28 July 2061.
        assert!((jpl_halley_perihelion_jd() - 2_474_034.22).abs() < 0.01);
    }
}
