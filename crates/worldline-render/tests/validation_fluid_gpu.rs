//! Validation: the GPU's SPH fluid (step 4.2) against worldline-core's
//! double-precision one, and how many particles this GPU can step
//! smoothly. It needs a GPU, so it is ignored by default; run it with
//! `cargo test --release -p worldline-render --test validation_fluid_gpu -- --ignored --nocapture`.

use glam::Vec3;
use worldline_core::constants::G;
use worldline_core::sph::{Fluid as CoreFluid, Particle, polytrope, polytrope_particles};
use worldline_render::{Fluid, FluidParticle};

/// Jupiter's mass and radius (IAU 2015 nominal), as in step 4.1's test.
const MASS: f64 = 1.266_865_3e17 / G;
const RADIUS: f64 = 6.9911e7;

fn gpu() -> (wgpu::Device, wgpu::Queue, String) {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    let info = adapter.get_info();
    (device, queue, format!("{}, {:?}", info.name, info.backend))
}

/// The fluid's units: the radius, the mass, and √(R³/GM).
fn units() -> (f64, f64, f64) {
    (RADIUS, MASS, (RADIUS.powi(3) / (G * MASS)).sqrt())
}

/// Core particles in the GPU's units.
fn to_gpu(particles: &[Particle]) -> Vec<FluidParticle> {
    let (l, m, t) = units();
    particles
        .iter()
        .map(|p| FluidParticle {
            position: (p.position / l).as_vec3(),
            velocity: (p.velocity * t / l).as_vec3(),
            mass: (p.mass / m) as f32,
            energy: (p.energy * (t / l).powi(2)) as f32,
            h: (p.h / l) as f32,
            density: (p.density * l.powi(3) / m) as f32,
            viscosity: p.viscosity as f32,
            acceleration: Vec3::ZERO,
            heating: 0.0,
        })
        .collect()
}

/// GPU particles back in SI, as a core fluid (its densities and forces
/// worked out again, in double precision).
fn to_core(particles: &[FluidParticle]) -> CoreFluid {
    let (l, m, t) = units();
    let core = particles
        .iter()
        .map(|p| Particle {
            position: p.position.as_dvec3() * l,
            velocity: p.velocity.as_dvec3() * l / t,
            mass: f64::from(p.mass) * m,
            energy: f64::from(p.energy) * (l / t).powi(2),
            h: f64::from(p.h) * l,
            density: f64::from(p.density) * m / l.powi(3),
            viscosity: f64::from(p.viscosity),
        })
        .collect();
    CoreFluid::new(core, 2.0)
}

#[test]
#[ignore = "needs a GPU"]
fn the_gpus_densities_and_forces_are_the_cores() {
    let (device, queue, name) = gpu();
    let (core, _) = polytrope(MASS, RADIUS, 1000);
    let fluid = Fluid::new(&device, &queue, &to_gpu(&core.particles), 2.0);
    let (gpu, _) = fluid.read(&device, &queue);
    let (l, m, t) = units();
    let forces = core.accelerations();
    let largest = forces.iter().map(|f| f.0.length()).fold(0.0, f64::max);
    let (mut density, mut force): (f64, f64) = (0.0, 0.0);
    for ((c, g), f) in core.particles.iter().zip(&gpu).zip(&forces) {
        let rho = f64::from(g.density) * m / l.powi(3);
        density = density.max((rho / c.density - 1.0).abs());
        let a = g.acceleration.as_dvec3() * l / (t * t);
        force = force.max((a - f.0).length() / largest);
    }
    // The GPU's Newton's method stops at 10⁻⁵ of h, so its densities, ∝
    // h⁻³, may be 3 × 10⁻⁵ off; pressure, ∝ ρ² here, twice that, and the
    // forces with it. Single precision's rounding (6 × 10⁻⁸ a term, over
    // tens of neighbours and a thousand pulls) is well under both.
    println!(
        "{name}, {} particles: densities off the core's by {density:.1e} at most (limit 3e-5), accelerations by {force:.1e} of the largest (limit 1e-4)",
        gpu.len()
    );
    assert!(density < 3e-5 && force < 1e-4);
}

/// The radius holding half the mass, and the kinetic energy over the
/// binding energy, of the GPU's fluid, read back.
fn measure(fluid: &Fluid, device: &wgpu::Device, queue: &wgpu::Queue) -> (CoreFluid, f64) {
    let (particles, clock) = fluid.read(device, queue);
    (to_core(&particles), f64::from(clock.time) * units().2)
}

#[test]
#[ignore = "needs a GPU"]
fn left_alone_on_the_gpu_a_planet_stays_in_equilibrium() {
    // Step 4.1's check, on the GPU: Jupiter as a polytrope of about 1,000
    // particles, settled for five dynamical times, then left alone for
    // ten. Its half-mass radius within h/2 of where it starts, its kinetic
    // energy under (h/2R)² of its binding energy, its energy kept to the
    // steps' (Δt/t_dyn)², and single precision's rounding on top: 10⁻⁶ a
    // step at most, over the steps taken.
    let (device, queue, name) = gpu();
    let (core, k) = polytrope(MASS, RADIUS, 1000);
    let (l, m, t) = units();
    let mut fluid = Fluid::new(&device, &queue, &to_gpu(&core.particles), 2.0);
    // K in the fluid's units: P/ρ² scales as L⁵/(M T²).
    let entropy = (k * m * t * t / l.powi(5)) as f32;
    fluid.set_settling(&queue, Some((entropy, 1.0 / 0.3)));
    while fluid.read(&device, &queue).1.time < 5.0 {
        fluid.step(&device, &queue, 20);
    }
    fluid.set_settling(&queue, None);
    fluid.halt(&device, &queue);
    let (settled, begun) = measure(&fluid, &device, &queue);
    let h = settled.particles.iter().map(|p| p.h).sum::<f64>() / settled.particles.len() as f64 / l;
    let start = settled.mass_radius(0.5) / l;
    let energy = settled.energy();
    let (mut moved, mut stirred, mut steps, mut now): (f64, f64, u32, f64) = (0.0, 0.0, 0, begun);
    while now - begun < 10.0 * t {
        fluid.step(&device, &queue, 10);
        steps += 10;
        let (state, time) = measure(&fluid, &device, &queue);
        moved = moved.max((state.mass_radius(0.5) / l - start).abs());
        stirred = stirred.max(state.kinetic() / state.gravitational().abs());
        now = time;
    }
    let (end, _) = measure(&fluid, &device, &queue);
    let drift = (end.energy() - energy) / energy.abs();
    let step = (now - begun) / t / f64::from(steps);
    let limit = step * step + 1e-6 * f64::from(steps);
    println!(
        "{name}: {} particles, mean h = {h:.3} R; over {:.1} dynamical times ({steps} steps) the half-mass radius moved at most {moved:.4} R ({:.2} h) from {start:.4} R; kinetic energy at most {stirred:.1e} of the binding energy (limit {:.1e}); energy kept to {drift:.1e} (limit {limit:.1e})",
        end.particles.len(),
        (now - begun) / t,
        moved / h,
        (0.5 * h).powi(2)
    );
    assert!(moved < 0.5 * h && stirred < (0.5 * h).powi(2) && drift.abs() < limit);
}

#[test]
#[ignore = "needs a GPU"]
fn the_particle_budget_for_this_gpu() {
    // Not a physics check: how long a step takes for ever more particles,
    // each fluid first settled for two dynamical times so its smoothing
    // lengths are its own, then timed over 30 steps.
    let (device, queue, name) = gpu();
    let (l, m, t) = units();
    for count in [2048, 4096, 8192, 16384, 32768, 65536] {
        let (particles, k) = polytrope_particles(MASS, RADIUS, count);
        let mut fluid = Fluid::new(&device, &queue, &to_gpu(&particles), 2.0);
        let entropy = (k * m * t * t / l.powi(5)) as f32;
        fluid.set_settling(&queue, Some((entropy, 1.0 / 0.3)));
        while fluid.read(&device, &queue).1.time < 2.0 {
            fluid.step(&device, &queue, 10);
        }
        fluid.set_settling(&queue, None);
        let start = std::time::Instant::now();
        fluid.step(&device, &queue, 30);
        fluid.read(&device, &queue);
        let ms = start.elapsed().as_secs_f64() * 1e3 / 30.0;
        println!("{name}: {} particles, {ms:.2} ms a step", fluid.len());
    }
}

#[test]
#[ignore = "needs a GPU"]
fn the_clock_comes_back_without_waiting() {
    // The app reads the fluid's clock without stalling a frame: the copy
    // that comes back in the background is exactly the one read by
    // waiting.
    let (device, queue, name) = gpu();
    let (particles, _) = polytrope_particles(MASS, RADIUS, 1000);
    let mut fluid = Fluid::new(&device, &queue, &to_gpu(&particles), 2.0);
    fluid.step(&device, &queue, 5);
    fluid.request_clock(&device, &queue);
    let mut polls = 0;
    let clock = loop {
        polls += 1;
        if let Some(clock) = fluid.poll_clock(&device) {
            break clock;
        }
        std::thread::yield_now();
    };
    let (_, waited) = fluid.read(&device, &queue);
    println!(
        "{name}: after 5 steps the clock came back without waiting ({polls} polls): time {} (waited for: {}), next step {}",
        clock.time, waited.time, clock.dt
    );
    assert!(clock.time > 0.0 && clock == waited);
}
