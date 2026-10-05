//! The GPU-drawn part of the 3D view: textured, lit, rotating globes.

use std::collections::HashMap;

use eframe::egui::{Color32, TextureId};
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use glam::{Mat3, Vec3};
use worldline_render::{Image, Material, Renderer, SphereDraw, View};

use crate::textures;

/// One body to draw as a globe.
pub struct Globe {
    /// Name, used to find its surface maps.
    pub name: String,
    /// Color for bodies without a map.
    pub color: Color32,
    /// Center relative to the camera, in m.
    pub center: Vec3,
    /// Radius, in m.
    pub radius: f32,
    /// Body-fixed frame → world axes.
    pub orientation: Mat3,
    /// Unit vector toward the Sun.
    pub sun_direction: Vec3,
    /// The Sun shines by itself.
    pub emissive: bool,
    /// Night-side glow (Earth's city lights).
    pub night_glow: f32,
}

/// Owns the GPU renderer and the uploaded textures.
pub struct GpuGlobes {
    renderer: Renderer,
    materials: HashMap<String, Material>,
    /// The renderer's output, registered with egui so it can be painted.
    texture: Option<TextureId>,
}

impl GpuGlobes {
    pub fn new(state: &RenderState) -> Self {
        Self {
            renderer: Renderer::new(&state.device),
            materials: HashMap::new(),
            texture: None,
        }
    }

    /// Draws the globes and returns the texture holding the result.
    /// Surface maps are decoded and uploaded the first time a body needs
    /// one: detail follows focus.
    pub fn draw(&mut self, state: &RenderState, view: View, globes: &[Globe]) -> Option<TextureId> {
        for globe in globes {
            if !self.materials.contains_key(&globe.name) {
                let material = load_material(&self.renderer, state, globe);
                self.materials.insert(globe.name.clone(), material);
            }
        }
        let draws: Vec<SphereDraw<'_>> = globes
            .iter()
            .map(|g| SphereDraw {
                center: g.center,
                radius: g.radius,
                orientation: g.orientation,
                sun_direction: g.sun_direction,
                emissive: g.emissive,
                night_glow: g.night_glow,
                material: &self.materials[&g.name],
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

fn load_material(renderer: &Renderer, state: &RenderState, globe: &Globe) -> Material {
    let Some(maps) = textures::surface_maps(&globe.name) else {
        let c = globe.color;
        let pixel = [c.r(), c.g(), c.b(), 255];
        let image = Image {
            width: 1,
            height: 1,
            rgba: &pixel,
        };
        return renderer.create_material(&state.device, &state.queue, &image, None);
    };
    let (width, height, rgba) = textures::decode(maps.surface);
    let surface = Image {
        width,
        height,
        rgba: &rgba,
    };
    let night = maps.night.map(textures::decode);
    let night_image = night.as_ref().map(|(width, height, rgba)| Image {
        width: *width,
        height: *height,
        rgba,
    });
    renderer.create_material(&state.device, &state.queue, &surface, night_image.as_ref())
}
