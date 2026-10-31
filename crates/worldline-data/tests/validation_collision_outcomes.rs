//! Step 6.1's check: Leinhardt & Stewart's (2012) catastrophic-disruption
//! threshold and largest remnants, reproduced for their published cases.
//!
//! Run with `cargo test -p worldline-data --test validation_collision_outcomes
//! -- --nocapture` to see the numbers.

use std::f64::consts::PI;

use worldline_core::impact::{
    Impact, Material, Regime, combined_radius, equal_mass_threshold, largest_share, second_share,
};
use worldline_data::{
    ImpactRun, RUBBLE_PILE_DENSITY, RUBBLE_PILE_RADIUS, impact_simulations,
    selected_impact_simulations,
};

/// A simulated collision: the paper's 10-km target, and a projectile of its
/// density.
fn impact(run: &ImpactRun) -> Impact {
    let volume = 4.0 / 3.0 * PI * RUBBLE_PILE_RADIUS.powi(3);
    let target_mass = RUBBLE_PILE_DENSITY * volume;
    Impact {
        target_mass,
        projectile_mass: target_mass * run.mass_ratio,
        target_radius: RUBBLE_PILE_RADIUS,
        projectile_radius: RUBBLE_PILE_RADIUS * run.mass_ratio.cbrt(),
        speed: run.speed,
        impact_parameter: run.b,
    }
}

#[test]
fn the_papers_worked_example() {
    // Section VI: Q*_RD = 5.3 J/kg and V* = 6.5 m/s for two equal small
    // bodies, c* = 5 and μ̄ = 0.37; at 45°, Q′*_RD "rises by almost a factor
    // of 5" and V* = 14 m/s. The text gives their sizes as R_C1 ~ 2 km, from
    // radii of 1 km at rock's density; but these numbers need R_C1 = 2.52 km,
    // 2^(4/3) km, which is two 2-km bodies at 1000 kg/m³, so that is what's
    // used here.
    let radius = 2e3;
    let mass = 4.0 / 3.0 * PI * radius * radius * radius * 1000.0;
    let material = Material::SMALL_BODIES;
    let (energy, speed) = equal_mass_threshold(2.0 * mass, material);
    let r_c1 = combined_radius(2.0 * mass);
    let oblique = Impact {
        target_mass: mass,
        projectile_mass: mass,
        target_radius: radius,
        projectile_radius: radius,
        speed: 1.0,
        impact_parameter: std::f64::consts::FRAC_PI_4.sin(),
    };
    let raised = oblique.threshold(material) / energy;
    let oblique_speed = speed * raised.sqrt();
    println!(
        "R_C1 {:.0} m: Q*_RD {energy:.2} J/kg (5.3), V* {speed:.2} m/s (6.5); at 45° Q′*_RD × {raised:.2} (almost 5), V′* {oblique_speed:.1} m/s (14)",
        r_c1
    );
    assert!((energy - 5.3).abs() <= 0.05);
    assert!((speed - 6.5).abs() <= 0.05);
    assert!(raised > 4.5 && raised < 5.0);
    assert!((oblique_speed - 14.0).abs() <= 0.5);
}

#[test]
fn table_1s_derived_columns() {
    // Everything the paper works out from each simulation, to the digits it
    // prints. It predicts remnants from the Q′*_RD it fitted, printed to two
    // digits, so those can differ by that rounding carried through the law.
    let selected = selected_impact_simulations();
    let (mut worst_energy, mut worst_alpha, mut worst_largest, mut worst_second) =
        (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut largest_checked, mut unstated) = (0, 0);
    for row in &selected {
        let impact = impact(&row.run);
        // The paper's energies run a little above eq. 1 at its nominal mass
        // ratios: the 3-digit Table A.3 gives 66.5 J/kg for 40 m/s at γ =
        // 0.1, where eq. 1 gives 66.1. Its bodies are lumps of particles,
        // which may be why. Rounding to 2 digits is on top of that.
        let energy = (impact.energy() - row.run.energy.value).abs() / row.run.energy.value;
        worst_energy = worst_energy.max(energy);

        assert!(
            (impact.interacting_share() - row.alpha.value).abs() <= row.alpha.rounding + 1e-12,
            "α {} against {:?}, {:?}",
            impact.interacting_share(),
            row.alpha,
            row.run
        );
        worst_alpha = worst_alpha.max((impact.interacting_share() - row.alpha.value).abs());

        let x = impact.energy() / row.threshold.value;
        let carried = 0.5 * x * row.threshold.rounding / row.threshold.value;
        if row.mark == "supercat" {
            assert!(x > 1.8, "{x}");
            continue;
        }
        // Two equal remnants: the paper's footnote gives N_lr = 2 and
        // N_slr = 4, but not how it got these masses. See the doc.
        if row.mark == "d" {
            unstated += 1;
            continue;
        }
        // One row sits just past 1.8, where the paper's law gives way to
        // eq. 44; its Table 1 stays on the straight line there.
        let ours = if x < 1.8 {
            largest_share(x)
        } else {
            1.0 - 0.5 * x
        };
        let printed = row.predicted_largest.expect("a prediction");
        let off = (ours - printed.value).abs();
        assert!(
            off <= printed.rounding + carried,
            "{:?}: {ours} against {:?}",
            row.run,
            printed
        );
        worst_largest = worst_largest.max(off);
        largest_checked += 1;

        // The table rounds these twice: 0.0047 prints as 0.01, by way of
        // 0.005. So a printed 0.01 can stand for as little as 0.0045.
        let second = second_share(ours);
        let printed = row.predicted_second.expect("a prediction");
        let off = (second - printed.value).abs();
        assert!(
            off <= 1.1 * printed.rounding + carried * 0.15 / 5.7,
            "{:?}: {second} against {:?}",
            row.run,
            printed
        );
        worst_second = worst_second.max(off);
    }
    println!(
        "{} rows: Q_R within {:.1}%; α within {worst_alpha:.4}; {largest_checked} predicted remnants within {worst_largest:.4} (largest) and {worst_second:.4} (second); {unstated} rows of two equal remnants not reproducible",
        selected.len(),
        100.0 * worst_energy
    );
    assert_eq!(largest_checked, 39);
    assert_eq!(unstated, 8);
    // 2-digit rounding: 4.5% at most, for 1.1e2.
    assert!(worst_energy < 0.05);
}

#[test]
fn the_catastrophic_disruption_threshold() {
    // The full recipe, from nothing but the bodies, against the Q′*_RD fitted
    // to each series, for the paper's fit to these very simulations. The paper
    // fits only α > 0.5, and says its angle correction "usually" lands within
    // a factor of 2. It singles out equal masses with 0 < b < b_crit, where
    // the collision's spin lowers the threshold, and so that case is shown
    // but not counted.
    let selected = selected_impact_simulations();
    let (mut counted, mut within) = (0, 0);
    for row in selected.iter().step_by(3) {
        let impact = impact(&row.run);
        let ours = impact.threshold(Material::RUBBLE_PILES);
        let ratio = ours / row.threshold.value;
        let spinning = row.run.mass_ratio == 1.0
            && row.run.b > 0.0
            && row.run.b < impact.critical_impact_parameter();
        let fitted = impact.interacting_share() > 0.5 && !spinning;
        println!(
            "γ {:<5} b {:.2}  α {:.2}: Q′*_RD {:>9.0} J/kg, simulated {:>9.0}: × {ratio:.2}{}",
            row.run.mass_ratio,
            row.run.b,
            impact.interacting_share(),
            ours,
            row.threshold.value,
            if fitted {
                ""
            } else {
                "  (outside the paper's fit)"
            }
        );
        if fitted {
            counted += 1;
            within += usize::from((0.5..=2.0).contains(&ratio));
        }
    }
    println!("{within} of {counted} within a factor of 2");
    assert_eq!(counted, 8);
    // "Usually": all but one.
    assert!(within >= counted - 1, "{within} of {counted}");
}

#[test]
fn the_largest_remnants_of_every_simulation() {
    // Every simulation in Table A.3, from the recipe alone. The paper says
    // its law is good to "about 10%" for near-normal impacts and worse for
    // highly oblique ones, so the limit is 10% of the total mass for b ≤
    // 0.35, on the median.
    let runs = impact_simulations();
    let median = |mut offs: Vec<f64>| {
        offs.sort_by(f64::total_cmp);
        offs[offs.len() / 2]
    };
    let off = |run: &ImpactRun| {
        let impact = impact(run);
        let outcome = impact.outcome(Material::RUBBLE_PILES);
        (outcome.largest / impact.total_mass() - run.largest).abs()
    };
    let near_normal: Vec<f64> = runs.iter().filter(|r| r.b <= 0.35).map(off).collect();
    let oblique: Vec<f64> = runs.iter().filter(|r| r.b > 0.35).map(off).collect();
    let (normal, slanted) = (median(near_normal.clone()), median(oblique.clone()));
    let mut all: Vec<f64> = near_normal.iter().chain(&oblique).copied().collect();
    all.sort_by(f64::total_cmp);
    println!(
        "a tenth of them are off by {:.3} or more; the worst by {:.3}",
        all[9 * all.len() / 10],
        all[all.len() - 1]
    );
    println!(
        "{} simulations: median error in the largest remnant, as a share of the total mass, {normal:.3} ({} with b ≤ 0.35), {slanted:.3} ({} oblique)",
        runs.len(),
        near_normal.len(),
        oblique.len()
    );
    // How often the regime matches what the simulation did. Hit-and-run
    // leaves the target's mass, accretion more, erosion less.
    let (mut hits, mut grazing_hits, mut hit_and_runs) = (0, 0, 0);
    for run in &runs {
        let impact = impact(run);
        let outcome = impact.outcome(Material::RUBBLE_PILES);
        let target = 1.0 / (1.0 + run.mass_ratio);
        let accreted = run.largest > target;
        let hit = match outcome.regime {
            Regime::Merging | Regime::PartialAccretion => accreted,
            Regime::HitAndRun => (run.largest - target).abs() < 0.05,
            Regime::Erosion | Regime::SuperCatastrophic => !accreted,
        };
        hits += usize::from(hit);
        if outcome.regime == Regime::HitAndRun {
            hit_and_runs += 1;
            grazing_hits += usize::from(hit);
        }
    }
    println!(
        "regime matches the simulation in {hits} of {}; hit-and-run in {grazing_hits} of {hit_and_runs}",
        runs.len()
    );
    assert!(normal <= 0.10, "{normal}");
}

#[test]
fn hit_and_run_leaves_the_projectile_as_the_reverse_impact_says() {
    // Grazing a target, a projectile at a modest speed survives; much faster
    // it comes apart. The paper calls this estimate first-order, and sets no
    // figure for it, so the error is shown, with no limit but that it can't
    // come out heavier than it went in.
    let mut compared = 0;
    let mut offs: Vec<f64> = Vec::new();
    for run in impact_simulations() {
        let impact = impact(&run);
        let outcome = impact.outcome(Material::RUBBLE_PILES);
        if outcome.regime == Regime::HitAndRun {
            compared += 1;
            let total = impact.total_mass();
            offs.push((outcome.second / total - run.second).abs());
            assert!(outcome.second <= impact.projectile_mass);
        }
    }
    offs.sort_by(f64::total_cmp);
    println!(
        "{compared} hit-and-runs: median error in the second remnant, as a share of the total mass, {:.3}",
        offs[offs.len() / 2]
    );
    assert!(compared > 0);
}

#[test]
fn a_mars_hitting_an_earth_by_speed() {
    // The regimes follow each other as the speed rises, in the order the
    // paper's Appendix sets, for planets' constants: head-on-ish, accretion
    // comes between merging and erosion; grazing, hit-and-run does, and
    // super-catastrophic disruption would take 200 km/s, past the laws'
    // range.
    let order = |b: f64, expected: &[Regime]| {
        let mut seen: Vec<Regime> = Vec::new();
        println!("A Mars into an Earth at b = {b}:");
        for speed in [8e3, 12e3, 16e3, 24e3, 40e3, 60e3, 80e3] {
            let impact = Impact {
                target_mass: 5.972e24,
                projectile_mass: 6.417e23,
                target_radius: 6.371e6,
                projectile_radius: 3.39e6,
                speed,
                impact_parameter: b,
            };
            let outcome = impact.outcome(Material::PLANETS);
            println!(
                "  {:>2.0} km/s: {:?}, largest {:.3}, second {:.3} of the mass (merges below {:.1}, erodes above {:.1}, super-catastrophic above {:.1} km/s)",
                speed / 1e3,
                outcome.regime,
                outcome.largest / impact.total_mass(),
                outcome.second / impact.total_mass(),
                outcome.merging_speed / 1e3,
                outcome.erosion_speed / 1e3,
                outcome.super_catastrophic_speed / 1e3,
            );
            if seen.last() != Some(&outcome.regime) {
                seen.push(outcome.regime);
            }
        }
        assert_eq!(seen, expected);
    };
    order(
        0.3,
        &[
            Regime::Merging,
            Regime::PartialAccretion,
            Regime::Erosion,
            Regime::SuperCatastrophic,
        ],
    );
    order(0.8, &[Regime::Merging, Regime::HitAndRun, Regime::Erosion]);
}
