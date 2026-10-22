//! The sky behind everything: a full-screen pass that shows an
//! equirectangular map of the sky along each pixel's ray, bent first by one
//! lens if there is one (see `sky.wgsl`).

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Vec3};

use crate::renderer::{COLOR_FORMAT, Image, upload_image};

/// One lens bending the sky: which way it is, how big its shadow, and how
/// much it bends each ray.
pub struct Lens<'a> {
    /// Unit vector from the camera toward it, in world axes.
    pub toward: Vec3,
    /// The angle from `toward` within which rays see black, in rad.
    pub edge: f32,
    /// How much more than a straight line each ray sweeps around it, in
    /// rad, sampled evenly in x = ln((ψ − edge)/(π − edge)) from `x_min`
    /// to 0, ψ the ray's angle from `toward`.
    pub bending: &'a [f32],
    /// The smallest x sampled.
    pub x_min: f32,
}

/// The camera, as the sky needs it.
pub struct SkyView<'a> {
    /// Output size in physical pixels.
    pub size: [u32; 2],
    /// Unit vector the camera looks along, in world axes.
    pub forward: Vec3,
    /// Unit vector pointing up on screen, in world axes.
    pub up: Vec3,
    /// Vertical field of view, in rad.
    pub fov_y: f32,
    /// Turns world directions into the map's axes (x toward its center,
    /// z toward its top).
    pub to_map: Mat3,
    /// How bright to show the map: 1 as stored.
    pub exposure: f32,
    /// The lens, if any.
    pub lens: Option<Lens<'a>>,
}

/// Matches `Sky` in sky.wgsl: each vec3 starts on a 16-byte boundary.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SkyUniform {
    right: [f32; 3],
    tan_half_fov: f32,
    up: [f32; 3],
    aspect: f32,
    forward: [f32; 3],
    lensing: f32,
    map_x: [f32; 3],
    edge: f32,
    map_y: [f32; 3],
    x_min: f32,
    map_z: [f32; 3],
    samples: f32,
    toward: [f32; 3],
    texel: f32,
    exposure: f32,
    _padding: [f32; 3],
}

/// Draws the sky into an offscreen texture.
pub struct Sky {
    pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    table: wgpu::Buffer,
    group: wgpu::BindGroup,
    /// The angle a texel of the map spans, in rad.
    texel: f32,
    output: Option<([u32; 2], wgpu::TextureView)>,
}

impl Sky {
    /// The most samples a lens's bending table may have.
    pub const MAX_SAMPLES: usize = 4096;

    /// Uploads the map (equirectangular, longitude 0 in the middle and
    /// increasing leftward, as sky maps are drawn) with its mipmaps, and
    /// builds the pass.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, map: &Image<'_>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sky.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sky layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<SkyUniform>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(4),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
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
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("sky"),
            size: size_of::<SkyUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let table = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lens bending"),
            size: (Self::MAX_SAMPLES * size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let map_view = upload_image(device, queue, map);
        // Longitude wraps around; latitude stops at the poles.
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            lod_min_clamp: 0.0,
            lod_max_clamp: 32.0,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("sky"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: table.as_entire_binding(),
                },
            ],
        });
        Self {
            pipeline,
            uniform,
            table,
            group,
            texel: std::f32::consts::TAU / map.width.max(1) as f32,
            output: None,
        }
    }

    /// Draws the sky for `view`, resizing the output if needed. Returns
    /// `true` when the output texture was recreated.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &SkyView<'_>,
    ) -> bool {
        let size = [view.size[0].max(1), view.size[1].max(1)];
        let resized = self.output.as_ref().is_none_or(|(s, _)| *s != size);
        if resized {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("sky output"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: COLOR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            self.output = Some((
                size,
                texture.create_view(&wgpu::TextureViewDescriptor::default()),
            ));
        }
        let (forward, up) = (view.forward.normalize(), view.up.normalize());
        let right = forward.cross(up).normalize();
        let rows = view.to_map.transpose();
        let mut uniform = SkyUniform {
            right: right.to_array(),
            tan_half_fov: (0.5 * view.fov_y).tan(),
            up: up.to_array(),
            aspect: size[0] as f32 / size[1] as f32,
            forward: forward.to_array(),
            lensing: 0.0,
            map_x: rows.x_axis.to_array(),
            edge: 0.0,
            map_y: rows.y_axis.to_array(),
            x_min: -1.0,
            map_z: rows.z_axis.to_array(),
            samples: 2.0,
            toward: Vec3::Z.to_array(),
            texel: self.texel,
            exposure: view.exposure,
            _padding: [0.0; 3],
        };
        if let Some(lens) = &view.lens
            && lens.bending.len() >= 2
        {
            let samples = lens.bending.len().min(Self::MAX_SAMPLES);
            queue.write_buffer(
                &self.table,
                0,
                bytemuck::cast_slice(&lens.bending[..samples]),
            );
            uniform.lensing = 1.0;
            uniform.edge = lens.edge;
            uniform.x_min = lens.x_min;
            uniform.samples = samples as f32;
            uniform.toward = lens.toward.normalize().to_array();
        }
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));

        let (_, output) = self.output.as_ref().expect("the output was just made");
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("sky") });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        resized
    }

    /// The finished sky.
    pub fn output(&self) -> Option<&wgpu::TextureView> {
        self.output.as_ref().map(|(_, view)| view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("sky.wgsl")).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates");
    }

    #[test]
    fn the_uniform_matches_the_shader() {
        // The shader's own layout of `Sky`, as naga lays it out for the GPU.
        let module = naga::front::wgsl::parse_str(include_str!("sky.wgsl")).expect("WGSL parses");
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("types lay out");
        let (handle, _) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Sky"))
            .expect("the Sky struct");
        assert_eq!(layouter[handle].size as usize, size_of::<SkyUniform>());
    }
}
