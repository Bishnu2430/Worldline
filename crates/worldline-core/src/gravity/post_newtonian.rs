use glam::DVec3;

use super::{EinsteinInfeldHoffmann, Gravity};
use crate::Body;
use crate::constants::C;

/// The parts of one body's acceleration, caused by another, beyond first
/// post-Newtonian order, in m/s².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairTerms {
    /// Second post-Newtonian order (2PN): conservative, of relative size
    /// (GM/rc²)² next to Newton's pull.
    pub second: DVec3,
    /// Radiation reaction (2.5PN): the pull that drains the pair's energy
    /// into gravitational waves, of relative size (GM/rc²)^(5/2).
    pub reaction: DVec3,
}

/// The 2PN and 2.5PN parts of body 1's acceleration due to body 2, from
/// their gravitational parameters `gm1`, `gm2` (m³/s²), positions `y1`,
/// `y2` and velocities `v1`, `v2`, in harmonic coordinates, in any frame.
///
/// The c⁻⁴ and c⁻⁵ terms of the two-body equations of motion: Blanchet
/// (2024), *Living Rev. Relativ.* 27, 4, section "The 3.5PN acceleration
/// and 3PN energy" (accelerations already reduced to positions and
/// velocities). Body 2's come from swapping the labels.
pub fn pair_terms(gm1: f64, gm2: f64, y1: DVec3, y2: DVec3, v1: DVec3, v2: DVec3) -> PairTerms {
    let d = y1 - y2;
    let r = d.length();
    let n = d / r;
    let v12 = v1 - v2;
    let (nv1, nv2, nv12) = (n.dot(v1), n.dot(v2), n.dot(v12));
    let (v1s, v2s, v1v2, v12s) = (
        v1.length_squared(),
        v2.length_squared(),
        v1.dot(v2),
        v12.length_squared(),
    );
    let (r2, r3, r4) = (r * r, r * r * r, r * r * r * r);
    // Products of the parameters: G²m₁m₂ = gm₁ gm₂ and so on.
    let (m12, m22, m112, m122, m222) = (
        gm1 * gm2,
        gm2 * gm2,
        gm1 * gm1 * gm2,
        gm1 * gm2 * gm2,
        gm2 * gm2 * gm2,
    );

    // 2PN, along n₁₂ and along v₁₂.
    let along_n = -57.0 / 4.0 * m112 / r4 - 69.0 / 2.0 * m122 / r4 - 9.0 * m222 / r4
        + gm2 / r2
            * (-15.0 / 8.0 * nv2.powi(4) + 1.5 * nv2 * nv2 * v1s
                - 6.0 * nv2 * nv2 * v1v2
                - 2.0 * v1v2 * v1v2
                + 4.5 * nv2 * nv2 * v2s
                + 4.0 * v1v2 * v2s
                - 2.0 * v2s * v2s)
        + m12 / r3
            * (19.5 * nv1 * nv1 - 39.0 * nv1 * nv2 + 8.5 * nv2 * nv2 - 3.75 * v1s - 2.5 * v1v2
                + 1.25 * v2s)
        + m22 / r3 * (2.0 * nv1 * nv1 - 4.0 * nv1 * nv2 - 6.0 * nv2 * nv2 - 8.0 * v1v2 + 4.0 * v2s);
    let along_v = m22 / r3 * (-2.0 * nv1 - 2.0 * nv2)
        + m12 / r3 * (-63.0 / 4.0 * nv1 + 55.0 / 4.0 * nv2)
        + gm2 / r2
            * (-6.0 * nv1 * nv2 * nv2 + 4.5 * nv2.powi(3) + nv2 * v1s - 4.0 * nv1 * v1v2
                + 4.0 * nv2 * v1v2
                + 4.0 * nv1 * v2s
                - 5.0 * nv2 * v2s);
    let second = (n * along_n + v12 * along_v) / C.powi(4);

    // 2.5PN.
    let along_n =
        (208.0 / 15.0 * m122 / r4 - 24.0 / 5.0 * m112 / r4 + 12.0 / 5.0 * m12 / r3 * v12s) * nv12;
    let along_v = 8.0 / 5.0 * m112 / r4 - 32.0 / 5.0 * m122 / r4 - 4.0 / 5.0 * m12 / r3 * v12s;
    let reaction = (n * along_n + v12 * along_v) / C.powi(5);

    PairTerms { second, reaction }
}

/// Relativistic N-body gravity for close pairs, such as two neutron stars:
/// the Einstein–Infeld–Hoffmann equations (first post-Newtonian order) for
/// every body, plus each pair's second-order (2PN) and radiation-reaction
/// (2.5PN) terms, through which a binary loses energy to gravitational
/// waves and spirals in.
///
/// The pairwise terms are exact for two bodies. With more, the 2PN terms
/// that couple three bodies are left out; they matter only when a third
/// body sits close to a compact pair. Adding pairwise terms to N-body
/// gravity is how N-body codes treat compact binaries (Kupi, Amaro-Seoane
/// & Spurzem 2006, *MNRAS* 371, L45; Mikkola & Merritt 2008, *AJ* 135,
/// 2398). For ordinary orbits the terms are below double precision, so the
/// solar system moves as under the first-order equations alone.
///
/// See `docs/physics/post-newtonian.md`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PostNewtonian {
    /// Leave out the radiation reaction, keeping only the conservative
    /// part (for checks that energy is conserved).
    pub conservative: bool,
}

impl PostNewtonian {
    /// The conservative part alone: everything but the radiation reaction.
    pub fn conservative() -> Self {
        Self { conservative: true }
    }
}

impl Gravity for PostNewtonian {
    fn name(&self) -> &'static str {
        if self.conservative {
            "Einstein–Infeld–Hoffmann (1PN) with 2PN between pairs"
        } else {
            "Einstein–Infeld–Hoffmann (1PN) with 2PN and gravitational-wave losses (2.5PN) between pairs"
        }
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        EinsteinInfeldHoffmann.accelerations(time, bodies, out);
        for (i, bi) in bodies.iter().enumerate() {
            for (j, bj) in bodies.iter().enumerate().skip(i + 1) {
                if bi.gm == 0.0 && bj.gm == 0.0 {
                    continue;
                }
                let on_i = pair_terms(
                    bi.gm,
                    bj.gm,
                    bi.position,
                    bj.position,
                    bi.velocity,
                    bj.velocity,
                );
                let on_j = pair_terms(
                    bj.gm,
                    bi.gm,
                    bj.position,
                    bi.position,
                    bj.velocity,
                    bi.velocity,
                );
                out[i] += on_i.second;
                out[j] += on_j.second;
                if !self.conservative {
                    out[i] += on_i.reaction;
                    out[j] += on_j.reaction;
                }
            }
        }
    }
}

/// The relative acceleration of two bodies, a = a₁ − a₂, split by
/// post-Newtonian order, in m/s².
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeAcceleration {
    /// Newton's: −(Gm/r²) n.
    pub newtonian: DVec3,
    /// First post-Newtonian order (1PN).
    pub first: DVec3,
    /// Second order (2PN).
    pub second: DVec3,
    /// Radiation reaction (2.5PN).
    pub reaction: DVec3,
}

impl RelativeAcceleration {
    /// All of them together.
    pub fn total(&self) -> DVec3 {
        self.newtonian + self.first + self.second + self.reaction
    }
}

/// The relative acceleration of two bodies in their center-of-mass frame,
/// harmonic coordinates, through 2.5PN, from their total gravitational
/// parameter `gm` (m³/s²), symmetric mass ratio `nu` = m₁m₂/(m₁+m₂)², and
/// separation `x` = y₁ − y₂ and relative velocity `v`:
///
/// a = −(Gm/r²) [(1 + 𝒜) n + ℬ v]
///
/// with the coefficients 𝒜 and ℬ of Blanchet (2024), section "Equations of
/// motion in the frame of the center of mass" (its 1PN, 2PN and 2.5PN parts; the
/// 2PN ones also in Kidder 1995, *Phys. Rev. D* 52, 821). An independent
/// form of the same physics as [`pair_terms`], for checks.
pub fn relative_acceleration(gm: f64, nu: f64, x: DVec3, v: DVec3) -> RelativeAcceleration {
    let r = x.length();
    let n = x / r;
    let rdot = n.dot(v);
    let v2 = v.length_squared();
    let u = gm / r; // Gm/r
    let scale = -gm / (r * r);
    let (c2, c4, c5) = (C * C, C.powi(4), C.powi(5));

    let a1 = -1.5 * rdot * rdot * nu + v2 + 3.0 * nu * v2 - u * (4.0 + 2.0 * nu);
    let b1 = -4.0 * rdot + 2.0 * rdot * nu;
    let (rd2, rd3, rd4) = (rdot * rdot, rdot.powi(3), rdot.powi(4));
    let a2 = 15.0 / 8.0 * rd4 * nu - 45.0 / 8.0 * rd4 * nu * nu - 4.5 * rd2 * nu * v2
        + 6.0 * rd2 * nu * nu * v2
        + 3.0 * nu * v2 * v2
        - 4.0 * nu * nu * v2 * v2
        + u * (-2.0 * rd2 - 25.0 * rd2 * nu - 2.0 * rd2 * nu * nu - 6.5 * nu * v2
            + 2.0 * nu * nu * v2)
        + u * u * (9.0 + 87.0 / 4.0 * nu);
    let b2 =
        4.5 * rd3 * nu + 3.0 * rd3 * nu * nu - 7.5 * rdot * nu * v2 - 2.0 * rdot * nu * nu * v2
            + u * (2.0 * rdot + 20.5 * rdot * nu + 4.0 * rdot * nu * nu);
    let a25 = 8.0 * u * nu / 5.0 * rdot * (-17.0 / 3.0 * u - 3.0 * v2);
    let b25 = 8.0 * u * nu / 5.0 * (3.0 * u + v2);

    RelativeAcceleration {
        newtonian: scale * n,
        first: scale * (a1 * n + b1 * v) / c2,
        second: scale * (a2 * n + b2 * v) / c4,
        reaction: scale * (a25 * n + b25 * v) / c5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::System;
    use crate::constants::GM_SUN;
    use crate::gravitational_waves::quadrupole_power;
    use crate::integrator::{Ias15, advance};
    use crate::orbit::{kepler_period, periapsis_state};

    /// Two bodies in their center-of-mass frame, to first post-Newtonian
    /// order, with separation `x` and relative velocity `v`: the positions
    /// y₁ = X₂ x, y₂ = −X₁ x (where they sit doesn't change any force), and
    /// velocities from differentiating Blanchet (2024)'s CM relation,
    /// y₁ = [X₂ + ν Δ (v²/2 − Gm/2r)/c²] x and likewise y₂.
    fn center_of_mass_pair(gm1: f64, gm2: f64, x: DVec3, v: DVec3) -> [Body; 2] {
        let gm = gm1 + gm2;
        let (x1, x2) = (gm1 / gm, gm2 / gm);
        let (nu, delta) = (x1 * x2, x1 - x2);
        let r = x.length();
        let rdot = x.dot(v) / r;
        let shift = nu * delta / (C * C)
            * ((0.5 * v.length_squared() - 0.5 * gm / r) * v - 0.5 * gm * rdot / r * (x / r));
        [
            Body::new("one", gm1, 1.0).at(x * x2).moving(v * x2 + shift),
            Body::new("two", gm2, 1.0)
                .at(-x * x1)
                .moving(-v * x1 + shift),
        ]
    }

    /// A pair whose field strength GM/(r c²) is `strength` at a separation
    /// with radial motion, so that every term counts.
    fn strong_pair(gm1: f64, gm2: f64, strength: f64) -> (DVec3, DVec3) {
        let gm = gm1 + gm2;
        let r = gm / (strength * C * C);
        let x = DVec3::new(0.8, 0.5, 0.2).normalize() * r;
        let speed = (gm / r).sqrt();
        let v = DVec3::new(-0.3, 0.9, 0.1).normalize() * (1.1 * speed);
        (x, v)
    }

    #[test]
    fn two_bodies_follow_the_center_of_mass_equations() {
        // The general-frame equations (EIH plus the pair terms), applied to
        // two bodies in their center-of-mass frame, must give the
        // center-of-mass equations' relative acceleration. What differs is
        // of third order (GM/rc²)³: from the center of mass's own 2PN
        // shift, which neither side keeps. A mistake in any 2PN term would
        // show at second order instead, (GM/rc²)². The test passes below the
        // geometric midpoint, (GM/rc²)^(5/2). For the radiation reaction a
        // mistake shows at order 5/2 and what's left is of order 7/2; the
        // midpoint is 3.
        let strength: f64 = 1e-3;
        for (gm1, gm2) in [
            (3.0 * GM_SUN, GM_SUN),
            (1.4 * GM_SUN, 1.4 * GM_SUN),
            (10.0 * GM_SUN, 1e-6 * GM_SUN),
        ] {
            let gm = gm1 + gm2;
            let nu = gm1 * gm2 / (gm * gm);
            let (x, v) = strong_pair(gm1, gm2, strength);
            let bodies = center_of_mass_pair(gm1, gm2, x, v);
            let mut out = [DVec3::ZERO; 2];
            PostNewtonian::default().accelerations(0.0, &bodies, &mut out);
            let expected = relative_acceleration(gm, nu, x, v);
            let newton = expected.newtonian.length();
            let mismatch = (out[0] - out[1] - expected.total()).length() / newton;

            let general = pair_terms(
                gm1,
                gm2,
                bodies[0].position,
                bodies[1].position,
                bodies[0].velocity,
                bodies[1].velocity,
            )
            .reaction
                - pair_terms(
                    gm2,
                    gm1,
                    bodies[1].position,
                    bodies[0].position,
                    bodies[1].velocity,
                    bodies[0].velocity,
                )
                .reaction;
            let reaction_mismatch = (general - expected.reaction).length() / newton;
            println!(
                "m₂/m₁ = {:.0e}: mismatch {mismatch:.1e} (2PN part {:.1e}, bound {:.1e}); reaction mismatch {reaction_mismatch:.1e} (reaction {:.1e}, bound {:.1e})",
                gm2 / gm1,
                expected.second.length() / newton,
                strength.powf(2.5),
                expected.reaction.length() / newton,
                strength.powi(3)
            );
            assert!(mismatch < strength.powf(2.5));
            assert!(reaction_mismatch < strength.powi(3));
        }
    }

    /// The pair's energy times G, in harmonic coordinates, to 2PN order,
    /// and with `schott`, the 2.5PN (Schott) term that makes the energy
    /// balance hold at every instant: Blanchet (2024), section "The 3.5PN
    /// acceleration and 3PN energy", body 1's terms plus the same with the
    /// labels swapped.
    fn energy(bodies: &[Body], schott: bool) -> f64 {
        let half = |b1: &Body, b2: &Body| {
            let (gm1, gm2) = (b1.gm, b2.gm);
            let d = b1.position - b2.position;
            let r = d.length();
            let n = d / r;
            let (v1, v2) = (b1.velocity, b2.velocity);
            let v12 = v1 - v2;
            let (nv1, nv2) = (n.dot(v1), n.dot(v2));
            let (v1s, v2s, v1v2) = (v1.length_squared(), v2.length_squared(), v1.dot(v2));
            let c2 = C * C;
            let newton = 0.5 * gm1 * v1s - 0.5 * gm1 * gm2 / r;
            let first = 0.5 * gm1 * gm1 * gm2 / (r * r)
                + 3.0 / 8.0 * gm1 * v1s * v1s
                + gm1 * gm2 / r * (-0.25 * nv1 * nv2 + 1.5 * v1s - 1.75 * v1v2);
            let second = -0.5 * gm1.powi(3) * gm2 / r.powi(3)
                - 19.0 / 8.0 * gm1 * gm1 * gm2 * gm2 / r.powi(3)
                + 5.0 / 16.0 * gm1 * v1s.powi(3)
                + gm1 * gm2 / r
                    * (3.0 / 8.0 * nv1.powi(3) * nv2 + 3.0 / 16.0 * nv1 * nv1 * nv2 * nv2
                        - 9.0 / 8.0 * nv1 * nv2 * v1s
                        - 13.0 / 8.0 * nv2 * nv2 * v1s
                        + 21.0 / 8.0 * v1s * v1s
                        + 13.0 / 8.0 * nv1 * nv1 * v1v2
                        + 0.75 * nv1 * nv2 * v1v2
                        - 55.0 / 8.0 * v1s * v1v2
                        + 17.0 / 8.0 * v1v2 * v1v2
                        + 31.0 / 16.0 * v1s * v2s)
                + gm1 * gm1 * gm2 / (r * r)
                    * (29.0 / 4.0 * nv1 * nv1 - 13.0 / 4.0 * nv1 * nv2 + 0.5 * nv2 * nv2
                        - 1.5 * v1s
                        + 1.75 * v2s);
            let schott_term = if schott {
                4.0 * gm1 * gm1 * gm2 / (5.0 * r * r)
                    * nv1
                    * (v12.length_squared() - 2.0 * (gm1 - gm2) / r)
            } else {
                0.0
            };
            newton + first / c2 + second / (c2 * c2) + schott_term / C.powi(5)
        };
        half(&bodies[0], &bodies[1]) + half(&bodies[1], &bodies[0])
    }

    /// A binary at periastron with eccentricity 0.5, whose field strength
    /// GM/(rc²) at periastron is `strength`, in its center-of-mass frame.
    fn eccentric_binary(strength: f64) -> (System, f64) {
        let (gm1, gm2) = (1.4 * GM_SUN, 1.0 * GM_SUN);
        let gm = gm1 + gm2;
        let e = 0.5;
        let a = gm / (strength * C * C) / (1.0 - e);
        let (x, v) = periapsis_state(a, e, gm);
        let bodies = center_of_mass_pair(gm1, gm2, x, v);
        (System::new(bodies.to_vec()), kepler_period(a, gm))
    }

    #[test]
    fn without_radiation_the_pair_keeps_its_energy() {
        // The 2PN energy wanders under the first-order equations alone:
        // they leave out a second-order part, (GM/rc²)² of the force. With
        // the 2PN terms, only third-order parts are left out, so it wanders
        // GM/rc² times less; a mistake in a 2PN term would leave it
        // wandering about as much. The test passes below the midpoint,
        // √(GM/rc²) times the first-order wander (the orbit's own
        // coefficient cancels in the comparison).
        let strength: f64 = 1e-3;
        let wander = |gravity: &dyn Gravity| {
            let (mut system, period) = eccentric_binary(strength);
            let start = energy(&system.bodies, false);
            let mut ias = Ias15::new();
            let mut worst: f64 = 0.0;
            for _ in 0..(10 * 64) {
                advance(&mut system, gravity, &mut ias, period / 64.0);
                worst = worst.max(((energy(&system.bodies, false) - start) / start).abs());
            }
            worst
        };
        let ours = wander(&PostNewtonian::conservative());
        let first_order = wander(&EinsteinInfeldHoffmann);
        let bound = strength.sqrt() * first_order;
        println!(
            "over 10 orbits the energy wanders by {first_order:.1e} of itself with first order alone, {ours:.1e} with 2PN (bound {bound:.1e})"
        );
        assert!(ours < bound);
    }

    #[test]
    fn radiation_carries_off_the_quadrupole_power() {
        // With the radiation reaction, the energy (with its Schott term)
        // falls exactly as fast as Einstein's quadrupole formula says the
        // waves carry it off, up to the next order: GM/rc² of it (measured
        // 2.5, 2.2 and 1.9 times GM/rc² at 10⁻², 3 × 10⁻³ and 10⁻³). A
        // wrong reaction term would miss by a sizable fraction. The test
        // allows √(GM/rc²) of the radiated energy, between the two.
        let strength: f64 = 3e-3;
        let (mut system, period) = eccentric_binary(strength);
        let gravity = PostNewtonian::default();
        let power = |s: &System| {
            let (b1, b2) = (&s.bodies[0], &s.bodies[1]);
            let x = b1.position - b2.position;
            let v = b1.velocity - b2.velocity;
            quadrupole_power(b1.gm, b2.gm, x.length(), v.length(), x.dot(v) / x.length())
        };
        let start = energy(&system.bodies, true);
        let mut ias = Ias15::new();
        let steps = 20 * 256;
        let dt = 20.0 * period / steps as f64;
        // Simpson's rule over the samples; the energy is E·G and the power
        // is in W, so radiated·G is compared.
        let mut radiated = 0.0;
        let mut previous = power(&system);
        for _ in 0..steps {
            advance(&mut system, &gravity, &mut ias, dt / 2.0);
            let middle = power(&system);
            advance(&mut system, &gravity, &mut ias, dt / 2.0);
            let after = power(&system);
            radiated += dt / 6.0 * (previous + 4.0 * middle + after);
            previous = after;
        }
        let radiated = radiated * crate::constants::G;
        let lost = start - energy(&system.bodies, true);
        let miss = (lost / radiated - 1.0).abs();
        println!(
            "over 20 orbits the pair lost {:.4e} of its energy; the quadrupole formula says {:.4e}: off by {miss:.1e} of it (bound {:.1e})",
            lost / start.abs(),
            radiated / start.abs(),
            strength.sqrt()
        );
        assert!(miss < strength.sqrt());
    }
}
