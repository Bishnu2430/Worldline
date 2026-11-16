//! The galaxy's stars as a swarm (step 7.2): the model's stars, drawn in
//! equilibrium by `examples/galaxy_stars.rs`. See
//! `docs/physics/galaxy-stars.md`.

use worldline_core::DVec3;
use worldline_core::galaxy_stars::Star;
use worldline_core::milky_way::KILOPARSEC;

/// The model's stars, galactocentric, as drawn.
pub fn galaxy_stars() -> Vec<Star> {
    include_str!("../data/galaxy-stars.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .skip(1)
        .map(|line| {
            let f: Vec<f64> = line
                .split(',')
                .map(|x| x.parse().expect("a drawn number"))
                .collect();
            Star {
                position: DVec3::new(f[1], f[2], f[3]) * KILOPARSEC,
                velocity: DVec3::new(f[4], f[5], f[6]) * 1e3,
                population: f[0] as u8,
            }
        })
        .collect()
}
