# Rendering globes

**Code:** `crates/worldline-render/` (GPU renderer: mesh, shader, mipmaps), `crates/worldline-app/src/gpu.rs` and `textures.rs` (connecting it to the app)

When a body is big enough on screen to see, at least 3 points across, Worldline stops drawing it as a dot and draws it as a textured, lit, rotating globe on the GPU. This is the first piece of the "detail follows focus" principle (see [ARCHITECTURE.md](ARCHITECTURE.md)).

## How a frame is drawn

The 3D view is drawn in three layers:

1. **Back, on the CPU through egui:** reference rings, orbit trails and the Sun's glow.
2. **Middle, on the GPU (`worldline-render`):** globes, drawn into an offscreen image that is transparent everywhere except the globes. egui then paints that image over the back layer, so trails pass behind planets naturally.
3. **Front, on the CPU through egui:** dots for bodies too small to see, spin axes, and labels. A dot is hidden if a nearer globe covers it.

## Precision across 12 orders of magnitude

The view spans from the camera a few thousand kilometers above Earth out to Pluto, 5 × 10⁹ km away. Two techniques keep that precise on a GPU that works in single precision:

- **Camera-relative positions.** The app subtracts the camera's position from every body in double precision, so the GPU only ever sees distances from the camera. Earth's center 25,000 km away is exact to under a meter in single precision.
- **Reversed, infinite depth buffer.** Depth is stored as a 32-bit float that's 1 at the near plane and falls toward 0 at infinity. Floats are densest near 0, which is where the far distances land, so precision stays even across the whole range. There's no far plane at all. The near plane sits halfway to the closest surface.

## Surfaces

- **Maps:** 2K (2048 × 1024) equirectangular maps from Solar System Scope (CC BY 4.0; see [CREDITS.md](../CREDITS.md)). Longitude 0° is in the middle, east to the right, so the IAU prime meridian lines up with the map. Venus uses the cloud-top map, since that's what you'd see.
- **Real rotation:** each globe is turned by its IAU orientation for the current moment (see [physics/rotation.md](physics/rotation.md)). So Earth's continents turn once per 23.93 hours, Uranus rolls on its side, and Venus turns backwards.
- **Map lookup per pixel.** Longitude and latitude are worked out per pixel from the body's own frame, so there's no stretching at the poles from the mesh.
- **No seam at ±180°.** At ±180° longitude the map's coordinate jumps from 1 back to 0, which would normally draw a visible line. The shader takes its sampling rate from a copy of the coordinate whose jump is on the opposite side (Tarini 2012).
- **Mipmaps averaged in linear light.** Every map gets a full chain of half-size copies, so a distant planet samples a properly averaged image instead of sparkling. The 2 × 2 averaging happens in linear light, not on raw sRGB values, so colors don't darken as they shrink. A unit test checks that black and white average to sRGB 188, not 128.
- **Loaded only when needed.** A map is decoded and uploaded the first time its body becomes a globe. Bodies you never zoom in on never cost memory.
- **4× multisampling** smooths planets' edges.

## Light

| Effect | Model | Kind |
|---|---|---|
| Sunlight on planets | Lambert: brightness ∝ cos(angle from overhead), lit from the Sun's actual direction | physics model |
| Earth's city lights | NASA night-lights map, fading in just past the day/night line | measured data |
| Sun's limb darkening | the Sun's edge looks dimmer because we see higher, cooler gas there. Eddington approximation: I(μ)/I(1) = 0.4 + 0.6 μ | physics model |
| 1% fill light on night sides | keeps the dark side from vanishing completely | visual aid, labeled in the app |

## Checks

- **CI, without a GPU:**
  - the shader is compiled and validated with naga, the same compiler wgpu uses;
  - the uniform-buffer layout matches the shader's padding rules;
  - the depth projection maps the near plane to 1 and infinity to 0;
  - the sphere's triangles all face outward;
  - mipmaps average in linear light;
  - all the bundled maps decode at 2048 × 1024.
- **Physics** (`validation_rotation.rs`): the Sun is overhead at **23.00°S, 178.76°W** at 2025-01-01 00:00 TDB. That's the Sun's declination on January 1, and local noon over the date line at midnight in Greenwich. It puts the day/night line where it really was.
- **By eye:** at the start date, Australia and the Pacific are in daylight and Antarctica in its 24-hour summer sun. Europe and North Africa show city lights in the middle of their night.

## Next

Step 1b.3 adds the up-close details:
- Earth's clouds and atmosphere,
- Saturn's real ring structure and shadows,
- Jupiter's bands and Great Red Spot,
- the Sun's surface.
