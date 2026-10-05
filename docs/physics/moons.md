# Moons: hierarchical integration and the planets' gravity fields

**Code:** `crates/worldline-core/src/hierarchy.rs`, `crates/worldline-core/src/gravity/{zonal,tesseral,figure}.rs`, `crates/worldline-data/src/moons.rs`
**Data:** `crates/worldline-data/data/{moons-2025-01-01,moons-2025-01-31,zonal-harmonics,tesseral-harmonics,moon-figures,ring-masses}.csv` · **Fetch script:** `tools/fetch_moons.py`
**Tests:** `crates/worldline-data/tests/validation_moons.rs`, `crates/worldline-core/tests/validation_oblateness.rs`

## Why it matters

Moons are fast. Phobos circles Mars in 7.7 hours and Io circles Jupiter in 1.8 days, while Neptune takes 165 years to go round the Sun. Moons also feel more than point masses do. A spinning planet bulges at its equator, Mars is lumpy, and Io, Europa and Ganymede pull on each other in a lock that has held for billions of years.

Worldline has the 21 major moons:
- **Mars:** Phobos, Deimos.
- **Jupiter:** Io, Europa, Ganymede, Callisto.
- **Saturn:** Mimas, Enceladus, Tethys, Dione, Rhea, Titan, Hyperion, Iapetus.
- **Uranus:** Miranda, Ariel, Umbriel, Titania, Oberon.
- **Neptune:** Triton.
- **Pluto:** Charon.

## The model

### 1. Hierarchical integration

Putting moons in the same integration as the planets would force the whole solar system onto steps of minutes. Instead, the simulation has two levels.

**The top level** holds the Sun, the planets, and each planetary system as one point at its barycenter. It runs as before: relativistic Einstein–Infeld–Hoffmann gravity with the IAS15 integrator.

**Each moon system** holds a planet and its moons, in their own frame centered on the system's barycenter. After every top-level step, each moon system catches up over the same interval, with its own IAS15 taking as many small steps as it needs. Inside, bodies feel:
- **each other** (Newtonian gravity);
- **the planet's gravity field** (sections 2 and 3);
- **the moons' own shapes** (section 4);
- **the tides of everything outside:** the Sun and the other planets. Each pulls on every member, minus its pull on the barycenter, which the top level already accounts for. Their positions during the step come from cubic Hermite interpolation between the step's ends.

The top level treats each system as a point mass at its barycenter. This is exact except for tiny tidal couplings, of relative size about (r/R)² times the moons' share of the system's mass. The same split is how JPL builds its ephemerides: planetary ephemerides (DE440) for the barycenters, and satellite ephemerides for the moons around them.

### 2. Oblateness: zonal harmonics

A spinning planet is flattened, so it pulls harder at its equator. Its potential beyond a point mass is

Φ = −(μ/r) [1 − Σ J_n (R/r)ⁿ P_n(sin φ)]

with latitude φ, reference radius R, and Legendre polynomials P_n. The acceleration, worked out analytically, is

a_n = (μ/r²) J_n (R/r)ⁿ [((n+1) P_n + u P_n′) r̂ − P_n′ k̂]

where u = sin φ and k̂ is the pole. Worldline uses J2 to J6 from each satellite ephemeris. Each moon's pull on the bulge is returned to the planet (Newton's third law). The pole comes from the IAU rotation model at the epoch; it moves by hundredths of a degree per century.

### 3. Mars's lumps: tesseral harmonics

Mars's Tharsis volcanic plateau makes its equator slightly elliptical, so its gravity depends on longitude and turns with the planet:

U = (μ/r) Σ (R/r)ⁿ P_nm(sin φ) [C_nm cos mλ + S_nm sin mλ]

Worldline uses all of MAR099's coefficients to degree and order 6. The accelerations come from Cunningham's recurrences (Montenbruck & Gill, *Satellite Orbits*, 2000, §3.2.4–3.2.5), which stay finite at the poles. Mars turns at its IAU rate about its IAU pole. The IAU prime meridian agrees with MAR099's own to 0.03°.

This turned out to matter a lot. Deimos orbits just outside the synchronous orbit, so Mars turns beneath it slowly and the lumps' pull doesn't average out. For a fixed starting state, the pull shifts the moon's mean motion by up to 6 J₂₂ (R/a)² n/(n − Ω):
- **Deimos:** about 5 × 10⁻⁵;
- **Phobos:** about 10⁻⁴.

Here J₂₂ = √(C22² + S22²) and Ω is Mars's spin rate. Without the lumps, Phobos and Deimos drift 108 km and 175 km from JPL in 30 days; with them, 0.10 km and 0.28 km.

### 4. Tidally locked moons' shapes

A moon that always shows its planet the same face is stretched into a slight egg shape, with its long axis toward the planet. Along that axis its degree-2 potential is (Gm/r)(R/r)²(J2/2 + 3 C22). So moon and planet attract each other harder than point masses would: each accelerates toward the other by an extra 3 G M_other R² (J2/2 + 3 C22) / r⁴. The coefficients are the ones JPL uses:
- MAR099 for Phobos and Deimos;
- JUP365 for the Galilean moons.

This explained Io's 12 km and Phobos's 4.6 km residuals. For a fixed starting state, the shift in mean motion is 2 × 3(R/r)²(J2/2 + 3 C22): 2.9 × 10⁻⁷ for Io, against 2.8 × 10⁻⁷ measured.

### 5. Masses that match the states

Each moon system keeps the GMs from the same JPL satellite solution that produced the moons' positions. The planetary ephemeris (DE440) supplies only each system's total, for its path around the Sun. The two must agree to 1 part in 10,000; Pluto's differ most, by 7.5 parts in 100,000.

Mixing solutions is costly. For a fixed starting state, a GM wrong by δμ/μ shifts the mean motion by 2δμ/μ. Using DE440's Pluto-system GM with plu060's Charon moved Charon 89 km off JPL in 30 days.

Saturn's rings carry about 1.1 km³/s² of GM, which SAT441 lists separately from Saturn's. The rings orbit well inside the moons, so Worldline adds that mass to Saturn's center. A ring of radius ρ pulls a moon at distance a harder than a point mass by a factor of about 1 + ¾(ρ/a)², a few percent of an already small pull.

## Sources

| What | Source |
|---|---|
| Moon positions and velocities | NASA JPL Horizons, from the satellite ephemerides MAR099, JUP365, SAT441, URA184 (major moons from the URA182 integration), NEP098 and PLU060 |
| GMs, J2–J6, Mars's C_nm and S_nm, moon shapes, Saturn's ring masses | The constants JPL integrated those ephemerides with, from NASA NAIF's comment files (`mar099.cmt`, `jup365.cmt`, `sat441.cmt`, `ura184_part-1.cmt`, `nep098_part-1.cmt`, `plu060.cmt`). A file can hold several integrations; the fetch script uses the one that covers 2025 and integrated these moons. |
| Radii | IAU mean radii (Archinal et al. 2018), NAIF `pck00011.tpc` |
| Tesseral recurrences | Montenbruck & Gill, *Satellite Orbits* (Springer 2000), §3.2 |
| Moon shape potential, osculating orbits | Murray & Dermott, *Solar System Dynamics* (Cambridge 1999), §2 and §4–5 |
| Laplace resonance libration | Lieske (1998), *A&A Supplement* 129, 205: period about 2071 days; free amplitude 0.066° (Lieske 1980) |

## Where it's valid

- **Newtonian gravity inside moon systems.** Relativity changes moon orbits by about GM/(c²r): 3 × 10⁻⁹ for Io, a few meters a month.
- **Fixed poles.** Planets' poles are held at their 2025 directions. They drift by hundredths of a degree per century.
- **No tidal dissipation.** Tides that slowly push moons outward (Io, the Moon) or pull them in (Phobos) change positions by meters over decades.
- **Not yet modeled:**
  - the small moons, which arrive in step 1b.5: Neptune's six inner moons, Uranus's Puck, Jupiter's Amalthea group;
  - the shapes of Saturn's moons, which SAT441 lists without the reference radius their coefficients need.
- **Hyperion tumbles chaotically.** The IAU publishes no rotation model for it, so it is drawn without spin.

## Validation

### Positions after 30 days

`moons_match_jpl_after_30_days` starts from JPL's states on 2025-01-01 00:00 TDB, simulates the whole hierarchy for 30 days, and compares each moon's position relative to its planet with JPL's on 2025-01-31.

**Criterion:** each moon must be closer to JPL's position than its own radius. Then the simulated moon overlaps the real one: it is drawn in the right place, and its eclipses and transits come at the right times.

| Moon | Error (km) | Radius (km) | Error (degrees, seen from the planet) |
|---|---|---|---|
| Phobos | 0.10 | 11 | 0.0006 |
| Deimos | 0.28 | 6 | 0.0007 |
| Io | 0.64 | 1822 | 0.0001 |
| Europa | 0.87 | 1561 | 0.0001 |
| Ganymede | 0.36 | 2631 | 0.00002 |
| Callisto | 1.44 | 2410 | 0.00004 |
| Mimas | 12.91 | 198 | 0.0040 |
| Enceladus | 1.30 | 252 | 0.0003 |
| Tethys | 8.21 | 531 | 0.0016 |
| Dione | 0.08 | 561 | 0.00001 |
| Rhea | 0.08 | 764 | 0.00001 |
| Titan | 0.26 | 2575 | 0.00001 |
| Hyperion | 0.17 | 139 | 0.00001 |
| Iapetus | 0.06 | 734 | < 0.00001 |
| Miranda | 1.55 | 236 | 0.0007 |
| Ariel | 0.10 | 579 | 0.00003 |
| Umbriel | 0.71 | 585 | 0.0002 |
| Titania | 0.02 | 789 | < 0.00001 |
| Oberon | 0.19 | 761 | 0.00002 |
| Triton | 21.64 | 1353 | 0.0035 |
| Charon | 0.33 | 606 | 0.0010 |

### Mars's lumps are needed

`mars_lumpy_gravity_keeps_phobos_and_deimos_on_track` runs Mars's system with and without the tesseral field:
- **with it,** both moons are within their radius of JPL;
- **without it,** both are more than their radius away (108 km and 175 km).

### The Laplace resonance

Io, Europa and Ganymede orbit in a 1:2:4 ratio, locked so that the Laplace angle φ = λ_Io − 3 λ_Europa + 2 λ_Ganymede stays at 180°, where λ is each moon's mean longitude. A triple conjunction can never happen.

`galilean_moons_hold_the_laplace_resonance` (slow; run with `cargo test --release -- --ignored`) measures φ in Jupiter's equatorial plane, averaged over each 30 days, for 12 years:
- **As JPL has them:** φ stays between 179.68° and 180.31°. The wobble has a period of about 480 days; the line of Io–Europa conjunctions turns once every 487 days.
- **After a push:** the test pushes Io forward by 3 m/s, which changes its mean motion enough that, left alone, φ would drift 462° in 12 years. Instead the moons' pull on each other turns it back. φ swings between 147.8° and 212.9°, against ±34.8° for a pendulum with the published period. It crosses 180° every 1022 days or so, a libration period of 2045 days against the published 2071 (1.3% short).
- **Assertions:** φ never goes round, and it swings back across 180° (it crosses 4 times in 12 years).

### Oblateness

`validation_oblateness.rs` checks J2 on its own. An Io-like orbit inclined 10° to Jupiter's equator must regress its node at the textbook rate −(3/2) n J2 (R/a)² cos i. Measured −0.12720 °/day against −0.12696 °/day. The 0.2% gap is within what the first-order formula leaves out: second-order terms, and the difference between instantaneous and averaged orbits.

### Unit tests

- The zonal acceleration equals minus the gradient of its potential. Legendre polynomials match their closed forms.
- The tesseral acceleration equals the gradient of its potential. C22 matches its closed form. Its zonal terms reproduce the zonal field to 10⁻¹².
- The moon-shape pull matches the degree-2 field along the long axis.
- A moon system stays centered on its host. Without oblateness, a moon keeps Kepler's period to 0.001°.

## How the residuals were tracked down

The first run was much worse than the table above. Each fix came from finding which physics JPL had and Worldline didn't:

| Symptom (30 days) | Cause | Fix | After |
|---|---|---|---|
| Charon 89 km, along its orbit | Pluto's GM from DE440 (975.5) with Charon from plu060 (975.43): δn/n = 2δμ/μ | Keep each satellite solution's own GMs | 0.3 km |
| Titania 13 km, Oberon 40 km | Horizons uses ura184 (major moons from URA182); the constants came from ura116xl | Read the integration Horizons actually used | ≤ 1.6 km |
| Phobos 113 km, Deimos 176 km, along track, growing steadily | Mars's longitude-dependent gravity (Tharsis), amplified for Deimos near synchronous orbit | Tesseral harmonics to degree 6 | 4.7 km, 0.05 km |
| Io 12 km, Phobos 4.7 km | Locked moons' egg shapes pull harder | J2, C22 shape term | 0.6 km, 0.1 km |
| Every Saturn moon behind JPL by ≈ 5 × 10⁻⁸ in mean motion | Ring mass (1.1 km³/s²) listed apart from Saturn's | Add it at Saturn's center | Dione, Rhea, Titan, Hyperion, Iapetus ≤ 0.3 km |

What remains:
- **Triton, 22 km** (2 × 10⁻⁶ in mean motion). Neptune's six small inner moons (3.2 km³/s² in all) aren't modeled yet; adding their mass at Neptune's center halves the error. They arrive in step 1b.5. The other half is unexplained.
- **Mimas, 13 km, and Tethys, 8 km** (3 × 10⁻⁷ in mean motion, both behind). These two are locked in a 2:1 resonance of their own. Their shapes, Saturn's tidal response and the rings' exact geometry are all too small by our estimates. The cause is not yet found.
- **Enceladus, 1.3 km.** Its shape, which SAT441 lists without a reference radius, accounts for about 1.6 km by estimate.
- **Miranda, 1.6 km.** Puck (0.13 km³/s², inside Miranda's orbit) accounts for about 0.8 km.

## Cost

One simulated year of the whole hierarchy takes about 1.7 s of computing (release build). Mars's system takes 1.1 s of that: Phobos needs about 43 steps per 7.7-hour orbit, and the lumpy field is evaluated at every substep.

At the app's default speed (a month per second) that's about a fifth of the 12 ms per-frame physics budget. At a year per second or faster, physics runs slower than requested and the app says so. Step 1b.5 adds cheaper treatment for moon systems that aren't in focus.
