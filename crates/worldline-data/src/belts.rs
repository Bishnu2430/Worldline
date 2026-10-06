//! The asteroid belt, Jupiter's Trojans and the Kuiper belt: tens of
//! thousands of real orbits from JPL's Small-Body Database.
//!
//! `tools/fetch_belts.py` regenerates the data. See `docs/physics/belts.md`.

use worldline_core::constants::{AU, DAY};
use worldline_core::orbit::Elements;

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
