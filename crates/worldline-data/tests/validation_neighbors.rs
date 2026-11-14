//! Step 7.1: the nearest and brightest stars as full bodies. α Centauri A and
//! B circle each other with their measured period, and Barnard's Star
//! crosses the sky at its measured rate.

use worldline_core::constants::{AU, JULIAN_YEAR};
use worldline_core::gravity::Newtonian;
use worldline_core::hierarchy::Hierarchy;
use worldline_core::{Body, System};
use worldline_data::{julian_year, neighbor_orbit, neighbors};

/// J2000.0, as a Julian Date.
const J2000: f64 = 2_451_545.0;

/// A milliarcsecond, in radians.
const MAS: f64 = std::f64::consts::TAU / (360.0 * 3600.0 * 1000.0);

/// The stars at Julian Date `jd`, as a region of their own: Newtonian
/// gravity, all on all.
fn region(jd: f64) -> Hierarchy {
    let bodies: Vec<Body> = neighbors(jd).into_iter().map(|n| n.body).collect();
    let mut h = Hierarchy::new(System::new(bodies), Box::new(Newtonian), Vec::new());
    h.collisions = true;
    h
}

fn index(h: &Hierarchy, name: &str) -> usize {
    h.top
        .bodies
        .iter()
        .position(|b| b.name == name)
        .expect("in the region")
}

#[test]
fn alpha_centauri_a_and_b_go_round_in_their_measured_period() {
    // Kervella et al. (2016): 79.929 ± 0.013 years. The simulated period
    // follows from the published semi-major axis (17.592″), parallax (747.17
    // mas) and masses (1.1055 and 0.9373 Suns) through Kepler's third law,
    // P ∝ (a/ϖ)^(3/2) M^(−1/2). Those were fitted with the period, so they
    // agree with it to the rounding of their printed digits. The limit is
    // the period's own uncertainty plus that rounding.
    let mut h = region(J2000 + 25.0 * 365.25);
    let (a, b) = (index(&h, "Alpha Centauri A"), index(&h, "Alpha Centauri B"));
    let separation = |h: &Hierarchy| (h.top.bodies[a].position - h.top.bodies[b].position).length();
    // Times of closest approach, each found from the three steps around it
    // by a parabola.
    let mut closest = Vec::new();
    let mut last: [(f64, f64); 3] = [(0.0, f64::MAX); 3];
    let end = 3.2 * 79.929 * JULIAN_YEAR;
    while h.time() < end {
        h.step((end - h.time()).min(0.02 * JULIAN_YEAR));
        last = [last[1], last[2], (h.time(), separation(&h))];
        if last[1].1 < last[0].1 && last[1].1 < last[2].1 {
            let ((t0, s0), (t1, s1), (t2, s2)) = (last[0], last[1], last[2]);
            let (d0, d2) = (t0 - t1, t2 - t1);
            let curve = ((s0 - s1) / d0 - (s2 - s1) / d2) / (d0 - d2);
            let slope = (s0 - s1) / d0 - curve * d0;
            closest.push(t1 - slope / (2.0 * curve));
        }
    }
    let periods: Vec<f64> = closest
        .windows(2)
        .map(|w| (w[1] - w[0]) / JULIAN_YEAR)
        .collect();
    let period = periods.iter().sum::<f64>() / periods.len() as f64;
    let orbit = neighbor_orbit("Alpha Centauri A").expect("in the data");
    let rounding = 1.5 * (0.0005 / 17.592) + 1.5 * (0.005 / 747.17) + 0.5 * (0.0001 / 2.0428);
    let limit = 0.013 + rounding * orbit.period;
    println!(
        "periastrons at {:?} (Julian years); period {period:.3} years, measured {:.3} ± 0.013; limit {limit:.3}",
        closest
            .iter()
            .map(|t| (julian_year(J2000 + 25.0 * 365.25) + t / JULIAN_YEAR).round())
            .collect::<Vec<_>>(),
        orbit.period,
    );
    // The first periastron since the start: 1955.604 + 79.929.
    let first = julian_year(J2000 + 25.0 * 365.25) + closest[0] / JULIAN_YEAR;
    println!(
        "first periastron {first:.3}, the orbit's {:.3}",
        orbit.periastron + orbit.period
    );
    assert!(periods.len() >= 2);
    assert!((period - orbit.period).abs() < limit, "{period}");
}

#[test]
fn alpha_centauri_b_is_where_it_was_seen() {
    // Kervella et al. (2016), Table 5: B from A at 2014.5426 (ALMA, which
    // their orbit weights most) and 2016.1893 (NACO). This checks the
    // orbit's elements are read as published: a different convention for
    // the node or the inclination would put B elsewhere.
    let orbit = neighbor_orbit("Alpha Centauri A").expect("in the data");
    for (year, angle, angle_error, separation, separation_error) in [
        (2014.5426, 282.91, 0.15, 4.208, 0.01),
        (2016.1893, 305.19, 0.30, 4.013, 0.02),
    ] {
        let (_, r, _) = orbit.relative(year);
        // North and east, in arcseconds.
        let arcsec = |m: f64| m / AU * orbit.parallax / 1000.0;
        let (north, east) = (arcsec(r.x), arcsec(r.y));
        let theta = east.atan2(north).to_degrees().rem_euclid(360.0);
        let rho = north.hypot(east);
        println!(
            "{year}: position angle {theta:.2}° ({angle} ± {angle_error}), separation {rho:.3}\" ({separation} ± {separation_error})"
        );
        // Three standard deviations of the measurement.
        assert!((theta - angle).abs() < 3.0 * angle_error, "{theta}");
        assert!((rho - separation).abs() < 3.0 * separation_error, "{rho}");
    }
}

#[test]
fn barnards_star_crosses_the_sky_at_its_measured_rate() {
    // Gaia DR3 measured its proper motion at 2016.0: (−801.551, 10362.394)
    // mas/yr, to 0.03 and 0.04. Over the simulation's first years it should
    // move as fast across the sky, seen from the solar system's barycenter,
    // plus how its approach speeds it up: dμ/dt = −2 μ v_r / d, about 1.3
    // mas/yr each year.
    let star = neighbors(J2000 + 16.0 * 365.25)
        .into_iter()
        .find(|n| n.body.name == "Barnard's Star")
        .expect("in the data");
    let (pmra, pmdec) = star.proper_motion;
    let (era, edec) = star.proper_motion_error.expect("Gaia gives them");
    let measured = pmra.hypot(pmdec);
    let error = (pmra * era).hypot(pmdec * edec) / measured;
    let distance = star.body.position.length();
    let speedup = -2.0 * measured * star.radial_velocity / distance * JULIAN_YEAR;
    // From the start, 2016.0, for a year, the rate measured over each
    // tenth of it.
    let mut h = region(J2000 + 16.0 * 365.25);
    let k = index(&h, "Barnard's Star");
    let direction = |h: &Hierarchy| h.top.bodies[k].position.normalize();
    let mut worst: f64 = 0.0;
    let mut before = direction(&h);
    for tenth in 1..=10 {
        let end = tenth as f64 * 0.1 * JULIAN_YEAR;
        while h.time() < end {
            h.step(end - h.time());
        }
        let now = direction(&h);
        // atan2 of the cross and dot products: acos of the dot product alone
        // loses four parts in a million at five microradians.
        let angle = now.cross(before).length().atan2(now.dot(before));
        let rate = angle / MAS / 0.1;
        // The rate halfway through the tenth.
        let wanted = measured + speedup * (tenth as f64 - 0.5) * 0.1;
        worst = worst.max((rate - wanted).abs());
        if tenth == 1 || tenth == 10 {
            println!(
                "{:.2}: {rate:.3} mas/yr, expected {wanted:.3} (Gaia {measured:.3} ± {error:.3} at 2016.0, \
                 speeding up {speedup:.3} a year)",
                2016.0 + (tenth as f64 - 0.5) * 0.1
            );
        }
        before = now;
    }
    println!("worst {worst:.4} mas/yr, against Gaia's {error:.3}");
    assert!(worst < error, "{worst}");
    // A distant tug wouldn't do this: the next nearest star pulls on it at
    // under 10⁻¹² m/s².
    let nearest = h
        .top
        .bodies
        .iter()
        .filter(|b| b.name != "Barnard's Star")
        .map(|b| b.gm / (b.position - h.top.bodies[k].position).length_squared())
        .fold(0.0, f64::max);
    println!("strongest pull on it {nearest:.1e} m/s²");
    assert!(nearest < 1e-12);
}
