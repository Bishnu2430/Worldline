# Architecture

## Principles

1. **Physics never depends on graphics.** `worldline-core` is a pure library with no window and no GPU. It runs headless in tests, so every physics claim can be checked automatically.
2. **Physics uses double precision.** All physics runs on the CPU in `f64`. The GPU (Intel Iris Xe has no native `f64`) does rendering and bulk approximate work, such as SPH and ray tracing, in local coordinates.
3. **Each interaction gets the right model.** No single equation covers the universe in real time, so the engine chooses the most accurate model that is valid for each situation (see below).
4. **Every model carries its credentials:** a source paper, a declared validity range, and a validation test.

## Crates

```
crates/
  worldline-core     physics: units, bodies, integrators, gravity models, events (collisions, mergers, collapse)
  worldline-data     catalogs and real-world snapshots (JPL Horizons, notable objects)
  worldline-app      desktop app: window, 3D view, egui interface, sandbox tools   (since step 1.5)
  worldline-render   wgpu GPU rendering: ray-traced lensing, accretion disks, SPH  (milestone 3)
```

Each crate is added when its milestone starts, not before. Until milestone 3, `worldline-app` draws the 3D view itself: positions are projected relative to the camera in double precision, then drawn with egui's painter, which itself renders through wgpu. See [app.md](app.md).

## Units and frames

Everything uses SI units in `f64`. Bodies store their gravitational parameter GM instead of mass, because GM is measured about 100,000 times more precisely than G (see [newtonian-gravity.md](physics/newtonian-gravity.md)). Each gravitationally bound system is simulated in its own local frame, centered near its barycenter, to keep precision. The renderer draws everything relative to the camera, so the GPU's single precision never limits accuracy.

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
| Compact binaries | Post-Newtonian up to 3.5PN, including radiation reaction | Blanchet, *Living Reviews in Relativity* 2014; Peters 1964 |
| Near a dominant black hole (q ≪ 1) | Kerr geodesics with radiation reaction | Kerr 1963; Bardeen, Press & Teukolsky 1972 |
| Merger | Fits to numerical relativity: final mass, spin, kick, ringdown | Jiménez-Forteza et al. 2017; Campanelli et al. 2007; Berti, Cardoso & Will 2006 |
| Tides matter (r ≲ a few r_t) | SPH with self-gravity | Monaghan 2005; Price 2012 |
| White dwarfs | Degenerate electron gas (Lane–Emden) | Chandrasekhar 1931 |
| Neutron stars | TOV structure with piecewise-polytrope equations of state | Tolman 1939; Oppenheimer & Volkoff 1939; Read et al. 2009 |
| Neutron star merger outcome | Prompt-collapse threshold | Bauswein, Baumgarte & Janka 2013 |
| Small black holes | Hawking evaporation (semiclassical) | Hawking 1974, 1975; Page 1976 |
| Time integration | IAS15 adaptive integrator | Rein & Spiegel 2015 |

The app always shows which model is active for the selected body, and warns when a scenario leaves that model's validity range.
