//! The live simulation behind the window: the solar system, its physics,
//! the clock, and the orbit trails.

mod save;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use worldline_core::constants::{AGE_OF_UNIVERSE, DAY};
use worldline_core::gravitational_waves::{merger_time, period_derivative};
use worldline_core::gravity::Gravity;
use worldline_core::hierarchy::{Collision, FREED_MOON_IDS, Hierarchy, MOON_SYSTEM_GRAVITY};
use worldline_core::integrator::{Ias15, Integrator};
use worldline_core::kepler::drift;
use worldline_core::magnetosphere::Dipole;
use worldline_core::regime::Regime;
use worldline_core::rotation::RotationModel;
use worldline_core::solar_wind::{Heliosphere, ParkerSpiral};
use worldline_core::swarm::{Particle, Swarm};
use worldline_core::{Body, DVec3};
use worldline_data::{
    BeltKind, ObjectKind, RadiationBelt, SmallBodyKind, SmallMoon, VoyagerCrossing,
};

use crate::catalogue::{self, Entry};

/// The recent path of one body around the Sun: about one orbit's worth.
/// Moons have none; they are shown with their current orbit (see
/// `view.rs`).
#[derive(Debug, Clone, Default)]
pub struct Trail {
    points: VecDeque<DVec3>,
    /// Total length of the path through `points`, in m.
    length: f64,
    /// The last state seen: time (s), position and velocity.
    last: Option<(f64, DVec3, DVec3)>,
}

impl Trail {
    /// A new point is recorded once the body has moved this fraction of its
    /// distance from the barycenter: about 0.5° of arc.
    const SPACING: f64 = 0.008_7;
    /// Upper limit on stored points, as a memory guard.
    const CAPACITY: usize = 4096;

    /// Adds the body's state at `time`. When a physics step jumps farther
    /// than the spacing, points in between come from cubic Hermite
    /// interpolation of the positions and velocities at both ends, so the
    /// trail stays smooth however long the steps (it's a drawing aid: the
    /// physics doesn't use it).
    fn record(&mut self, time: f64, position: DVec3, velocity: DVec3) {
        if let Some((t0, p0, v0)) = self.last {
            let h = time - t0;
            let pieces = ((position - p0).length() / (Self::SPACING * position.length())).ceil();
            if h > 0.0 && pieces > 1.0 && pieces < Self::CAPACITY as f64 {
                for k in 1..pieces as usize {
                    let s = k as f64 / pieces;
                    let (s2, s3) = (s * s, s * s * s);
                    let point = p0 * (2.0 * s3 - 3.0 * s2 + 1.0)
                        + v0 * (h * (s3 - 2.0 * s2 + s))
                        + position * (-2.0 * s3 + 3.0 * s2)
                        + velocity * (h * (s3 - s2));
                    self.add(point);
                }
            }
        }
        self.last = Some((time, position, velocity));
        self.add(position);
    }

    fn add(&mut self, position: DVec3) {
        let step = self.points.back().map(|last| (position - *last).length());
        if step.is_some_and(|s| s <= Self::SPACING * position.length()) {
            return;
        }
        self.points.push_back(position);
        self.length += step.unwrap_or(0.0);
        // Keep the path no longer than one circle at the body's distance
        // from the barycenter: roughly one orbit, whatever its size.
        let one_orbit = std::f64::consts::TAU * position.length();
        while self.points.len() > 2 {
            let oldest = (self.points[1] - self.points[0]).length();
            if self.length - oldest < one_orbit && self.points.len() <= Self::CAPACITY {
                break;
            }
            self.points.pop_front();
            self.length -= oldest;
        }
    }

    /// The recorded positions, oldest first, in m.
    pub fn points(&self) -> impl ExactSizeIterator<Item = DVec3> + '_ {
        self.points.iter().copied()
    }
}

/// One belt of asteroids or Kuiper belt objects, placed for drawing.
///
/// Its bodies are particles of the hierarchy's swarm: live, feeling every
/// massive body, including any added (see `docs/physics/swarm.md`).
pub struct BeltCloud {
    /// Which belt.
    pub kind: BeltKind,
    /// Its bodies' swarm labels: from here up to the next belt's first.
    first_id: u32,
    /// Where its bodies are now (those not swallowed), in m.
    pub positions: Vec<DVec3>,
}

/// A small moon torn from its planet: a swarm particle now.
#[derive(Debug, Clone, PartialEq)]
struct FreedMoon {
    /// Its swarm label.
    id: u32,
    name: String,
    /// Its planet's name.
    planet: String,
    /// Its place in its planet's list of small moons.
    index: usize,
    gm: f64,
    radius: f64,
}

/// Where a body in the flat list lives in the hierarchy.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// A top-level body.
    Top(usize),
    /// Body `body` of moon system `system` (body 0 is the planet).
    Moon { system: usize, body: usize },
    /// Small moon `index` of moon system `system`.
    Small { system: usize, index: usize },
    /// A massless body following the top level (a comet, a small asteroid).
    Follower(usize),
    /// A small moon set free: a swarm particle, by its label.
    Particle(u32),
    /// A body of the galactic center.
    Galactic(Far),
}

/// Where a galactic-center body lives in that region's hierarchy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Far {
    /// A top-level body: Sagittarius A* (0), or one added near it.
    Top(usize),
    /// A star following it.
    Follower(usize),
}

/// A region far from the solar system, simulated on its own: the galactic
/// center. The two regions don't pull on each other: 8,277 pc apart, each
/// one's tide on the other is below 10⁻²⁰ m/s². The region holds still
/// relative to the solar system: the galaxy's mass, which carries the Sun
/// around it every 230 million years, isn't simulated (v2).
pub struct Region {
    /// Where its origin (Sagittarius A*) is, in the solar system's frame.
    pub origin: DVec3,
    hierarchy: Hierarchy,
    /// Each star's spectral type ('e' early, 'l' late), in follower order.
    spectral_types: Vec<Option<char>>,
}

/// Which part of the simulation computes a body's motion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Computed {
    /// The top level: relativistic N-body gravity with the Sun and planets.
    TopLevel,
    /// A major moon, in its planet's moon system.
    MoonSystem,
    /// A small moon followed in detail, along the major moons' paths.
    SmallMoonInDetail,
    /// A small moon placed by its mean orbit.
    SmallMoonOnMeanOrbit,
    /// A massless body following the top level; `forces` if it feels
    /// non-gravitational forces too.
    Follower { forces: bool },
    /// A massless particle of the swarm.
    Swarm,
    /// A star following Sagittarius A*.
    GalacticStar,
}

/// Whether a body can be removed from the simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// Yes, along with this many moons.
    Allowed { moons: usize },
    /// Not the Sun: everything is measured from it.
    Sun,
    /// Not a moon on its own, yet.
    Moon,
    /// Not Sagittarius A*: its region is measured from it.
    GalacticCenter,
}

/// The solar system running live.
pub struct Simulation {
    /// Every body: the Sun and planets (each planet with moons in place of
    /// its system's barycenter) in the snapshot's order, the dwarf planets
    /// and asteroids heavy enough to pull on them, then the major moons, the
    /// small moons, and the massless asteroids and comets. Positions are
    /// refreshed after every step (the small moons' once per update).
    pub bodies: Vec<Body>,
    /// The starting moment, as a Julian Date (TDB).
    pub epoch_jd_tdb: f64,
    /// Simulated seconds per real second.
    pub speed: f64,
    /// Whether time is stopped.
    pub paused: bool,
    /// One trail per body, in the same order as the bodies (empty for moons).
    pub trails: Vec<Trail>,
    /// Fraction of the requested speed the last update achieved: 1.0 when
    /// physics kept up, less when it ran out of time.
    pub achieved: f64,
    /// The body each one closely orbits (a moon's planet), if any.
    parents: Vec<Option<usize>>,
    sources: Vec<Source>,
    /// IAU rotation model of each body, where one exists.
    rotations: Vec<Option<RotationModel>>,
    /// Each moon system's small moons, from JPL.
    small: Vec<Vec<SmallMoon>>,
    /// The moon system whose small moons are being computed in detail.
    detailed: Option<usize>,
    /// For each body, what sort of small body it is, if it is one, and
    /// whether it has a non-gravitational force model.
    small_bodies: Vec<Option<(SmallBodyKind, bool)>>,
    /// The bodies added in the sandbox: each one's name and the key of the
    /// catalogue entry it came from.
    added: Vec<(String, String)>,
    /// The asteroid belt, Jupiter's Trojans and the Kuiper belt.
    pub belts: Vec<BeltCloud>,
    /// The simulation time the belts were last placed at.
    belts_time: f64,
    /// Small moons torn from their planets, in label order.
    freed: Vec<FreedMoon>,
    /// Where each swarm particle is now, in the swarm's order.
    swarm_positions: Vec<DVec3>,
    /// What happened to the swarm since the app last looked: sentences for
    /// the top bar.
    pub notices: Vec<String>,
    /// The solar wind: 2025's average wind at Earth, on Parker's spiral.
    pub wind: ParkerSpiral,
    /// The heliosphere's boundaries, through the Voyager crossings.
    pub heliosphere: Heliosphere,
    /// Where Voyager 1 and 2 crossed them.
    pub crossings: Vec<VoyagerCrossing>,
    /// Each body's magnetic dipole and the model it comes from, for the
    /// planets that have a global field.
    pub dipoles: Vec<Option<(Dipole, String)>>,
    /// Earth's radiation belts.
    pub radiation_belts: Vec<RadiationBelt>,
    /// The solar wind's average flow pressure at 1 AU in 2025, in Pa.
    pub flow_pressure_at_1au: f64,
    /// Collisions since the app last looked, oldest first.
    pub events: Vec<Collision>,
    /// The galactic center.
    pub galaxy: Option<Region>,
    hierarchy: Hierarchy,
}

impl Simulation {
    /// The real solar system on 2025-01-01: relativistic gravity between
    /// the Sun, planets and heaviest asteroids, each planet with moons
    /// simulated in its own frame, and the lighter asteroids and comets
    /// following along.
    pub fn solar_system(speed: f64) -> Self {
        let epoch_jd_tdb = worldline_data::solar_system().epoch_jd_tdb;
        let mut hierarchy = worldline_data::full_solar_system();
        // The belts, as live particles labeled 0, 1, 2, … belt by belt.
        hierarchy.add_particles(worldline_data::belt_particles());
        let all_small = worldline_data::small_moons();
        let small: Vec<Vec<SmallMoon>> = hierarchy
            .moon_systems
            .iter()
            .map(|moons| {
                let planet = &moons.system.bodies[0].name;
                all_small
                    .iter()
                    .filter(|m| &m.parent == planet)
                    .cloned()
                    .collect()
            })
            .collect();
        let mut simulation = Self {
            bodies: Vec::new(),
            trails: Vec::new(),
            rotations: Vec::new(),
            epoch_jd_tdb,
            speed,
            paused: false,
            achieved: 1.0,
            parents: Vec::new(),
            sources: Vec::new(),
            small,
            detailed: None,
            small_bodies: Vec::new(),
            added: Vec::new(),
            belts: worldline_data::belts()
                .into_iter()
                .scan(0, |first, belt| {
                    let cloud = BeltCloud {
                        kind: belt.kind,
                        first_id: *first,
                        positions: Vec::new(),
                    };
                    *first += belt.orbits.len() as u32;
                    Some(cloud)
                })
                .collect(),
            belts_time: f64::NAN,
            freed: Vec::new(),
            swarm_positions: Vec::new(),
            notices: Vec::new(),
            galaxy: {
                let gc = worldline_data::galactic_center();
                Some(Region {
                    origin: gc.origin,
                    hierarchy: gc.hierarchy,
                    spectral_types: worldline_data::s_stars()
                        .iter()
                        .map(|s| s.spectral_type)
                        .collect(),
                })
            },
            wind: worldline_data::parker_spiral(),
            heliosphere: worldline_data::heliosphere(),
            crossings: worldline_data::voyager_crossings(),
            dipoles: Vec::new(),
            radiation_belts: worldline_data::radiation_belts(),
            flow_pressure_at_1au: worldline_data::mean_flow_pressure(),
            events: Vec::new(),
            hierarchy,
        };
        // Their mean orbits, for freeing them if they are torn away.
        for (moons, small) in simulation
            .hierarchy
            .moon_systems
            .iter_mut()
            .zip(&simulation.small)
        {
            moons.small_mean_orbits = small.iter().map(|m| m.orbit.clone()).collect();
        }
        simulation.reindex();
        simulation.refresh();
        simulation.refresh_small_moons();
        simulation
    }

    /// Rebuilds the flat list of bodies from the hierarchy: the Sun and
    /// planets (each planet with moons in place of its system's barycenter)
    /// in order, then the major moons, the small moons and the followers.
    /// Run whenever the set of bodies changes; trails are kept by name.
    fn reindex(&mut self) {
        let hierarchy = &self.hierarchy;
        let mut sources: Vec<Source> = (0..hierarchy.top.bodies.len()).map(Source::Top).collect();
        for (system, moons) in hierarchy.moon_systems.iter().enumerate() {
            sources[moons.host] = Source::Moon { system, body: 0 };
        }
        for (system, moons) in hierarchy.moon_systems.iter().enumerate() {
            sources
                .extend((1..moons.system.bodies.len()).map(|body| Source::Moon { system, body }));
        }
        for (system, moons) in self.small.iter().enumerate() {
            let free = |index: &usize| hierarchy.moon_systems[system].is_small_moon_free(*index);
            sources.extend(
                (0..moons.len())
                    .filter(|index| !free(index))
                    .map(|index| Source::Small { system, index }),
            );
        }
        sources.extend((0..hierarchy.follower_count()).map(Source::Follower));
        sources.extend(self.freed.iter().map(|m| Source::Particle(m.id)));
        if let Some(g) = &self.galaxy {
            sources
                .extend((0..g.hierarchy.top.bodies.len()).map(|k| Source::Galactic(Far::Top(k))));
            sources.extend(
                (0..g.hierarchy.follower_count()).map(|i| Source::Galactic(Far::Follower(i))),
            );
        }
        let bodies: Vec<Body> = sources
            .iter()
            .map(|source| match *source {
                Source::Top(k) => hierarchy.top.bodies[k].clone(),
                Source::Moon { system, body } => {
                    hierarchy.moon_systems[system].system.bodies[body].clone()
                }
                // Unknown masses and sizes are zero: no pull, drawn as a dot.
                Source::Small { system, index } => {
                    let moon = &self.small[system][index];
                    Body::new(
                        &moon.name,
                        moon.gm.unwrap_or(0.0),
                        moon.radius.unwrap_or(0.0),
                    )
                }
                Source::Follower(i) => hierarchy.follower(i).clone(),
                Source::Particle(id) => {
                    let moon = self
                        .freed
                        .iter()
                        .find(|m| m.id == id)
                        .expect("a freed moon");
                    Body::new(&moon.name, moon.gm, moon.radius)
                }
                Source::Galactic(far) => {
                    let g = self.galaxy.as_ref().expect("the galactic center");
                    match far {
                        Far::Top(k) => g.hierarchy.top.bodies[k].clone(),
                        Far::Follower(i) => g.hierarchy.follower(i).clone(),
                    }
                }
            })
            .collect();
        let galactic_center = sources
            .iter()
            .position(|s| matches!(s, Source::Galactic(Far::Top(0))));
        let catalog = worldline_data::small_bodies();
        let fields = worldline_data::planetary_fields();
        let index_of = |name: &str| bodies.iter().position(|b| b.name == name);
        let parents = sources
            .iter()
            .zip(&bodies)
            .map(|(source, body)| match *source {
                Source::Moon { system, body } if body > 0 => {
                    Some(hierarchy.moon_systems[system].host)
                }
                Source::Small { system, .. } => Some(hierarchy.moon_systems[system].host),
                Source::Galactic(Far::Follower(_)) => galactic_center,
                // The Moon is simulated at the top level, beside Earth.
                _ if body.name == "Moon" => index_of("Earth"),
                _ => None,
            })
            .collect();
        let known = |b: &Body| !self.added.iter().any(|(name, _)| name == &b.name);
        self.small_bodies = bodies
            .iter()
            .map(|b| {
                catalog
                    .iter()
                    .find(|s| known(b) && s.name == b.name)
                    .map(|s| (s.kind, s.forces.is_some()))
            })
            .collect();
        self.rotations = bodies
            .iter()
            .map(|b| {
                known(b)
                    .then(|| worldline_data::rotation_model(&b.name))
                    .flatten()
            })
            .collect();
        self.dipoles = bodies
            .iter()
            .map(|b| {
                fields
                    .iter()
                    .find(|f| known(b) && f.planet == b.name)
                    .map(|f| (f.dipole, f.model.clone()))
            })
            .collect();
        let mut old: std::collections::HashMap<String, Trail> = self
            .bodies
            .iter()
            .map(|b| b.name.clone())
            .zip(std::mem::take(&mut self.trails))
            .collect();
        self.trails = bodies
            .iter()
            .map(|b| old.remove(&b.name).unwrap_or_default())
            .collect();
        self.parents = parents;
        self.sources = sources;
        self.bodies = bodies;
    }

    /// Adds a body to the simulation, given relative to the solar system's
    /// barycenter: it joins the Sun and planets, pulling on them and pulled
    /// by them with relativistic gravity, and the moons feel its tides.
    /// Returns its place in the list.
    pub fn add_body(&mut self, body: Body, entry: &str) -> usize {
        let name = body.name.clone();
        self.hierarchy.add_body(body);
        self.added.push((name.clone(), entry.to_string()));
        self.reindex();
        self.refresh();
        self.refresh_small_moons();
        self.index_of(&name).expect("the new body is listed")
    }

    /// Adds a body to the sandbox in the region of body `near`: the
    /// galactic center if `near` is there, else the solar system. Given in
    /// the solar system's frame; returns its index in the list.
    pub fn add_body_near(&mut self, body: Body, entry: &str, near: usize) -> usize {
        let Source::Galactic(_) = self.sources[near] else {
            return self.add_body(body, entry);
        };
        let name = body.name.clone();
        let g = self.galaxy.as_mut().expect("the galactic center");
        let local = Body {
            position: body.position - g.origin,
            ..body
        };
        g.hierarchy.add_body(local);
        self.added.push((name.clone(), entry.to_string()));
        self.reindex();
        self.refresh();
        self.index_of(&name).expect("the new body is listed")
    }

    /// The body body `index`'s region is measured from: Sagittarius A* in
    /// the galactic center, the Sun (or what took its place) elsewhere.
    pub fn region_center(&self, index: usize) -> usize {
        match self.sources[index] {
            Source::Galactic(_) => self
                .sources
                .iter()
                .position(|s| matches!(s, Source::Galactic(Far::Top(0))))
                .unwrap_or(0),
            _ => 0,
        }
    }

    /// For Sagittarius A* in the galactic center: how far to stand back to
    /// see the stars around it, twice the median of their distances.
    pub fn cluster_reach(&self, index: usize) -> Option<f64> {
        let Source::Galactic(Far::Top(0)) = self.sources[index] else {
            return None;
        };
        let center = self.bodies[index].position;
        let mut distances: Vec<f64> = (0..self.bodies.len())
            .filter(|&i| self.parents[i] == Some(index))
            .map(|i| (self.bodies[i].position - center).length())
            .collect();
        distances.sort_by(f64::total_cmp);
        distances.get(distances.len() / 2).map(|d| 2.0 * d)
    }

    /// Whether body `index` is in the galactic center.
    pub fn is_galactic(&self, index: usize) -> bool {
        matches!(self.sources[index], Source::Galactic(_))
    }

    /// For a star of the galactic center: its spectral type ('e' early,
    /// 'l' late), if known.
    pub fn spectral_type(&self, index: usize) -> Option<char> {
        let Source::Galactic(Far::Follower(i)) = self.sources[index] else {
            return None;
        };
        self.galaxy
            .as_ref()?
            .spectral_types
            .get(i)
            .copied()
            .flatten()
    }

    /// Whether body `index` was added in the sandbox.
    pub fn is_added(&self, index: usize) -> bool {
        self.added
            .iter()
            .any(|(name, _)| name == &self.bodies[index].name)
    }

    /// The catalogue entry body `index` was added from, if it was added.
    pub fn entry(&self, index: usize) -> Option<&'static Entry> {
        let name = &self.bodies[index].name;
        self.added
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, key)| catalogue::entry(key))
    }

    /// Whether the Sun is still there: something that hits it can absorb
    /// it (a black hole, or a heavier star) and take its place as body 0.
    /// Without it there is no sunlight, solar wind or heliosphere.
    pub fn has_sun(&self) -> bool {
        self.bodies[0].name == "Sun" && !self.is_added(0)
    }

    /// Whether body `index` gives off its own light: the Sun, and stars,
    /// white dwarfs and neutron stars added from the catalogue.
    pub fn shines(&self, index: usize) -> bool {
        if index == 0 && self.has_sun() {
            return true;
        }
        self.entry(index).is_some_and(|e| {
            e.key == "copy:Sun"
                || matches!(
                    e.kind(),
                    Some(ObjectKind::Star | ObjectKind::WhiteDwarf | ObjectKind::NeutronStar)
                )
        })
    }

    /// Where the planets' light comes from: body 0, if it shines (the Sun,
    /// or a star that took its place). None if a black hole took it.
    pub fn light_source(&self) -> Option<DVec3> {
        self.shines(0).then(|| self.bodies[0].position)
    }

    /// The place in the list of the body named `name`, if it is there.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.name == name)
    }

    /// Whether body `index` can be removed, and what goes with it.
    pub fn removal(&self, index: usize) -> Removal {
        match self.sources[index] {
            Source::Top(0) => Removal::Sun,
            Source::Top(_) | Source::Follower(_) | Source::Particle(_) => {
                Removal::Allowed { moons: 0 }
            }
            Source::Moon { system, body: 0 } => Removal::Allowed {
                moons: self.hierarchy.moon_systems[system].system.bodies.len() - 1
                    + self.small[system].len(),
            },
            Source::Moon { .. } | Source::Small { .. } => Removal::Moon,
            Source::Galactic(Far::Top(0)) => Removal::GalacticCenter,
            Source::Galactic(_) => Removal::Allowed { moons: 0 },
        }
    }

    /// Removes body `index` (and a planet's moons with it), if allowed (see
    /// [`Self::removal`]).
    pub fn remove(&mut self, index: usize) {
        if !matches!(self.removal(index), Removal::Allowed { .. }) {
            return;
        }
        let name = self.bodies[index].name.clone();
        match self.sources[index] {
            Source::Top(k) => self.hierarchy.remove_body(k),
            Source::Moon { system, .. } => {
                let host = self.hierarchy.moon_systems[system].host;
                self.hierarchy.remove_body(host);
                self.small.remove(system);
                self.detailed = match self.detailed {
                    Some(d) if d == system => None,
                    Some(d) if d > system => Some(d - 1),
                    other => other,
                };
            }
            Source::Follower(i) => self.hierarchy.remove_follower(i),
            Source::Particle(id) => {
                self.hierarchy.remove_particles(&[id]);
                self.freed.retain(|m| m.id != id);
            }
            Source::Galactic(far) => {
                let g = self.galaxy.as_mut().expect("the galactic center");
                match far {
                    Far::Top(k) => g.hierarchy.remove_body(k),
                    Far::Follower(i) => {
                        g.hierarchy.remove_follower(i);
                        g.spectral_types.remove(i);
                    }
                }
            }
            Source::Small { .. } => return,
        }
        self.added.retain(|(n, _)| n != &name);
        self.reindex();
        self.refresh();
        self.refresh_small_moons();
    }

    /// Computes the small moons around body `index` in detail: those of
    /// its planet, if it is a planet with moons or one of their moons. All
    /// other small moons go back to JPL's mean orbits.
    ///
    /// At the snapshot moment the detailed moons start from JPL's exact
    /// states; later, from their mean orbits at that moment.
    pub fn focus_detail_on(&mut self, index: usize) {
        let planet = self.parents[index].unwrap_or(index);
        let system = self
            .hierarchy
            .moon_systems
            .iter()
            .position(|m| m.host == planet);
        if system == self.detailed {
            return;
        }
        if let Some(old) = self.detailed {
            self.hierarchy.moon_systems[old].clear_small_moons();
        }
        self.detailed = system;
        if let Some(system) = system {
            let time = self.time();
            let moons = self.small[system]
                .iter()
                .map(|moon| {
                    let (r, v) = match moon.state {
                        Some(state) if time == 0.0 => state,
                        _ => moon.orbit.state_at(time),
                    };
                    let body = Body::new(
                        &moon.name,
                        moon.gm.unwrap_or(0.0),
                        moon.radius.unwrap_or(0.0),
                    )
                    .at(r)
                    .moving(v);
                    (body, moon.regular)
                })
                .collect();
            self.hierarchy.moon_systems[system].set_small_moons(moons);
        }
        self.refresh_small_moons();
    }

    /// What sort of small body body `index` is (a dwarf planet, an
    /// asteroid, a comet…), and whether it feels non-gravitational forces,
    /// if it is one.
    pub fn small_body(&self, index: usize) -> Option<(SmallBodyKind, bool)> {
        self.small_bodies[index]
    }

    /// Whether body `index` is a small moon.
    pub fn is_small_moon(&self, index: usize) -> bool {
        matches!(self.sources[index], Source::Small { .. })
    }

    /// Whether body `index` is a small moon being computed in detail, rather
    /// than placed by its mean orbit.
    pub fn is_detailed(&self, index: usize) -> bool {
        matches!(self.sources[index], Source::Small { system, .. } if Some(system) == self.detailed)
    }

    /// Simulated time since the start, in s.
    pub fn time(&self) -> f64 {
        self.hierarchy.time()
    }

    /// The body that body `index` closely orbits (a moon's planet), if any.
    pub fn parent(&self, index: usize) -> Option<usize> {
        self.parents[index]
    }

    /// Whether body `index` belongs in the view while the camera follows
    /// body `focus` and body `selected` is selected. Moons appear only
    /// around the planet in focus (the followed planet, or the planet of the
    /// followed moon), or when selected themselves. Every other planet stays
    /// one clean dot, without its moons' labels and orbits crowding the
    /// view.
    pub fn in_view(&self, index: usize, focus: usize, selected: usize) -> bool {
        match self.parents[index] {
            Some(planet) => planet == self.parents[focus].unwrap_or(focus) || index == selected,
            None => true,
        }
    }

    /// How body `index` is oriented and spins, if known.
    pub fn rotation(&self, index: usize) -> Option<&RotationModel> {
        self.rotations.get(index)?.as_ref()
    }

    /// The gravity model between the Sun and planets.
    pub fn gravity(&self) -> &dyn Gravity {
        self.hierarchy.gravity()
    }

    /// The gravity model inside each planet's moon system.
    pub fn moon_gravity_name(&self) -> &'static str {
        MOON_SYSTEM_GRAVITY
    }

    /// The integrator in use, at both levels.
    pub fn integrator_name(&self) -> &'static str {
        Ias15::new().name()
    }

    /// The current moment, as a Julian Date (TDB).
    pub fn julian_date(&self) -> f64 {
        self.epoch_jd_tdb + self.time() / DAY
    }

    /// Advances the simulation by `seconds` of simulated time, however
    /// long that takes to compute.
    pub fn advance_by(&mut self, seconds: f64) {
        let target = self.time() + seconds;
        while self.time() < target {
            self.step_toward(target);
        }
        self.catch_up_galaxy();
        self.refresh_small_moons();
    }

    /// Advances the simulation to match `real_dt` seconds of real time,
    /// spending at most `budget` of computer time on it.
    pub fn update(&mut self, real_dt: f64, budget: Duration) {
        let requested = if self.paused {
            0.0
        } else {
            self.speed * real_dt
        };
        if requested <= 0.0 {
            self.achieved = 1.0;
            return;
        }
        let start = self.time();
        let target = start + requested;
        let clock = Instant::now();
        while self.time() < target && clock.elapsed() < budget {
            self.step_toward(target);
        }
        self.achieved = (self.time() - start) / requested;
        self.catch_up_galaxy();
        self.refresh_small_moons();
    }

    /// One step of the whole hierarchy, landing exactly on `target` if it
    /// gets there.
    fn step_toward(&mut self, target: f64) {
        let remaining = target - self.time();
        let taken = self.hierarchy.step(remaining);
        if taken >= remaining {
            self.hierarchy.set_time(target);
        }

        // Small moons freed (before their systems' lists go): name them.
        let freed = self.hierarchy.take_freed_moons();
        if !freed.is_empty() {
            for f in &freed {
                let Some(moon) = self
                    .small
                    .iter()
                    .flatten()
                    .filter(|m| m.parent == f.planet)
                    .nth(f.index)
                else {
                    continue;
                };
                self.freed.push(FreedMoon {
                    id: f.id,
                    name: moon.name.clone(),
                    planet: f.planet.clone(),
                    index: f.index,
                    gm: moon.gm.unwrap_or(0.0),
                    radius: moon.radius.unwrap_or(0.0),
                });
            }
            let (first, by) = (&freed[0], &freed[0].by);
            if !by.is_empty() {
                self.notices.push(match freed.len() {
                    1 => format!(
                        "{by}'s tides pulled {} away from {}",
                        self.freed[self.freed.len() - 1].name,
                        first.planet
                    ),
                    n => format!(
                        "{by}'s tides pulled {n} small moons away from {}",
                        first.planet
                    ),
                });
            }
            self.reindex();
        }
        for swallowed in self.hierarchy.take_swallowed() {
            let moons: Vec<String> = self
                .freed
                .iter()
                .filter(|m| swallowed.ids.contains(&m.id))
                .map(|m| m.name.clone())
                .collect();
            let others = swallowed.ids.len() - moons.len();
            let mut what = Vec::new();
            match others {
                0 => {}
                1 => what.push("a belt body".to_string()),
                n => what.push(format!("{n} belt bodies")),
            }
            what.extend(moons);
            self.notices
                .push(format!("{} swallowed {}", swallowed.by, what.join(", ")));
            self.freed.retain(|m| !swallowed.ids.contains(&m.id));
            self.reindex();
        }
        for unbound in self.hierarchy.take_unbound() {
            self.notices.push(format!(
                "{}'s tides tore {}'s major moons away",
                unbound.by, unbound.planet
            ));
            // The freed moons are top-level bodies now, and its small moons
            // swarm particles.
            self.small.remove(unbound.system);
            self.detailed = match self.detailed {
                Some(d) if d == unbound.system => None,
                Some(d) if d > unbound.system => Some(d - 1),
                other => other,
            };
            self.reindex();
        }
        let collisions = self.hierarchy.take_collisions();
        if !collisions.is_empty() {
            self.after_collisions(&collisions);
            self.events.extend(collisions);
        }
        self.refresh();
    }

    /// Brings the galactic center up to the solar system's time, in its own
    /// steps: once per update is enough, since nothing in either region
    /// feels the other.
    fn catch_up_galaxy(&mut self) {
        let Some(g) = &mut self.galaxy else {
            return;
        };
        let now = self.hierarchy.time();
        while g.hierarchy.time() < now {
            let left = now - g.hierarchy.time();
            if g.hierarchy.step(left) >= left {
                g.hierarchy.set_time(now);
            }
        }
        let collisions = g.hierarchy.take_collisions();
        if !collisions.is_empty() {
            for c in &collisions {
                self.added.retain(|(name, _)| name != &c.absorbed);
            }
            self.events.extend(collisions);
            self.reindex();
        }
        self.refresh();
    }

    /// Brings the list of bodies up to date after collisions: a planet
    /// that was absorbed takes its small moons with it, and a planet that
    /// absorbed something has its small moons placed again.
    fn after_collisions(&mut self, collisions: &[Collision]) {
        for collision in collisions {
            if let Some(system) = collision.dissolved_system {
                self.small.remove(system);
                self.detailed = match self.detailed {
                    Some(d) if d == system => None,
                    Some(d) if d > system => Some(d - 1),
                    other => other,
                };
            }
            self.added.retain(|(name, _)| name != &collision.absorbed);
        }
        self.reindex();
        if let Some(system) = self.detailed {
            let moons = &self.hierarchy.moon_systems[system];
            if moons.small_moon_count() != self.small[system].len() {
                let planet = moons.host;
                self.detailed = None;
                self.focus_detail_on(planet);
            }
        }
    }

    /// Which part of the simulation computes body `index`'s motion.
    pub fn computed(&self, index: usize) -> Computed {
        match self.sources[index] {
            Source::Top(_) | Source::Moon { body: 0, .. } => Computed::TopLevel,
            Source::Moon { .. } => Computed::MoonSystem,
            Source::Small { system, .. } if Some(system) == self.detailed => {
                Computed::SmallMoonInDetail
            }
            Source::Small { .. } => Computed::SmallMoonOnMeanOrbit,
            Source::Particle(_) => Computed::Swarm,
            Source::Galactic(Far::Top(_)) => Computed::TopLevel,
            Source::Galactic(Far::Follower(_)) => Computed::GalacticStar,
            Source::Follower(_) => Computed::Follower {
                forces: self.small_bodies[index].is_some_and(|s| s.1),
            },
        }
    }

    /// How strong gravity is at body `index` and how fast it moves,
    /// relative to what pulls on it hardest: its planet, for a moon; for
    /// anything else, the top-level body with the strongest pull.
    pub fn regime(&self, index: usize) -> Option<Regime> {
        let attractor = self.attractor(index)?;
        Some(Regime::of(&self.bodies[index], &self.bodies[attractor]))
    }

    /// What pulls on body `index` hardest: its planet, for a moon; for
    /// anything else, the top-level body with the strongest pull.
    fn attractor(&self, index: usize) -> Option<usize> {
        let body = &self.bodies[index];
        let top = match (self.sources[index], &self.galaxy) {
            (Source::Galactic(_), Some(g)) => &g.hierarchy.top,
            _ => &self.hierarchy.top,
        };
        match self.parents[index] {
            Some(planet) => Some(planet),
            None => top
                .bodies
                .iter()
                .enumerate()
                .filter(|(_, b)| b.name != body.name && b.gm > 0.0)
                .max_by(|(_, a), (_, b)| {
                    let pull = |x: &Body| x.gm / (x.position - body.position).length_squared();
                    pull(a).total_cmp(&pull(b))
                })
                .and_then(|(_, b)| self.index_of(&b.name)),
        }
    }

    /// For a body in a bound pair with what pulls on it hardest, if the
    /// gravitational waves they give off will merge them within the age of
    /// the universe: how fast the orbit's period shrinks (s/s, Peters &
    /// Mathews 1963) and how long until they merge (s, Peters 1964), from
    /// their two-body orbit as it is now.
    pub fn gravitational_waves(&self, index: usize) -> Option<(f64, f64)> {
        if self.computed(index) != Computed::TopLevel {
            return None;
        }
        let (body, other) = (&self.bodies[index], &self.bodies[self.attractor(index)?]);
        if body.gm <= 0.0 || other.gm <= 0.0 {
            return None;
        }
        let mu = body.gm + other.gm;
        let r = body.position - other.position;
        let v = body.velocity - other.velocity;
        let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        // The eccentricity vector, (v × h)/μ − r̂ with h = r × v.
        let e = (v.cross(r.cross(v)) / mu - r / r.length()).length();
        if a <= 0.0 || e >= 1.0 {
            return None;
        }
        let period = std::f64::consts::TAU * (a.powi(3) / mu).sqrt();
        let merging = merger_time(body.gm, other.gm, period, e);
        (merging < AGE_OF_UNIVERSE)
            .then(|| (period_derivative(body.gm, other.gm, period, e), merging))
    }

    /// For a moon: another massive body inside its planet's moon system
    /// (closer to the planet than the system's farthest major moon), whose
    /// collisions with the moons go undetected.
    pub fn intruder(&self, index: usize) -> Option<&str> {
        let planet = self.parents[index]?;
        let system = self
            .hierarchy
            .moon_systems
            .iter()
            .find(|m| m.host == planet)?;
        let reach = system
            .system
            .bodies
            .iter()
            .skip(1)
            .map(|b| (b.position - system.system.bodies[0].position).length())
            .fold(0.0, f64::max);
        let center = &self.bodies[planet];
        self.hierarchy
            .top
            .bodies
            .iter()
            .filter(|b| b.name != center.name && b.gm > 0.0)
            .find(|b| (b.position - center.position).length() < reach)
            .map(|b| b.name.as_str())
    }

    /// Copies positions and velocities out of the hierarchy and extends
    /// the trails of bodies that orbit the Sun.
    fn refresh(&mut self) {
        for (body, source) in self.bodies.iter_mut().zip(&self.sources) {
            let (position, velocity) = match *source {
                Source::Top(k) => {
                    let b = &self.hierarchy.top.bodies[k];
                    (b.position, b.velocity)
                }
                Source::Moon { system, body } => self.hierarchy.absolute(system, body),
                Source::Small { .. } | Source::Particle(_) => continue,
                Source::Galactic(far) => {
                    let g = self.galaxy.as_ref().expect("the galactic center");
                    let b = match far {
                        Far::Top(k) => &g.hierarchy.top.bodies[k],
                        Far::Follower(i) => g.hierarchy.follower(i),
                    };
                    (g.origin + b.position, b.velocity)
                }
                Source::Follower(i) => {
                    let b = self.hierarchy.follower(i);
                    (b.position, b.velocity)
                }
            };
            body.position = position;
            body.velocity = velocity;
        }
        let time = self.hierarchy.time();
        for ((trail, body), parent) in self.trails.iter_mut().zip(&self.bodies).zip(&self.parents) {
            if parent.is_none() {
                trail.record(time, body.position, body.velocity);
            }
        }
    }

    /// Places the small moons: from the detailed simulation for the moon
    /// system in focus, from JPL's mean orbits for the rest. Done once per
    /// update rather than every step, since the mean orbits can be
    /// evaluated at any time. The belts likewise.
    fn refresh_small_moons(&mut self) {
        let time = self.time();
        if time != self.belts_time {
            // The swarm's particles, carried from its clock to now on their
            // two-body orbits (shared among all cores), sorted out by label:
            // the belts in their ranges, then the freed moons.
            self.hierarchy.swarm_positions(&mut self.swarm_positions);
            let particles = &self.hierarchy.swarm().particles;
            let mut next = 0;
            for k in 0..self.belts.len() {
                let end = self.belts.get(k + 1).map_or(FREED_MOON_IDS, |b| b.first_id);
                let start = next;
                while next < particles.len() && particles[next].id < end {
                    next += 1;
                }
                self.belts[k].positions.clear();
                self.belts[k]
                    .positions
                    .extend_from_slice(&self.swarm_positions[start..next]);
            }
            self.belts_time = time;
        }
        let particles = &self.hierarchy.swarm().particles;
        for (body, source) in self.bodies.iter_mut().zip(&self.sources) {
            if let Source::Particle(id) = *source {
                if let Ok(k) = particles.binary_search_by_key(&id, |p| p.id) {
                    body.position = self.swarm_positions[k];
                    body.velocity = particles[k].velocity;
                }
                continue;
            }
            let Source::Small { system, index } = *source else {
                continue;
            };
            let (position, velocity) = if Some(system) == self.detailed {
                self.hierarchy.absolute_small(system, index)
            } else {
                let (planet, drift) = self.hierarchy.absolute(system, 0);
                let (r, v) = self.small[system][index].orbit.state_at(time);
                (planet + r, drift + v)
            };
            body.position = position;
            body.velocity = velocity;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::constants::GM_SUN;

    const GENEROUS: Duration = Duration::from_secs(10);

    #[test]
    fn one_real_second_advances_by_the_speed() {
        let mut sim = Simulation::solar_system(DAY);
        sim.update(1.0, GENEROUS);
        assert_eq!(sim.time(), DAY);
        assert_eq!(sim.achieved, 1.0);
        assert_eq!(sim.julian_date(), sim.epoch_jd_tdb + 1.0);
    }

    #[test]
    fn paused_means_no_time_passes() {
        let mut sim = Simulation::solar_system(DAY);
        sim.paused = true;
        sim.update(1.0, GENEROUS);
        assert_eq!(sim.time(), 0.0);
    }

    #[test]
    fn running_out_of_budget_is_reported() {
        let mut sim = Simulation::solar_system(1000.0 * DAY);
        sim.update(1.0, Duration::ZERO);
        assert!(sim.achieved < 1.0);
    }

    #[test]
    fn planets_stand_in_for_their_systems_and_moons_follow() {
        // Jupiter's entry is the planet itself, not its system's
        // barycenter, and Io is listed with Jupiter as its parent, about
        // 422,000 km away.
        let sim = Simulation::solar_system(DAY);
        let find = |name: &str| sim.bodies.iter().position(|b| b.name == name).unwrap();
        let (jupiter, io) = (find("Jupiter"), find("Io"));
        assert!(
            sim.bodies[jupiter].gm < 1.267e17,
            "Jupiter's GM is the planet's own"
        );
        assert_eq!(sim.parent(io), Some(jupiter));
        assert_eq!(sim.parent(find("Moon")), Some(find("Earth")));
        assert_eq!(sim.parent(jupiter), None);
        let distance = (sim.bodies[io].position - sim.bodies[jupiter].position).length();
        assert!((distance / 4.22e8 - 1.0).abs() < 0.01, "Io at {distance} m");
        // The top level's order is kept: Earth is still body 3.
        assert_eq!(find("Earth"), 3);
    }

    #[test]
    fn small_moons_are_detailed_only_around_the_focus() {
        // Pan starts on its mean orbit; focusing on Saturn computes it in
        // detail from JPL's exact state, and focusing elsewhere sends it
        // back to its mean orbit.
        let mut sim = Simulation::solar_system(DAY);
        let find =
            |sim: &Simulation, name: &str| sim.bodies.iter().position(|b| b.name == name).unwrap();
        let (saturn, pan, io) = (find(&sim, "Saturn"), find(&sim, "Pan"), find(&sim, "Io"));
        assert!(sim.is_small_moon(pan) && !sim.is_detailed(pan));
        assert_eq!(sim.parent(pan), Some(saturn));
        sim.focus_detail_on(saturn);
        assert!(sim.is_detailed(pan));
        let distance = (sim.bodies[pan].position - sim.bodies[saturn].position).length();
        assert!(
            (distance / 133_584e3 - 1.0).abs() < 0.01,
            "Pan at {distance} m"
        );
        sim.update(1.0, GENEROUS);
        sim.focus_detail_on(io);
        assert!(!sim.is_detailed(pan));
    }

    #[test]
    fn moons_show_only_around_the_planet_in_focus() {
        let sim = Simulation::solar_system(DAY);
        let find = |name: &str| sim.bodies.iter().position(|b| b.name == name).unwrap();
        let [sun, earth, moon, jupiter, io, himalia, titan] =
            ["Sun", "Earth", "Moon", "Jupiter", "Io", "Himalia", "Titan"].map(find);
        // Following the Sun: planets show, moons don't.
        for (body, shown) in [(earth, true), (jupiter, true), (moon, false), (io, false)] {
            assert_eq!(
                sim.in_view(body, sun, earth),
                shown,
                "{}",
                sim.bodies[body].name
            );
        }
        // Following Jupiter, or any of its moons: all of Jupiter's moons,
        // major and small, and no one else's.
        for focus in [jupiter, io, himalia] {
            assert!(sim.in_view(io, focus, focus) && sim.in_view(himalia, focus, focus));
            assert!(!sim.in_view(titan, focus, focus) && !sim.in_view(moon, focus, focus));
        }
        // A selected moon shows, even if its planet isn't in focus.
        assert!(sim.in_view(titan, sun, titan));
        assert!(sim.in_view(moon, earth, earth));
    }

    #[test]
    fn the_belts_are_live_particles() {
        // Every belt body is there, as a swarm particle, and a day later
        // they have all moved on. (How close they start to JPL's positions
        // is checked in worldline-data's validation_belts.)
        let mut sim = Simulation::solar_system(DAY);
        let total: usize = sim.belts.iter().map(|b| b.positions.len()).sum();
        assert_eq!(total, 28_331);
        assert_eq!(sim.hierarchy.swarm().len(), 28_331);
        assert!(
            sim.belts
                .iter()
                .flat_map(|b| &b.positions)
                .all(|p| p.is_finite())
        );
        let before: Vec<DVec3> = sim.belts.iter().map(|b| b.positions[0]).collect();
        sim.update(1.0, GENEROUS);
        for (belt, before) in sim.belts.iter().zip(before) {
            assert_ne!(belt.positions[0], before);
        }
    }

    /// A Jupiter-mass planet 1.5 AU out, on a circular orbit.
    #[test]
    fn a_tight_binary_shows_its_gravitational_waves() {
        // The Hulse–Taylor pair from the catalogue, placed 30 AU above the
        // Sun as the sandbox places it: each star's partner pulls hardest,
        // and their waves will merge them within the age of the universe.
        let mut sim = Simulation::solar_system(DAY);
        let pair = catalogue::entry("binary:PSR B1913+16").expect("listed");
        let above = DVec3::new(0.0, 0.0, 30.0 * worldline_core::constants::AU);
        let indices: Vec<usize> = pair
            .bodies(&sim)
            .into_iter()
            .map(|b| {
                let position = b.position + above;
                sim.add_body(b.at(position), &pair.key)
            })
            .collect();
        for index in indices {
            let (shrink, merging) = sim.gravitational_waves(index).expect("a tight pair");
            println!(
                "{}: dP/dt = {shrink:.4e}, merges in {:.0} million years",
                sim.bodies[index].name,
                merging / worldline_core::constants::JULIAN_YEAR / 1e6
            );
            // The catalog orbit is the measured one, so this is general
            // relativity's −2.40 × 10⁻¹² (the roadmap's number).
            assert!((shrink - -2.40e-12).abs() < 0.005e-12);
        }
        // Earth and the Sun would take far longer than the universe's age.
        let earth = sim.index_of("Earth").expect("Earth");
        assert!(sim.gravitational_waves(earth).is_none());
    }

    fn intruder(sim: &Simulation) -> Body {
        let jupiter = &sim.bodies[sim.index_of("Jupiter").unwrap()];
        let r = 1.5 * worldline_core::constants::AU;
        Body::new("New planet 1", jupiter.gm, jupiter.radius)
            .at(DVec3::new(r, 0.0, 0.0))
            .moving(DVec3::new(0.0, (GM_SUN / r).sqrt(), 0.0))
    }

    #[test]
    fn bodies_can_be_added_and_removed() {
        let mut sim = Simulation::solar_system(DAY);
        let new = sim.add_body(intruder(&sim), "copy:Jupiter");
        assert!(sim.is_added(new) && sim.rotation(new).is_none());
        assert_eq!(sim.removal(0), Removal::Sun);
        assert_eq!(sim.removal(sim.index_of("Io").unwrap()), Removal::Moon);
        // Jupiter goes with its 4 major and 111 small moons.
        let jupiter = sim.index_of("Jupiter").unwrap();
        assert_eq!(sim.removal(jupiter), Removal::Allowed { moons: 115 });
        let before = sim.bodies.len();
        sim.remove(jupiter);
        assert_eq!(sim.bodies.len(), before - 116);
        assert!(sim.index_of("Io").is_none() && sim.index_of("Himalia").is_none());
        // Saturn's moons still belong to Saturn, and everything still runs.
        let saturn = sim.index_of("Saturn").unwrap();
        assert_eq!(sim.parent(sim.index_of("Titan").unwrap()), Some(saturn));
        sim.update(1.0, GENEROUS);
        assert_eq!(sim.time(), DAY);
    }

    #[test]
    fn a_save_loads_back_exactly_and_runs_the_same_every_time() {
        let mut sim = Simulation::solar_system(10.0 * DAY);
        sim.add_body(intruder(&sim), "copy:Jupiter");
        sim.remove(sim.index_of("Mars").unwrap());
        sim.focus_detail_on(sim.index_of("Saturn").unwrap());
        sim.update(1.0, GENEROUS);
        let text = sim.save();
        // Every number loads back bit for bit.
        let mut loaded = Simulation::load(&text, DAY).unwrap();
        assert_eq!(loaded.save(), text);
        assert!(loaded.index_of("New planet 1").is_some() && loaded.index_of("Mars").is_none());
        // Two loads of the same save run identically.
        let mut a = Simulation::load(&text, 10.0 * DAY).unwrap();
        let mut b = Simulation::load(&text, 10.0 * DAY).unwrap();
        a.update(1.0, GENEROUS);
        b.update(1.0, GENEROUS);
        assert_eq!(a.save(), b.save());
        assert!(Simulation::load("not a save", DAY).is_err());
    }

    #[test]
    fn a_black_hole_can_take_the_suns_place_and_a_save_keeps_it() {
        // Gaia BH1, let go 0.05 AU from the Sun: it falls in within hours,
        // absorbs the Sun and becomes body 0. The Sun's light, wind and
        // heliosphere go with it, and a save loads back exactly.
        let mut sim = Simulation::solar_system(DAY);
        let entry = catalogue::entry("object:Gaia BH1").expect("listed");
        let hole = entry.bodies(&sim).remove(0);
        let sun = sim.bodies[0].clone();
        let at = sun.position + DVec3::new(0.0, 0.05 * worldline_core::constants::AU, 0.0);
        sim.add_body(hole.at(at).moving(sun.velocity), &entry.key);
        assert!(sim.has_sun() && sim.light_source().is_some());
        sim.update(1.0, GENEROUS);
        let collision = sim.events.first().expect("it fell in");
        assert_eq!(
            (collision.survivor.as_str(), collision.absorbed.as_str()),
            ("Gaia BH1", "Sun")
        );
        assert_eq!(sim.bodies[0].name, "Gaia BH1");
        assert!(!sim.has_sun() && sim.light_source().is_none());
        assert!(sim.entry(0).is_some_and(|e| e.key == "object:Gaia BH1"));
        let text = sim.save();
        let mut loaded = Simulation::load(&text, DAY).unwrap();
        assert_eq!(loaded.save(), text);
        assert!(!loaded.has_sun());
    }

    #[test]
    fn the_galactic_center_runs_alongside_and_takes_what_is_dropped_there() {
        // Sagittarius A* sits 8,277 pc away with its 39 stars, which keep
        // time with the solar system. A planet dropped 1,000 AU from it
        // joins its region and falls toward it, not toward the Sun.
        let mut sim = Simulation::solar_system(DAY);
        let sgr = sim.index_of("Sagittarius A*").expect("the galactic center");
        assert!(sim.is_galactic(sgr));
        let distance = (sim.bodies[sgr].position - sim.bodies[0].position).length();
        assert!((distance / worldline_core::constants::PARSEC - 8277.0).abs() < 1.0);
        let stars = (0..sim.bodies.len())
            .filter(|&i| sim.parent(i) == Some(sgr))
            .count();
        assert_eq!(stars, 39);
        let s2 = sim.index_of("S2").unwrap();
        let before = sim.bodies[s2].position;
        let at =
            sim.bodies[sgr].position + DVec3::new(1000.0 * worldline_core::constants::AU, 0.0, 0.0);
        let planet = Body::new("New planet 1", 4e14, 6.4e6).at(at);
        let new = sim.add_body_near(planet, "copy:Earth", sgr);
        assert!(sim.is_galactic(new));
        sim.update(30.0, GENEROUS);
        assert_ne!(sim.bodies[s2].position, before);
        let new = sim.index_of("New planet 1").unwrap();
        let toward = (sim.bodies[sgr].position - sim.bodies[new].position).normalize();
        assert!(sim.bodies[new].velocity.normalize().dot(toward) > 0.99);
    }

    #[test]
    fn a_collision_merges_bodies_in_the_list() {
        // An Earth-mass planet sent into Earth at 10 km/s from 20,000 km.
        let mut sim = Simulation::solar_system(DAY);
        let earth = sim.bodies[sim.index_of("Earth").unwrap()].clone();
        let toward = DVec3::new(2e7, 0.0, 0.0);
        let impactor = Body::new("New planet 1", earth.gm, earth.radius)
            .at(earth.position + toward)
            .moving(earth.velocity - toward.normalize() * 1e4);
        sim.add_body(impactor, "copy:Earth");
        let before = sim.bodies.len();
        sim.update(1.0, GENEROUS);
        let collision = sim.events.first().expect("they collided");
        // Equal masses: the earlier body, Earth, survives.
        assert_eq!(
            (collision.survivor.as_str(), collision.absorbed.as_str()),
            ("Earth", "New planet 1")
        );
        assert_eq!(sim.bodies.len(), before - 1);
        let merged = &sim.bodies[sim.index_of("Earth").unwrap()];
        assert_eq!(merged.gm, 2.0 * earth.gm);
        assert!(!sim.is_added(sim.index_of("Earth").unwrap()));
    }

    #[test]
    fn trails_cover_about_one_orbit() {
        // After more than a year, Earth's trail should reach back about one
        // orbit: its oldest point lies close to where Earth is now.
        // Run without a time budget, so a busy machine can't cut it short.
        let mut sim = Simulation::solar_system(worldline_core::constants::JULIAN_YEAR);
        sim.advance_by(1.2 * worldline_core::constants::JULIAN_YEAR);
        let earth = &sim.trails[3];
        let oldest = earth.points().next().unwrap();
        let now = sim.bodies[3].position;
        assert!((now - oldest).length() < 0.2 * now.length());
        assert!(
            earth.points().len() > 300,
            "trail too coarse to draw smoothly"
        );
    }
}
