//! Validation: light near a spinning black hole.
//!
//! The roadmap's checks: the shadow of a spinning hole is lopsided as
//! theory predicts, and a disk around it is brighter on the side moving
//! toward you. Rays are traced back through Kerr spacetime and checked
//! against results worked out other ways: step 3.1's rays for a hole that
//! doesn't spin, Bardeen's (1973) exact edge of the shadow, and, without
//! spin, where Binet's equation puts the rays when they cross the disk.

use std::f64::consts::PI;

use worldline_core::DVec3;
use worldline_core::kerr_light::{
    Camera, Traced, bardeen_shadow, disk_flux, disk_temperature, photon_orbits, seen_from_afar,
    spherical_orbit, trace,
};
use worldline_core::lensing::{self, Ray};

const TOLERANCE: f64 = 1e-12;

/// The look direction at angle `psi` from the hole, turned by `chi` around
/// the line to it, for a camera in the x–z plane on the +x side: chi = 0
/// toward −y (where a disk turning with the hole comes toward the camera),
/// chi = 90° toward +z (along the spin axis).
fn look(camera: DVec3, psi: f64, chi: f64) -> DVec3 {
    let toward = -camera.normalize();
    let across = DVec3::Z.cross(toward).normalize();
    let up = toward.cross(across);
    toward * psi.cos() + (across * chi.cos() + up * chi.sin()) * psi.sin()
}

/// The camera at distance `r` (units of GM/c²) and inclination `i` from
/// the spin axis, in the x–z plane.
fn place(r: f64, i: f64) -> DVec3 {
    DVec3::new(r * i.sin(), 0.0, r * i.cos())
}

#[test]
fn without_spin_it_is_step_3_1s_lens() {
    // M = 1 m: Schwarzschild rays from lensing.rs, in metres, are in units
    // of GM/c². Step 3.1's rays are good to step⁴ = 10⁻¹² per radian swept;
    // these, to the tolerance per step, so to steps × 10⁻¹², and past the
    // far cutoff they'd bend by under 10⁻¹⁰ rad more.
    let gm = worldline_core::constants::C.powi(2);
    let (mut worst, mut bound): (f64, f64) = (0.0, 0.0);
    let mut rays = 0;
    for r in [6.0, 40.0, 1e4] {
        let at = DVec3::new(r, 0.0, 0.0);
        let camera = Camera::at(0.0, at);
        for psi in [0.2, 0.5, 1.0, 2.0, 3.0] {
            let Ray::Escapes(swept) = lensing::trace(gm, r, psi, 1e-3) else {
                continue;
            };
            let look = look(at, psi, 0.3);
            let traced = trace(&camera, look, None, TOLERANCE);
            let sky = traced
                .sky
                .unwrap_or_else(|| panic!("captured at r = {r}, ψ = {psi}"));
            // Step 3.1's ray: swept around the hole from the camera's side.
            let side = (look - look.dot(at.normalize()) * at.normalize()).normalize();
            let expected = at.normalize() * swept.cos() + side * swept.sin();
            // The angle between, by its sine and cosine: exact when tiny.
            worst = worst.max(sky.cross(expected).length().atan2(sky.dot(expected)));
            rays += 1;
            bound = bound.max(traced.steps as f64 * TOLERANCE + 2.0 * PI * 1e-12 + 1e-10);
        }
    }
    println!("{rays} rays against step 3.1's: worst {worst:.1e} rad (bound {bound:.1e})");
    assert!(rays >= 12 && worst < bound);
}

/// The edge of the shadow along `chi`, by halving between a ray that falls
/// in and one that gets away: the ray just outside.
fn edge(camera: &Camera, chi: f64) -> Traced {
    let (mut inside, mut outside) = (0.0, PI / 2.0);
    for _ in 0..50 {
        let mid = 0.5 * (inside + outside);
        if trace(camera, look(camera.place, mid, chi), None, TOLERANCE)
            .sky
            .is_none()
        {
            inside = mid;
        } else {
            outside = mid;
        }
    }
    trace(camera, look(camera.place, outside, chi), None, TOLERANCE)
}

/// The photon orbit whose rays have ξ = `xi`, between the two equatorial
/// ones, where ξ falls from positive to negative.
fn orbit_with(a: f64, xi: f64) -> f64 {
    let (mut lo, mut hi) = photon_orbits(a);
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        if spherical_orbit(a, mid).0 > xi {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

#[test]
fn a_spinning_holes_shadow_is_bardeens() {
    // The rays along the edge of the shadow are those that spiral toward
    // Bardeen's spherical photon orbits: each ray's ξ = L/E names an orbit,
    // whose η = Q/E² it must have, however far the camera. A ray within δ
    // of the edge circles the hole about ln(1/δ)/2π times, so the halving
    // finds the edge to about the tracer's error, steps × 10⁻¹² of η.
    for (a, degrees) in [(0.5, 30.0), (0.9, 60.0), (0.9, 90.0), (0.99, 90.0)] {
        let i = f64::to_radians(degrees);
        let camera = Camera::at(a, place(1e3, i));
        let (mut worst, mut bound): (f64, f64) = (0.0, 0.0);
        let mut across = Vec::new();
        for k in 0..12 {
            let chi = 2.0 * PI * k as f64 / 12.0;
            let ray = edge(&camera, chi);
            let (xi, eta) = (ray.angular / ray.energy, ray.carter / ray.energy.powi(2));
            let (_, want) = spherical_orbit(a, orbit_with(a, xi));
            worst = worst.max((eta - want).abs() / want.abs().max(1.0));
            bound = bound.max(ray.steps as f64 * TOLERANCE);
            if k % 6 == 0 {
                across.push(seen_from_afar(a, i, xi, eta).expect("on the sky").0);
            }
        }
        // Lopsided: seen from afar, the edge across the spin axis on the
        // side coming toward you and on the side going away, against the
        // ends of Bardeen's outline.
        let outline = bardeen_shadow(a, i, 100_000);
        let ends = outline.iter().fold((f64::MAX, f64::MIN), |(lo, hi), p| {
            (lo.min(p.0), hi.max(p.0))
        });
        println!(
            "a = {a}, i = {degrees}°: edge rays' η off Bardeen's by {worst:.1e} (bound {bound:.1e}); \
             across the axis from {:.3} to {:.3} GM/c² (Bardeen's outline: {:.3} to {:.3})",
            across[0], across[1], ends.0, ends.1
        );
        assert!(worst < bound);
    }
}

/// Binet's equation, d²u/dφ² + u = 3u² (step 3.1's), for the ray seen at
/// `psi` from the hole by an observer at rest at `r`: its radius once it
/// has swept `angle` around the hole, or `None` if it falls in or escapes
/// first. Runge–Kutta steps of under 10⁻⁴ rad leave an error of order
/// 10⁻¹⁶.
fn binet(r: f64, psi: f64, angle: f64) -> Option<f64> {
    let n = 40_000;
    let step = angle / n as f64;
    let field = |u: f64| 3.0 * u * u - u;
    let mut u = 1.0 / r;
    let mut w = u * (1.0 - 2.0 * u).sqrt() / psi.tan();
    for _ in 0..n {
        if u <= 0.0 || (u > 1.0 / 3.0 && w > 0.0) {
            return None;
        }
        let k1 = (w, field(u));
        let k2 = (w + 0.5 * step * k1.1, field(u + 0.5 * step * k1.0));
        let k3 = (w + 0.5 * step * k2.1, field(u + 0.5 * step * k2.0));
        let k4 = (w + step * k3.1, field(u + step * k3.0));
        u += step / 6.0 * (k1.0 + 2.0 * k2.0 + 2.0 * k3.0 + k4.0);
        w += step / 6.0 * (k1.1 + 2.0 * k2.1 + 2.0 * k3.1 + k4.1);
    }
    Some(1.0 / u)
}

#[test]
fn without_spin_the_disk_is_crossed_where_binet_puts_the_rays() {
    // Without spin a ray stays in the plane of the hole, the camera and the
    // look direction, and crosses the equator when it has swept the angle
    // to the line where that plane meets it. There, light from a disk
    // circling at Ω = r^(−3/2) with u^t = 1/√(1 − 3/r) arrives shifted by
    // g = √(1 − 3/r)/(E − ΩL), with E = √(1 − 2/r_camera) for a camera at
    // rest and L = (camera × light's direction)_z.
    let (mut worst_r, mut worst_g, mut bound): (f64, f64, f64) = (0.0, 0.0, 0.0);
    let mut hits = 0;
    for degrees in [0.0, 60.0, 85.0] {
        let at = place(1e3, f64::to_radians(degrees));
        let camera = Camera::at(0.0, at);
        let out = at.normalize();
        for chi in [0.0, 1.0, 2.0, 3.0, 4.0, 5.0] {
            for psi in [0.006, 0.008, 0.012, 0.02, 0.04] {
                let look = look(at, psi, chi);
                let side = (look - look.dot(out) * out).normalize();
                let mut angle = (-out.z).atan2(side.z);
                if angle <= 0.0 {
                    angle += PI;
                }
                let Some(r) = binet(1e3, psi, angle).filter(|r| (6.0..100.0).contains(r)) else {
                    continue;
                };
                let traced = trace(&camera, look, Some((6.0, 100.0)), TOLERANCE);
                let hit = traced.disk.first().expect("it crosses the disk");
                let l = at.cross(-look).z;
                let g = (1.0 - 3.0 / r).sqrt() / ((1.0 - 2.0 / 1e3_f64).sqrt() - r.powf(-1.5) * l);
                worst_r = worst_r.max((hit.r / r - 1.0).abs());
                worst_g = worst_g.max((hit.g / g - 1.0).abs());
                bound = bound.max(traced.steps as f64 * TOLERANCE);
                hits += 1;
            }
        }
    }
    println!(
        "{hits} crossings: radius off Binet's by {worst_r:.1e}, shift by {worst_g:.1e} (bound {bound:.1e})"
    );
    assert!(hits > 20 && worst_r < bound && worst_g < bound);
}

#[test]
fn the_side_coming_toward_you_is_brighter() {
    // The roadmap's check. Seen nearly edge on, a disk turning with the
    // hole comes toward the camera on one side (chi = 0) and goes away on
    // the other. Light is shifted by g = 1/(u^t(E − ΩL)): up where the disk
    // approaches, down where it recedes, and its brightness, ∝ g⁴ (Iν/ν³
    // is the same for every observer), with it.
    let a = 0.9;
    let at = place(1e3, f64::to_radians(80.0));
    let camera = Camera::at(a, at);
    let disk = Some((
        worldline_core::gravity::kerr::innermost_stable_orbit(a, true),
        30.0,
    ));
    let shift = |chi: f64| {
        let traced = trace(&camera, look(at, 0.01, chi), disk, TOLERANCE);
        *traced.disk.first().expect("it crosses the disk")
    };
    let (toward, away) = (shift(0.0), shift(PI));
    println!(
        "a = {a}, 80° from the axis: coming toward you, light from r = {:.2} shifted by g = {:.4}; \
         going away, from r = {:.2} by g = {:.4}; brightness ratio (g/g)⁴ = {:.1}",
        toward.r,
        toward.g,
        away.r,
        away.g,
        (toward.g / away.g).powi(4)
    );
    assert!(toward.g > 1.0 && away.g < 1.0);
}

#[test]
fn the_disk_flux_is_page_and_thornes_integral() {
    // Li's closed form against the integral it solves (Page & Thorne
    // 1974): F = f/r with f = −Ω′(E − ΩL)⁻² ∫ from r_ms to r of
    // (E − ΩL) L′ dr, for circular orbits' E, L and Ω (Bardeen, Press &
    // Teukolsky 1972). L′ by a five-point stencil of h = 10⁻⁴ r: its
    // error, (h/r)⁴ = 10⁻¹⁶, and rounding, ε r/h ≈ 10⁻¹², are relative to
    // L′'s scale, and L′ vanishes at the innermost orbit, so they grow a
    // hundredfold 1% outside it: 10⁻¹⁰. Gauss–Legendre points, 16 to each
    // of 40 pieces, integrate the smooth rest to rounding.
    let orbit = |a: f64, r: f64| {
        let s = a * r.powf(-1.5);
        let root = (1.0 - 3.0 / r + 2.0 * s).sqrt();
        let e = (1.0 - 2.0 / r + s) / root;
        let l = r.sqrt() * (1.0 - 2.0 * s + a * a / (r * r)) / root;
        (e, l, 1.0 / (r.powf(1.5) + a))
    };
    let slope = |a: f64, r: f64| {
        let h = 1e-4 * r;
        let l = |r: f64| orbit(a, r).1;
        (l(r - 2.0 * h) - 8.0 * l(r - h) + 8.0 * l(r + h) - l(r + 2.0 * h)) / (12.0 * h)
    };
    let rule = worldline_core::milky_way::gauss_legendre(16);
    let mut worst: f64 = 0.0;
    for a in [0.0, 0.5, 0.9, 0.998] {
        let r_ms = worldline_core::gravity::kerr::innermost_stable_orbit(a, true);
        for r in [1.01 * r_ms, 1.5 * r_ms, 10.0, 100.0, 1000.0] {
            // In 40 pieces, even in ln(r − r_ms + 10⁻³ r_ms): L′ changes
            // fastest near the innermost orbit.
            let (lo, hi) = ((1e-3 * r_ms).ln(), (r - r_ms + 1e-3 * r_ms).ln());
            let integral: f64 = (0..40)
                .flat_map(|k| rule.iter().map(move |&(t, w)| (k, t, w)))
                .map(|(k, t, w)| {
                    let width = (hi - lo) / 40.0;
                    let v = lo + width * (k as f64 + 0.5 * (t + 1.0));
                    let u = r_ms + v.exp() - 1e-3 * r_ms;
                    let (e, l, omega) = orbit(a, u);
                    0.5 * width * w * v.exp() * (e - omega * l) * slope(a, u)
                })
                .sum();
            let (e, l, omega) = orbit(a, r);
            let d_omega = -1.5 * r.sqrt() / (r.powf(1.5) + a).powi(2);
            let expected = -d_omega / (e - omega * l).powi(2) * integral / r;
            worst = worst.max((disk_flux(a, r) / expected - 1.0).abs());
        }
    }
    let solar = worldline_core::constants::GM_SUN;
    println!(
        "closed form against the integral: worst {worst:.1e}; at a tenth of Eddington's rate, \
         a 10-Sun hole's disk peaks near {:.2e} K, Sagittarius A*'s (4.3 million Suns) near {:.2e} K",
        (1..400)
            .map(|k| disk_temperature(10.0 * solar, 0.0, 0.1, 6.0 + 0.1 * k as f64))
            .fold(0.0, f64::max),
        (1..400)
            .map(|k| disk_temperature(4.3e6 * solar, 0.0, 0.1, 6.0 + 0.1 * k as f64))
            .fold(0.0, f64::max)
    );
    assert!(worst < 1e-10);
}
