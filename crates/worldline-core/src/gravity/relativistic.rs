//! The top level's gravity: post-Newtonian N-body gravity everywhere, and
//! a black hole's exact spacetime for the light bodies it holds. See
//! `docs/physics/kerr.md`.

use glam::DVec3;

use super::kerr::geodesic_acceleration;
use super::{Gravity, PostNewtonian, relative_acceleration};
use crate::Body;
use crate::compact::{horizon_spin, is_black_hole};

/// How light a body must be, as ν = m M/(m + M)², to move in a black
/// hole's exact spacetime rather than by the post-Newtonian equations.
///
/// Each leaves something out. As a test body in the hole's spacetime, a
/// body's own mass is ignored, to relative order ν GM/rc²; the
/// post-Newtonian equations stop at (GM/rc²)³, and their radiation
/// reaction stops converging near GM/rc² ≈ 0.1. At the innermost stable
/// orbit (GM/rc² = 1/6), the deepest a body circles, the test-body error
/// is the smaller when ν < (1/6)² = 1/36: bodies about 35 times lighter
/// than the hole, or lighter still.
pub const TEST_BODY_NU: f64 = 1.0 / 36.0;

/// Whether `body` moves in `hole`'s exact spacetime, if `hole` is what
/// pulls on it hardest: `hole` is a black hole, the heavier, and `body` is
/// light enough (see [`TEST_BODY_NU`]).
pub fn moves_in_spacetime_of(body: &Body, hole: &Body) -> bool {
    let gm = body.gm + hole.gm;
    is_black_hole(hole) && hole.gm > body.gm && body.gm * hole.gm < TEST_BODY_NU * gm * gm
}

/// The black hole whose exact spacetime body `i` moves in, if any: the
/// body that pulls on it hardest, if [`moves_in_spacetime_of`] holds.
pub fn holding_hole(bodies: &[Body], i: usize) -> Option<usize> {
    let body = &bodies[i];
    let strongest = bodies
        .iter()
        .enumerate()
        .filter(|&(j, b)| j != i && b.gm > 0.0)
        .max_by(|(_, a), (_, b)| {
            let pull = |x: &Body| x.gm / (x.position - body.position).length_squared();
            pull(a).total_cmp(&pull(b))
        })?
        .0;
    moves_in_spacetime_of(body, &bodies[strongest]).then_some(strongest)
}

/// Post-Newtonian N-body gravity ([`PostNewtonian`]) for every body,
/// except those a black hole holds (see [`holding_hole`]). Such a body
/// moves in the hole's frame:
/// - **The hole's spacetime:** Kerr's, exactly, for the pair's total
///   mass and the hole's spin (its axis along +z), in Kerr–Schild
///   coordinates centered on the hole (see [`geodesic_acceleration`]).
/// - **Its radiation reaction:** the leading (2.5PN) term, which drains
///   the orbit's energy into gravitational waves at the quadrupole
///   formula's rate. Its post-Newtonian correction stops converging near
///   the innermost stable orbit, so it is left out here.
/// - **Everything else:** Newtonian tides, each body's pull on it minus
///   its pull on the hole.
/// - **The hole's own motion:** whatever moves the hole, as computed for
///   the hole.
#[derive(Debug, Clone, Copy, Default)]
pub struct Relativistic {
    /// The equations for every body the holes don't hold.
    pub post_newtonian: PostNewtonian,
}

impl Relativistic {
    /// The conservative part alone: everything but the radiation reaction.
    pub fn conservative() -> Self {
        Self {
            post_newtonian: PostNewtonian::conservative(),
        }
    }
}

impl Gravity for Relativistic {
    fn name(&self) -> &'static str {
        "Einstein–Infeld–Hoffmann (1PN) with 2PN and gravitational-wave losses between pairs; exact black-hole spacetime (Kerr) for the bodies a black hole holds"
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        self.post_newtonian.accelerations(time, bodies, out);
        if !bodies.iter().any(is_black_hole) {
            return;
        }
        // Heaviest first, so a hole's own acceleration is final before the
        // bodies it holds use it (a hole can itself be held by a bigger
        // one).
        let mut order: Vec<usize> = (0..bodies.len()).collect();
        order.sort_by(|&i, &j| bodies[j].gm.total_cmp(&bodies[i].gm));
        for i in order {
            let Some(h) = holding_hole(bodies, i) else {
                continue;
            };
            let (body, hole) = (&bodies[i], &bodies[h]);
            let gm = hole.gm + body.gm;
            // The hole's spin, as a fraction of the pair's mass.
            let spin = horizon_spin(hole.gm, hole.radius) * hole.gm / gm;
            let x = body.position - hole.position;
            let v = body.velocity - hole.velocity;
            let mut a = out[h] + geodesic_acceleration(gm, spin, x, v);
            if !self.post_newtonian.conservative {
                let nu = body.gm * hole.gm / (gm * gm);
                a += relative_acceleration(gm, nu, x, v).reaction;
            }
            for (j, other) in bodies.iter().enumerate() {
                if j == i || j == h || other.gm == 0.0 {
                    continue;
                }
                let to_body = other.position - body.position;
                let to_hole = other.position - hole.position;
                a += other.gm
                    * (to_body / to_body.length().powi(3) - to_hole / to_hole.length().powi(3));
            }
            out[i] = a;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compact::schwarzschild_radius;
    use crate::constants::{AU, GM_SUN};

    fn hole(gm: f64) -> Body {
        Body::new("hole", gm, schwarzschild_radius(gm))
    }

    #[test]
    fn only_light_bodies_held_by_a_black_hole_move_in_its_spacetime() {
        // A star of one Sun near a hole of 40 Suns (ν = 0.024): yes. Near a
        // hole of 30 Suns (ν = 0.031): no, the post-Newtonian equations
        // with their finite-mass terms do better. Near a star of 40 Suns:
        // no, it isn't a black hole. And the hole itself doesn't move in
        // the star's.
        let star = Body::new("star", GM_SUN, 7e8).at(DVec3::new(AU, 0.0, 0.0));
        let bodies = [hole(40.0 * GM_SUN), star.clone()];
        assert_eq!(holding_hole(&bodies, 1), Some(0));
        assert_eq!(holding_hole(&bodies, 0), None);
        assert_eq!(holding_hole(&[hole(30.0 * GM_SUN), star.clone()], 1), None);
        let big_star = Body::new("big star", 40.0 * GM_SUN, 7e9);
        assert_eq!(holding_hole(&[big_star, star], 1), None);
    }

    #[test]
    fn a_held_body_gets_the_holes_motion_plus_its_orbit_plus_tides() {
        // A hole of 40 Suns with a star 1 AU away, and a third star 10 AU
        // off pulling on both: the held star's acceleration is the hole's,
        // plus its Kerr orbit around the hole (with radiation reaction),
        // plus the third star's tide.
        let bodies = [
            hole(40.0 * GM_SUN),
            Body::new("star", GM_SUN, 7e8)
                .at(DVec3::new(AU, 0.0, 0.0))
                .moving(DVec3::new(0.0, 190e3, 0.0)),
            Body::new("far", 5.0 * GM_SUN, 7e8).at(DVec3::new(0.0, 10.0 * AU, 0.0)),
        ];
        let mut out = [DVec3::ZERO; 3];
        Relativistic::default().accelerations(0.0, &bodies, &mut out);
        let mut plain = [DVec3::ZERO; 3];
        PostNewtonian::default().accelerations(0.0, &bodies, &mut plain);
        // The hole and the far star: as post-Newtonian gravity has them.
        assert_eq!(out[0], plain[0]);
        assert_eq!(out[2], plain[2]);
        let gm = 41.0 * GM_SUN;
        let (x, v) = (bodies[1].position, bodies[1].velocity);
        let nu = 40.0 / (41.0 * 41.0);
        let tide = |p: DVec3| (bodies[2].position - p) / (bodies[2].position - p).length().powi(3);
        let expected = out[0]
            + geodesic_acceleration(gm, 0.0, x, v)
            + relative_acceleration(gm, nu, x, v).reaction
            + 5.0 * GM_SUN * (tide(x) - tide(DVec3::ZERO));
        assert!((out[1] - expected).length() <= 1e-15 * expected.length());
        // At 1 AU from 40 Suns, GM/rc² = 4 × 10⁻⁷. Kerr–Schild and the
        // post-Newtonian (harmonic) coordinates differ at first order in
        // it, so the two accelerations do too; a wrong or missing term
        // would differ at order 1. The test passes below the midpoint,
        // √(GM/rc²).
        let strength = gm / (AU * crate::constants::C * crate::constants::C);
        let miss = (out[1] - plain[1]).length() / plain[1].length();
        println!(
            "held star: Kerr and post-Newtonian accelerations agree to {miss:.1e} (GM/rc² = {strength:.1e})"
        );
        assert!(miss < strength.sqrt());
    }
}
