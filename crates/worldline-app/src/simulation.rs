//! The live simulation behind the window: the solar system, its physics,
//! the clock, and the orbit trails.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use worldline_core::constants::DAY;
use worldline_core::gravity::{EinsteinInfeldHoffmann, Gravity};
use worldline_core::integrator::{Ias15, Integrator};
use worldline_core::rotation::RotationModel;
use worldline_core::{DVec3, System};

/// The recent path of one body: about one orbit's worth.
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

/// The solar system running live.
pub struct Simulation {
    /// The bodies and the clock (seconds since `epoch_jd_tdb`).
    pub system: System,
    /// The starting moment, as a Julian Date (TDB).
    pub epoch_jd_tdb: f64,
    /// Simulated seconds per real second.
    pub speed: f64,
    /// Whether time is stopped.
    pub paused: bool,
    /// One trail per body, in the same order as the bodies.
    pub trails: Vec<Trail>,
    /// Fraction of the requested speed the last update achieved: 1.0 when
    /// physics kept up, less when it ran out of time.
    pub achieved: f64,
    /// IAU rotation model of each body, where one exists.
    rotations: Vec<Option<RotationModel>>,
    gravity: EinsteinInfeldHoffmann,
    integrator: Ias15,
}

impl Simulation {
    /// The real solar system on 2025-01-01, with relativistic gravity.
    pub fn solar_system(speed: f64) -> Self {
        let snapshot = worldline_data::solar_system();
        let system = snapshot.system();
        let mut simulation = Self {
            trails: vec![Trail::default(); system.bodies.len()],
            system,
            epoch_jd_tdb: snapshot.epoch_jd_tdb,
            speed,
            paused: false,
            achieved: 1.0,
            rotations: system_rotations(&snapshot.bodies),
            gravity: EinsteinInfeldHoffmann,
            integrator: Ias15::new(),
        };
        simulation.record_trails();
        simulation
    }

    /// How body `index` is oriented and spins, if known.
    pub fn rotation(&self, index: usize) -> Option<&RotationModel> {
        self.rotations.get(index)?.as_ref()
    }

    /// The gravity model in use.
    pub fn gravity(&self) -> &dyn Gravity {
        &self.gravity
    }

    /// The integrator in use.
    pub fn integrator_name(&self) -> &'static str {
        self.integrator.name()
    }

    /// The current moment, as a Julian Date (TDB).
    pub fn julian_date(&self) -> f64 {
        self.epoch_jd_tdb + self.system.time() / DAY
    }

    /// Advances the simulation by `seconds` of simulated time, however
    /// long that takes to compute.
    pub fn advance_by(&mut self, seconds: f64) {
        let target = self.system.time() + seconds;
        while self.system.time() < target {
            let remaining = target - self.system.time();
            let taken = self
                .integrator
                .step(&mut self.system, &self.gravity, remaining);
            if taken >= remaining {
                self.system.set_time(target);
            }
            self.record_trails();
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
        let start = self.system.time();
        let target = start + requested;
        let clock = Instant::now();
        while self.system.time() < target && clock.elapsed() < budget {
            let remaining = target - self.system.time();
            let taken = self
                .integrator
                .step(&mut self.system, &self.gravity, remaining);
            if taken >= remaining {
                self.system.set_time(target);
            }
            self.record_trails();
        }
        self.achieved = (self.system.time() - start) / requested;
    }

    fn record_trails(&mut self) {
        for (trail, body) in self.trails.iter_mut().zip(&self.system.bodies) {
            trail.record(body.position);
        }
    }
}

fn system_rotations(bodies: &[worldline_core::Body]) -> Vec<Option<RotationModel>> {
    bodies
        .iter()
        .map(|b| worldline_data::rotation_model(&b.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const GENEROUS: Duration = Duration::from_secs(10);

    #[test]
    fn one_real_second_advances_by_the_speed() {
        let mut sim = Simulation::solar_system(DAY);
        sim.update(1.0, GENEROUS);
        assert_eq!(sim.system.time(), DAY);
        assert_eq!(sim.achieved, 1.0);
        assert_eq!(sim.julian_date(), sim.epoch_jd_tdb + 1.0);
    }

    #[test]
    fn paused_means_no_time_passes() {
        let mut sim = Simulation::solar_system(DAY);
        sim.paused = true;
        sim.update(1.0, GENEROUS);
        assert_eq!(sim.system.time(), 0.0);
    }

    #[test]
    fn running_out_of_budget_is_reported() {
        let mut sim = Simulation::solar_system(1000.0 * DAY);
        sim.update(1.0, Duration::ZERO);
        assert!(sim.achieved < 1.0);
    }

    #[test]
    fn trails_cover_about_one_orbit() {
        // After more than a year, Earth's trail should reach back about one
        // orbit: its oldest point lies close to where Earth is now.
        let mut sim = Simulation::solar_system(worldline_core::constants::JULIAN_YEAR);
        sim.update(1.2, GENEROUS);
        let earth = &sim.trails[3];
        let oldest = earth.points().next().unwrap();
        let now = sim.system.bodies[3].position;
        assert!((now - oldest).length() < 0.2 * now.length());
        assert!(
            earth.points().len() > 300,
            "trail too coarse to draw smoothly"
        );
    }
}
