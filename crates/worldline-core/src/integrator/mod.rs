//! Time integrators: methods that advance a system through time.

mod ias15;
mod leapfrog;

pub use ias15::{Ias15, StepCriterion};
pub use leapfrog::Leapfrog;

use crate::System;
use crate::gravity::Gravity;

/// A method for advancing a [`System`] through time.
pub trait Integrator {
    /// Short name, shown in the app.
    fn name(&self) -> &'static str;

    /// Advances `system` by one step of at most `max_dt` seconds, including
    /// its clock. Returns the length of the step actually taken.
    fn step(&mut self, system: &mut System, gravity: &dyn Gravity, max_dt: f64) -> f64;
}

/// Advances `system` by exactly `duration` seconds, taking as many steps as
/// the integrator needs.
pub fn advance(
    system: &mut System,
    gravity: &dyn Gravity,
    integrator: &mut dyn Integrator,
    duration: f64,
) {
    assert!(
        duration >= 0.0 && duration.is_finite(),
        "duration must be finite and non-negative"
    );
    let end = system.time() + duration;
    while system.time() < end {
        let remaining = end - system.time();
        let taken = integrator.step(system, gravity, remaining);
        assert!(taken > 0.0, "{} took an empty step", integrator.name());
        if taken >= remaining {
            // Land exactly on `end` instead of leaving a rounding-sized sliver.
            system.set_time(end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Body;
    use crate::gravity::Newtonian;

    #[test]
    fn advance_lands_exactly_on_requested_time() {
        let mut system = System::new(vec![Body::new("lonely", 1.0, 1.0)]);
        let mut leapfrog = Leapfrog::new(0.3);
        advance(&mut system, &Newtonian, &mut leapfrog, 1.0);
        assert_eq!(system.time(), 1.0);
        advance(&mut system, &Newtonian, &mut leapfrog, 0.1);
        assert_eq!(system.time(), 1.1);
    }
}
