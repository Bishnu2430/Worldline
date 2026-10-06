# Zodiacal dust

**Code:** `crates/worldline-core/src/zodiacal.rs`, `crates/worldline-app/src/view.rs` (drawing)
**Tests:** `crates/worldline-core/tests/validation_zodiacal.rs`, plus unit tests

## What it is

The inner solar system is filled with dust from colliding asteroids and crumbling comets. It scatters sunlight into the **zodiacal light**: a faint glow along the ecliptic, visible from dark sites after dusk or before dawn. The same dust glows in the infrared, warmed by the Sun.

No catalog lists dust grains, so this is the one part of step 1b.7 that is a model rather than a list of real objects.

## The model

**Source:** Kelsall et al., "The COBE Diffuse Infrared Background Experiment search for the cosmic infrared background. II. Model of the interplanetary dust cloud", *ApJ* 508, 44 (1998). It was fitted to ten months of COBE DIRBE's all-sky survey in ten infrared bands, and is still the standard model for removing the zodiacal foreground from infrared surveys.

Worldline uses its main component, the **smooth cloud**:

n = n₀ R_c^−α f(ζ),  f(ζ) = exp(−β g^γ),  g = ζ²/2μ for ζ < μ, and ζ − μ/2 for ζ ≥ μ

- **n** is the dust's cross-sectional area per unit volume.
- **R_c** is the distance from the cloud's center.
- **ζ = |Z_c|/R_c** is the height above the cloud's symmetry plane, relative to that distance.

| Parameter | Value | Meaning |
|---|---|---|
| n₀ | 1.13 × 10⁻⁷ AU⁻¹ | density 1 AU from the center, in the plane |
| α | 1.34 | the density falls as R^−1.34: close to the 1/R that Poynting–Robertson drag produces as grains spiral into the Sun |
| β, γ, μ | 4.14, 0.942, 0.189 | the vertical profile: rounded near the plane, fan-shaped above it |
| i, Ω | 2.03°, 77.7° | the symmetry plane is tilted 2° to the ecliptic |
| X₀, Y₀, Z₀ | 0.0119, 0.0055, −0.0022 AU | the cloud's center sits slightly off the Sun |
| outer edge | 5.2 AU | about Jupiter's orbit; the model has no dust beyond |

**Precision.** The paper's tables round these values. Worldline uses the full-precision values distributed with the DIRBE model, as reproduced by ZodiPy (San et al. 2022, *A&A* 666, A107). A unit test checks that the published equations hold:
- the density is n₀ at 1 AU in the plane;
- it falls as R^−α;
- the two branches of g meet;
- the plane has the right tilt and node.

**Left out.** The model's three asteroid dust bands, the circumsolar ring just outside Earth's orbit, and the Earth-trailing blob are not included. They add a few percent of the zodiacal light.

## Validation: Helios (`cargo test`)

**The measurement.** Kelsall et al. fitted the model from Earth's orbit, looking out at 64°–124° from the Sun. An independent test is what happens closer in. Helios 1 and 2 flew in to 0.3 AU and measured the zodiacal light brightening as **R^−(2.3 ± 0.1)**, at every elongation from 17.5° to 135° (Leinert et al. 1981, *A&A* 103, 177).

**The model's prediction.** Worldline computes the sunlight the modeled dust scatters along a line of sight: ∫ n (1 AU/r)² ds, out to the 5.2 AU edge, with Simpson's rule. It does this from 0.3 and 1 AU in the ecliptic, at four elongations and four heliocentric longitudes:

| Longitude | 30° | 60° | 90° | 135° |
|---|---|---|---|---|
| 0° | 2.375 | 2.368 | 2.369 | 2.372 |
| 90° | 2.313 | 2.335 | 2.346 | 2.357 |
| 180° | 2.299 | 2.314 | 2.322 | 2.331 |
| 270° | 2.360 | 2.345 | 2.342 | 2.344 |

**The result.** All 16 lie within Helios's 2.3 ± 0.1. The spread with longitude comes from the cloud's center sitting 0.013 AU off the Sun.

**Why the comparison is fair.** For a density falling as R^−α, the brightness seen at a fixed elongation scales as R^−(α+1), whatever the scattering phase function. So this tests where the model puts the dust, independently of how the grains scatter light.

**Checking the integration itself.** A unit test compares the Simpson integration with a case that has an exact answer (α = 2 and no vertical falloff, where the integral has a closed form). They agree to 7 × 10⁻¹⁶.

## Drawing

The dust is drawn as a faint, warm glow in its tilted symmetry plane, centered where the model puts it. This is a **visual** layer built on the model:
- **Brightness.** The brightness at each radius is the scattered light along a line straight through the cloud, computed from the model and shown on a log scale. It drops by several hundred times from 0.3 to 5 AU.
- **Slant.** Seen at a slant, a thin transparent layer looks brighter, by 1/|cos θ|. The boost is capped at about 3, the cloud's thickness relative to its size.
- **Inside 0.3 AU,** where no probe has measured how the light brightens, the glow is held at its 0.3 AU brightness rather than extrapolated. The dust does continue inward: it is the Sun's F-corona.
- **When it shows.** Like the belts, the dust is drawn only when the view spans more than 0.05 AU.

Scattering is taken as equal in all directions. Real grains scatter much more forward, toward an observer looking past the Sun, which the drawing ignores.
