# Worldline

**A relativistic universe sandbox.** Drop black holes, neutron stars and planets into the real universe and watch general relativity play out. Every physics model is validated against theory and observation.

> **Status:** pre-alpha. The engine runs the real solar system in a 3D window, with relativistic gravity, all 459 of its known moons, and 62 dwarf planets, asteroids and comets. See the [roadmap](docs/ROADMAP.md).

![Worldline running the real solar system with relativistic gravity, five months after 1 January 2025](docs/images/solar-system-step-1.5.png)

![Earth on 1 January 2025: clouds, blue Rayleigh haze, the Pacific in daylight and night falling over Asia](docs/images/earth-step-1b.3.png)

![Saturn in April 2032 from below the ring plane: Cassini's measured ring structure, Saturn's shadow across the rings](docs/images/saturn-2032-step-1b.3.png)

![Saturn and its moons on 1 January 2025: each moon's current orbit, all in the plane of the nearly edge-on rings](docs/images/saturn-moons-step-1b.4.png)

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
| Energy conservation over 10,000 orbits (IAS15 integrator) | at the limit of double-precision rounding | ✅ passing, error 4 × 10⁻¹⁴ |
| One year of the real solar system vs. NASA JPL | Earth within 1 part in 10,000 | ✅ passing, Earth within 0.41 km with relativity (61 km without) |
| Mercury's perihelion precession (relativistic part) | 42.9805″ per century | ✅ passing, 42.9807″ per century |
| Planets' axial tilts (IAU rotation models plus simulated orbits) | NASA Planetary Fact Sheet | ✅ passing, all within 0.05° |
| Mercury's 3:2 spin–orbit resonance | 3 spins per 2 orbits | ✅ passing, ratio 1.50000 |
| The Moon keeps one face toward Earth (libration) | ±7.9° longitude, ±6.7° latitude at most | ✅ passing, −5.6° to +5.2°, −6.6° to +6.7° |
| Where the Sun is overhead on 1 Jan 2025, 00:00 TDB | about 23.0°S, 179°W | ✅ passing, 23.00°S, 178.76°W |
| Saturn's rings edge-on to Earth | 23 March 2025 | ✅ passing, +0.73 days |
| Saturn's equinox (Sun crosses the ring plane) | 6 May 2025 | ✅ passing, +0.54 days |
| Cassini-measured ring structure vs. PDS boundaries | B ring edge 117,570 km; empty Encke Gap | ✅ passing, 117,630 km (edge oscillates ±70 km); τ = 0.000 |
| 21 major moons after 30 days vs. NASA JPL | each within its own radius | ✅ passing, 18 within 1.6 km; worst Mimas 13 km (radius 198 km) |
| Every moon in JPL's list loads | 459 | ✅ passing, 459 (1 Moon, 21 major, 437 small) |
| Small moons after 30 days vs. NASA JPL, spot checks with measured sizes | each within its own radius | 🟨 41 of 48; 7 listed residuals (Epimetheus, four Saturn moonlets, Styx, Kerberos), 3–62 km |
| Janus and Epimetheus trade orbits | January 2026 | ✅ passing, 30 January 2026 |
| Phobos and Deimos need Mars's lumpy (Tharsis) gravity | off by more than their size without it | ✅ passing, 108 and 175 km without, 0.10 and 0.28 km with |
| Io–Europa–Ganymede Laplace resonance, after nudging Io | φ librates about 180° with a period of about 2071 days | ✅ passing, swings 148°–213°, period 2045 days |
| The heaviest asteroids' pull brings Mars closer to JPL | smaller error with them | ✅ passing, Mars 0.19 → 0.11 km after a year (Earth 0.41 → 0.21 km) |
| 62 dwarf planets, asteroids and comets after one year vs. NASA JPL | within 1 part in 10,000 | ✅ passing, 61 within 0.2 km; Bennu 2,420 km (1.5 × 10⁻⁵), a listed residual |
| Halley's Comet returns | July 2061 (JPL: 28 July) | ✅ passing, 28 July 2061, 17 minutes from JPL's time |
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
- double-click a body to fly to it.

To start already flown in to a body with time stopped:

```bash
cargo run --release -- --focus Earth --paused
```

To see Jupiter with its moons (`--zoom` puts the camera that many of the planet's radii away):

```bash
cargo run --release -- --focus Jupiter --zoom 40
```

To see Saturn's rings tilted toward the Sun in 2032 (`--advance` simulates that many years before starting):

```bash
cargo run --release -- --focus Saturn --advance 7.3
```

## Built with

Rust · wgpu · egui · data from NASA JPL and NAIF · textures from Solar System Scope (CC BY 4.0). See [CREDITS.md](CREDITS.md).

## Docs

- [Scope](docs/SCOPE.md): what v1 includes and excludes, and why
- [Roadmap](docs/ROADMAP.md): milestones and steps
- [Architecture](docs/ARCHITECTURE.md): how the engine picks physics models
- [The desktop app](docs/app.md): frame loop, double-precision 3D view, camera and trails
- [Rendering globes](docs/rendering.md): GPU globes, precision across 12 orders of magnitude, lighting, atmospheres
- [Saturn's rings](docs/physics/saturn-rings.md): Cassini's measured profile, transparency, shadows
- [Moons](docs/physics/moons.md): hierarchical integration, planets' gravity fields, and how each residual against JPL was tracked down
- [Small moons](docs/physics/small-moons.md): every known moon, computed in detail where you look
- [Small bodies](docs/physics/small-bodies.md): dwarf planets, asteroids and comets, outgassing comets, and Halley's 2061 return
- [Integrators](docs/physics/integrators.md): IAS15, and how a 2029 asteroid flyby changed how it picks its steps

## License

[MIT](LICENSE)
