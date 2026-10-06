# Solar-system data

**Code:** `crates/worldline-data/` · **Data:** `crates/worldline-data/data/*.csv` · **Fetch script:** `tools/fetch_solar_system.py`

## Where the numbers come from

| What | Source |
|---|---|
| Positions and velocities | NASA JPL Horizons, ephemeris DE441: JPL's fit to radar, spacecraft tracking and lunar laser ranging data |
| Gravitational parameters (GM) | JPL DE440 (Park et al. 2021, *AJ* 161, 105), from NAIF's `gm_de440.tpc` |
| Radii | Mean radii from the IAU Working Group on Cartographic Coordinates (Archinal et al. 2018). Sun: IAU 2015 nominal value. |

The fetch script copies JPL's numbers into the files exactly as published, with every digit, and each file's header records its sources and the date it was fetched. Rerunning `python tools/fetch_solar_system.py` regenerates them.

## Conventions

- **Time scale:** Barycentric Dynamical Time (TDB), the time JPL's ephemerides use. Simulation seconds are TDB seconds.
- **Frame:** ICRF axes, aligned with the ecliptic and mean equinox of J2000, so planets orbit close to the x–y plane. The origin is the solar system barycenter.
- **Bodies:** the Sun, Mercury, Venus, Earth, the Moon, and the system barycenters of Mars, Jupiter, Saturn, Uranus, Neptune and Pluto. A system barycenter is a planet plus its moons treated as one point at their combined center of mass. JPL's own ephemerides model the outer planets the same way. Earth and the Moon are separate, because the Moon is large and close enough that their separate motions matter.
- **Epochs:** 2025-01-01 00:00 TDB (the starting snapshot) and exactly one Julian year later, 2026-01-01 06:00 TDB (the reference for validation).
- **Moons:** the 21 major moons, with their planets' centers and gravity fields, come from a separate script, `tools/fetch_moons.py` (see [moons.md](moons.md)). The other 437 known moons come from `tools/fetch_small_moons.py` (see [small-moons.md](small-moons.md)).
- **Dwarf planets, asteroids and comets:** 62 of them, from `tools/fetch_small_bodies.py` (see [small-bodies.md](small-bodies.md)).

## Validation

`crates/worldline-data/tests/validation_solar_system.rs`: start from JPL's 2025 snapshot, simulate one year with IAS15, and compare with JPL's 2026 positions. The test runs twice, with Newtonian gravity and with relativistic (Einstein–Infeld–Hoffmann) gravity.

| Body | Newtonian error | Relativistic error | Improvement |
|---|---|---|---|
| Mercury | 88 km | 0.25 km | 345× |
| Venus | 96 km | 0.13 km | 764× |
| Earth | 61 km | **0.41 km** | 149× |
| Moon | 51 km | 16 km | 3× |
| Mars | 31 km | 0.19 km | 164× |
| Jupiter | 0.5 km | 0.14 km | 4× |
| Saturn, Uranus, Neptune, Pluto | ≤ 0.1 km | ≤ 0.1 km | about 1× |
| Sun | 0.1 km | 0.1 km | 1× |

The roadmap asked for Earth within 1 part in 10,000 (about 15,000 km). With relativity it lands within 410 m after a full year. The test requires:
- every body within 10 parts per million of its distance, under both models;
- each inner planet's error cut at least 10× by relativity.

## What the remaining errors mean

With Newtonian gravity alone, the errors followed relativity's signature: tens of km for the inner planets and well under 1 km for the outer ones. Step 1.3 predicted that adding relativity would shrink them, and step 1.4 confirmed it, by factors of 150 to 760.

What remains is physics JPL models that Worldline doesn't yet:
- **asteroids,** whose pull matters most for Mars. Step 1b.6 added the 22 heaviest, which cut Mars's error from 0.19 to 0.11 km and Earth's from 0.41 to 0.21 km (see [small-bodies.md](small-bodies.md#validation));
- **the Sun's slight flattening;**
- **for the Moon,** Earth's equatorial bulge and the tides between Earth and the Moon. These explain why the Moon improves only 3×.

See [einstein-infeld-hoffmann.md](einstein-infeld-hoffmann.md).
