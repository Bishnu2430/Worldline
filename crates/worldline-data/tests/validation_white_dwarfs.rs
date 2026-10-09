//! Validation: white dwarfs held up by degenerate electrons, and
//! Chandrasekhar's limit.
//!
//! Cold, ideal carbon–oxygen white dwarfs (two atomic mass units per
//! electron), solved from Chandrasekhar's exact equation of state for every
//! central density. The roadmap's check: the most they can weigh is
//! 1.46 Suns. Then both ends of the mass–radius relation against theory,
//! and Sirius B, the white dwarf whose mass and radius are best measured.
//!
//! `cargo test --release -p worldline-data --test validation_white_dwarfs -- --nocapture`

use std::f64::consts::PI;

use worldline_core::constants::{
    ATOMIC_MASS_UNIT, C, ELECTRON_MASS, G, GM_SUN, HBAR, SOLAR_RADIUS,
};
use worldline_core::white_dwarf::{
    CARBON_OXYGEN, chandrasekhar_limit, lane_emden, radius_for, white_dwarf,
};
use worldline_data::notable_object;

/// The density at which the electrons' Fermi momentum is mₑc, kg/m³:
/// central densities are given as ρ₀ x³, x being that momentum over mₑc.
fn rho0() -> f64 {
    let lambda = HBAR / (ELECTRON_MASS * C);
    CARBON_OXYGEN * ATOMIC_MASS_UNIT / (3.0 * PI * PI * lambda.powi(3))
}

#[test]
fn no_white_dwarf_passes_chandrasekhars_limit() {
    let limit = chandrasekhar_limit(CARBON_OXYGEN);
    println!(
        "Chandrasekhar's limit, carbon and oxygen: {:.4} Suns (the roadmap: 1.46)",
        limit / GM_SUN
    );
    // The roadmap's number, to the digits it gives.
    assert!((limit / GM_SUN - 1.46).abs() < 0.005);

    // Ever denser white dwarfs, with their electrons ever more
    // relativistic: the mass rises toward the limit and never reaches it.
    // As x grows the pressure tends to that of an n = 3 polytrope with
    // corrections of order 1/x², so the mass falls short by about 1/x²; a
    // wrong equation of state wouldn't approach the limit at all. The test
    // asks the shortfall to fall faster than 1/x, the midpoint.
    let mut previous: Option<(f64, f64)> = None;
    for x in [1.0, 10.0, 100.0, 1000.0] {
        let star = white_dwarf(rho0() * x * x * x, CARBON_OXYGEN);
        let short = 1.0 - star.gm / limit;
        println!(
            "x = {x:>6}: central density {:.2e} kg/m³, {:.5} Suns, {:.0} km; {short:.2e} short of the limit",
            star.central_density,
            star.gm / GM_SUN,
            star.radius / 1e3
        );
        assert!(short > 0.0);
        if let Some((px, pshort)) = previous {
            let power = (pshort / short).ln() / (x / px).ln();
            println!("  the shortfall falls as x^−{power:.2}");
            if x > 10.0 {
                assert!(power > 1.0);
            }
        }
        previous = Some((x, short));
    }
}

#[test]
fn light_white_dwarfs_are_polytropes_of_index_three_halves() {
    // When the electrons are far from relativistic (x ≪ 1), the pressure
    // is P = K ρ^(5/3) with K = (3π²)^(2/3) ħ²/(5 mₑ (μₑ mᵤ)^(5/3)), and the
    // star an n = 3/2 polytrope: M R³ = 4π ω ξ₁³ (5K/(8πG))³ for every
    // mass, with ξ₁ and ω its Lane–Emden constants. Corrections are of
    // order x²; a mistake would be of order 1. Allowed: the midpoint, x.
    let (xi, omega) = lane_emden(1.5);
    let k = (3.0 * PI * PI).powf(2.0 / 3.0) * HBAR * HBAR
        / (5.0 * ELECTRON_MASS * (CARBON_OXYGEN * ATOMIC_MASS_UNIT).powf(5.0 / 3.0));
    let expected = 4.0 * PI * omega * xi.powi(3) * (5.0 * k / (8.0 * PI * G)).powi(3);
    for x in [0.03, 0.01] {
        let star = white_dwarf(rho0() * x * x * x, CARBON_OXYGEN);
        let mr3 = star.gm / G * star.radius.powi(3);
        let miss = (mr3 / expected - 1.0).abs();
        println!(
            "x = {x}: {:.4} Suns, {:.0} km; M R³ off the polytrope's by {miss:.1e} ({:.1} x²)",
            star.gm / GM_SUN,
            star.radius / 1e3,
            miss / (x * x)
        );
        assert!(miss < x);
    }
}

#[test]
fn sirius_b_is_the_size_theory_gives_its_mass() {
    // Sirius B: 1.018 ± 0.011 Suns from its orbit (Bond et al. 2017),
    // 0.00803 ± 0.00011 Sun radii from its light and distance (Joyce et
    // al. 2018). The ideal model leaves out its heat (25,000 K at the
    // surface), which puffs it up, and the electrons' attraction to the
    // nuclei, which shrinks it, each by of order 1%. It must match within
    // the measurements' combined uncertainty: the radius's, and the mass's
    // carried through the mass–radius relation.
    let sirius = notable_object("Sirius B").expect("in the catalog");
    let mass = sirius.mass.value;
    let (mass_up, _) = sirius.mass.plus_minus.expect("an uncertainty");
    let (radius_up, _) = sirius.radius.plus_minus.expect("an uncertainty");
    let radius = sirius.radius.value;
    let predicted = radius_for(mass * GM_SUN, CARBON_OXYGEN).unwrap();
    let slope = (radius_for((mass + 0.001) * GM_SUN, CARBON_OXYGEN).unwrap() - predicted) / 0.001;
    let sigma = radius_up.hypot(slope * mass_up);
    println!(
        "Sirius B: an ideal {mass}-Sun white dwarf is {:.5} Sun radii ({:.0} km); measured {:.5} ± {:.5} ({:.0} km); off by {:.2} σ",
        predicted / SOLAR_RADIUS,
        predicted / 1e3,
        radius / SOLAR_RADIUS,
        radius_up / SOLAR_RADIUS,
        radius / 1e3,
        (predicted - radius).abs() / sigma
    );
    assert!((predicted - radius).abs() < sigma);
    println!("The mass–radius relation (carbon and oxygen, cold, ideal):");
    for m in [0.2, 0.4, 0.6, 0.8, 1.0, 1.2, 1.3, 1.4, 1.45] {
        let r = radius_for(m * GM_SUN, CARBON_OXYGEN).unwrap();
        println!(
            "  {m:>4} Suns: {:>6.0} km ({:.5} Sun radii)",
            r / 1e3,
            r / SOLAR_RADIUS
        );
    }
}
