# Scope

Decided 2026-10-04.

## Goal

A portfolio-grade desktop sandbox where you place real or invented celestial bodies and watch them evolve according to established physics. It should be accurate to the level of published theory, and prove it with automated checks against theory and observation.

## Decisions

| Question | Decision |
|---|---|
| Language | Rust (wgpu for the GPU, egui for the interface) |
| Platform | Desktop, Windows first |
| v1 finish line | The "Einstein Release" (below) |
| Quantum physics in v1 | Quantum-driven astrophysics only. The quantum lab comes in v2. |
| Look and feel | Cinematic by default, with toggleable data overlays |
| Owner's time | Under 5 hours a week, so steps are small and each one is checkable in about an hour |
| Target hardware | Intel Iris Xe integrated GPU. This sets particle budgets, and the GPU has no double precision. |

## What "accurate" means here

1. Every physics model is a published, peer-reviewed model, cited in `docs/physics/`.
2. Every model declares the range where it is valid. The app shows which model is running and warns when a scenario leaves that range.
3. Every model has at least one automated validation test against an analytic result or a real observation.
4. Approximations are labeled as approximations in the app, never presented as exact.
5. Where physics itself is unknown (inside event horizons, the deep interior of neutron stars, quantum gravity), the app says so instead of inventing an answer.

## In v1: the Einstein Release

**Engine and data**
- A double-precision physics core with a high-accuracy adaptive integrator and per-system local frames.
- The real solar system from NASA JPL Horizons.
- A catalog of notable objects: Sagittarius A\*, TON 618, M87\*, the Hulse–Taylor binary pulsar (PSR B1913+16), and the source parameters of GW150914 and GW170817.
- Saving and loading scenarios.

**Gravity**
- N-body gravity with Newtonian physics plus first-order relativistic corrections (Einstein–Infeld–Hoffmann).
- Higher-order post-Newtonian dynamics for compact binaries, including energy lost to gravitational waves.
- Exact Kerr-spacetime motion for bodies near a dominant black hole, including extreme mass ratios such as TON 618 with Sagittarius A\*.
- Black hole and neutron star mergers using fits to numerical-relativity simulations: final mass, spin, recoil kick and ringdown.
- Collisions between point bodies merge them, conserving mass, momentum and angular momentum.

**Gravitational waves**
- A waveform plot for a chosen observer, and an audible chirp.

**Quantum-driven astrophysics**
- White dwarfs: when the Chandrasekhar limit is exceeded, the star collapses or explodes.
- Neutron stars: structure solved from published equations of state (user-selectable). Collapse at maximum mass, and prompt versus delayed collapse in mergers.
- Hawking radiation: small black holes evaporate. This is labeled as theoretical, since it has never been observed.

**Seeing spacetime**
- GPU ray-traced black hole lensing and shadows, and the appearance of an accretion disk. The disk is visual only in v1, and labeled that way.
- Overlays: the active physics model, clock rates (time dilation), tidal fields, an embedding diagram, gravitational-wave ripples, orbit trails and live plots.

**Deforming matter**
- Planets and stars become self-gravitating fluids (SPH) when tides matter: tidal disruption, spaghettification and the Roche limit.

**Sandbox**
- Add, select, drag, launch, edit and delete bodies.
- Time controls from real time up to millions of years per second.

## v2: committed, starts once v1 is established

All of these will be built. They are ordered after v1 because they depend on its engine, not because they are optional.

- Stellar evolution (stars aging, supernovae of massive stars)
- A Milky Way built from Gaia data, a dark matter halo, galaxy-scale simulation, and a "compute in the background, then replay" mode
- TON 618 dropped into the full galaxy (v1 covers the two-black-hole merger only)
- Fluid collisions between planets with realistic rock and iron equations of state
- Accretion disk physics
- Quantum lab mode (Schrödinger equation)

## Never in scope

- Numerical relativity from first principles. It takes weeks on supercomputers, so Worldline uses fits to published results instead.
- Physics nobody knows: what happens at a singularity, and quantum gravity. The app labels these regions instead of simulating them.
