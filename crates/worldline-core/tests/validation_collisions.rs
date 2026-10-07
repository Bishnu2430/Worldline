//! Validation: collisions merge bodies and keep total momentum.
//!
//! With Newtonian gravity, total momentum Σ m v is conserved exactly: the
//! bodies' pulls on each other are equal and opposite. So it must match
//! before and after a collision up to rounding, about 10⁻¹⁶ per step; over
//! the few hundred steps of these runs, 10⁻¹² leaves ample room.

use worldline_core::constants::{AU, GM_SUN, SOLAR_RADIUS};
use worldline_core::gravity::Newtonian;
use worldline_core::hierarchy::{Hierarchy, MoonSystem};
use worldline_core::{Body, DVec3, System};

/// Total momentum of the top level, Σ GM v (G drops out of comparisons),
/// and its scale, Σ |GM v|.
fn momentum(h: &Hierarchy) -> (DVec3, f64) {
    let bodies = &h.top.bodies;
    (
        bodies.iter().map(|b| b.velocity * b.gm).sum(),
        bodies.iter().map(|b| (b.velocity * b.gm).length()).sum(),
    )
}

/// Runs until the first collision, or panics after `limit` seconds.
fn until_collision(h: &mut Hierarchy, limit: f64) -> worldline_core::hierarchy::Collision {
    let end = h.time() + limit;
    while h.time() < end {
        h.step(end - h.time());
        if let Some(c) = h.take_collisions().into_iter().next() {
            return c;
        }
    }
    panic!("no collision within {limit} s");
}

/// 1 AU from the Sun, on a circular orbit, offset by `along` meters along
/// the orbit and moving `faster` m/s faster than the circle.
fn on_orbit(along: f64, faster: f64) -> (DVec3, DVec3) {
    let speed = (GM_SUN / AU).sqrt();
    (
        DVec3::new(AU, along, 0.0),
        DVec3::new(0.0, speed + faster, 0.0),
    )
}

fn sun() -> Body {
    Body::new("Sun", GM_SUN, SOLAR_RADIUS)
}

#[test]
fn two_planets_collide_merge_and_keep_their_momentum() {
    // Two Earth-like planets on the same orbit, the one behind catching up
    // at 10 km/s.
    let (pa, va) = on_orbit(-2e8, 1e4);
    let (pb, vb) = on_orbit(0.0, 0.0);
    let a = Body::new("A", 6e14, 7.0e6).at(pa).moving(va);
    let b = Body::new("B", 4e14, 6.0e6).at(pb).moving(vb);
    let mut h = Hierarchy::new(
        System::new(vec![sun(), a, b]),
        Box::new(Newtonian),
        Vec::new(),
    );
    let (before, scale) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    println!(
        "{} absorbed {} at {:.2} km/s after {:.1} h; momentum change {:.1e} of its scale",
        collision.survivor,
        collision.absorbed,
        collision.speed / 1e3,
        collision.time / 3600.0,
        (after - before).length() / scale
    );
    assert_eq!(
        (collision.survivor.as_str(), collision.absorbed.as_str()),
        ("A", "B")
    );
    assert_eq!(h.top.bodies.len(), 2);
    let merged = &h.top.bodies[1];
    assert_eq!(merged.gm, 1e15);
    // Volumes add: (7³ + 6³)^(1/3) = 8.50 thousand km.
    assert!((merged.radius - (7.0e6f64.powi(3) + 6.0e6f64.powi(3)).cbrt()).abs() < 1.0);
    assert!((after - before).length() <= 1e-12 * scale);
}

#[test]
fn a_comet_falling_into_the_sun_is_absorbed_at_the_surface() {
    // A massless comet 1 AU out falling straight at the Sun at 50 km/s.
    // Energy conservation sets its speed at the Sun's surface:
    // v² = v₀² + 2GM (1/R − 1/r₀), 618 km/s. Its own steps near the Sun
    // last about a sixth of the local dynamical time, over which cubic
    // interpolation is good to ~10⁻³; the Sun's relativistic term adds
    // ~10⁻⁶. The bound allows ten times the first.
    let earth = Body::new("Earth", 3.986e14, 6.371e6)
        .at(DVec3::new(-AU, 0.0, 0.0))
        .moving(DVec3::new(0.0, -(GM_SUN / AU).sqrt(), 0.0));
    let mut h = Hierarchy::new(
        System::new(vec![sun(), earth]),
        Box::new(Newtonian),
        Vec::new(),
    );
    let (r0, v0) = (AU, 5e4);
    h.add_followers(vec![(
        Body::new("Comet", 0.0, 5e3)
            .at(DVec3::new(0.0, r0, 0.0))
            .moving(DVec3::new(0.0, -v0, 0.0)),
        None,
    )]);
    let collision = until_collision(&mut h, 60.0 * 86_400.0);
    let expected = (v0 * v0 + 2.0 * GM_SUN * (1.0 / SOLAR_RADIUS - 1.0 / r0)).sqrt();
    println!(
        "{} hit the {} at {:.1} km/s (energy conservation: {:.1} km/s) after {:.2} days",
        collision.absorbed,
        collision.survivor,
        collision.speed / 1e3,
        expected / 1e3,
        collision.time / 86_400.0
    );
    assert_eq!(collision.survivor, "Sun");
    assert_eq!(h.follower_count(), 0);
    assert!((collision.speed / expected - 1.0).abs() < 1e-2);
    // And when: on a radial orbit with energy E > 0, r = a (cosh η − 1)
    // and t = √(a³/GM) (sinh η − η), with a = GM / 2E. The fall takes the
    // difference between η at 1 AU and at the Sun's surface.
    let energy = 0.5 * v0 * v0 - GM_SUN / r0;
    let a = GM_SUN / (2.0 * energy);
    let clock = |r: f64| {
        let eta = (1.0 + r / a).acosh();
        (a.powi(3) / GM_SUN).sqrt() * (eta.sinh() - eta)
    };
    let fall = clock(r0) - clock(SOLAR_RADIUS);
    println!(
        "falling time: {:.4} days (radial Kepler orbit: {:.4} days)",
        collision.time / 86_400.0,
        fall / 86_400.0
    );
    assert!((collision.time / fall - 1.0).abs() < 1e-2);
}

/// A planet with a moon 4 × 10⁵ km out, 1 AU from the Sun, and an impactor
/// of mass `gm` closing on it from behind at 10 km/s.
fn planet_with_moon_and_impactor(gm: f64) -> Hierarchy {
    let (pp, vp) = on_orbit(0.0, 0.0);
    let (planet_gm, moon_gm) = (4e14, 5e12);
    let host = Body::new("Planet", planet_gm + moon_gm, 6.4e6)
        .at(pp)
        .moving(vp);
    let orbit = 4e8;
    let moon_speed = (planet_gm / orbit).sqrt();
    let system = vec![
        Body::new("Planet", planet_gm, 6.4e6),
        Body::new("Moon", moon_gm, 1.7e6)
            .at(DVec3::new(orbit, 0.0, 0.0))
            .moving(DVec3::new(0.0, moon_speed, 0.0)),
    ];
    let (pi, vi) = on_orbit(-2e8, 1e4);
    let impactor = Body::new("Impactor", gm, 7e6).at(pi).moving(vi);
    Hierarchy::new(
        System::new(vec![sun(), host, impactor]),
        Box::new(Newtonian),
        vec![MoonSystem::new(1, system)],
    )
}

#[test]
fn a_planet_absorbed_leaves_its_moon_behind() {
    let mut h = planet_with_moon_and_impactor(4e15);
    let (before, scale) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    println!(
        "{} absorbed {}, freeing {:?}; momentum change {:.1e}",
        collision.survivor,
        collision.absorbed,
        collision.freed,
        (after - before).length() / scale
    );
    assert_eq!(collision.survivor, "Impactor");
    assert_eq!(collision.freed, ["Moon"]);
    assert!(h.moon_systems.is_empty());
    assert!(h.top.bodies.iter().any(|b| b.name == "Moon"));
    assert!((after - before).length() <= 1e-12 * scale);
}

#[test]
fn a_planet_that_survives_keeps_its_moon() {
    let mut h = planet_with_moon_and_impactor(4e13);
    let (before, scale) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    assert_eq!(collision.survivor, "Planet");
    assert_eq!(h.moon_systems.len(), 1);
    let moons = &h.moon_systems[0].system;
    let distance = (moons.bodies[1].position - moons.bodies[0].position).length();
    println!(
        "Planet absorbed the impactor; its moon is {:.0} km away; momentum change {:.1e}",
        distance / 1e3,
        (after - before).length() / scale
    );
    // The moon still circles the planet, about 4 × 10⁵ km out.
    assert!((distance / 4e8 - 1.0).abs() < 0.1);
    assert!((after - before).length() <= 1e-12 * scale);
}
