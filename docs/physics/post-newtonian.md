# Compact binaries: second order and gravitational waves (2PN and 2.5PN)

**Code:** `crates/worldline-core/src/gravity/post_newtonian.rs` (the gravity model), `crates/worldline-core/src/gravitational_waves.rs` (radiated power, how fast an orbit shrinks, time to merge)
**Data:** `crates/worldline-data/data/binary-orbits.csv`: the Hulse–Taylor orbit and its measured and predicted period derivative, written by `tools/write_notable_objects.py`
**Tests:** `crates/worldline-data/tests/validation_gravitational_waves.rs`, plus unit tests in both code files
**Sources:**
- **Equations of motion:** Blanchet (2024), *Living Reviews in Relativity* 27, 4, sections "The 3.5PN acceleration and 3PN energy" and "Equations of motion in the frame of the center of mass", copied from the paper's own LaTeX. The 2PN terms are cross-checked against Kidder (1995), *Phys. Rev. D* 52, 821.
- **Pairwise terms in N-body codes:** Kupi, Amaro-Seoane & Spurzem (2006), *MNRAS* 371, L45; Mikkola & Merritt (2008), *AJ* 135, 2398.
- **Orbit decay and merger time:** Peters & Mathews (1963), *Phys. Rev.* 131, 435; Peters (1964), *Phys. Rev.* 136, B1224.
- **The measurement:** Weisberg & Huang (2016), *ApJ* 829, 55.

## Why it matters

In 1974 Russell Hulse and Joseph Taylor found a pulsar orbiting an unseen neutron star every 7.75 hours. Timing its pulses over decades showed the orbit shrinking: the period loses 76 microseconds a year. General relativity predicts exactly that loss from the energy the pair radiates as gravitational waves. It was the first evidence that gravitational waves exist (Nobel Prize 1993), twenty years before LIGO heard two black holes merge.

## The model

Worldline's top-level gravity is the Einstein–Infeld–Hoffmann equations, first post-Newtonian order (1PN), for every body (see [einstein-infeld-hoffmann.md](einstein-infeld-hoffmann.md)). On top of that, each pair of bodies gets two more terms of the two-body equations of motion, in harmonic coordinates, in any frame:
- **Second order (2PN):** conservative corrections of relative size (GM/rc²)². For two neutron stars 100 km apart that is about 10⁻³ of Newton's pull; for the Sun and Mercury, 10⁻¹⁵, below double precision.
- **Radiation reaction (2.5PN):** the pull that drains the pair's energy into gravitational waves, of relative size ν (GM/rc²)^(5/2), where ν = m₁m₂/(m₁+m₂)². It is the first term with an odd power of 1/c, which is why it is not conservative: it changes sign if time runs backwards. It takes energy out of the orbit exactly as fast as Einstein's quadrupole formula says the waves carry it off.

**Pairwise, as N-body codes do it.** The pair terms are exact for two bodies. With more, the 2PN terms that couple three bodies are left out. They matter only when a third body sits close to a compact pair. Adding two-body post-Newtonian terms to N-body gravity is how N-body codes treat compact binaries (Kupi et al. 2006; Mikkola & Merritt 2008).

**Nothing changes for the solar system.** The new terms are below double precision there, so every solar-system check gives the same numbers as with the first-order equations alone.

**Formulas for what the waves do,** from the orbit as it is now (`gravitational_waves.rs`):
- **Radiated power,** Einstein's quadrupole formula for two point masses: F = (8/15) G³ m₁² m₂² (12 v² − 11 ṙ²) / (c⁵ r⁴), with ṙ the radial speed.
- **How fast the period shrinks,** averaged over an orbit (Peters & Mathews 1963): dP/dt = −(192π/5c⁵) (2πG/P)^(5/3) m₁m₂ (m₁+m₂)^(−1/3) f(e). The eccentricity factor f(e) = (1 + 73e²/24 + 37e⁴/96)/(1 − e²)^(7/2) is about 12 for the Hulse–Taylor binary: most of the power goes out near periastron.
- **Time until the pair merges** (Peters 1964): the period and eccentricity shrink together along a curve Peters found in closed form. The time is an integral along it, computed by Simpson's rule. For a circular orbit it reduces to (3/8) P/|dP/dt|.

## In the app

The inspector's model line says the top level includes the 2PN and gravitational-wave terms. Its validity grade now allows for second order:
- **Within range** while what's left out, max(GM/rc², v²/c²)³, is below IAS15's 10⁻⁹ tolerance, i.e. GM/rc² under 10⁻³.
- **Approximate** while it stays under 1%, i.e. GM/rc² under 0.2.
- **Beyond** past that.

For a body in a bound pair that will merge within the age of the universe (13.787 billion years; Planck 2020), the inspector adds a line. For the Hulse–Taylor pair, dropped from the catalogue: "its orbit's period shrinks 75.8 µs a year, and the pair merges in 300.7 million years".

![The Hulse–Taylor pulsar selected, 30 AU from the Sun: the inspector names the model, grades it within range, and shows the orbit shrinking 75.8 µs a year](../images/hulse-taylor-step-2.2.png)

To see it: `cargo run --release -- --add "PSR B1913+16 pair:30" --focus "PSR B1913+16" --zoom 100000 --paused`.

## Where it's valid

- **Weak fields:** what's left out is of third order (3PN), (GM/rc²)³, and 3.5PN, the first correction to the radiation reaction, GM/rc² of it.
- **3PN and 3.5PN** come with step 2.3, where the chirp's phase needs them. For the Hulse–Taylor binary they change the period's shrink by about 10⁻⁵ of itself, a hundred times below the measurement's uncertainty.
- **The last orbits and the merger itself** need numerical relativity: step 2.4.
- **Spins are not part of the motion.** They tilt the orbit's plane and turn the stars' axes, but drain no energy, so they don't change how fast the period shrinks.
- **Waiting for a merger in the app** isn't practical yet. The Hulse–Taylor pair turns every 7.75 hours, so 300 million years is 340 billion orbits, each computed step by step. The inspector gives the time instead.

## Validation (`cargo test`)

`cargo test --release -p worldline-data --test validation_gravitational_waves -- --nocapture` and `cargo test --release -p worldline-core post_newtonian -- --nocapture`:

| Check | Expected | Result |
|---|---|---|
| Two bodies in their center-of-mass frame: the general-frame equations (EIH plus the pair terms) against the center-of-mass equations, GM/rc² = 10⁻³ | equal up to third order. A 2PN mistake would show at (GM/rc²)², so the test passes below (GM/rc²)^(5/2) = 3.2 × 10⁻⁸ of Newton's pull | 3.4 × 10⁻¹¹ (3:1 masses), 2 × 10⁻¹⁶ (equal masses); radiation reaction 10⁻²³ |
| An eccentric pair (e = 0.5, GM/rc² = 10⁻³ at periastron) without radiation reaction keeps its 2PN energy over 10 orbits | wanders GM/rc² times less than with first order alone; the test passes below √(GM/rc²) times as much | 9.3 × 10⁻⁹ against 2.4 × 10⁻⁵ (bound 7.5 × 10⁻⁷) |
| With radiation reaction, over 20 orbits at GM/rc² = 3 × 10⁻³, the energy lost against the quadrupole formula | equal up to the next order, GM/rc² of it; the test allows √(GM/rc²) = 5.5% | 0.66% (2.2 GM/rc², and 2.5, 2.2, 1.9 GM/rc² at 10⁻², 3 × 10⁻³, 10⁻³: the next order, as expected) |
| The Hulse–Taylor binary, 1000 orbits: its period's shrink against Peters & Mathews' formula, same masses and orbit | within √(GM/rc²) = 2.4 × 10⁻³ | 4.7 × 10⁻⁵ apart |
| Against general relativity's prediction from the measured masses, −2.40263(5) × 10⁻¹² (Weisberg & Huang 2016) | within 8.5 × 10⁻⁴, what rounding the catalog's masses to 0.001 Suns allows | **−2.40221 × 10⁻¹²**, 1.7 × 10⁻⁴ apart |
| The roadmap | −2.40 × 10⁻¹² s/s | −2.40221 × 10⁻¹² |
| Against the measurement, corrected for the galaxy's pull: −2.398(4) × 10⁻¹² | (printed) | 1.1σ, as general relativity's own prediction is |
| The same pair without radiation reaction | the period holds, to better than the precision demanded above | drifts 6.3 × 10⁻¹⁷ (bound 2.0 × 10⁻¹⁵) |
| Time to merge, a circular orbit | Peters' closed form (5/256) c⁵a⁴/(G³m₁m₂m), to 10⁻¹² | matches |

**How the shrink is measured.** As pulsar astronomers do: the simulation records the moment of every periastron passage, found by Newton's method on r·v, and a parabola through the 1000 times gives the period's rate of change. By the last passage the shrinking has added up to a delay of 34 ms.

**Matching the measured orbit.** An orbit started from Kepler's laws comes out slightly different under relativity: its period 1.5 × 10⁻⁴ longer, its eccentricity 3.6 × 10⁻⁵ larger. The eccentricity matters here because f(e) is steep: left unmatched, it moves the result by 3.2 × 10⁻⁴. So the test adjusts the starting orbit until the simulated period and eccentricity (from the distances at periastron and apastron) are the measured ones.

**The bounds.** Each comes from the theory's own orders, not from guesses. Where a correct result is of one order (say (GM/rc²)³) and a mistake would be of the order before ((GM/rc²)²), the test passes below the geometric midpoint between them. The prediction is compared within what rounding the catalog's masses allows, and the roadmap's number to the digits it gives.
