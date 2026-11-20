//! Step 7.2: the galaxy's stars as a swarm. Left alone, it keeps the
//! model's density; a black hole passing through scatters it as the impulse
//! approximation predicts.

use std::time::Instant;

use worldline_core::DVec3;
use worldline_core::constants::{GM_SUN, JULIAN_YEAR};
use worldline_core::galaxy_stars::{
    Perturber, Random, Recipe, Sampler, Star, advance, density_fit, equilibrium, stellar_components,
};
use worldline_core::milky_way::{KILOPARSEC, mcmillan_2017, milky_way};

#[test]
#[ignore = "about a minute"]
fn swarms_drawn_by_orbits_keep_the_models_density() {
    // The method that drew the app's swarm, afresh: 6,000 stars from 18,000
    // orbits. Its χ² against the model, then after a galactic year.
    let clock = Instant::now();
    let potential = milky_way();
    let components = stellar_components(&mcmillan_2017());
    let sampler = Sampler::new(potential, components.clone());
    let (mut stars, repeats) = equilibrium(&sampler, 6000, 3, Recipe::standard());
    let fit = density_fit(&stars, &components);
    println!(
        "drawn in {:.0} s, {repeats} from an orbit already used: {fit}",
        clock.elapsed().as_secs_f64()
    );
    assert!(fit.passes());
    let r0 = 8.21 * KILOPARSEC;
    let year = std::f64::consts::TAU * r0 / potential.circular_speed(r0);
    assert!(advance(&mut stars, potential, &[], year).is_empty());
    let fit = density_fit(&stars, &components);
    println!("after {:.0} million years: {fit}", year / JULIAN_YEAR / 1e6);
    assert!(fit.passes());
}

#[test]
#[ignore = "a few seconds"]
fn drawn_from_the_jeans_equations_alone_it_drifts() {
    // Why orbits are needed: stars drawn straight from the Jeans equations'
    // Gaussians start as the model, but drift from it within a galactic
    // year. This is the method's motivation, so it must fail the check.
    let potential = milky_way();
    let components = stellar_components(&mcmillan_2017());
    let sampler = Sampler::new(potential, components.clone());
    let mut stars = sampler.sample(20_000, 1);
    let start = density_fit(&stars, &components);
    let r0 = 8.21 * KILOPARSEC;
    let year = std::f64::consts::TAU * r0 / potential.circular_speed(r0);
    advance(&mut stars, potential, &[], year);
    let after = density_fit(&stars, &components);
    println!(
        "Jeans alone: drawn, {start}
   a galactic year on, {after}"
    );
    assert!(start.passes() && !after.passes());
}

/// A star turned about the galaxy's axis to the −x side, where the Sun is.
fn turn(star: Star) -> Star {
    let angle = std::f64::consts::PI - star.position.y.atan2(star.position.x);
    let (s, c) = angle.sin_cos();
    let rotate = |v: DVec3| DVec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z);
    Star {
        position: rotate(star.position),
        velocity: rotate(star.velocity),
        ..star
    }
}

/// The largest the galaxy's tide can be at `x`: the Frobenius norm of the
/// gradient of its pull there, by central differences over 10 pc, which
/// bounds the tidal tensor's largest eigenvalue, s⁻².
fn tide(x: DVec3) -> f64 {
    let potential = milky_way();
    let d = 1e-2 * KILOPARSEC;
    let mut sum = 0.0;
    for axis in [DVec3::X, DVec3::Y, DVec3::Z] {
        let g = (potential.acceleration(x + axis * d) - potential.acceleration(x - axis * d))
            / (2.0 * d);
        sum += g.length_squared();
    }
    sum.sqrt()
}

#[test]
fn a_black_hole_passing_through_scatters_it_as_the_impulse_approximation_predicts() {
    // A million-Sun black hole falls straight through the disk at 1,000
    // km/s, through a patch of the swarm at the Sun's radius: thin-disk
    // stars drawn from the model, kept if within 30 pc of the patch's
    // middle. It's aimed where the patch will be when it crosses the plane,
    // as the disk turns.
    let potential = milky_way();
    let sampler = Sampler::new(potential, stellar_components(&mcmillan_2017()));
    let center = DVec3::new(-8.21 * KILOPARSEC, 0.0, 0.0);
    let patch = 30e-3 * KILOPARSEC;
    let mut random = Random::new(2);
    let mut stars = Vec::new();
    while stars.len() < 300 {
        let star = turn(sampler.draw(0, &mut random));
        if (star.position - center).length() < patch {
            stars.push(star);
        }
    }
    let drift = stars.iter().map(|s| s.velocity).sum::<DVec3>() / stars.len() as f64;
    let gm = 1e6 * GM_SUN;
    let speed = 1000e3;
    let reach = 0.3 * KILOPARSEC;
    let crossing = reach / speed;
    let velocity = DVec3::new(0.0, 0.0, -speed);
    let aim = center + DVec3::new(drift.x, drift.y, 0.0) * crossing;
    let start = aim + DVec3::new(0.0, 0.0, reach);
    let hole_at = |t: f64| start + velocity * t;
    let tidal = tide(center);
    let (mut worst, mut largest, mut most_allowed) = (0.0f64, 0.0f64, 0.0f64);
    let mut kicks = Vec::new();
    for star in &stars {
        // When it passes closest, on straight lines, and the undisturbed
        // star's state then.
        let (p, u) = (start - star.position, velocity - star.velocity);
        let closest = -p.dot(u) / u.length_squared();
        let mut calm = *star;
        advance(std::slice::from_mut(&mut calm), potential, &[], closest);
        // The approximation, from then: the path from the start to a moment
        // five crossing times later, when the kick is measured.
        let (p, u) = (hole_at(closest) - calm.position, velocity - calm.velocity);
        let refine = -p.dot(u) / u.length_squared();
        let offset = p + u * refine;
        let b = offset.length();
        let relative = u.length();
        let wait = 5.0 * b / relative;
        let (before, after) = (relative * (closest + refine), relative * wait);
        // Across the path, toward it; and along it, since the kick is
        // measured before the hole has gone far enough for that part to
        // cancel: (GM/bV)(b/√(Z₁² + b²) − b/√(Z₂² + b²)) (Binney & Tremaine
        // 2008, §8.2, for a path from −Z₁ to +Z₂).
        let across = offset / b
            * (gm / (b * relative) * (after / after.hypot(b) + before / before.hypot(b)));
        let along =
            u / relative * (gm / (b * relative) * (b / before.hypot(b) - b / after.hypot(b)));
        let predicted = across + along;
        // The same star, disturbed and not, to that moment.
        let end = closest + refine + wait;
        let hole = Perturber {
            gm,
            radius: 0.0,
            start: (start, velocity),
            end: (hole_at(end), velocity),
        };
        let (mut kicked, mut calm) = (*star, *star);
        advance(std::slice::from_mut(&mut kicked), potential, &[hole], end);
        advance(std::slice::from_mut(&mut calm), potential, &[], end);
        let kick = kicked.velocity - calm.velocity;
        // What the approximation leaves out: how far the star is pulled off
        // its straight path while the hole passes, the kick over the
        // relative speed; and the galaxy's tide on the kick over the wait,
        // ½|T|τ². Twice that is allowed.
        let allowed = 2.0 * (predicted.length() / relative + 0.5 * tidal * wait * wait);
        let off = (kick - predicted).length() / predicted.length();
        kicks.push((predicted.length() / 1e3, off / allowed));
        worst = worst.max(off / allowed);
        largest = largest.max(predicted.length());
        most_allowed = most_allowed.max(allowed);
    }
    println!(
        "{} stars, kicks up to {:.2} km/s: off the prediction by at most {:.2} of what's allowed (which is at most {:.1e} of the kick)",
        stars.len(),
        largest / 1e3,
        worst,
        most_allowed
    );
    worldline_report::lines(
        "galaxy-swarm-flyby",
        "A million-Sun black hole falls through the galaxy's swarm at 1,000 km/s: each star's kick",
        "predicted kick (km/s)",
        "off the impulse approximation, as a share of what's allowed",
    )
    .log_x()
    .dots("300 stars near the Sun's radius", kicks)
    .expected("limit", [(1e-3, 1.0), (largest / 1e3 * 2.0, 1.0)])
    .note(&format!(
        "Allowed: twice what the approximation leaves out, the kick over the relative speed and the galaxy's tide \
         over the wait. The worst is {worst:.2} of it; kicks up to {:.2} km/s.",
        largest / 1e3
    ))
    .save(worldline_report::here!());
    assert!(worst < 1.0);
}
