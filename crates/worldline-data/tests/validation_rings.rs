//! Validation: Saturn's rings.
//!
//! 1. The measured Cassini profile must agree with the published ring
//!    boundaries: the B ring's sharp outer edge, the near-empty Cassini
//!    Division and Encke Gap, the opaque core of the B ring.
//! 2. The ring plane's orientation (IAU pole) plus the simulated orbits must
//!    reproduce 2025's ring-plane crossings: the rings edge-on to Earth on
//!    23 March and Saturn's equinox (the Sun crossing the ring plane) on
//!    6 May.

use worldline_core::constants::DAY;
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::integrator::{Ias15, advance};
use worldline_data::{
    RingFeature, rotation_model, saturn_ring_features, saturn_rings, solar_system,
};

fn feature(name: &str) -> RingFeature {
    saturn_ring_features()
        .into_iter()
        .find(|f| f.name == name)
        .unwrap()
}

/// Mean optical depth of the profile between two radii (m).
fn mean_tau(inner: f64, outer: f64) -> f64 {
    let profile = saturn_rings();
    let values: Vec<f64> = (0..profile.tau.len())
        .map(|i| {
            (
                profile.inner + (i as f64 + 0.5) * profile.bin,
                profile.tau[i],
            )
        })
        .filter(|(r, _)| (inner..outer).contains(r))
        .map(|(_, t)| t)
        .collect();
    values.iter().sum::<f64>() / values.len() as f64
}

#[test]
fn measured_profile_matches_the_published_ring_boundaries() {
    let profile = saturn_rings();

    // The B ring's outer edge. Its outermost zone dips below τ = 1 here and
    // there, so walk inward instead: start in the empty Huygens Gap just
    // outside the B ring and stop at the first opaque bin (τ > 1).
    let b_ring = feature("B Ring");
    let huygens = feature("Huygens Gap");
    let mut radius = huygens.inner + 200e3;
    while profile.tau_at(radius) < 1.0 {
        radius -= profile.bin;
    }
    println!(
        "B ring outer edge: measured {:.0} km, PDS table {:.0} km",
        radius / 1e3,
        b_ring.outer / 1e3
    );
    // The edge isn't round: Mimas's 2:1 resonance pulls it into an oval
    // whose radius swings about ±70 km around the planet (Porco et al. 1984;
    // Spitale & Porco 2010). One occultation samples one longitude, so it can
    // land that far either side of the tabulated edge.
    assert!((radius - b_ring.outer).abs() < 100e3);

    let cassini = feature("Cassini Division");
    let encke = feature("Encke Gap");
    let b3 = feature("Region B3");
    let a_ring = feature("A Ring");
    let cassini_tau = mean_tau(cassini.inner + 100e3, cassini.outer - 100e3);
    // Skip the edge bins, which blend gap and ring.
    let encke_tau = mean_tau(
        encke.inner + 2.0 * profile.bin,
        encke.outer - 2.0 * profile.bin,
    );
    let b3_tau = mean_tau(b3.inner, b3.outer);
    let a_tau = mean_tau(a_ring.inner + 100e3, encke.inner - 100e3);
    println!(
        "mean optical depth: Cassini Division {cassini_tau:.3} (PDS: 0 to 0.2), \
         Encke Gap {encke_tau:.3} (~0), B ring core {b3_tau:.2} (1 to 5, median 3.6), \
         A ring {a_tau:.2} (0.4 to 1)"
    );
    assert!(cassini_tau < 0.3);
    assert!(encke_tau < 0.05);
    assert!(b3_tau > 3.0);
    assert!((0.4..1.0).contains(&a_tau));
}

#[test]
fn rings_turn_edge_on_on_the_published_dates_in_2025() {
    // Track the angle between Saturn's ring plane and the directions to
    // Earth and to the Sun through the first half of 2025, and find when
    // each crosses zero.
    let snapshot = solar_system();
    let model = rotation_model("Saturn").unwrap();
    let mut system = snapshot.system();
    let (sun, earth, saturn) = (0, 3, 7);
    let mut ias = Ias15::new();

    let elevation = |system: &worldline_core::System, from: usize, jd: f64| {
        let pole = model.orientation(jd).north_pole();
        let direction = (system.bodies[from].position - system.bodies[saturn].position).normalize();
        direction.dot(pole).asin().to_degrees()
    };

    let mut previous: Option<(f64, f64, f64)> = None;
    let (mut earth_crossing, mut sun_crossing) = (None, None);
    for _ in 0..=180 {
        let jd = snapshot.epoch_jd_tdb + system.time() / DAY;
        let (e, s) = (elevation(&system, earth, jd), elevation(&system, sun, jd));
        if let Some((jd0, e0, s0)) = previous {
            // Linear interpolation inside the day the sign changed.
            if e0.signum() != e.signum() {
                earth_crossing = Some(jd0 + e0 / (e0 - e));
            }
            if s0.signum() != s.signum() {
                sun_crossing = Some(jd0 + s0 / (s0 - s));
            }
        }
        previous = Some((jd, e, s));
        advance(&mut system, &EinsteinInfeldHoffmann, &mut ias, DAY);
    }

    // Julian Dates of 2025-03-23 00:00 and 2025-05-06 00:00.
    let (earth_published, sun_published) = (2_460_757.5, 2_460_801.5);
    let earth_crossing = earth_crossing.expect("rings turn edge-on to Earth in early 2025");
    let sun_crossing = sun_crossing.expect("Saturn's equinox happens in 2025");
    println!(
        "Rings edge-on to Earth: {:+.2} days from 23 March 2025; Saturn's equinox: {:+.2} days from 6 May 2025",
        earth_crossing - earth_published,
        sun_crossing - sun_published
    );
    // Published dates are given to the day.
    assert!((earth_crossing - earth_published).abs() < 1.5);
    assert!((sun_crossing - sun_published).abs() < 1.5);
}
