//! Black holes merging: what numerical relativity says two black holes
//! that spiral together leave behind. No formula gives the last orbits and
//! the merger itself; supercomputer simulations do, and published fits to
//! hundreds of them give the final hole's mass and spin, its recoil, and
//! the tone it rings down with. See `docs/physics/mergers.md`.

use std::f64::consts::PI;

use glam::DVec3;

use crate::Body;
use crate::compact::{horizon_radius, horizon_spin};
use crate::constants::C;
use crate::gravity::center_of_mass;

/// What two black holes that spiral together leave: the final hole and
/// how it got there.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Remnant {
    /// The fraction of the pair's total mass radiated away as
    /// gravitational waves.
    pub radiated: f64,
    /// The final hole's spin, a = cJ/(GM²), from 0 to 1.
    pub spin: f64,
    /// Its recoil speed, in m/s: the waves carry off momentum unevenly
    /// when the masses differ.
    pub kick: f64,
}

/// The fraction of the total mass that two non-spinning black holes on a
/// quasi-circular orbit radiate as gravitational waves, from inspiral
/// through merger and ringdown, with ν = m₁m₂/(m₁+m₂)² (0 to 1/4).
///
/// Jiménez-Forteza, Keitel, Husa, Hannam, Khan & Pürrer (2017), *Phys. Rev.
/// D* 95, 064024 ("UIB2016"), their fit without spins, calibrated to 92
/// non-spinning simulations (root-mean-square error 4.14 × 10⁻⁵):
/// E = (1 − 2√2/3) ν + a₂ν² + a₃ν³ + a₄ν⁴, coefficients at the precision of
/// the paper's supplementary material (its Table VII rounds them). The linear
/// term is the binding energy of the innermost stable circular orbit of a
/// small body: what it radiates spiraling in before it plunges.
pub fn radiated_fraction(nu: f64) -> f64 {
    const A2: f64 = 0.560_990_413_531_337_4;
    const A3: f64 = -0.846_675_637_644_04;
    const A4: f64 = 3.145_145_224_278_187;
    let linear = 1.0 - 2.0 * 2f64.sqrt() / 3.0;
    nu * (linear + nu * (A2 + nu * (A3 + nu * A4)))
}

/// The spin a = cJ/(GM²) of the hole that two non-spinning black holes on
/// a quasi-circular orbit leave, with ν = m₁m₂/(m₁+m₂)².
///
/// Jiménez-Forteza et al. (2017), their fit without spins (root-mean-square
/// error 9.41 × 10⁻⁵): a = (2√3 ν + 5.24 a₂ν² + 1.3 a₃ν³)/(1 + 2.88 a₅ν),
/// coefficients at the precision of the supplementary material (its Table I
/// rounds them). The
/// linear term is the orbital angular momentum of a small body at the
/// innermost stable circular orbit, which it carries in.
pub fn final_spin(nu: f64) -> f64 {
    const A2: f64 = 3.832_634_161_870_857_7;
    const A3: f64 = -9.487_364_155_598_392;
    const A5: f64 = 2.513_487_514_564_837_4;
    (2.0 * 3f64.sqrt() * nu + 5.24 * A2 * nu * nu + 1.3 * A3 * nu.powi(3)) / (1.0 + 2.88 * A5 * nu)
}

/// The recoil speed (m/s) of the hole two non-spinning black holes on a
/// quasi-circular orbit leave, with ν = m₁m₂/(m₁+m₂)²: zero for equal
/// masses, by symmetry, and at most 175 km/s.
///
/// González, Sperhake, Brügmann, Hannam & Husa (2007), *Phys. Rev. Lett.*
/// 98, 091101, a fit to 30 simulations in the form Fitchett (1983) derived:
/// v = A ν² √(1 − 4ν) (1 + Bν), with A = 1.20 × 10⁴ km/s and B = −0.93.
pub fn kick_speed(nu: f64) -> f64 {
    const A: f64 = 1.20e7;
    const B: f64 = -0.93;
    A * nu * nu * (1.0 - 4.0 * nu).max(0.0).sqrt() * (1.0 + B * nu)
}

/// What two non-spinning black holes of gravitational parameters `gm1`
/// and `gm2` that spiral together leave.
pub fn remnant(gm1: f64, gm2: f64) -> Remnant {
    let gm = gm1 + gm2;
    let nu = (gm1 * gm2 / (gm * gm)).clamp(0.0, 0.25);
    Remnant {
        radiated: radiated_fraction(nu),
        spin: final_spin(nu),
        kick: kick_speed(nu),
    }
}

/// How a black hole rings when disturbed: its fundamental quasi-normal
/// mode (l = m = 2, n = 0), the tone a merger's final hole rings down
/// with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ringdown {
    /// The wave's frequency, Hz.
    pub frequency: f64,
    /// The time over which its amplitude falls by a factor e, s.
    pub damping: f64,
    /// Its quality factor Q = π f τ: about how many cycles it rings for.
    pub quality: f64,
}

/// The fundamental ringdown of a black hole of gravitational parameter
/// `gm` (m³/s²) and spin `spin` (0 to 0.99).
///
/// Berti, Cardoso & Will (2006), *Phys. Rev. D* 73, 064030, Table VIII,
/// fits to the exactly computed modes (largest error over spins 0 to 0.99:
/// 1.85% in frequency, 0.88% in quality factor): G M ω/c³ = f₁ + f₂(1 − a)^f₃
/// and Q = q₁ + q₂(1 − a)^q₃.
pub fn ringdown(gm: f64, spin: f64) -> Ringdown {
    let (f1, f2, f3) = (1.5251, -1.1568, 0.1292);
    let (q1, q2, q3) = (0.7000, 1.4187, -0.4990);
    let s = (1.0 - spin).max(0.0);
    let omega = (f1 + f2 * s.powf(f3)) * C.powi(3) / gm;
    let quality = q1 + q2 * s.powf(q3);
    let frequency = omega / (2.0 * PI);
    Ringdown {
        frequency,
        damping: quality / (PI * frequency),
        quality,
    }
}

/// A black-hole merger as it happened in the simulation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Merger {
    /// The gravitational parameter G m carried off by the waves, m³/s².
    pub radiated_gm: f64,
    /// The final hole's spin.
    pub spin: f64,
    /// The final hole's recoil, m/s, relative to the pair's center of
    /// mass.
    pub kick: DVec3,
    /// The larger of the two holes' own spins, which the fits leave out
    /// (they are for holes that don't spin).
    pub spins_left_out: f64,
}

/// Two black holes that have spiraled together, merged as numerical
/// relativity says: the final hole has the pair's mass less what the
/// waves carried off, the fitted spin (its horizon sized to match), and
/// sits at their center of mass (see [`center_of_mass`]), moving with it
/// plus the recoil. The waves carry off the rest of the momentum: the
/// radiated mass's share of the pair's motion, and the recoil's opposite.
///
/// The recoil lies in the orbital plane; which way within it depends on
/// the last orbit's phase at the merger, which isn't simulated, so it is
/// taken along the heavier hole's motion. The final hole keeps the
/// survivor's name.
pub fn merge_black_holes(survivor: &Body, absorbed: &Body) -> (Body, Merger) {
    let gm = survivor.gm + absorbed.gm;
    let (center, drift) = center_of_mass(survivor, absorbed);
    let fit = remnant(survivor.gm, absorbed.gm);
    let heavier = if survivor.gm >= absorbed.gm {
        survivor
    } else {
        absorbed
    };
    let along = heavier.velocity - drift;
    let direction = if along.length_squared() > 0.0 {
        along.normalize()
    } else {
        // At rest relative to each other: no orbit, no preferred direction
        // but the line between them.
        (survivor.position - absorbed.position).normalize_or_zero()
    };
    let kick = direction * fit.kick;
    let final_gm = gm * (1.0 - fit.radiated);
    let spins_left_out =
        horizon_spin(survivor.gm, survivor.radius).max(horizon_spin(absorbed.gm, absorbed.radius));
    (
        Body {
            name: survivor.name.clone(),
            gm: final_gm,
            radius: horizon_radius(final_gm, fit.spin),
            position: center,
            velocity: drift + kick,
        },
        Merger {
            radiated_gm: gm * fit.radiated,
            spin: fit.spin,
            kick,
            spins_left_out,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compact::{is_black_hole, schwarzschild_radius};
    use crate::constants::GM_SUN;

    #[test]
    fn equal_masses_match_the_most_accurate_simulation() {
        // Scheel et al. (2009), Phys. Rev. D 79, 024003: two equal,
        // non-spinning holes, 16 orbits through ringdown, leave
        // M_f/M = 0.95162 ± 0.00002 and spin 0.68646 ± 0.00004. The fits
        // must match within their own root-mean-square error over the
        // non-spinning simulations: 4.14 × 10⁻⁵ and 9.41 × 10⁻⁵.
        let mass = 1.0 - radiated_fraction(0.25);
        let spin = final_spin(0.25);
        println!(
            "equal masses: M_f/M = {mass:.6} (Scheel et al. 0.95162, off by {:.1e}), spin {spin:.6} (0.68646, off by {:.1e})",
            (mass - 0.95162).abs(),
            (spin - 0.68646).abs()
        );
        assert!((mass - 0.95162).abs() < 4.14e-5);
        assert!((spin - 0.68646).abs() < 9.41e-5);
        // No recoil when the masses are equal.
        assert_eq!(kick_speed(0.25), 0.0);
    }

    #[test]
    fn a_small_hole_falling_in_carries_its_last_stable_orbit() {
        // A body much lighter than the hole spirals in to the innermost
        // stable circular orbit of a non-spinning hole, 6 GM/c², having
        // radiated that orbit's binding energy, 1 − √(8/9) of its mass, and
        // carries in that orbit's angular momentum, √12 G M m/c. The fits
        // hold both limits; the next order is ν times a coefficient, a₂ =
        // 0.56 for the energy and 5.24 a₂ − 2.88 a₅ √12 = −5.0 for the spin.
        // The test allows twice that.
        let nu = 1e-6;
        let energy = radiated_fraction(nu) / nu;
        let spin = final_spin(nu) / nu;
        println!("ν = 1e-6: radiated {energy:.8} ν, spin {spin:.8} ν");
        assert!((energy - (1.0 - (8.0f64 / 9.0).sqrt())).abs() < 2.0 * 0.56 * nu);
        assert!((spin - 12f64.sqrt()).abs() < 2.0 * 5.0 * nu);
    }

    #[test]
    fn the_largest_kick_is_175_km_s() {
        // González et al. (2007) find the largest recoil from non-spinning
        // holes, 175.2 ± 11 km/s, at ν = 0.195 ± 0.005 (masses 0.36 to 1).
        let (nu, speed) = (0..=25_000)
            .map(|k| k as f64 * 1e-5)
            .map(|nu| (nu, kick_speed(nu)))
            .fold((0.0, 0.0f64), |best, p| if p.1 > best.1 { p } else { best });
        println!("largest kick {:.1} km/s at ν = {nu:.4}", speed / 1e3);
        assert!((speed / 1e3 - 175.2).abs() < 11.0);
        assert!((nu - 0.195).abs() < 0.005);
    }

    #[test]
    fn the_ringdown_fit_matches_the_computed_modes() {
        // Berti, Cardoso & Will (2006), Table II: G M ω/c³ (real part, and
        // the damping rate) of the l = m = 2, n = 0 mode, computed exactly,
        // printed to 4 digits. The fit must match within its stated largest
        // error (1.85% in frequency, 0.88% in quality factor), plus the
        // table's rounding (half a unit in the last digit of each part).
        let table = [
            (0.00, 0.3737, 0.0890),
            (0.10, 0.3870, 0.0887),
            (0.20, 0.4021, 0.0883),
            (0.30, 0.4195, 0.0877),
            (0.40, 0.4398, 0.0869),
            (0.50, 0.4641, 0.0856),
            (0.60, 0.4940, 0.0838),
            (0.70, 0.5326, 0.0808),
            (0.80, 0.5860, 0.0756),
            (0.90, 0.6716, 0.0649),
            (0.98, 0.8254, 0.0386),
        ];
        // A hole with G M/c³ = 1 s, so its frequencies are G M ω/c³ in s⁻¹.
        let gm = C.powi(3);
        for (spin, real, imaginary) in table {
            let fit = ringdown(gm, spin);
            let omega = 2.0 * PI * fit.frequency;
            let quality = real / (2.0 * imaginary);
            let (f_miss, q_miss) = (
                (omega / real - 1.0).abs(),
                (fit.quality / quality - 1.0).abs(),
            );
            let rounding = (0.00005 / real, 0.00005 / real + 0.00005 / imaginary);
            println!(
                "a = {spin:.2}: G M ω/c³ {omega:.4} vs {real} (off {:.2}%), Q {:.3} vs {quality:.3} (off {:.2}%)",
                100.0 * f_miss,
                fit.quality,
                100.0 * q_miss
            );
            assert!(f_miss < 0.0185 + rounding.0);
            assert!(q_miss < 0.0088 + rounding.1);
        }
    }

    #[test]
    fn a_merger_keeps_the_center_of_mass_and_kicks_in_the_orbital_plane() {
        // Two non-spinning holes of 30 and 10 Suns, 300 km apart in the x–y
        // plane, circling, the whole pair drifting at 1 km/s along z.
        let (gm1, gm2) = (30.0 * GM_SUN, 10.0 * GM_SUN);
        let gm = gm1 + gm2;
        let r = 3e5;
        let speed = (gm / r).sqrt();
        let drift = DVec3::new(0.0, 0.0, 1e3);
        let heavy = Body::new("heavy", gm1, schwarzschild_radius(gm1))
            .at(DVec3::new(-r * gm2 / gm, 0.0, 0.0))
            .moving(DVec3::new(0.0, -speed * gm2 / gm, 0.0) + drift);
        let light = Body::new("light", gm2, schwarzschild_radius(gm2))
            .at(DVec3::new(r * gm1 / gm, 0.0, 0.0))
            .moving(DVec3::new(0.0, speed * gm1 / gm, 0.0) + drift);
        let (hole, merger) = merge_black_holes(&light, &heavy);
        let nu = gm1 * gm2 / (gm * gm);
        println!(
            "30 + 10 Suns: {:.3} Suns radiated, spin {:.4}, kick {:.1} km/s",
            merger.radiated_gm / GM_SUN,
            merger.spin,
            merger.kick.length() / 1e3
        );
        assert_eq!(hole.name, "light");
        assert!(is_black_hole(&hole));
        assert!((hole.gm + merger.radiated_gm - gm).abs() <= f64::EPSILON * gm);
        // At their center of mass (to second post-Newtonian order: at
        // GM/rc² = 0.2 it is a few km from the mass-weighted one).
        let (center, motion) = center_of_mass(&light, &heavy);
        assert_eq!(hole.position, center);
        // Its spin is in its horizon's size.
        assert!((horizon_spin(hole.gm, hole.radius) - final_spin(nu)).abs() < 1e-12);
        assert_eq!(merger.spins_left_out, 0.0);
        // The recoil: the fitted speed on top of the center's motion (which
        // carries the drift), in the orbit's plane, along the heavier
        // hole's motion relative to the center.
        let kick = hole.velocity - motion;
        assert!((motion.z - drift.z).abs() < 1e-9);
        assert!((kick.length() / kick_speed(nu) - 1.0).abs() < 1e-12);
        assert!(kick.z.abs() < 1e-9);
        let along = (heavy.velocity - motion).normalize();
        assert!((kick.normalize() - along).length() < 1e-12);
    }
}
