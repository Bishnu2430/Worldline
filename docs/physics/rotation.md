# Rotation: how bodies are oriented and spin

**Code:** `crates/worldline-core/src/rotation.rs` (model), `crates/worldline-data/src/rotation.rs` and `kernel.rs` (data)
**Data:** `crates/worldline-data/data/pck00011-subset.tpc` and `nh_pcnh_010-subset.tpc` (Pluto and Charon) · **Fetch script:** `tools/fetch_rotation.py`
**Also:** each body's triaxial shape (`BODYnnn_RADII`) comes from the same kernel; a unit test checks Jupiter's and Saturn's flattening (0.06487, 0.09796) against NASA's fact sheet.
**Sources:** the IAU Working Group on Cartographic Coordinates and Rotational Elements: Archinal et al. (2018), *Celestial Mechanics and Dynamical Astronomy* 130, 22. Values come from NASA NAIF's planetary constants kernel `pck00011.tpc`, copied character for character. Pluto's and Charon's come from the New Horizons team's kernel `nh_pcnh_010.tpc` (NASA Planetary Data System), for the reason below.

## The model

For each body, the IAU gives:
- **Where the north pole points:** right ascension α₀ and declination δ₀, in the J2000 equatorial frame, each a slow polynomial in time.
- **How far the body has turned:** the prime meridian angle W, a polynomial in days since J2000. Its rate is the spin rate, and it's negative for bodies that spin backwards (Venus, Uranus).
- **Periodic wobbles:** precession, nutation and libration terms for some bodies, as sines and cosines of slowly changing angles. The Moon has 13. Mars uses angles that change quadratically in time.

Worldline evaluates these exactly as NASA's SPICE toolkit does:
- right ascension and the prime meridian get `sin` terms, and declination gets `cos` terms;
- the result is a rotation from the body's own frame to Worldline's ecliptic frame;
- the equatorial-to-ecliptic step uses the J2000 obliquity, 84381.448″, the same value JPL Horizons uses.

The kernel reader only reads the data blocks of the file. The file also contains example values inside its comments, which must be ignored.

**Spin axis versus north pole.** The IAU "north pole" is the pole on the north side of the solar system's plane. The *spin axis* follows the right-hand rule, so for retrograde spinners it is the south pole. Axial tilts are measured from the spin axis, which is why Venus's tilt is 177° rather than 3°.

## Where it's valid

Models are loaded for the Sun, the planets, Pluto, the Moon and the 21 major moons. Hyperion has none: it tumbles chaotically, so no rotation model can be published for it, and Worldline draws it without spin. Mars's model also sets which way its lumpy gravity field faces as it turns (see [moons.md](moons.md)).

**Pluto and Charon.** The two are tidally locked to each other, and the IAU defines Pluto's prime meridian as the mean sub-Charon meridian, and Charon's as the mean sub-Pluto meridian. pck00011's values for them date from at least 1994 and miss that definition by 1.45° (as of the 2015 flyby), growing 0.2° per century. The New Horizons team's kernel corrects the pole, rate and zero point to fit the PLU055 ephemeris, keeping each meridian on the other to under 0.02° from 1950 to 2050. Worldline loads it after pck00011, as SPICE users do, so its values replace pck00011's. Worldline found the error itself: a check that Pluto's map faces Charon correctly measured 1.51° with pck00011 (see [rendering.md](../rendering.md#spacecraft-maps)). The corrected pole also suits JPL's ephemeris better: JPL's mean elements for Pluto's four small moons are referred to Pluto's equator, and with it their orbital planes match JPL's 2025 positions within 0.02°–0.06°, against 0.04°–0.14° with pck00011's pole.

These are mean models, built for maps and pointing at the 0.01°–0.1° level. Earth's model is the simplest: it leaves out the detailed precession and nutation that the IERS tracks. It's accurate enough for rendering and for the checks below, but not for navigating spacecraft.

## Validation

`crates/worldline-data/tests/validation_rotation.rs` and unit tests, run by `cargo test`:

| Check | Expected | Measured |
|---|---|---|
| Axial tilts vs. NASA Planetary Fact Sheet (orbits from the 2025 JPL snapshot) | Mercury 0.034°, Venus 177.4°, Earth 23.4°, Mars 25.2°, Jupiter 3.1°, Saturn 26.7°, Uranus 97.8°, Neptune 28.3° | 0.034°, 177.361°, 23.436°, 25.195°, 3.118°, 26.731°, 97.770°, 28.350°: all within 0.05° |
| Mercury's 3:2 spin–orbit resonance | spins exactly 3 times per 2 orbits | orbit 87.969 days ÷ spin 58.646 days = **1.50000** |
| Earth's sidereal day | 23 h 56 min 4.1 s | 86164.10 s |
| The Moon keeps one face toward Earth (rotation model plus our simulated orbit, over a month) | Earth stays near (0°, 0°) in the Moon's sky, rocking at most ±7.9° in longitude and ±6.7° in latitude (libration) | longitude −5.58° to +5.18°, latitude −6.57° to **+6.67°** |
| Where the Sun is overhead on Earth at 2025-01-01 00:00 TDB | latitude = the Sun's declination, about −23.0°; longitude about −179.0° (noon over the date line, adjusted for the equation of time and TDB − UTC) | **−23.00°, −178.76°** |
| Pluto and Charon face each other at longitude 0° (rotation models plus JPL's 2025 positions; `validation_maps.rs`) | within one map pixel, 0.176° | 0.023° for both (1.507° with pck00011's values) |
| Cassini's laws for the Moon | spin axis 1.54° from the ecliptic pole, orbit axis 4.99°–5.30°, on opposite sides | 1.551°, 5.004°, 178.4° apart |

The Moon checks combine NASA's rotation model with Worldline's own relativistic N-body orbit. If either were wrong, Earth would drift across the Moon's sky.
