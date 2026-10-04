//! Gravity models.
//!
//! A model turns the current state of every body into the acceleration of
//! every body. Later steps add relativistic models behind the same trait,
//! and the engine picks one per situation (see `docs/ARCHITECTURE.md`).

mod newtonian;

pub use newtonian::Newtonian;

use glam::DVec3;

use crate::Body;

/// A model that computes gravitational accelerations.
pub trait Gravity {
    /// Short name, shown in the app's physics-model indicator.
    fn name(&self) -> &'static str;

    /// Writes the acceleration of each body, in m/s², into `out`.
    ///
    /// `out` must be the same length as `bodies`.
    fn accelerations(&self, bodies: &[Body], out: &mut [DVec3]);
}
