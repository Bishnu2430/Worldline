# Newtonian gravity

**Code:** `crates/worldline-core/src/gravity/newtonian.rs`
**Sources:** Newton, *Philosophiæ Naturalis Principia Mathematica* (1687). Kepler, *Harmonices Mundi* (1619), for the third law used to validate it.

## The model

Every pair of point masses attracts along the line between them, with a strength that falls off as the square of the distance. The acceleration of body *i* is

a_i = Σ_{j≠i} μ_j (r_j − r_i) / |r_j − r_i|³

where μ_j = G m_j is body *j*'s gravitational parameter.

The code visits each pair once and applies equal and opposite pulls (Newton's third law). That makes total momentum conserved to rounding error by construction.

## Why bodies store GM instead of mass

Orbits reveal GM directly, so it is measured to about 10 significant figures (the Sun's is 1.3271244 × 10²⁰ m³/s²). G alone is known to only about 5 (22 parts per million). Storing mass in kg and multiplying by G would throw away 5 digits of precision. Worldline stores GM, divides by G only for display, and does energy bookkeeping in units where G factors out.

## Where it is valid

Newtonian gravity is the weak-field, slow-motion limit of general relativity. It needs:

- weak gravity: ε = GM / (r c²) ≪ 1
- slow motion: v / c ≪ 1

Even at Mercury, where ε ≈ 2.5 × 10⁻⁸, it is measurably wrong. It misses Mercury's extra 43″ per century of perihelion precession. So for the solar system Worldline uses the [Einstein–Infeld–Hoffmann equations](einstein-infeld-hoffmann.md) instead. Newtonian gravity stays as the fast model where ε is tiny. It cannot produce light bending, gravitational waves or black holes at all.

The model also assumes bodies never share a position (the force would be infinite). Collision handling in step 1.7 enforces that.

## Validation

`crates/worldline-core/tests/validation_kepler.rs`, run by `cargo test`:

| Check | Expected | Measured |
|---|---|---|
| Mercury-like orbit period vs. Kepler's third law | 87.969026 days | relative error 9 × 10⁻⁹ |
| Earth-like orbit period | 365.256343 days | relative error 5 × 10⁻⁹ |
| Jupiter-like orbit period | 4332.333195 days | relative error 6 × 10⁻⁹ |
| Comet-like orbit (e = 0.7) period | 1897.930517 days | relative error 6 × 10⁻⁷ |
| Sun, Earth and Jupiter over 100 years: momentum | constant | changes by 1 × 10⁻¹⁴ |
| Same system: angular momentum | constant | changes by 7 × 10⁻¹⁵ |

The comet's larger error comes from the fixed-step integrator, not the gravity model. See [integrators.md](integrators.md).
