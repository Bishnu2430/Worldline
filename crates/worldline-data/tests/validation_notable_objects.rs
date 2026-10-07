//! Validation: the catalog of notable objects.
//!
//! The catalog's masses and sizes are copied from the papers that measured
//! them. These tests check that the roadmap's objects load with exactly the
//! published masses, that every object obeys general relativity's limits,
//! and that the catalog's numbers reproduce independent observations: the
//! redshift of Sirius B's light, the size of Sagittarius A* and M87* as
//! the Event Horizon Telescope saw them, and the turning of the Hulse–Taylor
//! pulsar's orbit, simulated with Worldline's own relativistic gravity.

use std::f64::consts::{PI, TAU};

use worldline_core::compact::{BUCHDAHL_RADII, gravitational_radius, gravitational_redshift};
use worldline_core::constants::{AU, C, GM_SUN, JULIAN_YEAR, PARSEC, SOLAR_RADIUS};
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::integrator::{Ias15, advance};
use worldline_core::orbit::two_body_system;
use worldline_core::{Body, System};
use worldline_data::{
    Distance, NotableObject, ObjectKind, RadiusBasis, binary_orbits, notable_object,
    notable_objects,
};

/// Microarcseconds per radian.
const MICROARCSEC: f64 = 180.0 / PI * 3600.0 * 1e6;

fn object(name: &str) -> NotableObject {
    notable_object(name).unwrap_or_else(|| panic!("{name} is in the catalog"))
}

fn distance(o: &NotableObject) -> f64 {
    match o.distance {
        Some(Distance::Meters(d)) => d,
        other => panic!("{} has no distance in meters: {other:?}", o.name),
    }
}

#[test]
fn the_roadmap_objects_load_with_their_published_masses() {
    println!(
        "{:<24} {:<13} {:>16}  radius",
        "object", "kind", "mass (Suns)"
    );
    for o in notable_objects() {
        let radius = match o.radius_basis {
            RadiusBasis::Horizon if o.radius.value > 1e9 => {
                format!("{:.4} AU (horizon)", o.radius.value / AU)
            }
            RadiusBasis::Horizon => format!("{:.2} km (horizon)", o.radius.value / 1e3),
            _ if o.radius.value > 1e7 => format!("{:.4} R_sun", o.radius.value / SOLAR_RADIUS),
            _ => format!("{:.0} km", o.radius.value / 1e3),
        };
        println!(
            "{:<24} {:<13} {:>16.6e}  {radius}",
            o.name,
            format!("{:?}", o.kind),
            o.mass.value
        );
    }
    // Step 2.1's check: the four objects the roadmap names, with exactly
    // the masses their papers publish.
    for (name, published) in [
        ("Sagittarius A*", 4.297e6),    // GRAVITY Collaboration 2022
        ("TON 618", 10f64.powf(10.82)), // Shemmer et al. 2004, log M = 10.82
        ("M87*", 6.5e9),                // Event Horizon Telescope 2019
        ("PSR B1913+16", 1.438),        // Weisberg & Huang 2016
        ("PSR B1913+16 companion", 1.390),
    ] {
        let o = object(name);
        assert_eq!(o.mass.value, published, "{name}");
        assert_eq!(o.body().gm, published * GM_SUN, "{name}");
    }
    assert_eq!(object("TON 618").kind, ObjectKind::BlackHole);
    assert_eq!(object("PSR B1913+16").kind, ObjectKind::NeutronStar);
}

#[test]
fn every_object_obeys_general_relativitys_limits() {
    // Buchdahl (1959): no static star is smaller than 9/4 GM/c², and a
    // black hole's horizon is no bigger than 2GM/c². So every black hole
    // must lie inside the limit and everything else outside it. This
    // also catches a radius entered in the wrong units.
    for o in notable_objects() {
        let compactness = o.radius.value / gravitational_radius(o.gm());
        println!("{:<24} R = {compactness:>12.4} GM/c²", o.name);
        if o.kind == ObjectKind::BlackHole {
            assert!(compactness <= 2.0, "{}", o.name);
        } else {
            assert!(compactness > BUCHDAHL_RADII, "{}", o.name);
        }
    }
    // Sagittarius A*'s horizon is 0.0848 AU, well inside Mercury's orbit;
    // TON 618's, 1300 AU, would hold the solar system out past Sedna.
    let sgr = object("Sagittarius A*").radius.value / AU;
    let ton = object("TON 618").radius.value / AU;
    println!(
        "Horizons: Sagittarius A* {sgr:.4} AU, M87* {:.0} AU, TON 618 {ton:.0} AU",
        object("M87*").radius.value / AU
    );
    assert!((sgr - 0.0848).abs() < 0.0001);
    // Cygnus X-1 spins at least 0.9985: its horizon is at most
    // GM/c² (1 + √(1 − 0.9985²)) = 1.0548 GM/c².
    let cyg = object("Cygnus X-1");
    let ratio = cyg.radius.value / gravitational_radius(cyg.gm());
    assert!((ratio - 1.0548).abs() < 1e-4, "{ratio}");
}

#[test]
fn sirius_b_reddens_its_light_as_hubble_measured() {
    // Light climbing out of a white dwarf loses energy: general relativity
    // predicts a redshift cz = c(1/√(1 − 2GM/Rc²) − 1). From Sirius B's
    // mass (from its orbit, Bond et al. 2017) and radius (from its
    // brightness, Joyce et al. 2018), compare with what the Hubble Space
    // Telescope measured: 80.65 ± 0.77 km/s (Joyce et al. 2018). The
    // prediction carries the mass's and radius's uncertainties; they must
    // agree within the combined uncertainty.
    let b = object("Sirius B");
    let predicted = gravitational_redshift(b.gm(), b.radius.value);
    let (dm, _) = b.mass.plus_minus.expect("published uncertainty");
    let (dr, _) = b.radius.plus_minus.expect("published uncertainty");
    let spread = predicted * ((dm / b.mass.value).powi(2) + (dr / b.radius.value).powi(2)).sqrt();
    let (measured, error) = (80.65e3, 0.77e3);
    let allowed = (spread * spread + error * error).sqrt();
    println!(
        "Sirius B's gravitational redshift: predicted {:.2} ± {:.2} km/s, Hubble measured {:.2} ± {:.2} km/s",
        predicted / 1e3,
        spread / 1e3,
        measured / 1e3,
        error / 1e3
    );
    assert!((predicted - measured).abs() < allowed);
}

#[test]
fn the_black_holes_look_as_big_as_the_event_horizon_telescope_saw() {
    // A black hole's angular size is set by θg = GM/(Dc²). The Event
    // Horizon Telescope measured it from its images, independently of the
    // stars' orbits that give Sagittarius A*'s mass and distance.
    let theta = |o: &NotableObject| gravitational_radius(o.gm()) / distance(o) * MICROARCSEC;
    let sgr = theta(&object("Sagittarius A*"));
    let m87 = theta(&object("M87*"));
    println!(
        "Sagittarius A*: θg = {sgr:.4} μas from GRAVITY's mass and distance; EHT image 4.8 +1.4/-0.7 μas"
    );
    println!(
        "M87*:           θg = {m87:.3} μas from EHT's mass and distance; EHT image 3.8 ± 0.4 μas"
    );
    // EHT 2022 (ApJL 930, L12, Table 1): 4.8 +1.4/−0.7 μas.
    assert!((4.8 - 0.7..=4.8 + 1.4).contains(&sgr));
    // The same table lists GRAVITY's orbits as giving 5.125 μas: our
    // number must match to the rounding of the published mass (4.297),
    // distance (8277) and θg (5.125).
    let rounding = sgr * (0.0005 / 4.297 + 0.5 / 8277.0) + 0.0005;
    assert!((sgr - 5.125).abs() < rounding, "{sgr}");
    // EHT 2019 (ApJL 875, L6): 3.8 ± 0.4 μas.
    assert!((m87 - 3.8).abs() < 0.4);
    assert!((distance(&object("Sagittarius A*")) / PARSEC - 8277.0).abs() < 1e-9);
}

/// Simulates two bodies with relativistic gravity for 100 days, starting
/// at periastron of an orbit with semi-major axis `a` and eccentricity
/// `e`, sampled `per_orbit` times every `period`. Returns how fast the
/// orbit's long axis turns, in degrees per year (a least-squares line
/// through the direction of the Laplace–Runge–Lenz vector), and the time
/// between closest approaches, in s.
fn turning(pair: (Body, Body), a: f64, e: f64, period: f64) -> (f64, f64) {
    let gm = pair.0.gm + pair.1.gm;
    let mut system = two_body_system(pair.0, pair.1, a, e);
    let relative = |s: &System| {
        let (p, c) = (&s.bodies[0], &s.bodies[1]);
        (c.position - p.position, c.velocity - p.velocity)
    };
    let angle = |s: &System| {
        let (r, v) = relative(s);
        let lrl = v.cross(r.cross(v)) - gm * r.normalize();
        lrl.y.atan2(lrl.x)
    };
    let mut ias = Ias15::new();
    let dt = period / 64.0;
    let mut points = vec![(0.0, angle(&system))];
    let mut closest = Vec::new();
    let mut approaching = false;
    while system.time() < 100.0 * 86_400.0 {
        let (r0, v0) = relative(&system);
        advance(&mut system, &EinsteinInfeldHoffmann, &mut ias, dt);
        let (r1, v1) = relative(&system);
        let (before, after) = (r0.dot(v0), r1.dot(v1));
        // Closest approach where r·v turns from negative to positive.
        if approaching && after >= 0.0 {
            closest.push(system.time() - dt * after / (after - before));
        }
        approaching = after < 0.0;
        points.push((system.time(), angle(&system)));
    }
    let n = points.len() as f64;
    let (tm, am) = (
        points.iter().map(|p| p.0).sum::<f64>() / n,
        points.iter().map(|p| p.1).sum::<f64>() / n,
    );
    let (num, den) = points.iter().fold((0.0, 0.0), |(num, den), (t, x)| {
        (num + (t - tm) * (x - am), den + (t - tm) * (t - tm))
    });
    let radial = (closest[closest.len() - 1] - closest[0]) / (closest.len() - 1) as f64;
    (num / den * JULIAN_YEAR * 180.0 / PI, radial)
}

#[test]
fn the_hulse_taylor_orbit_turns_as_measured() {
    // The Hulse–Taylor pulsar and its companion, two neutron stars, orbit
    // every 7.75 hours. Relativity turns the orbit's long axis 4.226585 ±
    // 0.000004 degrees a year (Weisberg & Huang 2016). The pair's masses
    // were found by fitting that turning (with another effect), so starting
    // from the catalog's masses, Worldline's relativistic gravity
    // (Einstein–Infeld–Hoffmann) must reproduce it, within what rounding
    // the masses to 0.001 Suns allows: the rate goes as M^(2/3), so
    // (2/3)(0.002/2.828) = 4.7 × 10⁻⁴.
    let (pulsar, companion) = (object("PSR B1913+16"), object("PSR B1913+16 companion"));
    let orbit = binary_orbits()
        .into_iter()
        .find(|o| o.system == "PSR B1913+16")
        .expect("the orbit is in the catalog");
    let pair = || (pulsar.body(), companion.body());
    let gm = pulsar.gm() + companion.gm();
    let e = orbit.eccentricity;
    // Kepler's third law gives the size of an orbit with the measured
    // period, but relativity lengthens the period of an orbit started
    // that way, by 1.5 × 10⁻⁴ here. What was measured is the period, so
    // resize the orbit until the simulated one matches it.
    let mut a = (gm * (orbit.period / TAU).powi(2)).cbrt();
    let (_, period) = turning(pair(), a, e, orbit.period);
    a *= (orbit.period / period).powf(2.0 / 3.0);
    let (simulated, period) = turning(pair(), a, e, orbit.period);
    // The first post-Newtonian formula (Robertson 1938; Damour & Deruelle
    // 1986): 3 (GM/c³)^(2/3) (Pb/2π)^(−5/3) / (1 − e²) radians a second.
    let formula = 3.0 * (gm / C.powi(3)).powf(2.0 / 3.0) * (orbit.period / TAU).powf(-5.0 / 3.0)
        / (1.0 - e * e)
        * JULIAN_YEAR
        * 180.0
        / PI;
    println!(
        "Hulse–Taylor: simulated period {:.4} h (measured {:.4} h)",
        period / 3600.0,
        orbit.period / 3600.0
    );
    println!(
        "periastron advance: simulated {simulated:.6}°/yr, formula {formula:.6}°/yr, measured {:.6}°/yr",
        orbit.periastron_advance
    );
    let relative = (simulated / orbit.periastron_advance - 1.0).abs();
    println!("simulated vs measured: {relative:.1e}");
    assert!((period / orbit.period - 1.0).abs() < 1e-6);
    assert!(relative < 4.7e-4);
}
