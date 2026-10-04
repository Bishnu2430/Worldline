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

## Validation

`crates/worldline-data/tests/validation_solar_system.rs`: start from JPL's 2025 snapshot, simulate one year with Newtonian gravity and IAS15, and compare with JPL's 2026 positions.

| Body | Error after one year | Error relative to its distance |
|---|---|---|
| Mercury | 88 km | 1.3 ppm |
| Venus | 96 km | 0.9 ppm |
| Earth | 61 km | 0.4 ppm |
| Moon | 51 km | 0.3 ppm |
| Mars | 31 km | 0.1 ppm |
| Jupiter | 0.5 km | 0.001 ppm |
| Saturn, Uranus, Neptune, Pluto | ≤ 0.1 km | < 0.001 ppm |
| Sun | 0.1 km | 0.1 ppm |

The roadmap asked for Earth within 1 part in 10,000 (about 15,000 km). It lands within 61 km, 250× better. The test requires every body to be within 10 parts per million of its distance.

## What the remaining errors mean

JPL's ephemeris includes physics this Newtonian simulation leaves out:
- **general relativity**, strongest close to the Sun,
- the gravity of **asteroids**,
- the **Sun's oblateness**,
- **tides** between Earth and the Moon.

The errors follow relativity's signature: tens of km for the inner planets and well under 1 km for the outer ones. The size fits too. Relativity rotates Mercury's orbit by about 0.43″ per year, which moves it roughly 120 km, the same scale as its 88 km error.

**Prediction for step 1.4:** adding relativistic (Einstein–Infeld–Hoffmann) gravity should shrink the inner planets' errors well below these values. Whatever remains then should be mostly asteroids.
