//! Validation: the major moons against NASA JPL, and the Laplace resonance.

use worldline_core::DVec3;
use worldline_core::constants::DAY;
use worldline_core::hierarchy::Hierarchy;
use worldline_data::{MoonSystemData, parse_moons, rotation_model, solar_system_with_moons};

const START_JD: f64 = 2_460_676.5;

fn reference() -> (f64, Vec<MoonSystemData>) {
    parse_moons(include_str!("../data/moons-2025-01-31.csv")).expect("valid reference data")
}

/// Position of `name` relative to its planet's center, from the simulation.
fn simulated(h: &Hierarchy, name: &str) -> Option<DVec3> {
    for (i, moons) in h.moon_systems.iter().enumerate() {
        if let Some(j) = moons.system.bodies.iter().position(|b| b.name == name) {
            let (planet, _) = h.absolute(i, 0);
            let (moon, _) = h.absolute(i, j);
            return Some(moon - planet);
        }
    }
    None
}

/// Distance (m) between the simulated and JPL positions of each moon of
/// `system` relative to its planet, with the moon's radius (m).
fn errors(h: &Hierarchy, system: &MoonSystemData) -> Vec<(String, f64, f64, f64)> {
    let planet = system.bodies[0].position;
    system.bodies[1..]
        .iter()
        .map(|moon| {
            let expected = moon.position - planet;
            let actual = simulated(h, &moon.name).expect("moon is simulated");
            (
                moon.name.clone(),
                (actual - expected).length(),
                expected.length(),
                moon.radius,
            )
        })
        .collect()
}

#[test]
fn moons_match_jpl_after_30_days() {
    // "Match" means the simulated moon overlaps JPL's: its center is closer
    // to JPL's than its own radius, so it is drawn in the right place and
    // its eclipses and transits come at the right times.
    let (end_jd, reference) = reference();
    let mut h = solar_system_with_moons();
    h.advance((end_jd - START_JD) * DAY);

    println!(
        "{:<10} {:>11} {:>12} {:>14} {:>17}",
        "moon", "error (km)", "radius (km)", "orbit (km)", "error (degrees)"
    );
    let mut failures = Vec::new();
    for system in &reference {
        for (name, error, orbit, radius) in errors(&h, system) {
            println!(
                "{:<10} {:>11.2} {:>12.0} {:>14.0} {:>17.5}",
                name,
                error / 1e3,
                radius / 1e3,
                orbit / 1e3,
                (error / orbit).to_degrees()
            );
            if error >= radius {
                failures.push(name);
            }
        }
    }
    assert!(
        failures.is_empty(),
        "farther from JPL than their own radius: {failures:?}"
    );
}

#[test]
fn mars_lumpy_gravity_keeps_phobos_and_deimos_on_track() {
    // Mars's Tharsis bulge makes its gravity depend on longitude (C22,
    // S22). Deimos orbits just outside the synchronous orbit, so Mars turns
    // beneath it only slowly and the lumps' pull adds up: without them,
    // both moons drift off JPL's track by more than their own size.
    let (end_jd, reference) = reference();
    let mars = reference.iter().find(|s| s.host == "Mars").unwrap();
    for with_lumps in [true, false] {
        let mut h = solar_system_with_moons();
        h.moon_systems.retain(|m| m.system.bodies[0].name == "Mars");
        if !with_lumps {
            h.moon_systems[0].tesseral = None;
        }
        h.advance((end_jd - START_JD) * DAY);
        for (name, error, _, radius) in errors(&h, mars) {
            println!(
                "{} Mars's lumps: {name} is {:.2} km from JPL (radius {:.1} km)",
                if with_lumps { "with" } else { "without" },
                error / 1e3,
                radius / 1e3
            );
            assert_eq!(error < radius, with_lumps, "{name}");
        }
    }
}

/// Mean longitude of an orbit, from its position and velocity relative to
/// the planet, measured in the planet's equatorial plane.
fn mean_longitude(r: DVec3, v: DVec3, mu: f64, pole: DVec3) -> f64 {
    // Axes in the planet's equator.
    let x = DVec3::Z.cross(pole).normalize();
    let y = pole.cross(x);
    let flat = |w: DVec3| DVec3::new(w.dot(x), w.dot(y), w.dot(pole));
    let (r, v) = (flat(r), flat(v));
    let e_vec = ((v.length_squared() - mu / r.length()) * r - r.dot(v) * v) / mu;
    let e = e_vec.length();
    let true_longitude = r.y.atan2(r.x);
    let periapsis = e_vec.y.atan2(e_vec.x);
    let nu = true_longitude - periapsis;
    let eccentric = 2.0 * (((1.0 - e) / (1.0 + e)).sqrt() * (nu / 2.0).tan()).atan();
    periapsis + eccentric - e * eccentric.sin()
}

/// The Laplace angle φ = λ_Io − 3 λ_Europa + 2 λ_Ganymede in degrees,
/// followed continuously (not wrapped to 0–360°), averaged over each 30
/// days for `years` years, after pushing Io forward along its orbit by
/// `kick` m/s. Returns the averages and Io's mean-motion change (°/day).
fn laplace_angle(kick: f64, years: usize) -> (Vec<f64>, f64) {
    let mut h = solar_system_with_moons();
    h.moon_systems
        .retain(|m| m.system.bodies[0].name == "Jupiter");
    let system = &mut h.moon_systems[0].system;
    let (jupiter, io) = (&system.bodies[0], &system.bodies[1]);
    let (r, v) = (
        io.position - jupiter.position,
        io.velocity - jupiter.velocity,
    );
    let mu = jupiter.gm + io.gm;
    // Mean motion n = √(μ/a³), with a from the vis-viva equation; pushing
    // along the orbit changes it by δn = −3n δv/v.
    let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
    let n = (mu / a.powi(3)).sqrt();
    let dn = (-3.0 * n * kick / v.length()).to_degrees() * DAY;
    system.bodies[1].velocity += v.normalize() * kick;

    let pole = rotation_model("Jupiter")
        .unwrap()
        .orientation(START_JD)
        .north_pole();
    let samples_per_day = 4;
    let (mut previous, mut unwrapped) = (None::<f64>, 0.0);
    let (mut sum, mut count, mut averages) = (0.0, 0, Vec::new());
    for _ in 0..years * 365 * samples_per_day {
        let s = &h.moon_systems[0].system;
        let p = &s.bodies[0];
        let longitude = |j: usize| {
            let m = &s.bodies[j];
            mean_longitude(
                m.position - p.position,
                m.velocity - p.velocity,
                p.gm + m.gm,
                pole,
            )
        };
        let phi = (longitude(1) - 3.0 * longitude(2) + 2.0 * longitude(3)).to_degrees();
        unwrapped = match previous {
            None => phi.rem_euclid(360.0),
            Some(last) => unwrapped + (phi - last + 180.0).rem_euclid(360.0) - 180.0,
        };
        previous = Some(phi);
        sum += unwrapped;
        count += 1;
        if count == 30 * samples_per_day {
            averages.push(sum / count as f64);
            (sum, count) = (0.0, 0);
        }
        h.advance(DAY / samples_per_day as f64);
    }
    (averages, dn)
}

#[test]
#[ignore = "slow: 24 simulated years; run with cargo test --release -- --ignored"]
fn galilean_moons_hold_the_laplace_resonance() {
    // Io, Europa and Ganymede are locked so that φ = λ_Io − 3 λ_Europa +
    // 2 λ_Ganymede stays at 180°: a triple conjunction can never happen.
    // Disturbed, φ rocks back and forth about 180° (it librates) with a
    // period of about 2071 days (Lieske 1998).
    const PUBLISHED_PERIOD: f64 = 2071.0;
    let years = 12;

    let (undisturbed, _) = laplace_angle(0.0, years);
    let low = undisturbed.iter().cloned().fold(f64::MAX, f64::min);
    let high = undisturbed.iter().cloned().fold(f64::MIN, f64::max);
    println!(
        "As JPL has them: φ (30-day averages) stays between {low:.3}° and {high:.3}° for {years} years"
    );

    // Push Io forward by 3 m/s. Left alone, φ would then drift steadily
    // through more than a full circle in 12 years. The moons' pull on each
    // other must instead turn it back: φ never goes round (it stays within
    // 180° of 180°) and crosses back over 180° again and again.
    let (disturbed, dn) = laplace_angle(3.0, years);
    let drift = dn.abs() * (years * 365) as f64;
    let low = disturbed.iter().cloned().fold(f64::MAX, f64::min);
    let high = disturbed.iter().cloned().fold(f64::MIN, f64::max);
    let crossings: Vec<f64> = disturbed
        .windows(2)
        .enumerate()
        .filter(|(_, w)| (w[0] - 180.0) * (w[1] - 180.0) < 0.0)
        .map(|(i, w)| (i as f64 + 0.5 + (180.0 - w[0]) / (w[1] - w[0])) * 30.0)
        .collect();
    let period =
        2.0 * (crossings[crossings.len() - 1] - crossings[0]) / (crossings.len() - 1) as f64;
    // A pendulum given a push swings out to (push rate) / (angular frequency).
    let predicted_swing = dn.abs() * PUBLISHED_PERIOD / std::f64::consts::TAU;
    println!("Nudged Io by 3 m/s: alone, φ would drift {drift:.0}° in {years} years");
    println!(
        "Instead φ swings between {low:.1}° and {high:.1}° (a pendulum predicts ±{predicted_swing:.1}°), \
         crossing 180° {} times",
        crossings.len()
    );
    println!("Libration period: {period:.0} days (published: about {PUBLISHED_PERIOD:.0} days)");
    assert!(
        drift > 360.0,
        "the push must be big enough to send φ round if nothing held it"
    );
    assert!(
        low > 0.0 && high < 360.0,
        "φ went round: the resonance broke"
    );
    assert!(crossings.len() >= 2, "φ never swung back");
}
