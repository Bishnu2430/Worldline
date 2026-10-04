//! Worldline's physics engine.
//!
//! This crate holds all of Worldline's physics and never depends on graphics,
//! so every physical claim it makes can be checked by headless tests.
//! All quantities are `f64` in SI units.

pub mod body;
pub mod constants;
pub mod diagnostics;
pub mod gravity;
pub mod integrator;
pub mod orbit;
pub mod system;

pub use body::Body;
pub use glam::DVec3;
pub use system::System;
