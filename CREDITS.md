# Credits

Worldline is built on published data and imagery. Thank you to everyone who makes it freely available.

## Data

| What | Source | License |
|---|---|---|
| Positions and velocities of the Sun, planets, Moon and Pluto | NASA JPL Horizons, ephemeris DE441 | Public domain (U.S. government work) |
| Gravitational parameters (GM) | JPL DE440 (Park et al. 2021), via NASA NAIF `gm_de440.tpc` | Public domain (U.S. government work) |
| Rotation models (spin axes and rates) | IAU WGCCRE (Archinal et al. 2018), via NASA NAIF `pck00011.tpc` | Public domain (U.S. government work) |
| Radii | IAU WGCCRE (Archinal et al. 2018); IAU 2015 Resolution B3 for the Sun | Published scientific values |

## Imagery

Planet, Moon and Sun surface maps in `crates/worldline-app/assets/textures/` are from **[Solar System Scope](https://www.solarsystemscope.com/textures/)** (INOVE), based on NASA imagery and elevation data. They are distributed under the **[Creative Commons Attribution 4.0 International license (CC BY 4.0)](https://creativecommons.org/licenses/by/4.0/)** and used unmodified.

Maps that Solar System Scope labels "fictional" (Ceres, Eris, Haumea, Makemake) are not used. Pluto is drawn in a flat color until a published global map is added.

## Software

Rust, [wgpu](https://wgpu.rs), [egui/eframe](https://github.com/emilk/egui), [glam](https://github.com/bitshifter/glam-rs) and the [image](https://github.com/image-rs/image) crate, under their respective MIT/Apache-2.0 licenses.
