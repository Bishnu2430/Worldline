# Light bent by a black hole, and the sky behind everything

**Code:**
- `crates/worldline-core/src/lensing.rs`: rays through a non-spinning hole's spacetime, the shadow, and the table the GPU reads;
- `crates/worldline-render/src/sky.rs` and `sky.wgsl`: the sky pass;
- `crates/worldline-app/src/sky.rs`: the sky map;
- `crates/worldline-data/src/stars.rs`: the bright stars.

**Data:** `crates/worldline-data/data/bright-stars.csv` and `crates/worldline-app/assets/textures/gaia_edr3_sky_2k.png`, both fetched by `tools/fetch_sky.py`.

**Tests:** `crates/worldline-core/tests/validation_lensing.rs`, plus unit tests in each crate.

**Sources:**
- **Rays near a black hole:** Binet's equation for light in Schwarzschild spacetime, d²u/dφ² + u = 3GMu²/c² with u = 1/r (e.g. Misner, Thorne & Wheeler 1973, §25.6).
- **The exact bending:** Darwin (1959), *Proc. R. Soc. A* 249, 180, as an elliptic integral computed by Carlson's method (Carlson 1988, *Math. Comp.* 51, 267; 1995, *Numer. Algorithms* 10, 13).
- **The shadow's edge seen from any distance:** Synge (1966), *MNRAS* 131, 463.
- **The sky:**
  - **The Milky Way:** "The colour of the sky from Gaia's Early Data Release 3", ESA/Gaia/DPAC, CC BY-SA 3.0 IGO, acknowledgement A. Moitinho.
  - **The bright stars:** the Yale Bright Star Catalogue, 5th revised edition (Hoffleit & Warren 1991), CDS catalog V/50.
- **Star colors:** Ballesteros (2012), *EPL* 97, 34008, for temperature from B − V.

![Gaia BH1 from 570 km, 40 GM/c²: its shadow, 7.27° across exactly as Synge's formula puts it, and the Milky Way bent into an arc around it](../images/lensing-step-3.1.png)

To see it: `cargo run --release -- --add "Gaia BH1:1" --focus "Gaia BH1" --zoom 20 --paused`.

## Why it matters

Light passing a black hole is bent, more the closer it passes.
- **The shadow:** light aimed closer than √27 GM/c² (2.6 times the horizon) falls in. So a black hole shows as a dark disk that big, its shadow, the thing the Event Horizon Telescope photographed in M87\* and Sagittarius A\*.
- **The ring:** light passing just outside the shadow is bent all the way around, so the sky behind is stretched into rings around it.
- **The Sun bends light too:** just 1.75″ at its edge. Measuring that in 1919 confirmed general relativity.

## Rays

A ray stays in one plane, where u = 1/r follows Binet's equation for light. Worldline traces each ray backward from the camera, which is taken at rest beside the hole.
- **The camera's angle sets the ray:** looking at angle ψ from the direction to the hole, the impact parameter is b = r sin ψ / √(1 − 2GM/c²r).
- **The shadow's edge** falls where Synge's formula puts it, sin ψ_c = √27 (GM/c²r) √(1 − 2GM/c²r). Rays inside it fall in.
- **Tracing:** fourth-order Runge–Kutta on Binet's equation, in steps of φ. The ray ends where u reaches 0, found on the cubic through the last step.
- **The result:** a ray that gets away has swept π − ψ + δ(ψ) around the hole from the camera, δ its bending. It then heads off in that direction, toward the sky.

**From the exact answer.** For a ray from and to far away, the bending is 2∫₀^u_t du/√(1/b² − u² + 2GMu³/c²) − π, u_t the turning point. That is an elliptic integral, done by Carlson's R_F, with the cubic's three roots found by trigonometry and polished by Newton's method far out. Far from the hole it is 4GM/(c²b) + (15π/4)(GM/c²b)² + ….

## What the GPU does, and doesn't

The GPU has no double precision, and in Worldline it knows nothing about physics. So the physics stays on the CPU:
- **The table:** for the camera's distance from the hole, `BendTable` traces 2,048 rays in double precision and tabulates δ(ψ). They are sampled evenly in x = ln((ψ − ψ_c)/(π − ψ_c)), since δ grows like −ln(ψ − ψ_c) toward the edge.
- **Cost and refresh:** building it takes a few milliseconds on all cores. It is built again whenever the camera has moved a thousandth of its distance.
- **The GPU's part,** for each pixel: its ray's angle ψ, δ looked up in the table, the direction the ray ends up in, and the sky map there. Black if ψ ≤ ψ_c.
- **Which hole:** the one whose Einstein ring, √(4GM/c²r) across, is widest from the camera, once that spans half a pixel. Any other is drawn as a dark disk the size of its shadow.

## The sky

- **The Milky Way:** Gaia's map of its glow (2,000 × 1,000, equirectangular in galactic coordinates), enlarged to 4,096 × 2,048.
- **The stars:** the 9,096 stars of the Bright Star Catalogue with positions, drawn into the map as small spots at their exact places.
  - **Brightness:** their flux, 10^(−0.4V), shown as bright as fits up to magnitude 2.3, beyond which they saturate. That's visual.
  - **Color:** from B − V, by Ballesteros's temperature and Planck's law at a display's red, green and blue.
- **Orientation:** the simulation's axes are turned into galactic ones by the obliquity and the Hipparcos rotation. Sagittarius A\* lands in the middle of the map, within a texel.
- **Detail for each pixel:** its level of detail comes from how much sky the pixel covers. So the map's seam doesn't show, and the sky blurs properly where the hole squeezes it.
- **The Starfield option** turns the sky off.

## Where it's valid

- **Non-spinning holes:** spin (Kerr) lensing is step 3.2.
- **The camera at rest beside the hole.** A camera moving fast would also see aberration.
- **One hole bends the sky;** the rest are drawn as dark disks.
- **Only the sky is bent.** Bodies, orbits and labels in front of or behind the hole are drawn unbent, and something behind the hole shows through its shadow.
- **The sky is as seen from the solar system:** from the galactic center the real sky would differ.
- **Star brightness and color** are visual approximations.

## Validation (`cargo test`)

`cargo test --release -p worldline-core --test validation_lensing -- --nocapture`:

| Check | Expected | Result |
|---|---|---|
| The shadow's edge, found by tracing rays from 3.5, 6, 30, 1,000 and a million GM/c² | Synge's sin ψ_c = √27 (M/r)√(1 − 2M/r): the impact parameter there √27 = 5.196152423 GM/c² | 4.1 × 10⁻¹⁴ apart; from 6 GM/c², exactly 45° |
| The shadow's radius from far away | √27 GM/c² | to rounding |
| Rays traced from far away against Darwin's exact bending, for b = 5.25 to 10,000 GM/c² | equal, to the tracing's step⁴ (10⁻¹²) | 5.7 × 10⁻¹³ rad |
| Light grazing the Sun (b = 696,000 km) | the roadmap's 1.75″; and 4GM/c²R + (15π/4)(GM/c²R)², to the next order's 10⁻¹⁰″ | 1.7512013″ = 1.7511903″ + 0.0000110″ |
| The table the GPU reads, from 2.5 (inside the photon sphere), 8, 40 and 10,000 GM/c² | moves the sky on screen by under a tenth of a pixel (0.8 rad over 1,080 rows), the table's error divided by how much the hole magnifies there | at most 0.006 pixel |

Other checks:
- `cargo test -p worldline-data stars`: the stars' positions, carried back to 1950 by their proper motions and turned into galactic coordinates, match the catalogue's own galactic coordinates. The median star is 0.0037° off, within half their last digit.
  - Without carrying them back, the fast movers (61 Cygni, Groombridge 1830, α Centauri) are off by up to 0.09°: that is how the epoch was found.
  - 8 of 9,096 entries stay off, by up to 0.023° (HR 7705); those coordinates disagree with the catalogue's own positions.
- `cargo test -p worldline-render`: the sky shader compiles and validates, and its uniform's layout as naga computes it matches the Rust side.
- `cargo test -p worldline-app sky`: Sagittarius A\* lands in the middle of the sky map, and star colors run from bluish to red as B − V grows.
- **In the app:** Gaia BH1 seen from 40 GM/c² (above) shows a shadow 146.0 px in radius across and down, centered on the view. Synge's 7.274° in a view 967 px tall puts it at 146 px.
