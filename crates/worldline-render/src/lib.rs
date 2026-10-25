//! Worldline's GPU renderer.
//!
//! Draws textured, lit, rotating bodies with wgpu into an offscreen texture
//! that the app shows inside its window, and the sky behind them, bent by a
//! lens it is handed a table for, or past a spinning black hole whose rays
//! it traces. Positions arrive already relative
//! to the camera, computed in double precision by the caller, so the GPU's
//! single precision never limits accuracy. This crate knows nothing about
//! physics or the window.

mod fluid;
mod mesh;
mod mipmap;
mod renderer;
mod sky;

pub use fluid::{Fluid, FluidClock, FluidParticle, MAX_TIDAL_BODIES, TidalBody};
pub use renderer::{
    Atmosphere, FluidDraw, Image, Material, MaterialImages, Renderer, Rings, SphereDraw, View,
};
pub use sky::{KerrDisk, KerrLens, Lens, Sky, SkyView, trace_rays};
