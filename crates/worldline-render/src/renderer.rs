//! The renderer: draws spheres into an offscreen texture.

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
/// 4× multisampling smooths the edges of planets.
const SAMPLES: u32 = 4;
/// Most spheres drawn in one frame.
const MAX_SPHERES: usize = 128;

/// An sRGB RGBA8 image, row by row from the top.
pub struct Image<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixels: 4 bytes each (red, green, blue, alpha), sRGB-encoded.
    pub rgba: &'a [u8],
}

/// A body's surface textures, uploaded to the GPU.
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

/// One body to draw.
pub struct SphereDraw<'a> {
    /// Center relative to the camera, in m.
    pub center: Vec3,
    /// Radius, in m.
    pub radius: f32,
    /// Rotation from the body's own frame (z toward the north pole, x toward
    /// the prime meridian) to world axes.
    pub orientation: Mat3,
    /// Unit vector from the body toward the Sun, in world axes.
    pub sun_direction: Vec3,
    /// Shines by itself (the Sun) instead of being lit.
    pub emissive: bool,
    /// Brightness of the night-side texture (Earth's city lights); 0 for none.
    pub night_glow: f32,
    /// The body's surface textures.
    pub material: &'a Material,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
}

/// Matches `Instance` in sphere.wgsl, including WGSL's padding rules: each
/// column of a mat3x3 and each vec3 takes 16 bytes.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct InstanceUniform {
    orientation: [[f32; 4]; 3],
    center: [f32; 3],
    radius: f32,
    sun_direction: [f32; 3],
    emissive: f32,
    night_glow: f32,
    _padding: [f32; 3],
}

struct Targets {
    size: [u32; 2],
    multisampled: wgpu::TextureView,
    depth: wgpu::TextureView,
    output: wgpu::TextureView,
}

/// Draws textured, lit, rotating spheres into an offscreen texture.
pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    globals: wgpu::Buffer,
    globals_group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_group: wgpu::BindGroup,
    instance_stride: u64,
    material_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    no_night: wgpu::TextureView,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    index_count: u32,
    targets: Option<Targets>,
}

impl Renderer {
    /// Builds the pipeline, the sphere mesh and the shared GPU buffers.
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sphere shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sphere.wgsl").into()),
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
        let material_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material layout"),
            entries: &[
                texture_entry(0),
                texture_entry(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sphere pipeline layout"),
            bind_group_layouts: &[
                Some(&globals_layout),
                Some(&instance_layout),
                Some(&material_layout),
            ],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sphere pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
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
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(true),
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
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

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

        // Each sphere's uniforms sit at a stride the GPU can bind at.
        let alignment = u64::from(device.limits().min_uniform_buffer_offset_alignment);
        let instance_stride = (size_of::<InstanceUniform>() as u64).div_ceil(alignment) * alignment;
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sphere instances"),
            size: instance_stride * MAX_SPHERES as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let instance_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sphere instances"),
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

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("surface sampler"),
            // Longitude wraps around; latitude stops at the poles.
            address_mode_u: wgpu::AddressMode::Repeat,
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
        });

        let (vertex_data, index_data) = mesh::uv_sphere(96, 48);
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere vertices"),
            contents: bytemuck::cast_slice(&vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("sphere indices"),
            contents: bytemuck::cast_slice(&index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            pipeline,
            globals,
            globals_group,
            instances,
            instance_group,
            instance_stride,
            material_layout,
            sampler,
            no_night: blank_texture(device),
            vertices,
            indices,
            index_count: index_data.len() as u32,
            targets: None,
        }
    }

    /// Uploads a body's surface map, and optionally a night-side map, with
    /// a full mipmap chain.
    pub fn create_material(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface: &Image<'_>,
        night: Option<&Image<'_>>,
    ) -> Material {
        let surface = upload(device, queue, surface);
        let night = night.map(|image| upload(device, queue, image));
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material"),
            layout: &self.material_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&surface),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        night.as_ref().unwrap_or(&self.no_night),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        Material { bind_group }
    }

    /// Draws `spheres` into the output texture, resizing it to `view.size`
    /// if needed. Returns `true` when the output texture was recreated, so
    /// anything displaying it must pick up the new one.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &View,
        spheres: &[SphereDraw<'_>],
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

        let spheres = &spheres[..spheres.len().min(MAX_SPHERES)];
        let mut instance_bytes = vec![0u8; self.instance_stride as usize * spheres.len()];
        for (i, sphere) in spheres.iter().enumerate() {
            let m = sphere.orientation;
            let uniform = InstanceUniform {
                orientation: [m.x_axis, m.y_axis, m.z_axis].map(|c| [c.x, c.y, c.z, 0.0]),
                center: sphere.center.to_array(),
                radius: sphere.radius,
                sun_direction: sphere.sun_direction.to_array(),
                emissive: if sphere.emissive { 1.0 } else { 0.0 },
                night_glow: sphere.night_glow,
                _padding: [0.0; 3],
            };
            let start = i * self.instance_stride as usize;
            instance_bytes[start..start + size_of::<InstanceUniform>()]
                .copy_from_slice(bytemuck::bytes_of(&uniform));
        }
        if !instance_bytes.is_empty() {
            queue.write_buffer(&self.instances, 0, &instance_bytes);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("spheres"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("spheres"),
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
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_bind_group(0, &self.globals_group, &[]);
            for (i, sphere) in spheres.iter().enumerate() {
                let offset = (i as u64 * self.instance_stride) as u32;
                pass.set_bind_group(1, &self.instance_group, &[offset]);
                pass.set_bind_group(2, &sphere.material.bind_group, &[]);
                pass.draw_indexed(0..self.index_count, 0, 0..1);
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
            "spheres output",
            COLOR_FORMAT,
            1,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        ),
    }
}

/// Uploads an image with its full mipmap chain.
fn upload(device: &wgpu::Device, queue: &wgpu::Queue, image: &Image<'_>) -> wgpu::TextureView {
    let levels = mipmap::chain(image.width, image.height, image.rgba);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("surface map"),
        size: wgpu::Extent3d {
            width: image.width,
            height: image.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1 + levels.len() as u32,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let full = std::iter::once((image.width, image.height, image.rgba));
    let smaller = levels
        .iter()
        .map(|l| (l.width, l.height, l.rgba.as_slice()));
    for (mip_level, (width, height, rgba)) in full.chain(smaller).enumerate() {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: mip_level as u32,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
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

/// A 1×1 black texture, for bodies without a night-side map.
fn blank_texture(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("blank"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: COLOR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles_and_validates() {
        // Uses the same shader compiler wgpu uses, so shader mistakes are
        // caught even on machines without a GPU (like CI).
        let module =
            naga::front::wgsl::parse_str(include_str!("sphere.wgsl")).expect("WGSL parses");
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
        assert_eq!(size_of::<InstanceUniform>(), 96);
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
