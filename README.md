# Worldline

**A relativistic universe sandbox.** Drop black holes, neutron stars and planets into the real universe and watch general relativity play out. Every physics model is validated against theory and observation.

> **Status:** pre-alpha. Planning is done and engine work is starting. See the [roadmap](docs/ROADMAP.md).

## What v1 will do

- Start from the real solar system (NASA JPL data) and a catalog of notable objects: Sagittarius A\*, TON 618, M87\* and the Hulse–Taylor binary pulsar.
- Add, drag and launch bodies, with time running anywhere from real time to millions of years per second.
- Simulate relativistic gravity: orbits precess, binaries spiral together by emitting gravitational waves, and black holes merge.
- Play the gravitational-wave chirp and show light bending around black holes.
- Show planets and stars torn apart by tides.
- Include quantum physics where it decides the outcome: white dwarf and neutron star mass limits, and Hawking evaporation.
- Always show which physics model is running, and warn when a scenario goes beyond known physics.

## Validated against reality

| Test | Expected | Status |
|---|---|---|
| Mercury's perihelion precession (relativistic part) | 43″ per century | planned |
| Light bending at the Sun's edge | 1.75″ | planned |
| Hulse–Taylor binary pulsar orbital decay | −2.40 × 10⁻¹² s/s | planned |
| Innermost stable orbit, photon sphere and shadow of a non-spinning black hole | 6, 3 and √27 GM/c² | planned |
| GW150914 final black hole | about 62 M☉, spin about 0.67 | planned |
| Chandrasekhar limit (ideal carbon-oxygen white dwarf) | 1.46 M☉ | planned |
| Tidal disruption radius | r_t ≈ R (M/m)^(1/3) | planned |

## Built with

Rust · wgpu · egui

## Docs

- [Scope](docs/SCOPE.md): what v1 includes and excludes, and why
- [Roadmap](docs/ROADMAP.md): milestones and steps
- [Architecture](docs/ARCHITECTURE.md): how the engine picks physics models

## License

[MIT](LICENSE)
