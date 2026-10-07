//! The asteroid belt, Jupiter's Trojans and the Kuiper belt: tens of
//! thousands of real orbits from JPL's Small-Body Database.
//!
//! `tools/fetch_belts.py` regenerates the data. See `docs/physics/belts.md`.

use worldline_core::DVec3;
use worldline_core::constants::{AU, DAY, GM_SUN};
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::hierarchy::Hierarchy;
use worldline_core::orbit::{Elements, KeplerOrbit};
use worldline_core::swarm::{Particle, Swarm};

/// Which belt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BeltKind {
    /// The main asteroid belt between Mars and Jupiter, with the Hildas
    /// and Cybeles at its outer edge.
    AsteroidBelt,
    /// The asteroids sharing Jupiter's orbit, 60° ahead of and behind it.
    JupiterTrojans,
    /// Everything with an orbit beyond Neptune's.
    KuiperBelt,
}

impl BeltKind {
    /// Its name, for display.
    pub fn name(self) -> &'static str {
        match self {
            BeltKind::AsteroidBelt => "Asteroid belt",
            BeltKind::JupiterTrojans => "Jupiter's Trojans",
            BeltKind::KuiperBelt => "Kuiper belt",
        }
    }
}

/// One belt's orbits.
#[derive(Debug, Clone, PartialEq)]
pub struct Belt {
    /// Which belt it is.
    pub kind: BeltKind,
    /// Each body's osculating heliocentric orbit, with its epoch in
    /// simulation time (seconds after the 2025-01-01 snapshot).
    pub orbits: Vec<Elements>,
    /// Each body's designation (its number, if it has one).
    pub designations: Vec<String>,
}

fn designations(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| line.split(',').next().unwrap_or_default().to_string())
        .collect()
}

fn parse(text: &str, start_jd: f64) -> Vec<Elements> {
    text.lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<f64> = line
                .split(',')
                .skip(1)
                .map(|v| v.parse().expect("published element"))
                .collect();
            Elements {
                a: f[0] * AU,
                e: f[1],
                inclination: f[2].to_radians(),
                node: f[3].to_radians(),
                periapsis: f[4].to_radians(),
                mean_anomaly: f[5].to_radians(),
                epoch: (f[6] - start_jd) * DAY,
            }
        })
        .collect()
}

/// The three belts.
pub fn belts() -> Vec<Belt> {
    let start = crate::solar_system().epoch_jd_tdb;
    [
        (
            BeltKind::AsteroidBelt,
            include_str!("../data/asteroid-belt.csv"),
        ),
        (
            BeltKind::JupiterTrojans,
            include_str!("../data/jupiter-trojans.csv"),
        ),
        (
            BeltKind::KuiperBelt,
            include_str!("../data/kuiper-belt.csv"),
        ),
    ]
    .into_iter()
    .map(|(kind, text)| Belt {
        kind,
        orbits: parse(text, start),
        designations: designations(text),
    })
    .collect()
}

/// The belts' bodies as swarm particles at the snapshot (2025-01-01 00:00
/// TDB), in the top level's frame, labeled 0, 1, 2, … in the order of
/// [`belts`].
///
/// JPL gives their orbits at an epoch of its own, 2026-06-09 for all but
/// a few dozen. Carried back to the snapshot on fixed ellipses, they would
/// start off by the planets' pull over those 524 days, about 1/1,000 of
/// their distance. Instead the Sun, planets and heaviest asteroids are run
/// forward to that epoch, recording their paths, and the bodies are run
/// back along them with the swarm's own scheme. The few with other epochs
/// are carried back on their ellipses.
pub fn belt_particles() -> Vec<Particle> {
    let belts = belts();
    let all: Vec<&Elements> = belts.iter().flat_map(|b| &b.orbits).collect();
    // The common epoch: the one most orbits share.
    let mut epochs: Vec<f64> = all.iter().map(|o| o.epoch).collect();
    epochs.sort_by(f64::total_cmp);
    let common = epochs[epochs.len() / 2];
    let mut top = crate::solar_system().system();
    top.bodies.extend(
        crate::small_bodies()
            .iter()
            .filter(|b| b.gm.is_some())
            .map(|b| b.body()),
    );
    let sun_now = (top.bodies[0].position, top.bodies[0].velocity);
    let mut hierarchy = Hierarchy::new(top, Box::new(EinsteinInfeldHoffmann), Vec::new());
    let mut field = hierarchy.record_paths(common);
    let sun_then = &hierarchy.top.bodies[0];
    let (shared, own): (Vec<_>, Vec<_>) =
        all.iter().enumerate().partition(|(_, o)| o.epoch == common);
    let mut swarm = Swarm::new(common);
    swarm.add(shared.iter().map(|(k, o)| {
        let (r, v) = KeplerOrbit::new(o, GM_SUN).state_at(common);
        Particle {
            position: sun_then.position + r,
            velocity: sun_then.velocity + v,
            id: *k as u32,
        }
    }));
    swarm.advance(&mut field, 0.0);
    let mut particles = swarm.particles;
    particles.extend(own.iter().map(|(k, o)| {
        let (r, v) = KeplerOrbit::new(o, GM_SUN).state_at(0.0);
        Particle {
            position: sun_now.0 + r,
            velocity: sun_now.1 + v,
            id: *k as u32,
        }
    }));
    particles.sort_by_key(|p| p.id);
    particles
}

/// A belt body whose path NASA JPL Horizons gives, to check the swarm.
#[derive(Debug, Clone, PartialEq)]
pub struct BeltSample {
    /// Its designation (a number, if it has one).
    pub designation: String,
    /// Which belt it is in.
    pub kind: BeltKind,
    /// Position (m) and velocity (m/s) at the snapshot, 2025-01-01
    /// 00:00 TDB, relative to the solar system barycenter.
    pub start: (DVec3, DVec3),
    /// The same, exactly one Julian year later (2026-01-01 06:00 TDB).
    pub one_year_on: (DVec3, DVec3),
}

/// Two dozen of the belts' bodies, spread through each belt, from JPL.
pub fn belt_samples() -> Vec<BeltSample> {
    include_str!("../data/belt-samples.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let n: Vec<f64> = f[2..]
                .iter()
                .map(|v| v.trim().parse::<f64>().expect("JPL's number") * 1e3)
                .collect();
            let state = |k: usize| {
                (
                    DVec3::new(n[k], n[k + 1], n[k + 2]),
                    DVec3::new(n[k + 3], n[k + 4], n[k + 5]),
                )
            };
            BeltSample {
                designation: f[0].to_string(),
                kind: match f[1] {
                    "asteroid belt" => BeltKind::AsteroidBelt,
                    "jupiter trojans" => BeltKind::JupiterTrojans,
                    _ => BeltKind::KuiperBelt,
                },
                start: state(0),
                one_year_on: state(6),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn belts_load() {
        let belts = belts();
        let counts: Vec<usize> = belts.iter().map(|b| b.orbits.len()).collect();
        assert_eq!(counts, [19_971, 4_509, 3_851]);
        // The first Trojan in the file is 588 Achilles, the first one found
        // (1906), with its elements at JD 2461200.5.
        let achilles = belts[1].orbits[0];
        assert_eq!(achilles.a, 5.215_906_988_352_895 * AU);
        assert_eq!(achilles.epoch, (2_461_200.5 - 2_460_676.5) * DAY);
    }
}
