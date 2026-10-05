# Saturn's rings

**Code:** `crates/worldline-data/src/rings.rs` (data), `crates/worldline-render/src/bodies.wgsl` (`fs_ring` and the ring shadow in `fs_globe`)
**Data:** `crates/worldline-data/data/saturn-rings-profile.csv`, `saturn-ring-features.csv` · **Fetch script:** `tools/fetch_rings.py`

## The data

**The measured profile.** Cassini's radio science experiment beamed signals from the spacecraft through the rings to Earth, and the dimming of the signal at each radius measured how much ring material lay in the way. Worldline uses the **Rev 7 egress occultation (3 May 2005, X band)**, archived by NASA's Planetary Data System Ring-Moon Systems Node:
- data set `CO-SR-RSS-4/5-OCC-V2.0`, product `RSS_2005_123_X43_E_TAU_10KM`;
- reconstruction method from Marouf et al. 1986, *Icarus* 68, 120.

The profile runs from 72,770 to 145,000 km from Saturn's center and resolves structure down to 10 km. The fetch script:
- applies the label's two radius corrections;
- averages the samples into 7,222 bins of 10 km;
- keeps each bin's detection threshold.

The quantity is **normal optical depth τ**: light passing straight through a ring keeps a fraction e^(−τ). Two adjustments when loading:
- **Noise below zero** is set to zero.
- **The B ring's core** is so opaque that the radio signal sank into the noise. There the value is the detection threshold, τ ≈ 5, which is a lower limit; the true value is at least that.

**The named features.** The PDS node's "Vital Statistics for Saturn's Rings" table lists 41 rings, regions, gaps and ringlets with their boundaries. Worldline copies it for validation and labels.

## How the rings are drawn

| Effect | How | Kind |
|---|---|---|
| Ring structure | each pixel looks up the measured τ at its radius | measured data |
| Detail follows focus | the profile is stored with mipmaps that average transmission e^(−τ). From far away the rings blur into smooth bands; up close, the 10 km structure appears. Averaging transmission (not τ) is exactly what an unresolved stretch of ring lets through. | – |
| Transparency | seen at a slant, the line of sight crosses more ring: coverage = 1 − e^(−τ/μ), μ = cosine of the angle from the ring's normal. That's why the rings look solid edge-on and translucent face-on. | physics model |
| Brightness | sunlight arrives at a slant too: the particles catch 1 − e^(−τ/μ₀) of it, μ₀ = sine of the Sun's height above the rings. Brightness ∝ albedo · μ₀ · caught. From the unlit side only light that diffuses through shows. A simplified particle-layer model. | physics model |
| Particle brightness | A and B ring particles brighter (albedo 0.55) than those in the C ring and Cassini Division (0.25); two levels | visual approximation |
| Saturn's shadow on the rings | each ring point checks whether the line toward the Sun hits Saturn's ellipsoid | physics model |
| The rings' shadow on Saturn | each point on Saturn follows the sunbeam back to the ring plane and keeps e^(−τ/μ₀) of it | physics model |
| Saturn's shape | ellipsoid with the IAU radii: 60,268 km equatorial, 54,364 km polar (9.8% flattened) | measured data |

**Why the rings look dim in 2025.** Saturn's equinox, when the Sun crosses the ring plane, was on 6 May 2025. Near equinox μ₀ is tiny, so sunlight grazes the rings and they catch very little: they really are dim, as Cassini saw at the 2009 equinox. Run `cargo run --release -- --focus Saturn --advance 7.3` to see them in 2032, tilted almost 27° toward the Sun. Drag the camera below the ring plane to see their sunlit face; the Sun is south of the rings until the 2039 equinox.

## Validation

`crates/worldline-data/tests/validation_rings.rs`:

| Check | Published | Measured |
|---|---|---|
| B ring outer edge (walking inward from the empty Huygens Gap) | 117,570 km (PDS table). The edge is an oval swinging about ±70 km, shaped by Mimas's 2:1 resonance (Porco et al. 1984; Spitale & Porco 2010). | 117,630 km |
| Cassini Division, mean τ | 0 to 0.2 | 0.22 (including its ringlets) |
| Encke Gap, mean τ | ~0 | 0.000 |
| B ring core (region B3), mean τ | 1 to 5, the most opaque region | 4.65 |
| A ring, mean τ | 0.4 to 1 | 0.86 |
| **Rings edge-on to Earth** (orbits from our simulation + IAU pole) | 23 March 2025 | **+0.73 days** |
| **Saturn's equinox** (Sun crossing the ring plane) | 6 May 2025 | **+0.54 days** |

The last two combine three independent pieces: JPL's starting positions, Worldline's relativistic N-body integration, and NASA NAIF's model of Saturn's pole. All three have to be right for the dates to come out within a day.
