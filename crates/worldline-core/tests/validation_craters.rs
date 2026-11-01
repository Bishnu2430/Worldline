//! Step 6.3: the crater laws against the paper's own numbers, and against
//! Meteor Crater and Chicxulub from their estimated impactors.
//!
//! The limit is the published scaling's spread: Holsapple (1993) gives
//! crater volumes to "a factor of perhaps two".

use std::f64::consts::PI;

use worldline_core::constants::{AU, G, GM_SUN, SOLAR_RADIUS};
use worldline_core::crater::{Crater, Ground, Strike, dig};
use worldline_core::gravity::Newtonian;
use worldline_core::hierarchy::{Hierarchy, MoonSystem};
use worldline_core::{Body, DVec3, System};

const EARTH_GRAVITY: f64 = 9.81;
const MOON_GRAVITY: f64 = 1.62;

/// A sphere's mass, kg, from its diameter (m) and density (kg/m³).
fn sphere(diameter: f64, density: f64) -> f64 {
    PI / 6.0 * diameter.powi(3) * density
}

/// A strike from the horizontal at `degrees`.
fn strike(mass: f64, speed: f64, degrees: f64, gravity: f64) -> Strike {
    Strike {
        mass,
        speed,
        impact_parameter: degrees.to_radians().cos(),
        gravity,
    }
}

/// Whether `value` rounds to `printed`, which has `digits` after the point.
fn rounds_to(value: f64, printed: f64, digits: i32) -> bool {
    let scale = 10f64.powi(digits);
    (value * scale).round() == (printed * scale).round()
}

#[test]
fn table_1_is_reproduced() {
    // V = 0.48 m^0.783 G^−0.65 U^1.3, in kg, Earth's gravities and km/s: how
    // the table prints its rocks, straight down at 10 km/s, far above
    // strength's reach. Those exponents take α as 0.65, which μ = 0.55
    // makes 0.647, so the limit is that difference, as a power of π₂, plus
    // the rounding of 0.48.
    let ground = Ground::HARD_ROCK;
    let alpha = 3.0 * ground.mu / (2.0 + ground.mu);
    for mass in [1e15f64, 1e17, 1e19] {
        let crater = dig(&strike(mass, 1e4, 90.0, EARTH_GRAVITY), &ground);
        let printed = 0.48 * mass.powf(0.783) * 10f64.powf(1.3);
        let size = (3.0 * mass / (4.0 * PI * ground.density)).cbrt();
        let pi2 = 3.22 * EARTH_GRAVITY * size / 1e8;
        let limit = 0.005 / 0.48 + (0.65 - alpha) * pi2.ln().abs();
        let off = crater.volume / printed - 1.0;
        println!(
            "gravity regime, {mass:.0e} kg: V = {:.3e} m³, {:+.1}% from the table (limit {:.1}%)",
            crater.volume,
            100.0 * off,
            100.0 * limit
        );
        assert!(off.abs() <= limit, "{off} against {limit}");
    }
    // V = c m U^1.65, far below gravity's reach: a very small impactor.
    let strength = |ground: Ground| {
        let crater = dig(&strike(1e-9, 1e4, 90.0, EARTH_GRAVITY), &ground);
        crater.volume / (1e-9 * 10f64.powf(1.65))
    };
    let (soft, hard) = (strength(Ground::SOFT_ROCK), strength(Ground::HARD_ROCK));
    println!("strength regime: V = {soft:.4} m U^1.65 (0.009), {hard:.4} m U^1.65 (0.005)");
    assert!(rounds_to(soft, 0.009, 3), "{soft}");
    assert!(rounds_to(hard, 0.005, 3), "{hard}");
}

#[test]
fn the_papers_lunar_example_is_reproduced() {
    // p. 363: a 25 km radius impactor at 25 km/s on the Moon, by its dry
    // soil law, excavates 141 km, with a transient rim of 184 km and a depth
    // below it of about 72 km, and settles at a radius of 244 km and a depth
    // of 7.3 km. The limit is the rounding of its constants 0.69 and 7.8, of
    // the Moon's gravity if it took 1/6 of Earth's (a 0.16% change), and of
    // each printed result. The final rim goes as 1.086 powers of the
    // excavation, and the depth as 0.301 of that.
    let ground = Ground::DRY_SOIL;
    let mass = 4.0 / 3.0 * PI * 25e3f64.powi(3) * ground.density;
    let crater = dig(&strike(mass, 25e3, 90.0, MOON_GRAVITY), &ground);
    let rim = 1.3 * crater.excavation_radius;
    println!(
        "excavation {:.1} km (141), transient rim {:.1} km (184), depth {:.1} km (72), \
         final rim {:.1} km (244), final depth {:.2} km (7.3)",
        crater.excavation_radius / 1e3,
        rim / 1e3,
        rim / 2.55 / 1e3,
        crater.rim_radius / 1e3,
        crater.depth / 1e3
    );
    let close = |value: f64, printed: f64, limit: f64| {
        assert!(
            (value / printed - 1.0).abs() <= limit,
            "{value} against {printed}"
        );
    };
    let rounding = 0.05 / 7.8 + 0.005 / 0.69 + 0.0016;
    close(crater.excavation_radius, 141e3, rounding + 0.5 / 141.0);
    close(rim, 184e3, rounding + 0.5 / 184.0);
    close(rim / 2.55, 72e3, rounding + 0.5 / 72.0);
    close(crater.rim_radius, 244e3, 1.1 * rounding + 0.5 / 244.0);
    close(crater.depth, 7.3e3, 0.301 * 1.1 * rounding + 0.05 / 7.3);
    assert!(crater.complex);
}

#[test]
fn an_impact_finds_the_planet_not_its_barycenter() {
    // Earth and the Moon, whose barycenter lies 4,671 km off Earth's center,
    // and a rock falling along the line through them, from the side away
    // from the Moon. The rock feels the two as one mass at the barycenter,
    // as everything at the top level does, so energy gives its speed at any
    // distance. It must touch at Earth's surface, at 13.6 km/s, not 4,671 km
    // further in, where touching the barycenter's sphere would put it, at
    // 15.4 km/s.
    let (gm_earth, gm_moon) = (3.986e14f64, 4.903e12f64);
    let total = gm_earth + gm_moon;
    let distance = 3.844e8f64;
    let off = distance * gm_moon / total;
    let around = (total / distance).sqrt();
    let system = Body::new("system", total, 6.371e6)
        .at(DVec3::new(AU, 0.0, 0.0))
        .moving(DVec3::new(0.0, (GM_SUN / AU).sqrt(), 0.0));
    let planet = Body::new("Earth", gm_earth, 6.371e6)
        .at(DVec3::new(-off, 0.0, 0.0))
        .moving(DVec3::new(0.0, -around * gm_moon / total, 0.0));
    let moon = Body::new("Moon", gm_moon, 1.737e6)
        .at(DVec3::new(distance - off, 0.0, 0.0))
        .moving(DVec3::new(0.0, around * gm_earth / total, 0.0));
    let rock = Body::new("rock", G * 3e8, 20.0)
        .at(system.position + planet.position + DVec3::new(-2e7, 0.0, 0.0))
        .moving(system.velocity + planet.velocity + DVec3::new(12e3, 0.0, 0.0));
    let sun = Body::new("Sun", GM_SUN, SOLAR_RADIUS);
    let top = System::new(vec![sun, system, rock]);
    let moons = vec![MoonSystem::new(1, vec![planet, moon])];
    let mut h = Hierarchy::new(top, Box::new(Newtonian), moons);
    h.collisions = true;
    let collision = loop {
        h.step(60.0);
        if let Some(c) = h.take_collisions().into_iter().next() {
            break c;
        }
        assert!(h.time() < 7200.0, "no impact");
    };
    assert_eq!(collision.survivor, "system");
    let dug = collision.crater.expect("a crater");
    println!(
        "struck at b = {:.5}, {:.1} km/s, on a surface pulling at {:.2} m/s², at {:.4?}",
        dug.strike.impact_parameter,
        dug.strike.speed / 1e3,
        dug.strike.gravity,
        dug.at
    );
    let radius = 6.371e6 + 20.0;
    let wanted =
        (12e3f64.powi(2) + 2.0 * total * (1.0 / (radius + off) - 1.0 / (2e7 + off))).sqrt();
    println!("energy says {:.1} km/s", wanted / 1e3);
    assert!((dug.strike.speed / wanted - 1.0).abs() < 2e-3);
    assert!((dug.at + DVec3::X).length() < 0.01);
    // Earth's gravity, not Earth's and the Moon's together.
    assert!((dug.strike.gravity - gm_earth / 6.371e6f64.powi(2)).abs() < 1e-12);
    // It joined the planet.
    assert_eq!(h.top.bodies.len(), 2);
}

/// Prints a crater against the real one and checks it's within the spread.
fn compare(name: &str, crater: &Crater, measured: f64) {
    let diameter = 2.0 * crater.rim_radius;
    let spread = crater.spread();
    println!(
        "{name}: {:.0} m across, from {:.0} to {:.0} m with the law's spread; \
         measured {measured:.0} m, {:+.0}%; {} crater, {:.0} m deep",
        diameter,
        diameter / spread,
        diameter * spread,
        100.0 * (diameter / measured - 1.0),
        if crater.complex { "complex" } else { "simple" },
        crater.depth,
    );
    assert!(
        measured >= diameter / spread && measured <= diameter * spread,
        "{name}: {diameter} against {measured}"
    );
}

#[test]
fn meteor_crater_from_its_estimated_impactor() {
    // Melosh & Collins (2005): 300,000 tons of iron, reaching the ground at
    // about 12 km/s. Its angle isn't known, so 45°, the likeliest of all
    // (Shoemaker 1962). The target is sediments, and the crater 1,186 m
    // across.
    let ground = Ground::SOFT_ROCK;
    let crater = dig(&strike(3e8, 12e3, 45.0, EARTH_GRAVITY), &ground);
    compare("Meteor Crater", &crater, 1186.0);
    assert!(!crater.complex);
    // If only the half that stayed whole dug the crater, as a bound.
    let half = dig(&strike(1.5e8, 12e3, 45.0, EARTH_GRAVITY), &ground);
    println!(
        "   with only the half that stayed whole: {:.0} m",
        2.0 * half.rim_radius
    );
    // Collins et al. (2005), eqs. 21 and 22, for the same strike. Their
    // constant 1.161 is a best estimate between 0.8 and 1.5.
    let l = (6.0 * 3e8 / (PI * 7800.0)).cbrt();
    let collins = 1.25
        * 1.161
        * (7800.0f64 / 2500.0).cbrt()
        * l.powf(0.78)
        * 12e3f64.powf(0.44)
        * EARTH_GRAVITY.powf(-0.22)
        * 45f64.to_radians().sin().cbrt();
    println!("   by Collins et al.'s law: {collins:.0} m");
}

#[test]
fn chicxulub_from_its_estimated_impactor() {
    // Collins et al. (2020): a 17 km impactor of density 2630 kg/m³ at
    // 12 km/s and 60°, chosen to make a Chicxulub-sized crater in their
    // simulations. The crater is usually given as 180 km across, and up to
    // about 200 km (Morgan et al. 2016).
    let ground = Ground::HARD_ROCK;
    let mass = sphere(17e3, 2630.0);
    let crater = dig(&strike(mass, 12e3, 60.0, EARTH_GRAVITY), &ground);
    compare("Chicxulub", &crater, 180e3);
    assert!(crater.complex);
    let reach = 2.0 * crater.rim_radius * crater.spread();
    println!(
        "   the law's spread reaches {:.0} km, so 200 km is {:.0}% past it",
        reach / 1e3,
        100.0 * (200e3 / reach - 1.0)
    );
    // Collins et al. (2005), eqs. 21 and 27, for the same strike.
    let transient = 1.161
        * (2630.0f64 / 2700.0).cbrt()
        * 17e3f64.powf(0.78)
        * 12e3f64.powf(0.44)
        * EARTH_GRAVITY.powf(-0.22)
        * 60f64.to_radians().sin().cbrt();
    let collins = |d: f64| 1.17 * (d / 1e3).powf(1.13) / 3.2f64.powf(0.13);
    println!(
        "   by Collins et al.'s law: {:.0} km, and up to {:.0} km with their constant at 1.5",
        collins(transient),
        collins(transient * 1.5 / 1.161)
    );
}
