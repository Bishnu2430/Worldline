//! Worldline's GPU renderer.
//!
//! Draws textured, lit, rotating bodies with wgpu into an offscreen texture
//! that the app shows inside its window. Positions arrive already relative
//! to the camera, computed in double precision by the caller, so the GPU's
//! single precision never limits accuracy. This crate knows nothing about
//! physics or the window.

mod mesh;
mod mipmap;
mod renderer;

pub use renderer::{Image, Material, Renderer, SphereDraw, View};
