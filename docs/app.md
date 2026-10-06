# The desktop app

**Code:** `crates/worldline-app/` · **Run:** `cargo run --release` (options: `-- --focus Jupiter --zoom 40 --paused --advance 7`; `--zoom` sets how many of the focused body's radii away the camera starts, and `--advance` runs the physics that many years past the 2025 snapshot before the window opens)

The app is a window around the physics engine. It never does physics itself. It asks `worldline-core` to advance the simulation, then draws the result.

## Each frame

1. **Measure real time.** It measures how much real time has passed since the last frame, capped at 0.1 s so a stall (like dragging the window) doesn't cause a jump.
2. **Advance the physics.** It advances by *speed × real time*. Each step runs the whole hierarchy: relativistic gravity for the Sun and planets, then each planet's moon system in its own frame (see [physics/moons.md](physics/moons.md)). These are the same models the validation tests check. After each step the app copies positions into one flat list of bodies: the Sun and planets in the snapshot's order, then the moons. A planet with moons stands in for its system's barycenter.
3. **Respect a time budget.** Physics may use at most 12 ms of each frame. If the chosen speed needs more, the simulation runs slower than requested and the top bar shows an orange note with the percentage achieved. **Worldline never takes bigger, less accurate steps to keep up.** Accuracy wins over speed, and the app says so when that happens.
4. **Point the camera.** The camera moves to the body it is following.
5. **Draw.** It draws the panels and the 3D view: rings and trails at the back, textured globes from the GPU in the middle, dots and labels in front. See [rendering.md](rendering.md).

## The 3D view

- **Projected in double precision.** Every position is taken relative to the camera in double precision, and only the final screen coordinates become single precision. GPUs work in single precision (about 7 digits), which can't place Neptune (4.5 × 10¹² m out) to better than about a kilometer. Projecting relative to the camera keeps positions precise at any zoom; a unit test nudges a body 30 AU out by 1 km and checks that it moves on screen by exactly the right amount.
- **Drawing.** egui's painter draws the shapes, rendering through wgpu: rings, trails, bodies and labels. The GPU ray tracer for black holes arrives in milestone 3.
- **To scale.** Distances and positions are true. Bodies are drawn at their true size when that's visible; otherwise as dots at least 3 points wide, and the app says so in the Display panel.
- **Camera.** It orbits a target, with "up" pointing to ecliptic north, so prograde orbits run counterclockwise seen from above. Drag rotates and scroll zooms, down to 1.1 radii from a body's center. Double-clicking a body (or pressing *Fly to it*) flies the camera there in 1.2 s, ending 4 radii out so the globe fills most of the view. The fly-in eases in and out, and zooms evenly in scale, so going from 4 AU to 25,000 km looks smooth.
- **Trails.** Each trail keeps about one orbit of path, measured as one circle's length at the body's distance from the barycenter, so Mercury and Neptune both show one loop. Trails are drawn in the barycentric frame, and they fade from old to new. A physics step can carry a body farther than the trail's point spacing (about half a degree of arc as seen from the barycenter), especially in the outer solar system. Then the points in between are filled in by cubic Hermite interpolation from the positions and velocities at both ends of the step. This only smooths the drawing; the physics never uses trails.
- **Moons.** A moon can circle its planet several times between physics steps (Phobos takes 7.7 hours), too fast for a recorded trail. So each moon shows its *osculating orbit* instead: the ellipse it would follow if only its planet pulled on it, computed from its current position and velocity. It fades toward the moon like a trail. Moons and their orbits appear only once the view resolves them: when a moon's distance from its planet, on screen, clears the planet's dot or globe. Zoomed out, a planet stays a single clean dot.
- **Labels.** The selected body is labeled first, then bodies in order of mass. A label is skipped if it would overlap one already placed, which keeps a planet's crowded inner moons from piling up.
- **Spin axes.** Each body gets a line along its spin axis, from the IAU rotation models. A white dot marks the end the spin points toward (right-hand rule), so Venus's dot points down. The inspector shows each body's sidereal day, spin direction and axial tilt. See [physics/rotation.md](physics/rotation.md).
- **Details and their kind.** The inspector lists what's shown up close for the selected body, each tagged **measured** (observed data), **model** (a physics model) or **visual** (a visual aid or simplification). See [rendering.md](rendering.md).
- **Small moons and focus.** All 459 known moons are in the list. Flying to a planet, or to any of its moons, switches that planet's small moons to detailed simulation. Every other small moon rides an approximate orbit, and the inspector says which it is. A small moon shows its orbit only while selected, since hundreds of ellipses would hide everything else. A moon whose size hasn't been measured is drawn as a dot, and the camera flies to 1,000 km from it. See [physics/small-moons.md](physics/small-moons.md).
- **The body list** shows each planet with its moons indented beneath it, the small moons folded into a collapsible "N small moons" entry. Below the planets, collapsible groups hold the dwarf planets, the bodies beyond Neptune, the asteroids, the comets and the interstellar visitors.
- **Small bodies** are drawn as dots in a color for their kind: comets pale blue, interstellar visitors magenta, and the rest in shades of tan and gray-brown. The inspector says whether a body pulls on the planets or only follows them, and whether it feels outgassing or the Yarkovsky push. See [physics/small-bodies.md](physics/small-bodies.md). For a moon, the inspector gives its distance and speed relative to its planet, and its axial tilt relative to its orbit around the planet. The Physics panel names both gravity models: the one between the planets and the one inside moon systems.
- **Dates.** The top bar shows the simulation time as a calendar date in TDB, JPL's time scale, using Meeus's Julian Date algorithm.

## Tests

`cargo test -p worldline-app` covers:
- **Calendar:** known dates, including J2000, Sputnik 1 and the first Gregorian day, and rounding at midnight.
- **Camera:** projection geometry, hiding points behind the camera, size falling off with distance, double-precision stability 30 AU out, and the zoom limits.
- **Simulation loop:** one real second advances by exactly the chosen speed; pausing stops time; running out of budget is reported; trails reach back about one orbit and stay smooth across long steps; planets stand in for their systems' barycenters, with their moons listed under them; small moons are computed in detail only around the focus.
- **Picking:** clicking selects the nearest body within reach.
