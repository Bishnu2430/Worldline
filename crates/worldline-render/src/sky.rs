//! The sky behind everything: a full-screen pass that shows an
//! equirectangular map of the sky along each pixel's ray, bent first by one
//! lens if there is one (see `sky.wgsl`). The lens is either looked up from
//! a table of bending angles, or a spinning black hole whose rays are
//! traced one by one, with a disk around it if asked.

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4, Vec3};

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

/// A spinning black hole whose rays are traced one by one, in its own
/// coordinates: units of GM/c², the hole at the origin spinning about +z,
/// with the world's axes (see `sky.wgsl`).
pub struct KerrLens<'a> {
    /// Where the camera is.
    pub camera: Vec3,
    /// Takes (1, look) to the energy and momentum (E, p) of the ray the
    /// camera sees along the world direction look.
    pub launch: Mat4,
    /// The hole's spin, 0 to 1.
    pub spin: f32,
    /// A ray moving inward inside this radius can't come back out.
    pub capture: f32,
    /// A ray moving outward past this radius has escaped.
    pub far: f32,
    /// A disk in the hole's equatorial plane, if any.
    pub disk: Option<KerrDisk<'a>>,
}

/// A thin disk around a [`KerrLens`].
pub struct KerrDisk<'a> {
    /// Its inner and outer radius.
    pub inner: f32,
    /// See `inner`.
    pub outer: f32,
    /// At radii evenly spaced in ln r from `inner` to `outer`: ln T (its
    /// temperature), its angular velocity Ω and u^t.
    pub rings: &'a [[f32; 3]],
    /// Linear RGB at ln T evenly spaced from `ln_t_min` by `ln_t_step`.
    pub colors: &'a [[f32; 3]],
    /// See `colors`.
    pub ln_t_min: f32,
    /// See `colors`.
    pub ln_t_step: f32,
    /// What the colors are multiplied by before being shown.
    pub exposure: f32,
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
    /// Or the spinning hole whose rays are traced, if any. It takes the
    /// place of `lens`.
    pub kerr: Option<KerrLens<'a>>,
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

/// Matches `Kerr` in sky.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct KerrUniform {
    launch: [f32; 16],
    camera: [f32; 3],
    spin: f32,
    capture: f32,
    far: f32,
    disk_inner: f32,
    disk_outer: f32,
    disk: f32,
    radii: f32,
    ln_t_min: f32,
    ln_t_step: f32,
    colors: f32,
    disk_exposure: f32,
    steps: f32,
    _padding: f32,
}

/// The most steps a traced ray takes: one still circling the hole after
/// them is drawn black.
const KERR_STEPS: f32 = 600.0;

impl KerrUniform {
    /// The uniform for `lens`, and its table: the disk's rings, then its
    /// colors, three floats each.
    fn new(lens: &KerrLens<'_>) -> (Self, Vec<f32>) {
        let mut uniform = Self {
            launch: lens.launch.to_cols_array(),
            camera: lens.camera.to_array(),
            spin: lens.spin,
            capture: lens.capture,
            far: lens.far,
            disk_inner: 1.0,
            disk_outer: 2.0,
            disk: 0.0,
            radii: 2.0,
            ln_t_min: 0.0,
            ln_t_step: 1.0,
            colors: 2.0,
            disk_exposure: 1.0,
            steps: KERR_STEPS,
            _padding: 0.0,
        };
        let mut table = vec![0.0; 12];
        if let Some(disk) = &lens.disk
            && disk.rings.len() >= 2
            && disk.colors.len() >= 2
        {
            let rings = &disk.rings[..disk.rings.len().min(Sky::MAX_RINGS)];
            let colors = &disk.colors[..disk.colors.len().min(Sky::MAX_COLORS)];
            uniform.disk_inner = disk.inner;
            uniform.disk_outer = disk.outer;
            uniform.disk = 1.0;
            uniform.radii = rings.len() as f32;
            uniform.ln_t_min = disk.ln_t_min;
            uniform.ln_t_step = disk.ln_t_step;
            uniform.colors = colors.len() as f32;
            uniform.disk_exposure = disk.exposure;
            table = rings.iter().chain(colors).flatten().copied().collect();
        }
        (uniform, table)
    }
}

/// Draws the sky into an offscreen texture.
pub struct Sky {
    pipeline: wgpu::RenderPipeline,
    kerr_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    table: wgpu::Buffer,
    kerr_uniform: wgpu::Buffer,
    kerr_table: wgpu::Buffer,
    group: wgpu::BindGroup,
    /// The angle a texel of the map spans, in rad.
    texel: f32,
    output: Option<([u32; 2], wgpu::TextureView)>,
    /// Everything the last frame was drawn from, so an unchanged view
    /// isn't drawn again: tracing rays one by one is costly.
    drawn: Vec<u8>,
}

impl Sky {
    /// The most samples a lens's bending table may have.
    pub const MAX_SAMPLES: usize = 4096;
    /// The most rings a [`KerrDisk`] may have.
    pub const MAX_RINGS: usize = 512;
    /// The most colors a [`KerrDisk`] may have.
    pub const MAX_COLORS: usize = 1024;

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
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<KerrUniform>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
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
        // The same pass, its rays bent by the table or traced.
        let pipeline = |entry_point: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry_point),
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
                    entry_point: Some(entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: COLOR_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let (kerr_pipeline, pipeline) = (pipeline("fs_kerr"), pipeline("fs_sky"));
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
        let kerr_uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kerr"),
            size: size_of::<KerrUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let kerr_table = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kerr disk"),
            size: (3 * (Self::MAX_RINGS + Self::MAX_COLORS) * size_of::<f32>()) as u64,
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: kerr_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: kerr_table.as_entire_binding(),
                },
            ],
        });
        Self {
            pipeline,
            kerr_pipeline,
            uniform,
            table,
            kerr_uniform,
            kerr_table,
            group,
            texel: std::f32::consts::TAU / map.width.max(1) as f32,
            output: None,
            drawn: Vec::new(),
        }
    }

    /// Draws the sky for `view`, resizing the output if needed, unless it
    /// is the view last drawn. Returns `true` when the output texture was
    /// recreated.
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
        let mut bending: &[f32] = &[];
        if let Some(lens) = &view.lens
            && lens.bending.len() >= 2
        {
            bending = &lens.bending[..lens.bending.len().min(Self::MAX_SAMPLES)];
            uniform.lensing = 1.0;
            uniform.edge = lens.edge;
            uniform.x_min = lens.x_min;
            uniform.samples = bending.len() as f32;
            uniform.toward = lens.toward.normalize().to_array();
        }
        let kerr = view.kerr.as_ref().map(KerrUniform::new);
        let mut inputs = bytemuck::bytes_of(&uniform).to_vec();
        inputs.extend_from_slice(bytemuck::cast_slice(bending));
        if let Some((uniform, table)) = &kerr {
            inputs.extend_from_slice(bytemuck::bytes_of(uniform));
            inputs.extend_from_slice(bytemuck::cast_slice(table));
        }
        if !resized && inputs == self.drawn {
            return false;
        }
        self.drawn = inputs;
        queue.write_buffer(&self.uniform, 0, bytemuck::bytes_of(&uniform));
        if !bending.is_empty() {
            queue.write_buffer(&self.table, 0, bytemuck::cast_slice(bending));
        }
        if let Some((uniform, table)) = &kerr {
            queue.write_buffer(&self.kerr_uniform, 0, bytemuck::bytes_of(uniform));
            queue.write_buffer(&self.kerr_table, 0, bytemuck::cast_slice(table));
        }

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
            pass.set_pipeline(if view.kerr.is_some() {
                &self.kerr_pipeline
            } else {
                &self.pipeline
            });
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

/// Traces the rays the camera sees along the world directions `looks`
/// past `lens`, on the GPU, as the sky pass does: where on the sky each
/// came from, or `None` if it was mostly stopped (by the hole or the disk). For
/// checking the shader against the rays worldline-core traces.
pub fn trace_rays(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    lens: &KerrLens<'_>,
    looks: &[Vec3],
) -> Vec<Option<Vec3>> {
    use wgpu::util::DeviceExt;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("sky shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("sky.wgsl").into()),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("trace rays"),
        layout: None,
        module: &shader,
        entry_point: Some("cs_trace"),
        compilation_options: wgpu::PipelineCompilationOptions::default(),
        cache: None,
    });
    let (uniform, table) = KerrUniform::new(lens);
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("kerr"),
        contents: bytemuck::bytes_of(&uniform),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let table = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("kerr disk"),
        contents: bytemuck::cast_slice(&table),
        usage: wgpu::BufferUsages::STORAGE,
    });
    let probes: Vec<[f32; 4]> = looks.iter().map(|l| [l.x, l.y, l.z, 0.0]).collect();
    let size = (probes.len() * size_of::<[f32; 4]>()) as u64;
    let probe_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("probes"),
        contents: bytemuck::cast_slice(&probes),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("probes readback"),
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("trace rays"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 4,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: table.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: probe_buffer.as_entire_binding(),
            },
        ],
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("trace rays"),
    });
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.dispatch_workgroups(looks.len().div_ceil(64) as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&probe_buffer, 0, &readback, 0, size);
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("the probes map");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let view = readback
        .get_mapped_range(..)
        .expect("the probes are mapped");
    let traced: Vec<[f32; 4]> = bytemuck::cast_slice(&view).to_vec();
    traced
        .iter()
        .map(|t| (t[3] < 0.5).then(|| Vec3::new(t[0], t[1], t[2])))
        .collect()
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
        let (handle, _) = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Kerr"))
            .expect("the Kerr struct");
        assert_eq!(layouter[handle].size as usize, size_of::<KerrUniform>());
    }
}
