//! Gravity models.
//!
//! A model turns the current state of every body into the acceleration of
//! every body. The engine will pick a model per situation
//! (see `docs/ARCHITECTURE.md`).

mod eih;
mod newtonian;

pub use eih::EinsteinInfeldHoffmann;
pub use newtonian::Newtonian;

use glam::DVec3;

use crate::Body;

/// A model that computes gravitational accelerations.
pub trait Gravity {
    /// Short name, shown in the app's physics-model indicator.
    fn name(&self) -> &'static str;

    /// Whether accelerations depend on velocities as well as positions.
    /// Relativistic models do, and some integrators can't handle that.
    fn velocity_dependent(&self) -> bool {
        false
    }

    /// Writes the acceleration of each body, in m/s², into `out`.
    ///
    /// `out` must be the same length as `bodies`.
    fn accelerations(&self, bodies: &[Body], out: &mut [DVec3]);
}
