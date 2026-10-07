# Magnetospheres and radiation belts

**Code:** `crates/worldline-core/src/magnetosphere.rs`, `crates/worldline-data/src/magnetospheres.rs`, `crates/worldline-app/src/view.rs` (drawing) and `app.rs` (inspector)
**Data:** `crates/worldline-data/data/planetary-dipoles.csv`, `radiation-belts.csv`, and the flow pressure column of `solar-wind-2025.csv` · **Fetch scripts:** `tools/fetch_magnetic_fields.py`, `tools/fetch_solar_wind.py`
**Tests:** `crates/worldline-data/tests/validation_magnetospheres.rs`, plus unit tests in the core

![Earth's magnetosphere on 1 January 2025: the radiation belts along dipole field lines around the tilted magnetic axis (inner orange, outer blue), inside the magnetopause](../images/magnetosphere-step-1b.9.png)

## Planets' magnetic fields

Six planets generate global magnetic fields. To first order each is a dipole, given by field models as the degree-1 Gauss coefficients g₁⁰, g₁¹ and h₁¹:

| Planet | Model | Field at the equator | Tilt from the spin axis | Source |
|---|---|---|---|---|
| Mercury | offset dipole, 484 km north of center | 195 nT | under 3° (taken as 0) | Anderson et al. 2012, *JGR* 117, E00L12 |
| Earth | IGRF-14 at 2025.0 | 29,733 nT | 9.2° | IAGA, IGRF-14 (NOAA NCEI) |
| Jupiter | JRM09 (Juno) | 416,974 nT | 10.3° | Connerney et al. 2018, *GRL* 45, 2590 |
| Saturn | Cassini Grand Finale | 21,139 nT | under 0.007° | Cao et al. 2020, *Icarus* 344, 113541 |
| Uranus | Q3 (Voyager 2) | 22,836 nT | 58.6° | Connerney et al. 1987, *JGR* 92, 15329 |
| Neptune | O8 (Voyager 2) | 14,243 nT | 46.9° | Connerney et al. 1991, *JGR* 96, 19023 |

Venus and Mars have no global field; the solar wind meets their upper atmospheres. The inspector says so.

**Checks.**
- **Earth's pole:** the IGRF dipole puts Earth's north geomagnetic pole at 80.8°N, 72.8°W.
- **Uranus:** Q3 reproduces the published 0.228 gauss and 58.6° tilt.
- **Earth's field:** a unit test checks the field against the gradient of its potential.

## The magnetopause: pressure balance

A planet's field holds the solar wind off. The boundary, the **magnetopause**, sits where the field's magnetic pressure balances the wind's dynamic pressure (Chapman & Ferraro 1931):

K ρv² = (f B₀ (R/r)³)² / 2μ₀  ⇒  r = R (f² B₀² / 2μ₀ K ρv²)^(1/6)

- **ρv²** is the wind's flow pressure. NASA OMNI publishes it hourly, helium included; 2025 averaged 2.62 nPa at Earth. Farther out, the wind thins as 1/r².
- **K = 0.88:** the share of that pressure that reaches the nose after the bow shock slows the flow, for γ = 5/3 (Spreiter, Summers & Alksne 1966).
- **f = 2.44:** how much the magnetopause's own currents strengthen the planet's field at the nose. A flat boundary would double it; the self-consistent shape gives 2.44 (Mead & Beard 1964).
- **The sixth root** makes the boundary stiff: doubling the wind's pressure moves it in by only 11%.

## Validation: Earth (`cargo test`)

**About 10 Earth radii (the roadmap's figure).** With 2025's average wind, the magnetopause sits **9.84 Earth radii sunward**.

**Hour by hour against measurements.** Shue et al. (1998, *JGR* 103, 17691) fitted the magnetopause to spacecraft crossings:
- **The fit:** r₀ = (10.22 + 1.29 tanh(0.184 (B_z + 8.14))) D_p^(−1/6.6).
- **Its accuracy:** a stated scatter about the real crossings of **1.23 Earth radii**.
- **The comparison:** given the same measured wind for all 8,664 hours of 2025 with data, the pressure-balance physics must agree with Shue's fit at least that well.

| | Earth radii |
|---|---|
| Average difference, physics − Shue | +0.11 |
| RMS difference | **0.26**, within 1.23 |
| Hours within ±1 | 99% |

**One shortcut.** Shue's fit takes the field's north–south component in GSM coordinates; these data are GSE, rotated about the Sun–Earth line by up to about 35°. Near B_z = 0 Shue's standoff changes by 0.04 Earth radii per nT, so the difference is a few tenths of an Earth radius at most.

**Physics not in the balance:**
- **Reconnection.** A southward interplanetary field reconnects with Earth's and erodes the dayside. Shue's B_z term carries this; the pressure balance doesn't.
- **Extreme compressions.** Below 8 Earth radii, even Shue's fit drifts from the real crossings (Staples et al. 2020).

## The other planets (printed by the test)

The same balance with each planet's dipole and 2025's average wind, against what spacecraft found:

| Planet | Pressure balance | Observed | |
|---|---|---|---|
| Mercury | 1.4 radii | 1.45 (MESSENGER; Winslow et al. 2013) | agrees |
| Jupiter | 40.8 | 63 or 92, two common states (Joy et al. 2002) | **far too small**: plasma from Io's volcanoes inflates the magnetosphere |
| Saturn | 18.7 | about 22 or 27 (Achilleos et al. 2008) | too small: plasma from Enceladus inflates it |
| Uranus | 24.3 | 18.3 (Voyager 2's single crossing) | one snapshot of a variable boundary |
| Neptune | 23.9 | 26.5 (Voyager 2) | agrees within the variability |

These aren't asserted: for Jupiter and Saturn the model leaves out real physics (internal plasma pressure), and the app says so in the inspector. Only Earth, where the balance is the whole story, is held to a standard.

## Earth's radiation belts

Energetic particles trapped by Earth's field form two belts. They are drawn where they typically lie, measured in **L**, the distance at which a dipole field line crosses the magnetic equator, in Earth radii:
- the **inner belt** at L ≈ 1.1–2.5 (protons of 10–100 MeV);
- the **outer belt** at L ≈ 3–7 (electrons of 0.1–10 MeV);
- the slot between them.

These are the extents seen in the Van Allen Probes era (e.g. *Earth Planet. Phys.* 2023, doi:10.26464/epp2023009). The belts swell and shrink with solar storms, so the drawing shows their typical place.

**Shape.** Each belt is drawn as its inner and outer shells of dipole field lines, r = L R cos² λ, around the magnetic axis as it points at that moment (IGRF, turning with Earth). This shape is a visual approximation. Jupiter's and Saturn's intense belts aren't drawn yet.

## In the app

With a magnetized planet in focus:
- **The magnetopause** is drawn as a wireframe.
  - **Nose:** at the pressure-balance distance.
  - **Orientation:** pointed into the solar wind as the moving planet meets it, about 3.5° off the Sun line at Earth, from its 30 km/s orbital speed.
  - **Flanks:** flaring as Shue et al. measured at Earth, r(θ) = r₀ (2/(1+cos θ))^α. For the other planets, Earth's flaring is borrowed (a visual approximation).
- **Earth's radiation belts** appear inside it.
- **The inspector** shows the planet's field, its tilt, the model it comes from, and the magnetopause distance. For Venus and Mars, it explains that they have no global field.
