//! A set of bodies evolving together.

use glam::DVec3;

use crate::Body;

/// A gravitating system: its bodies plus the current simulation time.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct System {
    /// The bodies. Integrators never reorder them.
    pub bodies: Vec<Body>,
    /// Simulation time, in s since the scenario started.
    time: f64,
    /// Rounding error carried by `time`. Without it, a clock advanced by
    /// millions of steps drifts away from the true elapsed time.
    time_compensation: f64,
}

impl System {
    /// Creates a system at time zero.
    pub fn new(bodies: Vec<Body>) -> Self {
        Self {
            bodies,
            time: 0.0,
            time_compensation: 0.0,
        }
    }

    /// Simulation time, in s since the scenario started.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Advances the clock by `dt` seconds, using compensated (Kahan)
    /// summation so that rounding errors don't accumulate.
    pub fn tick(&mut self, dt: f64) {
        let y = dt - self.time_compensation;
        let t = self.time + y;
        self.time_compensation = (t - self.time) - y;
        self.time = t;
    }

    /// Sets the clock to exactly `time` seconds.
    pub fn set_time(&mut self, time: f64) {
        self.time = time;
        self.time_compensation = 0.0;
    }

    /// Total gravitational parameter Σμ, in m³/s².
    pub fn total_gm(&self) -> f64 {
        self.bodies.iter().map(|b| b.gm).sum()
    }

    /// Position of the barycenter (center of mass), in m.
    pub fn barycenter(&self) -> DVec3 {
        self.weighted_mean(|b| b.position)
    }

    /// Velocity of the barycenter, in m/s.
    pub fn barycenter_velocity(&self) -> DVec3 {
        self.weighted_mean(|b| b.velocity)
    }

    /// Shifts every body so the barycenter sits at rest at the origin.
    ///
    /// Simulating in this frame keeps coordinates small, which preserves
    /// floating-point precision.
    pub fn move_to_barycentric_frame(&mut self) {
        let r = self.barycenter();
        let v = self.barycenter_velocity();
        for b in &mut self.bodies {
            b.position -= r;
            b.velocity -= v;
        }
    }

    fn weighted_mean(&self, value: impl Fn(&Body) -> DVec3) -> DVec3 {
        let total = self.total_gm();
        if total == 0.0 {
            return DVec3::ZERO;
        }
        self.bodies.iter().map(|b| b.gm * value(b)).sum::<DVec3>() / total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_does_not_drift_over_many_ticks() {
        // Adding 0.1 ten million times the naive way gives 999999.99983898,
        // an error of 1.6 × 10⁻⁴. The compensated clock is exact.
        let mut system = System::default();
        for _ in 0..10_000_000 {
            system.tick(0.1);
        }
        assert_eq!(system.time(), 1_000_000.0);
    }

    #[test]
    fn barycentric_frame_puts_center_of_mass_at_rest_at_origin() {
        let mut system = System::new(vec![
            Body::new("heavy", 3.0, 1.0)
                .at(DVec3::new(1.0, 2.0, 3.0))
                .moving(DVec3::new(0.5, 0.0, 0.0)),
            Body::new("light", 1.0, 1.0)
                .at(DVec3::new(-4.0, 0.0, 1.0))
                .moving(DVec3::new(0.0, -2.0, 0.0)),
        ]);
        system.move_to_barycentric_frame();
        assert!(system.barycenter().length() < 1e-15);
        assert!(system.barycenter_velocity().length() < 1e-15);
        // Relative geometry is unchanged.
        let d = system.bodies[1].position - system.bodies[0].position;
        assert_eq!(d, DVec3::new(-5.0, -2.0, -2.0));
    }
}
