//! Worldline's GPU renderer.
//!
//! Draws textured, lit, rotating bodies with wgpu into an offscreen texture
//! that the app shows inside its window, and the sky behind them, bent by a
//! lens it is handed a table for. Positions arrive already relative
//! to the camera, computed in double precision by the caller, so the GPU's
//! single precision never limits accuracy. This crate knows nothing about
//! physics or the window.

mod mesh;
mod mipmap;
mod renderer;
mod sky;

pub use renderer::{
    Atmosphere, Image, Material, MaterialImages, Renderer, Rings, SphereDraw, View,
};
pub use sky::{Lens, Sky, SkyView};
