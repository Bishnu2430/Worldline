//! Draws the galaxy's stars in equilibrium in McMillan's (2017) model and
//! writes them to `crates/worldline-data/data/galaxy-stars.csv` (step 7.2).
//!
//! `cargo run --release -p worldline-data --example galaxy_stars [count] [seed]`
//!
//! It takes a few minutes on all cores: every star is chosen from three
//! orbits, each followed for over a billion years. See
//! `docs/physics/galaxy-stars.md`.

use std::fmt::Write;
use std::time::Instant;

use worldline_core::galaxy_stars::{Recipe, Sampler, equilibrium, stellar_components};
use worldline_core::milky_way::{KILOPARSEC, mcmillan_2017, milky_way};

fn main() {
    let mut args = std::env::args().skip(1);
    let count: usize = args.next().and_then(|a| a.parse().ok()).unwrap_or(30_000);
    let seed: u64 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1);
    let clock = Instant::now();
    let components = stellar_components(&mcmillan_2017());
    let sampler = Sampler::new(milky_way(), components.clone());
    let recipe = Recipe::standard();
    let (stars, repeats) = equilibrium(&sampler, count, seed, recipe);
    let year = 365.25 * 86_400.0;
    let mut out = String::new();
    let _ = writeln!(
        out,
        "# Worldline: {count} stars of the Milky Way, written by `cargo run --release -p \
         worldline-data --example galaxy_stars {count} {seed}`."
    );
    let _ = writeln!(
        out,
        "# Drawn from the stellar parts of McMillan's (2017, MNRAS 465, 76) mass model ({}), \
         each in proportion to its mass, in equilibrium in its potential by Schwarzschild's \
         method: {} orbits a star from the Jeans equations, let mix for {:.0} million years, \
         weighted by where they spend the next {:.0} million, each star at a random moment of \
         its orbit. Not measured stars: each stands for {:.3e} Suns. See \
         docs/physics/galaxy-stars.md.",
        components
            .iter()
            .map(|c| c.name())
            .collect::<Vec<_>>()
            .join(", "),
        recipe.orbits_per_star,
        recipe.settle / year / 1e6,
        recipe.window / year / 1e6,
        sampler.masses().iter().sum::<f64>() / count as f64 / worldline_core::constants::SOLAR_MASS
    );
    let _ = writeln!(
        out,
        "# Galactocentric: x from the Sun toward the center, z toward the north galactic pole; \
         x, y, z in kpc, vx, vy, vz in km/s. population: 0 thin disk, 1 thick disk, 2 bulge."
    );
    let _ = writeln!(out, "population,x,y,z,vx,vy,vz");
    for s in &stars {
        let (x, v) = (s.position / KILOPARSEC, s.velocity / 1e3);
        let _ = writeln!(
            out,
            "{},{:.7},{:.7},{:.7},{:.5},{:.5},{:.5}",
            s.population, x.x, x.y, x.z, v.x, v.y, v.z
        );
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/galaxy-stars.csv");
    std::fs::write(&path, out).expect("the data folder is writable");
    println!(
        "wrote {} stars to {} in {:.0} s; {repeats} drawn from an orbit already used",
        stars.len(),
        path.display(),
        clock.elapsed().as_secs_f64()
    );
}
