//! Validation: the Milky Way's mass model and its potential.
//!
//! McMillan's (2017) best-fitting model, solved as his code (GalPot) solves
//! it (Dehnen & Binney 1998): each disk's thin layer analytically, the
//! rest as a multipole expansion. Checked three ways: the solver against
//! potentials worked out independently, the model against the numbers
//! McMillan publishes, and the Sun's orbit against the one his paper's
//! appendix traces with GalPot.

use std::f64::consts::{PI, TAU};

use worldline_core::DVec3;
use worldline_core::constants::{G, JULIAN_YEAR, PARSEC, SOLAR_MASS};
use worldline_core::milky_way::{
    GalacticOrbit, KILOPARSEC, MassModel, Potential, Resolution, gauss_legendre, mcmillan_2017,
    milky_way,
};

const KMS: f64 = 1e3;
const MYR: f64 = 1e6 * JULIAN_YEAR;

/// The solver's interpolation error scale: cubics in ln r, on a grid of
/// spacing h = ln 10/100, are good to order h⁴.
fn our_h4() -> f64 {
    let r = Resolution::default();
    (std::f64::consts::LN_10 / r.per_decade as f64).powi(4)
}

/// GalPot's: 201 points over seven decades of radius, h = ln(10⁷)/200.
/// Our solver has to beat it for a comparison with GalPot's numbers to test
/// theirs.
fn galpot_h4() -> f64 {
    ((1e7f64).ln() / 200.0).powi(4)
}

/// J_n(x), from Bessel's integral (1/2π)∫cos(nτ − x sin τ) dτ over a
/// period by the trapezoid rule, which for a periodic integrand is exact to
/// rounding once it has comfortably more points than x.
fn bessel(n: i32, x: f64) -> f64 {
    let points = (x + 10.0 * x.cbrt() + 40.0) as usize;
    let sum: f64 = (0..points)
        .map(|j| {
            let tau = TAU * j as f64 / points as f64;
            (n as f64 * tau - x * tau.sin()).cos()
        })
        .sum();
    sum / points as f64
}

/// The limit of a slowly converging sequence of partial sums, by Wynn's
/// epsilon algorithm.
fn wynn(sums: &[f64]) -> f64 {
    let mut previous = vec![0.0; sums.len() + 1];
    let mut current = sums.to_vec();
    let mut best = *sums.last().unwrap();
    let mut column = 0;
    while current.len() > 1 {
        let next: Vec<f64> = (0..current.len() - 1)
            .map(|n| {
                let d = current[n + 1] - current[n];
                previous[n + 1] + if d == 0.0 { f64::INFINITY } else { 1.0 / d }
            })
            .collect();
        column += 1;
        if next.iter().any(|x| !x.is_finite()) {
            break;
        }
        if column % 2 == 0 {
            best = *next.last().unwrap();
        }
        previous = current;
        current = next;
    }
    best
}

/// ∫₀^∞ f(k) J_n(kR) dk: in half periods of the Bessel function, summed
/// by Wynn's algorithm.
fn hankel(f: impl Fn(f64) -> f64, n: i32, r: f64) -> f64 {
    let rule = gauss_legendre(16);
    let half = PI / r;
    let mut sum = 0.0;
    let sums: Vec<f64> = (0..40)
        .map(|i| {
            let a = i as f64 * half;
            for &(x, w) in &rule {
                let k = a + 0.5 * half * (1.0 + x);
                sum += 0.5 * half * w * f(k) * bessel(n, k * r);
            }
            sum
        })
        .collect();
    wynn(&sums)
}

/// The force of a disk with surface density Σ₀e^(−R/R_d) and an
/// exponential vertical profile, by the Hankel transform (Binney &
/// Tremaine 2008, §2.6.2): Φ = −2πG ∫ J₀(kR) S(k) H(k, z) dk, with
/// S(k) = Σ₀R_d²/(1 + k²R_d²)^(3/2) and H the vertical profile smoothed by
/// e^(−k|z − z′|).
fn exponential_disk_force(sigma: f64, rd: f64, zd: f64, r: f64, z: f64) -> (f64, f64) {
    let a = 1.0 / zd;
    let profile = |k: f64| -> (f64, f64) {
        let (ek, ea) = ((-k * z).exp(), (-a * z).exp());
        let (mixed, mixed_dz) = if (k - a).abs() < 1e-9 * a {
            (z * ea, ea * (1.0 - a * z))
        } else {
            ((ea - ek) / (k - a), (k * ek - a * ea) / (k - a))
        };
        (
            0.5 * a * ((ek + ea) / (a + k) + mixed),
            0.5 * a * ((-k * ek - a * ea) / (a + k) + mixed_dz),
        )
    };
    let s = |k: f64| sigma * rd * rd / (1.0 + k * k * rd * rd).powf(1.5);
    let fr = -2.0 * PI * G * hankel(|k| k * s(k) * profile(k).0, 1, r);
    let fz = 2.0 * PI * G * hankel(|k| s(k) * profile(k).1, 0, r);
    (fr, fz)
}

/// The force of a spheroid stratified on m² = R² + z²/q², by its shells
/// (Binney & Tremaine 2008, eq. 2.140, differentiated):
/// F_R = −2πGqR ∫ρ(m²(u)) du/((1 + u)²√(q² + u)),
/// F_z = −2πGqz ∫ρ(m²(u)) du/((1 + u)(q² + u)^(3/2)),
/// m²(u) = R²/(1 + u) + z²/(q² + u). Integrated in ln(1 + u).
fn spheroid_force(density: impl Fn(f64) -> f64, q: f64, r: f64, z: f64) -> (f64, f64) {
    let rule = gauss_legendre(8);
    let (top, intervals) = (80.0, 4000);
    let step = top / intervals as f64;
    let (mut ir, mut iz) = (0.0, 0.0);
    for i in 0..intervals {
        for &(x, w) in &rule {
            let s = step * (i as f64 + 0.5 * (1.0 + x));
            let u = s.exp_m1();
            let du = 0.5 * step * w * s.exp();
            let rho = density((r * r / (1.0 + u) + z * z / (q * q + u)).sqrt());
            ir += du * rho / ((1.0 + u).powi(2) * (q * q + u).sqrt());
            iz += du * rho / ((1.0 + u) * (q * q + u).powf(1.5));
        }
    }
    (-2.0 * PI * G * q * r * ir, -2.0 * PI * G * q * z * iz)
}

#[test]
fn the_solver_matches_potentials_worked_out_independently() {
    let model = mcmillan_2017();
    let kpc = KILOPARSEC;

    // The dark-matter halo alone: an NFW sphere, Φ = −4πGρ₀r_s³ ln(1 + r/r_s)/r.
    let halo = model.spheroids[1];
    let alone = Potential::new(&MassModel {
        disks: vec![],
        spheroids: vec![halo],
    });
    let k = 4.0 * PI * G * halo.density * halo.scale.powi(3);
    let mut worst: f64 = 0.0;
    for r in [0.01, 0.3, 1.0, 8.2, 50.0, 300.0] {
        let r = r * kpc;
        let exact = -k * (r / halo.scale).ln_1p() / r;
        let mass = k * ((r / halo.scale).ln_1p() - r / (r + halo.scale));
        let (fr, _) = alone.force(r, 0.0);
        worst = worst
            .max((alone.potential(r, 0.0) / exact - 1.0).abs())
            .max((fr / (-mass / (r * r)) - 1.0).abs());
    }
    println!(
        "halo alone, against its closed form: worst relative error {worst:.1e} (h⁴ = {:.1e})",
        our_h4()
    );

    // The thin disk alone, against its Hankel transform.
    let disk = model.disks[0];
    let alone = Potential::new(&MassModel {
        disks: vec![disk],
        spheroids: vec![],
    });
    let mut worst_disk: f64 = 0.0;
    for (r, z) in [
        (8.2, 0.0208),
        (8.2, 0.3),
        (8.2, 1.1),
        (3.0, 0.1),
        (15.0, 0.5),
    ] {
        let (r, z) = (r * kpc, z * kpc);
        let (fr, fz) = alone.force(r, z);
        let (hr, hz) = exponential_disk_force(
            disk.surface_density,
            disk.scale_length,
            disk.scale_height,
            r,
            z,
        );
        let scale = hr.hypot(hz);
        println!(
            "thin disk at R = {:.1} kpc, z = {:.4} kpc: F_R {fr:.9e} (Hankel {hr:.9e}), F_z {fz:.9e} (Hankel {hz:.9e})",
            r / kpc,
            z / kpc,
        );
        worst_disk = worst_disk
            .max((fr - hr).abs() / scale)
            .max((fz - hz).abs() / scale);
    }
    println!(
        "thin disk alone, against the Hankel transform: worst relative error {worst_disk:.1e}"
    );

    // The flattened bulge alone, against its shells.
    let bulge = model.spheroids[0];
    let alone = Potential::new(&MassModel {
        disks: vec![],
        spheroids: vec![bulge],
    });
    let mut worst_bulge: f64 = 0.0;
    for (r, z) in [(0.5, 0.0), (1.0, 0.2), (2.0, 1.0), (8.2, 0.0208)] {
        let (r, z) = (r * kpc, z * kpc);
        let (fr, fz) = alone.force(r, z);
        let (sr, sz) = spheroid_force(|m| bulge.density_at(m, 0.0), bulge.axis_ratio, r, z);
        let scale = sr.hypot(sz);
        println!(
            "bulge at R = {:.1} kpc, z = {:.4} kpc: F_R {fr:.9e} (shells {sr:.9e}), F_z {fz:.9e} (shells {sz:.9e})",
            r / kpc,
            z / kpc,
        );
        worst_bulge = worst_bulge
            .max((fr - sr).abs() / scale)
            .max((fz - sz).abs() / scale);
    }
    println!("bulge alone, against its shells: worst relative error {worst_bulge:.1e}");
    println!("GalPot's own grid resolves {:.1e}", galpot_h4());
    assert!(worst < our_h4());
    assert!(worst_disk < galpot_h4() && worst_bulge < galpot_h4());
}

#[test]
fn the_model_is_mcmillans() {
    let model = mcmillan_2017();
    let clock = std::time::Instant::now();
    let potential = milky_way();
    println!("solved in {:.0} ms", clock.elapsed().as_secs_f64() * 1e3);
    let kpc = KILOPARSEC;
    let msun_pc2 = SOLAR_MASS / (PARSEC * PARSEC);

    // Table 3: the circular speed at his R₀ = 8.21 kpc, 233.1 km/s, and the
    // halo's density there, 0.0101 M☉/pc³, to their last digit.
    let v0 = potential.circular_speed(8.21 * kpc) / KMS;
    let halo = model.spheroids[1].density_at(8.21 * kpc, 0.0) / (SOLAR_MASS / PARSEC.powi(3));
    println!(
        "circular speed at 8.21 kpc: {v0:.4} km/s (McMillan: 233.1); halo density there {halo:.5} M☉/pc³ (0.0101)"
    );
    assert!((v0 - 233.1).abs() <= 0.05);
    assert!((halo - 0.0101).abs() <= 0.000_05);

    // Table 1: the gas disks' surface densities at 8.33 kpc and their
    // masses, to their last digit.
    for (disk, sigma, mass) in [(model.disks[2], 10.0, 1.1e10), (model.disks[3], 2.0, 1.2e9)] {
        let s = disk.surface_density_at(8.33 * kpc) / msun_pc2;
        let m = disk.mass() / SOLAR_MASS;
        println!(
            "{}: {s:.4} M☉/pc² at 8.33 kpc (McMillan: {sigma}); {m:.4e} M☉ ({mass:.1e})",
            disk.name
        );
        assert!((s - sigma).abs() <= 0.5);
        assert!((m - mass).abs() <= 0.05 * 10f64.powf(mass.log10().floor()));
    }
    for d in &model.disks[..2] {
        println!("{}: {:.4e} M☉", d.name, d.mass() / SOLAR_MASS);
    }
    let bulge = model.spheroids[0].mass_within(1e3 * kpc) / SOLAR_MASS;
    let unit = 2.0 * PI * G * msun_pc2;
    let kz = -potential.force(8.21 * kpc, 1.1 * kpc).1 / unit;
    println!(
        "bulge: {bulge:.4e} M☉; vertical force 1.1 kpc above the plane at 8.21 kpc: {kz:.3} × 2πG M☉/pc²"
    );

    // The appendix: the Sun's orbit in this model, traced by GalPot from
    // R = 8.2 kpc, z = 0.014 kpc, (v_R, v_z, v_φ) = (11.1, 7.25, −245) km/s
    // for 10 Gyr. GalPot's grid (see `galpot_h4`) leaves its numbers errors
    // of order h⁴.
    let h4 = galpot_h4();
    let x = DVec3::new(8.2 * kpc, 0.0, 0.014 * kpc);
    let v = DVec3::new(11.1, -245.0, 7.25) * KMS;
    let mut orbit = GalacticOrbit::new(potential, x, v);
    let energy = orbit.energy() / (KMS * KMS);
    let e = orbit.survey(10e3 * MYR, 0.5 * MYR);
    orbit.advance_to(10e3 * MYR);
    let drift = orbit.energy() / (energy * KMS * KMS) - 1.0;
    let angular = x.x * v.y / (kpc * KMS);
    // The guiding radius, where a circular orbit has the same angular
    // momentum.
    let (mut lo, mut hi) = (5.0 * kpc, 15.0 * kpc);
    for _ in 0..80 {
        let mid = 0.5 * (lo + hi);
        if mid * potential.circular_speed(mid) < angular.abs() * kpc * KMS {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let guiding = 0.5 * (lo + hi) / kpc;
    // GalPot extrapolates the halo past its grid's 1,000 kpc as r⁻³, where
    // the NFW halo's density really falls as r⁻³(1 + r_s/r)⁻²: that leaves
    // out 4πGρ₀r_s⁴/(r_max + r_s)² of depth everywhere inside.
    let halo = model.spheroids[1];
    let r_max = 1e3 * kpc;
    let missing = 4.0 * PI * G * halo.density * halo.scale.powi(4)
        / (r_max + halo.scale).powi(2)
        / (KMS * KMS);
    let galpot_energy = -153_122.0 - missing;
    println!("the Sun's orbit over 10 Gyr (energy kept to {drift:.1e}):");
    for (what, ours, galpot) in [
        ("guiding radius, kpc", guiding, 8.62121),
        ("least radius, kpc", e.r_min / kpc, 8.11988),
        ("greatest radius, kpc", e.r_max / kpc, 9.18502),
        ("greatest height, kpc", e.z_max / kpc, 0.104202),
        ("energy, (km/s)²", energy, galpot_energy),
        ("angular momentum, kpc km/s", angular, -2009.0),
    ] {
        let off = (ours / galpot - 1.0).abs();
        println!("  {what}: {ours:.6} (GalPot {galpot:.6}; off {off:.1e})");
        assert!(off < h4, "{what}");
    }
    println!(
        "  GalPot's energy is −153122, made {missing:.2} (km/s)² deeper for the halo's tail; its grid's h⁴ is {h4:.1e}"
    );
    println!(
        "  periods: radial {:.1} Myr, vertical {:.1} Myr, around the center {:.1} Myr",
        e.radial_period.unwrap() / MYR,
        e.vertical_period.unwrap() / MYR,
        e.azimuthal_period / MYR
    );
    // The integration adds less than the potential's own interpolation.
    assert!(drift.abs() < our_h4());
}
