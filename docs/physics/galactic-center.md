# The galactic center: Sagittarius A* and the S-stars

**Code:** `crates/worldline-data/src/galactic_center.rs` (positions and orbits), `crates/worldline-app/src/simulation.rs` (the region)
**Data:** `crates/worldline-data/data/s-stars.csv` and `sgr-a-star-position.csv`, written by `tools/write_galactic_center.py`
**Tests:** `crates/worldline-data/tests/validation_galactic_center.rs`, plus unit tests

![Sagittarius A* and the 39 stars with published orbits around it, colored by type: young, hot stars blue-white, cool giants orange](../images/galactic-center-step-2.1c.png)

To see it: `cargo run --release -- --focus "Sagittarius A*"`, or double-click Sagittarius A\* in the body list. The camera flies out from the solar system and in again.

## Where it is

**Distance and direction.** Sagittarius A\*, the Milky Way's central black hole, sits 8,277 pc (27,000 light-years) away. That's GRAVITY's distance, from the same orbits that give its mass of 4.297 million Suns (GRAVITY Collaboration 2022).
- **Its position on the sky** comes from VLBI radio astrometry: right ascension 17h 45m 40.034047s, declination −29° 00′ 28.21601″ (ICRF3; Gordon, de Witt & Jacobs 2023).
- **Into the simulation's frame:** that position is turned from the equator into the ecliptic of J2000.

**A region of its own.** The galactic center is simulated separately from the solar system, on the same clock:
- **No pull between them:** 8,277 pc apart, each one's tide on the other is below 10⁻²⁰ m/s², so neither needs to feel the other.
- **Bodies dropped near it** join its region and feel it.

**Held still.** The region holds still relative to the solar system. In reality the galaxy's mass carries the Sun around the galactic center every 230 million years, at about 250 km/s. Simulating that needs the galaxy's stars, gas and dark matter, which come in v2, and the app says so. Nothing is drawn between the two regions for the same reason.

## The stars around it

39 stars with published orbits:
- **S2, S29, S38 and S55** from GRAVITY's 2022 four-star fit, the most precise orbits known.
- **35 more** from Gillessen et al. 2017. S111 is left out: its path is unbound (hyperbolic).

**From orbit to position.** Each orbit is given by its size on the sky (a, in arcseconds, converted with the paper's own distance), shape (e), orientation (i, Ω, ω) and pericenter time. The Thiele–Innes constants turn it into offsets north, east and along the line of sight (Wright & Howard 2009), the line of sight pointing away from us, as the papers define it. S2 then comes out with the radial-velocity swing GRAVITY measured around its 2018 pericenter: about +4,000 km/s down to −2,000 km/s.

**Osculating orbits.** GRAVITY's orbits include relativity and are osculating at an earlier date (2010.35 for S2). Those stars are placed there and run forward to the 2025 snapshot with the region's own gravity. Gillessen's are Keplerian fits, placed on their ellipses directly.

**Their gravity.** The stars are massless here (S2 is about 14 Suns against 4.3 million). Each follows Sagittarius A\*'s Newtonian pull plus its first relativistic correction for a test body: the Schwarzschild term, which turns S2's orbit 12′ each time round.

## Validation (`cargo test`)

`cargo test --release -p worldline-data --test validation_galactic_center -- --nocapture`:

| Check | Expected | Result |
|---|---|---|
| Sagittarius A\*'s direction, turned into galactic coordinates (the Hipparcos catalogue's rotation) | SIMBAD's l = 359.94423568°, b = −0.04616002°, from a radio position under 0.05″ from ours: within 0.0001° | l = 359.94421485°, b = −0.04615777° |
| S2's next pericenter after 2018.3789 | one Keplerian period later (16.045 years), within what first-order relativity changes in the timing: GM/(r_p c²) of a period, 2.08 days | 1.18 days off, in 2034.43 |
| S2's pericenter | GRAVITY: about 120 AU, about 7,700 km/s | 119.4 AU at 7,749 km/s |
| S2's orbit turning, simulated over ten orbits | GRAVITY measured 0.997 ± 0.144 times general relativity's 12.20′ per orbit | 12.26′ per orbit, 1.005 times |

The simulated turning is 0.5% above the first-order formula. That's the size of the next order for an orbit this eccentric; the Hulse–Taylor binary shows the same in [compact-objects.md](compact-objects.md). It is well within GRAVITY's measurement.

## Not handled yet

- **The galaxy:** the Milky Way's stars, gas and dark matter, and the Sun's orbit around the center (v2).
- **The S-stars' own masses and sizes:** they are points, which their effect on each other allows.
- **Exact black-hole motion:** S2 never comes within 1,400 horizon radii, where first-order relativity is accurate, but anything dropped closer needs step 2.5.
- **The other catalog objects** (M87\*, TON 618, Cygnus X-1…) stay in the catalogue, to be dropped where you like, not at their real places.
