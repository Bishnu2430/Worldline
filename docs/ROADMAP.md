# Roadmap: v1 "Einstein Release"

Every step ends with something you can run and a number or behavior you can check. Steps are sized so checking one takes about an hour.

Status: ⬜ not started · 🟨 in progress · ✅ done

At under 5 hours a week, v1 is roughly 6–12 months away. We'll recalibrate after M1.

## M1: Engine core and the real solar system

| # | Step | How you verify it | Status |
|---|---|---|---|
| 1.1 | Rust workspace, physical constants, body types, Newtonian gravity, first integrator | `cargo test`: a two-body orbit obeys Kepler's third law and conserves energy | ✅ |
| 1.2 | IAS15 high-accuracy adaptive integrator | Energy error stays near the limit of double precision over 10,000 orbits | ⬜ |
| 1.3 | Real solar system loaded from a NASA JPL Horizons snapshot | After one simulated year, Earth's position matches JPL's to better than 1 part in 10,000 | ⬜ |
| 1.4 | Einstein–Infeld–Hoffmann relativistic gravity | Mercury's perihelion precesses 43″ per century beyond the Newtonian value | ⬜ |
| 1.5 | First window: 3D view, bodies, orbit trails, camera, time controls | `cargo run` shows the solar system moving | ⬜ |
| 1.6 | Sandbox tools: select, inspect, add, drag-launch, delete, save and load | Drop a Jupiter-mass planet into the inner solar system and watch the orbits get disturbed | ⬜ |
| 1.7 | Collisions (merge, conserving momentum) and a first version of the physics-model indicator | Two bodies collide; total momentum before and after matches | ⬜ |

## M2: Compact objects and gravitational waves

| # | Step | How you verify it | Status |
|---|---|---|---|
| 2.1 | Black holes, neutron stars, white dwarfs, and the notable-objects catalog | Sagittarius A\*, TON 618, M87\* and PSR B1913+16 load with published masses | ⬜ |
| 2.2 | Post-Newtonian binary dynamics with gravitational-wave energy loss | The Hulse–Taylor pulsar's orbital period shrinks at −2.40 × 10⁻¹² s/s | ⬜ |
| 2.3 | Gravitational waveform plot and audible chirp | The chirp's frequency sweep matches the theoretical formula | ⬜ |
| 2.4 | Mergers using numerical-relativity fits (mass, spin, kick, ringdown) | A GW150914 replay leaves a black hole of about 62 M☉ with spin about 0.67 | ⬜ |
| 2.5 | Kerr-spacetime motion and extreme-mass-ratio inspiral | Innermost stable orbit at 6 GM/c² for a non-spinning hole. TON 618 + Sagittarius A\* runs. | ⬜ |
| 2.6 | White dwarfs and the Chandrasekhar limit | The computed limit is 1.46 M☉ for an ideal carbon-oxygen white dwarf | ⬜ |
| 2.7 | Neutron stars from equations of state, maximum mass, prompt collapse | The SLy equation of state gives a maximum mass of about 2.05 M☉ | ⬜ |
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
