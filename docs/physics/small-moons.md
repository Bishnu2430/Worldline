# Small moons: every known moon, computed in detail where you look

**Code:** `crates/worldline-core/src/mean_elements.rs`, `crates/worldline-core/src/hierarchy.rs` (small moons), `crates/worldline-data/src/small_moons.rs`, `crates/worldline-app/src/simulation.rs` (detail follows focus)
**Data:** `crates/worldline-data/data/{moon-elements,small-moons-2025-01-01,small-moons-2025-01-31,inner-moon-masses}.csv` · **Fetch script:** `tools/fetch_small_moons.py`
**Tests:** `crates/worldline-data/tests/validation_small_moons.rs`, plus unit tests in the core

## What "every known moon" means

JPL's Solar System Dynamics group keeps the list of planetary satellites with computed orbits, with their mean orbital elements ([ssd.jpl.nasa.gov/sats/elem](https://ssd.jpl.nasa.gov/sats/elem/)). Worldline loads every moon on it.

| Planet | In JPL's list | Major moons | Small moons |
|---|---|---|---|
| Earth | 1 | the Moon | 0 |
| Mars | 2 | 2 | 0 |
| Jupiter | 115 | 4 | 111 |
| Saturn | 291 | 8 | 283 |
| Uranus | 29 | 5 | 24 |
| Neptune | 16 | 1 | 15 |
| Pluto | 5 | 1 | 4 |
| **Total** | **459** | **22** | **437** |

JPL's table has 460 rows: Puck appears twice, once from each of two Uranus solutions. The fetch script keeps the entry from the solution Horizons serves.

Most of them are small irregular moons, a few kilometers across, found since 2000 on distant, tilted and often backward orbits. Saturn alone gained 128 in 2025.

## Two ways to place a small moon

Small moons are too light to matter to anything else. The heaviest, Neptune's Proteus, has 1/550 of Triton's mass and 4 × 10⁻⁷ of Neptune's. So Worldline computes them only as carefully as the view needs. This is the "detail follows focus" principle (see [ARCHITECTURE.md](../ARCHITECTURE.md)).

### Out of focus: JPL's state, carried along at JPL's mean rates

For each moon JPL publishes an average ellipse, its mean elements:
- **Shape:** size and shape (a, e).
- **Orientation:** tilt and node (i, Ω), and periapsis (ω).
- **Phase:** mean anomaly (M) at an epoch.
- **Timing:** the sidereal period P, and the periods at which the periapsis and node precess.

Advancing the angles at those steady rates places the moon at any date, at negligible cost (`MeanElements`):
- **The moon's direction around the planet** repeats every sidereal period: Ṁ = 2π/P − ω̇ − Ω̇·sign(cos i). So the precession can't throw off where the moon is along its orbit.
- **The speed** follows Kepler's laws, with the effective gravity μ = (2π/P)² a³ that the observed period implies. This already includes the planet's flattening.
- **Precession directions.** JPL lists the precession periods without signs, so Worldline takes the directions from the physics that drives them:
  - **The node** regresses for prograde orbits and advances for retrograde ones, as −cos i. This holds for both a planet's flattening and the Sun's tide (Murray & Dermott 1999, eq. 6.249; Kozai 1962).
  - **The periapsis** advances. J2 drives it forward below 63.4° and above 116.6° inclination, and the solar tide drives it forward whenever it circulates. Where it doesn't circulate (Kozai librators such as Margaret), JPL gives no period and Worldline holds it fixed.
- **Reference frames.** Elements are given relative to one of three planes:
  - **ecliptic:** the J2000 ecliptic, which is Worldline's own frame;
  - **Laplace:** the moon's local Laplace plane, whose pole JPL lists;
  - **equatorial:** the planet's equator, about its spin axis.

  In each case the node is measured from that plane's ascending node on the ICRF equator.

Most of these elements are dated 2000. A quarter century of propagating a period rounded to a few digits loses track of where along its orbit a moon is. An irregular moon's real ellipse also wanders far from the long-term average. So where JPL has the moon's exact state at the 2025 snapshot (435 of the 437), Worldline starts from it:
- **Shape:** the ellipse through that state (the osculating ellipse).
- **Motion:** carried round once per JPL's sidereal period, with its periapsis and node precessing at JPL's rates.

So the approximation starts exact and drifts slowly. A month on, it is typically 0.27% of the moon's distance from JPL's position (median; 1.5% at the 90th percentile). The worst cases are moons whose orbits change quickly, such as the co-orbitals Janus and Epimetheus. The app labels these positions as approximate.

### In focus: computed in detail

When you fly to a planet or any of its moons, its small moons switch to full simulation. They start from JPL Horizons' exact states at the 2025 snapshot, or from their approximate orbits if time has moved on. Each small moon feels:
- the planet's gravity field (oblateness, and Mars's lumps);
- the major moons;
- the tides of the Sun and planets;
- the other small moons that have measured masses.

Small moons don't pull on the planet or the major moons. That keeps the major moons' integration, validated in step 1b.4, exactly as it was. Physically they are test particles: massless as far as the major moons are concerned.

**Following the major moons.** The major moons are integrated first, and every step is recorded: positions, velocities and accelerations. The small moons then follow those recorded paths. Between records, positions come from quintic Hermite interpolation, accurate to about (ωh)⁶ a / 46,080 for an orbit of radius a and angular rate ω over a step of length h. That is meters at Io.

**Two kinds of integrator, following JPL's own classification.**
- **Regular moons** share one integrator, so the ones with measured masses pull on each other. JPL gives their elements relative to a Laplace or equatorial plane. They include ring moons, co-orbitals, Trojans, and Pluto's small moons, which tug on each other noticeably because Pluto's system is so light.
- **Irregular moons** each get their own integrator and pace. JPL gives their elements relative to the ecliptic, and they are thousands of moon radii apart, so their pulls on each other are negligible. A moon that takes two years to orbit takes long steps instead of being dragged along at Pan's 14-hour pace.

**Each solution's own constants.** As with the major moons, small moons are computed with the constants of the JPL solution their states come from. Two cases matter:
- **States relative to the planet's center.** These are fetched directly from Horizons. Saturn's ring moons come from SAT415, built on the older planetary ephemeris DE437. Subtracting Saturn's position from a different solution misplaced Pan by 117 km, which changed its orbital speed by 0.1% and sent it 54,000 km off JPL in a month.
- **The oblateness the moons' solution fits.** Uranus's inner moons come from URA184, which fits J2 = 4.036 × 10⁻³, against URA182's 3.509 × 10⁻³ for the major moons (`small-moon-zonal.csv`). With the major moons' J2 these moons drifted 3,000–12,000 km in a month, exactly as much as the 15% difference predicts. With their own J2 they drift 1–5 km.

**Inner moons' mass.** JPL's integration of the major moons includes some small inner moons with their masses: Neptune's six inner moons, Puck, and Jupiter's Amalthea group. To the major moons, they pull like extra mass at the planet's center, so that is how the major moons feel them (`inner-moon-masses.csv`). Simulated small moons feel them one by one instead.

## Where it's valid

- **Mean orbits:**
  - good for where a moon is around its planet at a glance;
  - not for timing an eclipse or a close approach.
- **Detailed simulation:**
  - **Physics:** the same as for the major moons: Newtonian gravity in the moon system's frame, with the planet's field and outside tides.
  - **What's left out:** the small moons' pull on the major moons; the shapes of small moons; ring particles' gravity beyond the rings' total mass.
- **Moons without a 2025 ephemeris.** A few listed moons have no JPL ephemeris covering 2025; Horizons stops at their last observation. These are placed and started from their published mean elements alone:
  - Daphnis: Horizons has no ephemeris after 17 January 2018.
  - S/2025 U 1: discovered in 2025; Horizons doesn't list it as a moon yet.

## Validation

`cargo test -p worldline-data --test validation_small_moons -- --nocapture`:

**Every moon in JPL's list loads.** There are 459, each with a mean orbit:
- the Moon;
- the 21 major moons;
- 437 small moons, 435 of which have JPL's exact 2025 state.

**Spot checks after 30 days.** Every small moon with a 2025 state is simulated in detail for 30 days and compared with JPL. The 48 whose size the IAU or JPL has measured are held to the major moons' criterion: closer to JPL than their own radius. Highlights:

| Group | Error after 30 days |
|---|---|
| Jupiter's inner moons (Metis, Adrastea, Amalthea, Thebe) | 1–6 km |
| Jupiter's irregular moons (Himalia, Elara, Pasiphae, Sinope …) | 0.07–0.22 km |
| Saturn's ring moons (Pan, Atlas, Prometheus, Pandora) | 0.7–8 km |
| Saturn's Trojans (Telesto, Calypso, Helene, Polydeuces) | 0.3–6.5 km |
| Phoebe | 0.06 km |
| Uranus's inner moons (Cordelia to Puck) | 1.3–4.4 km |
| Neptune's inner moons (Naiad to Proteus), Nereid | 3.5–28 km |
| Nix, Hydra | 0.5, 7.9 km |

41 of the 48 pass. The other seven are listed in the test as known residuals, so every other moon is held to the criterion and the list can't silently go stale:

| Moon | Error | Radius | Notes |
|---|---|---|---|
| Epimetheus | 62 km | 58 km | Janus, its co-orbital partner, is 9 km off. Not yet explained. |
| Aegaeon, Methone, Anthe, Pallene | 3–10 km | 0.4–2.3 km | Moonlets near Mimas, whose own position differs from JPL's by 13 km (see [moons.md](moons.md)) |
| Styx, Kerberos | 5.6, 6.9 km | 5.2, 6.0 km | Not yet explained. Using the masses from Pluto's own solution (PLU060) changes little. |

Every irregular moon checked lands within 0.3 km of JPL, a few millionths of its distance from the planet.

**Janus and Epimetheus trade orbits** (slow; `cargo test --release -- --ignored`). Their semi-major axes cross on day 394 after the snapshot, 30 January 2026. Their observed close approaches came in January 2006, 2010, 2014, 2018 and 2022, so January 2026 was due. Only their own masses can do this, so it confirms that small moons pull on each other.

**JPL's mean elements and Worldline's conventions.** The published elements are checked against JPL's states where JPL dates them at the snapshot itself: Uranus's inner moons.
- **Orbital planes:** agree within 0.02–0.06°, inside the 0.12° that the published digits allow.
- **Distances from Uranus:** agree within the planet's short-period wobble.
- **Phase:** every one of these moons sits as far along its orbit as 14.3 minutes of its motion. That is a uniform offset in the published elements, not a frame error, and it is printed for the record.

**In the core:**
- **A small moon at a major moon's L4 point** stays in its tadpole orbit (59.998°–60.002° from the major moon over 100 orbits), as the restricted three-body problem requires.
- **A lone small moon** keeps Kepler's period to 0.001°.
- **An osculating ellipse** passes back through the state it came from.

## Cost

One simulated year takes, in a release build:
- **No small moons:** 1.7 s.
- **One planet's small moons in detail:** 1.6–5.8 s, Saturn's 283 being the most.

At the default speed (a month per second) that fits within the 12 ms per-frame physics budget. Small moons out of focus cost almost nothing: each is placed directly from its orbit formula once per frame.
