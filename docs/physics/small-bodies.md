# Small bodies: dwarf planets, asteroids and comets

**Code:** `crates/worldline-core/src/hierarchy.rs` (followers), `crates/worldline-core/src/gravity/nongravitational.rs`, `crates/worldline-data/src/small_bodies.rs`
**Data:** `crates/worldline-data/data/small-bodies-2025-01-01.csv`, `small-bodies-2026-01-01T06.csv` · **Fetch script:** `tools/fetch_small_bodies.py`
**Tests:** `crates/worldline-data/tests/validation_small_bodies.rs`, plus unit tests in the core

## What's included

62 bodies, chosen because they are the heaviest, were visited by spacecraft, or are famous:

| Group | Count | Bodies |
|---|---|---|
| Dwarf planets | 4 | Ceres, Eris, Haumea, Makemake (Pluto is already with the planets) |
| Beyond Neptune | 5 | Gonggong, Quaoar, Orcus, Sedna, Arrokoth (visited by New Horizons) |
| Asteroids | 35 | The 15 heaviest after Ceres (Pallas, Vesta, Hygiea, …); spacecraft targets (Eros, Itokawa, Ryugu, Bennu, Didymos, Lutetia, Psyche, Dinkinesh, Donaldjohanson, Patroclus, Eurybates, …); near-Earth asteroids such as Apophis, Phaethon, Icarus and Toutatis |
| Comets | 15 | Halley, Encke, Swift–Tuttle, Tempel–Tuttle, Hale–Bopp, NEOWISE, Tsuchinshan–ATLAS, and comets visited by spacecraft (Tempel 1, Borrelly, Wild 2, Hartley 2, Churyumov–Gerasimenko, Giacobini–Zinner, Wirtanen); Pons–Brooks |
| Interstellar visitors | 3 | ʻOumuamua (2017), Borisov (2019), 3I/ATLAS (2025) |

The full asteroid belt, Jupiter's Trojans and the Kuiper belt, with hundreds of thousands of bodies from real catalogs, come in step 1b.7.

## Where the numbers come from

| What | Source |
|---|---|
| Positions and velocities | NASA JPL Horizons, each body's own JPL orbit solution (for example, JPL#48 for Ceres and JPL#75 for Halley; the data file's header lists them all). Same epochs and frame as the planets: 2025-01-01 00:00 TDB, and one Julian year later for validation. |
| Masses (GM) | JPL DE440 (Park et al. 2021), from NAIF's `gm_de440.tpc`. Only for the 22 bodies that file lists: Ceres, the 15 heaviest other asteroids, Eros, Didymos, Eris, Haumea, Quaoar and Orcus. |
| Sizes | IAU mean radii from `pck00011.tpc`, else the radius Horizons lists. Some comets and interstellar objects have none, and are drawn as dots. |
| Non-gravitational forces | The force model fitted in the same Horizons orbit solution (20 bodies). |
| Spin axes and rates | IAU WGCCRE, `pck00011.tpc`, for the 14 bodies it covers: Ceres, Pallas, Vesta, 52 Europa, Davida, Lutetia, Ida, Eros, Gaspra, Steins, Itokawa, Tempel 1, Borrelly and Churyumov–Gerasimenko. (Ida and Gaspra are filed under the ids NAIF gave them for the Galileo flybys, 2431010 and 9511010.) |
| Halley's 2061 perihelion (for validation) | JPL Horizons, the osculating elements of solution JPL#75 at 2061-07-01 |

Comets are fetched with Horizons' "closest apparition" rule (`CAP`), which picks the orbit solution for the apparition closest to today. `NOFRAG` excludes fragments.

## Two ways to simulate a small body

### Heavy enough to matter: in the top level

The 22 bodies with DE440 masses join the Sun and planets at the top level. They feel, and exert, the same relativistic Einstein–Infeld–Hoffmann gravity (see [einstein-infeld-hoffmann.md](einstein-infeld-hoffmann.md)). Ceres has 1/6,400 of Earth's mass, and the main belt as a whole pulls hardest on Mars, the planet closest to it. JPL's planetary ephemeris includes these bodies for that reason, and so does Worldline. The rule is simple: a body pulls on the planets exactly when DE440 says it does.

### Too light to matter: followers

The other 40 are effectively massless: even Makemake has less than 1/1,000 of Earth's mass, and most are a few kilometers across. They can't pull on anything we can measure. Putting them in the top level would also be costly: IAS15 sizes each step for the fastest-changing body, so one comet rounding the Sun would force tiny steps on the whole solar system.

Instead, each one is a **follower** with its own IAS15 integrator. It works the way small moons follow their major moons (see [small-moons.md](small-moons.md)):

1. **The top level takes a step.** It records every body's position, velocity and acceleration at both ends of the step.
2. **The path in between is interpolated.** The record defines a quintic Hermite curve for each body, accurate to the same order as the step itself.
3. **Each follower catches up.** It integrates across the same interval, taking as many steps as it needs, on a clock that restarts at zero each step, so time doesn't lose digits to a large running total.

A follower feels:
- **Newtonian gravity** from every top-level body (the Sun, planets, planetary systems as barycenters, and the 22 heavy small bodies).
- **The Sun's relativistic correction** for a test body, the Schwarzschild term (IERS Conventions 2010, eq. 10.12, with β = γ = 1):

  a = (μ/c²r³) [(4μ/r − v²) r + 4 (r·v) v]

  This makes orbits precess as general relativity predicts, including Icarus's and Phaethon's, whose perihelia lie closer to the Sun than Mercury's. The planets' own relativistic terms are smaller than the Sun's by their mass ratio (1/1,047 for Jupiter), far below anything the validation can see.
- **Non-gravitational forces,** where JPL's solution includes them (below).

**Check:** a follower on Mercury's orbit precesses 5.018670 × 10⁻⁶ rad over 10 orbits; general relativity's 6πμ/(c²a(1−e²)) per orbit predicts 5.018654 × 10⁻⁶ rad. They agree to 3 parts per million. The test requires 10⁻⁴.

## Non-gravitational forces

**Source:** Marsden, Sekanina & Yeomans, "Comets and nongravitational forces. V", *Astronomical Journal* 78, 211 (1973). Yeomans & Chodas, *Astronomical Journal* 98, 1083 (1989) for the delay.

Sunlight pushes some small bodies in ways gravity can't explain:
- **A comet's jets.** As ice sublimates on the sunward side, the escaping gas pushes the comet like a weak rocket. Cowell and Crommelin's careful gravitational prediction of Halley's 1910 return came out about 3 days early; this push is why.
- **The Yarkovsky effect.** An asteroid absorbs sunlight and re-radiates the heat a few hours later, after it has turned. The heat leaves in a slightly different direction from the light that came in, so the asteroid gets a tiny, steady push. It shrinks Bennu's orbit by about 285 m a year (Farnocchia et al. 2021).

JPL fits the push in each body's orbit solution using one standard form:

a = g(r) (A1 r̂ + A2 t̂ + A3 n̂),  g(r) = α (r/r₀)^−m (1 + (r/r₀)^n)^−k

- **Directions.** r̂ points away from the Sun, t̂ along the motion in the orbit's plane, and n̂ perpendicular to the orbit.
- **A1, A2, A3** are the fitted strengths.
- **g(r)** says how the push fades with distance:
  - **For comets,** the default curve follows water ice sublimating: α = 0.1112620426, r₀ = 2.808 AU, m = 2.15, n = 5.093, k = 4.6142, so g(1 AU) = 1. Beyond about 3 AU it plunges, as the ice stops sublimating.
  - **For the Yarkovsky effect,** JPL uses g = (1 AU / r)².
  - **ʻOumuamua** has its own fitted curve.
- **A delay Δt.** Many comets outgas hardest some days after perihelion, once the heat has soaked in. With a delay, g is evaluated at the distance the comet had Δt earlier along its two-body orbit:
  - **For bound orbits,** this uses Kepler's equation for the ellipse.
  - **For unbound orbits** (Borisov, 3I/ATLAS), it uses the hyperbolic form, e sinh H − H = n t.

20 of the 62 bodies have a fitted model: 11 comets, 6 near-Earth asteroids (Yarkovsky) and all 3 interstellar objects. The app labels each one in the inspector.

## Validation

### The asteroids' mass shrinks Mars's error (`cargo test`)

The planets were simulated for one year with relativistic gravity, with and without the 22 heavy small bodies, and compared with JPL's 2026 positions:

| Body | Without small bodies | With them |
|---|---|---|
| Sun | 0.135 km | 0.004 km |
| Mercury | 0.255 km | 0.120 km |
| Venus | 0.126 km | 0.017 km |
| Earth | 0.410 km | 0.209 km |
| **Mars** | **0.189 km** | **0.109 km** |
| Jupiter | 0.137 km | 0.047 km |
| Saturn | 0.044 km | 0.003 km |
| Uranus, Neptune, Pluto | 0.02–0.12 km | unchanged within 0.01 km |
| Moon | 16.4 km | 16.6 km |

The roadmap asked for Mars's error to shrink. It falls by 42%, and most other bodies improve too. The Sun improves 30×, because it is pulled by the asteroids the way they are pulled by it. The remaining 0.1 km for Mars likely includes the pull of the hundreds of smaller asteroids DE440 also integrates. The Moon's 16 km is a separate issue, unaffected here: Earth's bulge and the Earth–Moon tides (see [solar-system-data.md](solar-system-data.md)).

### Every small body after one year (`cargo test`)

The whole solar system was run for one year (moons and all), then every small body was compared with JPL. The standard is the planets' standard from step 1.3: within 1 part in 10,000 of its distance from the Sun.

| Result | Bodies |
|---|---|
| Within 0.2 km | 61 of 62. Typical errors are 0.01 km (10 m). The largest are Ceres at 0.18 km, then Vesta at 0.10 km and Apophis at 0.05 km. 3I/ATLAS, which passed perihelion and outgassed hard during the year, is within 0.004 km. |
| Bennu | 2,420 km (1.5 × 10⁻⁵ of its distance): an open residual, see below |

**3I/ATLAS, a residual tracked down.**
- **The symptom.** The first version missed by 123 km.
- **The cause.** Its fitted push lags 9.5 days behind its distance from the Sun, and the delayed distance was first taken along a straight line, on the theory that an orbit this open (e = 6.1) is nearly straight. Near perihelion it isn't: the straight line put the comet 0.56% too far from the Sun. Its push, which goes as 1/r², came out about 1% weak. And that push is large: at 1 AU, about 100 times Halley's.
- **The fix.** Stepping back along the true hyperbola brought the error down to 4 m. A unit test checks the hyperbola against a direct numerical integration: they agree to rounding (2 × 10⁻¹⁶), where the straight line was off by 5.6 × 10⁻³.

**Bennu, an open residual.**
- **What's different.** Horizons doesn't serve Bennu from an ordinary orbit solution. It uses a special trajectory, JPL#118 (Farnocchia et al. 2021, *Icarus* 369, 114594), fitted to OSIRIS-REx tracking and built on the older DE424 planetary ephemeris. Its model has 343 perturbing asteroids, the Yarkovsky effect and solar radiation pressure.
- **Its force model can't be loaded.** Its record lists A1 = A2 = A3 = 0 and describes the sunlight forces through a physical model (density, area-to-mass ratio) instead of the standard form. So Worldline runs Bennu on gravity alone.
- **Those forces don't explain it.** Rough estimates put them far below the error:
  - Yarkovsky would move Bennu about 1.4 km along its orbit in a year;
  - radiation pressure about 0.1 km.
- **The trajectory itself is fine.** We checked JPL's served trajectory day by day through 2025: it is smooth, with no seam.
- **The remaining candidate** is a mismatch of reference points. Bennu's trajectory is measured from DE424's solar system barycenter; Worldline's Sun comes from DE441. If Bennu starts even 100–200 km off relative to the Sun, its orbital period is slightly off, and the error grows along its orbit to this size within a year. This is not confirmed. The error is still well within the 1-in-10,000 standard, so the test passes. The residual is reported, not hidden.

### Halley's Comet returns in July 2061 (`cargo test --release -- --ignored`)

Starting from Halley's January 2025 state, near its farthest point from the Sun, the simulation runs 36.5 years to mid-2061. It then measures the comet's distance from the Sun every hour through the summer:

| | Perihelion |
|---|---|
| Worldline | JD 2474034.208 TDB, 28 July 2061 at about 17:00, 0.593 AU from the Sun |
| JPL (Horizons, solution JPL#75) | JD 2474034.220 TDB, 28 July 2061 at 17:17 |
| Difference | −0.012 days (17 minutes), within the test's one-hour sampling |

![Halley's Comet at perihelion on 28 July 2061 in Worldline, its path diving in toward the Sun](../images/halley-2061-step-1b.6.png)

To see it in the app (the window opens after about two minutes of simulating; `--zoom` counts Halley's 5 km radii, so 40 million puts the camera about 1.4 AU out):

```bash
cargo run --release -- --focus Halley --zoom 40000000 --advance 36.57 --paused
```

The roadmap's bar was "returns in July 2061", a month-wide target. Worldline matches JPL's prediction to within the hour, after 36 years and nearly 6 billion km of travel. That includes Halley's outgassing model, its pass by the giant planets, and the Sun's relativistic pull. The run skips the moons, which don't affect Halley, and takes about 35 seconds in a release build.

### Smaller checks (unit tests)

- **The water-ice curve.** g(1 AU) = 1 within 10⁻⁴; the published constants carry 4–5 significant figures.
- **Directions.** A1, A2 and A3 push away from the Sun, along the motion, and north.
- **Delay.** Leaving perihelion, a delayed model pushes harder.
- **Hyperbola.** Stepping back along an unbound orbit agrees with integrating it.
- **Loading.** The data loads, with Halley's model, Ceres's DE440 mass, Bennu massless, and JPL's 2061 perihelion.

## Limits

- **Earth is a point.** Apophis passes 38,000 km from Earth's center on 13 April 2029. Earth's equatorial bulge and the Moon's exact pull at that range bend its path in ways JPL models and Worldline doesn't yet. After the flyby, Apophis's position is approximate.
- **22 perturbers, not 343.** DE440 integrates 343 asteroids, but NAIF's GM file publishes the masses of only the heaviest. Worldline includes those 22 bodies; the rest are followers.
- **Comet models are fits.** A comet's outgassing changes from one pass to the next, and JPL fits it to the passes observed. Run far into the future, a comet's position grows less certain. That is true of JPL's predictions too.
- **No breakups or collisions.** Fragments are excluded, and small bodies pass through each other (collisions arrive in step 1.7).
- **No surface maps yet.** Small bodies are drawn in flat colors by kind. Solar System Scope's dwarf-planet maps are labeled fictional, so they aren't used (see [CREDITS.md](../../CREDITS.md)).
