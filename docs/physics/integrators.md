# Integrators

An integrator advances the simulation through time. Gravity says how every body is accelerating *right now*. The integrator turns that into where every body will be a small time step later, and repeats. All the simulation's accuracy over long times depends on this step.

## Leapfrog (velocity Verlet)

**Code:** `crates/worldline-core/src/integrator/leapfrog.rs`
**Sources:** Verlet, *Physical Review* 159, 98 (1967). Hairer, Lubich & Wanner, "Geometric numerical integration illustrated by the Störmer–Verlet method", *Acta Numerica* 12, 399 (2003).

Each step of size h:

1. **Kick:** v ← v + (h/2) a(r)
2. **Drift:** r ← r + h v
3. **Kick:** v ← v + (h/2) a(r), using the new positions

Its properties:

- **Second-order accurate:** halving the step cuts the error by 4.
- **Symplectic:** it exactly conserves a quantity very close to the true energy. So the energy error oscillates inside a fixed band forever instead of growing. Simpler methods (such as Euler's) make planets slowly spiral outward or inward.
- **Time-reversible:** run it backwards and it retraces its path.

### Limits

- **Fixed step.** A step small enough for a comet's fast pass near the Sun is wasted on the slow far end of its orbit. On very eccentric orbits, leapfrog loses accuracy (see the comet row below). It also adds a small artificial precession that grows with step size, which would swamp Mercury's 43″ per century. IAS15 (below) is the accurate default. Leapfrog remains the fast preview integrator.
- **Position-dependent forces only.** It assumes acceleration depends only on positions, so it is not used with relativistic gravity, which also depends on velocities.

### Validation

`crates/worldline-core/tests/validation_kepler.rs` and unit tests, run by `cargo test`:

| Check | Expected | Measured |
|---|---|---|
| Energy error band, orbits 1–100 vs. 1901–2000 (e = 0.5, 1000 steps per orbit) | no growth | 1.073 × 10⁻⁴ in both |
| Error ratio when the step halves | 4 (second order) | 4.000 |
| Run 5,000 steps forward, reverse velocities, run 5,000 back | returns to start | yes, within rounding |
| Comet-like orbit (e = 0.7) period, 50,000 steps per orbit | Kepler's third law | relative error 6 × 10⁻⁷ (vs. ~10⁻⁸ for near-circular orbits) |

## IAS15

**Code:** `crates/worldline-core/src/integrator/ias15.rs`
**Source:** Rein & Spiegel, "IAS15: a fast, adaptive, high-order integrator for gravitational dynamics, accurate to machine precision over a billion orbits", *MNRAS* 446, 1424 (2015). It is the default integrator of the REBOUND research code.

### How it works

1. **Model the acceleration within a step as a polynomial.** Each body's acceleration over the step is written as a(s) = a₀ + b₀s + b₁s² + … + b₆s⁷, where s runs from 0 to 1 across the step.
2. **Fit it at 8 special points.** The polynomial is sampled at the Gauss–Radau points: the start of the step plus the 7 roots of P₇(x) + P₈(x), where Pₙ are Legendre polynomials. These spacings make integrating the polynomial accurate to 15th order, far beyond leapfrog's 2nd.
3. **Iterate to self-consistency.** Positions at each point depend on the polynomial, and the polynomial depends on accelerations at those positions. So the method predicts, recomputes the accelerations, refits, and repeats (a predictor–corrector loop) until the corrections stop shrinking.
4. **Measure its own error and pick the next step.** The highest-order coefficient b₆, relative to the acceleration, estimates the step's error. The next step is scaled by (ε / error)^(1/7) with ε = 10⁻⁹, which keeps the error per step below double-precision rounding. A step that should have been more than 4× shorter is thrown away and redone.
5. **Get a head start.** The polynomial from one step is shifted forward to predict the next one, so the iteration starts close to the answer.

Because the method predicts velocities as well as positions, it works with forces that depend on velocity. Relativistic gravity in step 1.4 needs that.

Worldline computes the method's conversion tables from the 8 points at compile time, instead of copying long tables of digits, and a unit test confirms each point is a true root.

### Keeping time without drift

While validating IAS15, a test caught a bug outside the integrator: the simulation clock. Adding each step's length with ordinary rounding made the clock drift 4.5 × 10⁻¹¹ s from the true elapsed time over 36,000 steps. The bodies were correct; the clock reported the wrong time for them. Now the clock and IAS15's positions and velocities all use compensated (Kahan) summation, which carries each rounding error into the next addition instead of losing it. A unit test adds 0.1 s ten million times and requires exactly 1,000,000 s. Ordinary summation is off by 1.6 × 10⁻⁴ s.

### Limits

- Each pass of the fit evaluates the forces 7 times, so for a quick preview leapfrog is cheaper.
- The error control looks at the system as a whole (the "global" criterion). A tiny body in a tight orbit around a distant pair may get steps sized for the pair. This doesn't matter for v1's scenarios. The per-body criterion from Pham, Rein & Spiegel (2024) can replace it if needed.

### Validation

`crates/worldline-core/tests/validation_ias15.rs` and unit tests in `ias15.rs`, run by `cargo test`:

| Check | Expected | Measured |
|---|---|---|
| Energy error over 10,000 orbits (e = 0.5, 1.2 million steps) | below the rounding limit 10 ε √N = 2.4 × 10⁻¹² | 9.8 × 10⁻¹⁵ |
| Earth-like orbit (e = 0.017) back at periapsis after 100 orbits | below 10⁻⁹ AU | 7.7 × 10⁻¹³ AU (12 cm) |
| e = 0.9 orbit, same | below 10⁻⁹ AU | 3.2 × 10⁻¹² AU (48 cm) |
| e = 0.99 orbit, same | below 10⁻⁹ AU | 2.4 × 10⁻¹⁰ AU (36 m) |
| Step sizes around an e = 0.9 orbit | shortest at periapsis, longest at apoapsis | 1.9 h at periapsis, 224 h at apoapsis (115×) |
| Spring (harmonic oscillator), 1,000 oscillations | at the rounding limit | position and velocity error 3.7 × 10⁻¹³ |
| Damped spring (velocity-dependent force) vs. exact solution | rounding level | error below 10⁻¹⁴ |
| Gauss–Radau points | roots of P₇ + P₈ | each within 10⁻¹⁵ |

Why 10⁻⁹ AU for orbit closure: step 1.4 has to detect relativity moving Mercury's perihelion by about 2 × 10⁻⁵ AU over 100 orbits, so the integrator's own error must be at least 10,000× smaller.

Compare leapfrog: 50,000 steps per orbit leave a 6 × 10⁻⁷ period error on an e = 0.7 orbit. IAS15 uses about 240 steps per orbit on a harder e = 0.9 orbit and returns within 48 cm after 100 orbits.
