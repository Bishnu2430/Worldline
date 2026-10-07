//! The swarm: tens of thousands of massless bodies (the asteroid belt,
//! Jupiter's Trojans, the Kuiper belt, small moons torn from their planets)
//! that feel every massive body, including any added in the sandbox.
//!
//! Too many for an integrator each, they use the scheme planetary
//! scientists use for test particles, Wisdom and Holman's (1991, *AJ* 102,
//! 1528), generalized to whatever body dominates each particle:
//!
//! - **Drift:** carried exactly along its two-body orbit around its
//!   *center* (see [`crate::kepler::drift`]), the body whose sphere of
//!   influence it is in: the smallest Hill sphere containing it, else the
//!   strongest pull. Normally the Sun; inside a planet's Hill sphere, the
//!   planet; near a black hole, the black hole.
//! - **Kicks:** half a step's worth of every other body's pull before and
//!   after, minus the pull on the center itself (the particle is followed
//!   in the center's frame, which those bodies accelerate).
//!
//! The error per step scales as each kick's strength relative to the
//! center's pull, times the square of the step over the time in which that
//! kick changes. Each particle takes steps short enough to keep that below
//! [`TOLERANCE`], so a black hole passing through the belt shortens the
//! steps of the asteroids near it and no others.
//!
//! The massive bodies' paths come from the top level's own steps (see
//! [`Field`]), interpolated as for the followers in `hierarchy.rs`. The
//! swarm runs behind the top level by up to [`CADENCE`], to take steps of
//! its own length whatever the top level's. See `docs/physics/swarm.md`.

use std::collections::VecDeque;

use glam::DVec3;

use crate::kepler::drift;

/// How long the swarm's steps are, at most: 64 days, a fifteenth of the
/// shortest orbit in the asteroid belt (the Hungarias', 2.6 years). The
/// error of Wisdom–Holman schemes grows as the step squared; checked
/// against JPL, belt bodies stray by 2 × 10⁻⁶ of their distance in a year
/// at this step (median), 1 × 10⁻⁷ at 16 days, at a quarter of the cost
/// (see `docs/physics/swarm.md`).
pub const CADENCE: f64 = 64.0 * 86_400.0;

/// The largest error a particle's kicks may make in a step, relative to
/// its center's pull: the kick's strength relative to it, times the square
/// of the step over the time in which the kick changes.
pub const TOLERANCE: f64 = 1e-5;

/// Most steps a particle takes in one of the swarm's steps, however close
/// its encounter: a bound on the work a single particle can cost.
const MAX_SUBSTEPS: usize = 4096;

/// A massless body in the swarm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// Position in the top level's frame, in m.
    pub position: DVec3,
    /// Velocity in the top level's frame, in m/s.
    pub velocity: DVec3,
    /// Its owner's label for it.
    pub id: u32,
}

/// The massive bodies of one stretch of time over which they stay the same
/// (no body added, removed or merged).
#[derive(Debug, Clone, PartialEq)]
pub struct Bodies {
    /// Their names.
    pub names: Vec<String>,
    /// Their gravitational parameters, in m³/s².
    pub gms: Vec<f64>,
    /// Their radii, in m.
    pub radii: Vec<f64>,
}

/// A recorded step of the massive bodies: each one's position, velocity and
/// acceleration at its start and end.
#[derive(Debug, Clone)]
struct Segment {
    t0: f64,
    t1: f64,
    /// Index into [`Field::bodies`].
    bodies: usize,
    start: Vec<[DVec3; 3]>,
    end: Vec<[DVec3; 3]>,
}

/// The massive bodies' recorded paths, for the swarm to follow.
#[derive(Debug, Clone, Default)]
pub struct Field {
    bodies: Vec<(u64, Bodies)>,
    segments: VecDeque<Segment>,
}

impl Field {
    /// Records a step of the massive bodies from `t0` to `t1`: their states
    /// (position, velocity, acceleration) at each end. `generation` names
    /// the set of bodies; it must change whenever the set does.
    pub fn record(
        &mut self,
        generation: u64,
        bodies: impl FnOnce() -> Bodies,
        (t0, start): (f64, Vec<[DVec3; 3]>),
        (t1, end): (f64, Vec<[DVec3; 3]>),
    ) {
        if self.bodies.last().is_none_or(|(g, _)| *g != generation) {
            self.bodies.push((generation, bodies()));
        }
        self.segments.push_back(Segment {
            t0,
            t1,
            bodies: self.bodies.len() - 1,
            start,
            end,
        });
    }

    /// The bodies of generation `generation`, if recorded: each one's
    /// position and velocity at `time`, if their recorded paths cover it.
    pub fn states_of(&self, generation: u64, time: f64) -> Option<Vec<(DVec3, DVec3)>> {
        let k = self
            .segments
            .iter()
            .position(|s| self.bodies[s.bodies].0 == generation && s.t0 <= time && time <= s.t1)?;
        let mut out = Vec::new();
        self.states(k, time, &mut out);
        Some(out)
    }

    /// The names of the bodies of generation `generation`, if recorded.
    pub fn names_of(&self, generation: u64) -> Option<&[String]> {
        self.bodies
            .iter()
            .find(|(g, _)| *g == generation)
            .map(|(_, b)| &b.names[..])
    }

    /// The end of the recorded paths.
    pub fn end(&self) -> Option<f64> {
        self.segments.back().map(|s| s.t1)
    }

    /// Forgets what lies before `time`.
    fn forget_before(&mut self, time: f64) {
        while self.segments.len() > 1 && self.segments[0].t1 <= time {
            self.segments.pop_front();
        }
        if let Some(first) = self.segments.front().map(|s| s.bodies) {
            self.bodies.drain(..first);
            for segment in &mut self.segments {
                segment.bodies -= first;
            }
        }
    }

    /// The segment containing `time` (the first, before it).
    fn locate(&self, time: f64) -> usize {
        self.segments
            .partition_point(|s| s.t1 <= time)
            .min(self.segments.len() - 1)
    }

    /// Every body's position and velocity at `time`, within segment `k`,
    /// by quintic Hermite interpolation and its derivative.
    fn states(&self, k: usize, time: f64, out: &mut Vec<(DVec3, DVec3)>) {
        let segment = &self.segments[k];
        let h = segment.t1 - segment.t0;
        out.clear();
        if h <= 0.0 {
            out.extend(segment.end.iter().map(|[p, v, _]| (*p, *v)));
            return;
        }
        let s = ((time - segment.t0) / h).clamp(0.0, 1.0);
        let (s2, s3) = (s * s, s * s * s);
        let (s4, s5) = (s3 * s, s3 * s2);
        let b = [
            1.0 - 10.0 * s3 + 15.0 * s4 - 6.0 * s5,
            h * (s - 6.0 * s3 + 8.0 * s4 - 3.0 * s5),
            h * h * (0.5 * s2 - 1.5 * s3 + 1.5 * s4 - 0.5 * s5),
            h * h * (0.5 * s3 - s4 + 0.5 * s5),
            h * (-4.0 * s3 + 7.0 * s4 - 3.0 * s5),
            10.0 * s3 - 15.0 * s4 + 6.0 * s5,
        ];
        let d = [
            (-30.0 * s2 + 60.0 * s3 - 30.0 * s4) / h,
            1.0 - 18.0 * s2 + 32.0 * s3 - 15.0 * s4,
            h * (s - 4.5 * s2 + 6.0 * s3 - 2.5 * s4),
            h * (1.5 * s2 - 4.0 * s3 + 2.5 * s4),
            -12.0 * s2 + 28.0 * s3 - 15.0 * s4,
            (30.0 * s2 - 60.0 * s3 + 30.0 * s4) / h,
        ];
        out.extend(segment.start.iter().zip(&segment.end).map(|(a, e)| {
            let terms = [a[0], a[1], a[2], e[2], e[1], e[0]];
            let position = terms.iter().zip(b).map(|(t, w)| *t * w).sum();
            let velocity = terms.iter().zip(d).map(|(t, w)| *t * w).sum();
            (position, velocity)
        }));
    }
}

/// Particles a body swallowed (or a particle reached a body's surface).
#[derive(Debug, Clone, PartialEq)]
pub struct Swallowed {
    /// The body.
    pub by: String,
    /// The particles' labels.
    pub ids: Vec<u32>,
    /// When the swarm's step in which it happened ended.
    pub time: f64,
}

/// The swarm of massless particles.
#[derive(Debug, Clone, Default)]
pub struct Swarm {
    /// The particles, in the order they were added (less any swallowed).
    pub particles: Vec<Particle>,
    /// Each particle's center at the end of its last step, by index among
    /// the bodies then (see [`Self::center_bodies`]).
    centers: Vec<u32>,
    /// The swarm's own clock: where the particles are, in s.
    time: f64,
    /// The bodies the centers are counted among.
    center_bodies: Option<u64>,
}

/// What the bodies look like at the two ends of a swarm step, shared by all
/// particles.
struct Context<'a> {
    field: &'a Field,
    gms: &'a [f64],
    radii: &'a [f64],
    /// Each body's Hill radius, relative to the more massive body that
    /// pulls on it hardest (infinite for the most massive).
    hill: Vec<f64>,
    start: Vec<(DVec3, DVec3)>,
    end: Vec<(DVec3, DVec3)>,
    t0: f64,
    t1: f64,
}

/// One particle's step: where it ended, and its center, or what swallowed
/// it.
enum Outcome {
    Moved(Particle, u32),
    Swallowed(usize),
}

impl Swarm {
    /// An empty swarm whose clock reads `time`.
    pub fn new(time: f64) -> Self {
        Self {
            time,
            ..Self::default()
        }
    }

    /// The swarm's clock, in s: where its particles are.
    pub fn time(&self) -> f64 {
        self.time
    }

    /// Sets the clock, as when the swarm is restored from a save.
    pub fn set_time(&mut self, time: f64) {
        self.time = time;
    }

    /// How many particles there are.
    pub fn len(&self) -> usize {
        self.particles.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.particles.is_empty()
    }

    /// Adds particles, given at the swarm's time.
    pub fn add(&mut self, particles: impl IntoIterator<Item = Particle>) {
        for particle in particles {
            self.particles.push(particle);
            self.centers.push(u32::MAX);
        }
    }

    /// Removes the particles with these labels.
    pub fn remove(&mut self, ids: &[u32]) {
        let mut k = 0;
        self.particles.retain(|p| {
            let keep = !ids.contains(&p.id);
            if !keep {
                self.centers.remove(k);
            } else {
                k += 1;
            }
            keep
        });
    }

    /// Each particle's center at the swarm's time, by index among the
    /// bodies of the generation given, if known.
    pub fn centers(&self) -> Option<(u64, &[u32])> {
        self.center_bodies.map(|g| (g, &self.centers[..]))
    }

    /// Works out the centers of particles that have none yet (all of them,
    /// if the centers known are among other bodies), from the bodies of
    /// generation `generation`: their GMs and states at the swarm's time.
    pub fn assign_centers(&mut self, generation: u64, gms: &[f64], states: &[(DVec3, DVec3)]) {
        let all = self.center_bodies != Some(generation);
        let hill = hill_radii(gms, states);
        for (p, c) in self.particles.iter().zip(&mut self.centers) {
            if all || *c == u32::MAX {
                *c = center_of(p.position, p.velocity, gms, &hill, states) as u32;
            }
        }
        self.center_bodies = Some(generation);
    }

    /// Carries every particle to time `to` along the recorded field, in
    /// steps of at most [`CADENCE`], each within one set of bodies; back in
    /// time if `to` is earlier (the scheme runs the same either way).
    /// Returns what was swallowed.
    pub fn advance(&mut self, field: &mut Field, to: f64) -> Vec<Swallowed> {
        let mut swallowed = Vec::new();
        let (Some(first), Some(end)) = (field.segments.front().map(|s| s.t0), field.end()) else {
            return swallowed;
        };
        let to = to.clamp(first, end);
        let forward = to >= self.time;
        let mut scratch = Vec::new();
        while self.time != to {
            let k = field.locate(self.time);
            let bodies = field.segments[k].bodies;
            // Up to the cadence, and no further than this set of bodies lasts.
            let same = |s: &&Segment| s.bodies == bodies;
            let t1 = if forward {
                let last = field.segments.iter().skip(k).take_while(same).last();
                (self.time + CADENCE).min(to).min(last.map_or(to, |s| s.t1))
            } else {
                let k = field.locate(self.time.next_down());
                let earliest = field
                    .segments
                    .iter()
                    .take(k + 1)
                    .rev()
                    .take_while(same)
                    .last();
                (self.time - CADENCE)
                    .max(to)
                    .max(earliest.map_or(to, |s| s.t0))
            };
            if t1 == self.time {
                break;
            }
            let (generation, set) = &field.bodies[bodies];
            let mut start = Vec::new();
            let k = if forward {
                k
            } else {
                field.locate(self.time.next_down())
            };
            field.states(k, self.time, &mut start);
            let k1 = if forward {
                field.locate(t1.next_down())
            } else {
                field.locate(t1)
            };
            field.states(k1, t1, &mut scratch);
            let end = scratch.clone();
            let context = Context {
                field,
                gms: &set.gms,
                radii: &set.radii,
                hill: hill_radii(&set.gms, &start),
                start,
                end,
                t0: self.time,
                t1,
            };
            let outcomes = step_all(&self.particles, &context);
            let mut kept = Vec::with_capacity(self.particles.len());
            let mut centers = Vec::with_capacity(self.particles.len());
            let mut eaten: Vec<(usize, u32)> = Vec::new();
            for (particle, outcome) in self.particles.iter().zip(outcomes) {
                match outcome {
                    Outcome::Moved(moved, center) => {
                        kept.push(moved);
                        centers.push(center);
                    }
                    Outcome::Swallowed(by) => eaten.push((by, particle.id)),
                }
            }
            for by in 0..set.names.len() {
                let ids: Vec<u32> = eaten.iter().filter(|e| e.0 == by).map(|e| e.1).collect();
                if !ids.is_empty() {
                    swallowed.push(Swallowed {
                        by: set.names[by].clone(),
                        ids,
                        time: t1,
                    });
                }
            }
            self.particles = kept;
            self.centers = centers;
            self.center_bodies = Some(*generation);
            self.time = t1;
        }
        if forward {
            field.forget_before(self.time);
        }
        swallowed
    }
}

/// Each body's Hill radius d (m / 3M)^(1/3), relative to the more massive
/// body that pulls on it hardest; infinite for the most massive.
fn hill_radii(gms: &[f64], states: &[(DVec3, DVec3)]) -> Vec<f64> {
    (0..gms.len())
        .map(|k| {
            let attractor = (0..gms.len())
                .filter(|&j| gms[j] > gms[k])
                .map(|j| (j, gms[j] / (states[j].0 - states[k].0).length_squared()))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            match attractor {
                Some((j, _)) if gms[k] > 0.0 => {
                    (states[j].0 - states[k].0).length() * (gms[k] / (3.0 * gms[j])).cbrt()
                }
                Some(_) => 0.0,
                None => f64::INFINITY,
            }
        })
        .collect()
}

/// Steps every particle, sharing them among the processor's cores.
fn step_all(particles: &[Particle], context: &Context) -> Vec<Outcome> {
    let run = |chunk: &[Particle]| {
        let mut scratch = Vec::with_capacity(context.gms.len());
        chunk
            .iter()
            .map(|p| step(*p, context, &mut scratch))
            .collect::<Vec<_>>()
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if particles.len() < 512 || threads == 1 {
        return run(particles);
    }
    let chunk = particles.len().div_ceil(threads);
    std::thread::scope(|scope| {
        let handles: Vec<_> = particles
            .chunks(chunk)
            .map(|chunk| scope.spawn(move || run(chunk)))
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("a swarm thread finished"))
            .collect()
    })
}

/// The body a particle at `x` moving at `v` is followed around. Among the
/// bodies it is bound to (its two-body energy relative to them negative),
/// the one with the smallest Hill sphere containing it, else the one
/// pulling hardest; if it is bound to none, the one pulling hardest. A
/// body racing past never becomes the center of what it doesn't capture.
fn center_of(x: DVec3, v: DVec3, gms: &[f64], hill: &[f64], bodies: &[(DVec3, DVec3)]) -> usize {
    let mut nested: Option<(usize, f64)> = None;
    let mut strongest_bound: Option<(usize, f64)> = None;
    let mut strongest = (0, 0.0);
    for (k, (position, velocity)) in bodies.iter().enumerate() {
        let gm = gms[k];
        if gm <= 0.0 {
            continue;
        }
        let r2 = (*position - x).length_squared();
        let pull = gm / r2;
        if pull > strongest.1 {
            strongest = (k, pull);
        }
        if 0.5 * (*velocity - v).length_squared() * r2.sqrt() >= gm {
            continue;
        }
        if strongest_bound.is_none_or(|b| pull > b.1) {
            strongest_bound = Some((k, pull));
        }
        let hill = hill[k];
        if r2 < hill * hill && nested.is_none_or(|b| hill < b.1) {
            nested = Some((k, hill));
        }
    }
    nested.or(strongest_bound).map_or(strongest.0, |b| b.0)
}

/// The pull of every body but the center on a particle at `x` moving at
/// `v`, in the center's frame (each body's pull on the particle minus its
/// pull on the center), and the longest step that keeps the scheme's
/// errors within [`TOLERANCE`]:
///
/// - **Splitting** (Wisdom & Holman 1991): the error per step is the
///   kicks' strength relative to the center's pull, ε, times the square of
///   the step over the particle's orbital period P around the center, so
///   dt ≤ P √(TOLERANCE/ε).
/// - **Sampling:** each kick, directly on the particle and through the
///   center, changes in a time τ (the shorter of the time to cross the
///   distance d at the relative speed and the dynamical time √(d³/GM)).
///   Sampled every dt, a kick of strength a is missed by about
///   a τ (dt/τ)² of velocity; that must stay within TOLERANCE of the
///   particle's orbital speed v_c, so dt ≤ τ √(TOLERANCE v_c/(a τ)). A
///   kick that gives less than that in all its time needs no limit: the
///   Sun's wobble from Mercury, felt by everything around the Sun, is one.
fn assess(
    x: DVec3,
    v: DVec3,
    center: usize,
    context: &Context,
    bodies: &[(DVec3, DVec3)],
) -> (DVec3, f64) {
    let (at, moving) = bodies[center];
    let gm_center = context.gms[center];
    let r = (x - at).length();
    let central = gm_center / (r * r);
    let orbital_speed = (gm_center / r).sqrt();
    let allowed = TOLERANCE * orbital_speed;
    let mut kick = DVec3::ZERO;
    let mut step = f64::INFINITY;
    for (k, (position, velocity)) in bodies.iter().enumerate() {
        let gm = context.gms[k];
        if k == center || gm == 0.0 {
            continue;
        }
        for (from, speed, sign) in [(x, v, 1.0), (at, moving, -1.0)] {
            let toward = *position - from;
            let d2 = toward.length_squared();
            let d = d2.sqrt();
            let a = gm / d2;
            kick += toward * (sign * a / d);
            let crossing = d / (*velocity - speed).length().max(1e-30);
            let tau = crossing.min(d / (gm / d).sqrt());
            let gained = a * tau;
            if gained > allowed {
                step = step.min(tau * (allowed / gained).sqrt());
            }
        }
    }
    let strength = kick.length() / central;
    let period = std::f64::consts::TAU * r / orbital_speed;
    if strength > 0.0 {
        step = step.min(period * (TOLERANCE / strength).sqrt());
    }
    (kick, step)
}

/// Steps one particle from the context's start to its end. `scratch`
/// holds the bodies' states at times within the step.
fn step(particle: Particle, context: &Context, scratch: &mut Vec<(DVec3, DVec3)>) -> Outcome {
    let Particle {
        mut position,
        mut velocity,
        id,
    } = particle;
    let mut t = context.t0;
    let mut at_start = true;
    let mut center = 0;
    // The kick and step limit at the end of the last step, for the center
    // then.
    let mut cached: Option<(usize, DVec3, f64)> = None;
    let span = context.t1 - context.t0;
    let (way, shortest) = (span.signum(), span.abs() / MAX_SUBSTEPS as f64);
    while (context.t1 - t) * way > 0.0 {
        let bodies: &[(DVec3, DVec3)] = if at_start { &context.start } else { scratch };
        center = center_of(position, velocity, context.gms, &context.hill, bodies);
        let (kick, limit) = match cached {
            Some((c, kick, limit)) if c == center => (kick, limit),
            _ => assess(position, velocity, center, context, bodies),
        };
        let left = (context.t1 - t).abs();
        let size = limit.max(shortest).min(left);
        let t_next = if size >= left * (1.0 - 1e-9) {
            context.t1
        } else {
            t + way * size
        };
        // Signed: negative going back.
        let dt = t_next - t;
        // Kick, drift, kick, in the center's frame.
        let (at, moving) = bodies[center];
        let carried = drift(
            position - at,
            velocity - moving + kick * (0.5 * dt),
            context.gms[center],
            dt,
        );
        if carried.closest <= context.radii[center] {
            return Outcome::Swallowed(center);
        }
        let bodies: &[(DVec3, DVec3)] = if t_next == context.t1 {
            &context.end
        } else {
            context
                .field
                .states(context.field.locate(t_next), t_next, scratch);
            at_start = false;
            scratch
        };
        let (at, moving) = bodies[center];
        position = at + carried.position;
        velocity = moving + carried.velocity;
        let (kick, limit) = assess(position, velocity, center, context, bodies);
        velocity += kick * (0.5 * dt);
        cached = Some((center, kick, limit));
        for (k, (body, _)) in bodies.iter().enumerate() {
            if (*body - position).length_squared() <= context.radii[k] * context.radii[k] {
                return Outcome::Swallowed(k);
            }
        }
        t = t_next;
    }
    Outcome::Moved(
        Particle {
            position,
            velocity,
            id,
        },
        center as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{AU, DAY, GM_SUN, SOLAR_RADIUS};
    use crate::orbit::{Elements, KeplerOrbit};

    /// A field with the Sun alone, at rest at the origin, for `days`.
    fn sun_alone(days: f64) -> Field {
        let mut field = Field::default();
        let still = vec![[DVec3::ZERO; 3]];
        field.record(
            0,
            || Bodies {
                names: vec!["Sun".into()],
                gms: vec![GM_SUN],
                radii: vec![SOLAR_RADIUS],
            },
            (0.0, still.clone()),
            (days * DAY, still),
        );
        field
    }

    #[test]
    fn alone_with_the_sun_a_particle_keeps_its_ellipse() {
        // With nothing else pulling, the drifts are exact: after 1,000
        // days, the swarm's asteroid is where Kepler's equation puts it.
        let elements = Elements {
            a: 2.5 * AU,
            e: 0.2,
            inclination: 0.1,
            node: 0.4,
            periapsis: 1.3,
            mean_anomaly: 2.0,
            epoch: 0.0,
        };
        let orbit = KeplerOrbit::new(&elements, GM_SUN);
        let (r, v) = orbit.state_at(0.0);
        let mut swarm = Swarm::new(0.0);
        swarm.add([Particle {
            position: r,
            velocity: v,
            id: 7,
        }]);
        let mut field = sun_alone(1000.0);
        let swallowed = swarm.advance(&mut field, 1000.0 * DAY);
        assert!(swallowed.is_empty());
        let expected = orbit.position_at(1000.0 * DAY);
        let error = (swarm.particles[0].position - expected).length() / expected.length();
        assert!(error < 1e-12, "{error:e}");
        assert_eq!(swarm.centers(), Some((0, &[0][..])));
    }

    #[test]
    fn the_swarm_runs_backward_as_well_as_forward() {
        // Out and back over 1,000 days lands where it started, to rounding.
        let orbit = KeplerOrbit::new(
            &Elements {
                a: 3.1 * AU,
                e: 0.3,
                inclination: 0.2,
                node: 2.0,
                periapsis: 0.5,
                mean_anomaly: 4.0,
                epoch: 0.0,
            },
            GM_SUN,
        );
        let (r, v) = orbit.state_at(0.0);
        let mut swarm = Swarm::new(0.0);
        swarm.add([Particle {
            position: r,
            velocity: v,
            id: 1,
        }]);
        let mut field = sun_alone(1000.0);
        swarm.advance(&mut field, 1000.0 * DAY);
        let mut field = sun_alone(1000.0);
        swarm.advance(&mut field, 0.0);
        assert_eq!(swarm.time(), 0.0);
        assert!((swarm.particles[0].position - r).length() < 1e-9 * r.length());
    }

    #[test]
    fn a_particle_that_reaches_the_surface_is_swallowed() {
        // Falling from 1 AU at rest: the fall takes 64.6 days.
        let mut swarm = Swarm::new(0.0);
        swarm.add([Particle {
            position: DVec3::new(AU, 0.0, 0.0),
            velocity: DVec3::new(0.0, 1.0, 0.0),
            id: 3,
        }]);
        let mut field = sun_alone(100.0);
        let swallowed = swarm.advance(&mut field, 100.0 * DAY);
        assert_eq!(swallowed.len(), 1);
        assert_eq!(
            (swallowed[0].by.as_str(), swallowed[0].ids.as_slice()),
            ("Sun", &[3][..])
        );
        assert!(swarm.is_empty());
    }
}
