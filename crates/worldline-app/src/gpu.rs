//! The GPU-drawn part of the 3D view: textured, lit, rotating globes with
//! their atmospheres and rings.

use std::collections::HashMap;

use eframe::egui::{Color32, TextureId};
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use glam::{Mat3, Vec3};
use worldline_render::{
    Atmosphere, Image, Material, MaterialImages, Renderer, Rings, SphereDraw, View,
};

use crate::textures;

/// One body to draw as a globe.
pub struct Globe {
    /// Name, used to find its surface maps.
    pub name: String,
    /// Color for bodies without a map.
    pub color: Color32,
    /// Center relative to the camera, in m.
    pub center: Vec3,
    /// Triaxial radii (equatorial, equatorial, polar), in m.
    pub radii: Vec3,
    /// Body-fixed frame → world axes.
    pub orientation: Mat3,
    /// Unit vector toward the Sun.
    pub sun_direction: Vec3,
    /// The Sun shines by itself.
    pub emissive: bool,
    /// Night-side glow (Earth's city lights).
    pub night_glow: f32,
    /// Rings, with their measured profile.
    pub rings: Option<Rings>,
    /// A clear atmosphere, if any.
    pub atmosphere: Option<Atmosphere>,
}

/// Owns the GPU renderer and the uploaded textures.
pub struct GpuGlobes {
    renderer: Renderer,
    /// Each body's textures, and whether they include clouds.
    materials: HashMap<String, (Material, bool)>,
    /// The renderer's output, registered with egui so it can be painted.
    texture: Option<TextureId>,
}

impl GpuGlobes {
    pub fn new(state: &RenderState) -> Self {
        Self {
            renderer: Renderer::new(&state.device, &state.queue),
            materials: HashMap::new(),
            texture: None,
        }
    }

    /// Draws the globes and returns the texture holding the result.
    /// Textures are decoded and uploaded the first time a body needs them:
    /// detail follows focus.
    pub fn draw(&mut self, state: &RenderState, view: View, globes: &[Globe]) -> Option<TextureId> {
        for globe in globes {
            if !self.materials.contains_key(&globe.name) {
                let material = load_material(&self.renderer, state, globe);
                self.materials.insert(globe.name.clone(), material);
            }
        }
        let draws: Vec<SphereDraw<'_>> = globes
            .iter()
            .map(|g| {
                let (material, clouds) = &self.materials[&g.name];
                SphereDraw {
                    center: g.center,
                    radii: g.radii,
                    orientation: g.orientation,
                    sun_direction: g.sun_direction,
                    emissive: g.emissive,
                    night_glow: g.night_glow,
                    clouds: *clouds,
                    rings: g.rings,
                    atmosphere: g.atmosphere,
                    material,
                }
            })
            .collect();

        let resized = self
            .renderer
            .render(&state.device, &state.queue, &view, &draws);
        let output = self.renderer.output()?;
        let mut egui_renderer = state.renderer.write();
        match self.texture {
            Some(id) if resized => egui_renderer.update_egui_texture_from_wgpu_texture(
                &state.device,
                output,
                wgpu::FilterMode::Linear,
                id,
            ),
            Some(_) => {}
            None => {
                self.texture = Some(egui_renderer.register_native_texture(
                    &state.device,
                    output,
                    wgpu::FilterMode::Linear,
                ));
            }
        }
        self.texture
    }
}

/// Decodes and uploads a body's maps (and ring profile), returning the
/// material and whether it has clouds.
fn load_material(renderer: &Renderer, state: &RenderState, globe: &Globe) -> (Material, bool) {
    let c = globe.color;
    let flat_color = [c.r(), c.g(), c.b(), 255];
    let maps = textures::surface_maps(&globe.name);
    let decode = |bytes: Option<&'static [u8]>| bytes.map(textures::decode);
    let surface = decode(maps.as_ref().map(|m| m.surface));
    let night = decode(maps.as_ref().and_then(|m| m.night));
    let clouds = decode(maps.as_ref().and_then(|m| m.clouds));
    fn image(decoded: &(u32, u32, Vec<u8>)) -> Image<'_> {
        Image {
            width: decoded.0,
            height: decoded.1,
            rgba: &decoded.2,
        }
    }
    let ring_transmission: Option<Vec<f32>> = globe.rings.map(|_| {
        worldline_data::saturn_rings()
            .transmission()
            .iter()
            .map(|&t| t as f32)
            .collect()
    });

    let images = MaterialImages {
        surface: surface.as_ref().map(image).unwrap_or(Image {
            width: 1,
            height: 1,
            rgba: &flat_color,
        }),
        night: night.as_ref().map(image),
        clouds: clouds.as_ref().map(image),
        ring_transmission: ring_transmission.as_deref(),
    };
    let has_clouds = images.clouds.is_some();
    (
        renderer.create_material(&state.device, &state.queue, &images),
        has_clouds,
    )
}
