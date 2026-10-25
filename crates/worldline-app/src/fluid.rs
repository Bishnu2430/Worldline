//! A body modeled as a self-gravitating fluid (step 4.2): an n = 1
//! polytrope of its mass and radius, as many SPH particles as this GPU
//! steps smoothly, stepped and drawn on the GPU (worldline-render's
//! `Fluid`), with the equations of worldline-core's `sph.rs`.
//!
//! The fluid keeps its own time, a step a frame: a planet's dynamical time
//! is tens of minutes, and the simulation's speeds run days to centuries a
//! second. Step 4.3 couples the two.

use eframe::egui_wgpu::RenderState;
use worldline_core::sph::{Particle, polytrope_particles};
use worldline_render::{Fluid, FluidClock, FluidParticle};

/// Frames a second taken as smooth playback.
pub const SMOOTH: f64 = 30.0;

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
/// slower than [`SMOOTH`]. That counts everything else a frame does too.
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
        self.done = rate < SMOOTH || self.trials.len() == TRIALS.len();
        true
    }

    /// The budget, once found: the largest fluid that ran smoothly (the
    /// smallest tried if none did).
    pub fn chosen(&self) -> Option<usize> {
        if !self.done {
            return None;
        }
        let smooth = self.trials.iter().rfind(|(_, rate)| *rate >= SMOOTH);
        smooth.or(self.trials.first()).map(|(count, _)| *count)
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
    /// When it stopped settling and went free, in dynamical times.
    pub freed_at: Option<f32>,
    /// The internal energy drawn mid-scale: the median particle's at the
    /// start.
    pub energy: f32,
}

impl FluidBody {
    /// Builds the body, of gravitational parameter `gm` and `radius`, as
    /// about `count` particles, and starts it settling.
    pub fn new(state: &RenderState, name: &str, gm: f64, radius: f64, count: usize) -> Self {
        let units = Units::new(gm, radius);
        let (particles, k) = polytrope_particles(units.mass, units.length, count);
        let particles: Vec<FluidParticle> = particles.iter().map(|p| units.particle(p)).collect();
        let mut energies: Vec<f32> = particles.iter().map(|p| p.energy).collect();
        energies.sort_by(f32::total_cmp);
        let energy = energies.get(energies.len() / 2).copied().unwrap_or(1.0);
        let mut fluid = Fluid::new(&state.device, &state.queue, &particles, 2.0);
        fluid.set_settling(&state.queue, Some((units.entropy(k), 1.0 / DAMPING_TIME)));
        Self {
            name: name.to_string(),
            units,
            fluid,
            clock: FluidClock { time: 0.0, dt: 0.0 },
            freed_at: None,
            energy,
        }
    }

    pub fn fluid(&self) -> &Fluid {
        &self.fluid
    }

    /// Whether it is still relaxing toward equilibrium.
    pub fn settling(&self) -> bool {
        self.freed_at.is_none()
    }

    /// One frame's step. The clock comes back a frame or so later, without
    /// waiting for the GPU; once it shows the fluid has settled, it is set
    /// at rest and let go.
    pub fn advance(&mut self, state: &RenderState) {
        let (device, queue) = (&state.device, &state.queue);
        if let Some(clock) = self.fluid.poll_clock(device) {
            self.clock = clock;
            if self.settling() && clock.time >= SETTLE {
                self.fluid.set_settling(queue, None);
                self.fluid.halt(device, queue);
                self.freed_at = Some(clock.time);
            }
        }
        self.fluid.step(device, queue, 1);
        self.fluid.request_clock(device, queue);
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
    fn the_budget_is_the_largest_fluid_that_ran_smoothly() {
        let mut budget = Budget::default();
        // 4,096 particles at 60 frames a second: smooth, on to 8,192.
        assert_eq!(budget.trying(), Some(4096));
        while !budget.frame(4096, 1.0 / 60.0) {}
        assert_eq!(budget.trials.len(), 1);
        assert!((budget.trials[0].1 - 60.0).abs() < 1e-9);
        // The warm-up's slow frames don't count.
        assert_eq!(budget.trying(), Some(8192));
        budget.frame(8192, 0.4);
        while !budget.frame(8192, 1.0 / 31.0) {}
        assert!((budget.trials[1].1 - 31.0).abs() < 1e-9);
        assert_eq!(budget.chosen(), None);
        // 16,384 at 20 frames a second: too slow, so the budget is 8,192.
        while !budget.frame(16384, 1.0 / 20.0) {}
        assert!(budget.done);
        assert_eq!(budget.trying(), None);
        assert_eq!(budget.chosen(), Some(8192));
        // None smooth: the smallest.
        budget.trials = vec![(4096, 12.0)];
        assert_eq!(budget.chosen(), Some(4096));
    }
}
