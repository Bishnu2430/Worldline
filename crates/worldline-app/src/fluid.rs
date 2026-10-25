//! A body modeled as a self-gravitating fluid: an n = 1 polytrope of its
//! mass and radius, as many SPH particles as this GPU steps smoothly,
//! stepped and drawn on the GPU (worldline-render's `Fluid`), with the
//! equations of worldline-core's `sph.rs` (step 4.2). It feels the tides
//! of the bodies around it, and the simulation's time follows its own, a
//! step a frame (step 4.3).

use std::time::Instant;

use eframe::egui_wgpu::RenderState;
use worldline_core::DVec3;
use worldline_core::sph::{Particle, polytrope_particles};
use worldline_core::tides::{fallback_power, return_time};
use worldline_render::{Fluid, FluidClock, FluidParticle, TidalBody};

/// Frames a second taken as smooth playback.
pub const SMOOTH: f64 = 30.0;

/// The frame rate a trial must keep. The budget's trials run while time
/// waits for the fluid, without the physics; once time runs, the physics
/// may take its budget of each frame, 12 ms. So a trial's frames must
/// leave that of a 30th of a second: 21.3 ms, 47 frames a second.
pub fn trial_rate() -> f64 {
    1.0 / (1.0 / SMOOTH - crate::app::PHYSICS_BUDGET.as_secs_f64())
}

/// The particle counts tried for the budget, smallest first; the largest
/// is what the grid's hash table was sized for.
const TRIALS: [usize; 6] = [4096, 8192, 16384, 32768, 65536, 131072];

/// A trial's frames: the first ones are let go by (the fluid built, the
/// GPU catching up), then the rest timed.
const TRIAL_WARM_UP: f64 = 0.5;
const TRIAL_TIMED: f64 = 1.5;

/// Settling: dynamical times to relax for, and the damping's time constant
/// in them, as in `sph.rs`.
const SETTLE: f32 = 5.0;
const DAMPING_TIME: f32 = 0.3;

/// The fluid's units: the body's radius, its mass, and its dynamical time
/// √(R³/GM), in which G = 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Units {
    /// m.
    pub length: f64,
    /// kg.
    pub mass: f64,
    /// s.
    pub time: f64,
}

impl Units {
    /// For a body of gravitational parameter `gm` (m³/s²) and `radius` (m).
    pub fn new(gm: f64, radius: f64) -> Self {
        Self {
            length: radius,
            mass: gm / worldline_core::constants::G,
            time: (radius.powi(3) / gm).sqrt(),
        }
    }

    /// A particle in these units.
    pub fn particle(&self, p: &Particle) -> FluidParticle {
        let (l, m, t) = (self.length, self.mass, self.time);
        FluidParticle {
            position: (p.position / l).as_vec3(),
            velocity: (p.velocity * t / l).as_vec3(),
            mass: (p.mass / m) as f32,
            energy: (p.energy * (t / l).powi(2)) as f32,
            h: (p.h / l) as f32,
            density: (p.density * l.powi(3) / m) as f32,
            viscosity: p.viscosity as f32,
            acceleration: glam::Vec3::ZERO,
            heating: 0.0,
        }
    }

    /// The polytrope's K (P = Kρ², in SI) in these units: P/ρ² scales as
    /// L⁵/(M T²).
    pub fn entropy(&self, k: f64) -> f32 {
        (k * self.mass * self.time * self.time / self.length.powi(5)) as f32
    }
}

/// How many particles this GPU steps smoothly: found by running the app
/// with ever larger fluids, a step a frame and drawn, until frames come
/// slower than [`trial_rate`]. That counts everything else a frame does
/// too.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Budget {
    /// Particles and frames a second, for each fluid tried so far.
    pub trials: Vec<(usize, f64)>,
    /// Whether the trials are over.
    pub done: bool,
    /// The trial under way: seconds since it began, and the frames and
    /// seconds timed.
    clock: (f64, u32, f64),
}

impl Budget {
    /// The particle count of the trial under way.
    pub fn trying(&self) -> Option<usize> {
        (!self.done).then(|| TRIALS[self.trials.len()])
    }

    /// Starts the trial under way over, its fluid just built.
    pub fn restart(&mut self) {
        self.clock = (0.0, 0, 0.0);
    }

    /// Counts a frame of `seconds` toward the trial under way, of a fluid
    /// of `count` particles. Returns `true` when that trial is over (and
    /// the next, if any, begins).
    pub fn frame(&mut self, count: usize, seconds: f64) -> bool {
        if self.done {
            return false;
        }
        let (since, frames, timed) = &mut self.clock;
        *since += seconds;
        if *since > TRIAL_WARM_UP {
            *frames += 1;
            *timed += seconds;
        }
        if *timed < TRIAL_TIMED {
            return false;
        }
        let rate = f64::from(*frames) / *timed;
        self.trials.push((count, rate));
        self.clock = (0.0, 0, 0.0);
        self.done = rate < trial_rate() || self.trials.len() == TRIALS.len();
        true
    }

    /// The budget, once found: the largest fluid that ran smoothly (the
    /// smallest tried if none did).
    pub fn chosen(&self) -> Option<usize> {
        if !self.done {
            return None;
        }
        let smooth = self.trials.iter().rfind(|(_, rate)| *rate >= trial_rate());
        smooth.or(self.trials.first()).map(|(count, _)| *count)
    }
}

/// The fluid every body is made from: an n = 1 polytrope of this GPU's
/// budget of particles, settled once in its own units (G = R = M = 1), in
/// which every n = 1 polytrope is the same. Readied the first time a body
/// needs it: the budget's trials first, then the settling, each drawn as
/// that body (the trials time drawing too).
#[derive(Default)]
pub struct Lab {
    pub budget: Budget,
    stage: Stage,
}

#[derive(Default)]
enum Stage {
    /// Not needed yet.
    #[default]
    Idle,
    /// Asked for, nothing built yet.
    Asked,
    /// Finding the budget: the fluid on trial, its particle count and
    /// median internal energy.
    Trial(usize, Fluid, f32),
    /// Settling the budget's fluid, and its median internal energy.
    Settling(Fluid, f32),
    /// Settled, read back: its particles, and the median internal energy.
    Ready(Vec<FluidParticle>, f32),
}

/// Steps a frame while settling: two, as the frame rate while it settles
/// is not the fluid's to keep.
const SETTLING_STEPS: u32 = 2;

impl Lab {
    /// Starts readying the fluid, if it isn't already.
    pub fn start(&mut self) {
        if matches!(self.stage, Stage::Idle) {
            self.stage = Stage::Asked;
        }
    }

    /// Whether it has been asked for and isn't ready yet.
    pub fn busy(&self) -> bool {
        matches!(
            self.stage,
            Stage::Asked | Stage::Trial(..) | Stage::Settling(..)
        )
    }

    /// The settled fluid, once ready: its particles and median energy.
    pub fn template(&self) -> Option<(&[FluidParticle], f32)> {
        match &self.stage {
            Stage::Ready(particles, energy) => Some((particles, *energy)),
            _ => None,
        }
    }

    /// The fluid being tried or settled, and its median energy, to draw
    /// as the body waiting for it.
    pub fn showing(&self) -> Option<(&Fluid, f32)> {
        match &self.stage {
            Stage::Trial(_, fluid, energy) | Stage::Settling(fluid, energy) => {
                Some((fluid, *energy))
            }
            _ => None,
        }
    }

    /// What it is doing, in words, while busy.
    pub fn status(&self) -> Option<String> {
        match &self.stage {
            Stage::Asked | Stage::Trial(..) => self.budget.trying().map(|count| {
                format!("finding this GPU's particle budget: trying {count} particles")
            }),
            Stage::Settling(..) => Some("settling the fluid".to_string()),
            _ => None,
        }
    }

    /// The part of a frame that took `seconds` spent readying it, if
    /// `allowed` (the trials' frame rates mean nothing while the app is
    /// starting or the camera flying).
    pub fn advance(&mut self, state: &RenderState, seconds: f64, allowed: bool) {
        let (device, queue) = (&state.device, &state.queue);
        if matches!(self.stage, Stage::Asked | Stage::Trial(..)) {
            let Some(trying) = self.budget.trying() else {
                let count = self.budget.chosen().unwrap_or(TRIALS[0]);
                let (fluid, energy) = polytrope(state, count);
                self.stage = Stage::Settling(fluid, energy);
                return;
            };
            if !allowed {
                return;
            }
            match &mut self.stage {
                Stage::Trial(count, fluid, _) if *count == trying => {
                    self.budget.frame(trying, seconds);
                    fluid.step(device, queue, 1);
                }
                _ => {
                    let (fluid, energy) = polytrope(state, trying);
                    self.stage = Stage::Trial(trying, fluid, energy);
                    self.budget.restart();
                }
            }
            return;
        }
        if let Stage::Settling(fluid, _) = &mut self.stage {
            if let Some(clock) = fluid.poll_clock(device)
                && clock.time >= SETTLE
            {
                fluid.set_settling(queue, None);
                fluid.halt(device, queue);
                let (particles, _) = fluid.read(device, queue);
                let mut energies: Vec<f32> = particles.iter().map(|p| p.energy).collect();
                energies.sort_by(f32::total_cmp);
                let energy = energies[energies.len() / 2];
                self.stage = Stage::Ready(particles, energy);
                return;
            }
            fluid.step(device, queue, SETTLING_STEPS);
            fluid.request_clock(device, queue);
        }
    }
}

/// An n = 1 polytrope of about `count` particles in its own units,
/// settling, and its median internal energy.
fn polytrope(state: &RenderState, count: usize) -> (Fluid, f32) {
    let units = Units::new(1.0, 1.0);
    let (particles, k) = polytrope_particles(units.mass, units.length, count);
    let particles: Vec<FluidParticle> = particles.iter().map(|p| units.particle(p)).collect();
    let mut energies: Vec<f32> = particles.iter().map(|p| p.energy).collect();
    energies.sort_by(f32::total_cmp);
    let mut fluid = Fluid::new(&state.device, &state.queue, &particles, 2.0);
    fluid.set_settling(&state.queue, Some((units.entropy(k), 1.0 / DAMPING_TIME)));
    (fluid, energies[energies.len() / 2])
}

/// How far the fluid may run ahead of the simulation, in its steps: as
/// far as its clock comes back late, a frame or two.
const LEAD: f32 = 3.0;

/// How often its debris is worked out again, s of real time: reading its
/// particles back makes the GPU finish its work first.
const DEBRIS_EVERY: f64 = 2.0;

/// From how many times the first piece's return on the fallback's power is
/// estimated: by then the law's own correction for how dM/dE curves is
/// down to about 2% (see `docs/physics/disruption.md`).
pub const LATE: f64 = 30.0;

/// A body's debris (step 4.4): each piece's orbit around what tore it, as
/// if alone with it from now on.
#[derive(Debug, Clone, PartialEq)]
pub struct Debris {
    /// What tore it, by name.
    pub by: String,
    /// The share of its mass bound to that body.
    pub bound: f64,
    /// When each bound piece comes back, s after the closest approach, in
    /// order: its orbit's period.
    pub times: Vec<f64>,
    /// The share of its mass each piece carries.
    pub share: f64,
    /// Whether it is set: worked out far enough out that the pieces'
    /// orbits no longer change.
    pub set: bool,
}

impl Debris {
    /// The rate it comes back at, in shares of its mass a second, in bins
    /// a quarter of a decade wide from the first piece's return: each
    /// bin's middle (geometric) and rate.
    pub fn rates(&self) -> Vec<(f64, f64)> {
        let (Some(&first), Some(&last)) = (self.times.first(), self.times.last()) else {
            return Vec::new();
        };
        let width = 10f64.powf(0.25);
        let mut rates = Vec::new();
        let mut edge = first;
        let mut k = 0;
        while edge <= last {
            let next = edge * width;
            let start = k;
            while k < self.times.len() && self.times[k] < next {
                k += 1;
            }
            if k > start {
                let rate = (k - start) as f64 * self.share / (next - edge);
                rates.push(((edge * next).sqrt(), rate));
            }
            edge = next;
        }
        rates
    }

    /// The power α of t^(−α) the rate falls as, from [`LATE`] times the
    /// first piece's return on, and its standard error (see
    /// `tides::fallback_power`): if at least 30 pieces come back then.
    pub fn power(&self) -> Option<(f64, f64)> {
        let from = LATE * self.times.first()?;
        let later = self.times.iter().filter(|&&t| t > from).count();
        (later >= 30)
            .then(|| fallback_power(&self.times, from))
            .flatten()
    }
}

/// A body modeled as a fluid.
pub struct FluidBody {
    /// The body it models.
    pub name: String,
    pub units: Units,
    fluid: Fluid,
    /// The fluid's clock, as last read back (in dynamical times).
    pub clock: FluidClock,
    /// The simulation's time when it became a fluid, s.
    pub since: f64,
    /// Made a fluid by hand: it stays one until made a point again.
    pub by_hand: bool,
    /// The internal energy drawn mid-scale.
    pub energy: f32,
    /// Its closest approach so far to what pulls it hardest: that body's
    /// name, and the simulation's time (s) and distance (m) then.
    pub closest: Option<(String, f64, f64)>,
    /// Its debris, once it has torn and passed its closest approach, and
    /// when that was last worked out.
    pub debris: Option<Debris>,
    measured: Option<Instant>,
}

impl FluidBody {
    /// The body, of gravitational parameter `gm` and `radius`, as the
    /// settled `template` (scaled to it by its units), from the
    /// simulation's time `since`.
    pub fn new(
        state: &RenderState,
        name: &str,
        (gm, radius): (f64, f64),
        (template, energy): (&[FluidParticle], f32),
        since: f64,
        by_hand: bool,
    ) -> Self {
        Self {
            name: name.to_string(),
            units: Units::new(gm, radius),
            fluid: Fluid::new(&state.device, &state.queue, template, 2.0),
            clock: FluidClock {
                time: 0.0,
                dt: 0.0,
                outside: 0,
            },
            since,
            by_hand,
            energy,
            closest: None,
            debris: None,
            measured: None,
        }
    }

    /// Notes that it is `distance` (m) from `by`, what pulls it hardest, at
    /// the simulation's time `now`. Returns whether it has passed its
    /// closest approach to it.
    pub fn track(&mut self, by: &str, now: f64, distance: f64) -> bool {
        match &mut self.closest {
            Some((name, when, nearest)) if name == by => {
                if distance < *nearest {
                    (*when, *nearest) = (now, distance);
                }
                distance > *nearest
            }
            _ => {
                self.closest = Some((by.to_string(), now, distance));
                false
            }
        }
    }

    /// Works out its debris again, if it hasn't for a while (or if it is
    /// `set` now): each piece's orbit around `by` (its name and
    /// gravitational parameter), it being at `relative` and moving at
    /// `motion` relative to that body (m, m/s). Reads its particles back,
    /// waiting for the GPU.
    pub fn measure_debris(
        &mut self,
        state: &RenderState,
        (by, gm): (&str, f64),
        relative: DVec3,
        motion: DVec3,
        set: bool,
    ) {
        if !set
            && self
                .measured
                .is_some_and(|t| t.elapsed().as_secs_f64() < DEBRIS_EVERY)
        {
            return;
        }
        self.measured = Some(Instant::now());
        let (particles, _) = self.fluid.read(&state.device, &state.queue);
        let (l, t) = (self.units.length, self.units.time);
        let mut times: Vec<f64> = particles
            .iter()
            .filter_map(|p| {
                let r = relative + p.position.as_dvec3() * l;
                let v = motion + p.velocity.as_dvec3() * (l / t);
                return_time(gm, 0.5 * v.length_squared() - gm / r.length())
            })
            .collect();
        times.sort_by(f64::total_cmp);
        let share = 1.0 / particles.len().max(1) as f64;
        self.debris = Some(Debris {
            by: by.to_string(),
            bound: times.len() as f64 * share,
            times,
            share,
            set,
        });
    }

    pub fn fluid(&self) -> &Fluid {
        &self.fluid
    }

    /// How far the simulation may go: the fluid's time as last read back.
    pub fn reached(&self) -> f64 {
        self.since + f64::from(self.clock.time) * self.units.time
    }

    /// One frame: the tides of `bodies` (relative to it, in its units, at
    /// the simulation's time `now`), particles counted beyond `outside`
    /// (in its radii), and a step if `step` and the simulation has kept up.
    /// The clock comes back a frame or so later, without waiting for the
    /// GPU.
    pub fn advance(
        &mut self,
        state: &RenderState,
        now: f64,
        bodies: &[TidalBody],
        outside: f32,
        step: bool,
    ) {
        let (device, queue) = (&state.device, &state.queue);
        if let Some(clock) = self.fluid.poll_clock(device) {
            self.clock = clock;
        }
        let then = ((now - self.since) / self.units.time) as f32;
        self.fluid.set_tides(queue, then, bodies, outside);
        if step && self.clock.time - then <= LEAD * self.clock.dt {
            self.fluid.step(device, queue, 1);
            self.fluid.request_clock(device, queue);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_its_own_units_a_polytrope_has_g_one_and_k_two_over_pi() {
        // K = 2GR²/π, so with G = R = M = 1 it is 2/π, whatever the body.
        let units = Units::new(3.986_004_418e14, 6.371e6);
        let (particles, k) = polytrope_particles(units.mass, units.length, 500);
        assert!((f64::from(units.entropy(k)) - 2.0 / std::f64::consts::PI).abs() < 1e-6);
        let total: f32 = particles.iter().map(|p| units.particle(p).mass).sum();
        assert!((total - 1.0).abs() < 1e-5, "total mass {total}");
        // G M T²/L³ = 1.
        let g =
            worldline_core::constants::G * units.mass * units.time.powi(2) / units.length.powi(3);
        assert!((g - 1.0).abs() < 1e-12);
    }

    #[test]
    fn debris_coming_back_as_t_to_the_minus_five_thirds_shows_so() {
        // Pieces of equal mass back at the quantiles of a rate ∝ t^(−5/3)
        // from 1 s on: the rates fall by 10^(5/3·¼) a bin, and the power
        // is 5/3.
        let n = 4000;
        let times: Vec<f64> = (0..n)
            .map(|k| (1.0 - (k as f64 + 0.5) / n as f64).powf(-1.5))
            .collect();
        let debris = Debris {
            by: "Hole".into(),
            bound: 0.5,
            times,
            share: 0.5 / n as f64,
            set: true,
        };
        let rates = debris.rates();
        let slope = (rates[6].1 / rates[2].1).log10() / (rates[6].0 / rates[2].0).log10();
        assert!((slope + 5.0 / 3.0).abs() < 0.02, "{slope}");
        let (power, _) = debris.power().unwrap();
        assert!((power - 5.0 / 3.0).abs() < 0.01, "{power}");
    }

    #[test]
    fn the_budget_is_the_largest_fluid_that_ran_smoothly() {
        // Trials must leave the physics its 12 ms of a 30th of a second.
        assert!((trial_rate() - 46.875).abs() < 1e-9, "{}", trial_rate());
        let mut budget = Budget::default();
        // 4,096 particles at 60 frames a second: smooth, on to 8,192.
        assert_eq!(budget.trying(), Some(4096));
        while !budget.frame(4096, 1.0 / 60.0) {}
        assert_eq!(budget.trials.len(), 1);
        assert!((budget.trials[0].1 - 60.0).abs() < 1e-9);
        // The warm-up's slow frames don't count.
        assert_eq!(budget.trying(), Some(8192));
        budget.frame(8192, 0.4);
        while !budget.frame(8192, 1.0 / 50.0) {}
        assert!((budget.trials[1].1 - 50.0).abs() < 1e-9);
        assert_eq!(budget.chosen(), None);
        // 16,384 at 43 frames a second: too slow, so the budget is 8,192.
        while !budget.frame(16384, 1.0 / 43.0) {}
        assert!(budget.done);
        assert_eq!(budget.trying(), None);
        assert_eq!(budget.chosen(), Some(8192));
        // None smooth: the smallest.
        budget.trials = vec![(4096, 12.0)];
        assert_eq!(budget.chosen(), Some(4096));
    }
}
