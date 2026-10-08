# Black holes, neutron stars, white dwarfs and the notable-objects catalog

**Code:** `crates/worldline-core/src/compact.rs` (horizons, limits, redshift), `crates/worldline-core/src/collision.rs` and `crates/worldline-core/src/hierarchy.rs` (what happens when they meet things), `crates/worldline-data/src/notable.rs` (the catalog), `crates/worldline-app/src/catalogue.rs` (adding them)
**Data:** `crates/worldline-data/data/notable-objects.csv` and `binary-orbits.csv`, written by `tools/write_notable_objects.py`
**Tests:** `crates/worldline-data/tests/validation_notable_objects.rs`, `crates/worldline-core/tests/validation_collisions.rs`, plus unit tests

## The catalog

Twenty real objects, each with its mass, its size and where it is, copied digit for digit from the paper that measured them (each row names its source):

| Kind | Objects | Mass |
|---|---|---|
| Black holes | Sagittarius A\* | 4.297 ± 0.012 million Suns (GRAVITY Collaboration 2022) |
| | M87\* | 6.5 ± 0.7 billion Suns (Event Horizon Telescope 2019) |
| | TON 618 | 10^10.82 ≈ 66 billion Suns, uncertain by a factor of two (Shemmer et al. 2004) |
| | Cygnus X-1, Gaia BH1, Gaia BH3 | 21.2, 9.62 and 32.70 Suns |
| | GW150914's two black holes and the one they made | 35.6, 30.6 → 63.1 Suns, spin 0.69 (LIGO/Virgo GWTC-1) |
| Neutron stars | The Hulse–Taylor pulsar PSR B1913+16 and its companion | 1.438 and 1.390 ± 0.001 Suns (Weisberg & Huang 2016) |
| | PSR J0740+6620, PSR J0030+0451 | 2.08 and 1.34 Suns, with radii measured by NICER |
| | GW170817's two neutron stars | 1.46 and 1.27 Suns |
| White dwarf | Sirius B | 1.018 Suns, 0.00803 Sun radii |
| Stars | Sirius A, Alpha Centauri A and B, Proxima Centauri | 2.063, 1.1055, 0.9373 and 0.1221 Suns |

Uncertainties are as published: one standard deviation, except the gravitational-wave catalog's 90% intervals.

**How each size is known.** The catalog says, for every object:
- **Measured:** stars by interferometry, Sirius B from its brightness and distance, neutron stars by NICER's X-ray timing.
- **Estimated:** Proxima Centauri's radius comes from a relation between brightness and size, not a direct measurement.
- **Assumed:** the Hulse–Taylor neutron stars' radii have never been measured. They take the 11.9 km measured for GW170817's neutron stars, and the app says so.
- **Horizon:** a black hole's size is its event horizon (below). Where its spin isn't measured, it is taken as not spinning, which gives the largest horizon its mass allows.

**Too far for a distance.** TON 618 is listed by its redshift (2.219), since turning that into a distance depends on a cosmological model.

## The physics

**Event horizons.** A black hole of mass M that doesn't spin has its horizon at the Schwarzschild radius, 2GM/c². Spinning (Kerr), with dimensionless spin a from 0 to 1, it shrinks to r₊ = GM/c² (1 + √(1 − a²)), down to GM/c² at the fastest spin.
- **Sagittarius A\*:** 0.0848 AU, well inside Mercury's orbit.
- **M87\*:** 128 AU.
- **TON 618:** 1,304 AU, big enough to hold the solar system out past Sedna.
- **Cygnus X-1:** spins at least 0.9985, so its horizon is at most 1.0548 GM/c², 33 km.

**What counts as a black hole.** No static star can be smaller than 9/4 GM/c² (Buchdahl 1959): anything more compact must collapse. So a body is a black hole exactly when its radius is under that limit. The engine uses this test, so it holds for black holes made by merging too.

**Gravitational redshift.** Light climbing out of a body's gravity loses energy: from the surface of a static body of radius R, 1 + z = 1/√(1 − 2GM/(Rc²)) (the Schwarzschild solution). The inspector shows it for stars, white dwarfs and neutron stars.

**The shadow.** A black hole seen from afar is a dark disk of radius √27 GM/c², 2.6 times its horizon, because light passing closer falls in (Synge 1966). The app draws black holes as that disk, edged by a thin ring so they show against the sky. How light bends around the shadow, and the bright ring it makes, comes with the ray tracer in step 3.1.

## When they meet other bodies

**A black hole survives any collision.** Nothing comes back out of a horizon. When a black hole touches anything, even a heavier star, the result is a black hole:
- **Mass:** the masses add.
- **Momentum:** kept exactly, as in every merger (see [collisions.md](collisions.md)).
- **Horizon:** grows in proportion to the mass, as it does at a fixed spin.

Two simplifications are labeled. The merger is instant: in reality, a black hole much lighter than the star it enters takes time to consume it. And two black holes merging radiate a few percent of their mass as gravitational waves; that comes with step 2.4's numerical-relativity fits.

**Swallowing the Sun.** Whatever absorbs the Sun (a black hole, or a heavier star) takes its place as body 0, which the comets' and asteroids' relativistic term is measured from. Without the Sun there is no sunlight, solar wind, heliosphere or magnetopause, and the app stops showing them. If a black hole took its place, the planets go dark.

**Dropped inside a horizon.** A body placed so that it already overlaps another merges with it at once, before the next step. A black hole dropped into the solar system swallows everything inside its horizon immediately: TON 618 dropped 5 AU from the Sun leaves nothing else within 1,304 AU.

**Moons torn from their planet.** A planet's major moons are computed in the planet's own frame, feeling everything else's tides. When another body comes so close that its tide on a moon passes 1/12 of the planet's pull, the moons are freed: they join the top level, where everything pulls on everything, and collisions with them are detected. For a distant body that threshold sits at half the planet's Hill radius, beyond which no prograde moon stays bound for long (Domingos, Winter & Yokoyama 2006). In the real solar system the largest such ratio is 1 × 10⁻⁴ (the Sun on Saturn's Iapetus), so nothing changes there. The planet's small moons are freed too, as live particles that keep their names (see [swarm.md](swarm.md)).

**Launching near a black hole.** No circular orbit exists closer than 6 G(M + m)/c², the innermost stable circular orbit, so a body placed closer starts at rest and falls in. Launch speeds are capped at half the speed of light, already far beyond what first-order relativistic gravity covers. The model indicator turns red there.

## Validation (`cargo test`)

`cargo test --release -p worldline-data --test validation_notable_objects -- --nocapture`:

| Check | Expected | Result |
|---|---|---|
| Sagittarius A\*, TON 618, M87\* and PSR B1913+16 load with their published masses | 4.297 × 10⁶, 10^10.82, 6.5 × 10⁹, 1.438 and 1.390 Suns | exactly |
| Every black hole lies inside Buchdahl's limit and everything else outside it | 2 GM/c² or less; more than 9/4 GM/c² | all 20 (Sirius B 3,716 GM/c², the neutron stars 4.0–6.4) |
| Sirius B's gravitational redshift, from its orbital mass and measured radius | 80.65 ± 0.77 km/s measured by Hubble (Joyce et al. 2018) | 80.70 ± 1.41 km/s |
| Sagittarius A\*'s angular size GM/(Dc²) from GRAVITY's mass and distance | 4.8 +1.4/−0.7 μas in the Event Horizon Telescope's image (2022) | 5.124 μas (EHT's own figure for GRAVITY: 5.125) |
| M87\*'s angular size from its mass and distance | 3.8 ± 0.4 μas (EHT 2019) | 3.819 μas |
| The Hulse–Taylor orbit's turning, simulated with Worldline's relativistic gravity | 4.226585 ± 0.000004°/yr measured (Weisberg & Huang 2016); within 4.7 × 10⁻⁴, what rounding the masses to 0.001 Suns allows | 4.226561°/yr, off by 6 × 10⁻⁶ |

**The Hulse–Taylor orbit's size.** Kepler's third law gives the orbit for the measured 7.75-hour period, but relativity lengthens the period of an orbit started that way, by 1.5 × 10⁻⁴. What was measured is the period, so the test resizes the orbit until the simulated period matches. Left unmatched, the turning comes out 1.7 × 10⁻⁴ slow. That offset grows with orbital speed squared, as the next relativistic order should.

`cargo test --release -p worldline-core --test validation_collisions -- --nocapture`: a black hole of 10 Suns falls into the Sun from 0.1 AU. It hits at 2,000 km/s after 14.7 hours, as energy conservation predicts, and becomes a black hole of 11 Suns with a 32.49 km horizon in the Sun's place. Momentum changes by 4 × 10⁻¹⁷ of what the bodies carry into the impact.

## Not handled yet

- **Motion close to a black hole** needs exact general relativity (step 2.5). The post-Newtonian equations used everywhere else lose accuracy within a few horizon radii, and the model indicator says so.
- **Binaries spiral in:** since step 2.2, close pairs lose energy to gravitational waves, and the Hulse–Taylor orbit shrinks as measured (see [post-newtonian.md](post-newtonian.md)).
- **Small moons of a planet not in focus** ride their mean orbits until an added body could tear them away; then they are freed as live particles (see [swarm.md](swarm.md)). The belts have been live since step 2.1b.
- **Neutron star and white dwarf interiors**, collapse and mergers: steps 2.6 and 2.7.
- **Where the real objects are:** Sagittarius A\* is at its real place, with the stars orbiting it, since step 2.1c (see [galactic-center.md](galactic-center.md)). The other catalog objects are dropped where you like.
