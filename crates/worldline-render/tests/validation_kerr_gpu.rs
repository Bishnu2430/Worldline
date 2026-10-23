//! Validation: the shader's rays past a spinning black hole against
//! worldline-core's, traced in double precision with error control. It
//! needs a GPU, so it is ignored by default; run it with
//! `cargo test --release -p worldline-render -- --ignored --nocapture`.
//!
//! The standard is a pixel of the app's usual view, 0.8 rad over 1,080
//! lines. Single precision and the shader's steps should keep 99% of rays
//! within a tenth of one on the sky. Rays that pass close to the shadow's
//! edge circle the hole first, and any error grows e^(2π) times each turn:
//! where a small change of direction moves a ray's end on the sky A times
//! as far, the same error in the picture is A times smaller, and that is
//! held to a tenth of a pixel. Within a pixel of the edge itself, rays may
//! disagree about falling in.

use glam::{DVec3, Vec3};
use worldline_core::kerr_light::{Camera, photon_orbits, trace};
use worldline_render::{KerrLens, trace_rays};

/// A pixel of the app's usual view, rad.
const PIXEL: f64 = 0.8 / 1080.0;

/// Directions spread evenly over the sky (a Fibonacci sphere).
fn spread(n: usize) -> impl Iterator<Item = DVec3> {
    (0..n).map(move |k| {
        let z = 1.0 - (2.0 * k as f64 + 1.0) / n as f64;
        let phi = k as f64 * std::f64::consts::PI * (3.0 - 5f64.sqrt());
        let s = (1.0 - z * z).sqrt();
        DVec3::new(s * phi.cos(), s * phi.sin(), z)
    })
}

#[test]
#[ignore = "needs a GPU"]
fn the_shaders_rays_are_the_cores() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    for (a, place) in [
        (0.9, DVec3::new(30.0, 0.0, 8.0)),
        (0.99, DVec3::new(0.0, -12.0, 3.0)),
        (0.0, DVec3::new(500.0, 200.0, 100.0)),
    ] {
        let camera = Camera::at(a, place);
        let lens = KerrLens {
            camera: camera.place.as_vec3(),
            launch: camera.launch().as_mat4(),
            spin: a as f32,
            capture: photon_orbits(a).0 as f32,
            far: (100.0 * camera.r) as f32,
            disk: None,
        };
        // All around, and as many again within ten shadows of the hole.
        let toward = -place.normalize();
        let near = 10.0 * 27f64.sqrt() / camera.r;
        let looks: Vec<DVec3> = spread(4000)
            .chain(spread(4000).map(|d| (toward + near * (d - d.dot(toward) * toward)).normalize()))
            .collect();
        let single: Vec<Vec3> = looks.iter().map(|d| d.as_vec3()).collect();
        let gpu = trace_rays(&device, &queue, &lens, &single);
        // Whether a ray is within a pixel of the shadow's edge: a pixel
        // over, one way or another, it falls in where it didn't or the
        // other way round.
        let sky = |look: DVec3| trace(&camera, look, None, 1e-10).sky;
        let near_edge = |look: DVec3| {
            let (u, v) = look.any_orthonormal_pair();
            [u, -u, v, -v]
                .iter()
                .any(|&s| sky((look + PIXEL * s).normalize()).is_none() != sky(look).is_none())
        };
        // How many times farther its end on the sky moves than its start.
        let magnified = |look: DVec3, end: DVec3| {
            let (u, v) = look.any_orthonormal_pair();
            [u, v]
                .iter()
                .filter_map(|&s| sky((look + 1e-6 * s).normalize()))
                .map(|moved| (moved - end).length() / 1e-6)
                .fold(1.0, f64::max)
        };
        let mut errors = Vec::new();
        let (mut disagree, mut circled) = (0, 0);
        for (look, gpu) in looks.iter().zip(gpu) {
            match (sky(*look), gpu) {
                (Some(core), Some(gpu)) => {
                    let gpu = gpu.as_dvec3().normalize();
                    let error = core.cross(gpu).length().atan2(core.dot(gpu));
                    if error > 0.1 * PIXEL {
                        let times = magnified(*look, core);
                        assert!(
                            error / times < 0.1 * PIXEL,
                            "{look}: off by {error:e} rad, magnified only {times:.1} times"
                        );
                        circled += 1;
                    }
                    errors.push(error);
                }
                (None, None) => {}
                _ => {
                    assert!(
                        near_edge(*look),
                        "{look}: disagrees on falling in, far from the edge"
                    );
                    disagree += 1;
                }
            }
        }
        errors.sort_by(f64::total_cmp);
        let at = |q: f64| errors[((errors.len() - 1) as f64 * q) as usize];
        println!(
            "a = {a}, camera at r = {:.1}: {} rays escape, off by {:.1e} rad at the median, \
             {:.1e} at 99% (a pixel is {PIXEL:.1e}); {circled} more than a tenth of a pixel \
             off, all magnified enough, and {disagree} disagreeing on falling in, all at the edge",
            camera.r,
            errors.len(),
            at(0.5),
            at(0.99),
        );
        assert!(at(0.99) < 0.1 * PIXEL);
    }
}

#[test]
#[ignore = "needs a GPU"]
fn how_long_a_traced_sky_takes() {
    // Not a physics check: how long this GPU takes to trace an 800 × 500
    // view (the app traces at half resolution) past a hole 26 GM/c² away,
    // nearly edge on, turning the camera a little each frame.
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    let gray = vec![40u8; 64 * 32 * 4];
    let map = worldline_render::Image {
        width: 64,
        height: 32,
        rgba: &gray,
    };
    let mut sky = worldline_render::Sky::new(&device, &queue, &map);
    let a = 0.9985;
    let place = DVec3::new(-26.0, -5.0, 4.0);
    let camera = Camera::at(a, place);
    let view = |turn: f64| {
        let forward = (-place.normalize() + DVec3::new(0.0, 0.0, turn)).normalize();
        let up = forward.cross(DVec3::Z).cross(forward).normalize();
        (forward.as_vec3(), up.as_vec3())
    };
    let view = |turn: f64| worldline_render::SkyView {
        size: [800, 500],
        forward: view(turn).0,
        up: view(turn).1,
        fov_y: 0.8,
        to_map: glam::Mat3::IDENTITY,
        exposure: 1.0,
        lens: None,
        kerr: Some(KerrLens {
            camera: camera.place.as_vec3(),
            launch: camera.launch().as_mat4(),
            spin: a as f32,
            capture: photon_orbits(a).0 as f32,
            far: (100.0 * camera.r) as f32,
            disk: None,
        }),
    };
    sky.render(&device, &queue, &view(0.0));
    device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("the GPU finishes");
    let frames = 10;
    let start = std::time::Instant::now();
    for frame in 0..frames {
        sky.render(&device, &queue, &view(1e-3 * f64::from(frame + 1)));
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the GPU finishes");
    }
    println!(
        "{}: {:.1} ms a frame",
        adapter.get_info().name,
        start.elapsed().as_secs_f64() * 1e3 / frames as f64
    );
}
