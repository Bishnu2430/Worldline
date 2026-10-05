use glam::DVec3;

use super::Gravity;
use crate::Body;
use crate::constants::C;

/// The Einstein–Infeld–Hoffmann equations: N-body gravity with general
/// relativity's first post-Newtonian (1PN) corrections.
///
/// On top of Newton's law, the corrections account for how gravity is
/// changed by the bodies' own speeds, by the gravitational potential they
/// sit in, and by the motion of their neighbours. They are why Mercury's
/// orbit precesses 43″ per century more than Newton predicts.
///
/// Valid while gravity is weak and motion slow (GM / rc² ≪ 1, v ≪ c).
/// Terms of order (GM / rc²)² (2PN) and gravitational-wave losses (2.5PN)
/// are left out. Written in harmonic coordinates with the PPN parameters
/// β = γ = 1 of general relativity, in the form JPL uses for its planetary
/// ephemerides: Folkner et al. (2014), IPN Progress Report 42-196, eq. 27.
/// Original: Einstein, Infeld & Hoffmann, Ann. Math. 39, 65 (1938).
/// See `docs/physics/einstein-infeld-hoffmann.md`.
#[derive(Debug, Clone, Copy, Default)]
pub struct EinsteinInfeldHoffmann;

impl Gravity for EinsteinInfeldHoffmann {
    fn name(&self) -> &'static str {
        "Einstein–Infeld–Hoffmann (1PN)"
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
        let n = bodies.len();
        assert_eq!(n, out.len(), "need one acceleration slot per body");

        // First pass: each body's Newtonian acceleration, and the Newtonian
        // potential it sits in, Σ_k μ_k / r_jk.
        let mut newtonian = vec![DVec3::ZERO; n];
        let mut potential = vec![0.0; n];
        for (i, bi) in bodies.iter().enumerate() {
            for (j, bj) in bodies.iter().enumerate().skip(i + 1) {
                let d = bj.position - bi.position;
                let r = d.length();
                debug_assert!(r > 0.0, "{} and {} share a position", bi.name, bj.name);
                let inv_r3 = 1.0 / (r * r * r);
                newtonian[i] += d * (bj.gm * inv_r3);
                newtonian[j] -= d * (bi.gm * inv_r3);
                potential[i] += bj.gm / r;
                potential[j] += bi.gm / r;
            }
        }

        // Second pass: the 1PN corrections, added to the Newtonian part
        // separately so the tiny corrections keep their full precision.
        let inv_c2 = 1.0 / (C * C);
        for (i, bi) in bodies.iter().enumerate() {
            let vi = bi.velocity;
            let mut correction = DVec3::ZERO;
            for (j, bj) in bodies.iter().enumerate() {
                if i == j {
                    continue;
                }
                let vj = bj.velocity;
                let aj = newtonian[j];
                let d = bj.position - bi.position; // r_j − r_i
                let r = d.length();
                let inv_r3 = 1.0 / (r * r * r);
                let radial_vj = -d.dot(vj) / r; // (r_i − r_j) · v_j / r_ij

                // Corrections to the strength of the Newtonian pull.
                let strength = -4.0 * potential[i] - potential[j]
                    + vi.length_squared()
                    + 2.0 * vj.length_squared()
                    - 4.0 * vi.dot(vj)
                    - 1.5 * radial_vj * radial_vj
                    + 0.5 * d.dot(aj);
                correction += d * (bj.gm * inv_r3 * strength);

                // A pull along the relative velocity.
                let along_velocity = (-d).dot(4.0 * vi - 3.0 * vj);
                correction += (vi - vj) * (bj.gm * inv_r3 * along_velocity);

                // Body j's own acceleration feeds back on body i.
                correction += aj * (3.5 * bj.gm / r);
            }
            out[i] = newtonian[i] + correction * inv_c2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GM_SUN;

    /// The 1PN relative acceleration of two bodies in their center-of-mass
    /// frame, harmonic coordinates: Blanchet, Living Rev. Relativ. 17, 2
    /// (2014), eqs. 219–222 truncated at 1PN. An independent check on the
    /// N-body equations above.
    fn two_body_1pn(mu: f64, nu: f64, r: DVec3, v: DVec3) -> DVec3 {
        let dist = r.length();
        let n = r / dist;
        let rdot = n.dot(v);
        let inv_c2 = 1.0 / (C * C);
        let a_coeff = (1.0 + 3.0 * nu) * v.length_squared()
            - 1.5 * nu * rdot * rdot
            - 2.0 * (2.0 + nu) * mu / dist;
        let b_coeff = -2.0 * (2.0 - nu) * rdot;
        -(mu / (dist * dist)) * ((1.0 + a_coeff * inv_c2) * n + b_coeff * inv_c2 * v)
    }

    #[test]
    fn two_bodies_reduce_to_the_textbook_formula() {
        // Strong enough fields that the 1PN part is large (GM / rc² ~ 10⁻⁵),
        // with unequal masses and a radial velocity so every term matters.
        // The second case is a near test particle orbiting a star.
        for (mu1, mu2) in [(3.0 * GM_SUN, GM_SUN), (GM_SUN, 1e-10 * GM_SUN)] {
            let mu = mu1 + mu2;
            let nu = mu1 * mu2 / (mu * mu);
            let r = DVec3::new(6.0e8, 2.0e8, 1.0e7);
            let v = DVec3::new(-3.0e5, 4.0e5, 2.0e4);
            // Center-of-mass frame: body 1 at +(μ₂/μ) r, body 2 at −(μ₁/μ) r.
            let bodies = [
                Body::new("one", mu1, 1.0)
                    .at(r * (mu2 / mu))
                    .moving(v * (mu2 / mu)),
                Body::new("two", mu2, 1.0)
                    .at(-r * (mu1 / mu))
                    .moving(-v * (mu1 / mu)),
            ];
            let mut acc = [DVec3::ZERO; 2];
            EinsteinInfeldHoffmann.accelerations(0.0, &bodies, &mut acc);

            let expected = two_body_1pn(mu, nu, r, v);
            let newtonian = -mu * r / r.length().powi(3);
            let pn_part = (expected - newtonian).length();
            let mismatch = ((acc[0] - acc[1]) - expected).length();
            // What's left should be 2PN-sized: (GM / rc²)² relative to Newton,
            // i.e. far smaller than the 1PN part itself.
            assert!(
                mismatch < 1e-3 * pn_part,
                "μ₂/μ₁ = {}: mismatch {mismatch:e} vs 1PN part {pn_part:e}",
                mu2 / mu1
            );
        }
    }

    #[test]
    fn corrections_scale_with_how_relativistic_the_system_is() {
        // The 1PN corrections are of relative size ε = GM / rc² + v² / c².
        // For a weak, slow system they must be about that small, no bigger.
        let (mu_a, mu_b, r, v) = (1.0e10, 2.0e10, 1.0e7, 10.0);
        let bodies = [
            Body::new("a", mu_a, 1.0),
            Body::new("b", mu_b, 1.0)
                .at(DVec3::new(r, 0.0, 0.0))
                .moving(DVec3::new(0.0, v, 0.0)),
        ];
        let epsilon = (mu_a + mu_b) / (r * C * C) + v * v / (C * C);
        let (mut relativistic, mut newtonian) = ([DVec3::ZERO; 2], [DVec3::ZERO; 2]);
        EinsteinInfeldHoffmann.accelerations(0.0, &bodies, &mut relativistic);
        super::super::Newtonian.accelerations(0.0, &bodies, &mut newtonian);
        for (a, b) in relativistic.iter().zip(&newtonian) {
            let relative = (*a - *b).length() / b.length();
            assert!(relative < 10.0 * epsilon, "{relative:e} vs ε = {epsilon:e}");
        }
    }
}
