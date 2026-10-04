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

- **Fixed step.** A step small enough for a comet's fast pass near the Sun is wasted on the slow far end of its orbit. On very eccentric orbits, leapfrog loses accuracy (see the comet row below). It also adds a small artificial precession that grows with step size, which would swamp Mercury's 43″ per century. Step 1.2 adds IAS15, an adaptive high-order integrator, as the accurate default. Leapfrog remains the fast preview integrator.
- **Position-dependent forces only.** It assumes acceleration depends only on positions, so it is not used with relativistic gravity, which also depends on velocities.

### Validation

`crates/worldline-core/tests/validation_kepler.rs` and unit tests, run by `cargo test`:

| Check | Expected | Measured |
|---|---|---|
| Energy error band, orbits 1–100 vs. 1901–2000 (e = 0.5, 1000 steps per orbit) | no growth | 1.073 × 10⁻⁴ in both |
| Error ratio when the step halves | 4 (second order) | 4.000 |
| Run 5,000 steps forward, reverse velocities, run 5,000 back | returns to start | yes, within rounding |
| Comet-like orbit (e = 0.7) period, 50,000 steps per orbit | Kepler's third law | relative error 6 × 10⁻⁷ (vs. ~10⁻⁸ for near-circular orbits) |
