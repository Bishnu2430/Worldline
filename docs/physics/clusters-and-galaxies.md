# Star clusters, the Orion Nebula and satellite galaxies

**Code:**
- `crates/worldline-data/src/deep_sky.rs`: the objects as published, and their places in space and in the galaxy;
- `crates/worldline-app/src/simulation.rs`: stepping them with the galaxy's swarm (`GalaxySwarm`), and listing them as bodies;
- `crates/worldline-app/src/view.rs`: `draw_deep_sky`.

**Data:** `crates/worldline-data/data/clusters-and-galaxies.csv`, written by `tools/fetch_clusters_and_galaxies.py`.

**Tests:**
- unit tests in `deep_sky.rs`;
- `crates/worldline-data/tests/validation_clusters_and_galaxies.rs`;
- `the_clusters_and_galaxies_are_where_measured_move_and_save` in `crates/worldline-app/src/simulation.rs`.

**Sources:**
- **Globular clusters:** Baumgardt et al.'s Galactic Globular Cluster Database, version 4 (2023). It gives each cluster's:
  - center, from the paper each row names (most from Goldsbury et al. 2010);
  - distance (Baumgardt & Vasiliev 2021, *MNRAS* 505, 5957);
  - proper motion from Gaia EDR3 (Vasiliev & Baumgardt 2021, *MNRAS* 505, 5978);
  - radial velocity, from the database's own compilation (Baumgardt et al. 2019, *MNRAS* 482, 5138);
  - mass and half-light radius, from its N-body fits (Baumgardt & Hilker 2018, *MNRAS* 478, 1520).
- **Open clusters:** Hunt & Reffert (2023), *A&A* 673, A114, from Gaia DR3: each cluster's densest point, its distance, its members' mean proper motion and Gaia radial velocity, and the radius holding half its members.
- **The Orion Nebula:** Kuhn et al. (2019), *ApJ* 870, 32. Their Table 1 gives the center, proper motion and distance of the Orion Nebula Cluster, the stars inside the nebula, from Gaia DR2. Their Table 5 gives its radial velocity, the median of 12 stars' from Gaia DR2.
- **Satellite galaxies:** the Local Volume Database (Pace 2025, *OJAp* 8, 142), release v1.1.2. It names the paper behind each number. For the Large Magellanic Cloud:
  - distance: Pietrzyński et al. 2019, *Nature* 567, 200;
  - proper motion: Kallivayalil et al. 2013, *ApJ* 764, 161, from Hubble;
  - radial velocity: van der Marel et al. 2002, *AJ* 124, 2639.
- **Galactic coordinates:** the Hipparcos catalogue's rotation (ESA 1997, vol. 1, §1.5.3), as in [galactic-center.md](galactic-center.md).

## Why it matters

The galaxy had its mass (step 2.9), its stars as a swarm (step 7.2) and its nearest stars one by one (step 7.1). Now its famous objects are there too, each where astronomers measured it and moving as they measured:
- the Pleiades and the other open clusters with a Messier number, and the Hyades;
- the Orion Nebula;
- every globular cluster with a measured motion, from 47 Tucanae and Omega Centauri to Crater, 147 kpc out;
- the Magellanic Clouds and every other satellite galaxy with a measured motion, out to Leo I at 258 kpc.

## What's there

| Kind | How many | From | For example |
|---|---|---|---|
| Open clusters | 28 | Hunt & Reffert 2023 | Hyades 47.19 pc, Pleiades 134.84 pc, Praesepe 183.49 pc, M67 837 pc |
| Nebulae | 1 | Kuhn et al. 2019 | Orion Nebula 403 pc |
| Globular clusters | 166 | Baumgardt et al. 2023 | M4 1.85 kpc, 47 Tucanae 4.52 kpc, Omega Centauri 5.43 kpc, M13 7.42 kpc |
| Satellite galaxies | 56 | Local Volume Database | Sagittarius Dwarf 26.3 kpc, Large Magellanic Cloud 49.59 kpc, Small Magellanic Cloud 62.8 kpc, Leo I 258 kpc |

**What's left out:**
- **Satellites:** one of the database's 57 confirmed satellites of the Milky Way or the Large Magellanic Cloud, Carina IV, which has no proper motion yet.
- **Open clusters:** those without a Messier number, of the thousands Gaia has found.
- **Globular clusters:** none. Every one in Baumgardt's database has a measured motion. Mercer 5 has no mass or radius there.

**Names.** Each is named as it's best known: "Pleiades", "M13", "Omega Centauri", "NGC 6397". Its other designations are in the inspector.

## From a catalogue to a place in the galaxy

Each catalogue gives an object's:
- **direction:** right ascension and declination;
- **distance:**
  - for the clusters, in parsecs or kiloparsecs;
  - for the satellites, as a distance modulus, m − M = 5 log₁₀(d/10 pc);
- **motion across the sky:** its proper motion, μ, in milliarcseconds a year;
- **motion along the line of sight:** its radial velocity, v_r.

As for the nearby stars ([neighbors.md](neighbors.md)), these give a place and velocity relative to the solar system's barycenter:
- **Place:** the distance along the direction.
- **Velocity:** μ times the distance across the sky, plus v_r along it.
- **The moment:** each is for its catalogue's epoch, 2016.0 or 2015.5. It's carried in a straight line to the simulation's start. Over those nine years the galaxy's pull bends the path by under 2 × 10⁵ km, under 10⁻¹² of its distance.

**Into the galaxy.** Adding the barycenter's own place and motion around the galaxy's center turns this into a place and velocity there. The barycenter's are measured too: GRAVITY's distance to Sagittarius A\*, its motion across the sky and the Sun's height above the midplane ([milky-way.md](milky-way.md)).

## How they move

Like the galaxy's swarm ([galaxy-stars.md](galaxy-stars.md)), they're tracers in McMillan's (2017) potential:
- **Their steps:** each by leapfrog steps of its own, with the swarm, every 1,000 years of simulated time.
- **Between steps:** carried in straight lines.
- **What pulls on them:** every body of a hundredth of a Sun or more, in any region. A black hole dropped among them kicks them as it kicks the swarm's stars.
- **Swallowed:** one that reaches a body's surface or horizon is gone into it.
- **Massless:** they pull on nothing. A cluster is a point; its stars aren't simulated one by one.

**In the app** they're bodies in the list, under their kind. They can be selected, followed and removed, but have no clock of their own. Saves keep where each is. Saves from before this step place them as measured and carry them through the galaxy's potential to the save's moment.

## Validation

**The roadmap's check** (`cargo test -p worldline-data --test validation_clusters_and_galaxies -- --nocapture`):

| Check | Expected | Result |
|---|---|---|
| Each of the 251, placed in the galaxy as the app places it, then measured back from the barycenter at its epoch: direction, distance (in its published unit), proper motion and radial velocity | as published, to double precision's rounding: 10⁻¹² of its galaxy-centered place or velocity, ten thousand roundings | the worst at 0.001 of that |
| Each one's galactic longitude and latitude, from its right ascension and declination, against those its catalogue prints | half a unit in the last digit printed, plus 0.1″, see below | globular clusters: worst 1.77″, 0.94 of what's allowed; open clusters: 17.8″, 0.98; satellite galaxies: 0.025″, 0.25 |

**What the first check shows.** It checks the conversion from catalogue to galaxy and back, the same as the nearby stars' check does. The app adds nothing to where an object was measured.

**What the second check adds.** It's independent: the catalogues worked out their galactic coordinates themselves. Agreement shows the direction is read and turned into the galaxy's frame as theirs is, and each catalogue's printed precision shows in its worst case:
- **Globular clusters:** latitudes to 0.001°, so up to 1.8″ apart.
- **Open clusters:** latitudes to four digits, so up to 18″ apart.

**The 0.1″ allowance.** Two correct conversions can differ by:
- **The definitions:** the galactic pole and the zero of longitude are defined to 10⁻⁵ degrees, half a unit of which is 0.018″ in each of three angles.
- **The axes:** some conversions, astropy's among them, apply those angles in the axes of the J2000 equator and equinox rather than the ICRS's. The IAU 2000 frame bias turns those by 23.2 mas.

Together they come to under 0.08″. The satellite galaxies' coordinates are printed to 10⁻⁷ degrees, and theirs differ from Worldline's by up to 0.025″: the frame bias showing.

**Also checked** (`cargo test --release -p worldline-app --bin worldline the_clusters_and_galaxies -- --nocapture`):

| Check | Expected | Result |
|---|---|---|
| In the app, at the start: each where its catalogue puts it, carried from its epoch | to rounding | Hyades 47.1910 pc, Pleiades 134.8414 pc, Orion Nebula 403.0002 pc, Omega Centauri 5,430.0021 pc, Large Magellanic Cloud 49.591 kpc |
| A thousand years on, stepped with the swarm | each at x₀ + v₀t + ½at², a the galaxy's pull and every heavy body's; what's left is of third order, under (vt/r) of the ½at² term | the worst, Sculptor, at 0.20 of that |
| Removing one, then a save and load | the rest where they were, to the last digit; the removed one stays gone | passes |

## In the app

```bash
cargo run --release -- --focus "Omega Centauri" --zoom 9000 --pitch 40 --paused
```

Each object is a soft disk the size of its half-light radius, or for an open cluster the radius holding half its members, with a dot at its center. Seen from far away it's just the dot. The colors only tell the kinds apart:

| Kind | Color |
|---|---|
| Globular clusters | warm yellow |
| Open clusters | pale blue |
| The Orion Nebula | pink |
| Satellite galaxies | lavender |

Selecting one shows:
- its other designations;
- its mass where its catalogue gives one, and its size;
- how far it is from the Sun and the galaxy's center;
- its published distance, proper motion and radial velocity;
- where each comes from.

They're listed under **Open clusters**, **Nebulae**, **Globular clusters** and **Satellite galaxies**.

![The Milky Way from 70 kpc: the disk and bulge of the swarm, the yellow dots of the globular clusters around them, the lavender satellite galaxies farther out, and the Magellanic Clouds as two soft disks at the lower right; the inspector shows Omega Centauri, 3.94 million Suns, 5,430 parsecs from the Sun](../images/clusters-and-galaxies-step-7.3.png)

## Where it's valid

- **Points, not stars.** A cluster or galaxy is one point moving as its center does. Its stars aren't simulated, and nothing stretches or disrupts it: tides don't pull a cluster into streams, and the Sagittarius Dwarf isn't torn apart.
- **Their own pull is left out.** The Large Magellanic Cloud weighs about 1.38 × 10¹¹ Suns (Erkal et al. 2019, *MNRAS* 487, 2685), a tenth of the Milky Way. Its pull would bend the paths of the Small Magellanic Cloud and of the halo's clusters and stars, and swing the Milky Way's center around. McMillan's potential has none of it: over a few hundred million years the Clouds' paths here differ from their real ones.
- **The nebula's gas isn't simulated.** The Orion Nebula is placed and moves as the star cluster inside it. Kuhn et al.'s Gaia distance, 403 pc, is one of several published: Kounkel et al. (2017) measured 388 ± 5 pc with radio parallaxes. Its size isn't in their tables, so it's drawn as a dot.
- **Centers are a choice.** A galaxy as spread out as the Large Magellanic Cloud has no single center: here it's the database's, 78.76°, −69.19°. Centers found from its light, its gas or its stars' rotation differ by about a degree, nearly a kiloparsec at its distance.
- **Published values, no spread.** Each is placed at its published values. The errors are shown, but not sampled.
- **Epochs.** Globular clusters' centers come from several surveys of the last few decades, and are taken as at 2016.0, the epoch of their Gaia motions. The fastest across the sky, M4 at 22.8 mas a year, moves under half an arcsecond in twenty years.

## Data

`tools/fetch_clusters_and_galaxies.py`:
- reads Baumgardt's database's two tables from its web pages;
- asks VizieR for Hunt & Reffert's catalogue;
- reads the Local Volume Database at a fixed commit from GitHub;
- writes the Orion Nebula's row from Kuhn et al.'s tables, digit for digit.

The open clusters are chosen by their NGC, IC or Melotte names: M39 is listed there as Theia 517, among whose names is NGC 7092.
