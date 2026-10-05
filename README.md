# Worldline

**A relativistic universe sandbox.** Drop black holes, neutron stars and planets into the real universe and watch general relativity play out. Every physics model is validated against theory and observation.

> **Status:** pre-alpha. The engine runs the real solar system with relativistic gravity in a 3D window. See the [roadmap](docs/ROADMAP.md).

![Worldline running the real solar system with relativistic gravity, five months after 1 January 2025](docs/images/solar-system-step-1.5.png)

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
| Kepler's third law (Mercury, Earth and Jupiter orbit periods) | T = 2π √(a³/GM) | ✅ passing, to 1 part in 100 million |
| Energy conservation over 10,000 orbits (IAS15 integrator) | at the limit of double-precision rounding | ✅ passing, error 1 × 10⁻¹⁴ |
| One year of the real solar system vs. NASA JPL | Earth within 1 part in 10,000 | ✅ passing, Earth within 0.41 km with relativity (61 km without) |
| Mercury's perihelion precession (relativistic part) | 42.9805″ per century | ✅ passing, 42.9807″ per century |
| Planets' axial tilts (IAU rotation models plus simulated orbits) | NASA Planetary Fact Sheet | ✅ passing, all within 0.05° |
| Mercury's 3:2 spin–orbit resonance | 3 spins per 2 orbits | ✅ passing, ratio 1.50000 |
| The Moon keeps one face toward Earth (libration) | ±7.9° longitude, ±6.7° latitude at most | ✅ passing, −5.6° to +5.2°, −6.6° to +6.7° |
| Light bending at the Sun's edge | 1.75″ | planned |
| Hulse–Taylor binary pulsar orbital decay | −2.40 × 10⁻¹² s/s | planned |
| Innermost stable orbit, photon sphere and shadow of a non-spinning black hole | 6, 3 and √27 GM/c² | planned |
| GW150914 final black hole | about 62 M☉, spin about 0.67 | planned |
| Chandrasekhar limit (ideal carbon-oxygen white dwarf) | 1.46 M☉ | planned |
| Tidal disruption radius | r_t ≈ R (M/m)^(1/3) | planned |

## Run it

You need [Rust](https://rustup.rs). From the repository root:

```bash
cargo run --release
```

The first build takes a few minutes. Then:
- drag to rotate,
- scroll to zoom,
- click a body to inspect it,
- double-click a body to follow it.

## Built with

Rust · wgpu · egui

## Docs

- [Scope](docs/SCOPE.md): what v1 includes and excludes, and why
- [Roadmap](docs/ROADMAP.md): milestones and steps
- [Architecture](docs/ARCHITECTURE.md): how the engine picks physics models
- [The desktop app](docs/app.md): frame loop, double-precision 3D view, camera and trails

## License

[MIT](LICENSE)
