# Architecture

## Principles

1. **Physics never depends on graphics.** `worldline-core` is a pure library with no window and no GPU. It runs headless in tests, so every physics claim can be checked automatically.
2. **Physics uses double precision.** All physics runs on the CPU in `f64`. The GPU (Intel Iris Xe has no native `f64`) does rendering and bulk approximate work, such as SPH and ray tracing, in local coordinates.
3. **Each interaction gets the right model.** No single equation covers the universe in real time, so the engine chooses the most accurate model that is valid for each situation (see below).
4. **Every model carries its credentials:** a source paper, a declared validity range, and a validation test.
5. **Detail follows focus.** Unfocused bodies are drawn simply, and their massless companions (small moons, ring particles, asteroids) are computed coarsely or only when needed. The focused body shows every fact we know. This never touches the physics of massive bodies, which always run at full accuracy: massless test particles can't pull on them.

## Crates

```
crates/
  worldline-core     physics: units, bodies, integrators, gravity models, events (collisions, mergers, collapse)
  worldline-data     catalogs and real-world snapshots (JPL Horizons, notable objects)
  worldline-app      desktop app: window, 3D view, egui interface, sandbox tools   (since step 1.5)
  worldline-render   wgpu GPU rendering: textured globes now; ray-traced lensing,
                     accretion disks and SPH in milestone 3                       (since step 1b.2)
```

Each crate is added when its milestone starts, not before. The 3D view is drawn in layers. Rings, trails, dots and labels are projected relative to the camera in double precision and drawn with egui's painter. Globes are drawn by `worldline-render` on the GPU. See [app.md](app.md) and [rendering.md](rendering.md).

## Units and frames

Everything uses SI units in `f64`. Bodies store their gravitational parameter GM instead of mass, because GM is measured about 100,000 times more precisely than G (see [newtonian-gravity.md](physics/newtonian-gravity.md)). Each gravitationally bound system is simulated in its own local frame, centered near its barycenter, to keep precision. The renderer draws everything relative to the camera, so the GPU's single precision never limits accuracy.

## Regions

The galactic center is simulated as a region of its own, 8,277 pc from the solar system, on the same clock. At that distance each one's tide on the other is below 10⁻²⁰ m/s², so neither feels the other. A region holds its bodies around a local origin; the app adds the origin when drawing. Bodies dropped near the galactic center join its region. See [physics/galactic-center.md](physics/galactic-center.md).

## Hierarchical integration

Fast inner orbits shouldn't set the pace for everything. The solar system runs as a hierarchy (`worldline-core/src/hierarchy.rs`):
- **The top level** holds the Sun, the planets, and each planetary system as one point at its barycenter. It uses relativistic gravity and its own IAS15 integrator.
- **Each moon system** holds a planet and its moons in their own barycentric frame. After each top-level step it catches up with its own IAS15, taking as many small steps as its moons need. Inside, bodies feel each other, the planet's gravity field (oblateness and, for Mars, its lumps), tidally locked moons' shapes, and the tides of the Sun and planets, interpolated across the step.

This mirrors how JPL builds its ephemerides: planetary ephemerides for the barycenters, satellite ephemerides for the moons. See [physics/moons.md](physics/moons.md).

**The sandbox changes the hierarchy at its top.**
- **Adding.** An added body joins the top level. The moon systems' tides and the followers' paths are rebuilt from the top level at every step, so they include it at once.
- **Removing.** A removed planet takes its moon system with it.
- **Restarting.** The top level's integrator restarts after every change, since its memory of past steps no longer applies.
- **The app's list.** The app rebuilds its flat list of bodies from the hierarchy after each change.
- **Limits.** The hierarchy treats each planet's moons as a system disturbed by outside tides. An added body that dives deep into a moon system breaks that assumption, and the moons' motion there becomes approximate. Collisions arrive in step 1.7.

**Small moons** (437 of them) are too light to pull on the major moons, so they don't take part in the major moons' integration. When their planet is in focus, they follow the major moons' recorded paths:
- **Regular moons** share one integrator, so the ones with mass can pull on each other.
- **Irregular moons** each get their own, at their own slow pace.

Out of focus, they ride approximate orbits that cost almost nothing. This is principle 5 at work. See [physics/small-moons.md](physics/small-moons.md).

**Dwarf planets, asteroids and comets** split by mass:
- **The 22 heaviest** (Ceres, Vesta, Eris and others whose masses JPL's DE440 publishes) join the top level, with relativistic gravity.
- **The other 40 are followers.** They are massless, so they can't pull on anything. Each has its own IAS15 and follows the top level's recorded path, the way small moons follow the major moons. A comet rounding the Sun takes many short steps without forcing them on the planets.

Followers feel Newtonian gravity from every top-level body, the Sun's relativistic term, and the outgassing or Yarkovsky push JPL fits for them. See [physics/small-bodies.md](physics/small-bodies.md).

**The belts** (28,331 real asteroids, Trojans and Kuiper belt objects) are the far end of principle 5. Each rides a fixed ellipse around the Sun from JPL's elements, outside the integration altogether: one Kepler's equation per body per frame, shared among the CPU's cores. Over a year they drift from their true paths by about 10⁻⁴ of their distance. See [physics/belts.md](physics/belts.md). The zodiacal dust is a density model drawn as a glow ([physics/zodiacal-dust.md](physics/zodiacal-dust.md)).

## Choosing a physics model

For each body or pair of bodies, the engine computes a few dimensionless numbers:

| Number | Meaning |
|---|---|
| ε = GM / (r c²) | How strong gravity is: 0 is flat space, 0.5 is an event horizon |
| v / c | How relativistic the motion is |
| q = m₂ / m₁ | Mass ratio |
| r / r_t | Distance compared with the tidal disruption radius |

It uses them to pick a model:

| Situation | Model | Source |
|---|---|---|
| Ordinary orbits | Newtonian plus first post-Newtonian (Einstein–Infeld–Hoffmann) N-body | Einstein, Infeld & Hoffmann 1938; used in JPL planetary ephemerides |
| Asteroids and comets too light to pull on the planets | Newtonian pull of the Sun and planets, plus the Sun's 1PN (Schwarzschild) term and JPL's non-gravitational forces | IERS Conventions 2010; Marsden, Sekanina & Yeomans 1973 |
| Belt asteroids, Kuiper belt objects and freed small moons (the swarm) | Massless particles: exact two-body drifts around whatever dominates each, with kicks from every other body (generalized Wisdom–Holman), steps set by error estimates | Wisdom & Holman 1991; [physics/swarm.md](physics/swarm.md) |
| Zodiacal dust | COBE DIRBE smooth-cloud density model | Kelsall et al. 1998 |
| Bodies that touch | Perfectly inelastic merger: momentum conserved, volumes add (a black hole survives and its horizon grows with its mass); contact found along each step | [physics/collisions.md](physics/collisions.md) |
| Black holes, neutron stars, white dwarfs (as point bodies) | Published masses and sizes; Kerr horizons; Buchdahl's limit tells a black hole from a star | [physics/compact-objects.md](physics/compact-objects.md); Buchdahl 1959 |
| A moon system meets an intruder | The moons join the top level once another body's tide on one passes 1/12 of the planet's pull (half the Hill radius, far away) | Domingos, Winter & Yokoyama 2006 |
| Stars around Sagittarius A\* | Its Newtonian pull plus its first relativistic (Schwarzschild) term, from published orbits | GRAVITY Collaboration 2022; Gillessen et al. 2017; [physics/galactic-center.md](physics/galactic-center.md) |
| Sunlight and its travel time | Inverse-square law from the IAU luminosity; distance over c plus the Shapiro delay | IAU 2015 B3; Shapiro 1964 |
| Solar wind | Parker spiral with the measured average wind at Earth | Parker 1958; NASA OMNI |
| Heliosphere | Boundaries measured at the Voyager crossings; Rankine half-body shape between them (visual) | Stone et al. 2005–2019; Bzowski et al. 2015 |
| Planetary magnetic fields | Measured dipoles (degree-1 Gauss coefficients) | IGRF-14; JRM09; Cao et al. 2020; Q3; O8; Anderson et al. 2012 |
| Magnetopause | Pressure balance of the dipole with the measured solar wind (Chapman–Ferraro, f = 2.44, K = 0.88); Earth's measured flaring | Mead & Beard 1964; Spreiter et al. 1966; Shue et al. 1998 |
| Moons, inside their planet's system | Newtonian N-body plus the planet's zonal and tesseral harmonics, locked moons' shapes, and outside tides | JPL satellite ephemeris models; Montenbruck & Gill 2000 |
| Compact binaries | EIH for every body, plus each pair's second-order (2PN) and radiation-reaction (2.5PN) terms, in harmonic coordinates. 3PN and 3.5PN come with step 2.3. | Blanchet, *Living Reviews in Relativity* 2024; Kupi, Amaro-Seoane & Spurzem 2006; Mikkola & Merritt 2008; Peters & Mathews 1963; [physics/post-newtonian.md](physics/post-newtonian.md) |
| Near a dominant black hole (q ≪ 1) | Kerr geodesics with radiation reaction | Kerr 1963; Bardeen, Press & Teukolsky 1972 |
| Merger | Fits to numerical relativity: final mass, spin, kick, ringdown | Jiménez-Forteza et al. 2017; Campanelli et al. 2007; Berti, Cardoso & Will 2006 |
| Tides matter (r ≲ a few r_t) | SPH with self-gravity | Monaghan 2005; Price 2012 |
| White dwarfs | Degenerate electron gas (Lane–Emden) | Chandrasekhar 1931 |
| Neutron stars | TOV structure with piecewise-polytrope equations of state | Tolman 1939; Oppenheimer & Volkoff 1939; Read et al. 2009 |
| Neutron star merger outcome | Prompt-collapse threshold | Bauswein, Baumgarte & Janka 2013 |
| Small black holes | Hawking evaporation (semiclassical) | Hawking 1974, 1975; Page 1976 |
| Time integration | IAS15 adaptive integrator, with the timescale step criterion | Rein & Spiegel 2015; Pham, Rein & Spiegel 2024 |

The app always shows which model is active for the selected body, and warns when a scenario leaves that model's validity range.

**First version (step 1.7).** The inspector names the model computing the selected body. It shows ε and v/c relative to what pulls on the body hardest, and grades them for the post-Newtonian order the model keeps:
- **Within range** while what it leaves out (relative size max(ε, v²/c²)ⁿ⁺¹ for a model keeping order n) is below IAS15's 10⁻⁹ tolerance.
- **Approximate** while it stays under 1%.
- **Beyond** past that.

The top level keeps second order since step 2.2 (within range up to ε = 10⁻³); everything else is graded for first order (up to 3 × 10⁻⁵).

It also flags small moons on mean orbits, and massive bodies inside a moon system (where collisions with moons aren't detected yet). See [physics/collisions.md](physics/collisions.md).
