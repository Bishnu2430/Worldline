//! The live simulation behind the window: the solar system, its physics,
//! the clock, and the orbit trails.

mod save;

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use worldline_core::constants::DAY;
use worldline_core::constants::GM_SUN;
use worldline_core::gravity::Gravity;
use worldline_core::hierarchy::{Hierarchy, MOON_SYSTEM_GRAVITY};
use worldline_core::integrator::{Ias15, Integrator};
use worldline_core::magnetosphere::Dipole;
use worldline_core::orbit::KeplerOrbit;
use worldline_core::rotation::RotationModel;
use worldline_core::solar_wind::{Heliosphere, ParkerSpiral};
use worldline_core::{Body, DVec3};
use worldline_data::{BeltKind, RadiationBelt, SmallBodyKind, SmallMoon, VoyagerCrossing};

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
/// Its bodies are massless and far too many to integrate in real time, so
/// each rides a fixed ellipse around the Sun from JPL's elements. Over a
/// year they stray from their true paths by about a ten-thousandth of their
/// distance (see `docs/physics/belts.md`).
pub struct BeltCloud {
    /// Which belt.
    pub kind: BeltKind,
    orbits: Vec<KeplerOrbit>,
    /// Each body's position relative to the Sun at the last update, in m.
    pub positions: Vec<DVec3>,
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
    /// Names of the bodies added in the sandbox.
    added: Vec<String>,
    /// The asteroid belt, Jupiter's Trojans and the Kuiper belt.
    pub belts: Vec<BeltCloud>,
    /// The simulation time the belts were last placed at.
    belts_time: f64,
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
    hierarchy: Hierarchy,
}

impl Simulation {
    /// The real solar system on 2025-01-01: relativistic gravity between
    /// the Sun, planets and heaviest asteroids, each planet with moons
    /// simulated in its own frame, and the lighter asteroids and comets
    /// following along.
    pub fn solar_system(speed: f64) -> Self {
        let epoch_jd_tdb = worldline_data::solar_system().epoch_jd_tdb;
        let hierarchy = worldline_data::full_solar_system();
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
                .map(|belt| BeltCloud {
                    kind: belt.kind,
                    positions: vec![DVec3::ZERO; belt.orbits.len()],
                    orbits: belt
                        .orbits
                        .iter()
                        .map(|elements| KeplerOrbit::new(elements, GM_SUN))
                        .collect(),
                })
                .collect(),
            belts_time: f64::NAN,
            wind: worldline_data::parker_spiral(),
            heliosphere: worldline_data::heliosphere(),
            crossings: worldline_data::voyager_crossings(),
            dipoles: Vec::new(),
            radiation_belts: worldline_data::radiation_belts(),
            flow_pressure_at_1au: worldline_data::mean_flow_pressure(),
            hierarchy,
        };
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
            sources.extend((0..moons.len()).map(|index| Source::Small { system, index }));
        }
        sources.extend((0..hierarchy.follower_count()).map(Source::Follower));
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
            })
            .collect();
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
                // The Moon is simulated at the top level, beside Earth.
                _ if body.name == "Moon" => index_of("Earth"),
                _ => None,
            })
            .collect();
        let known = |b: &Body| !self.added.contains(&b.name);
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
    pub fn add_body(&mut self, body: Body) -> usize {
        let name = body.name.clone();
        self.hierarchy.add_body(body);
        self.added.push(name.clone());
        self.reindex();
        self.refresh();
        self.refresh_small_moons();
        self.index_of(&name).expect("the new body is listed")
    }

    /// Whether body `index` was added in the sandbox.
    pub fn is_added(&self, index: usize) -> bool {
        self.added.contains(&self.bodies[index].name)
    }

    /// The place in the list of the body named `name`, if it is there.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.name == name)
    }

    /// Whether body `index` can be removed, and what goes with it.
    pub fn removal(&self, index: usize) -> Removal {
        match self.sources[index] {
            Source::Top(0) => Removal::Sun,
            Source::Top(_) | Source::Follower(_) => Removal::Allowed { moons: 0 },
            Source::Moon { system, body: 0 } => Removal::Allowed {
                moons: self.hierarchy.moon_systems[system].system.bodies.len() - 1
                    + self.small[system].len(),
            },
            Source::Moon { .. } | Source::Small { .. } => Removal::Moon,
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
            Source::Small { .. } => return,
        }
        self.added.retain(|n| n != &name);
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
        self.refresh();
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
                Source::Small { .. } => continue,
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
            // 28,000 Kepler's equations take about 3.4 ms on one core;
            // they are independent, so they are shared among all cores.
            let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
            for belt in &mut self.belts {
                let chunk = belt.positions.len().div_ceil(threads).max(1);
                std::thread::scope(|scope| {
                    for (positions, orbits) in belt
                        .positions
                        .chunks_mut(chunk)
                        .zip(belt.orbits.chunks(chunk))
                    {
                        scope.spawn(move || {
                            for (position, orbit) in positions.iter_mut().zip(orbits) {
                                *position = orbit.position_at(time);
                            }
                        });
                    }
                });
            }
            self.belts_time = time;
        }
        for (body, source) in self.bodies.iter_mut().zip(&self.sources) {
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
    fn belts_are_placed_where_their_orbits_say() {
        // Every body lies between its orbit's perihelion a(1 − e) and
        // aphelion a(1 + e) from the Sun, and a day later they have all
        // moved on.
        let mut sim = Simulation::solar_system(DAY);
        let total: usize = sim.belts.iter().map(|b| b.positions.len()).sum();
        assert_eq!(total, 28_331);
        for (cloud, data) in sim.belts.iter().zip(worldline_data::belts()) {
            for (position, orbit) in cloud.positions.iter().zip(&data.orbits) {
                let r = position.length();
                let (q, big_q) = (orbit.a * (1.0 - orbit.e), orbit.a * (1.0 + orbit.e));
                // Rounding: positions are good to about 10⁻¹⁵ of a.
                assert!(q * (1.0 - 1e-12) <= r && r <= big_q * (1.0 + 1e-12));
            }
        }
        let before = sim.belts[0].positions[0];
        sim.update(1.0, GENEROUS);
        assert_ne!(sim.belts[0].positions[0], before);
    }

    /// A Jupiter-mass planet 1.5 AU out, on a circular orbit.
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
        let new = sim.add_body(intruder(&sim));
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
        sim.add_body(intruder(&sim));
        sim.remove(sim.index_of("Mars").unwrap());
        sim.focus_detail_on(sim.index_of("Saturn").unwrap());
        sim.update(1.0, GENEROUS);
        let text = sim.save();
        // Every number loads back bit for bit.
        let loaded = Simulation::load(&text, DAY).unwrap();
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
    fn trails_cover_about_one_orbit() {
        // After more than a year, Earth's trail should reach back about one
        // orbit: its oldest point lies close to where Earth is now.
        let mut sim = Simulation::solar_system(worldline_core::constants::JULIAN_YEAR);
        sim.update(1.2, GENEROUS);
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
