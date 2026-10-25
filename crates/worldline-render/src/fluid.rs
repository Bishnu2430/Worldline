//! A self-gravitating fluid of SPH particles, stepped on the GPU in single
//! precision (see `fluid.wgsl`): worldline-core's `sph.rs`, the same
//! equations, for many more particles. It works in the fluid's own units,
//! in which G = 1; the caller chooses them (the body's radius and mass,
//! say) and converts.

use std::num::NonZeroU64;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// Buckets of the hashed grid: a multiple of 1,024 and at most 256 of
/// those, as the scan needs.
const TABLE: u32 = 1 << 18;

/// A fluid particle, in the fluid's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidParticle {
    /// Position.
    pub position: Vec3,
    /// Velocity.
    pub velocity: Vec3,
    /// Mass.
    pub mass: f32,
    /// Internal energy per unit mass.
    pub energy: f32,
    /// Smoothing length: a first guess going in, solved for coming out.
    pub h: f32,
    /// Density (coming out).
    pub density: f32,
    /// Artificial viscosity parameter α.
    pub viscosity: f32,
    /// Acceleration (coming out).
    pub acceleration: Vec3,
    /// Heating du/dt (coming out).
    pub heating: f32,
}

/// Matches `Particle` in fluid.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuParticle {
    position: [f32; 4],
    velocity: [f32; 4],
    fields: [f32; 4],
    force: [f32; 4],
    predicted: [f32; 4],
    extra: [f32; 4],
}

/// Matches `Params` in fluid.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Params {
    count: u32,
    table: u32,
    gamma: f32,
    settling: f32,
    entropy: f32,
    damping: f32,
    _padding: [f32; 2],
}

/// Matches `State` in fluid.wgsl.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct State {
    time: f32,
    dt: f32,
    cell: f32,
    _padding: f32,
    dt_next: u32,
    h_max: u32,
    h_min: u32,
    _padding1: u32,
}

/// The fluid's clock, as last read back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FluidClock {
    /// Time since it started.
    pub time: f32,
    /// The next step.
    pub dt: f32,
}

/// The passes of one step, in order.
const STEP: [&str; 15] = [
    "kick_drift",
    "clear",
    "count",
    "scan_blocks",
    "scan_sums",
    "scan_add",
    "scatter",
    "gather",
    "density",
    "predict",
    "gravity",
    "forces",
    "kick",
    "finish",
    "",
];

/// An SPH fluid on the GPU.
pub struct Fluid {
    count: u32,
    params: Params,
    params_buffer: wgpu::Buffer,
    state: wgpu::Buffer,
    particles: wgpu::Buffer,
    group: wgpu::BindGroup,
    pipelines: Vec<(&'static str, wgpu::ComputePipeline)>,
    /// The clock copied back without waiting: the copy, whether one is
    /// under way, and whether it has arrived.
    clock: wgpu::Buffer,
    clock_pending: bool,
    clock_ready: Arc<AtomicBool>,
}

impl Fluid {
    /// Uploads `particles` (ideal gas of adiabatic index `gamma`) and works
    /// out their densities and forces, ready to step.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        particles: &[FluidParticle],
        gamma: f32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fluid shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("fluid.wgsl").into()),
        });
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: false },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("fluid layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: NonZeroU64::new(size_of::<Params>() as u64),
                    },
                    count: None,
                },
                storage(1),
                storage(2),
                storage(3),
                storage(4),
                storage(5),
                storage(6),
                storage(7),
                storage(8),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fluid pipeline layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipelines = STEP
            .iter()
            .chain(&["rest"])
            .filter(|name| !name.is_empty())
            .map(|&name| {
                let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(name),
                    layout: Some(&pipeline_layout),
                    module: &shader,
                    entry_point: Some(name),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
                (name, pipeline)
            })
            .collect();
        let count = particles.len() as u32;
        let params = Params {
            count,
            table: TABLE,
            gamma,
            settling: 0.0,
            entropy: 0.0,
            damping: 0.0,
            _padding: [0.0; 2],
        };
        let buffer = |label, size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage,
                mapped_at_creation: false,
            })
        };
        let copyable = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::COPY_SRC;
        let params_buffer = buffer(
            "fluid params",
            size_of::<Params>() as u64,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let state = buffer("fluid state", size_of::<State>() as u64, copyable);
        let particle_buffer = buffer(
            "fluid particles",
            (particles.len().max(1) * size_of::<GpuParticle>()) as u64,
            copyable | wgpu::BufferUsages::VERTEX,
        );
        let counts = buffer(
            "fluid counts",
            u64::from(TABLE) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let starts = buffer(
            "fluid starts",
            (u64::from(TABLE) + 1) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let sorted = buffer(
            "fluid sorted",
            u64::from(count.max(1)) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        let blocks = buffer("fluid blocks", 256 * 4, wgpu::BufferUsages::STORAGE);
        let bucket_h = buffer(
            "fluid bucket h",
            u64::from(TABLE) * 4,
            wgpu::BufferUsages::STORAGE,
        );
        // Four vec4s a particle, in the grid's order.
        let near = buffer(
            "fluid neighbours",
            u64::from(count.max(1)) * 64,
            wgpu::BufferUsages::STORAGE,
        );
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("fluid"),
            layout: &layout,
            entries: &[
                (0, &params_buffer),
                (1, &state),
                (2, &particle_buffer),
                (3, &counts),
                (4, &starts),
                (5, &sorted),
                (6, &blocks),
                (7, &bucket_h),
                (8, &near),
            ]
            .map(|(binding, buffer)| wgpu::BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            }),
        });
        let gpu: Vec<GpuParticle> = particles
            .iter()
            .map(|p| GpuParticle {
                position: [p.position.x, p.position.y, p.position.z, p.mass],
                velocity: [p.velocity.x, p.velocity.y, p.velocity.z, p.energy],
                fields: [p.h, p.density, 1.0, p.viscosity],
                force: [0.0; 4],
                predicted: [0.0; 4],
                extra: [0.0; 4],
            })
            .collect();
        queue.write_buffer(&particle_buffer, 0, bytemuck::cast_slice(&gpu));
        queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&params));
        let h_max = particles.iter().map(|p| p.h).fold(0.0, f32::max);
        let h_min = particles.iter().map(|p| p.h).fold(f32::MAX, f32::min);
        queue.write_buffer(
            &state,
            0,
            bytemuck::bytes_of(&State {
                time: 0.0,
                dt: 0.0,
                cell: 2.0 * h_min.max(0.25 * h_max),
                _padding: 0.0,
                dt_next: f32::MAX.to_bits(),
                h_max: 0,
                h_min: f32::MAX.to_bits(),
                _padding1: 0,
            }),
        );
        let clock = buffer(
            "fluid clock",
            size_of::<FluidClock>() as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        let fluid = Self {
            count,
            params,
            params_buffer,
            state,
            particles: particle_buffer,
            group,
            pipelines,
            clock,
            clock_pending: false,
            clock_ready: Arc::new(AtomicBool::new(false)),
        };
        fluid.prime(device, queue);
        fluid
    }

    /// How many particles.
    pub fn len(&self) -> usize {
        self.count as usize
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The buffer of particles, for drawing them.
    pub(crate) fn particle_buffer(&self) -> &wgpu::Buffer {
        &self.particles
    }

    /// Works out the densities and forces where the particles are, without
    /// moving them: a step of no time.
    fn prime(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // dt = 0, the time left as it is.
        queue.write_buffer(&self.state, 4, bytemuck::bytes_of(&0.0f32));
        self.run(device, queue, 1, &STEP[1..14]);
    }

    /// Encodes and submits `repeat` runs of the passes `names`.
    fn run(&self, device: &wgpu::Device, queue: &wgpu::Queue, repeat: u32, names: &[&str]) {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fluid steps"),
        });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_bind_group(0, &self.group, &[]);
            for _ in 0..repeat {
                for name in names.iter().filter(|n| !n.is_empty()) {
                    let pipeline = &self
                        .pipelines
                        .iter()
                        .find(|(n, _)| n == name)
                        .expect("a pipeline for every pass")
                        .1;
                    let groups = match *name {
                        "clear" | "scan_add" => TABLE.div_ceil(256),
                        "scan_blocks" => TABLE / 1024,
                        "scan_sums" | "finish" => 1,
                        // Two particles to each of 64 invocations.
                        "gravity" => self.count.div_ceil(128),
                        _ => self.count.div_ceil(64),
                    };
                    pass.set_pipeline(pipeline);
                    pass.dispatch_workgroups(groups, 1, 1);
                }
            }
        }
        queue.submit([encoder.finish()]);
    }

    /// Takes `steps` steps, each of the time its Courant and force
    /// conditions allow.
    pub fn step(&self, device: &wgpu::Device, queue: &wgpu::Queue, steps: u32) {
        self.run(device, queue, steps, &STEP);
    }

    /// While settling, motion is damped at `damping` (1/time) and each
    /// particle's internal energy set to K ρ^(γ−1)/(γ − 1) after every
    /// step, `entropy` being K; `None` lets it go free.
    pub fn set_settling(&mut self, queue: &wgpu::Queue, settling: Option<(f32, f32)>) {
        let (entropy, damping) = settling.unwrap_or((0.0, 0.0));
        self.params.settling = if settling.is_some() { 1.0 } else { 0.0 };
        self.params.entropy = entropy;
        self.params.damping = damping;
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&self.params));
    }

    /// Sets every particle at rest, and works out its forces there.
    pub fn halt(&self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.run(device, queue, 1, &["rest"]);
        self.prime(device, queue);
    }

    /// Starts copying the clock back, after the steps submitted so far,
    /// unless a copy is already under way. [`Fluid::poll_clock`] picks it
    /// up once it arrives.
    pub fn request_clock(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.clock_pending {
            return;
        }
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fluid clock"),
        });
        encoder.copy_buffer_to_buffer(
            &self.state,
            0,
            &self.clock,
            0,
            size_of::<FluidClock>() as u64,
        );
        queue.submit([encoder.finish()]);
        let ready = Arc::clone(&self.clock_ready);
        self.clock.map_async(wgpu::MapMode::Read, .., move |_| {
            ready.store(true, Ordering::Release);
        });
        self.clock_pending = true;
    }

    /// The clock copy started by [`Fluid::request_clock`], if it has
    /// arrived; never waits for the GPU.
    pub fn poll_clock(&mut self, device: &wgpu::Device) -> Option<FluidClock> {
        if !self.clock_pending {
            return None;
        }
        // Runs the copy's callback if the GPU is done with it.
        let _ = device.poll(wgpu::PollType::Poll);
        if !self.clock_ready.swap(false, Ordering::Acquire) {
            return None;
        }
        // Nothing to read if the mapping failed (the device lost, say).
        let clock = self.clock.get_mapped_range(..).ok().map(|view| {
            let [time, dt]: [f32; 2] = bytemuck::pod_read_unaligned(&view[..]);
            FluidClock { time, dt }
        });
        if clock.is_some() {
            self.clock.unmap();
        }
        self.clock_pending = false;
        clock
    }

    /// Reads the particles and the clock back, waiting for the GPU.
    pub fn read(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (Vec<FluidParticle>, FluidClock) {
        let particle_bytes = u64::from(self.count) * size_of::<GpuParticle>() as u64;
        let state_bytes = size_of::<State>() as u64;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("fluid readback"),
            size: particle_bytes + state_bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("fluid readback"),
        });
        encoder.copy_buffer_to_buffer(&self.particles, 0, &readback, 0, particle_bytes);
        encoder.copy_buffer_to_buffer(&self.state, 0, &readback, particle_bytes, state_bytes);
        queue.submit([encoder.finish()]);
        readback.map_async(wgpu::MapMode::Read, .., |result| {
            result.expect("the fluid maps");
        });
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the GPU finishes");
        let view = readback.get_mapped_range(..).expect("the fluid is mapped");
        let gpu: &[GpuParticle] = bytemuck::cast_slice(&view[..particle_bytes as usize]);
        let state: State = bytemuck::pod_read_unaligned(&view[particle_bytes as usize..]);
        let particles = gpu
            .iter()
            .map(|p| FluidParticle {
                position: Vec3::new(p.position[0], p.position[1], p.position[2]),
                velocity: Vec3::new(p.velocity[0], p.velocity[1], p.velocity[2]),
                mass: p.position[3],
                energy: p.velocity[3],
                h: p.fields[0],
                density: p.fields[1],
                viscosity: p.fields[3],
                acceleration: Vec3::new(p.force[0], p.force[1], p.force[2]),
                heating: p.force[3],
            })
            .collect();
        (
            particles,
            FluidClock {
                time: state.time,
                dt: state.dt,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn module() -> naga::Module {
        naga::front::wgsl::parse_str(include_str!("fluid.wgsl")).expect("WGSL parses")
    }

    #[test]
    fn shader_compiles_and_validates() {
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module())
        .expect("shader validates");
    }

    #[test]
    fn the_buffers_match_the_shader() {
        let module = module();
        let mut layouter = naga::proc::Layouter::default();
        layouter.update(module.to_ctx()).expect("types lay out");
        for (name, size) in [
            ("Particle", size_of::<GpuParticle>()),
            ("Params", size_of::<Params>()),
            ("State", size_of::<State>()),
        ] {
            let (handle, _) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(name))
                .expect("the struct");
            assert_eq!(layouter[handle].size as usize, size, "{name}");
        }
    }
}
