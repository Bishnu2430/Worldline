# The belts: the asteroid belt, Jupiter's Trojans and the Kuiper belt

**Code:** `crates/worldline-core/src/orbit.rs` (`KeplerOrbit`), `crates/worldline-data/src/belts.rs`, `crates/worldline-app/src/simulation.rs` and `view.rs` (drawing)
**Data:** `crates/worldline-data/data/{asteroid-belt,jupiter-trojans,kuiper-belt}.csv` · **Fetch script:** `tools/fetch_belts.py`
**Tests:** `crates/worldline-data/tests/validation_belts.rs`, plus unit tests

![The asteroid belt (tan) and Jupiter's Trojans (gold) on 1 January 2025, with Jupiter at the top: the Trojan clouds sit 60° to either side of it](../images/belts-step-1b.7.png)

To see it: `cargo run --release -- --focus Sun --zoom 2500` (the camera 2,500 solar radii, about 12 AU, from the Sun).

## What's included

28,331 real orbits from JPL's Small-Body Database (SBDB), each body's osculating elements as JPL publishes them, in full precision:

| Belt | Count | Selection |
|---|---|---|
| Asteroid belt | 19,971 | Every main-belt asteroid (SBDB classes inner, middle and outer belt, including the Hildas and Cybeles) brighter than absolute magnitude H = 14, roughly larger than 5–10 km |
| Jupiter's Trojans | 4,509 | Every Jupiter Trojan brighter than H = 14 |
| Kuiper belt | 3,851 | Every trans-Neptunian object with a well-determined orbit (JPL orbit condition code 5 or better) |

**Why a brightness cut.** About 1.4 million main-belt asteroids are known, far more than the app could draw smoothly. Surveys have found essentially every asteroid brighter than H ≈ 14. So this sample has no bias toward whatever is easiest to see, which matters for statistics like the Kirkwood gaps.

**Overlap.** The 62 bodies simulated individually since step 1b.6 (Ceres, Vesta, Patroclus, Eris…) are left out, so nothing is drawn twice.

## How they move: fixed ellipses

The belts are massless test particles. They can't pull on anything, and 28,000 of them are far too many to integrate in real time. So each rides a fixed **Keplerian ellipse** around the Sun, from its JPL elements (`KeplerOrbit`):
- **One equation per body.** Placing a body costs one solution of Kepler's equation, E − e sin E = M. Each orbit's orientation is computed once, up front.
- **Cost.** All 28,331 take about 3.4 ms on one core. The app shares them among all cores and updates them once per frame.
- **Frame.** Positions are relative to the Sun's simulated position.

This is "detail follows focus" (see [ARCHITECTURE.md](../ARCHITECTURE.md)). The belts are background; the 62 named small bodies get the full N-body treatment.

**How approximate.** A fixed ellipse leaves out the planets' pull. Jupiter's is the strongest, about 1/1,000 of the Sun's (their mass ratio), so over less than an orbit a body should stray by less than that fraction of its distance. The test runs this two-body model against JPL for a year, on the 44 asteroids, dwarf planets and trans-Neptunian objects of step 1b.6:
- the median drift is **1.6 × 10⁻⁴** of the body's distance;
- the worst is Davida at 8.2 × 10⁻⁴;
- all stay under Jupiter's 9.55 × 10⁻⁴.

At the belt's distance, the median is about 65,000 km after a year, a fraction of a pixel at the scales where belts are drawn. Over decades the drift grows, and the app labels belt positions as a model.

## What the real orbits show

The point of using real catalogs is that real structure appears without being put in. Planets' resonances carve the belts, and Worldline checks that the catalog shows it.

**How significant.** Each test counts the orbits within 0.2% of a resonance's semi-major axis. It compares that with what a smooth belt would put there, judged from bands 2–4% away on both sides. The difference is then measured in standard deviations of chance (Poisson noise). The bar is 5σ, the conventional standard for a discovery.

**Where resonances fall.** A resonance's location follows from Kepler's third law: an orbit taking p/q of a planet's sidereal period (NASA fact sheets: Jupiter 4,332.589 days, Neptune 60,189 days) has a = (GM☉ T² / 4π²)^(1/3).

### Kirkwood gaps (`cargo test`)

An asteroid whose period is a simple fraction of Jupiter's meets Jupiter at the same points of its orbit, again and again. The repeated tugs add up, its orbit turns chaotic, and it is eventually thrown out of the belt (Wisdom 1982 showed how, for the 3:1). Daniel Kirkwood noticed the gaps in 1866, with fewer than 100 asteroids known.

| Resonance | a (AU) | Observed | Smooth belt would have | Significance |
|---|---|---|---|---|
| 3:1 | 2.5005 | 0 | 180.3 | −13.4σ |
| 5:2 | 2.8236 | 7 | 152.2 | −11.8σ |
| 7:3 | 2.9565 | 50 | 234.9 | −12.1σ |
| 2:1 | 3.2765 | 2 | 302.0 | −17.3σ |

### Resonances that protect (`cargo test`)

Some resonances do the opposite. Their geometry keeps the asteroid away from the planet whenever they pass, so the resonance shelters it:

| Group | Resonance | a (AU) | Observed | Smooth belt would have | Significance |
|---|---|---|---|---|---|
| Hildas | Jupiter 3:2 | 3.9692 | 116 | 0.7 | +138σ |
| Plutinos | Neptune 3:2 | 39.386 | 112 | 13.9 | +26σ |

- **The Hildas** go round three times for every two of Jupiter's. Seen from above, as a group they trace a triangle with Jupiter.
- **Pluto and the Plutinos** go round twice for every three of Neptune's. That keeps them from ever meeting it, even though Pluto crosses Neptune's orbit. Many were probably swept into the resonance as Neptune migrated outward (Malhotra 1993, *Nature* 365, 819).
- **Plutinos are easier to find.** Their perihelia come close to Neptune's distance, which makes them easier to discover than other Kuiper belt objects. But a 26σ excess is far beyond what that bias explains.

### Jupiter's Trojans (`cargo test`)

Lagrange showed in 1772 that a small body sharing a planet's orbit can stay put 60° ahead of the planet (the L4 point) or 60° behind it (L5). There, it sits at the corners of equilateral triangles with the Sun. Real Trojans swing around these points. The test compares each Trojan's mean longitude with Jupiter's at the 2025 snapshot, and requires ±60° to lie within the middle half (the interquartile range) of each cloud:

| Cloud | Trojans | Median | Middle half |
|---|---|---|---|
| L4 (ahead) | 2,684 | +62.0° | +54.2° to +70.9° |
| L5 (behind) | 1,825 | −61.9° | −70.6° to −54.1° |

More Trojans lead Jupiter than trail it, an asymmetry surveys have long noted and that isn't fully explained.

## Drawing

- **Markers.** Each body is a tiny square (1.6 points), colored by belt: tan for the main belt, gold for the Trojans, ice blue for the Kuiper belt. All 28,331 go into one mesh per frame.
- **When they show.** Belts are drawn only when the camera is more than 0.05 AU from what it looks at. Zoomed in on a planet, their markers would scatter across the sky like stars that aren't there.
- **Not clickable.** The belts aren't in the body list and can't be selected; the 62 named small bodies can.

## Limits

- **Fixed ellipses.** There are no planetary perturbations, so positions drift over years (see above). Resonant structure is in the catalog's elements, not produced by the drawing.
- **A brightness-limited sample.** Smaller asteroids, which are the vast majority, aren't drawn. The Kuiper belt sample is whatever surveys have found and tracked well enough, which favors bodies that come closer to the Sun.
- **No collisional families or shapes.** Every body is a point.
