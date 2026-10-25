# Fluids: smoothed particle hydrodynamics (SPH)

**Code:** `crates/worldline-core/src/sph.rs`

**Tests:** `crates/worldline-core/tests/validation_sph.rs`, plus unit tests in `sph.rs`

**Sources:**
- **The method:** Price (2012), *J. Comput. Phys.* 231, 759, "Smoothed particle hydrodynamics and magnetohydrodynamics":
  - the M4 cubic spline kernel (eq. 6);
  - the smoothing length set by the density, with η = 1.2 (eq. 10, and his recommendation in §5);
  - the "grad-h" term Ω (eqs. 27–28);
  - the equation of motion (eq. 30);
  - the internal energy equation (eq. 35);
  - Monaghan's (1992) artificial viscosity as his eq. 98 gives it (α = 1, β = 2, ε = 0.01);
  - Morris & Monaghan's (1997) switch, his eq. 105 (σ = 0.1, α_min = 0.1).
- **The cap on the switch, α_max − α:** Rosswog et al. (2000), *A&A* 360, 171.
- **Gravity softened by the kernel:** Price & Monaghan (2007), *MNRAS* 374, 1347.
- **Time steps:** Monaghan (1992), *ARA&A* 30, 543, with Price et al.'s (2018, *PASA* 35, e031) coefficients 0.3 and 0.25.
- **The polytrope:** Lane–Emden for n = 1 (e.g. Chandrasekhar 1939, *An Introduction to the Study of Stellar Structure*, ch. IV).
- **Jupiter:** IAU 2015 nominal GM and radius.

## Why it matters

So far every body has been a point, or a rigid shape. But a planet that passes too close to a star or a black hole is stretched by tides until it deforms, sheds mass or is torn apart, and planets that collide splash. To show that, a body has to become a fluid. This step builds that fluid, on the CPU in double precision. Later steps:
- **4.2** moves it to the GPU for many more particles.
- **4.3** turns a point into a fluid when tides start to matter.
- **4.4** tears one apart.
- **6.4** collides planets as fluids.

## The method

SPH represents a fluid as particles. Each carries a mass, a velocity, an internal energy per unit mass and a **smoothing length**, h, the size of the region it represents.
- **Kernel:** a particle's mass is spread out by a bell-shaped kernel W over 2h, the M4 cubic spline.
- **Density:** a point's density is the sum of its neighbours' spread-out masses, ρ_a = Σ m_b W(r_ab, h_a).
- **Smoothing length:** each particle's h follows its density, h = 1.2 (m/ρ)^(1/3). That gives about 58 neighbours everywhere, so dense regions get fine resolution and sparse ones coarse. Since ρ depends on h and h on ρ, the two are solved together by Newton's method, to 10⁻⁶ of h.
- **Pressure:** the gas is ideal, P = (γ − 1)ρu.
- **Forces and heating:** the equations of motion and of internal energy are derived from a Lagrangian. So they keep momentum and angular momentum exactly, and energy to the time steps' error.
  - **Pressure:** dv_a/dt = −Σ_b m_b [P_a/(Ω_a ρ_a²) ∇W_ab(h_a) + P_b/(Ω_b ρ_b²) ∇W_ab(h_b)], where Ω corrects for h changing with density.
  - **Heating:** du_a/dt = P_a/(Ω_a ρ_a²) Σ_b m_b (v_a − v_b)·∇W_ab(h_a).
- **Shocks:** an artificial viscosity (Monaghan's) acts between particles closing in on each other, and heats them as much as it slows them.
  - **The switch:** its strength α is each particle's own (Morris & Monaghan). It decays to 0.1 over a few smoothing lengths' sound-crossing times, and grows toward 1 where the gas is compressed (−∇·v > 0).
  - **Why:** without it, the viscosity heats the particles' small jiggles in a calm fluid, and the test star slowly swelled: 2% in ten dynamical times.
- **Gravity:** every particle pulls on every other, its mass spread out as the kernel, so two never pull infinitely hard.
  - **Force:** particle b pulls on a with G m_b M(r/h)/r². M(q) is the share of the kernel's mass within q, averaged over the two particles' h so that momentum is kept.
  - **Potential:** −G m ψ(q)/h, with ψ(0) = 7/5.
  - **How it's computed:** directly, pair by pair, on all the computer's cores. That is right for thousands of particles; 4.2 moves to the GPU for more.
- **Time steps:** leapfrog (kick–drift–kick), all particles together. Each step is the smallest over the particles of:
  - **sound crossing:** 0.3 h/(c + 1.2α(c + 2μ));
  - **force:** 0.25 √(h/|a|).

## Building a star or planet

**The polytrope:** an n = 1 polytrope (P = Kρ², γ = 2) has an exact structure: ρ = ρ_c sin ξ/ξ, ξ = πr/R, ρ_c = πM/(4R³).
- **Radius:** every one with a given K has the same radius, R = π√(K/2πG), whatever its mass.
- **Why Jupiter:** this is why gas giants from a Saturn's mass to a few Jupiters' are about the same size, and Jupiter is close to one: its central density comes out at 4.4 g/cm³.

**Placing the particles:** they start on a cubic lattice in a sphere, each moved out along its radius until the mass inside matches the polytrope's. Each is given the polytrope's energy, u = Kρ.

**Settling:** that start isn't quite SPH's own equilibrium, so `settle` lets it relax for five dynamical times, √(R³/GM), at the polytrope's entropy.
- **During it,** motion is damped with a time constant of 0.3 of one, and each energy is reset to Kρ after every step.
- **Then** it is set at rest and left alone, its energy now evolving freely.

## Where it's valid

- **Resolution:** SPH resolves nothing finer than a smoothing length. With 1,000 particles that is about a sixth of the radius, with 4,000 a ninth. A structure's errors shrink as h², faster in practice here.
- **Noise:** its particles jiggle around their places by a fraction of a smoothing length. That is SPH's noise, about 10⁻⁴ of the binding energy in motion here.
- **Equations of state:** only an ideal gas so far. Rock and iron (the Tillotson equation of state) come with giant impacts in 6.4.
- **Gravity is Newtonian,** and computed directly, pair by pair. Thousands of particles are fine on the CPU; tens of thousands are 4.2's.
- **Not yet in the app.** A body becomes a fluid in step 4.3.

## Validation (`cargo test`)

`cargo test --release -p worldline-core --test validation_sph -- --nocapture --include-ignored`:

| Check | Expected | Result |
|---|---|---|
| **The roadmap's check:** Jupiter as an n = 1 polytrope of 968 particles, settled, then left alone for ten dynamical times (about 4.6 hours) | It stays in equilibrium, to what SPH resolves (mean h = 0.176 R). Its half-mass radius stays within h/2 of where it started, and its kinetic energy under that of an oscillation that size, (h/2R)² = 7.8 × 10⁻³ of the binding energy. Without pressure it would fall in within one dynamical time | moved at most 0.0206 R (0.12 h); kinetic energy at most 1.6 × 10⁻⁴ of the binding energy |
| Its energy over those ten dynamical times | kept to the leapfrog steps' error, (Δt/t_dyn)² = 1.4 × 10⁻³ | 4.1 × 10⁻⁵ |
| Its structure | the polytrope's: its half-mass radius within a smoothing length of Lane–Emden's 0.6066 R | 0.6186 R |
| With 4,032 particles (slow; ignored by default) | closer to Lane–Emden by at least 0.4, as h² shrinks | 0.6076 R, 0.0010 R off against 0.0120 R: 12 times closer |

Also `cargo test -p worldline-core sph`: the kernel's mass adds up to 1, the softened potential's slope is the softened pull, ψ(0) = 7/5 and ψ = 1/q beyond 2h, and w′ is w's slope.
