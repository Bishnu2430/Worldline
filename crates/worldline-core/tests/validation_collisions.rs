//! Validation: collisions merge bodies and keep total momentum.
//!
//! With Newtonian gravity, total momentum Σ m v is conserved exactly: the
//! bodies' pulls on each other are equal and opposite. So it must match
//! before and after a collision up to rounding, about 10⁻¹⁶ per step; over
//! the few hundred steps of these runs, 10⁻¹² leaves ample room.

use worldline_core::compact::{is_black_hole, schwarzschild_radius};
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
    // at 2 km/s: gentle enough to meet below their mutual escape speed, so
    // they truly merge.
    let (pa, va) = on_orbit(-2e8, 2e3);
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
    // An impactor ten times the planet's mass. As it closes in, its tide
    // on the moon passes 1/12 of the planet's pull, and the moon is freed
    // to orbit on its own (see `Unbound`); then the impactor absorbs the
    // planet. The moon survives.
    let mut h = planet_with_moon_and_impactor(4e15);
    let (before, scale) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    let unbound = h.take_unbound();
    println!(
        "{}'s moons {:?} freed by {}; then {} absorbed {}; momentum change {:.1e}",
        unbound[0].planet,
        unbound[0].freed,
        unbound[0].by,
        collision.survivor,
        collision.absorbed,
        (after - before).length() / scale
    );
    assert_eq!(unbound[0].freed, ["Moon"]);
    assert_eq!(unbound[0].by, "Impactor");
    assert_eq!(collision.survivor, "Impactor");
    assert!(h.moon_systems.is_empty());
    assert!(h.top.bodies.iter().any(|b| b.name == "Moon"));
    assert!((after - before).length() <= 1e-12 * scale);
}

#[test]
fn a_planet_that_survives_keeps_its_moon() {
    // An impactor of a hundred-thousandth of the planet's mass: even at
    // contact its tide on the moon is under 1% of the planet's pull, so the
    // moon system stays whole and the planet absorbs it.
    let mut h = planet_with_moon_and_impactor(4e9);
    let (before, scale) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    assert_eq!(collision.survivor, "Planet");
    assert!(h.take_unbound().is_empty());
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

#[test]
fn a_black_hole_that_hits_the_sun_takes_its_place() {
    // A black hole of 10 Suns let go 0.1 AU from the Sun, with a planet and
    // its moon 1 AU out. A black hole survives any collision: the merged
    // body is a black hole of 11 Suns, its horizon 2GM/c² = 32.5 km, and it
    // takes the Sun's place as body 0. Passing the planet 1 AU away, the
    // hole's tide on the moon is 0.13 of the planet's pull, past 1/12: the
    // moon is freed before the hole reaches the Sun.
    let mut h = planet_with_moon_and_impactor(0.0);
    let gm = 10.0 * GM_SUN;
    h.top.bodies[2] =
        Body::new("Hole", gm, schwarzschild_radius(gm)).at(DVec3::new(0.0, 0.1 * AU, 0.0));
    h.add_followers(vec![(
        Body::new("Comet", 0.0, 5e3).at(DVec3::new(0.0, -3.0 * AU, 0.0)),
        None,
    )]);
    let (before, _) = momentum(&h);
    let collision = until_collision(&mut h, 10.0 * 86_400.0);
    let (after, _) = momentum(&h);
    // The bodies start at rest, so the scale is the momentum each carries
    // into the impact: GM v of the Sun at its share of the relative speed,
    // 10/11 of it.
    let scale = GM_SUN * collision.speed * 10.0 / 11.0;
    let hole = &h.top.bodies[0];
    println!(
        "{} absorbed the {} at {:.0} km/s after {:.1} h; now {:.1} Suns with a {:.2} km horizon; momentum change {:.1e}",
        collision.survivor,
        collision.absorbed,
        collision.speed / 1e3,
        collision.time / 3600.0,
        hole.gm / GM_SUN,
        hole.radius / 1e3,
        (after - before).length() / scale
    );
    assert_eq!(
        (collision.survivor.as_str(), collision.absorbed.as_str()),
        ("Hole", "Sun")
    );
    assert_eq!(hole.name, "Hole");
    assert!(is_black_hole(hole));
    let expected = schwarzschild_radius(11.0 * GM_SUN);
    assert!((hole.radius - expected).abs() < 1e-12 * expected);
    assert_eq!(
        h.top
            .bodies
            .iter()
            .map(|b| b.name.as_str())
            .collect::<Vec<_>>(),
        ["Hole", "Planet", "Moon"]
    );
    let unbound = h.take_unbound();
    assert_eq!(
        (unbound[0].by.as_str(), unbound[0].planet.as_str()),
        ("Hole", "Planet")
    );
    assert!((after - before).length() <= 1e-12 * scale);
    // Everything goes on: the comet now falls toward the hole.
    h.advance(86_400.0);
    assert_eq!(h.follower_count(), 1);
}

#[test]
fn an_impact_is_measured_where_the_bodies_touch() {
    // A Mars sent past an Earth, aimed 8,000 km off center from 50,000 km.
    // Gravity bends it inward, so it hits, more nearly head-on than it was
    // aimed. Newton fixes both numbers at contact: its energy gives the
    // speed there, and its angular momentum, unchanged, gives the impact
    // parameter: b = (aim × v₀) / (R_sum × v). The interpolation across the
    // step has to deliver them as finely as the scaling laws can use them:
    // the paper's tables give b to ±0.005, and speeds to two digits.
    use worldline_core::constants::G;
    use worldline_core::impact::Regime;
    let (gm_earth, gm_mars) = (3.986e14, 4.283e13);
    let (r_earth, r_mars) = (6.371e6, 3.39e6);
    let (reach, gm) = (r_earth + r_mars, gm_earth + gm_mars);
    let (distance, aim, speed0): (f64, f64, f64) = (5e7, 8e6, 2e3);
    let start = (distance * distance + aim * aim).sqrt();
    let top = System::new(vec![
        Body::new("Earth", gm_earth, r_earth),
        Body::new("Mars", gm_mars, r_mars)
            .at(DVec3::new(-distance, aim, 0.0))
            .moving(DVec3::new(speed0, 0.0, 0.0)),
    ]);
    let mut h = Hierarchy::new(top, Box::new(Newtonian), Vec::new());
    let collision = until_collision(&mut h, 1e6);
    let speed = (speed0 * speed0 + 2.0 * gm * (1.0 / reach - 1.0 / start)).sqrt();
    let b = aim * speed0 / (reach * speed);
    let (impact, outcome) = collision.outcome.expect("the scaling laws cover it");
    println!(
        "{:.3} km/s ({:.3}), b {:.5} ({b:.5}): {:?}, largest remnant {:.4} of the mass, Q_R {:.3e} of Q′*_RD {:.3e} J/kg",
        collision.speed / 1e3,
        speed / 1e3,
        impact.impact_parameter,
        outcome.regime,
        outcome.largest / impact.total_mass(),
        outcome.energy,
        outcome.threshold,
    );
    assert!((collision.speed / speed - 1.0).abs() < 0.005);
    assert!((impact.impact_parameter - b).abs() < 0.005);
    assert_eq!(impact.target_mass, gm_earth / G);
    // It arrives just above the escape speed of the two: a slow, glancing
    // blow between planets, so it stays together.
    assert!(matches!(
        outcome.regime,
        Regime::Merging | Regime::PartialAccretion | Regime::HitAndRun
    ));
}

/// What a run holds: the top level's bodies and the fragments in the swarm,
/// as (GM, position, velocity).
fn everything(h: &mut Hierarchy) -> Vec<(f64, DVec3, DVec3)> {
    h.sync_swarm();
    let bodies = h.top.bodies.iter().map(|b| (b.gm, b.position, b.velocity));
    let debris = h.debris();
    let fragments = h
        .swarm()
        .particles
        .iter()
        .map(|p| (debris[&p.id], p.position, p.velocity));
    bodies.chain(fragments).collect()
}

/// A Mars sent into an Earth, in a Newtonian run, as it was the moment
/// before they touch: `b` is the impact parameter, `speed` the relative speed
/// at that moment, 300 km apart.
fn a_mars_into_an_earth(speed: f64, b: f64) -> Hierarchy {
    use worldline_core::gravity::Newtonian;
    let (r_earth, r_mars) = (6.371e6, 3.39e6);
    let (sin, cos) = (b, (1.0 - b * b).sqrt());
    let reach = (r_earth + r_mars) * 1.0003;
    let top = System::new(vec![
        Body::new("Earth", 3.986e14, r_earth),
        Body::new("Mars", 4.283e13, r_mars)
            .at(DVec3::new(-cos, sin, 0.0) * reach)
            .moving(DVec3::new(speed, 0.0, 0.0)),
    ]);
    Hierarchy::new(top, Box::new(Newtonian), Vec::new())
}

#[test]
fn a_collision_keeps_its_mass_momentum_and_energy() {
    // Step 6.2's check, through the whole simulation. Everything is
    // worked out again from what's really there before and after: the
    // bodies and the swarm's particles, not the model's own figures. To
    // rounding: 130 terms, each good to one part in 2⁵².
    use worldline_core::constants::G;
    use worldline_core::impact::Regime;
    let rounding = 130.0 * f64::EPSILON;
    let mut seen = Vec::new();
    for (speed, b) in [
        (9e3, 0.3),
        (14e3, 0.3),
        (40e3, 0.3),
        (75e3, 0.3),
        (16e3, 0.8),
    ] {
        let mut h = a_mars_into_an_earth(speed, b);
        let before = everything(&mut h);
        let collision = until_collision(&mut h, 1e4);
        let after = everything(&mut h);
        let mass = |all: &[(f64, DVec3, DVec3)]| all.iter().map(|x| x.0).sum::<f64>();
        let momentum = |all: &[(f64, DVec3, DVec3)]| all.iter().map(|x| x.2 * x.0).sum::<DVec3>();
        let scale: f64 = before.iter().map(|x| (x.2 * x.0).length()).sum();
        let left = collision.aftermath.expect("the laws cover it");
        let outcome = collision.outcome.expect("the laws cover it").1;
        println!(
            "{:>2.0} km/s, b {b}: {:?}, {} bodies and {} fragments; mass off by {:.1e}, momentum by {:.1e}; heat {:.3e} J of {:.3e} J of motion",
            speed / 1e3,
            outcome.regime,
            h.top.bodies.len(),
            left.fragments.len(),
            (mass(&after) / mass(&before) - 1.0).abs(),
            (momentum(&after) - momentum(&before)).length() / scale,
            left.heat,
            left.before.kinetic,
        );
        assert!((mass(&after) / mass(&before) - 1.0).abs() <= rounding);
        assert!((momentum(&after) - momentum(&before)).length() <= rounding * scale);

        // The energy, from what's there. The frame is the center of mass's,
        // which hasn't moved, as the bodies are bound by nothing else.
        let total = mass(&before);
        let drift = momentum(&before) / total;
        let kinetic: f64 = left
            .fragments
            .iter()
            .map(|f| (f.gm, f.velocity))
            .chain(
                [Some(&left.largest), left.second.as_ref()]
                    .into_iter()
                    .flatten()
                    .map(|b| (b.gm, b.velocity)),
            )
            .map(|(gm, v)| 0.5 * gm * (v - drift).length_squared() / G)
            .sum();
        let scale =
            left.before.kinetic.abs() + left.before.potential.abs() + left.before.binding.abs();
        // The kinetic energy of what's flying is what the accounts say.
        println!(
            "   kinetic off by {:.1e} of the energy's scale",
            (kinetic - left.after.kinetic).abs() / scale
        );
        assert!(
            (kinetic - left.after.kinetic).abs() <= rounding * scale,
            "{kinetic}"
        );
        assert!(left.heat >= 0.0, "{}", left.heat);
        let closes = left.before.total() - left.after.total() - left.heat;
        assert!(closes.abs() <= rounding * scale);
        seen.push(outcome.regime);
    }
    assert_eq!(
        seen,
        [
            Regime::Merging,
            Regime::PartialAccretion,
            Regime::Erosion,
            Regime::SuperCatastrophic,
            Regime::HitAndRun
        ]
    );
}

#[test]
fn a_hit_and_run_leaves_both_going_their_ways() {
    let mut h = a_mars_into_an_earth(16e3, 0.8);
    let collision = until_collision(&mut h, 1e4);
    assert!(collision.parted());
    assert_eq!(h.top.bodies.len(), 2);
    assert_eq!(h.top.bodies[1].name, "Mars");
    // They separate, and don't merge on the next step.
    let apart = |h: &Hierarchy| (h.top.bodies[0].position - h.top.bodies[1].position).length();
    let start = apart(&h);
    for _ in 0..20 {
        h.step(60.0);
    }
    assert!(h.take_collisions().is_empty());
    assert!(apart(&h) > start, "{start}");
}

#[test]
fn a_slow_hit_and_run_comes_back_and_merges() {
    // Two Earths that only just have the speed to part: the velocity a
    // hit-and-run loses leaves them bound, under their mutual escape speed
    // of 11.2 km/s. They fall back together, as the paper says hit-and-runs
    // make merging likelier, and however often they meet, each time slower,
    // they end as one.
    use worldline_core::gravity::Newtonian;
    let (gm, radius, b) = (3.986e14, 6.371e6, 0.8f64);
    let at = DVec3::new(-(1.0 - b * b).sqrt(), b, 0.0) * (2.0 * radius * 1.0003);
    let top = System::new(vec![
        Body::new("Earth", gm, radius),
        Body::new("Second", gm, radius)
            .at(at)
            .moving(DVec3::X * 11.9e3),
    ]);
    let mut h = Hierarchy::new(top, Box::new(Newtonian), Vec::new());
    let first = until_collision(&mut h, 1e4);
    assert!(first.parted(), "{:?}", first.outcome);
    let started = h.time();
    let mut speeds = vec![first.speed];
    let last = loop {
        let next = until_collision(&mut h, 3e7);
        speeds.push(next.speed);
        if !next.parted() {
            break next;
        }
        assert!(speeds.len() < 20, "{speeds:?}");
    };
    println!(
        "met {} times, at {speeds:.0?} m/s, over {:.1} hours: {:?}",
        speeds.len(),
        (h.time() - started) / 3600.0,
        last.outcome.map(|o| o.1.regime)
    );
    assert_eq!(h.top.bodies.len(), 1);
    assert!(speeds.windows(2).all(|w| w[1] < w[0]), "{speeds:?}");
}

#[test]
fn fragments_that_fall_back_give_back_their_mass() {
    // A fragment dropped from just above a planet comes back down, and the
    // planet takes back its mass and momentum.
    use worldline_core::debris::Fragment;
    use worldline_core::gravity::Newtonian;
    let gm = 3.986e14;
    let top = System::new(vec![
        Body::new("Sun", GM_SUN, SOLAR_RADIUS).at(DVec3::X * AU),
        Body::new("Earth", gm, 6.371e6),
    ]);
    let mut h = Hierarchy::new(top, Box::new(Newtonian), Vec::new());
    let piece = Fragment {
        position: DVec3::Y * 7e6,
        velocity: -DVec3::Y * 100.0,
        gm: 1e9,
    };
    h.add_debris(&[piece]);
    assert_eq!(h.debris().len(), 1);
    while h.time() < 2e4 {
        h.step(600.0);
    }
    h.sync_swarm();
    let swallowed = h.take_swallowed();
    assert_eq!(swallowed.len(), 1, "{swallowed:?}");
    assert_eq!(
        (swallowed[0].by.as_str(), swallowed[0].fragments),
        ("Earth", 1)
    );
    assert!(h.debris().is_empty());
    assert_eq!(h.top.bodies[1].gm, gm + 1e9);
}
