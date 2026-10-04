use glam::DVec3;

use super::Integrator;
use crate::System;
use crate::gravity::Gravity;

/// The leapfrog method in kick–drift–kick form, also called velocity Verlet.
///
/// Each step gives the velocities half a kick from the current
/// accelerations, drifts the positions a full step, then gives the second
/// half kick from the new accelerations.
///
/// It is second-order accurate, time-reversible and symplectic: the energy
/// error oscillates but does not drift, even over millions of orbits. That
/// makes it a good fast integrator. It assumes accelerations depend only on
/// positions, so it is not used with velocity-dependent relativistic gravity.
/// Source: Verlet 1967. See `docs/physics/integrators.md`.
#[derive(Debug, Clone)]
pub struct Leapfrog {
    /// Fixed step size, in s.
    pub dt: f64,
    acc: Vec<DVec3>,
}

impl Leapfrog {
    /// Creates a leapfrog integrator with a fixed step of `dt` seconds.
    pub fn new(dt: f64) -> Self {
        assert!(
            dt > 0.0 && dt.is_finite(),
            "step size must be positive and finite"
        );
        Self {
            dt,
            acc: Vec::new(),
        }
    }
}

impl Integrator for Leapfrog {
    fn name(&self) -> &'static str {
        "Leapfrog"
    }

    fn step(&mut self, system: &mut System, gravity: &dyn Gravity, max_dt: f64) -> f64 {
        let h = self.dt.min(max_dt);
        let bodies = &mut system.bodies;
        self.acc.resize(bodies.len(), DVec3::ZERO);

        gravity.accelerations(bodies, &mut self.acc);
        for (b, a) in bodies.iter_mut().zip(&self.acc) {
            b.velocity += 0.5 * h * *a;
            b.position += h * b.velocity;
        }
        gravity.accelerations(bodies, &mut self.acc);
        for (b, a) in bodies.iter_mut().zip(&self.acc) {
            b.velocity += 0.5 * h * *a;
        }

        system.tick(h);
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;
    use crate::constants::{AU, DAY, GM_SUN};
    use crate::gravity::Newtonian;
    use crate::orbit::two_body_system;

    #[test]
    fn running_backwards_retraces_the_path() {
        // Time reversibility: integrate forward, reverse every velocity,
        // integrate the same number of steps, and you arrive back where you
        // started, up to rounding error.
        let mut system = two_body_system(
            Body::new("star", GM_SUN, 7e8),
            Body::new("planet", 4e14, 6.4e6),
            AU,
            0.6,
        );
        let start = system.clone();
        let mut leapfrog = Leapfrog::new(DAY);
        for _ in 0..5_000 {
            leapfrog.step(&mut system, &Newtonian, f64::INFINITY);
        }
        for b in &mut system.bodies {
            b.velocity = -b.velocity;
        }
        for _ in 0..5_000 {
            leapfrog.step(&mut system, &Newtonian, f64::INFINITY);
        }
        for (now, then) in system.bodies.iter().zip(&start.bodies) {
            assert!((now.position - then.position).length() < 1e-6 * AU);
            assert!((now.velocity + then.velocity).length() < 1e-6 * then.velocity.length());
        }
    }
}
