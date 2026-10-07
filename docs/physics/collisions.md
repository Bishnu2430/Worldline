# Collisions and the physics-model indicator

**Code:** `crates/worldline-core/src/collision.rs`, `crates/worldline-core/src/hierarchy.rs` (detection and merging), `crates/worldline-core/src/regime.rs` (the indicator's numbers), `crates/worldline-app/src/app.rs` (the indicator)
**Tests:** `crates/worldline-core/tests/validation_collisions.rs`, `crates/worldline-data/tests/validation_sandbox.rs`, plus unit tests

## Merging

Two bodies that touch merge, **perfectly inelastically**:
- **Mass:** the masses add.
- **Position:** the merged body sits at their center of mass.
- **Velocity:** it moves with their combined momentum, so **momentum is conserved exactly**.
- **Size:** its volume is the sum of theirs, as if both had the same density (the real result depends on what they're made of; this is labeled as an assumption).
- **Energy:** the kinetic energy of their relative motion, ½ μ v² with μ = m₁m₂/(m₁ + m₂), becomes heat. A unit test checks that the energy lost is exactly that.

**Who survives:** a black hole, whatever it hits (see [compact-objects.md](compact-objects.md)); otherwise the more massive body keeps its name, and of equal masses, the one that came first. Whatever absorbs the Sun takes its place as body 0.

**Bodies placed overlapping** merge at once, before the next step: a black hole dropped into the solar system swallows everything inside its horizon.

**Planets with moons.** A planet with moons is, at the top of the hierarchy, its system's center of mass:
- **When it survives,** the impact is on the planet itself. It takes the impactor's mass and momentum, while its moons keep their exact positions and velocities and go on orbiting. The top-level body takes on the combined momentum directly, so the conservation is exact.
- **When it is absorbed,** its major moons are left behind as independent bodies orbiting the Sun. Usually they are already free by then: an impactor big enough to absorb the planet tears them away first (below). They are re-centered on their system's own center of mass, and their masses scaled to the planetary ephemeris's total for the system (JPL's two solutions differ by under 10⁻⁴). So the momentum they carry is exactly what the system carried. Its small moons go on as live particles of the swarm (see [swarm.md](swarm.md)).

**Comets and small asteroids** (the massless followers) that hit the Sun or a planet are absorbed; being massless, they change no momentum.

## Detecting contact

Checking only the ends of steps would let a fast body pass straight through another in between. So contact is checked along each step:
- **Each pair's relative motion** is interpolated between the step's ends, using a cubic Hermite curve through positions and velocities.
- **The first moment** their distance comes within the sum of their radii is found by sampling, then bisection.
- **Its speed** at that moment is recorded as the impact speed.
- **For massive bodies,** the integrator shortens its steps as they close in, so each step's ends bracket the approach well.
- **For followers,** the check runs after each of their own steps, which shorten near a body. A comet diving into the Sun is caught at the surface even when the planets' step lasts days.

## Validation (`cargo test`)

With Newtonian gravity, total momentum is conserved exactly, so before and after a collision it must match up to rounding:

| Collision | Momentum before vs. after | Other checks |
|---|---|---|
| Two Earth-like planets, one catching up at 10 km/s | 1.1 × 10⁻¹⁶ of its scale | they meet at 15.6 km/s (sped up by their pull); mass adds; radius (7³ + 6³)^(1/3) |
| A heavy impactor absorbs a planet with a moon | 1.2 × 10⁻¹⁹ | its tides free the moon first |
| A light impactor (10⁻⁵ of the planet's mass) hits a planet with a moon | 1.7 × 10⁻¹⁶ | the planet keeps its moon, still 4.0 × 10⁵ km out |
| A black hole of 10 Suns falls into the Sun from 0.1 AU | 4 × 10⁻¹⁷ | it survives as a black hole of 11 Suns, horizon 32.49 km, in the Sun's place |

**A comet falling into the Sun.** A comet falling straight at the Sun from 1 AU at 50 km/s is absorbed at the surface, not passed through:
- **Speed:** it hits at 618.3 km/s, where energy conservation, v² = v₀² + 2GM(1/R − 1/r₀), gives 618.3 km/s.
- **Time:** it hits after 24.6177 days, and the radial Kepler orbit, t = √(a³/GM)(sinh η − η), gives 24.6177 days.

**With the real solar system and relativistic gravity:** a Jupiter-mass body sent into Mars from 10⁶ km behind at 10 km/s.
- **The moons:** its tides free Phobos and Deimos 1.2 hours in; it hits Phobos at 58.29 km/s after 14.6 hours.
- **The impact:** it hits Mars after 14.7 hours at 57.49 km/s, where √(v₀² + 2GM(1/r − 1/r₀)) = 57.47 km/s, and absorbs it. Deimos is left orbiting the Sun.
- **Momentum:** total momentum changes by 1.1 × 10⁻¹¹, within the (v/c)² ≈ 4 × 10⁻⁸ by which relativistic gravity lets the Newtonian momentum wobble anyway.

**Moons torn away first.** When another body's tide on a planet's moon passes 1/12 of the planet's pull, the moons are freed to the top level, where their collisions are detected like any other (see [compact-objects.md](compact-objects.md)). A Jupiter-mass body sent into Mars frees Phobos and Deimos 13 hours before it arrives, hits Phobos, then Mars.

## Not handled yet

- **Collisions inside moon systems** (a moon hitting its planet or another moon, with nothing else near) aren't detected. When a massive body is inside a moon system without tearing its moons away, the indicator says so.
- **Fragmentation, cratering and rebounds:** every touch is a merger. Debris from giant impacts needs the fluid simulation of milestone 4.
- **The merged body's spin:** it keeps the survivor's rotation model. The impact's angular momentum isn't added to its spin yet.

## The physics-model indicator (first version)

For the selected body, the inspector shows three things.

**Which model computes it:**
- relativistic N-body gravity with the Sun and planets;
- its planet's moon system;
- following the major moons;
- JPL's mean orbit;
- or following the Sun and planets, with outgassing where it applies.

**The regime it is in,** measured from what pulls on it hardest (its planet, for a moon):
- **ε = GM/(rc²)** says how strong gravity is: 0 in flat space, ½ at a black hole's horizon.
- **v/c** says how fast it moves.

**Whether the model covers that regime.** The first post-Newtonian equations keep terms one order beyond Newton (relative size ε and v²/c²). The next order, which they leave out, is of relative size ε², ε v²/c² and v⁴/c⁴:
- **Green (within range):** ε and v²/c² both under 3 × 10⁻⁵. What's left out is below the integrator's own tolerance of 10⁻⁹.
- **Amber (approximate):** under 0.1, so what's left out stays below 1%.
- **Red (beyond the model):** gravity strong enough to need higher orders or full general relativity, as near neutron stars and black holes (milestone 2).

| Example | ε | Verdict |
|---|---|---|
| Io around Jupiter | 3.3 × 10⁻⁹ | within range |
| Mercury at perihelion | 3.2 × 10⁻⁸ | within range |
| A comet at the Sun's surface | 2.1 × 10⁻⁶ | within range |
| 10 km from a neutron star | 0.21 | beyond |

Amber notes also flag:
- small moons placed by mean orbits;
- a massive body inside a moon system, where its collisions with moons go undetected.

The full indicator, with overlays and warnings across the view, is step 3.3.
