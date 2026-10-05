//! The live simulation behind the window: the solar system, its physics,
//! the clock, and the orbit trails.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use worldline_core::constants::DAY;
use worldline_core::gravity::Gravity;
use worldline_core::hierarchy::{Hierarchy, MOON_SYSTEM_GRAVITY};
use worldline_core::integrator::{Ias15, Integrator};
use worldline_core::rotation::RotationModel;
use worldline_core::{Body, DVec3};

/// The recent path of one body around the Sun: about one orbit's worth.
/// Moons have none; they are shown with their current orbit (see
/// `view.rs`).
#[derive(Debug, Clone, Default)]
pub struct Trail {
    points: VecDeque<DVec3>,
    /// Total length of the path through `points`, in m.
    length: f64,
}

impl Trail {
    /// A new point is recorded once the body has moved this fraction of its
    /// distance from the barycenter: about 0.5° of arc, or more if the
    /// physics steps are coarser than that.
    const SPACING: f64 = 0.008_7;
    /// Upper limit on stored points, as a memory guard.
    const CAPACITY: usize = 4096;

    fn record(&mut self, position: DVec3) {
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

/// Where a body in the flat list lives in the hierarchy.
#[derive(Debug, Clone, Copy)]
enum Source {
    /// A top-level body.
    Top(usize),
    /// Body `body` of moon system `system` (body 0 is the planet).
    Moon { system: usize, body: usize },
}

/// The solar system running live.
pub struct Simulation {
    /// Every body: the Sun and planets (each planet with moons in place of
    /// its system's barycenter) in the snapshot's order, then the moons.
    /// Positions are refreshed after every step.
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
    hierarchy: Hierarchy,
}

impl Simulation {
    /// The real solar system on 2025-01-01, with its major moons:
    /// relativistic gravity between the Sun and planets, and each planet
    /// with moons simulated in its own frame.
    pub fn solar_system(speed: f64) -> Self {
        let epoch_jd_tdb = worldline_data::solar_system().epoch_jd_tdb;
        let hierarchy = worldline_data::solar_system_with_moons();

        let mut sources: Vec<Source> = (0..hierarchy.top.bodies.len()).map(Source::Top).collect();
        for (system, moons) in hierarchy.moon_systems.iter().enumerate() {
            sources[moons.host] = Source::Moon { system, body: 0 };
        }
        for (system, moons) in hierarchy.moon_systems.iter().enumerate() {
            sources
                .extend((1..moons.system.bodies.len()).map(|body| Source::Moon { system, body }));
        }
        let bodies: Vec<Body> = sources
            .iter()
            .map(|source| match *source {
                Source::Top(k) => hierarchy.top.bodies[k].clone(),
                Source::Moon { system, body } => {
                    hierarchy.moon_systems[system].system.bodies[body].clone()
                }
            })
            .collect();
        let index_of = |name: &str| bodies.iter().position(|b| b.name == name);
        let parents = sources
            .iter()
            .zip(&bodies)
            .map(|(source, body)| match *source {
                Source::Moon { system, body } if body > 0 => {
                    let host = hierarchy.moon_systems[system].host;
                    Some(host)
                }
                // The Moon is simulated at the top level, beside Earth.
                _ if body.name == "Moon" => index_of("Earth"),
                _ => None,
            })
            .collect();

        let mut simulation = Self {
            trails: vec![Trail::default(); bodies.len()],
            rotations: bodies
                .iter()
                .map(|b| worldline_data::rotation_model(&b.name))
                .collect(),
            bodies,
            epoch_jd_tdb,
            speed,
            paused: false,
            achieved: 1.0,
            parents,
            sources,
            hierarchy,
        };
        simulation.refresh();
        simulation
    }

    /// Simulated time since the start, in s.
    pub fn time(&self) -> f64 {
        self.hierarchy.time()
    }

    /// The body that body `index` closely orbits (a moon's planet), if any.
    pub fn parent(&self, index: usize) -> Option<usize> {
        self.parents[index]
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
            };
            body.position = position;
            body.velocity = velocity;
        }
        for ((trail, body), parent) in self.trails.iter_mut().zip(&self.bodies).zip(&self.parents) {
            if parent.is_none() {
                trail.record(body.position);
            }
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
