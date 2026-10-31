//! What a collision does (step 6.2): the outcome of [`crate::impact`] carried
//! out. The largest remnant goes on, the mass it lost flies off as fragments,
//! and the energy that went missing heats what's left. See
//! `docs/physics/debris.md`.
//!
//! - **The remnants' masses** are Leinhardt & Stewart's (2012).
//! - **The largest remnant's velocity** is theirs too: the center of mass's
//!   after a head-on collision, the target's own past b = 0.7, and linear in
//!   b between; and approaching the center of mass's as it takes in more of
//!   the projectile, which they report but don't give a formula for, so that
//!   is linear too.
//! - **A hit-and-run** slows both bodies by the 10% the paper reports for its
//!   targets at b = 0.9, the one figure it gives: that is what pays for what
//!   the projectile throws off, and it makes a later merger likelier, as the
//!   paper says it does.
//! - **The fragments** are as Chambers's (2013) N-body code has them, as
//!   Quintana et al. (2016, §3.1) describe it: equal in size, a simplification
//!   for tractability, and dispersed slightly above the escape speed in
//!   uniform directions within a plane. Here that is the whole circle after
//!   a head-on collision. Otherwise the fragments have momentum to carry, and
//!   they spread over a narrower arc instead, at the same speed: Worldline's
//!   choice, the cheapest way to carry it.
//!
//! Mass, momentum and the center of mass are the same after as before, to
//! rounding. Energy is too, by accounting for all of it: kinetic energy in
//! the center of mass's frame, the bodies' gravity on each other, and each
//! body's binding energy as a uniform sphere, −(3/5) G m²/R. What's left over
//! is the heat.

use glam::DVec3;

use crate::Body;
use crate::collision::merge;
use crate::constants::G;
use crate::impact::{Outcome, Regime};

/// How many fragments the mass thrown off is split into. Worldline's
/// choice: enough to show the spray, and few enough to follow.
pub const FRAGMENTS: usize = 128;

/// How far above the escape speed fragments leave. The source says only
/// "slightly"; 5% is Worldline's choice.
const ESCAPE_MARGIN: f64 = 1.05;

/// Where fragments and a parting projectile start, as a multiple of the
/// surface's distance, so they don't start inside what they left.
const CLEARANCE: f64 = 1.01;

/// The impact parameter past which the largest remnant keeps the target's
/// velocity, in the paper's Appendix.
const KEEPS_ITS_VELOCITY: f64 = 0.7;

/// The most of its velocity, in the center of mass's frame, that a target
/// keeps in a hit-and-run: the paper's targets typically lose about 10% at
/// b = 0.9, and more at b = 0.7, and its data give no law for how that varies.
const KEPT_IN_A_HIT_AND_RUN: f64 = 0.9;

/// The narrowest arc fragments spread over, ± this many radians: 30°, so
/// they spray rather than leaving as one stream. Worldline's choice.
const NARROWEST: f64 = std::f64::consts::FRAC_PI_6;

/// A piece of what a collision threw off.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fragment {
    /// Where it starts, m.
    pub position: DVec3,
    /// Its velocity, m/s.
    pub velocity: DVec3,
    /// Its gravitational parameter, m³/s².
    pub gm: f64,
}

/// A collision's energy, J, in the center of mass's frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Energy {
    /// Everything's motion.
    pub kinetic: f64,
    /// The bodies' gravity on each other, and on the fragments.
    pub potential: f64,
    /// What holds each body together, as a uniform sphere.
    pub binding: f64,
}

impl Energy {
    /// All of it.
    pub fn total(&self) -> f64 {
        self.kinetic + self.potential + self.binding
    }
}

/// What a collision leaves.
#[derive(Debug, Clone, PartialEq)]
pub struct Aftermath {
    /// The largest remnant, keeping the target's name.
    pub largest: Body,
    /// The projectile, if it goes on: in a hit-and-run.
    pub second: Option<Body>,
    /// What flew off.
    pub fragments: Vec<Fragment>,
    /// The energy before.
    pub before: Energy,
    /// The energy after, heat aside.
    pub after: Energy,
    /// The energy that became heat, J: never negative.
    pub heat: f64,
}

/// A uniform sphere's binding energy, J.
fn binding(gm: f64, radius: f64) -> f64 {
    -0.6 * gm * gm / (G * radius)
}

/// The mean of the cosines of `FRAGMENTS` directions evenly spread over
/// ±`half` of a line: how much of their speed goes along it.
fn along(half: f64) -> f64 {
    let n = FRAGMENTS as f64;
    let sum: f64 = (0..FRAGMENTS)
        .map(|k| (half * (2.0 * (k as f64 + 0.5) / n - 1.0)).cos())
        .sum();
    sum / n
}

/// The half-width of the arc whose fragments, all at `speed`, carry `carry`
/// on average along its middle: from a full circle for nothing to carry, down
/// to [`NARROWEST`]. Bisection, as it can't be solved outright.
fn arc_for(carry: f64, speed: f64) -> f64 {
    let (mut narrow, mut wide) = (NARROWEST, std::f64::consts::PI);
    for _ in 0..60 {
        let middle = 0.5 * (narrow + wide);
        if along(middle) * speed > carry {
            narrow = middle;
        } else {
            wide = middle;
        }
    }
    0.5 * (narrow + wide)
}

/// The pieces of a collision, in its center of mass's frame, before any
/// allowance for the energy available.
struct Pieces {
    /// Each body's gravitational parameter, radius, position and velocity.
    bodies: Vec<(f64, f64, DVec3, DVec3)>,
    /// Each fragment's position and velocity.
    fragments: Vec<(DVec3, DVec3)>,
    /// A fragment's gravitational parameter.
    fragment_gm: f64,
}

impl Pieces {
    /// Their gravity on each other: the bodies on each other and on the
    /// fragments. Fragments are too light to pull on anything, as in the
    /// swarm that carries them.
    fn potential(&self) -> f64 {
        let mut sum = 0.0;
        for (i, a) in self.bodies.iter().enumerate() {
            for b in &self.bodies[i + 1..] {
                sum -= a.0 * b.0 / (G * (a.2 - b.2).length());
            }
            for fragment in &self.fragments {
                sum -= a.0 * self.fragment_gm / (G * (a.2 - fragment.0).length());
            }
        }
        sum
    }

    fn binding(&self) -> f64 {
        self.bodies.iter().map(|b| binding(b.0, b.1)).sum()
    }

    fn kinetic(&self) -> f64 {
        let of = |gm: f64, v: DVec3| 0.5 * gm * v.length_squared() / G;
        let bodies: f64 = self.bodies.iter().map(|b| of(b.0, b.3)).sum();
        let fragments: f64 = self
            .fragments
            .iter()
            .map(|f| of(self.fragment_gm, f.1))
            .sum();
        bodies + fragments
    }
}

/// Fragments leaving a body of `body_gm` and `radius` at `center`, in the
/// plane of `plane`, with mean velocity `bulk`: from the surface, heading away
/// from it, all at one speed 5% over its escape speed, as the body's mass
/// already counts what stays. With nothing to carry, as `bulk` is the body's
/// own velocity, they spread over the whole circle. The more they carry, the
/// narrower the arc, and past the narrowest they go faster instead. That is
/// the least energy that gets every one away and carries the momentum: any
/// more would come out of the heat.
fn spray(
    center: (DVec3, DVec3),
    body_gm: f64,
    radius: f64,
    bulk: DVec3,
    plane: (DVec3, DVec3),
) -> Vec<(DVec3, DVec3)> {
    let carry = bulk - center.1;
    let floor = ESCAPE_MARGIN * (2.0 * body_gm / radius).sqrt();
    let (half, speed) = if carry.length() >= floor * along(NARROWEST) {
        (NARROWEST, carry.length() / along(NARROWEST))
    } else {
        (arc_for(carry.length(), floor), floor)
    };
    let first = carry
        .try_normalize()
        .or(plane.0.try_normalize())
        .unwrap_or(DVec3::X);
    let normal = first
        .cross(plane.1)
        .try_normalize()
        .or(first.cross(plane.0).try_normalize())
        .unwrap_or_else(|| first.any_orthonormal_vector());
    let second = normal.cross(first);
    (0..FRAGMENTS)
        .map(|k| {
            let angle = half * (2.0 * (k as f64 + 0.5) / FRAGMENTS as f64 - 1.0);
            let direction = first * angle.cos() + second * angle.sin();
            (
                center.0 + direction * (radius * CLEARANCE),
                center.1 + direction * speed,
            )
        })
        .collect()
}

/// What the collision of `target` and `projectile` leaves, as `outcome`
/// says. `separation` and `relative` are the projectile's position and
/// velocity relative to the target at the moment they touched: they set
/// the plane of the spray, and where a hit-and-run's projectile emerges.
pub fn aftermath(
    target: &Body,
    projectile: &Body,
    separation: DVec3,
    relative: DVec3,
    outcome: &Outcome,
    impact_parameter: f64,
) -> Aftermath {
    let total = target.gm + projectile.gm;
    let center = (target.position * target.gm + projectile.position * projectile.gm) / total;
    let drift = (target.velocity * target.gm + projectile.velocity * projectile.gm) / total;
    let (target_velocity, projectile_velocity) =
        (target.velocity - drift, projectile.velocity - drift);
    let before = Energy {
        kinetic: 0.5
            * (target.gm * target_velocity.length_squared()
                + projectile.gm * projectile_velocity.length_squared())
            / G,
        potential: -target.gm * projectile.gm
            / (G * (target.position - projectile.position).length()),
        binding: binding(target.gm, target.radius) + binding(projectile.gm, projectile.radius),
    };
    let largest_gm = (outcome.largest * G).clamp(0.0, total);
    // The more of the projectile the target takes in, the closer it comes
    // to the center of mass: all of it there, as in a merger.
    let lost = total - largest_gm;
    let share = (impact_parameter / KEEPS_ITS_VELOCITY).min(1.0);
    let accreting = (lost / projectile.gm).min(1.0);
    let volume = target.radius.powi(3) + projectile.radius.powi(3);
    let remnant_velocity = target_velocity * (share * accreting);
    let plane = (relative, separation);

    // Nothing lost, nothing to fling: they become one.
    if outcome.regime == Regime::Merging || lost <= 0.0 {
        let largest = merge(target, projectile);
        let after = Energy {
            binding: binding(largest.gm, largest.radius),
            ..Energy::default()
        };
        return Aftermath {
            largest,
            second: None,
            fragments: Vec::new(),
            before,
            after,
            heat: before.total() - after.total(),
        };
    }

    let mut pieces = if outcome.regime == Regime::HitAndRun {
        let remnant_velocity = target_velocity * (share * accreting).min(KEPT_IN_A_HIT_AND_RUN);
        let second_gm = (outcome.second * G).clamp(0.0, projectile.gm);
        let flung = projectile.gm - second_gm;
        let fragment_gm = flung / FRAGMENTS as f64;
        // Past the target along the way it was going: where the straight
        // line it came in on leaves the target's reach.
        let way = relative.try_normalize().unwrap_or(separation.normalize());
        let exit = (separation - way * (2.0 * separation.dot(way))).normalize();
        let bulk = -remnant_velocity * (target.gm / projectile.gm);
        // A projectile that survives as less than a fragment doesn't go on.
        let survives = second_gm > fragment_gm;
        let (body_gm, body_radius) = if survives {
            let scale = (second_gm / projectile.gm).cbrt();
            (second_gm, projectile.radius * scale)
        } else {
            (projectile.gm, projectile.radius)
        };
        let at = exit * ((target.radius + body_radius) * CLEARANCE);
        let fragments = if flung > 0.0 {
            spray((at, bulk), body_gm, body_radius, bulk, plane)
        } else {
            Vec::new()
        };
        let mut bodies = vec![(target.gm, target.radius, DVec3::ZERO, remnant_velocity)];
        if survives {
            bodies.push((second_gm, body_radius, at, bulk));
        }
        Pieces {
            bodies,
            fragments,
            fragment_gm: if survives {
                fragment_gm
            } else {
                projectile.gm / FRAGMENTS as f64
            },
        }
    } else {
        let radius = (volume * largest_gm / total).cbrt();
        let bulk = -remnant_velocity * (largest_gm / lost);
        Pieces {
            bodies: vec![(largest_gm, radius, DVec3::ZERO, remnant_velocity)],
            fragments: spray(
                (DVec3::ZERO, remnant_velocity),
                largest_gm,
                radius,
                bulk,
                plane,
            ),
            fragment_gm: lost / FRAGMENTS as f64,
        }
    };

    // Everything is relative to the largest remnant so far. Bring it to
    // the center of mass's frame, and stay there: out at Earth's distance
    // from the Sun, positions round to tens of micrometers.
    let moment: DVec3 = pieces.bodies.iter().map(|b| b.2 * b.0).sum::<DVec3>()
        + pieces.fragments.iter().map(|f| f.0).sum::<DVec3>() * pieces.fragment_gm;
    let shift = moment / total;
    for body in &mut pieces.bodies {
        body.2 -= shift;
    }
    for fragment in &mut pieces.fragments {
        fragment.0 -= shift;
    }

    // The motion can't have more energy than there is: what the remnants'
    // gravity and binding leave. If it would, everything slows together,
    // which keeps the momentum.
    let (potential, binding_after) = (pieces.potential(), pieces.binding());
    let available = before.total() - potential - binding_after;
    let wanted = pieces.kinetic();
    let spent = wanted > available;
    if spent {
        let scale = (available.max(0.0) / wanted).sqrt();
        for body in &mut pieces.bodies {
            body.3 *= scale;
        }
        for fragment in &mut pieces.fragments {
            fragment.1 *= scale;
        }
    }
    let after = Energy {
        kinetic: pieces.kinetic(),
        potential,
        binding: binding_after,
    };

    // Out of that frame. Rounding's last word on the momentum and center of
    // mass goes to the largest remnant.
    let fragment_gm = pieces.fragment_gm;
    let fragments: Vec<Fragment> = pieces
        .fragments
        .iter()
        .map(|f| Fragment {
            position: f.0 + center,
            velocity: f.1 + drift,
            gm: fragment_gm,
        })
        .collect();
    let mut bodies: Vec<(f64, f64, DVec3, DVec3)> = pieces
        .bodies
        .iter()
        .map(|b| (b.0, b.1, b.2 + center, b.3 + drift))
        .collect();
    let of_fragments =
        |value: fn(&Fragment) -> DVec3| fragments.iter().map(value).sum::<DVec3>() * fragment_gm;
    let momentum = bodies.iter().map(|b| b.3 * b.0).sum::<DVec3>() + of_fragments(|f| f.velocity);
    let moment = bodies.iter().map(|b| b.2 * b.0).sum::<DVec3>() + of_fragments(|f| f.position);
    let first_gm = bodies[0].0;
    bodies[0].3 += (drift * total - momentum) / first_gm;
    bodies[0].2 += (center * total - moment) / first_gm;

    let body = |k: usize, name: &str| {
        let (gm, radius, position, velocity) = bodies[k];
        Body {
            name: name.to_string(),
            gm,
            radius,
            position,
            velocity,
        }
    };
    Aftermath {
        largest: body(0, &target.name),
        second: (bodies.len() > 1).then(|| body(1, &projectile.name)),
        fragments,
        before,
        after,
        // If the motion took it all, what's left is rounding. It is too where
        // nothing was lost, and of either sign.
        heat: if spent {
            0.0
        } else {
            (before.total() - after.total()).max(0.0)
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::impact::{Impact, Material};

    const EARTH: f64 = 3.986e14;
    const MARS: f64 = 4.283e13;

    /// A Mars meeting an Earth at `speed`, at impact parameter `b`, and
    /// what comes of it. Both are moving, to test the frames.
    fn collision(speed: f64, b: f64, gm: f64) -> (Body, Body, Aftermath) {
        let (big, small) = (6.371e6, 3.39e6 * (gm / MARS).cbrt());
        let reach = big + small;
        let (sin, cos) = (b, (1.0 - b * b).sqrt());
        let separation = DVec3::new(-cos, sin, 0.0) * reach;
        let relative = DVec3::X * speed;
        let drift = DVec3::new(3e4, -1e4, 2e3);
        let target = Body::new("Earth", EARTH, big)
            .at(DVec3::new(1.5e11, 2e10, 0.0))
            .moving(drift);
        let projectile = Body::new("Mars", gm, small)
            .at(target.position + separation)
            .moving(drift + relative);
        let impact = Impact {
            target_mass: EARTH / G,
            projectile_mass: gm / G,
            target_radius: big,
            projectile_radius: small,
            speed,
            impact_parameter: b,
        };
        let outcome = impact.outcome(Material::PLANETS);
        let after = aftermath(&target, &projectile, separation, relative, &outcome, b);
        (target, projectile, after)
    }

    /// The sum of `gm`, `gm × position` and `gm × velocity`, and their scale.
    fn totals(bodies: &[&Body], fragments: &[Fragment]) -> (f64, DVec3, DVec3, f64) {
        let (mut gm, mut moment, mut momentum, mut scale) = (0.0, DVec3::ZERO, DVec3::ZERO, 0.0f64);
        for (g, p, v) in bodies
            .iter()
            .map(|b| (b.gm, b.position, b.velocity))
            .chain(fragments.iter().map(|f| (f.gm, f.position, f.velocity)))
        {
            gm += g;
            moment += p * g;
            momentum += v * g;
            scale += g * v.length();
        }
        (gm, moment, momentum, scale)
    }

    #[test]
    fn everything_adds_up_in_every_regime() {
        // Mass, momentum and the center of mass the same, and the energy
        // accounted for, to rounding: 128 fragments' worth of it, the sums'
        // terms each good to one part in 2⁵².
        let (mut checked, mut thrown, mut spent) = (0, 0, Vec::new());
        for gm in [MARS, 1e13, EARTH] {
            for b in [0.0, 0.2, 0.45, 0.6, 0.75, 0.9, 0.98] {
                for k in 0..60 {
                    let speed = 3e3 * 1.08f64.powi(k);
                    let (target, projectile, after) = collision(speed, b, gm);
                    let before = totals(&[&target, &projectile], &[]);
                    let mut bodies = vec![&after.largest];
                    bodies.extend(after.second.as_ref());
                    let then = totals(&bodies, &after.fragments);
                    let terms = 2.0 + after.fragments.len() as f64;
                    let rounding = terms * f64::EPSILON;
                    assert!((then.0 / before.0 - 1.0).abs() <= rounding, "{speed} {b}");
                    assert!(
                        (then.2 - before.2).length() <= rounding * before.3.max(then.3),
                        "{speed} {b}"
                    );
                    assert!(
                        (then.1 - before.1).length() <= rounding * before.1.length(),
                        "{speed} {b}"
                    );
                    let scale = after.before.kinetic.abs()
                        + after.before.potential.abs()
                        + after.before.binding.abs()
                        + after.after.kinetic.abs()
                        + after.after.potential.abs()
                        + after.after.binding.abs();
                    let closure = after.before.total() - after.after.total() - after.heat;
                    assert!(closure.abs() <= rounding * scale, "{speed} {b}");
                    // Energy can't come from nowhere.
                    assert!(after.heat >= -rounding * scale, "{speed} {b}: {after:?}");
                    checked += 1;
                    if !after.fragments.is_empty() {
                        thrown += 1;
                        if after.heat == 0.0 {
                            spent.push(((speed / 1e3).round(), b));
                        }
                    }
                }
            }
        }
        println!(
            "of {checked} collisions, {thrown} threw mass off, and the motion took all the energy in {}: {spent:?}",
            spent.len()
        );
        assert!(0 < thrown && thrown < checked, "some merge, some don't");
    }

    #[test]
    fn a_merger_is_a_merger() {
        let (target, projectile, after) = collision(9e3, 0.2, MARS);
        assert!(after.fragments.is_empty() && after.second.is_none());
        assert_eq!(after.largest, merge(&target, &projectile));
        assert!(after.heat > 0.0);
    }

    #[test]
    fn fragments_are_equal_and_fly_off() {
        // Head on, the remnant keeps the center of mass's velocity, and the
        // fragments leave it evenly, 5% over its escape speed.
        let (target, projectile, after) = collision(30e3, 0.0, MARS);
        assert_eq!(after.fragments.len(), FRAGMENTS);
        let total = target.gm + projectile.gm;
        let drift = (target.velocity * target.gm + projectile.velocity * projectile.gm) / total;
        assert!((after.largest.velocity - drift).length() < 1e-6);
        let escape = (2.0 * after.largest.gm / after.largest.radius).sqrt();
        for fragment in &after.fragments {
            assert_eq!(fragment.gm, after.fragments[0].gm);
            let speed = (fragment.velocity - after.largest.velocity).length();
            assert!(
                (speed / (ESCAPE_MARGIN * escape) - 1.0).abs() < 1e-9,
                "{speed}"
            );
            let out = (fragment.position - after.largest.position).length();
            assert!((out / (after.largest.radius * CLEARANCE) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn no_fragment_leaves_too_slowly_to_get_away() {
        // Off center the fragments carry momentum, and every one must still
        // escape: the remnant's mass already counts what stays.
        for (speed, b) in [(14e3, 0.3), (40e3, 0.3), (60e3, 0.1)] {
            let (_, _, after) = collision(speed, b, MARS);
            assert_eq!(after.fragments.len(), FRAGMENTS, "{speed} {b}");
            assert!(after.heat > 0.0, "{speed} {b}: nothing to spare");
            let largest = &after.largest;
            let escape = (2.0 * largest.gm / largest.radius).sqrt();
            for fragment in &after.fragments {
                let leaving = (fragment.velocity - largest.velocity).length();
                assert!(
                    leaving >= ESCAPE_MARGIN * escape * (1.0 - 1e-9),
                    "{speed} {b}: {leaving} of {escape}, heat {:.3e}, kinetic {:.3e}",
                    after.heat,
                    after.after.kinetic
                );
            }
        }
    }

    #[test]
    fn when_the_energy_runs_short_everything_slows_together() {
        // Rare: a glancing blow that erodes the target, whose few fragments
        // must carry all the projectile's momentum. Then all of it slows
        // alike, so the momentum stays, and nothing is left for heat.
        let (_, _, after) = collision(51.7e3, 0.75, MARS);
        assert_eq!(after.fragments.len(), FRAGMENTS);
        assert_eq!(after.heat, 0.0);
        let scale = after.before.kinetic + after.before.binding.abs();
        let rest = after.before.total() - after.after.total();
        assert!(rest.abs() < 130.0 * f64::EPSILON * scale, "{rest}");
    }

    #[test]
    fn a_hit_and_run_leaves_two_bodies_parting() {
        let (target, projectile, after) = collision(16e3, 0.8, MARS);
        let second = after.second.as_ref().expect("the projectile goes on");
        assert_eq!(second.name, "Mars");
        assert_eq!(after.largest.gm, target.gm);
        assert_eq!(after.largest.radius, target.radius);
        assert!(second.gm <= projectile.gm && second.gm > 0.0);
        // Clear of the target, and moving away from it.
        let apart = second.position - after.largest.position;
        assert!(apart.length() > target.radius + second.radius);
        assert!(apart.dot(second.velocity - after.largest.velocity) > 0.0);
        // It slowed, as the paper's targets do: that paid for what the
        // projectile threw off, and the rest is heat.
        let drift = (target.velocity * target.gm + projectile.velocity * projectile.gm)
            / (target.gm + projectile.gm);
        let kept = (after.largest.velocity - drift).length() / (target.velocity - drift).length();
        assert!((kept / KEPT_IN_A_HIT_AND_RUN - 1.0).abs() < 1e-9, "{kept}");
        assert!(after.heat > 0.0);
    }
}
