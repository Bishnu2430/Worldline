# Star colors

**Code:** `crates/worldline-core/src/blackbody.rs` (Planck's law, the CIE sums, sRGB); `crates/worldline-app/src/view.rs` (`blackbody_color`); `crates/worldline-app/src/gpu.rs` (the Sun's map recolored).

**Data:**
- `crates/worldline-data/data/cie-1931-2deg.csv`: the CIE 1931 colour-matching functions, 2° observer, 360 to 830 nm at 1 nm, the CIE's own dataset (DOI 10.25039/CIE.DS.xvudnb9b, CC BY-SA 4.0), fetched by `tools/fetch_cie.py` and checked against its published SHA-256;
- `crates/worldline-data/data/star-temperatures.csv`: each star's effective temperature and its source, written by `tools/write_notable_objects.py`.

**Tests:** `crates/worldline-data/tests/validation_star_colors.rs`; unit tests in `blackbody.rs` and `cie.rs`.

Step 8.5b draws each star the color of a blackbody at its published effective temperature: the temperature of a blackbody that gives off as much light from each square metre.

## The model

1. **Planck's law** gives a blackbody's light at each wavelength: B(λ, T) ∝ λ⁻⁵ / (e^(c₂/λT) − 1), with c₂ = hc/k = 1.438 776 877 × 10⁻² m K (exact, from the SI's h, k and c).
2. **The CIE 1931 colour-matching functions** x̄, ȳ, z̄ (CIE 018:2019, Table 6) weigh it as the eye does. Summed at their 1 nm steps, they give the color's CIE XYZ (CIE 015:2018).
3. **sRGB.** IEC 61966-2-1:1999's matrix turns XYZ into a screen's linear red, green and blue, and its transfer curve encodes them.
4. **Its hue, at full brightness.** The color is scaled so its brightest channel is full: brightness is drawn separately (a globe's limb darkening, a dot's size). A channel a screen can't show, below 0, is clipped to 0: that happens only for blackbodies cooler than about 1,900 K, redder than a screen's red.

The Sun's surface map, an artist's rendering, is recolored to its 5,772 K color: each pixel keeps its brightness against the map's mean, so the granulation and sunspots stay.

## The stars and their temperatures

| Star | Temperature | Source | Drawn as |
|---|---|---|---|
| Sun | 5,772 K, nominal | IAU 2015 Resolution B3 (Prša et al. 2016, *AJ* 152, 41) | #fff1ea |
| Sirius A | 9,900 ± 200 K | Kervella et al. 2003, *A&A* 408, 681, §2.4 (after Lemke 1989) | #cedaff |
| Sirius B, a white dwarf | 25,922 ± 296 K | Joyce et al. 2018, *MNRAS* 481, 2361, Table 3: the mean of four HST spectra | #a5bdff |
| α Centauri A | 5,795 ± 19 K | Kervella et al. 2017, *A&A* 597, A137, Table 5 | #fff1ea |
| α Centauri B | 5,231 ± 21 K | Kervella et al. 2017, Table 5 | #ffead8 |
| Proxima Centauri | 3,054 ± 79 K | Boyajian et al. 2012, *ApJ* 757, 112, Table 6 (GJ 551) | #ffb971 |

Each is the paper the catalogue already cites for the star's radius, where it gives a temperature; Proxima's radius comes from relations, so its temperature is Boyajian et al.'s interferometric one. Copies of these stars added from the catalogue are drawn the same.

**Not by temperature, and labeled so.** The other nearby stars keep their kinds' colors (their temperatures aren't in the data yet), as do the stars around Sagittarius A* (by spectral type, early or late). No neutron star here has a measured temperature of its whole surface: up close they're white (see [lensing.md](lensing.md#a-neutron-stars-own-light-step-85b)). The inspector's rows under a star say which.

## Where it's valid

- **A star isn't a perfect blackbody.** Its spectral lines and absorption change its color a little; the blackbody's is the standard approximation to it.
- **The color is a screen's.** sRGB can't show every color the eye can; a blackbody between 1,900 K and 40,000 K falls inside it.
- **What we see from Earth differs.** The Sun looks yellow from the ground because the air scatters blue light away; from space it's the near-white drawn here.

## Validation

**The CIE's own number for its standard lamp** (`illuminant_a_comes_out_as_the_cie_publishes_it`). The CIE defines illuminant A, a tungsten lamp's light, as a blackbody: Planck's law with c₂ = 1.435 × 10⁻² m K at 2848 K. Its published chromaticity is x = 0.44757, y = 0.40745 (CIE 015:2018, Table 1). Summed over the whole table, Worldline gets x = 0.447574, y = 0.407439.

The limit is the published values' rounding (5 × 10⁻⁶) plus how far the sums CIE 015 allows (360 to 830 nm or 380 to 780 nm, at 1 nm or 5 nm) spread from Worldline's: 2.9 × 10⁻⁶ in x and 8.2 × 10⁻⁶ in y. So 7.9 × 10⁻⁶ and 1.3 × 10⁻⁵. It passes, by 4 × 10⁻⁶ and 1.1 × 10⁻⁵.

**The table is the CIE's** (`the_table_is_the_one_the_cie_published`): its columns' sums and sample row match those the dataset's metadata publishes, to the last digit.

**sRGB** (`d65_white_is_the_screens_white`): D65, the sRGB white, maps to equal channels within 1.6 × 10⁻⁴, the most the matrix's four printed places allow.

**Hotter is bluer** (`hotter_stars_are_bluer`): along the six stars, x falls and the drawn blue grows against the red as the temperature rises.
