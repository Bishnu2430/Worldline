//! Step 7.4's check: seen from above, the galaxy's swarm as the GPU draws
//! it thins out with McMillan's (2017) scale length. It needs a GPU, so it
//! is ignored by default; run it with
//! `cargo test --release -p worldline-render --test validation_glow_gpu -- --ignored --nocapture`.

use glam::{DVec3, Vec3};
use worldline_core::constants::{G, GM_SUN, PARSEC};
use worldline_core::galaxy_stars::{Component, Random, Star, spread, stellar_components};
use worldline_core::milky_way::{KILOPARSEC, mcmillan_2017};
use worldline_render::{GlowStar, GlowView, glow_columns};

/// The annuli the profile is measured in, kpc: 0.5 kpc wide, from 4 to 12,
/// past the bulge (which is cut off by 2.1 kpc) and inside where the disks
/// thin to a few stars a bin.
const INNER: f64 = 4.0;
const WIDTH: f64 = 0.5;
const BINS: usize = 16;

/// The scale length of ln y against x by least squares, −1/slope.
fn scale_length(x: &[f64], y: &[f64]) -> f64 {
    let ln: Vec<f64> = y.iter().map(|v| v.ln()).collect();
    let n = x.len() as f64;
    let (mx, my) = (x.iter().sum::<f64>() / n, ln.iter().sum::<f64>() / n);
    let sxy: f64 = x.iter().zip(&ln).map(|(a, b)| (a - mx) * (b - my)).sum();
    let sxx: f64 = x.iter().map(|a| (a - mx).powi(2)).sum();
    -sxx / sxy
}

fn centers() -> Vec<f64> {
    (0..BINS)
        .map(|k| INNER + WIDTH * (k as f64 + 0.5))
        .collect()
}

fn bin(r_kpc: f64) -> Option<usize> {
    let k = ((r_kpc - INNER) / WIDTH).floor();
    (k >= 0.0 && (k as usize) < BINS).then_some(k as usize)
}

/// The scale length of the stars' own surface density, counted in the
/// annuli: no smoothing, no drawing.
fn counted(stars: &[Star], picks: impl Iterator<Item = usize>) -> f64 {
    let mut counts = [0.0; BINS];
    for i in picks {
        let p = stars[i].position;
        if let Some(k) = bin(p.x.hypot(p.y) / KILOPARSEC) {
            counts[k] += 1.0;
        }
    }
    let density: Vec<f64> = centers()
        .iter()
        .zip(counts)
        .map(|(c, n)| n / (2.0 * std::f64::consts::PI * c * WIDTH))
        .collect();
    scale_length(&centers(), &density)
}

#[test]
#[ignore = "needs a GPU"]
fn from_above_the_drawn_disk_thins_out_with_the_models_scale_length() {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU");
    assert!(
        adapter
            .features()
            .contains(wgpu::Features::FLOAT32_BLENDABLE),
        "the check adds columns in single precision, which needs FLOAT32_BLENDABLE"
    );
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::FLOAT32_BLENDABLE,
        ..Default::default()
    }))
    .expect("a device");

    // The app's swarm, each star spread as the app spreads it, its weight
    // its mass in Suns, lengths in parsecs: the column comes out in Suns
    // per square parsec.
    let components = stellar_components(&mcmillan_2017());
    let stars = worldline_data::galaxy_stars();
    let total: f64 = components.iter().map(Component::mass).sum();
    let mass = total / stars.len() as f64;
    let suns = (mass * G / GM_SUN) as f32;
    let height = 200.0 * KILOPARSEC;
    let camera = DVec3::Z * height;
    let glow: Vec<GlowStar> = stars
        .iter()
        .map(|s| GlowStar {
            position: ((s.position - camera) / PARSEC).as_vec3().to_array(),
            spread: (spread(&components, mass, s.position) / PARSEC) as f32,
            color: [suns; 3],
            padding: 0.0,
        })
        .collect();
    // Straight down on the center, 15 kpc to each edge, 1,024 pixels
    // across: 29 pc a pixel.
    let pixels = 1024;
    let fov_y = 2.0 * (15.0 * KILOPARSEC / height).atan();
    let (forward, up) = (-Vec3::Z, Vec3::Y);
    let view = GlowView {
        size: [pixels, pixels],
        forward,
        up,
        fov_y: fov_y as f32,
        exposure: 1.0,
    };
    let columns = glow_columns(&device, &queue, &view, &glow);

    // Each pixel's ray, where it crosses the midplane, and the surface
    // density there: the column times the cosine of the ray's slant. The
    // model's surface density at the same places: its disks', and its
    // bulge's, integrated through.
    let (forward, up) = (forward.as_dvec3(), up.as_dvec3());
    let right = forward.cross(up);
    let tan = (0.5 * fov_y).tan();
    // The bulge's, through the disk 20 kpc each way in 10 pc steps,
    // tabulated every 10 pc in radius.
    let step = 10.0 * PARSEC;
    let bulge_at = |r: f64| -> f64 {
        (0..2000)
            .map(|k| {
                let z = (k as f64 + 0.5) * step;
                components
                    .iter()
                    .filter(|c| matches!(c, Component::Spheroid(_)))
                    .map(|b| b.density(r, z))
                    .sum::<f64>()
                    * 2.0
                    * step
            })
            .sum()
    };
    let start = INNER * KILOPARSEC - step;
    let table: Vec<f64> = (0..(WIDTH * BINS as f64 * 100.0) as usize + 3)
        .map(|k| bulge_at(start + k as f64 * step))
        .collect();
    let model = |r: f64| -> f64 {
        let disks: f64 = components
            .iter()
            .map(|c| match c {
                Component::Disk(d) => d.surface_density_at(r),
                Component::Spheroid(_) => 0.0,
            })
            .sum();
        let at = (r - start) / step;
        let k = at.floor() as usize;
        let through = table[k] + (table[k + 1] - table[k]) * (at - k as f64);
        (disks + through) * PARSEC * PARSEC * G / GM_SUN
    };
    let (mut drawn, mut expected, mut pixels_in) = ([0.0; BINS], [0.0; BINS], [0usize; BINS]);
    for row in 0..pixels {
        for col in 0..pixels {
            let x = (col as f64 + 0.5) / pixels as f64 * 2.0 - 1.0;
            let y = 1.0 - (row as f64 + 0.5) / pixels as f64 * 2.0;
            let ray = (forward + right * (x * tan) + up * (y * tan)).normalize();
            let reach = height / -ray.z;
            let r = (ray.x * reach).hypot(ray.y * reach);
            let Some(k) = bin(r / KILOPARSEC) else {
                continue;
            };
            let column = columns[(row * pixels + col) as usize][0] as f64;
            drawn[k] += column * -ray.z;
            expected[k] += model(r);
            pixels_in[k] += 1;
        }
    }
    let mean = |sums: [f64; BINS]| -> Vec<f64> {
        sums.iter()
            .zip(pixels_in)
            .map(|(s, n)| s / n as f64)
            .collect()
    };
    let (drawn, expected) = (mean(drawn), mean(expected));
    let r_drawn = scale_length(&centers(), &drawn);
    let r_model = scale_length(&centers(), &expected);

    // How much a 30,000-star sample's profile wanders: the same fit to the
    // stars counted in the annuli, over 300 samples drawn from them with
    // replacement.
    let mut random = Random::new(7);
    let n = stars.len();
    let samples: Vec<f64> = (0..300)
        .map(|_| {
            let picks: Vec<usize> = (0..n)
                .map(|_| (random.next_u64() % n as u64) as usize)
                .collect();
            counted(&stars, picks.into_iter())
        })
        .collect();
    let average = samples.iter().sum::<f64>() / samples.len() as f64;
    let sigma = (samples.iter().map(|s| (s - average).powi(2)).sum::<f64>()
        / (samples.len() - 1) as f64)
        .sqrt();
    let r_counted = counted(&stars, 0..n);

    println!("seen from 200 kpc above the center, 1,024 × 1,024 pixels, 29 pc each:");
    for ((c, d), e) in centers().iter().zip(&drawn).zip(&expected) {
        println!("  R {c:5.2} kpc: drawn {d:7.2} Suns/pc², the model's {e:7.2}");
    }
    println!(
        "scale length, 4 to 12 kpc: the model's {r_model:.4} kpc; drawn {r_drawn:.4} kpc, off by \
         {:.4} ({:.2} of the sample's σ, {sigma:.4} kpc; limit 3). The stars counted, without \
         drawing: {r_counted:.4} kpc",
        (r_drawn - r_model).abs(),
        (r_drawn - r_model).abs() / sigma
    );
    let profile = |values: &[f64]| -> Vec<(f64, f64)> {
        centers()
            .iter()
            .copied()
            .zip(values.iter().copied())
            .collect()
    };
    worldline_report::lines(
        "galaxy-glow",
        "The galaxy's swarm drawn on the GPU, seen from 200 kpc above: the disk thins out with the model's scale length",
        "distance from the center (kpc)",
        "surface density (Suns per pc²)",
    )
    .log_y()
    .dots("drawn on the GPU, 1,024 × 1,024 pixels", profile(&drawn))
    .expected("McMillan's (2017) model, seen the same way", profile(&expected))
    .note(&format!(
        "Scale length from 4 to 12 kpc: drawn {r_drawn:.4} kpc, the model's {r_model:.4}: {:.2} of a 30,000-star \
         sample's standard deviation, {sigma:.4} kpc (limit 3).",
        (r_drawn - r_model).abs() / sigma
    ))
    .save(worldline_report::here!());
    assert!((r_drawn - r_model).abs() < 3.0 * sigma);
}

#[test]
#[ignore = "needs a GPU"]
fn how_long_the_glow_takes() {
    // Not a physics check: how long this GPU takes to draw the swarm as a
    // glow at the app's resolution (a third of a 1,920 × 1,050 view), from
    // 40 kpc out and from inside the disk at the Sun's place, looking at the
    // galaxy's center.
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("a GPU");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    let components = stellar_components(&mcmillan_2017());
    let stars = worldline_data::galaxy_stars();
    let total: f64 = components.iter().map(Component::mass).sum();
    let mass = total / stars.len() as f64;
    let spreads: Vec<f32> = stars
        .iter()
        .map(|s| (spread(&components, mass, s.position) / PARSEC) as f32)
        .collect();
    let mut glow = worldline_render::Glow::new(&device);
    for (what, eye) in [
        ("from 40 kpc out", DVec3::new(0.0, -30.0, 26.5) * KILOPARSEC),
        (
            "from inside, at the Sun",
            DVec3::new(-8.2, 0.0, 0.02) * KILOPARSEC,
        ),
    ] {
        let forward = (-eye).normalize();
        let up = forward.cross(DVec3::Z).cross(forward).normalize();
        let view = GlowView {
            size: [640, 350],
            forward: forward.as_vec3(),
            up: up.as_vec3(),
            fov_y: 0.8,
            exposure: 1.0 / 150.0,
        };
        let glow_stars: Vec<GlowStar> = stars
            .iter()
            .zip(&spreads)
            .map(|(s, &h)| GlowStar {
                position: ((s.position - eye) / PARSEC).as_vec3().to_array(),
                spread: h,
                color: [1.0; 3],
                padding: 0.0,
            })
            .collect();
        glow.render(&device, &queue, &view, &glow_stars);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the GPU finishes");
        let frames = 20;
        let start = std::time::Instant::now();
        for _ in 0..frames {
            glow.render(&device, &queue, &view, &glow_stars);
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("the GPU finishes");
        }
        let each = start.elapsed().as_secs_f64() * 1e3 / frames as f64;
        let screens = worldline_render::glow_coverage(&view, &glow_stars);
        let fragments = screens * 640.0 * 350.0;
        println!(
            "{what}: {each:.1} ms a frame for {} stars, covering {screens:.1} screens: {:.1} \
             million fragments, {:.2} million a millisecond",
            glow_stars.len(),
            fragments / 1e6,
            fragments / 1e6 / each
        );
        // At the size the app draws it: a third of the view's, or coarser
        // to keep to the budget.
        let size = worldline_render::glow_size([1920, 1050], screens, 1.0 / 3.0);
        let sized = GlowView { size, ..view };
        glow.render(&device, &queue, &sized, &glow_stars);
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the GPU finishes");
        let start = std::time::Instant::now();
        for _ in 0..frames {
            glow.render(&device, &queue, &sized, &glow_stars);
            device
                .poll(wgpu::PollType::wait_indefinitely())
                .expect("the GPU finishes");
        }
        let each = start.elapsed().as_secs_f64() * 1e3 / frames as f64;
        println!(
            "  as the app draws it, {} × {}: {each:.1} ms a frame",
            size[0], size[1]
        );
    }
}
