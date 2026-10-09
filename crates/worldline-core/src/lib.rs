//! Worldline's physics engine.
//!
//! This crate holds all of Worldline's physics and never depends on graphics,
//! so every physical claim it makes can be checked by headless tests.
//! All quantities are `f64` in SI units.

pub mod body;
pub mod collision;
pub mod compact;
pub mod constants;
pub mod diagnostics;
pub mod gravitational_waves;
pub mod gravity;
pub mod hierarchy;
pub mod integrator;
pub mod kepler;
pub mod magnetosphere;
pub mod mean_elements;
pub mod merger;
pub mod neutron_star;
pub mod orbit;
pub mod regime;
pub mod rotation;
pub mod solar_wind;
pub mod sunlight;
pub mod swarm;
pub mod system;
pub mod white_dwarf;
pub mod zodiacal;

pub use body::Body;
pub use glam::{DMat3, DVec3};
pub use system::System;
