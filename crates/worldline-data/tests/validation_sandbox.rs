//! Validation: dropping a Jupiter-mass planet into the inner solar system.

use worldline_core::constants::{AU, C, G, GM_SUN, JULIAN_YEAR};
use worldline_core::diagnostics::{linear_momentum, newtonian_energy};
use worldline_core::hierarchy::Hierarchy;
use worldline_core::mean_elements::MeanElements;
use worldline_core::{Body, DMat3, DVec3};
use worldline_data::full_solar_system;

/// The real solar system with the planets and heavy asteroids at full
/// accuracy. The moon systems are left out: the planets see each as one
/// point at its barycenter anyway, and the moons would only slow the run.
fn solar_system() -> Hierarchy {
    let mut h = full_solar_system();
    h.moon_systems.clear();
    h
}

/// A planet of Jupiter's mass 1.5 AU from the Sun, between Earth and Mars,
/// launched on a circular orbit: at the speed √(G(M☉ + m)/r) relative to
/// the Sun, perpendicular to the Sun's direction, in the ecliptic.
fn intruder(h: &Hierarchy) -> Body {
    let sun = &h.top.bodies[0];
    let jupiter = h.top.bodies.iter().find(|b| b.name == "Jupiter").unwrap();
    let r = 1.5 * AU;
    let speed = ((sun.gm + jupiter.gm) / r).sqrt();
    Body::new("Intruder", jupiter.gm, jupiter.radius)
        .at(sun.position + DVec3::new(-r, 0.0, 0.0))
        .moving(sun.velocity + DVec3::new(0.0, -speed, 0.0))
}

/// A body's osculating semi-major axis (AU) and eccentricity around the Sun.
fn orbit(h: &Hierarchy, name: &str) -> (f64, f64) {
    let sun = &h.top.bodies[0];
    let b = h.top.bodies.iter().find(|b| b.name == name).unwrap();
    let o = MeanElements::osculating(
        b.position - sun.position,
        b.velocity - sun.velocity,
        sun.gm + b.gm,
        DMat3::IDENTITY,
        0.0,
    );
    (o.a / AU, o.e)
}

#[test]
fn a_jupiter_mass_planet_disturbs_the_inner_orbits() {
    let mut calm = solar_system();
    let mut disturbed = solar_system();
    disturbed.add_body(intruder(&disturbed));
    let energy = newtonian_energy(&disturbed.top);
    let momentum = linear_momentum(&disturbed.top);
    // The scale of the total momentum: the sum of the bodies' own, in
    // kg m/s like `linear_momentum`.
    let scale: f64 = disturbed
        .top
        .bodies
        .iter()
        .map(|b| (b.gm * b.velocity).length())
        .sum::<f64>()
        / G;
    let planets = ["Venus", "Earth", "Mars", "Intruder"];
    let start: Vec<(f64, f64)> = planets[..3].iter().map(|p| orbit(&calm, p)).collect();

    // After one year: how far Earth has been pulled, relative to the Sun,
    // from where it would be. (The Sun itself is pulled too, and the whole
    // system now drifts with the momentum the intruder brought; measuring
    // from the Sun leaves just the disturbance of Earth's orbit.)
    calm.advance(JULIAN_YEAR);
    disturbed.advance(JULIAN_YEAR);
    let earth = |h: &Hierarchy| {
        let e = h.top.bodies.iter().find(|b| b.name == "Earth").unwrap();
        e.position - h.top.bodies[0].position
    };
    let shift = (earth(&disturbed) - earth(&calm)).length();
    let sun_shift = (disturbed.top.bodies[0].position - calm.top.bodies[0].position).length();
    println!(
        "after a year, Earth's orbit is {:.0} km off (and the Sun {:.0} km)",
        shift / 1e3,
        sun_shift / 1e3
    );
    calm.advance(9.0 * JULIAN_YEAR);
    disturbed.advance(9.0 * JULIAN_YEAR);

    println!(
        "{:<9} {:>22} {:>22} {:>22}",
        "planet", "now: a (AU), e", "in 10 yr, alone", "with the intruder"
    );
    for (i, planet) in planets.iter().enumerate() {
        let before = start.get(i).copied().unwrap_or((1.5, 0.0));
        let alone = if i < 3 {
            orbit(&calm, planet)
        } else {
            (f64::NAN, f64::NAN)
        };
        let with = orbit(&disturbed, planet);
        println!(
            "{planet:<9} {:>13.5} {:>8.5} {:>13.5} {:>8.5} {:>13.5} {:>8.5}",
            before.0, before.1, alone.0, alone.1, with.0, with.1
        );
    }

    // The physics stays sound with the intruder. Relativistic gravity
    // conserves its own energy and momentum; the Newtonian ones it is
    // checked by wobble only by about (v/c)², 4 × 10⁻⁸ for Mercury at
    // perihelion (59 km/s). The bound allows a few times that.
    let v_over_c_squared = (59e3 / C).powi(2);
    let energy_drift = ((newtonian_energy(&disturbed.top) - energy) / energy).abs();
    let momentum_drift = (linear_momentum(&disturbed.top) - momentum).length() / scale;
    println!(
        "Newtonian energy drift {energy_drift:.1e}, momentum drift {momentum_drift:.1e} ((v/c)² = {v_over_c_squared:.1e})"
    );
    assert!(energy_drift < 2.5 * v_over_c_squared);
    assert!(momentum_drift < 2.5 * v_over_c_squared);

    // The disturbance has the right size. The intruder's pull on Earth and
    // on the Sun differ by about its pull at 1 AU, f = Gm / (1 AU)²
    // = 5.7 × 10⁻⁶ m/s², and over a year that alone moves Earth relative to
    // the Sun by about ½ f t² ≈ 2.8 million km. Geometry (how the two pulls
    // point) and the drift that builds up along the orbit change that by
    // factors of a few, so the measured shift must land within a factor of
    // 10 either way.
    let pull = disturbed.top.bodies.last().unwrap().gm / (AU * AU);
    let estimate = 0.5 * pull * JULIAN_YEAR * JULIAN_YEAR;
    println!(
        "estimate ½ f t² = {:.0} km: measured / estimate = {:.2}",
        estimate / 1e3,
        shift / estimate
    );
    assert!((0.1..10.0).contains(&(shift / estimate)));
}

#[test]
fn a_planet_launched_at_circular_speed_stays_on_a_circle() {
    // Launched as the app launches a body when it is placed without a drag:
    // its distance from the Sun must stay at 1.5 AU, up to the other
    // planets' pull (Jupiter's is the largest: about its mass ratio to the
    // Sun's, 10⁻³, of the Sun's own).
    let mut h = solar_system();
    h.add_body(intruder(&h));
    let distance = |h: &Hierarchy| {
        let b = h.top.bodies.iter().find(|b| b.name == "Intruder").unwrap();
        (b.position - h.top.bodies[0].position).length() / AU
    };
    let period = std::f64::consts::TAU * ((1.5 * AU).powi(3) / GM_SUN).sqrt();
    let (mut least, mut most) = (f64::MAX, 0.0_f64);
    for _ in 0..100 {
        h.advance(period / 100.0);
        let d = distance(&h);
        (least, most) = (least.min(d), most.max(d));
    }
    println!("over one orbit: {least:.5} to {most:.5} AU from the Sun");
    assert!((least / 1.5 - 1.0).abs() < 1e-3 && (most / 1.5 - 1.0).abs() < 1e-3);
}
