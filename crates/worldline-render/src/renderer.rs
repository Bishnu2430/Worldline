//! The renderer: draws bodies into an offscreen texture.

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};
use wgpu::util::DeviceExt;

use crate::{mesh, mipmap};

/// The output texture's format. sRGB, so lighting math happens in linear
/// light and the GPU encodes the result for display.
const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
/// 32-bit float depth, used reversed (1 near, 0 far) for even precision
/// across the huge range of distances in space.
const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
/// Ring profiles: transmission from 0 to 1, stored linearly.
const PROFILE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;
/// 4× multisampling smooths the edges of planets.
const SAMPLES: u32 = 4;
/// Most bodies drawn in one frame.
const MAX_BODIES: usize = 128;

/// An sRGB RGBA8 image, row by row from the top.
pub struct Image<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels: 4 bytes each (red, green, blue, alpha), sRGB-encoded.
    pub rgba: &'a [u8],
}

/// Everything that makes up a body's look, before upload.
pub struct MaterialImages<'a> {
    /// The sunlit surface (equirectangular map).
    pub surface: Image<'a>,
    /// What glows on the night side (Earth's city lights).
    pub night: Option<Image<'a>>,
    /// Clouds, white on black (Earth).
    pub clouds: Option<Image<'a>>,
    /// Rings: the fraction of light passing straight through, e^(−τ), in
    /// equal radial steps from the inner to the outer edge of the profile.
    pub ring_transmission: Option<&'a [f32]>,
}

/// A body's textures, uploaded to the GPU.
pub struct Material {
    bind_group: wgpu::BindGroup,
}

/// The camera, as the renderer needs it. The camera sits at the origin.
#[derive(Debug, Clone, Copy)]
pub struct View {
    /// Output size in physical pixels.
    pub size: [u32; 2],
    /// Vertical field of view, in radians.
    pub fov_y: f32,
    /// Unit vector the camera looks along, in world axes.
    pub forward: Vec3,
    /// Unit vector pointing up on screen, in world axes.
    pub up: Vec3,
    /// Distance to the near clipping plane, in m. Keep it below the
    /// distance to the nearest surface.
    pub near: f32,
}

/// Where a body's ring profile sits.
#[derive(Debug, Clone, Copy)]
pub struct Rings {
    /// Radius of the profile's inner edge, in m.
    pub inner: f32,
    /// Radius of the profile's outer edge, in m.
    pub outer: f32,
}

/// A clear atmosphere that scatters sunlight (Rayleigh scattering).
#[derive(Debug, Clone, Copy)]
pub struct Atmosphere {
    /// Height of its top above the surface, in m.
    pub height: f32,
    /// Height over which its density falls by a factor e, in m.
    pub scale_height: f32,
    /// Scattering coefficients at the surface for red, green and blue, per m.
    pub rayleigh: Vec3,
}

/// One body to draw.
pub struct SphereDraw<'a> {
    /// Center relative to the camera, in m.
    pub center: Vec3,
    /// Triaxial radii in the body's frame (equatorial, equatorial, polar), in m.
    pub radii: Vec3,
    /// Rotation from the body's own frame (z toward the north pole, x toward
    /// the prime meridian) to world axes.
    pub orientation: Mat3,
    /// Unit vector from the body toward the Sun, in world axes.
    pub sun_direction: Vec3,
    /// Shines by itself (the Sun) instead of being lit.
    pub emissive: bool,
    /// Brightness of the night-side texture (Earth's city lights); 0 for none.
    pub night_glow: f32,
    /// Whether the material has a cloud map to draw.
    pub clouds: bool,
    /// Rings, if the material has a ring profile.
    pub rings: Option<Rings>,
    /// Atmosphere, if any.
    pub atmosphere: Option<Atmosphere>,
    /// The body's textures.
    pub material: &'a Material,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
}

/// Matches `Instance` in bodies.wgsl, including WGSL's padding rules: each
/// column of a mat3x3 and each vec3 starts on a 16-byte boundary.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceUniform {
    orientation: [[f32; 4]; 3],
    center: [f32; 3],
    emissive: f32,
    radii: [f32; 3],
    night_glow: f32,
    sun_direction: [f32; 3],
    clouds: f32,
    ring_inner: f32,
    ring_outer: f32,
    atmosphere_height: f32,
    scale_height: f32,
    rayleigh: [f32; 3],
    _padding: f32,
}

/// One drawing layer: its pipeline, its mesh, and which bodies it draws.
type Layer<'r> = (
    &'r wgpu::RenderPipeline,
    &'r Mesh,
    fn(&SphereDraw<'_>) -> bool,
);

struct Targets {
    size: [u32; 2],
    multisampled: wgpu::TextureView,
    depth: wgpu::TextureView,
    output: wgpu::TextureView,
}

struct Mesh {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
}

impl Mesh {
    fn new(device: &wgpu::Device, label: &str, vertices: &[[f32; 3]], indices: &[u32]) -> Self {
        Self {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            count: indices.len() as u32,
        }
    }
}

/// Draws textured, lit, rotating bodies, their atmospheres and their rings
/// into an offscreen texture.
pub struct Renderer {
    globe_pipeline: wgpu::RenderPipeline,
    atmosphere_pipeline: wgpu::RenderPipeline,
    ring_pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_group: wgpu::BindGroup,
    instance_stride: u64,
    material_layout: wgpu::BindGroupLayout,
    surface_sampler: wgpu::Sampler,
    ring_sampler: wgpu::Sampler,
    black: wgpu::TextureView,
    no_rings: wgpu::TextureView,
    sphere: Mesh,
    quad: Mesh,
    targets: Option<Targets>,
}

impl Renderer {
    /// Builds the pipelines, the meshes and the shared GPU buffers.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bodies shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bodies.wgsl").into()),
        });

        let uniform_entry = |dynamic: bool, size: usize| wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: NonZeroU64::new(size as u64),
            },
            count: None,
        };
        let globals_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("globals layout"),
            entries: &[uniform_entry(false, size_of::<Globals>())],
        });
        let instance_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("instance layout"),
            entries: &[uniform_entry(true, size_of::<InstanceUniform>())],
        });
        let texture_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let sampler_entry = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        };
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material layout"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                texture_entry(2),
                sampler_entry(3),
                texture_entry(4),
                sampler_entry(5),
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bodies pipeline layout"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&instance_layout),
                Some(&material_layout),
            ],
            immediate_size: 0,
        });

        // Globes are opaque and write depth. Atmospheres and rings are
        // translucent: they blend over what's behind them (premultiplied),
        // are hidden by nearer surfaces, but don't hide anything themselves.
        let pipeline = |label, vertex, fragment, translucent: bool, cull| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: size_of::<[f32; 3]>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    })],
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: cull,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH_FORMAT,
                    depth_write_enabled: Some(!translucent),
                    depth_compare: Some(wgpu::CompareFunction::Greater),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: SAMPLES,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: translucent
                            .then_some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let globe_pipeline = pipeline(
            "globes",
            "vs_globe",
            "fs_globe",
            false,
            Some(wgpu::Face::Back),
        );
        let atmosphere_pipeline = pipeline(
            "atmospheres",
            "vs_atmosphere",
            "fs_atmosphere",
            true,
            Some(wgpu::Face::Back),
        );
        let ring_pipeline = pipeline("rings", "vs_ring", "fs_ring", true, None);

        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("globals"),
            layout: &globals_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });

        // Each body's uniforms sit at a stride the GPU can bind at.
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let instance_stride = (size_of::<InstanceUniform>() as u64).div_ceil(alignment) * alignment;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("body instances"),
            size: instance_stride * MAX_BODIES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("body instances"),
            layout: &instance_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &instances,
                    offset: 0,
                    size: NonZeroU64::new(size_of::<InstanceUniform>() as u64),
                }),
            }],
        });

        let sampler = |label, wrap_u| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wrap_u,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_min_clamp: 0.0,
                lod_max_clamp: 32.0,
                compare: None,
                anisotropy_clamp: 8,
                border_color: None,
            })
        };
        // Longitude wraps around; radius and latitude stop at their edges.
        let surface_sampler = sampler("surface sampler", wgpu::AddressMode::Repeat);
        let ring_sampler = sampler("ring sampler", wgpu::AddressMode::ClampToEdge);

        let (sphere_vertices, sphere_indices) = mesh::uv_sphere(96, 48);
        let (quad_vertices, quad_indices) = mesh::quad();

        Self {
            globe_pipeline,
            atmosphere_pipeline,
            ring_pipeline,
            globals,
            globals_group,
            instances,
            instance_group,
            instance_stride,
            material_layout,
            surface_sampler,
            ring_sampler,
            black: single_texel(device, queue, COLOR_FORMAT, &[0, 0, 0, 255]),
            // No rings: everything passes straight through.
            no_rings: single_texel(device, queue, PROFILE_FORMAT, &[255]),
            sphere: Mesh::new(device, "sphere", &sphere_vertices, &sphere_indices),
            quad: Mesh::new(device, "ring quad", &quad_vertices, &quad_indices),
            targets: None,
        }
    }

    /// Uploads a body's textures, each with a full mipmap chain.
    pub fn create_material(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        images: &MaterialImages<'_>,
    ) -> Material {
        let surface = upload_image(device, queue, &images.surface);
        let night = images
            .night
            .as_ref()
            .map(|i| upload_image(device, queue, i));
        let clouds = images
            .clouds
            .as_ref()
            .map(|i| upload_image(device, queue, i));
        let rings = images
            .ring_transmission
            .map(|p| upload_profile(device, queue, p));
        fn view(v: &wgpu::TextureView) -> wgpu::BindingResource<'_> {
            wgpu::BindingResource::TextureView(v)
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view(&surface),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: view(night.as_ref().unwrap_or(&self.black)),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: view(clouds.as_ref().unwrap_or(&self.black)),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.surface_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: view(rings.as_ref().unwrap_or(&self.no_rings)),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.ring_sampler),
                },
            ],
        });
        Material { bind_group }
    }

    /// Draws `bodies` into the output texture, resizing it to `view.size`
    /// if needed: globes first, then atmospheres and rings over them.
    /// Returns `true` when the output texture was recreated, so anything
    /// displaying it must pick up the new one.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &View,
        bodies: &[SphereDraw<'_>],
    ) -> bool {
        let size = [view.size[0].max(1), view.size[1].max(1)];
        let resized = self.targets.as_ref().is_none_or(|t| t.size != size);
        if resized {
            self.targets = Some(create_targets(device, size));
        }
        let targets = self.targets.as_ref().expect("targets were just created");

        let globals = Globals {
            view_proj: view_projection(view).to_cols_array_2d(),
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));

        let bodies = &bodies[..bodies.len().min(MAX_BODIES)];
        let mut instance_bytes = vec![0u8; self.instance_stride as usize * bodies.len()];
        for (i, body) in bodies.iter().enumerate() {
            let uniform = instance_uniform(body);
            let start = i * self.instance_stride as usize;
            instance_bytes[start..start + size_of::<InstanceUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        if !instance_bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, &instance_bytes);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("bodies"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bodies"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.multisampled,
                    depth_slice: None,
                    resolve_target: Some(&targets.output),
                    ops: wgpu::Operations {
                        // Transparent, so the window's own background and
                        // trails show around the planets.
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.globals_group, &[]);
            let layers: [Layer<'_>; 3] = [
                (&self.globe_pipeline, &self.sphere, |_| true),
                (&self.atmosphere_pipeline, &self.sphere, |b| {
                    b.atmosphere.is_some()
                }),
                (&self.ring_pipeline, &self.quad, |b| b.rings.is_some()),
            ];
            for (pipeline, mesh, wanted) in layers {
                pass.set_pipeline(pipeline);
                pass.set_vertex_buffer(0, mesh.vertices.slice(..));
                pass.set_index_buffer(mesh.indices.slice(..), wgpu::IndexFormat::Uint32);
                for (i, body) in bodies.iter().enumerate().filter(|(_, b)| wanted(b)) {
                    let offset = (i as u64 * self.instance_stride) as u32;
                    pass.set_bind_group(1, &self.instance_group, &[offset]);
                    pass.set_bind_group(2, &body.material.bind_group, &[]);
                    pass.draw_indexed(0..mesh.count, 0, 0..1);
                }
            }
        }
        queue.submit([encoder.finish()]);
        resized
    }

    /// The finished image, premultiplied by coverage: transparent where
    /// nothing was drawn.
    pub fn output(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.output)
    }
}

fn instance_uniform(body: &SphereDraw<'_>) -> InstanceUniform {
    let m = body.orientation;
    let rings = body.rings.unwrap_or(Rings {
        inner: 0.0,
        outer: 0.0,
    });
    let air = body.atmosphere.unwrap_or(Atmosphere {
        height: 0.0,
        scale_height: 1.0,
        rayleigh: Vec3::ZERO,
    });
    InstanceUniform {
        orientation: [m.x_axis, m.y_axis, m.z_axis].map(|c| [c.x, c.y, c.z, 0.0]),
        center: body.center.to_array(),
        emissive: if body.emissive { 1.0 } else { 0.0 },
        radii: body.radii.to_array(),
        night_glow: body.night_glow,
        sun_direction: body.sun_direction.to_array(),
        clouds: if body.clouds { 1.0 } else { 0.0 },
        ring_inner: rings.inner,
        ring_outer: rings.outer,
        atmosphere_height: air.height,
        scale_height: air.scale_height,
        rayleigh: air.rayleigh.to_array(),
        _padding: 0.0,
    }
}

/// Camera-relative world → clip space, with an infinitely distant far plane
/// and reversed depth.
fn view_projection(view: &View) -> Mat4 {
    let aspect = view.size[0].max(1) as f32 / view.size[1].max(1) as f32;
    // WebGPU's convention: depth runs 0 to 1 and screen y points up.
    let projection = glam::camera::rh::proj::directx::perspective_infinite_reverse(
        view.fov_y, aspect, view.near,
    );
    projection * glam::camera::rh::view::look_to_mat4(Vec3::ZERO, view.forward, view.up)
}

fn create_targets(device: &wgpu::Device, size: [u32; 2]) -> Targets {
    let texture = |label, format, samples, usage| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: samples,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default())
    };
    Targets {
        size,
        multisampled: texture(
            "multisampled color",
            COLOR_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
        depth: texture(
            "depth",
            DEPTH_FORMAT,
            SAMPLES,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        ),
        output: texture(
            "bodies output",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        ),
    }
}

/// Creates a texture from mip levels (largest first), each given as
/// (width, height, bytes).
fn upload_levels(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    label: &str,
    format: wgpu::TextureFormat,
    bytes_per_texel: u32,
    levels: &[(u32, u32, &[u8])],
) -> wgpu::TextureView {
    let (width, height, _) = levels[0];
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (mip_level, &(width, height, bytes)) in levels.iter().enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_texel * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// Uploads an sRGB image with its full mipmap chain.
fn upload_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    image: &Image<'_>,
) -> wgpu::TextureView {
    let smaller = mipmap::chain(image.width, image.height, image.rgba);
    let levels: Vec<(u32, u32, &[u8])> = std::iter::once((image.width, image.height, image.rgba))
        .chain(
            smaller
                .iter()
                .map(|l| (l.width, l.height, l.rgba.as_slice())),
        )
        .collect();
    upload_levels(device, queue, "surface map", COLOR_FORMAT, 4, &levels)
}

/// Uploads a ring profile as a one-pixel-tall texture with its mipmaps.
/// From far away the mipmaps average the rings into simple bands; up close
/// the full measured structure shows: detail follows focus.
fn upload_profile(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    profile: &[f32],
) -> wgpu::TextureView {
    let bytes: Vec<Vec<u8>> = mipmap::profile_chain(profile)
        .iter()
        .map(|level| {
            level
                .iter()
                .map(|&t| (t.clamp(0.0, 1.0) * 255.0).round() as u8)
                .collect()
        })
        .collect();
    let levels: Vec<(u32, u32, &[u8])> = bytes
        .iter()
        .map(|l| (l.len() as u32, 1, l.as_slice()))
        .collect();
    upload_levels(device, queue, "ring profile", PROFILE_FORMAT, 1, &levels)
}

/// A 1×1 texture holding one texel.
fn single_texel(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    format: wgpu::TextureFormat,
    texel: &[u8],
) -> wgpu::TextureView {
    upload_levels(
        device,
        queue,
        "single texel",
        format,
        texel.len() as u32,
        &[(1, 1, texel)],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles_and_validates() {
        // Uses the same shader compiler wgpu uses, so shader mistakes are
        // caught even on machines without a GPU (like CI).
        let module =
            naga::front::wgsl::parse_str(include_str!("bodies.wgsl")).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates");
    }

    #[test]
    fn uniform_layouts_match_the_shader() {
        // WGSL pads each mat3x3 column and each vec3 to 16 bytes.
        assert_eq!(size_of::<InstanceUniform>(), 128);
        assert_eq!(size_of::<Globals>(), 64);
    }

    #[test]
    fn depth_is_reversed_and_reaches_infinity() {
        let view = View {
            size: [800, 600],
            fov_y: 0.8,
            forward: -Vec3::Z,
            up: Vec3::Y,
            near: 10.0,
        };
        let m = view_projection(&view);
        let depth = |distance: f32| {
            let clip = m * Vec3::new(0.0, 0.0, -distance).extend(1.0);
            clip.z / clip.w
        };
        assert!((depth(10.0) - 1.0).abs() < 1e-6, "near plane at depth 1");
        assert!((depth(20.0) - 0.5).abs() < 1e-6);
        assert!(
            depth(1e12) > 0.0 && depth(1e12) < 1e-10,
            "far away approaches 0"
        );
    }
}
