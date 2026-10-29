use glam::DVec3;

use super::{Gravity, ZonalField};
use crate::Body;

/// A body's equatorial bulge, as zonal harmonics (see [`ZonalField`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Bulge {
    /// The bulging body, by name.
    pub body: String,
    /// Its field beyond a point mass.
    pub field: ZonalField,
}

/// Top-level gravity, plus the pull of the bulges of bodies simulated
/// there: Earth's, on the Moon (the other planets' moons feel theirs in
/// their own moon systems). Each bulge pulls on every other body, and each
/// pulls back on the bulge with the opposite force (Newton's third law).
/// See `docs/physics/moons.md`.
pub struct WithBulges<G> {
    /// Everything else.
    pub gravity: G,
    /// The bulges.
    pub bulges: Vec<Bulge>,
}

impl<G: Gravity> Gravity for WithBulges<G> {
    fn name(&self) -> &'static str {
        self.gravity.name()
    }

    fn velocity_dependent(&self) -> bool {
        self.gravity.velocity_dependent()
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        self.gravity.accelerations(time, bodies, out);
        for bulge in &self.bulges {
            let Some(b) = bodies.iter().position(|body| body.name == bulge.body) else {
                continue;
            };
            let center = &bodies[b];
            for (i, body) in bodies.iter().enumerate() {
                if i == b {
                    continue;
                }
                let a = bulge
                    .field
                    .acceleration(center.gm, body.position - center.position);
                out[i] += a;
                out[b] -= a * (body.gm / center.gm);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gravity::Newtonian;

    #[test]
    fn a_bulge_adds_its_field_and_conserves_momentum() {
        let field = ZonalField {
            radius: 6.4e6,
            coefficients: vec![(2, 1e-3), (3, -2e-6)],
            pole: DVec3::Z,
        };
        let bodies = vec![
            Body::new("Earth", 4e14, 6.4e6),
            Body::new("Moon", 5e12, 1.7e6).at(DVec3::new(3e8, 1e8, 5e7)),
            Body::new("Probe", 0.0, 1.0).at(DVec3::new(0.0, 0.0, 7e6)),
        ];
        let gravity = WithBulges {
            gravity: Newtonian,
            bulges: vec![Bulge {
                body: "Earth".into(),
                field: field.clone(),
            }],
        };
        let (mut with, mut without) = (vec![DVec3::ZERO; 3], vec![DVec3::ZERO; 3]);
        gravity.accelerations(0.0, &bodies, &mut with);
        Newtonian.accelerations(0.0, &bodies, &mut without);
        // The field adds to the point-mass pull, so the sum rounds at the
        // scale of the whole acceleration.
        for i in 1..3 {
            let expected = field.acceleration(4e14, bodies[i].position);
            let rounding = 4.0 * f64::EPSILON * with[i].length();
            assert!((with[i] - without[i] - expected).length() <= rounding);
        }
        // The total force is still zero, to rounding.
        let total: DVec3 = bodies.iter().zip(&with).map(|(b, a)| *a * b.gm).sum();
        let scale = with[1].length() * 5e12;
        assert!(total.length() <= 1e-15 * scale, "{total}");
    }
}
