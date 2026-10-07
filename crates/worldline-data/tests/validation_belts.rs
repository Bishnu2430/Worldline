//! Validation: the structure that resonances carve into the real asteroid
//! belt, Jupiter's Trojans and the Kuiper belt, and how far the two-body
//! orbits used to draw them drift.

use std::f64::consts::TAU;

use worldline_core::DMat3;
use worldline_core::constants::{AU, DAY, GM_SUN};
use worldline_core::kepler::drift;
use worldline_core::mean_elements::MeanElements;
use worldline_core::orbit::{Elements, KeplerOrbit};
use worldline_core::swarm::Particle;
use worldline_data::{BeltKind, SmallBodyKind, Snapshot, belts, small_bodies, solar_system};

/// Sidereal orbital periods in days, from NASA's planetary fact sheets.
const JUPITER_PERIOD: f64 = 4332.589;
const NEPTUNE_PERIOD: f64 = 60189.0;

fn orbits(kind: BeltKind) -> Vec<Elements> {
    belts()
        .into_iter()
        .find(|b| b.kind == kind)
        .expect("belt")
        .orbits
}

/// Where an asteroid's orbit takes `ratio` times as long as a planet's
/// with period `period` (days): Kepler's third law around the Sun.
fn resonance(period: f64, ratio: f64) -> f64 {
    let n = TAU / (period * ratio * DAY);
    (GM_SUN / (n * n)).cbrt()
}

/// How many orbits have semi-major axes within 0.2% of `center`, how many
/// a smooth belt would put there (judged from the bands 2–4% away on both
/// sides), and how many standard deviations of chance (Poisson noise in
/// the expected count) the difference is.
fn count_at(orbits: &[Elements], center: f64) -> (usize, f64, f64) {
    let near = orbits
        .iter()
        .filter(|o| (o.a - center).abs() < 0.002 * center)
        .count();
    let beside = orbits
        .iter()
        .filter(|o| (0.02 * center..0.04 * center).contains(&(o.a - center).abs()))
        .count();
    // The bands are 0.04 of `center` wide in all; the window 0.004.
    let expected = beside as f64 * 0.004 / 0.04;
    (near, expected, (near as f64 - expected) / expected.sqrt())
}

#[test]
fn kirkwood_gaps_open_at_jupiters_resonances() {
    // An asteroid whose period is a simple fraction of Jupiter's meets
    // Jupiter at the same points of its orbit again and again, and the
    // repeated tugs make its orbit chaotic until it is thrown out
    // (Wisdom 1982). Kirkwood noticed the gaps in 1866, with under 100 asteroids known.
    // The standard: each gap must be emptier than a smooth belt by more
    // than 5 standard deviations of chance.
    let belt = orbits(BeltKind::AsteroidBelt);
    println!(
        "{:<6} {:>8} {:>9} {:>9} {:>8}",
        "", "a (AU)", "observed", "expected", "sigma"
    );
    for (name, ratio) in [
        ("3:1", 1.0 / 3.0),
        ("5:2", 2.0 / 5.0),
        ("7:3", 3.0 / 7.0),
        ("2:1", 1.0 / 2.0),
    ] {
        let a = resonance(JUPITER_PERIOD, ratio);
        let (observed, expected, sigma) = count_at(&belt, a);
        println!(
            "{name:<6} {:>8.4} {observed:>9} {expected:>9.1} {sigma:>8.1}",
            a / AU
        );
        assert!(sigma < -5.0, "no gap at the {name} resonance");
    }
}

#[test]
fn hildas_crowd_into_jupiters_3_2_resonance() {
    // The 3:2 resonance does the opposite: its geometry keeps the Hildas
    // away from Jupiter whenever they pass, so the resonance protects
    // them. The same standard, as an excess.
    let belt = orbits(BeltKind::AsteroidBelt);
    let a = resonance(JUPITER_PERIOD, 2.0 / 3.0);
    let (observed, expected, sigma) = count_at(&belt, a);
    println!(
        "3:2 at {:.4} AU: {observed} observed, {expected:.1} expected, {sigma:+.1} sigma",
        a / AU
    );
    assert!(sigma > 5.0);
}

#[test]
fn plutinos_crowd_into_neptunes_3_2_resonance() {
    // Pluto and the Plutinos go round twice while Neptune goes round
    // three times, which keeps them from ever meeting it, even though
    // Pluto crosses Neptune's orbit. Many were probably swept in as
    // Neptune migrated outward (Malhotra 1993).
    let kuiper = orbits(BeltKind::KuiperBelt);
    let a = resonance(NEPTUNE_PERIOD, 3.0 / 2.0);
    let (observed, expected, sigma) = count_at(&kuiper, a);
    println!(
        "Neptune 3:2 at {:.3} AU: {observed} observed, {expected:.1} expected, {sigma:+.1} sigma",
        a / AU
    );
    assert!(sigma > 5.0);
}

/// Wraps an angle into (−180°, 180°], in degrees.
fn wrap_degrees(angle: f64) -> f64 {
    let d = angle.to_degrees().rem_euclid(360.0);
    if d > 180.0 { d - 360.0 } else { d }
}

#[test]
fn trojans_cluster_60_degrees_ahead_of_and_behind_jupiter() {
    // Lagrange (1772): a small body sharing a planet's orbit can stay put
    // 60° ahead of the planet (L4) or behind it (L5), at the corners of
    // equilateral triangles with the Sun. Real Trojans swing around these
    // points. The standard: ±60° must lie within the middle half of each
    // cloud (between its quartiles).
    let system = solar_system().system();
    let sun = &system.bodies[0];
    let jupiter = system.bodies.iter().find(|b| b.name == "Jupiter").unwrap();
    let orbit = MeanElements::osculating(
        jupiter.position - sun.position,
        jupiter.velocity - sun.velocity,
        sun.gm + jupiter.gm,
        DMat3::IDENTITY,
        0.0,
    );
    let jupiter_longitude = orbit.node + orbit.periapsis + orbit.mean_anomaly;
    let offsets: Vec<f64> = orbits(BeltKind::JupiterTrojans)
        .iter()
        .map(|o| wrap_degrees(o.mean_longitude(0.0, GM_SUN) - jupiter_longitude))
        .collect();
    for (name, ahead, target) in [("L4", true, 60.0), ("L5", false, -60.0)] {
        let mut cloud: Vec<f64> = offsets
            .iter()
            .copied()
            .filter(|&d| (d > 0.0) == ahead)
            .collect();
        cloud.sort_by(f64::total_cmp);
        let quantile = |q: f64| cloud[((cloud.len() - 1) as f64 * q).round() as usize];
        let (low, median, high) = (quantile(0.25), quantile(0.5), quantile(0.75));
        println!(
            "{name}: {} Trojans, median {median:+.1}°, middle half {low:+.1}° to {high:+.1}°",
            cloud.len()
        );
        assert!(
            (low..=high).contains(&target),
            "{name} isn't centered on {target}°"
        );
    }
}

#[test]
fn two_body_orbits_drift_less_than_jupiters_share_of_gravity() {
    // The belts ride fixed ellipses around the Sun, leaving out the
    // planets' pull. Jupiter's, the strongest, is about 1/1,000 of the
    // Sun's (their mass ratio), so over a year, less than an orbit for
    // anything beyond Mars, a body should stray by less than that fraction
    // of its distance. Checked against JPL with the asteroids and
    // trans-Neptunian objects of step 1b.6, whose 2025 and 2026 states JPL
    // gives.
    let start = solar_system();
    let end = Snapshot::parse(include_str!("../data/solar-system-2026-01-01T06.csv"))
        .expect("valid reference");
    let (sun_start, sun_end) = (&start.bodies[0], &end.bodies[0]);
    let jupiter = start.bodies.iter().find(|b| b.name == "Jupiter").unwrap();
    let gm_ratio = jupiter.gm / sun_start.gm;
    let reference = worldline_data::small_bodies_one_year_on();
    let year = (end.epoch_jd_tdb - start.epoch_jd_tdb) * DAY;
    let mut worst: (f64, String) = (0.0, String::new());
    let mut fractions = Vec::new();
    for body in small_bodies().iter().filter(|b| {
        matches!(
            b.kind,
            SmallBodyKind::Asteroid | SmallBodyKind::DwarfPlanet | SmallBodyKind::TransNeptunian
        )
    }) {
        let r = body.state.0 - sun_start.position;
        let v = body.state.1 - sun_start.velocity;
        let osculating = MeanElements::osculating(r, v, GM_SUN, DMat3::IDENTITY, 0.0);
        let elements = Elements {
            a: osculating.a,
            e: osculating.e,
            inclination: osculating.inclination,
            node: osculating.node,
            periapsis: osculating.periapsis,
            mean_anomaly: osculating.mean_anomaly,
            epoch: 0.0,
        };
        let predicted = KeplerOrbit::new(&elements, GM_SUN).position_at(year);
        let actual = reference[&body.name].0 - sun_end.position;
        let fraction = (predicted - actual).length() / actual.length();
        fractions.push(fraction);
        if fraction > worst.0 {
            worst = (fraction, body.name.clone());
        }
    }
    fractions.sort_by(f64::total_cmp);
    println!(
        "{} bodies: median drift {:.1e} of distance, worst {:.1e} ({}); Jupiter/Sun = {gm_ratio:.2e}",
        fractions.len(),
        fractions[fractions.len() / 2],
        worst.0,
        worst.1
    );
    assert!(worst.0 < gm_ratio, "{} drifted {:.1e}", worst.1, worst.0);
}

#[test]
fn the_live_belts_follow_jpl_where_fixed_ellipses_drift() {
    // The belts as live particles feel the planets' pull. Two dozen of
    // their bodies, spread through each belt, started from JPL's states on
    // 2025-01-01 and run for a Julian year with the real solar system,
    // compared with where JPL puts them then; and the same bodies on fixed
    // two-body ellipses around the Sun, as the belts were drawn before.
    let start = solar_system();
    let end = Snapshot::parse(include_str!("../data/solar-system-2026-01-01T06.csv"))
        .expect("valid reference");
    let (sun_start, sun_end) = (&start.bodies[0], &end.bodies[0]);
    let year = (end.epoch_jd_tdb - start.epoch_jd_tdb) * DAY;
    let samples = worldline_data::belt_samples();
    let mut h = worldline_data::full_solar_system();
    h.add_particles(samples.iter().enumerate().map(|(k, s)| Particle {
        position: s.start.0,
        velocity: s.start.1,
        id: k as u32,
    }));
    h.advance(year);
    h.sync_swarm();
    let (mut live, mut fixed) = (Vec::new(), Vec::new());
    for (k, sample) in samples.iter().enumerate() {
        let particle = h
            .swarm()
            .particles
            .iter()
            .find(|p| p.id == k as u32)
            .unwrap();
        let actual = sample.one_year_on.0;
        let distance = (actual - sun_end.position).length();
        let ellipse = drift(
            sample.start.0 - sun_start.position,
            sample.start.1 - sun_start.velocity,
            GM_SUN,
            year,
        );
        let live_error = (particle.position - actual).length() / distance;
        let fixed_error = (sun_end.position + ellipse.position - actual).length() / distance;
        println!(
            "{:<12} {:?}: live {:.1e}, fixed ellipse {:.1e} of its distance ({:.0} km and {:.0} km)",
            sample.designation,
            sample.kind,
            live_error,
            fixed_error,
            live_error * distance / 1e3,
            fixed_error * distance / 1e3
        );
        live.push(live_error);
        fixed.push(fixed_error);
    }
    live.sort_by(f64::total_cmp);
    fixed.sort_by(f64::total_cmp);
    println!(
        "median: live {:.1e}, fixed {:.1e}; worst: live {:.1e}, fixed {:.1e}",
        live[live.len() / 2],
        fixed[fixed.len() / 2],
        live[live.len() - 1],
        fixed[fixed.len() - 1]
    ); // The standard for comparisons with JPL since step 1.3: 1 part in
    // 10,000 after a year. The fixed ellipses don't meet it.
    assert!(live[live.len() - 1] < 1e-4);
    assert!(fixed[fixed.len() / 2] > 1e-4);
}

#[test]
fn the_belts_start_where_jpl_puts_them() {
    // JPL gives the belts' orbits at 2026-06-09; the swarm starts them at
    // the 2025-01-01 snapshot by running them back along the planets'
    // recorded paths. Checked against JPL's own states at the snapshot for
    // the two dozen sample bodies, and against carrying them back on fixed
    // ellipses instead.
    let clock = std::time::Instant::now();
    let particles = worldline_data::belt_particles();
    let took = clock.elapsed();
    let belts = belts();
    let start = solar_system();
    let sun = &start.bodies[0];
    let mut offset = 0;
    let mut index = std::collections::HashMap::new();
    for belt in &belts {
        for (k, designation) in belt.designations.iter().enumerate() {
            index.insert(designation.clone(), (offset + k, belt.orbits[k]));
        }
        offset += belt.orbits.len();
    }
    let (mut run_back, mut ellipses) = (Vec::new(), Vec::new());
    for sample in worldline_data::belt_samples() {
        let (k, elements) = index[&sample.designation];
        let particle = particles[k];
        assert_eq!(particle.id, k as u32);
        let actual = sample.start.0;
        let distance = (actual - sun.position).length();
        let ellipse = sun.position + KeplerOrbit::new(&elements, GM_SUN).position_at(0.0);
        run_back.push((particle.position - actual).length() / distance);
        ellipses.push((ellipse - actual).length() / distance);
    }
    run_back.sort_by(f64::total_cmp);
    ellipses.sort_by(f64::total_cmp);
    println!(
        "{} bodies placed in {:.2} s; samples off JPL at the snapshot: run back, median {:.1e} and worst {:.1e} of their distance; on fixed ellipses, {:.1e} and {:.1e}",
        particles.len(),
        took.as_secs_f64(),
        run_back[run_back.len() / 2],
        run_back[run_back.len() - 1],
        ellipses[ellipses.len() / 2],
        ellipses[ellipses.len() - 1]
    ); // As for the year's run: within 1 part in 10,000 of JPL.
    assert!(run_back[run_back.len() - 1] < 1e-4);
}
