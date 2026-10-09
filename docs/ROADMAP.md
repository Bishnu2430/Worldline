# Roadmap: v1 "Einstein Release"

Every step ends with something you can run and a number or behavior you can check. Steps are sized so checking one takes about an hour.

Status: ⬜ not started · 🟨 in progress · ✅ done

At under 5 hours a week, v1 is roughly 6–12 months away. We'll recalibrate after M1.

## M1: Engine core and the real solar system

| # | Step | How you verify it | Status |
|---|---|---|---|
| 1.1 | Rust workspace, physical constants, body types, Newtonian gravity, first integrator | `cargo test`: a two-body orbit obeys Kepler's third law and conserves energy | ✅ |
| 1.2 | IAS15 high-accuracy adaptive integrator | Energy error stays near the limit of double precision over 10,000 orbits | ✅ |
| 1.3 | Real solar system loaded from a NASA JPL Horizons snapshot | After one simulated year, Earth's position matches JPL's to better than 1 part in 10,000 | ✅ |
| 1.4 | Einstein–Infeld–Hoffmann relativistic gravity | Mercury's perihelion precesses 43″ per century beyond the Newtonian value | ✅ |
| 1.5 | First window: 3D view, bodies, orbit trails, camera, time controls | `cargo run` shows the solar system moving | ✅ |
| 1.6 | Sandbox tools: select, inspect, add, drag-launch, delete, save and load | Drop a Jupiter-mass planet into the inner solar system and watch the orbits get disturbed | ✅ |
| 1.7 | Collisions (merge, conserving momentum) and a first version of the physics-model indicator | Two bodies collide; total momentum before and after matches | ✅ |

## M1b: The full solar system

Added 2026-10-05, before steps 1.6 and 1.7. Detail follows focus: see [SCOPE.md](SCOPE.md#added-to-v1-m1b-the-full-solar-system-decided-2026-10-05).

| # | Step | How you verify it | Status |
|---|---|---|---|
| 1b.1 | Real rotation: IAU spin axes and rates from NASA NAIF | Axial tilts match NASA within 0.1°. Mercury spins 3 times per 2 orbits. The Moon keeps one face toward Earth. Spin axes show in the app. | ✅ |
| 1b.2 | GPU renderer: textured, lit, rotating planets. Dots far away, spheres up close. Focusing zooms in. Earth's night side lit by cities. | Earth's continents turn once per 23.93 h. Uranus rolls on its side. Venus turns backwards. | ✅ |
| 1b.3 | Focus details: Earth's clouds and atmosphere, Saturn's rings (real structure up close, with shadows), Jupiter's bands and Great Red Spot, the Sun's surface | Measured ring structure matches the PDS boundaries (e.g. the Cassini Division, 117,500–122,050 km). Rings turn edge-on on the published 2025 dates. The Great Red Spot turns with Jupiter's 9.9 h day. | ✅ |
| 1b.4 | Hierarchical integration and the major moons | Io, Europa and Ganymede hold their 1:2:4 resonance. Moon positions match JPL after 30 days. | ✅ |
| 1b.5 | All known moons (more than 400), computed in detail when their planet is in focus | Every moon in JPL's list loads, and spot checks match JPL | ✅ |
| 1b.6 | Dwarf planets, major asteroids and comets | Adding the asteroids' mass shrinks Mars's error against JPL. Halley's Comet returns in July 2061. | ✅ |
| 1b.7 | The asteroid belt, Jupiter's Trojans, the Kuiper belt and zodiacal dust, from real catalogs | Kirkwood gaps appear in the real asteroids' orbits. Trojans cluster 60° ahead of and behind Jupiter. | ✅ |
| 1b.8 | The Sun in focus: solar wind (Parker spiral), heliosphere boundaries, sunlight and light travel time | 1361 W/m² of sunlight at Earth. Light takes 8.3 min to reach Earth. The spiral crosses 1 AU at about 45°. | ✅ |
| 1b.9 | Planets in focus: magnetospheres and radiation belts | Earth's magnetopause sits about 10 Earth radii sunward, from pressure balance with the solar wind | ✅ |

## M2: Compact objects and gravitational waves

| # | Step | How you verify it | Status |
|---|---|---|---|
| 2.1 | Black holes, neutron stars, white dwarfs, and the notable-objects catalog | Sagittarius A\*, TON 618, M87\* and PSR B1913+16 load with published masses | ✅ |
| 2.1b | Everything feels everything: the asteroid and Kuiper belts and every small moon become live particles that feel any added body | A black hole flyby kicks the belt as the impulse approximation predicts; with nothing added, belt bodies match JPL to 1 part in 10,000 after a year | ✅ |
| 2.1c | The galactic center: Sagittarius A\* with the stars that orbit it, placed 8.3 kpc away in its true direction | S2's 16-year orbit, and its relativistic precession of 12′ per orbit as GRAVITY measured | ✅ |
| 2.2 | Post-Newtonian binary dynamics with gravitational-wave energy loss | The Hulse–Taylor pulsar's orbital period shrinks at −2.40 × 10⁻¹² s/s | ✅ |
| 2.3 | Gravitational waveform plot and audible chirp | The chirp's frequency sweep matches the theoretical formula | ✅ |
| 2.4 | Mergers using numerical-relativity fits (mass, spin, kick, ringdown) | A GW150914 replay leaves a black hole of about 62 M☉ with spin about 0.67 | ✅ |
| 2.5 | Kerr-spacetime motion and extreme-mass-ratio inspiral | Innermost stable orbit at 6 GM/c² for a non-spinning hole. TON 618 + Sagittarius A\* runs. | ✅ |
| 2.6 | White dwarfs and the Chandrasekhar limit | The computed limit is 1.46 M☉ for an ideal carbon-oxygen white dwarf | ✅ |
| 2.7 | Neutron stars from equations of state, maximum mass, prompt collapse | The SLy equation of state gives a maximum mass of about 2.05 M☉ | ✅ |
| 2.8 | Hawking evaporation | Lifetimes match t = 5120πG²M³/(ħc⁴) | ⬜ |

## M3: Seeing curved spacetime

| # | Step | How you verify it | Status |
|---|---|---|---|
| 3.1 | GPU ray tracer for a non-spinning black hole, with a starfield | Shadow radius √27 GM/c². Light grazing the Sun bends 1.75″. | ⬜ |
| 3.2 | Spinning (Kerr) black hole lensing and accretion disk appearance | The shadow is lopsided as theory predicts. The disk is brighter on the side moving toward you. | ⬜ |
| 3.3 | Overlays: model indicator with validity warnings, clock-rate heatmap, tidal field, embedding diagram, gravitational-wave ripples | Each overlay toggles on and off, and its values match hand calculations | ⬜ |
| 3.4 | Proper time versus distant-observer time | A clock near a black hole visibly runs slow by the predicted factor | ⬜ |

## M4: Matter that deforms

| # | Step | How you verify it | Status |
|---|---|---|---|
| 4.1 | SPH self-gravitating fluid on the CPU | A star or planet left alone stays in equilibrium | ⬜ |
| 4.2 | SPH on the GPU for tens of thousands of particles | Smooth playback at the particle budget set for your GPU | ⬜ |
| 4.3 | Automatic switching between point body and fluid body when tides matter | A planet stays a point until it nears the tidal radius, then becomes a fluid | ⬜ |
| 4.4 | Tidal disruption scenarios | Disruption begins near r_t ≈ R(M/m)^(1/3). Debris falls back at a rate ∝ t^(−5/3). | ⬜ |

## M5: Portfolio release

| # | Step | How you verify it | Status |
|---|---|---|---|
| 5.1 | Scenario library: Mercury, Hulse–Taylor, GW150914, GW170817, planet versus black hole, TON 618 meets Sagittarius A\*, white dwarf pushed past the limit, evaporating mini black hole | Each scenario loads and plays from a menu | ⬜ |
| 5.2 | Validation report: plots generated automatically into `docs/validation/` | Every claim in the README links to a passing test and a plot | ⬜ |
| 5.3 | README with GIFs, physics docs, Windows release build | A fresh machine can download and run it | ⬜ |

## After v1

v2 is committed. Its contents are listed in [SCOPE.md](SCOPE.md#v2-committed-starts-once-v1-is-established), and it will be broken into steps here once v1 ships.
