# Einstein–Infeld–Hoffmann gravity (first post-Newtonian)

**Code:** `crates/worldline-core/src/gravity/eih.rs`
**Sources:** Einstein, Infeld & Hoffmann, *Annals of Mathematics* 39, 65 (1938). Implemented in the form JPL uses for its planetary ephemerides: Folkner et al. (2014), *IPN Progress Report* 42-196, eq. 27, with the general-relativity values β = γ = 1. Checked against the two-body equations in Blanchet (2014), *Living Reviews in Relativity* 17, 2.

## Why it matters

In 1859 Urbain Le Verrier found that Mercury's orbit turns, or precesses, slightly faster than Newtonian gravity can explain, even after accounting for every planet's pull. The modern value of the excess is 43″ (arcseconds) per century. For half a century people looked for a hidden planet, "Vulcan", to explain it. In 1915 Einstein showed that general relativity predicts exactly 43″. It was the first evidence that Newton's theory is incomplete.

## The model

General relativity can't be solved exactly for many bodies. But when gravity is weak and motion slow, as in the solar system, it can be expanded as Newton's law plus small corrections in powers of v²/c² and GM/(rc²). Keeping the first set of corrections gives the "first post-Newtonian" (1PN) equations of motion. For many bodies these are the Einstein–Infeld–Hoffmann (EIH) equations:

a_i = Σ_{j≠i} μ_j (r_j − r_i)/r_ij³ · { 1 + (1/c²) [ −4 Σ_{k≠i} μ_k/r_ik − Σ_{k≠j} μ_k/r_jk + v_i² + 2v_j² − 4 v_i·v_j − (3/2)((r_i − r_j)·v_j / r_ij)² + ½ (r_j − r_i)·a_j ] }
  + (1/c²) Σ_{j≠i} μ_j/r_ij³ [(r_i − r_j)·(4v_i − 3v_j)] (v_i − v_j)
  + (7/2c²) Σ_{j≠i} μ_j a_j / r_ij

In plain language, the corrections say that gravity's pull depends on:
- **the potential each body sits in.** Clocks run slower and space is curved deeper in a gravity well (the Σ μ/r terms).
- **how fast the bodies move.** Moving mass-energy gravitates differently (the v² and v·v terms).
- **the direction of motion.** A small pull acts along the relative velocity (the second line). This is what makes orbits precess.
- **the neighbours' accelerations.** Body j's acceleration feeds back on body i (the a_j terms). At this order, the Newtonian a_j is accurate enough.

### Implementation notes

- **Two passes, both O(N²).** The first pass computes every body's Newtonian acceleration and the potential it sits in. The second adds the corrections.
- **Corrections kept separate.** They are about 10⁻⁸ of the Newtonian pull for Mercury, so they're summed on their own and added at the end, where they keep their full precision.
- **Coordinates.** The equations are in harmonic coordinates, the same as JPL's. The simulation clock is coordinate time, matching JPL's TDB.
- **Needs IAS15.** The acceleration depends on velocities, which leapfrog can't handle, so leapfrog now refuses any model that reports `velocity_dependent()`.

## Where it's valid

- Weak fields and slow motion: GM/(rc²) ≪ 1 and v/c ≪ 1. In the solar system both are below 10⁻⁷.
- Second-order terms of size (GM/rc²)² are left out. They matter for compact binaries such as neutron stars, which step 2.2 adds.
- Energy lost to gravitational waves (the "2.5PN" term) is left out, so this model can't make binaries spiral together. That is also step 2.2.
- Bodies are treated as points. The Sun's oblateness, tides and asteroids are not modeled.

## Validation

| Check | Expected | Measured |
|---|---|---|
| Two bodies vs. Blanchet's textbook 1PN formula (unequal masses, and a test particle) | agreement up to 2PN-sized differences | passes, mismatch below 0.1% of the relativistic part |
| Corrections in a weak, slow system | about ε = GM/rc² + v²/c² in size | passes |
| **Mercury's perihelion precession** (Sun and Mercury, 1 century, sampled daily) | GR formula 6πGM / (c²a(1−e²)) per orbit: **42.9805″** per century | **42.9807″** per century (5 ppm). Newtonian control: 0.0000″ |
| One year of the real solar system vs. JPL | inner-planet errors at least 10× smaller than Newtonian | see below |

The Mercury test requires agreement within 0.01%, ten times tighter than the ±0.04″ astronomers achieve in observing the effect.

### Step 1.3's prediction, tested

| Body | Newtonian error after 1 year | With EIH | Improvement |
|---|---|---|---|
| Mercury | 88 km | 0.25 km | 345× |
| Venus | 96 km | 0.13 km | 764× |
| Earth | 61 km | 0.41 km | 149× |
| Mars | 31 km | 0.19 km | 164× |
| Moon | 51 km | 16 km | 3× |
| Jupiter–Pluto | ≤ 0.5 km | ≤ 0.14 km | up to 4× |

Relativity was the main physics missing for the inner planets. What remains is physics Worldline doesn't model yet:
- **asteroids,** whose pull matters most for Mars;
- **the Sun's slight flattening;**
- **for the Moon,** Earth's equatorial bulge and the tides between Earth and the Moon.
