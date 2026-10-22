//! Validation: light bent by a non-spinning black hole.
//!
//! The roadmap's checks: the shadow's radius is √27 GM/c², and light
//! grazing the Sun is bent by 1.75″. Rays are traced through Schwarzschild
//! spacetime by Runge–Kutta (what the app's ray tracer looks up), and
//! checked against two exact results worked out another way: Darwin's
//! (1959) elliptic integral for the bending, and Synge's (1966) formula
//! for the shadow's edge seen from any distance.

use std::f64::consts::PI;

use worldline_core::constants::{C, GM_SUN, SOLAR_RADIUS};
use worldline_core::lensing::{BendTable, Ray, critical_impact, deflection, shadow_angle, trace};

/// Fine enough Runge–Kutta steps that their error, of order step⁴ per
/// radian swept, is 10⁻¹².
const FINE: f64 = 1e-3;

#[test]
fn the_shadow_is_root_27_and_its_edge_where_synge_puts_it() {
    let gm = 10.0 * GM_SUN;
    let m = gm / (C * C);
    // From each distance, the edge between rays that fall in and rays that
    // get away, found by halving: against Synge's sin θ = √27 (M/r)√(1 − 2M/r).
    let mut worst: f64 = 0.0;
    for r in [3.5, 6.0, 30.0, 1e3, 1e6] {
        let distance = r * m;
        let (mut inside, mut outside) = (0.0, PI);
        for _ in 0..60 {
            let mid = 0.5 * (inside + outside);
            match trace(gm, distance, mid, FINE) {
                Ray::Captured => inside = mid,
                Ray::Escapes(_) => outside = mid,
            }
        }
        let traced = 0.5 * (inside + outside);
        let exact = shadow_angle(gm, distance);
        // The impact parameter at the edge: √27 M for any observer.
        let b = distance * traced.sin() / (1.0 - 2.0 / r).sqrt() / m;
        println!(
            "from {r} GM/c²: the edge at {:.9}° (Synge {:.9}°), impact parameter {b:.9} GM/c² (√27 = {:.9})",
            traced.to_degrees(),
            exact.to_degrees(),
            27f64.sqrt()
        );
        worst = worst.max((traced / exact - 1.0).abs());
    }
    println!("worst: {worst:.1e}");
    assert!(worst < 1e-6);
    // The shadow's radius seen from far away.
    assert!((critical_impact(gm) / (27f64.sqrt() * m) - 1.0).abs() < 1e-15);
}

#[test]
fn rays_bend_as_darwins_integral_says() {
    let gm = 10.0 * GM_SUN;
    let m = gm / (C * C);
    // From very far away, a ray at impact parameter b is turned by
    // Darwin's exact angle; far out, by 4M/b + (15π/4)(M/b)².
    let far = 1e12 * m;
    let mut worst: f64 = 0.0;
    for b in [5.25, 5.5, 6.0, 10.0, 100.0, 1e4] {
        let psi = (b * m / far * (1.0 - 2.0 * m / far).sqrt()).asin();
        let Ray::Escapes(swept) = trace(gm, far, psi, FINE) else {
            panic!("captured at b = {b}");
        };
        let traced = swept - (PI - psi);
        let exact = deflection(gm, b * m).expect("outside the shadow");
        let weak = 4.0 / b + 15.0 * PI / 4.0 / (b * b);
        println!(
            "b = {b} GM/c²: traced {traced:.12} rad, Darwin {exact:.12} rad, weak field {weak:.6} rad"
        );
        worst = worst.max((traced - exact).abs());
    }
    println!("worst difference: {worst:.1e} rad");
    assert!(worst < 1e-9);
}

#[test]
fn light_grazing_the_sun_is_bent_by_1_75_arcseconds() {
    let arcsec = PI / 180.0 / 3600.0;
    let exact = deflection(GM_SUN, SOLAR_RADIUS).unwrap() / arcsec;
    let x = GM_SUN / (C * C * SOLAR_RADIUS);
    let einstein = 4.0 * x / arcsec;
    let second = 15.0 * PI / 4.0 * x * x / arcsec;
    println!(
        "at the Sun's edge: {exact:.7}″ (Einstein's 4GM/c²R: {einstein:.7}″, plus the next order (15π/4)(GM/c²R)²: {:.7}″)",
        einstein + second
    );
    // The roadmap's 1.75″, to its last digit.
    assert!((exact - 1.75).abs() <= 0.005);
    // Beyond the second order, (128/3)(GM/c²R)³ is 10⁻¹⁰″.
    assert!((exact - einstein - second).abs() < 1e-9);
}

/// The size of a pixel: the app's 0.8 rad field of view over the 1,080
/// rows of a full-HD screen, in rad.
const PIXEL: f64 = 0.8 / 1080.0;

#[test]
fn the_table_the_gpu_reads_places_the_sky_within_a_tenth_of_a_pixel() {
    // A ray at angle ψ ends up swept π − ψ + δ(ψ) around the hole. An error
    // Δδ in the table moves what the sky shows there by Δδ/|1 − δ′(ψ)| on
    // screen: near the shadow's edge, where δ′ is huge, hardly at all.
    let gm = 10.0 * GM_SUN;
    let m = gm / (C * C);
    let bending = |r: f64, psi: f64| match trace(gm, r * m, psi, FINE) {
        Ray::Escapes(swept) => Some(swept - (PI - psi)),
        Ray::Captured => None,
    };
    for r in [2.5, 8.0, 40.0, 1e4] {
        let table = BendTable::new(gm, r * m);
        let mut worst: f64 = 0.0;
        for k in 1..400 {
            // From just outside the edge to looking straight away.
            let x = BendTable::X_MIN * (k as f64 / 400.0).powi(2);
            let psi = table.edge + (PI - table.edge) * x.exp() * 0.999_63;
            let h = 1e-4 * (psi - table.edge).min(PI - psi);
            let (Some(here), Some(before), Some(after)) =
                (bending(r, psi), bending(r, psi - h), bending(r, psi + h))
            else {
                continue;
            };
            let slope = (after - before) / (2.0 * h);
            let shift = (table.bending(psi).unwrap() - here).abs() / (1.0 - slope).abs();
            worst = worst.max(shift);
        }
        println!(
            "from {r} GM/c²: the edge at {:.4}°; the table moves the sky by at most {:.1e} pixel",
            table.edge.to_degrees(),
            worst / PIXEL
        );
        assert!(worst < 0.1 * PIXEL);
    }
}
