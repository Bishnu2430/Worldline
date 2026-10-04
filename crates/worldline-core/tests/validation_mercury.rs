//! Validation: the precession of Mercury's perihelion.
//!
//! In the 1850s Le Verrier found that Mercury's orbit turns slightly faster
//! than Newtonian gravity explains. In 1915 Einstein showed that general
//! relativity accounts for the difference exactly: 43″ per century. This
//! was the first evidence for general relativity.
//!
//! The test simulates the Sun and Mercury alone for a century, with and
//! without relativity, and measures how fast the orbit's closest point
//! turns. Newtonian gravity must give zero; relativity must give
//! Δω = 6πGM / (c² a (1 − e²)) per orbit.

use std::f64::consts::PI;

use worldline_core::constants::{AU, C, DAY, GM_SUN, JULIAN_YEAR, SOLAR_RADIUS};
use worldline_core::gravity::{EinsteinInfeldHoffmann, Gravity, Newtonian};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::orbit::{kepler_period, two_body_system};
use worldline_core::{Body, System};

/// Mercury's mean orbital elements (Standish, J2000) and DE440 GM.
const A: f64 = 0.387_098_93 * AU;
const E: f64 = 0.205_630_69;
const GM_MERCURY: f64 = 2.203_186_855_140_000_3e13;

const ARCSEC_PER_RADIAN: f64 = 180.0 / PI * 3600.0;
const CENTURY: f64 = 100.0 * JULIAN_YEAR;

fn sun_and_mercury() -> System {
    two_body_system(
        Body::new("Sun", GM_SUN, SOLAR_RADIUS),
        Body::new("Mercury", GM_MERCURY, 2.4394e6),
        A,
        E,
    )
}

/// Direction of the orbit's closest point, from the Laplace–Runge–Lenz
/// vector A = v × h − μ r̂, which points at periapsis.
fn perihelion_angle(system: &System) -> f64 {
    let (sun, mercury) = (&system.bodies[0], &system.bodies[1]);
    let r = mercury.position - sun.position;
    let v = mercury.velocity - sun.velocity;
    let mu = sun.gm + mercury.gm;
    let lrl = v.cross(r.cross(v)) - mu * r.normalize();
    lrl.y.atan2(lrl.x)
}

/// Simulates a century, sampling the perihelion direction daily, and
/// returns the least-squares rate in arcseconds per century.
fn precession_rate(gravity: &dyn Gravity) -> f64 {
    let mut system = sun_and_mercury();
    let mut ias = Ias15::new();
    let samples = (CENTURY / DAY) as usize;
    let mut points = Vec::with_capacity(samples + 1);
    points.push((0.0, perihelion_angle(&system)));
    for _ in 0..samples {
        advance(&mut system, gravity, &mut ias, DAY);
        points.push((system.time(), perihelion_angle(&system)));
    }
    let n = points.len() as f64;
    let t_mean = points.iter().map(|p| p.0).sum::<f64>() / n;
    let a_mean = points.iter().map(|p| p.1).sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (t, angle) in &points {
        num += (t - t_mean) * (angle - a_mean);
        den += (t - t_mean) * (t - t_mean);
    }
    num / den * CENTURY * ARCSEC_PER_RADIAN
}

#[test]
fn mercury_perihelion_precesses_43_arcseconds_per_century() {
    let mu = GM_SUN + GM_MERCURY;
    let per_orbit = 6.0 * PI * mu / (C * C * A * (1.0 - E * E));
    let orbits_per_century = CENTURY / kepler_period(A, mu);
    let theory = per_orbit * orbits_per_century * ARCSEC_PER_RADIAN;

    let newtonian = precession_rate(&Newtonian);
    let relativistic = precession_rate(&EinsteinInfeldHoffmann);
    println!("Newtonian gravity:          {newtonian:+.4}″ per century");
    println!("Einstein–Infeld–Hoffmann:   {relativistic:+.4}″ per century");
    println!("General relativity formula: {theory:+.4}″ per century");

    assert!(
        newtonian.abs() < 0.001,
        "Newtonian orbit precessed {newtonian}″"
    );
    // Astronomers measure the anomaly to about ±0.04″ (0.1%). The simulation
    // must match the theory far more closely than that: within 0.01%.
    let relative = (relativistic - theory).abs() / theory;
    assert!(
        relative < 1e-4,
        "measured {relativistic}″ vs theory {theory}″"
    );
}
