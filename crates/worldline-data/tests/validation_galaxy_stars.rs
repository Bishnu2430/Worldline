//! Step 7.2's check on the swarm the app uses: the 30,000 stars in
//! `data/galaxy-stars.csv`, left alone for a galactic year, keep McMillan's
//! (2017) density.

use worldline_core::constants::JULIAN_YEAR;
use worldline_core::galaxy_stars::{advance, density_fit, stellar_components};
use worldline_core::milky_way::{KILOPARSEC, mcmillan_2017, milky_way};
use worldline_data::galaxy_stars;

#[test]
#[ignore = "about ten seconds"]
fn left_alone_the_swarm_keeps_the_models_density() {
    // Its χ² against the model, as drawn and after one turn of a circular
    // orbit at the Sun's radius. The limit is what χ² exceeds by chance
    // once in a thousand times: the swarm is noisy, as any sample is, and
    // must be no noisier than that.
    let potential = milky_way();
    let components = stellar_components(&mcmillan_2017());
    let mut stars = galaxy_stars();
    let start = density_fit(&stars, &components);
    println!("{} stars as drawn: {start}", stars.len());
    assert!(start.passes());
    let r0 = 8.21 * KILOPARSEC;
    let year = std::f64::consts::TAU * r0 / potential.circular_speed(r0);
    assert!(advance(&mut stars, potential, &[], year).is_empty());
    let after = density_fit(&stars, &components);
    println!(
        "after {:.0} million years: {after}",
        year / JULIAN_YEAR / 1e6
    );
    worldline_report::margins(
        "galaxy-swarm-density",
        "The galaxy's 30,000 stars, left alone, keep McMillan's (2017) density",
    )
    .check(
        "as drawn: χ² against the model, over its bins",
        start.chi2,
        start.limit,
        format!(
            "χ² {:.1} over {} degrees of freedom (limit {:.1}: exceeded by chance once in a thousand)",
            start.chi2, start.dof, start.limit
        ),
    )
    .check(
        &format!("a galactic year on, {:.0} million years", year / JULIAN_YEAR / 1e6),
        after.chi2,
        after.limit,
        format!("χ² {:.1} over {} degrees of freedom", after.chi2, after.dof),
    )
    .save(worldline_report::here!());
    assert!(after.passes());
}

#[test]
fn the_swarm_has_each_component_in_proportion_to_its_mass() {
    let components = stellar_components(&mcmillan_2017());
    let stars = galaxy_stars();
    let total: f64 = components.iter().map(|c| c.mass()).sum();
    for (c, component) in components.iter().enumerate() {
        let count = stars.iter().filter(|s| s.population as usize == c).count();
        let wanted = stars.len() as f64 * component.mass() / total;
        println!("{}: {count} stars, {wanted:.1} by mass", component.name());
        // Drawn by rounding the running share: within one star.
        assert!((count as f64 - wanted).abs() <= 1.0);
    }
}
