# The desktop app

**Code:** `crates/worldline-app/` · **Run:** `cargo run --release`

The app is a window around the physics engine. It never does physics itself. It asks `worldline-core` to advance the simulation, then draws the result.

## Each frame

1. **Measure real time.** It measures how much real time has passed since the last frame, capped at 0.1 s so a stall (like dragging the window) doesn't cause a jump.
2. **Advance the physics.** It advances by *speed × real time*, stepping IAS15 with relativistic gravity, the same models the validation tests check.
3. **Respect a time budget.** Physics may use at most 12 ms of each frame. If the chosen speed needs more, the simulation runs slower than requested and the top bar shows an orange note with the percentage achieved. **Worldline never takes bigger, less accurate steps to keep up.** Accuracy wins over speed, and the app says so when that happens.
4. **Point the camera.** The camera moves to the body it is following.
5. **Draw.** It draws the panels and the 3D view.

## The 3D view

- **Projected in double precision.** Every position is taken relative to the camera in double precision, and only the final screen coordinates become single precision. GPUs work in single precision (about 7 digits), which can't place Neptune (4.5 × 10¹² m out) to better than about a kilometer. Projecting relative to the camera keeps positions precise at any zoom; a unit test nudges a body 30 AU out by 1 km and checks that it moves on screen by exactly the right amount.
- **Drawing.** egui's painter draws the shapes, rendering through wgpu: rings, trails, bodies and labels. The GPU ray tracer for black holes arrives in milestone 3.
- **To scale.** Distances and positions are true. Bodies are drawn at their true size when that's visible; otherwise as dots at least 3 points wide, and the app says so in the Display panel.
- **Camera.** It orbits a target, with "up" pointing to ecliptic north, so prograde orbits run counterclockwise seen from above. Drag rotates, scroll zooms, and double-clicking a body makes the camera follow it.
- **Trails.** Each trail keeps about one orbit of path, measured as one circle's length at the body's distance from the barycenter, so Mercury and Neptune both show one loop. Trails are drawn in the barycentric frame, and they fade from old to new.
- **Labels.** The selected body is labeled first, then bodies in order of mass. A label is skipped if its body overlaps one already labeled, which hides the Moon's label when zoomed out on the Earth–Moon pair.
- **Spin axes.** Each body gets a line along its spin axis, from the IAU rotation models. A white dot marks the end the spin points toward (right-hand rule), so Venus's dot points down. The inspector shows each body's sidereal day, spin direction and axial tilt. See [physics/rotation.md](physics/rotation.md).
- **Dates.** The top bar shows the simulation time as a calendar date in TDB, JPL's time scale, using Meeus's Julian Date algorithm.

## Tests

`cargo test -p worldline-app` covers:
- **Calendar:** known dates, including J2000, Sputnik 1 and the first Gregorian day, and rounding at midnight.
- **Camera:** projection geometry, hiding points behind the camera, size falling off with distance, double-precision stability 30 AU out, and the zoom limits.
- **Simulation loop:** one real second advances by exactly the chosen speed; pausing stops time; running out of budget is reported; trails reach back about one orbit.
- **Picking:** clicking selects the nearest body within reach.
