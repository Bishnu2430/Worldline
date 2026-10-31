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
pub mod eclipses;
pub mod fields;
pub mod gravitational_waves;
pub mod gravity;
pub mod hawking;
pub mod hierarchy;
pub mod horizon;
pub mod impact;
pub mod integrator;
pub mod kepler;
pub mod kerr_light;
pub mod lensing;
pub mod magnetosphere;
pub mod mean_elements;
pub mod merger;
pub mod milky_way;
pub mod neutron_star;
pub mod orbit;
pub mod regime;
pub mod rotation;
pub mod shadow_on_earth;
pub mod solar_wind;
pub mod sph;
pub mod sunlight;
pub mod swarm;
pub mod system;
pub mod tides;
pub mod timescales;
pub mod white_dwarf;
pub mod zodiacal;

pub use body::Body;
pub use glam::{DMat3, DVec3};
pub use system::System;
