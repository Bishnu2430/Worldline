//! Validation: black holes evaporate by Hawking radiation, in the
//! textbook model (a black body at the Hawking temperature, the size of
//! the horizon, emitting photons): the roadmap's check is that their
//! lifetimes are t = 5120π G²M³/(ħc⁴).
//!
//! `cargo test --release -p worldline-core --test validation_hawking -- --nocapture`

use std::f64::consts::PI;

use worldline_core::compact::schwarzschild_radius;
use worldline_core::constants::{AU, C, DAY, G, GM_SUN, HBAR, JULIAN_YEAR, K_B};
use worldline_core::gravity::PostNewtonian;
use worldline_core::hawking::{CMB_TEMPERATURE, after, lifetime, power, temperature};
use worldline_core::hierarchy::Hierarchy;
use worldline_core::{Body, DVec3, System};

#[test]
fn the_lifetime_follows_from_the_temperature_and_a_black_body() {
    // Built from first principles, independently of the module's
    // formulas: Stefan–Boltzmann's σ = π² k⁴/(60 ħ³ c²), the Hawking
    // temperature ħc³/(8πGMk), and the horizon's area 4π (2GM/c²)².
    let sigma = PI * PI * K_B.powi(4) / (60.0 * HBAR.powi(3) * C * C);
    println!("σ = {sigma:.9e} W/m²K⁴ (CODATA: 5.670374419e-8)");
    assert!((sigma / 5.670_374_419e-8 - 1.0).abs() < 1e-9);
    for mass in [1e8, 1e11, 7.35e22, 1.989e30] {
        let gm = G * mass;
        let t = HBAR * C.powi(3) / (8.0 * PI * gm * K_B);
        let r = 2.0 * gm / (C * C);
        let p = sigma * t.powi(4) * 4.0 * PI * r * r;
        assert!((temperature(gm) / t - 1.0).abs() < 1e-12);
        assert!((power(gm) / p - 1.0).abs() < 1e-12);
        // The lifetime is the time to radiate the mass away,
        // ∫₀^M c² dM′/P(M′), here by Simpson's rule (exact: P ∝ 1/M²).
        let n = 64;
        let integrand = |m: f64| {
            C * C
                / (sigma
                    * (HBAR * C.powi(3) / (8.0 * PI * G * m * K_B)).powi(4)
                    * 4.0
                    * PI
                    * (2.0 * G * m / (C * C)).powi(2))
        };
        let h = mass / n as f64;
        let mut sum = integrand(mass);
        for k in 1..n {
            sum += integrand(k as f64 * h) * if k % 2 == 1 { 4.0 } else { 2.0 };
        }
        let radiated = sum * h / 3.0;
        // The roadmap's formula.
        let roadmap = 5120.0 * PI * G * G * mass.powi(3) / (HBAR * C.powi(4));
        println!(
            "{mass:.3e} kg: {t:.3e} K, {p:.3e} W, lifetime {roadmap:.4e} s ({:.3e} years); integrated {radiated:.4e} s",
            roadmap / JULIAN_YEAR
        );
        assert!((lifetime(gm) / roadmap - 1.0).abs() < 1e-12);
        assert!((radiated / roadmap - 1.0).abs() < 1e-12);
    }
}

#[test]
fn a_mini_black_hole_evaporates_on_schedule_in_the_simulation() {
    // A hole of 10⁸ kg (smaller than a proton, a hundred thousand tonnes)
    // on a circular orbit 1 AU from the Sun, in the hierarchy the app runs:
    // it must vanish within one step of its lifetime, 2.7 years, having
    // shrunk as M(t) = M₀ (1 − t/τ)^(1/3) on the way.
    let mass = 1e8;
    let gm = G * mass;
    let tau = lifetime(gm);
    let speed = (GM_SUN / AU).sqrt();
    let mut h = Hierarchy::new(
        System::new(vec![
            Body::new("Sun", GM_SUN, 6.957e8),
            Body::new("hole", gm, schwarzschild_radius(gm))
                .at(DVec3::new(AU, 0.0, 0.0))
                .moving(DVec3::new(0.0, speed, 0.0)),
        ]),
        Box::new(PostNewtonian::default()),
        Vec::new(),
    );
    let mut checked = false;
    let mut longest: f64 = 0.0;
    while h.top.bodies.len() == 2 {
        longest = longest.max(h.step(DAY));
        if !checked && h.time() > 0.5 * tau {
            let expected = mass * (1.0 - h.time() / tau).cbrt();
            let now = h.top.bodies[1].gm / G;
            println!(
                "at {:.3} years: {now:.6e} kg, expected {expected:.6e} (off by {:.1e}); horizon {:.3e} m",
                h.time() / JULIAN_YEAR,
                (now / expected - 1.0).abs(),
                h.top.bodies[1].radius
            );
            assert!((now / expected - 1.0).abs() < 1e-9);
            assert!(
                (h.top.bodies[1].radius / schwarzschild_radius(h.top.bodies[1].gm) - 1.0).abs()
                    < 1e-9
            );
            checked = true;
        }
        assert!(h.time() < 2.0 * tau, "it should be gone");
    }
    let gone = h.take_evaporated();
    assert_eq!(gone.len(), 1);
    assert_eq!(gone[0].name, "hole");
    let late = gone[0].time - tau;
    println!(
        "lifetime {:.6} years (5120πG²M³/ħc⁴); it vanished {:.3} days after, within one step ({:.3} days)",
        tau / JULIAN_YEAR,
        late / DAY,
        longest / DAY
    );
    assert!(late >= 0.0 && late <= longest);
    assert!(checked);
    // Its last moments radiate its rest energy; before that, the step
    // ending at its lifetime can't overshoot it.
    assert!(after(gm, tau * (1.0 + 1e-9)).is_none());
}

#[test]
fn what_the_textbook_model_leaves_out() {
    // Carr, Kohri, Sendouda & Yokoyama (2010), Phys. Rev. D 81, 104019,
    // section "Evaporation of primordial black holes: Lifetime", give
    // dM₁₀/dt = −5.34 × 10⁻⁵ f(M) M₁₀⁻² s⁻¹ (M₁₀ in units of 10¹⁰ g), with
    // f = 1 for holes heavier than 10¹⁷ g, which emit photons, three
    // neutrinos and gravitons, with greybody factors. That is 13.5 times
    // the textbook photon black body's rate (the label's number, to its
    // digits). Printed: the mass that ends its life today (theirs:
    // 5.1 × 10¹⁴ g, with f = 1.9) and the mass as warm as the cosmic
    // background.
    let m10 = 1e7; // 10¹⁰ g in kg
    let textbook = power(G * m10) / (C * C) / m10; // in M₁₀ per second, at M₁₀ = 1
    let ratio = 5.34e-5 / textbook;
    println!("Carr et al.'s rate for f = 1 is {ratio:.2} times the textbook photon black body's");
    assert!((ratio - 13.5).abs() < 0.05);
    let age = 13.7e9 * JULIAN_YEAR;
    let ending_now = (age * HBAR * C.powi(4) / (5120.0 * PI * G * G)).cbrt();
    println!(
        "ending its life now (13.7 billion years): {ending_now:.2e} kg in the textbook model; Carr et al. 5.1e11 kg"
    );
    let warm = HBAR * C.powi(3) / (8.0 * PI * G * K_B * CMB_TEMPERATURE);
    println!(
        "as warm as the cosmic background (2.725 K): {warm:.2e} kg, {:.2} Moons; heavier holes aren't evaporating today",
        warm / 7.346e22
    );
}
