# Getting started

Worldline is a universe you can run. The Sun, planets, moons, asteroids and comets start where NASA's ephemerides put them on 1 January 2025, and move under gravity, Einstein's where it matters. Around them are the nearest stars, the stars circling the black hole at the galaxy's center, and the galaxy itself. Everything you see is computed as it plays, and each number in the panels comes from a published measurement or from the simulation.

## Your first minute

- **Time runs** when you start: a month a second. **Pause** at the top left stops it, and the menu beside it sets how fast it runs, from real time to a hundred years a second.
- **Look around** by dragging in the view, and zoom with the scroll wheel. A flick keeps the view turning for a moment.
- **Fly to a body** by double-clicking it, or by picking it in the **Bodies** list on the right and pressing F. The camera follows it from then on.

[Fly to Saturn](do:fly:Saturn)

[Fly to Earth](do:fly:Earth)

## The panel on the right

The **Bodies** list holds everything there is, grouped: the planets with their moons, the dwarf planets, asteroids and comets, the nearby stars, the galaxy's center, its clusters and satellite galaxies. Type in its search box to find one.

Under it is the selected body's card: what it is, its mass and size, how far it is and how fast it moves, and the **physics model** that moves it, with a colored dot saying whether that model is within its limits (green), approximate (amber) or beyond what's known (red).

## Where to go next

- **Watch a film.** Every famous event in the library plays as a short film, with captions and cards: try the double pulsar, or two black holes merging.
- **Build something.** **Add a body** puts a planet, a star or a black hole wherever you click, and you can watch what it does to everything else.
- **Look up.** **Sky** stands you on Earth, anywhere, and shows the sky from there.

[Play the double pulsar's film](do:play:double-pulsar)

[Open the catalogue of bodies to add](do:catalogue)

# Controls

## Mouse

- **Drag** in the view to turn it around what the camera follows; **scroll** to zoom.
- **Click** a body to select it; **double-click** to fly to it and follow it.
- While adding a body, **click** to place it on a circular orbit, or **drag** to launch it with the speed and direction you drag.

## Keys

- **Space** runs or stops time.
- **+** or **]** runs it faster, **−** or **[** slower.
- **F** flies to the selected body and follows it.
- **Esc** closes the open windows, or stops adding bodies.
- **Ctrl+S** saves the sandbox.
- **F3** shows how long each frame takes.
- **F1** opens this manual.
- **→** and **←** skip a film's shots.

## The top bar

From the left: **Pause** or **Play**; the speed; **⟲**, which starts over from 1 January 2025; **Now**, which runs with your computer's clock; **Scenarios**, **Eclipses** and **Sky**; **☰**, a menu of every window and the keys; **Add a body**, **Save** and **Load**; and the date and time, in UTC.

When something happens, a line of text at the top of the view says so for a few seconds: a scenario starting and what it opened, a body added, a planet becoming a fluid.

# The view

## What's drawn

- **Bodies** far off are dots in their colors; close up, the planets and moons are globes from spacecraft maps, lit by the Sun, and Saturn wears its measured rings.
- **Stars** are the color of a blackbody at their published temperatures where the data has one: the Sun, Sirius A and B, α Centauri A and B, Proxima.
- **A neutron star** up close is drawn as its own gravity bends its light: bigger than it is, more than half its surface in view.
- **A black hole** bends the sky behind it into a ring, and its shadow is the size general relativity gives it. A spinning one, with its disk, is traced ray by ray.
- **Orbit trails** show about one orbit of each body's path; a moon shows its orbit as it is at that moment.
- **The sky** behind is Gaia's map of the Milky Way; far out, the galaxy's stars glow as the model of its mass puts them.

## Display and quality

**Display**, under the selected body's card, turns each layer on or off: the starfield and the galaxy's glow, trails, distance rings, names, spin axes, textured globes, the belts, the zodiacal dust, the solar wind, the heliosphere, magnetospheres, shadows and the accretion disk.

**Quality** (Low, Medium or High) sets how much is drawn, not the physics: on a weaker computer, Low draws fewer belt particles at a lower resolution. It's picked for your computer at the start, and you can start with `--quality low`.

## Overlays

**Overlays**, beside Display, draw what gravity does: **Models** (which physics moves each body), **Clock rates** (how much slower clocks run in gravity), **Tides** (how a body is stretched), **Embedding diagram** (the shape of space near a black hole), **Proper time** and **Gravitational waves**. Each says what it shows in the view's top-left corner.

# Windows and tools

## Scenarios

The library of famous events, each set up in one click from the solar system as it was on 1 January 2025. Every one plays as a film: the camera moves shot by shot, captions tell the story, and cards beside the bodies give published measurements and what the simulation measures as it plays. The bar at the bottom pauses it, skips shots, and **Leave** hands the camera back to you while the scenario plays on.

[Open the scenarios](do:scenarios)

## Eclipses

Every solar and lunar eclipse of the next five years, found by running the Sun, Earth and Moon ahead. **Go to** takes you to one: for a solar eclipse, the Moon's shadow on Earth and the path it sweeps.

[Open the eclipse finder](do:eclipses)

## Sky from a place

Stand anywhere on Earth and look up: the Sun, Moon and planets where they are in that sky, with refraction, and when the Sun and Moon rise and set.

[Stand on Earth](do:sky)

## Add a body

The catalogue: copies of the planets and the Sun, the notable stars, white dwarfs, neutron stars and black holes, the double pulsar and the merging pairs, and a hypothetical tiny black hole. Pick one, then click in the view to put it on a circular orbit, or drag to launch it. It pulls on everything and everything pulls on it.

[Open the catalogue](do:catalogue)

## Gravitational waves

For a pair of neutron stars or black holes close enough to send waves we could detect: the strain an observer on Earth would record, as a plot, and as sound, pitched up for small speakers if it's too low to hear. Select one of such a pair and open it from **☰**, or play the GW150914 scenario.

[Hear two black holes merge](do:play:gw150914)

## The Moon-forming impact

Canup's (2004) giant impact, a Mars-sized planet striking the young Earth, as thousands of particles of rock and iron, in a window of its own.

[Open the impact](do:impact)

## Save, Load, Now and starting over

- **Save** writes the whole sandbox to a file, and **Load** reads one back, exactly as it was.
- **Now** runs the simulation forward to your computer's clock, and keeps it there.
- **⟲** starts over from 1 January 2025.

## The frame's timings

F3, or **Display → Frame timings**, shows over the view how long each frame takes and where the time goes: physics, the interface and the view.

[Show the timings](do:timings)

# Scenarios

Each starts afresh from 1 January 2025 and plays as a film. What never happened says *imagined* in its first caption.

- **Mercury's perihelion.** The turning of Mercury's orbit that Newton's gravity couldn't explain, and Einstein's 43 seconds of arc a century, measured as it plays.

[Play it](do:play:mercury)

- **The Hulse–Taylor pulsar.** The first binary pulsar: its orbit turned and shrunk by relativity, as timed for decades.

[Play it](do:play:hulse-taylor)

- **The double pulsar.** Two pulsars on a 2.45-hour orbit: the most precise test of gravitational waves.

[Play it](do:play:double-pulsar)

- **GW150914: two black holes merge.** The first gravitational waves heard, in real time.

[Play it](do:play:gw150914)

- **GW170817: two neutron stars merge.** Their last three minutes, in real time; then the gamma rays and the kilonova.

[Play it](do:play:gw170817)

- **A planet meets a black hole.** Imagined: a Jupiter torn apart by tides.

[Play it](do:play:planet-black-hole)

- **TON 618 swallows Sagittarius A\*.** Imagined: one of the heaviest black holes known and the Milky Way's own.

[Play it](do:play:ton-618)

- **A white dwarf past its limit.** Imagined: two white dwarfs, together past Chandrasekhar's limit.

[Play it](do:play:white-dwarf)

- **An evaporating black hole.** Imagined: a tiny black hole glowing away by Hawking radiation.

[Play it](do:play:mini-black-hole)

- **The next total solar eclipse.** 12 August 2026, over Greenland, Iceland and Spain.

[Play it](do:play:eclipse)

- **The Moon-forming impact.** The giant impact the Moon formed from.

[Play it](do:play:moon-impact)

- **A black hole through the Sun's neighborhood.** Imagined: a black hole passing the Sun.

[Play it](do:play:black-hole-flyby)

# Physics models and their limits

Worldline picks a model for each body by what it needs: the inspector's **Model** row says which, and whether the body is within its limits. Each model below says what it does, what it's good for, and where it stops being right. Each has its own page in the project's docs, with its equations, its source, and the test that checks it against a measurement.

## Gravity

- **Newtonian gravity** (newtonian-gravity.md): the inverse-square law, for moons, asteroids and anything far from strong gravity. Good where GM/rc² and v/c are tiny.
- **Einstein–Infeld–Hoffmann** (einstein-infeld-hoffmann.md): general relativity's first correction, for the Sun, planets and stars. It turns Mercury's orbit by its 43 seconds of arc a century. It leaves out (GM/rc²)², which matters for compact pairs.
- **Compact pairs** (post-newtonian.md): the next orders, and gravitational waves carrying energy away, so neutron stars and black holes spiral together. It ends a few orbits before they merge, where the approximation stops converging.
- **Near a black hole** (kerr.md): a body that a black hole pulls on hardest moves in the hole's exact spacetime (Kerr). Exact for a test body; its own mass and its waves' losses are approximate.
- **Mergers** (mergers.md): two black holes become one of the mass, spin, kick and ringdown that numerical relativity's fits give, for holes that don't spin.
- **How time is stepped** (integrators.md): an adaptive integrator (IAS15) that picks each step so its error stays below rounding, and faster ones where that is enough.

## Bodies and their surroundings

- **The solar system's data** (solar-system-data.md, small-bodies.md): NASA JPL's positions and masses. Comets' outgassing is JPL's fit, less certain the further ahead you run.
- **Moons** (moons.md, small-moons.md, rotation.md): every known moon, the major ones in full, the small ones in full where you look and on their mean orbits elsewhere; the planets' gravity fields; how everything spins. Tides that slowly push moons away are left out.
- **Saturn's rings** (saturn-rings.md): their measured profile, drawn with the shadow they cast.
- **The belts** (belts.md, swarm.md): the asteroid belt, Jupiter's Trojans and the Kuiper belt as a swarm of particles that feel every massive body. Their gaps come from the catalog, not from new simulation.
- **The galaxy** (milky-way.md, galaxy-stars.md, neighbors.md, clusters-and-galaxies.md, galactic-center.md): the Milky Way's mass and the Sun's orbit round it, without its bar or spiral arms; its stars as a swarm, each drawn point standing for 1.8 million Suns; the nearest and brightest stars as real bodies; clusters and satellite galaxies as points; the stars circling Sagittarius A*.
- **Compact objects** (compact-objects.md, white-dwarfs.md, neutron-stars.md): black holes, white dwarfs up to Chandrasekhar's limit, and neutron stars up to their maximum mass, from published equations of state, for cold stars that don't spin.
- **Hawking evaporation** (hawking.md): a tiny black hole glowing away. The textbook model, photons only; what happens in its last moments is unknown, and the simulation lets it vanish.

## When things meet

- **Collisions** (collisions.md, collision-outcomes.md, debris.md): what a collision leaves, from scaling laws fitted to simulations, within their range of sizes and speeds; outside it, bodies merge.
- **Craters** (craters.md): their size from scaling laws, to about a factor of two, with no atmosphere or oceans.
- **Tides and fluids** (tides.md, sph.md, disruption.md): a planet nearing something much heavier becomes a fluid of particles that tides can stretch and tear apart, and its debris falls back as theory predicts.
- **Giant impacts** (giant-impacts.md): planets colliding as fluids of rock and iron, at a resolution that keeps the mass and spin left in orbit, not finer detail.

## Light, time and the sky

- **Light bent by gravity** (lensing.md): the sky around a black hole, its shadow, a spinning hole's disk, and a neutron star's own light. The disk's gas is drawn, not simulated.
- **Star colors** (star-colors.md): a blackbody's color at each star's published temperature. A real star isn't quite a blackbody.
- **Time and clocks** (time.md, proper-time.md): the simulation's time scales and Earth's measured rotation, predicted to 2027; each body's own clock, slowed by gravity and speed.
- **Eclipses and the sky** (eclipses.md, sky-from-earth.md): the Moon's and Earth's shadows, eclipses found ahead to about 2031, and the sky from any place on Earth. Rise and set times are the calculation's: real air moves them by a minute or more.
- **The Sun's reach** (sun-reach.md, magnetospheres.md, zodiacal-dust.md): sunlight, the solar wind and the heliosphere; planets' magnetic fields and radiation belts; the zodiacal dust.
- **Overlays** (overlays.md): what each overlay computes, and to what order.

# Where the numbers come from

Every model cites its source and has a test that checks it against a published measurement: Mercury's perihelion, the Hulse–Taylor and double pulsars' orbits, GPS clocks, the 2026 eclipse's path, a black hole's shadow and dozens more. The project's README lists each claim with its test and a plot of what the test measured.

The data comes from NASA JPL, ESA's Gaia, the IAU, the CIE and the papers each value is copied from, with the digits as published. The project's CREDITS.md says where every number, map and font comes from, and under what license.

Where physics is unknown, Worldline doesn't invent it: the panels say so instead, and the model indicator turns amber or red.
