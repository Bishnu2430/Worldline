# Credits

Worldline is built on published data and imagery. Thank you to everyone who makes it freely available.

## Data

| What | Source | License |
|---|---|---|
| Positions and velocities of the Sun, planets, Moon and Pluto | NASA JPL Horizons, ephemeris DE441 | Public domain (U.S. government work) |
| Gravitational parameters (GM) | JPL DE440 (Park et al. 2021), via NASA NAIF `gm_de440.tpc` | Public domain (U.S. government work) |
| Positions and velocities of the major moons | NASA JPL Horizons, satellite ephemerides MAR099, JUP365, SAT441, URA184, NEP098 and PLU060 | Public domain (U.S. government work) |
| Every other known moon: mean orbital elements and measured sizes and masses | JPL Solar System Dynamics, Planetary Satellite Mean Elements and Physical Parameters (ssd.jpl.nasa.gov/sats) | Public domain (U.S. government work) |
| Positions and velocities of the small moons | NASA JPL Horizons, the satellite ephemerides each moon comes from (JUP347–JUP365, SAT415–SAT459, URA184, NEP098, NEP104, PLU060) | Public domain (U.S. government work) |
| Moon-system constants: GMs, planets' gravity harmonics (J2–J6, Mars's C_nm and S_nm), moons' shapes, Saturn's ring masses | The JPL satellite ephemerides' own constants, from the NASA NAIF comment files `mar099.cmt`, `jup365.cmt`, `sat441.cmt`, `ura184_part-1.cmt`, `nep098_part-1.cmt`, `plu060.cmt` | Public domain (U.S. government work) |
| Positions, velocities and non-gravitational force models of the dwarf planets, asteroids, comets and interstellar objects | NASA JPL Horizons, each body's JPL small-body orbit solution (Bennu: Farnocchia et al. 2021, solution JPL#118) | Public domain (U.S. government work) |
| Masses of Ceres, the heaviest asteroids, Eros, Didymos, Eris, Haumea, Quaoar and Orcus | JPL DE440 (Park et al. 2021), via NASA NAIF `gm_de440.tpc` | Public domain (U.S. government work) |
| Orbits of the asteroid belt, Jupiter's Trojans and the Kuiper belt (28,331 bodies) | NASA JPL Small-Body Database Query API (ssd-api.jpl.nasa.gov/sbdb_query.api) | Public domain (U.S. government work) |
| Zodiacal dust model | Kelsall et al. (1998), *ApJ* 508, 44, from COBE DIRBE; full-precision parameters as reproduced by ZodiPy (San et al. 2022, *A&A* 666, A107) | Published scientific values |
| Zodiacal light brightening toward the Sun (validation) | Helios 1 and 2, Leinert et al. (1981), *A&A* 103, 177 | Published scientific values |
| Hourly solar wind at Earth, 2025 | NASA OMNI (King & Papitashvili 2005, *J. Geophys. Res.* 110, A02104), served by CDPP/AMDA (IRAP, France) through HAPI | Public domain (U.S. government work); AMDA service |
| Voyager 1 and 2 positions at their heliosphere crossings | NASA JPL Horizons | Public domain (U.S. government work) |
| Dates and distances of the Voyager crossings | Stone et al. 2005 (*Science* 309, 2017), 2008 (*Nature* 454, 71), 2013 (*Science* 341, 150), 2019 (*Nature Astronomy* 3, 1013) | Published scientific values |
| Direction of the interstellar wind | IBEX: Bzowski et al. (2015), *ApJS* 220, 28 | Published scientific values |
| Total solar irradiance (validation) | SORCE TIM: Kopp & Lean (2011), *Geophys. Res. Lett.* 38, L01706 | Published scientific values |
| Solar luminosity | IAU 2015 Resolution B3 (nominal value) | Published values |
| Rotation models (spin axes and rates), including 14 asteroids and comets | IAU WGCCRE (Archinal et al. 2018), via NASA NAIF `pck00011.tpc` | Public domain (U.S. government work) |
| Radii and shapes | IAU WGCCRE (Archinal et al. 2018), via NASA NAIF `pck00011.tpc`; IAU 2015 Resolution B3 for the Sun | Public domain (U.S. government work) / published values |
| Saturn's ring profile | Cassini Radio Science occultation Rev 7 (2005), NASA PDS Ring-Moon Systems Node, data set CO-SR-RSS-4/5-OCC-V2.0 | Public domain (U.S. government work) |
| Saturn's ring features | NASA PDS Ring-Moon Systems Node, "Vital Statistics for Saturn's Rings" | Public domain (U.S. government work) |
| Earth's Rayleigh scattering coefficients | Bruneton & Neyret (2008), *Computer Graphics Forum* 27(4) | Published scientific values |
| Corona brightness model | Baumbach (1937), as given in *Allen's Astrophysical Quantities* | Published scientific values |

## Imagery

Planet, Moon and Sun surface maps and Earth's cloud map in `crates/worldline-app/assets/textures/` are from **[Solar System Scope](https://www.solarsystemscope.com/textures/)** (INOVE), based on NASA imagery and elevation data. They are distributed under the **[Creative Commons Attribution 4.0 International license (CC BY 4.0)](https://creativecommons.org/licenses/by/4.0/)** and used unmodified.

Maps that Solar System Scope labels "fictional" (Ceres, Eris, Haumea, Makemake) are not used. Pluto is drawn in a flat color until a published global map is added.

## Software

Rust, [wgpu](https://wgpu.rs), [egui/eframe](https://github.com/emilk/egui), [glam](https://github.com/bitshifter/glam-rs) and the [image](https://github.com/image-rs/image) crate, under their respective MIT/Apache-2.0 licenses.
