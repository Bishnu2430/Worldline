//! The GPU-drawn part of the 3D view: textured, lit, rotating globes with
//! their atmospheres and rings, the sky, and the galaxy's glow.

use std::collections::HashMap;

use eframe::egui::{Color32, TextureId};
use eframe::egui_wgpu::RenderState;
use eframe::wgpu;
use glam::{Mat3, Vec3};
use worldline_render::{
    Atmosphere, FluidDraw, Glow, GlowStar, GlowView, Image, Material, MaterialImages, Pit,
    Renderer, Rings, Sky, SkyView, SphereDraw, View,
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
    /// For a star with a surface map, the color of a blackbody at its
    /// temperature: the map is recolored to it, its pattern kept.
    pub tint: Option<Color32>,
    /// Night-side glow (Earth's city lights).
    pub night_glow: f32,
    /// Rings, with their measured profile.
    pub rings: Option<Rings>,
    /// A clear atmosphere, if any.
    pub atmosphere: Option<Atmosphere>,
    /// Craters on its surface.
    pub pits: Vec<Pit>,
}

/// Owns the GPU renderer and the uploaded textures.
pub struct GpuGlobes {
    renderer: Renderer,
    /// Each body's textures, and whether they include clouds.
    materials: HashMap<String, (Material, bool)>,
    /// The renderer's output, registered with egui so it can be painted.
    texture: Option<TextureId>,
    /// The widest a surface map is uploaded, pixels (see
    /// `Quality::map_width`).
    map_width: u32,
}

impl GpuGlobes {
    pub fn new(state: &RenderState, map_width: u32) -> Self {
        Self {
            renderer: Renderer::new(&state.device, &state.queue),
            materials: HashMap::new(),
            texture: None,
            map_width,
        }
    }

    /// Sets the widest a surface map is uploaded. The maps already
    /// uploaded at another width are let go, to be uploaded again as
    /// they're next needed.
    pub fn set_map_width(&mut self, width: u32) {
        if width != self.map_width {
            self.map_width = width;
            self.materials.clear();
        }
    }

    /// Draws the globes and the fluids' particles and returns the texture
    /// holding the result. Textures are decoded and uploaded the first time
    /// a body needs them: detail follows focus.
    pub fn draw(
        &mut self,
        state: &RenderState,
        view: View,
        globes: &[Globe],
        fluids: &[FluidDraw<'_>],
    ) -> Option<TextureId> {
        for globe in globes {
            if !self.materials.contains_key(&globe.name) {
                let material = load_material(&self.renderer, state, globe, self.map_width);
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
                    pits: &g.pits,
                    material,
                }
            })
            .collect();

        let resized = self
            .renderer
            .render(&state.device, &state.queue, &view, &draws, fluids);
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

/// Owns the galaxy's glow pass and its output's place in egui.
pub struct GpuGlow {
    glow: Glow,
    texture: Option<TextureId>,
}

impl GpuGlow {
    /// Makes the pass.
    pub fn new(state: &RenderState) -> Self {
        Self {
            glow: Glow::new(&state.device),
            texture: None,
        }
    }

    /// Draws `stars` as a glow and returns the texture holding it.
    pub fn draw(
        &mut self,
        state: &RenderState,
        view: &GlowView,
        stars: &[GlowStar],
    ) -> Option<TextureId> {
        let resized = self.glow.render(&state.device, &state.queue, view, stars);
        let output = self.glow.output()?;
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

/// Owns the sky's GPU pass and its output's place in egui.
pub struct GpuSky {
    sky: Sky,
    texture: Option<TextureId>,
}

impl GpuSky {
    /// Builds the sky map (Gaia's Milky Way and the bright stars) and
    /// uploads it.
    pub fn new(state: &RenderState) -> Self {
        let (width, height, rgba) = crate::sky::map();
        let image = Image {
            width,
            height,
            rgba: &rgba,
        };
        Self {
            sky: Sky::new(&state.device, &state.queue, &image),
            texture: None,
        }
    }

    /// Draws the sky and returns the texture holding it.
    pub fn draw(&mut self, state: &RenderState, view: &SkyView<'_>) -> Option<TextureId> {
        let resized = self.sky.render(&state.device, &state.queue, view);
        let output = self.sky.output()?;
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
/// A star's surface map recolored to `tint`, the color of a blackbody at
/// its temperature: each pixel keeps its brightness against the map's
/// mean (the granulation, the sunspots), in the tint's hue. A visual
/// choice: the maps are artists' renderings, their hue not measured.
fn recolor((_, _, rgba): &mut (u32, u32, Vec<u8>), tint: Color32) {
    let brightness = |p: &[u8; 4]| {
        0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])
    };
    let pixels = (rgba.len() / 4).max(1) as f32;
    let mean = rgba.as_chunks::<4>().0.iter().map(brightness).sum::<f32>() / pixels;
    if mean <= 0.0 {
        return;
    }
    let tint = [
        f32::from(tint.r()),
        f32::from(tint.g()),
        f32::from(tint.b()),
    ];
    for pixel in rgba.as_chunks_mut::<4>().0 {
        let k = brightness(pixel) / mean;
        for (channel, t) in pixel.iter_mut().zip(tint) {
            *channel = (t * k).round().clamp(0.0, 255.0) as u8;
        }
    }
}

fn load_material(
    renderer: &Renderer,
    state: &RenderState,
    globe: &Globe,
    map_width: u32,
) -> (Material, bool) {
    let c = globe.color;
    let flat_color = [c.r(), c.g(), c.b(), 255];
    let maps = textures::surface_maps(&globe.name);
    let decode = |bytes: Option<&'static [u8]>| {
        bytes.map(|b| crate::quality::fit_map(textures::decode(b), map_width))
    };
    let mut surface = decode(maps.as_ref().map(|m| m.surface));
    if let (Some(map), Some(tint)) = (&mut surface, globe.tint) {
        recolor(map, tint);
    }
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
