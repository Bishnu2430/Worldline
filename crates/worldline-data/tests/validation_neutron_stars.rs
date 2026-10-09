//! Validation: neutron stars from the SLy equation of state and general
//! relativity's equations for a static star.
//!
//! The roadmap's check: SLy holds up at most about 2.05 Suns. Then SLy's
//! stars against Read et al.'s published values, against the neutron stars
//! NICER and radio timing have measured, and the mass above which two
//! merging neutron stars collapse at once into a black hole (Bauswein,
//! Baumgarte & Janka 2013), checked against their own simulations, with
//! GW170817.
//!
//! `cargo test --release -p worldline-data --test validation_neutron_stars -- --nocapture`

use worldline_core::constants::GM_SUN;
use worldline_core::neutron_star::{
    PiecewisePolytrope, maximum_mass, prompt_collapse_threshold, radius_for,
};
use worldline_data::notable_object;

#[test]
fn sly_holds_up_at_most_about_2_05_suns() {
    let sly = PiecewisePolytrope::sly();
    let heaviest = maximum_mass(&sly);
    let mass = heaviest.gm / GM_SUN;
    let r14 = radius_for(&sly, 1.4 * GM_SUN).unwrap() / 1e3;
    println!(
        "SLy: heaviest {mass:.4} Suns, radius {:.3} km, central density {:.3e} kg/m³; a 1.4-Sun star is {r14:.3} km",
        heaviest.radius / 1e3,
        heaviest.central_density
    );
    // The roadmap's number, to its digits.
    assert!((mass - 2.05).abs() < 0.005);
    // Read et al. (2009, Table III), for this same fit: 2.049 Suns × 1.0002
    // and 11.736 km × (1 − 0.0021). They computed with the `rns` code's
    // own values of G and the Sun's mass, older than today's by up to
    // 10⁻³ each (the Sun's mass taken anywhere from 1.987 to 1.989 × 10³³
    // g); the comparison allows 0.2%.
    let (read_mass, read_r14) = (2.049 * 1.0002, 11.736 * (1.0 - 0.0021));
    println!(
        "Read et al.'s fit: {read_mass:.4} Suns (off by {:.2}%), {read_r14:.3} km (off by {:.2}%)",
        100.0 * (mass / read_mass - 1.0),
        100.0 * (r14 / read_r14 - 1.0)
    );
    assert!((mass / read_mass - 1.0).abs() < 2e-3);
    assert!((r14 / read_r14 - 1.0).abs() < 2e-3);
}

#[test]
fn sly_agrees_with_the_measured_neutron_stars() {
    let sly = PiecewisePolytrope::sly();
    let heaviest = maximum_mass(&sly).gm / GM_SUN;
    // The heaviest well-measured neutron star, PSR J0740+6620: 2.08 ± 0.07
    // Suns (Fonseca et al. 2021). An equation of state must hold it up:
    // SLy's maximum lies within its uncertainty.
    let j0740 = notable_object("PSR J0740+6620").expect("in the catalog");
    let (_, below) = j0740.mass.plus_minus.expect("an uncertainty");
    println!(
        "PSR J0740+6620: {} − {below} Suns; SLy holds up to {heaviest:.3}",
        j0740.mass.value
    );
    assert!(heaviest >= j0740.mass.value - below);
    // PSR J0030+0451, 1.34 Suns, 12.71 +1.14/−1.19 km (Riley et al. 2019,
    // NICER): SLy's star of that mass must lie within the radius's range.
    let j0030 = notable_object("PSR J0030+0451").expect("in the catalog");
    let (up, down) = j0030.radius.plus_minus.expect("an uncertainty");
    let r = radius_for(&sly, j0030.mass.value * GM_SUN).unwrap();
    println!(
        "PSR J0030+0451: {} Suns, {:.2} +{:.2}/−{:.2} km measured; SLy: {:.2} km",
        j0030.mass.value,
        j0030.radius.value / 1e3,
        up / 1e3,
        down / 1e3,
        r / 1e3
    );
    assert!(r >= j0030.radius.value - down && r <= j0030.radius.value + up);
    // For the record: NICER's radius for J0740+6620, 12.39 +1.30/−0.98 km
    // (Riley et al. 2021), against SLy's heaviest star, ~10 km. SLy is on
    // the soft side of what NICER finds; the app says which model it is.
    println!(
        "PSR J0740+6620's radius: {:.2} km measured; SLy's heaviest star: {:.2} km",
        j0740.radius.value / 1e3,
        maximum_mass(&sly).radius / 1e3
    );
}

#[test]
fn merging_neutron_stars_collapse_at_once_above_the_threshold() {
    // Bauswein, Baumgarte & Janka (2013), Table I: for 12 equations of
    // state, the heaviest static star, the radius of a 1.6-Sun star, and the
    // total mass above which their merger simulations collapsed at once.
    // Their fit, k = M_thres/M_max = −3.606 G M_max/(c² R₁.₆) + 2.380, misses
    // their k by at most 0.025; here it must too, allowing for the table's
    // rounding of M_thres and M_max (± 0.005 Suns each).
    let table: [(&str, f64, f64, f64); 12] = [
        ("NL3", 2.79, 14.81, 3.85),
        ("GS1", 2.75, 14.79, 3.85),
        ("LS375", 2.71, 13.71, 3.65),
        ("DD2", 2.42, 13.26, 3.35),
        ("Shen", 2.22, 14.46, 3.45),
        ("TM1", 2.21, 14.36, 3.45),
        ("SFHX", 2.13, 11.98, 3.05),
        ("GS2", 2.09, 13.31, 3.25),
        ("SFHO", 2.06, 11.76, 2.95),
        ("LS220", 2.04, 12.43, 3.05),
        ("TMA", 2.02, 13.73, 3.25),
        ("IUF", 1.95, 12.57, 3.05),
    ];
    let mut worst: f64 = 0.0;
    for (name, m_max, r16, m_thres) in table {
        let fit = prompt_collapse_threshold(m_max * GM_SUN, r16 * 1e3) / GM_SUN;
        let k_miss = ((fit - m_thres) / m_max).abs();
        // M_thres ± 0.005 and M_max ± 0.005 move k by up to
        // 0.005 (1 + k)/M_max.
        let rounding = 0.005 * (1.0 + m_thres / m_max) / m_max;
        println!("{name:>6}: threshold {m_thres} Suns, the fit {fit:.3} (k off by {k_miss:.3})");
        assert!(k_miss <= 0.025 + rounding);
        worst = worst.max(k_miss);
    }
    println!("largest miss in k: {worst:.3} (theirs: 0.025)");

    // SLy's threshold, and GW170817: 2.73 Suns in all (1.46 + 1.27), whose
    // light (a kilonova) showed it didn't collapse at once.
    let sly = PiecewisePolytrope::sly();
    let heaviest = maximum_mass(&sly);
    let r16 = radius_for(&sly, 1.6 * GM_SUN).unwrap();
    let threshold = prompt_collapse_threshold(heaviest.gm, r16) / GM_SUN;
    let gw170817 = 1.46 + 1.27;
    println!(
        "SLy: R₁.₆ = {:.2} km, threshold {threshold:.2} Suns; GW170817's {gw170817} Suns: {}",
        r16 / 1e3,
        if gw170817 < threshold {
            "no prompt collapse"
        } else {
            "prompt collapse"
        }
    );
    assert!(gw170817 < threshold);
}
