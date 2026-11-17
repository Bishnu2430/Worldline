//! A cloud of stars drawn as a glow (step 7.4): each star a Gaussian of
//! mass, each pixel the column of mass along its ray, added up on the GPU
//! and tone mapped to lie over the sky (see `glow.wgsl`). This crate knows
//! nothing of what the stars are: the caller hands it each one's place
//! relative to the camera, how far it spreads and its weight, in any one
//! unit of length.

use std::num::NonZeroU64;

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

use crate::renderer::COLOR_FORMAT;

/// A star is drawn out to this many standard deviations: beyond, it would
/// add under e⁻⁸ = 3 × 10⁻⁴ of its column at its center.
pub const GLOW_CUTOFF: f32 = 4.0;

/// The format the columns are added up in for showing: half floats, which
/// every GPU can blend.
const SUM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// One star to draw.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct GlowStar {
    /// Where it is relative to the camera.
    pub position: [f32; 3],
    /// How far it spreads: its Gaussian's standard deviation, in the same
    /// unit.
    pub spread: f32,
    /// Its color (linear RGB) times its weight. The column drawn is in units
    /// of weight per unit length squared.
    pub color: [f32; 3],
    /// Unused.
    pub padding: f32,
}

/// How the glow is seen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlowView {
    /// Output size in physical pixels.
    pub size: [u32; 2],
    /// Unit vector the camera looks along.
    pub forward: Vec3,
    /// Unit vector pointing up on screen.
    pub up: Vec3,
    /// Vertical field of view, in rad.
    pub fov_y: f32,
    /// What the column is multiplied by before it's shown: a column of
    /// 1/exposure is shown at half brightness.
    pub exposure: f32,
}

/// Matches `View` in glow.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ViewUniform {
    right: [f32; 3],
    tan_half_fov: f32,
    up: [f32; 3],
    aspect: f32,
    forward: [f32; 3],
    cutoff: f32,
    exposure: f32,
    frustum: f32,
    padding: [f32; 2],
}

impl ViewUniform {
    fn new(view: &GlowView) -> Self {
        let size = [view.size[0].max(1), view.size[1].max(1)];
        let (forward, up) = (view.forward.normalize(), view.up.normalize());
        let right = forward.cross(up).normalize();
        let tan_half_fov = (0.5 * view.fov_y).tan();
        let aspect = size[0] as f32 / size[1] as f32;
        Self {
            right: right.to_array(),
            tan_half_fov,
            up: up.to_array(),
            aspect,
            forward: forward.to_array(),
            cutoff: GLOW_CUTOFF,
            exposure: view.exposure,
            // To the screen's corners.
            frustum: (tan_half_fov * (1.0 + aspect * aspect).sqrt()).atan(),
            padding: [0.0; 2],
        }
    }
}

/// The pipelines and their layouts.
struct Pipelines {
    stars: wgpu::RenderPipeline,
    stars_layout: wgpu::BindGroupLayout,
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(size_of::<ViewUniform>() as u64),
        },
        count: None,
    }
}

fn shader(device: &wgpu::Device) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("glow shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("glow.wgsl").into()),
    })
}

/// The pipeline adding up stars' columns into a `format` target.
fn star_pipelines(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    format: wgpu::TextureFormat,
) -> Pipelines {
    let stars_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("glow stars layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: NonZeroU64::new(size_of::<GlowStar>() as u64),
                },
                count: None,
            },
        ],
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("glow stars"),
        bind_group_layouts: &[Some(&stars_layout)],
        immediate_size: 0,
    });
    let add = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };
    let stars = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("glow stars"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: Some("vs_star"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: Some("fs_star"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState {
                    color: add,
                    alpha: add,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    });
    Pipelines {
        stars,
        stars_layout,
    }
}

fn target(
    device: &wgpu::Device,
    label: &str,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

/// A buffer holding `stars`, grown as needed.
fn star_buffer(device: &wgpu::Device, count: usize) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glow stars"),
        size: (count.max(1) * size_of::<GlowStar>()) as u64,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Records adding up `count` stars' columns into `sum`.
fn add_stars(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    group: &wgpu::BindGroup,
    sum: &wgpu::TextureView,
    count: usize,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("glow stars"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: sum,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    if count > 0 {
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..6, 0..count as u32);
    }
}

/// The glow pass and its output, for showing.
pub struct Glow {
    pipelines: Pipelines,
    tone: wgpu::RenderPipeline,
    tone_layout: wgpu::BindGroupLayout,
    uniform: wgpu::Buffer,
    stars: wgpu::Buffer,
    capacity: usize,
    group: wgpu::BindGroup,
    /// The sum of columns and the tone-mapped output, their size, and the
    /// tone pass's bind group.
    targets: Option<Targets>,
}

struct Targets {
    size: [u32; 2],
    sum: wgpu::TextureView,
    output: wgpu::TextureView,
    group: wgpu::BindGroup,
}

impl Glow {
    /// Makes the pass's pipelines.
    pub fn new(device: &wgpu::Device) -> Self {
        let module = shader(device);
        let pipelines = star_pipelines(device, &module, SUM_FORMAT);
        let tone_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glow tone layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glow tone"),
            bind_group_layouts: &[Some(&tone_layout)],
            immediate_size: 0,
        });
        let tone = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("glow tone"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_tone"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_tone"),
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
            label: Some("glow view"),
            size: size_of::<ViewUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let stars = star_buffer(device, 1);
        let group = Self::star_group(device, &pipelines.stars_layout, &uniform, &stars);
        Self {
            pipelines,
            tone,
            tone_layout,
            uniform,
            stars,
            capacity: 1,
            group,
            targets: None,
        }
    }

    fn star_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform: &wgpu::Buffer,
        stars: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glow stars"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: stars.as_entire_binding(),
                },
            ],
        })
    }

    /// Draws `stars` as seen by `view`, resizing the output if needed.
    /// Returns `true` when the output texture was recreated.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        view: &GlowView,
        stars: &[GlowStar],
    ) -> bool {
        let size = [view.size[0].max(1), view.size[1].max(1)];
        let resized = self.targets.as_ref().is_none_or(|t| t.size != size);
        if resized {
            let sum = target(
                device,
                "glow sum",
                size,
                SUM_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
            let output = target(
                device,
                "glow output",
                size,
                COLOR_FORMAT,
                wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            )
            .create_view(&wgpu::TextureViewDescriptor::default());
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("glow tone"),
                layout: &self.tone_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: self.uniform.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&sum),
                    },
                ],
            });
            self.targets = Some(Targets {
                size,
                sum,
                output,
                group,
            });
        }
        if stars.len() > self.capacity {
            self.capacity = stars.len().next_power_of_two();
            self.stars = star_buffer(device, self.capacity);
            self.group = Self::star_group(
                device,
                &self.pipelines.stars_layout,
                &self.uniform,
                &self.stars,
            );
        }
        queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::bytes_of(&ViewUniform::new(view)),
        );
        if !stars.is_empty() {
            queue.write_buffer(&self.stars, 0, bytemuck::cast_slice(stars));
        }
        let targets = self.targets.as_ref().expect("the targets were just made");
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("glow"),
        });
        add_stars(
            &mut encoder,
            &self.pipelines.stars,
            &self.group,
            &targets.sum,
            stars.len(),
        );
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glow tone"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &targets.output,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.tone);
            pass.set_bind_group(0, &targets.group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([encoder.finish()]);
        resized
    }

    /// The finished glow: premultiplied, transparent where there's none.
    pub fn output(&self) -> Option<&wgpu::TextureView> {
        self.targets.as_ref().map(|t| &t.output)
    }
}

/// How many pixels the glow may work through in a frame, counting each
/// star's pixels: 60 million, about 10 ms on an Intel Iris Xe, which
/// works through 6.6 million a millisecond seen from outside the galaxy
/// (`validation_glow_gpu.rs`).
pub const GLOW_FRAGMENTS: f64 = 6e7;

/// The size to draw the glow at for a view `full` pixels across, whose
/// stars cover `screens` screens' worth (see [`glow_coverage`]): `finest`
/// of its size in each direction (a third, say), or coarser, to keep to
/// [`GLOW_FRAGMENTS`].
pub fn glow_size(full: [u32; 2], screens: f64, finest: f64) -> [u32; 2] {
    let all = f64::from(full[0].max(1)) * f64::from(full[1].max(1));
    let pixels = (GLOW_FRAGMENTS / screens.max(1e-9)).min(all * finest * finest);
    let scale = (pixels / all).sqrt();
    full.map(|n| ((f64::from(n) * scale).ceil() as u32).max(1))
}

/// How many screens' worth of pixels drawing `stars` for `view` works
/// through, as the vertex shader draws them: a star close enough to light
/// a cone wider than 60° (or reaching behind the camera) covers the whole
/// screen; one in view, the square around its cone; one out of view,
/// nothing. A square's share is its area on the screen where it's
/// projected, at most 1. The caller can size the output to it, so a view
/// from inside the galaxy, where many stars cover everything, is drawn
/// coarser: its light is smooth there anyway.
pub fn glow_coverage(view: &GlowView, stars: &[GlowStar]) -> f64 {
    let u = ViewUniform::new(view);
    let forward = Vec3::from(u.forward);
    let (tan, aspect) = (f64::from(u.tan_half_fov), f64::from(u.aspect));
    let screen = 4.0 * tan * tan * aspect;
    stars
        .iter()
        .map(|s| {
            let p = Vec3::from(s.position);
            let d = f64::from(p.length());
            let reach = f64::from(GLOW_CUTOFF * s.spread);
            if d <= 1.155 * reach {
                return 1.0;
            }
            let half = (reach / d).asin();
            let toward = p / d as f32;
            let cos = f64::from(toward.dot(forward)).clamp(-1.0, 1.0);
            if cos.acos() > half + f64::from(u.frustum) {
                return 0.0;
            }
            // Behind or at the screen's edge: the shader falls back to the
            // whole screen when a corner nears the camera's plane.
            if cos < 1e-3 + half.sin() {
                return 1.0;
            }
            // The square, 2 tan(half) a side one unit along the cone, seen
            // `cos` off axis: as large on the screen, divided by cos³.
            let side = 2.0 * half.tan();
            (side * side / cos.powi(3) / screen).min(1.0)
        })
        .sum()
}

/// The column along each pixel's ray as `view` sees `stars`, before tone
/// mapping, row by row from the top: the shader's own sum, in single
/// precision, for checking. The device needs
/// `wgpu::Features::FLOAT32_BLENDABLE`.
pub fn glow_columns(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    view: &GlowView,
    stars: &[GlowStar],
) -> Vec<[f32; 4]> {
    let format = wgpu::TextureFormat::Rgba32Float;
    let size = [view.size[0].max(1), view.size[1].max(1)];
    let module = shader(device);
    let pipelines = star_pipelines(device, &module, format);
    let uniform = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glow view"),
        size: size_of::<ViewUniform>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&uniform, 0, bytemuck::bytes_of(&ViewUniform::new(view)));
    let buffer = star_buffer(device, stars.len());
    queue.write_buffer(&buffer, 0, bytemuck::cast_slice(stars));
    let group = Glow::star_group(device, &pipelines.stars_layout, &uniform, &buffer);
    let texture = target(
        device,
        "glow columns",
        size,
        format,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let sum = texture.create_view(&wgpu::TextureViewDescriptor::default());
    // Rows padded to 256 bytes for the copy.
    let row = (size[0] as usize * 16).next_multiple_of(256);
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("glow columns"),
        size: (row * size[1] as usize) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("glow columns"),
    });
    add_stars(&mut encoder, &pipelines.stars, &group, &sum, stars.len());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row as u32),
                rows_per_image: Some(size[1]),
            },
        },
        wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    readback.map_async(wgpu::MapMode::Read, .., |result| {
        result.expect("the columns map");
    });
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let bytes = readback
        .get_mapped_range(..)
        .expect("the columns are mapped");
    bytes
        .chunks(row)
        .flat_map(|r| bytemuck::cast_slice::<u8, [f32; 4]>(&r[..size[0] as usize * 16]).to_vec())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_compiles_and_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("glow.wgsl")).expect("WGSL parses");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("shader validates");
    }

    #[test]
    fn the_uniform_and_stars_match_the_shader() {
        let module = naga::front::wgsl::parse_str(include_str!("glow.wgsl")).expect("WGSL parses");
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("types lay out");
        let size = |name: &str| {
            let (handle, _) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(name))
                .expect(name);
            layouter[handle].size as usize
        };
        assert_eq!(size("View"), size_of::<ViewUniform>());
        assert_eq!(size("Star"), size_of::<GlowStar>());
    }

    #[test]
    fn the_glow_is_drawn_coarser_where_it_costs_more() {
        // A third of 1,920 × 1,050 when that's within the budget; coarser
        // when the stars cover more screens than it allows.
        assert_eq!(glow_size([1920, 1050], 100.0, 1.0 / 3.0), [640, 350]);
        let [w, h] = glow_size([1920, 1050], 4000.0, 1.0 / 3.0);
        let fragments = 4000.0 * f64::from(w) * f64::from(h);
        assert!(fragments <= GLOW_FRAGMENTS * 1.02 && fragments > GLOW_FRAGMENTS * 0.98);
        assert!((f64::from(w) / f64::from(h) - 1920.0 / 1050.0).abs() < 0.02);
    }

    #[test]
    fn the_whole_view_fits_in_its_cone() {
        // A square view 90° across: its corners are 54.7° off axis.
        let view = ViewUniform::new(&GlowView {
            size: [100, 100],
            forward: Vec3::Z,
            up: Vec3::Y,
            fov_y: std::f32::consts::FRAC_PI_2,
            exposure: 1.0,
        });
        assert!((view.frustum.to_degrees() - 54.7356).abs() < 1e-3);
    }
}
