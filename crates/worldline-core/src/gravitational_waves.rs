//! Gravitational waves from a binary at leading (quadrupole) order: the
//! power it radiates (and that power's first correction), how fast its
//! orbit shrinks, how long its two bodies have before they merge, and the
//! strain an observer records.
//!
//! Valid while the bodies move slowly and gravity between them is weak
//! (the same conditions as the post-Newtonian equations); corrections are
//! of relative order GM/(rc²). See `docs/physics/post-newtonian.md`.

use std::f64::consts::PI;

use glam::DVec3;

use crate::Body;
use crate::constants::{C, G};

/// The two polarizations of a gravitational wave's strain at an observer,
/// dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strain {
    /// h₊: stretches space along the first basis direction while squeezing
    /// it along the second.
    pub plus: f64,
    /// h×: the same, turned 45°.
    pub cross: f64,
}

/// The strain that `bodies` send to an observer `distance` (m) away in the
/// unit direction `toward` (from the bodies to the observer), at leading
/// (quadrupole) order: h_ij = (2G / c⁴R) d²I_ij/dt² projected across the
/// line of sight, with I_ij = Σ m x_i x_j about the center of mass and the
/// bodies' Newtonian accelerations (Einstein 1918; Blanchet 2024, section
/// "The quadrupole moment formalism").
///
/// The polarization basis: p = ẑ × N (normalized; x̂ if N is along ẑ) and
/// q = N × p, with ẑ the simulation's north (the ecliptic pole), so
/// h₊ = (h_pp − h_qq)/2 and h× = h_pq.
pub fn strain(bodies: &[Body], toward: DVec3, distance: f64) -> Strain {
    let total: f64 = bodies.iter().map(|b| b.gm).sum();
    let center: DVec3 = bodies.iter().map(|b| b.position * b.gm).sum::<DVec3>() / total;
    let drift: DVec3 = bodies.iter().map(|b| b.velocity * b.gm).sum::<DVec3>() / total;
    let n = toward.normalize();
    let p = DVec3::Z.cross(n).try_normalize().unwrap_or(DVec3::X);
    let q = n.cross(p);
    // d²I/dt² projected: Σ Gm (2 (v·e)(v·f) + (x·e)(a·f) + (a·e)(x·f)),
    // with Gm in place of m since h carries G.
    let second = |e: DVec3, f: DVec3| {
        bodies
            .iter()
            .enumerate()
            .map(|(i, b)| {
                let x = b.position - center;
                let v = b.velocity - drift;
                let a: DVec3 = bodies
                    .iter()
                    .enumerate()
                    .filter(|(j, _)| *j != i)
                    .map(|(_, o)| {
                        let d = o.position - b.position;
                        d * (o.gm / d.length().powi(3))
                    })
                    .sum();
                b.gm * (2.0 * v.dot(e) * v.dot(f) + x.dot(e) * a.dot(f) + a.dot(e) * x.dot(f))
            })
            .sum::<f64>()
    };
    let scale = 2.0 / (C.powi(4) * distance);
    Strain {
        plus: scale * 0.5 * (second(p, p) - second(q, q)),
        cross: scale * second(p, q),
    }
}

/// The power a pair of bodies radiates in gravitational waves at this
/// instant, in W: Einstein's quadrupole formula for two point masses,
/// F = (8/15) G³ m₁² m₂² (12 v² − 11 ṙ²) / (c⁵ r⁴), with the separation r,
/// relative speed v and its radial part ṙ. Kidder (1995), *Phys. Rev. D*
/// 52, 821, section "Energy Loss" (with G = c = 1 there).
pub fn quadrupole_power(gm1: f64, gm2: f64, r: f64, v: f64, rdot: f64) -> f64 {
    8.0 / 15.0 * gm1 * gm1 * gm2 * gm2 * (12.0 * v * v - 11.0 * rdot * rdot)
        / (G * C.powi(5) * r.powi(4))
}

/// The first correction to [`quadrupole_power`], GM/rc² of it, in W (to
/// add to it): Wagoner & Will (1976), as given by Kidder (1995), section
/// "Energy Loss", in harmonic coordinates, with ν = m₁m₂/(m₁+m₂)²:
///
/// (2/105) G³ m₁² m₂² / (c⁷ r⁴) { (785 − 852ν) v⁴ − 160 (17 − ν) (Gm/r) v²
///   + 8 (367 − 15ν) (Gm/r) ṙ² − 2 (1487 − 1392ν) v² ṙ²
///   + 3 (687 − 620ν) ṙ⁴ + 16 (1 − 4ν) (Gm/r)² }.
pub fn quadrupole_power_correction(gm1: f64, gm2: f64, r: f64, v: f64, rdot: f64) -> f64 {
    let gm = gm1 + gm2;
    let nu = gm1 * gm2 / (gm * gm);
    let (u, v2, rd2) = (gm / r, v * v, rdot * rdot);
    2.0 / 105.0 * gm1 * gm1 * gm2 * gm2 / (G * C.powi(7) * r.powi(4))
        * ((785.0 - 852.0 * nu) * v2 * v2 - 160.0 * (17.0 - nu) * u * v2
            + 8.0 * (367.0 - 15.0 * nu) * u * rd2
            - 2.0 * (1487.0 - 1392.0 * nu) * v2 * rd2
            + 3.0 * (687.0 - 620.0 * nu) * rd2 * rd2
            + 16.0 * (1.0 - 4.0 * nu) * u * u)
}

/// How the orbit's period `period` (s) of a bound pair with eccentricity
/// `e` shrinks, averaged over an orbit, in s/s (negative): Peters & Mathews
/// (1963), as given by Blanchet (2024), *Living Rev. Relativ.* 27, 4,
/// section "The quadrupole moment formalism":
///
/// dP/dt = −(192π/5c⁵) (2πG/P)^(5/3) m₁m₂ (m₁+m₂)^(−1/3)
///         (1 + 73e²/24 + 37e⁴/96) / (1 − e²)^(7/2).
pub fn period_derivative(gm1: f64, gm2: f64, period: f64, e: f64) -> f64 {
    let gm = gm1 + gm2;
    // G^(5/3) m₁m₂ m^(−1/3) = (Gm₁)(Gm₂)(Gm)^(−1/3).
    -192.0 * PI / (5.0 * C.powi(5))
        * (2.0 * PI / period).powf(5.0 / 3.0)
        * gm1
        * gm2
        * gm.powf(-1.0 / 3.0)
        * enhancement(e)
}

/// How the eccentricity falls, averaged over an orbit, in 1/s: Blanchet
/// (2024), same section,
///
/// de/dt = −(608π/15c⁵) (e/P) (2πG/P)^(5/3) m₁m₂ (m₁+m₂)^(−1/3)
///         (1 + 121e²/304) / (1 − e²)^(5/2).
pub fn eccentricity_derivative(gm1: f64, gm2: f64, period: f64, e: f64) -> f64 {
    let gm = gm1 + gm2;
    -608.0 * PI / (15.0 * C.powi(5)) * e / period
        * (2.0 * PI / period).powf(5.0 / 3.0)
        * gm1
        * gm2
        * gm.powf(-1.0 / 3.0)
        * (1.0 + 121.0 / 304.0 * e * e)
        / (1.0 - e * e).powf(2.5)
}

/// Peters and Mathews' eccentricity "enhancement" of the radiated power,
/// (1 + 73e²/24 + 37e⁴/96) / (1 − e²)^(7/2): about 12 for the Hulse–Taylor
/// binary.
pub fn enhancement(e: f64) -> f64 {
    let e2 = e * e;
    (1.0 + 73.0 / 24.0 * e2 + 37.0 / 96.0 * e2 * e2) / (1.0 - e2).powf(3.5)
}

/// How long until a bound pair merges, in s, if gravitational waves were
/// all that changed its orbit: Peters (1964), *Phys. Rev.* 136, B1224.
///
/// The period and eccentricity shrink together along Peters' curve
/// e²(1 + 121e²/304)^(145/121) / (1 − e²)^(19/6) = c₀ P^(19/9) (Blanchet
/// 2024, same section), so the time is ∫ de / |de/dt| along it, from `e`
/// down to a circle. For a circular orbit that is (3/8) P / |dP/dt|.
pub fn merger_time(gm1: f64, gm2: f64, period: f64, e: f64) -> f64 {
    assert!(
        (0.0..1.0).contains(&e),
        "only bound orbits (0 ≤ e < 1) merge"
    );
    let circular = 3.0 / 8.0 * period / -period_derivative(gm1, gm2, period, 0.0);
    if e < 1e-6 {
        // What eccentricity this small adds is below 10⁻¹¹.
        return circular;
    }
    let curve = |e: f64| {
        let e2 = e * e;
        e2 * (1.0 + 121.0 / 304.0 * e2).powf(145.0 / 121.0) / (1.0 - e2).powf(19.0 / 6.0)
    };
    let c0 = curve(e) / period.powf(19.0 / 9.0);
    let period_at = |e: f64| (curve(e) / c0).powf(9.0 / 19.0);
    // Near a circle 1/|de/dt| grows like e^(29/19). Substituting
    // e = e₀ s^k with k = 19/48 makes the integrand smooth in s, so
    // Simpson's rule converges quickly: de = k e₀ s^(k−1) ds, and the
    // integrand goes as s^(29k/19 + k − 1) = s⁰, finite at s = 0 (where it
    // is evaluated just above zero).
    const K: f64 = 19.0 / 48.0;
    let integrand = |s: f64| {
        let s = s.max(1e-12);
        let ei = e * s.powf(K);
        let de_ds = K * e * s.powf(K - 1.0);
        de_ds / -eccentricity_derivative(gm1, gm2, period_at(ei), ei)
    };
    let steps = 4096;
    let h = 1.0 / steps as f64;
    let mut sum = 0.0;
    for k in 0..=steps {
        let value = integrand(k as f64 * h);
        let weight = if k == 0 || k == steps {
            1.0
        } else if k % 2 == 1 {
            4.0
        } else {
            2.0
        };
        sum += weight * value;
    }
    sum * h / 3.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{GM_SUN, JULIAN_YEAR, PARSEC};
    use crate::orbit::{kepler_period, two_body_system};

    #[test]
    fn a_circular_pair_seen_face_on_and_edge_on() {
        // A circular pair in the x–y plane. Face-on (from +z), both
        // polarizations swing with the same amplitude 4G²m₁m₂/(c⁴Rr), a
        // quarter of a cycle apart, at twice the orbital frequency. Edge-on
        // (from +x), h₊ has half that amplitude and h× vanishes: the factors
        // (1 + cos²ι)/2 and cos ι of the textbook waveform.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let r = 1e6;
        let distance = 40e6 * PARSEC;
        let mut system = two_body_system(
            Body::new("one", gm1, 1.0),
            Body::new("two", gm2, 1.0),
            r,
            0.0,
        );
        let amplitude = 4.0 * gm1 * gm2 / (C.powi(4) * distance * r);
        let period = kepler_period(r, gm1 + gm2);
        let mut face = Vec::new();
        let mut edge = Vec::new();
        let omega = std::f64::consts::TAU / period;
        // Turn the pair through one orbit by hand (positions only matter).
        let start = system.bodies.clone();
        for k in 0..64 {
            let angle = omega * period * k as f64 / 64.0;
            let turn = glam::DMat3::from_rotation_z(angle);
            for (b, s) in system.bodies.iter_mut().zip(&start) {
                b.position = turn * s.position;
                b.velocity = turn * s.velocity;
            }
            face.push(strain(&system.bodies, DVec3::Z, distance));
            edge.push(strain(&system.bodies, DVec3::X, distance));
        }
        let peak = |values: &[f64]| values.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let plus: Vec<f64> = face.iter().map(|s| s.plus).collect();
        let cross: Vec<f64> = face.iter().map(|s| s.cross).collect();
        let edge_plus: Vec<f64> = edge.iter().map(|s| s.plus).collect();
        let edge_cross: Vec<f64> = edge.iter().map(|s| s.cross).collect();
        assert!((peak(&plus) / amplitude - 1.0).abs() < 1e-12);
        assert!((peak(&cross) / amplitude - 1.0).abs() < 1e-12);
        // Face-on the wave is circularly polarized: h₊² + h×² is constant.
        for s in &face {
            assert!(((s.plus.hypot(s.cross)) / amplitude - 1.0).abs() < 1e-12);
        }
        assert!((peak(&edge_plus) / (0.5 * amplitude) - 1.0).abs() < 1e-12);
        assert!(peak(&edge_cross) < 1e-12 * amplitude);
        // Twice the orbital frequency: h₊ changes sign 4 times an orbit.
        let flips = plus
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        assert_eq!(flips, 4);
    }

    #[test]
    fn a_circular_pair_merges_as_the_closed_form_says() {
        // For a circle Peters gives T = (5/256) c⁵ a⁴ / (G³ m₁ m₂ m), in
        // terms of the separation a; it must agree with (3/8) P/|dP/dt|.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let a = 1e9;
        let period = kepler_period(a, gm1 + gm2);
        let closed = 5.0 / 256.0 * C.powi(5) * a.powi(4) / (gm1 * gm2 * (gm1 + gm2));
        let ours = merger_time(gm1, gm2, period, 0.0);
        assert!((ours / closed - 1.0).abs() < 1e-12, "{ours} vs {closed}");
    }

    #[test]
    fn eccentricity_shortens_the_wait() {
        // A nearly circular orbit takes as long as a circle with the same
        // period, to order e²; an eccentric one radiates faster near
        // periastron and merges sooner.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let period = 8.0 * 3600.0;
        let circle = merger_time(gm1, gm2, period, 0.0);
        let nearly = merger_time(gm1, gm2, period, 1e-3);
        assert!((nearly / circle - 1.0).abs() < 1e-5, "{}", nearly / circle);
        let eccentric = merger_time(gm1, gm2, period, 0.6);
        assert!(eccentric < 0.5 * circle);
        println!(
            "8-hour orbit: merges in {:.0} Myr as a circle, {:.0} Myr at e = 0.6",
            circle / JULIAN_YEAR / 1e6,
            eccentric / JULIAN_YEAR / 1e6
        );
    }
}
