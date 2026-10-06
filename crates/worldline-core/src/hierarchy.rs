//! Hierarchical integration: planets with moons, simulated in their own
//! frames at their own pace.
//!
//! Io circles Jupiter in 1.8 days and Phobos circles Mars in 7.7 hours.
//! Putting them in the same integration as the planets would force the
//! whole solar system onto steps of minutes. Instead:
//!
//! 1. The **top level** (the Sun, planets, and each planetary system as a
//!    single point at its barycenter) steps with its own integrator and
//!    gravity model, as before.
//! 2. Each **moon system** (a planet and its moons) then catches up over
//!    the same interval, in its own barycentric frame, with its own
//!    integrator taking as many small steps as it needs. Inside, bodies
//!    feel each other, the planet's oblateness and lumps, the shapes of
//!    tidally locked moons, and the tides of the Sun and other planets,
//!    whose positions are interpolated across the step.
//!
//! 3. **Small moons**, when a moon system has them switched on, are too
//!    small to pull on the major moons. They follow the major moons' paths,
//!    recorded step by step and interpolated. The regular ones (near the
//!    planet's equator) share an integrator, so they can pull on each
//!    other; the irregular ones (distant, tilted) each have their own and
//!    keep their own slow pace.
//!
//! 4. **Followers**: bodies at the top level too light to pull on anything
//!    (comets, small asteroids) follow the recorded paths of the Sun,
//!    planets and massive asteroids, each with its own integrator, so a
//!    comet grazing the Sun doesn't shrink everyone's steps.
//!
//! The top level sees each system as a point mass at its barycenter, which
//! is exact apart from tiny tidal coupling terms (relative size ~ (r/R)²
//! times the moons' share of the mass). See `docs/physics/moons.md` and
//! `docs/physics/small-moons.md`.

use glam::DVec3;

use crate::constants::C;
use crate::gravity::{
    Gravity, Newtonian, NonGravitational, SynchronousFigure, TesseralField, ZonalField,
};
use crate::integrator::{Ias15, Integrator, advance};
use crate::{Body, System};

/// Name of the gravity model inside moon systems, for display.
pub const MOON_SYSTEM_GRAVITY: &str = "Newtonian + planet's gravity field + tides";

/// A planet and its moons.
pub struct MoonSystem {
    /// Index of the planetary system's barycenter among the top-level bodies.
    pub host: usize,
    /// The planet (first) and its moons, relative to the system's
    /// barycenter, which sits at the host's position.
    pub system: System,
    /// The planet's oblateness, if known.
    pub zonal: Option<ZonalField>,
    /// The planet's longitude-dependent lumps, if known.
    pub tesseral: Option<TesseralField>,
    /// Shapes of tidally locked moons, by index into `system.bodies`.
    pub figures: Vec<(usize, SynchronousFigure)>,
    /// GM of small moons orbiting inside the major moons, included in the
    /// planet's GM: to the major moons they pull like mass at the planet's
    /// center. Simulated small moons feel them individually instead.
    pub inner_mass: f64,
    /// The planet's oblateness as the small moons' own JPL solution has it,
    /// if that differs from the major moons'. Small moons feel this one.
    pub small_moon_zonal: Option<ZonalField>,
    /// Small moons being simulated, in groups that share an integrator.
    groups: Vec<ParticleGroup>,
    /// Where each small moon is: (group, index within the group).
    slots: Vec<(usize, usize)>,
    integrator: Ias15,
}

/// Small moons integrated together, following the major moons' paths.
struct ParticleGroup {
    /// Positions relative to the moon system's barycenter.
    system: System,
    /// GM of the planet as these moons feel it.
    central_gm: f64,
    integrator: Ias15,
}

impl MoonSystem {
    /// A planet (first) and its moons, in the top level's frame, whose
    /// barycenter is top-level body `host`.
    ///
    /// The moon system keeps its own GMs, which should come from the same
    /// solution as the moons' positions; the host's GM (from the planetary
    /// ephemeris) only governs the system's path around the Sun.
    pub fn new(host: usize, bodies: Vec<Body>) -> Self {
        Self {
            host,
            system: System::new(bodies),
            zonal: None,
            tesseral: None,
            figures: Vec::new(),
            inner_mass: 0.0,
            small_moon_zonal: None,
            groups: Vec::new(),
            slots: Vec::new(),
            integrator: Ias15::new(),
        }
    }

    /// Adds the GM (m³/s²) of small moons that orbit inside the major moons
    /// to the planet's, for the major moons' sake. See [`Self::inner_mass`].
    pub fn with_inner_mass(mut self, gm: f64) -> Self {
        self.system.bodies[0].gm += gm;
        self.inner_mass += gm;
        self
    }

    /// Starts simulating small moons, given with positions and velocities
    /// relative to the planet's center and whether each is regular,
    /// replacing any already simulated. Their GMs pull on each other but
    /// not on the planet or major moons.
    ///
    /// The regular moons (those orbiting near the planet's equator, which
    /// JPL describes relative to a Laplace or equatorial plane) are
    /// integrated together, at the pace of the fastest, so the ones with
    /// mass pull on each other. They should include every small moon behind
    /// [`Self::inner_mass`], which they then feel one by one. The irregular
    /// moons (distant and tilted, thousands of moon radii apart) each get
    /// their own integrator and pace, and feel the planet's full GM.
    pub fn set_small_moons(&mut self, moons: Vec<(Body, bool)>) {
        let planet = &self.system.bodies[0];
        let (origin, drift) = (planet.position, planet.velocity);
        let time = self.system.time();
        let close_gm = planet.gm - self.inner_mass;
        let full_gm = planet.gm;

        self.groups.clear();
        self.slots.clear();
        let mut close = Vec::new();
        let mut close_slots = Vec::new();
        for (i, (moon, regular)) in moons.into_iter().enumerate() {
            let moon = Body {
                position: moon.position + origin,
                velocity: moon.velocity + drift,
                ..moon
            };
            if regular {
                self.slots.push((usize::MAX, close.len()));
                close_slots.push(i);
                close.push(moon);
            } else {
                self.slots.push((self.groups.len(), 0));
                self.groups
                    .push(ParticleGroup::new(vec![moon], full_gm, time));
            }
        }
        if !close.is_empty() {
            let group = self.groups.len();
            for &i in &close_slots {
                self.slots[i].0 = group;
            }
            self.groups.push(ParticleGroup::new(close, close_gm, time));
        }
    }

    /// Stops simulating small moons.
    pub fn clear_small_moons(&mut self) {
        self.groups.clear();
        self.slots.clear();
    }

    /// How many small moons are being simulated.
    pub fn small_moon_count(&self) -> usize {
        self.slots.len()
    }

    /// Small moon `index` (in the order given to [`Self::set_small_moons`]),
    /// relative to the moon system's barycenter.
    pub fn small_moon(&self, index: usize) -> &Body {
        let (group, slot) = self.slots[index];
        &self.groups[group].system.bodies[slot]
    }
}

impl ParticleGroup {
    fn new(bodies: Vec<Body>, central_gm: f64, time: f64) -> Self {
        let mut system = System::new(bodies);
        system.set_time(time);
        Self {
            system,
            central_gm,
            integrator: Ias15::new(),
        }
    }
}

impl MoonSystem {
    /// Adds the planet's oblateness.
    pub fn with_zonal(mut self, zonal: ZonalField) -> Self {
        self.zonal = Some(zonal);
        self
    }

    /// Adds the planet's longitude-dependent lumps.
    pub fn with_tesseral(mut self, tesseral: TesseralField) -> Self {
        self.tesseral = Some(tesseral);
        self
    }

    /// Adds the shape of moon `index` (an index into the bodies, so at
    /// least 1), which keeps its long axis toward the planet.
    pub fn with_figure(mut self, index: usize, figure: SynchronousFigure) -> Self {
        assert!(index > 0 && index < self.system.bodies.len(), "not a moon");
        self.figures.push((index, figure));
        self
    }
}

/// The solar system as a top level plus moon systems.
pub struct Hierarchy {
    /// The Sun, planets and planetary-system barycenters.
    pub top: System,
    /// Planets with moons.
    pub moon_systems: Vec<MoonSystem>,
    gravity: Box<dyn Gravity + Send + Sync>,
    integrator: Ias15,
    followers: Vec<Follower>,
}

/// A body too light to pull on anything, following the top level.
struct Follower {
    /// The body alone, in the top level's frame.
    system: System,
    forces: Option<NonGravitational>,
    integrator: Ias15,
}

impl Hierarchy {
    /// Builds a hierarchy. Each moon system's bodies are shifted so its
    /// barycenter sits at rest at its host. A moon system's total GM and
    /// its host's may differ slightly (they come from different JPL
    /// solutions), and must agree to 1 part in 10,000.
    pub fn new(
        top: System,
        gravity: Box<dyn Gravity + Send + Sync>,
        moon_systems: Vec<MoonSystem>,
    ) -> Self {
        let moon_systems = moon_systems
            .into_iter()
            .map(|mut moons| {
                let host_gm = top.bodies[moons.host].gm;
                assert!(
                    (moons.system.total_gm() / host_gm - 1.0).abs() < 1e-4,
                    "moon system's mass disagrees with its host's"
                );
                moons.system.move_to_barycentric_frame();
                moons.system.set_time(top.time());
                moons
            })
            .collect();
        Self {
            top,
            moon_systems,
            gravity,
            integrator: Ias15::new(),
            followers: Vec::new(),
        }
    }

    /// Adds bodies that follow the top level without pulling on it, given
    /// in the top level's frame, each with its non-gravitational force
    /// model if it has one. Each feels every top-level body's Newtonian
    /// pull, plus the Sun's (top-level body 0) first relativistic
    /// correction.
    pub fn add_followers(&mut self, bodies: Vec<(Body, Option<NonGravitational>)>) {
        let time = self.top.time();
        for (body, forces) in bodies {
            let mut system = System::new(vec![body]);
            system.set_time(time);
            self.followers.push(Follower {
                system,
                forces,
                integrator: Ias15::new(),
            });
        }
    }

    /// How many followers there are.
    pub fn follower_count(&self) -> usize {
        self.followers.len()
    }

    /// Follower `index`, in the top level's frame.
    pub fn follower(&self, index: usize) -> &Body {
        &self.followers[index].system.bodies[0]
    }

    /// Simulation time, in s.
    pub fn time(&self) -> f64 {
        self.top.time()
    }

    /// The top level's gravity model.
    pub fn gravity(&self) -> &dyn Gravity {
        self.gravity.as_ref()
    }

    /// Sets every level's clock, to land exactly on a target time after a
    /// step that reached it up to rounding.
    pub fn set_time(&mut self, time: f64) {
        self.top.set_time(time);
        for moons in &mut self.moon_systems {
            moons.system.set_time(time);
            for group in &mut moons.groups {
                group.system.set_time(time);
            }
        }
        for follower in &mut self.followers {
            follower.system.set_time(time);
        }
    }

    /// Advances everything by one top-level step of at most `max_dt`
    /// seconds. Returns the length of the step.
    pub fn step(&mut self, max_dt: f64) -> f64 {
        let t0 = self.top.time();
        let before: Vec<(DVec3, DVec3)> = self
            .top
            .bodies
            .iter()
            .map(|b| (b.position, b.velocity))
            .collect();
        let track = (!self.followers.is_empty())
            .then(|| Track::starting_at(&self.top, self.gravity.as_ref()));
        // The Sun alone has nothing to integrate (and a zero force would
        // leave the integrator's error estimate only rounding noise).
        let taken = if self.top.bodies.len() == 1 {
            self.top.tick(max_dt);
            max_dt
        } else {
            self.integrator
                .step(&mut self.top, self.gravity.as_ref(), max_dt)
        };
        let t1 = self.top.time();
        if let Some(mut track) = track {
            track.record(&self.top, self.gravity.as_ref());
            // Followers run on a clock that starts at zero with each step.
            // Seconds since the snapshot reach 10⁸ within a few years, where
            // a double resolves only about 10⁻⁸ s; in a close flyby (Apophis
            // past Earth in 2029) the integrator takes steps short enough
            // for that graininess to look like error, and would shrink its
            // steps without end.
            track.rebase(t0);
            let gms: Vec<f64> = self.top.bodies.iter().map(|b| b.gm).collect();
            for follower in &mut self.followers {
                let gravity = FollowerGravity {
                    track: &track,
                    gms: &gms,
                    forces: follower.forces,
                };
                follower.system.set_time(0.0);
                advance(
                    &mut follower.system,
                    &gravity,
                    &mut follower.integrator,
                    t1 - t0,
                );
                follower.system.set_time(t1);
            }
        }

        for moons in &mut self.moon_systems {
            // Everything else at the top level, seen from this system's
            // barycenter at the start and end of the step.
            let host = moons.host;
            let perturbers = self
                .top
                .bodies
                .iter()
                .enumerate()
                .filter(|(k, _)| *k != host)
                .map(|(k, b)| Perturber {
                    gm: b.gm,
                    start: (before[k].0 - before[host].0, before[k].1 - before[host].1),
                    end: (
                        b.position - self.top.bodies[host].position,
                        b.velocity - self.top.bodies[host].velocity,
                    ),
                })
                .collect();
            let tides = Tides { perturbers, t0, t1 };
            let gravity = MoonSystemGravity {
                zonal: moons.zonal.as_ref(),
                tesseral: moons.tesseral.as_ref(),
                figures: &moons.figures,
                tides: &tides,
            };
            // A planet without major moons sits still at its system's
            // barycenter: nothing to integrate (and its forces, zero up to
            // rounding, would leave the integrator's error estimate
            // nothing but noise to go on).
            let lone = moons.system.bodies.len() == 1;
            if moons.groups.is_empty() {
                if !lone {
                    let interval = t1 - moons.system.time();
                    advance(&mut moons.system, &gravity, &mut moons.integrator, interval);
                }
                moons.system.set_time(t1);
                continue;
            }

            // With small moons along, record the major moons' path step by
            // step, as advance() would take it, for the small moons to follow.
            let mut track = Track::starting_at(&moons.system, &gravity);
            while moons.system.time() < t1 {
                let remaining = t1 - moons.system.time();
                let taken = if lone {
                    remaining
                } else {
                    moons
                        .integrator
                        .step(&mut moons.system, &gravity, remaining)
                };
                assert!(taken > 0.0, "the moon system took an empty step");
                if taken >= remaining {
                    moons.system.set_time(t1);
                }
                track.record(&moons.system, &gravity);
            }
            let gms: Vec<f64> = moons.system.bodies.iter().map(|b| b.gm).collect();
            for group in &mut moons.groups {
                let gravity = SmallMoonGravity {
                    track: &track,
                    gms: &gms,
                    central_gm: group.central_gm,
                    zonal: moons.small_moon_zonal.as_ref().or(moons.zonal.as_ref()),
                    tesseral: moons.tesseral.as_ref(),
                    tides: &tides,
                };
                let interval = t1 - group.system.time();
                advance(&mut group.system, &gravity, &mut group.integrator, interval);
                group.system.set_time(t1);
            }
        }
        taken
    }

    /// Advances everything by exactly `duration` seconds.
    pub fn advance(&mut self, duration: f64) {
        let end = self.time() + duration;
        while self.time() < end {
            let remaining = end - self.time();
            if self.step(remaining) >= remaining {
                // Land exactly on `end`. Otherwise rounding can leave a
                // sliver too thin to move the clock, and the loop never ends.
                self.set_time(end);
            }
        }
    }

    /// Position and velocity of body `body` of moon system `index`, in the
    /// top level's frame.
    pub fn absolute(&self, index: usize, body: usize) -> (DVec3, DVec3) {
        let moons = &self.moon_systems[index];
        let host = &self.top.bodies[moons.host];
        let b = &moons.system.bodies[body];
        (host.position + b.position, host.velocity + b.velocity)
    }

    /// Position and velocity of small moon `moon` of moon system `index`,
    /// in the top level's frame.
    pub fn absolute_small(&self, index: usize, moon: usize) -> (DVec3, DVec3) {
        let moons = &self.moon_systems[index];
        let host = &self.top.bodies[moons.host];
        let b = moons.small_moon(moon);
        (host.position + b.position, host.velocity + b.velocity)
    }
}

/// Cubic Hermite interpolation between two states (position, velocity)
/// `h` seconds apart, at fraction `s` of the way. Good enough for the
/// distant Sun and planets, which barely curve during a step.
fn hermite(start: (DVec3, DVec3), end: (DVec3, DVec3), s: f64, h: f64) -> DVec3 {
    let (s2, s3) = (s * s, s * s * s);
    start.0 * (2.0 * s3 - 3.0 * s2 + 1.0)
        + start.1 * (h * (s3 - 2.0 * s2 + s))
        + end.0 * (-2.0 * s3 + 3.0 * s2)
        + end.1 * (h * (s3 - s2))
}

/// The tides of the top-level bodies on a moon system during one top-level
/// step.
struct Tides {
    perturbers: Vec<Perturber>,
    t0: f64,
    t1: f64,
}

impl Tides {
    /// Adds each outside body's pull on every body, minus its pull on the
    /// barycenter (the origin), which the top level already handles. The
    /// outside bodies' positions come from cubic Hermite interpolation
    /// between the step's ends.
    fn add(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        let h = self.t1 - self.t0;
        let s = if h > 0.0 {
            ((time - self.t0) / h).clamp(0.0, 1.0)
        } else {
            1.0
        };
        for p in &self.perturbers {
            let r = hermite(p.start, p.end, s, h);
            let on_barycenter = r * (p.gm / r.length().powi(3));
            for (body, a) in bodies.iter().zip(out.iter_mut()) {
                let d = r - body.position;
                *a += d * (p.gm / d.length().powi(3)) - on_barycenter;
            }
        }
    }
}

/// The major bodies' states at the end of each of their steps through one
/// top-level step.
///
/// Between records, positions come from quintic Hermite interpolation of
/// position, velocity and acceleration at both ends, accurate to about
/// (ωh)⁶ a / 46,080 for an orbit of radius a and angular rate ω over a step
/// of length h. IAS15 takes steps of a few percent of an orbit, so the
/// error is meters at Io.
struct Track {
    times: Vec<f64>,
    /// Each body's position, velocity and acceleration at each record.
    states: Vec<Vec<[DVec3; 3]>>,
}

impl Track {
    fn starting_at(system: &System, gravity: &dyn Gravity) -> Self {
        let mut track = Self {
            times: Vec::new(),
            states: Vec::new(),
        };
        track.record(system, gravity);
        track
    }

    fn record(&mut self, system: &System, gravity: &dyn Gravity) {
        let mut accelerations = vec![DVec3::ZERO; system.bodies.len()];
        gravity.accelerations(system.time(), &system.bodies, &mut accelerations);
        self.times.push(system.time());
        self.states.push(
            system
                .bodies
                .iter()
                .zip(accelerations)
                .map(|(b, a)| [b.position, b.velocity, a])
                .collect(),
        );
    }

    /// Counts the recorded times from `origin`.
    fn rebase(&mut self, origin: f64) {
        for time in &mut self.times {
            *time -= origin;
        }
    }

    /// The recorded step containing `time`: its index, how far through it
    /// `time` is (0 to 1), and its length.
    fn locate(&self, time: f64) -> (usize, f64, f64) {
        if self.times.len() < 2 {
            return (0, 0.0, 0.0);
        }
        let k = self
            .times
            .partition_point(|&t| t <= time)
            .saturating_sub(1)
            .min(self.times.len() - 2);
        let h = self.times[k + 1] - self.times[k];
        let s = if h > 0.0 {
            ((time - self.times[k]) / h).clamp(0.0, 1.0)
        } else {
            1.0
        };
        (k, s, h)
    }

    /// Body `body`'s velocity at a place found by [`Self::locate`], by
    /// linear interpolation (used only for the Sun, which barely changes
    /// speed within a step).
    fn velocity(&self, (k, s, h): (usize, f64, f64), body: usize) -> DVec3 {
        let v0 = self.states[k][body][1];
        if h == 0.0 {
            return v0;
        }
        v0.lerp(self.states[k + 1][body][1], s)
    }

    /// Body `body`'s position at a place found by [`Self::locate`].
    fn position(&self, (k, s, h): (usize, f64, f64), body: usize) -> DVec3 {
        let [p0, v0, a0] = self.states[k][body];
        if h == 0.0 {
            return p0;
        }
        let [p1, v1, a1] = self.states[k + 1][body];
        // The quintic Hermite basis on [0, 1].
        let (s2, s3) = (s * s, s * s * s);
        let (s4, s5) = (s3 * s, s3 * s2);
        p0 * (1.0 - 10.0 * s3 + 15.0 * s4 - 6.0 * s5)
            + v0 * (h * (s - 6.0 * s3 + 8.0 * s4 - 3.0 * s5))
            + a0 * (h * h * (0.5 * s2 - 1.5 * s3 + 1.5 * s4 - 0.5 * s5))
            + a1 * (h * h * (0.5 * s3 - s4 + 0.5 * s5))
            + v1 * (h * (-4.0 * s3 + 7.0 * s4 - 3.0 * s5))
            + p1 * (10.0 * s3 - 15.0 * s4 + 6.0 * s5)
    }
}

/// Gravity on a follower: every top-level body's Newtonian pull along its
/// recorded path, the Sun's first relativistic correction, and the
/// follower's non-gravitational force, if any.
struct FollowerGravity<'a> {
    track: &'a Track,
    /// GMs of the top-level bodies, in the track's order; body 0 is the Sun.
    gms: &'a [f64],
    forces: Option<NonGravitational>,
}

impl Gravity for FollowerGravity<'_> {
    fn name(&self) -> &'static str {
        "Newtonian + the Sun's 1PN term + non-gravitational forces"
    }

    fn velocity_dependent(&self) -> bool {
        true
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        let at = self.track.locate(time);
        let positions: Vec<DVec3> = (0..self.gms.len())
            .map(|j| self.track.position(at, j))
            .collect();
        let sun_velocity = self.track.velocity(at, 0);
        let mu = self.gms[0];
        for (body, a) in bodies.iter().zip(out.iter_mut()) {
            *a = DVec3::ZERO;
            for (position, gm) in positions.iter().zip(self.gms) {
                let d = *position - body.position;
                *a += d * (gm / d.length().powi(3));
            }
            // The Sun's first post-Newtonian term for a test body, in
            // harmonic coordinates with β = γ = 1 (the Schwarzschild term,
            // IERS Conventions 2010, eq. 10.12; the one-body limit of
            // Moyer 2003, eq. 4-61): (μ/c²r³)[(4μ/r − v²) r + 4 (r·v) v].
            let r = body.position - positions[0];
            let v = body.velocity - sun_velocity;
            let distance = r.length();
            *a += (r * (4.0 * mu / distance - v.length_squared()) + v * (4.0 * r.dot(v)))
                * (mu / (C * C * distance.powi(3)));
            if let Some(forces) = self.forces {
                *a += forces.acceleration(r, v, mu);
            }
        }
    }
}

/// Gravity on small moons: the planet's field, the major moons along their
/// recorded paths, the small moons' own pulls on each other, and the tides.
struct SmallMoonGravity<'a> {
    track: &'a Track,
    /// GMs of the planet and major moons, in the track's order.
    gms: &'a [f64],
    /// GM of the planet as these moons feel it.
    central_gm: f64,
    zonal: Option<&'a ZonalField>,
    tesseral: Option<&'a TesseralField>,
    tides: &'a Tides,
}

impl Gravity for SmallMoonGravity<'_> {
    fn name(&self) -> &'static str {
        MOON_SYSTEM_GRAVITY
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        let at = self.track.locate(time);
        let planet = self.track.position(at, 0);
        let majors: Vec<DVec3> = (1..self.gms.len())
            .map(|j| self.track.position(at, j))
            .collect();
        let frame = self.tesseral.map(|t| t.frame_at(time));
        for (i, (body, a)) in bodies.iter().zip(out.iter_mut()).enumerate() {
            let r = body.position - planet;
            *a = -r * (self.central_gm / r.length().powi(3));
            if let Some(zonal) = self.zonal {
                *a += zonal.acceleration(self.central_gm, r);
            }
            if let (Some(tesseral), Some(frame)) = (self.tesseral, frame) {
                *a += tesseral.acceleration_in_frame(self.central_gm, r, frame);
            }
            for (position, gm) in majors.iter().zip(&self.gms[1..]) {
                let d = *position - body.position;
                *a += d * (gm / d.length().powi(3));
            }
            for (k, other) in bodies.iter().enumerate() {
                if k != i && other.gm > 0.0 {
                    let d = other.position - body.position;
                    *a += d * (other.gm / d.length().powi(3));
                }
            }
        }
        self.tides.add(time, bodies, out);
    }
}

/// A top-level body pulling on a moon system, with its position and
/// velocity relative to the system's barycenter at both ends of the step.
struct Perturber {
    gm: f64,
    start: (DVec3, DVec3),
    end: (DVec3, DVec3),
}

/// Gravity inside a moon system: the bodies' mutual pull, the planet's
/// oblateness and lumps (body 0 is the planet), the moons' shapes, and the
/// tides of everything outside.
struct MoonSystemGravity<'a> {
    zonal: Option<&'a ZonalField>,
    tesseral: Option<&'a TesseralField>,
    figures: &'a [(usize, SynchronousFigure)],
    tides: &'a Tides,
}

impl Gravity for MoonSystemGravity<'_> {
    fn name(&self) -> &'static str {
        MOON_SYSTEM_GRAVITY
    }

    fn accelerations(&self, time: f64, bodies: &[Body], out: &mut [DVec3]) {
        Newtonian.accelerations(time, bodies, out);

        // The planet's bulge and lumps pull on each moon, and each moon
        // pulls back on the planet with the opposite force.
        let planet = &bodies[0];
        let frame = self.tesseral.map(|t| t.frame_at(time));
        for i in 1..bodies.len() {
            let r = bodies[i].position - planet.position;
            let mut a = DVec3::ZERO;
            if let Some(zonal) = self.zonal {
                a += zonal.acceleration(planet.gm, r);
            }
            if let (Some(tesseral), Some(frame)) = (self.tesseral, frame) {
                a += tesseral.acceleration_in_frame(planet.gm, r, frame);
            }
            out[i] += a;
            out[0] -= a * (bodies[i].gm / planet.gm);
        }

        // A locked moon's long axis, aimed at the planet, pulls the two
        // together a little harder.
        for &(i, figure) in self.figures {
            let a = figure.acceleration(planet.gm, bodies[i].position - planet.position);
            out[i] += a;
            out[0] -= a * (bodies[i].gm / planet.gm);
        }

        self.tides.add(time, bodies, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DMat3;
    use crate::constants::{AU, DAY, GM_SUN};
    use crate::orbit::{kepler_period, periapsis_state};

    /// A Sun, plus a "planetary system" barycenter 5 AU out holding a planet
    /// and one moon.
    fn star_planet_moon() -> Hierarchy {
        let (gm_planet, gm_moon) = (1.267e17, 5.96e12);
        let (r, v) = periapsis_state(5.2 * AU, 0.0, GM_SUN + gm_planet + gm_moon);
        let top = System::new(vec![
            Body::new("Sun", GM_SUN, 7e8),
            Body::new("system", gm_planet + gm_moon, 7e7)
                .at(r)
                .moving(v),
        ]);
        let (mr, mv) = periapsis_state(4.217e8, 0.004, gm_planet + gm_moon);
        let bodies = vec![
            Body::new("planet", gm_planet, 7e7).at(r).moving(v),
            Body::new("moon", gm_moon, 1.8e6).at(r + mr).moving(v + mv),
        ];
        Hierarchy::new(top, Box::new(Newtonian), vec![MoonSystem::new(1, bodies)])
    }

    #[test]
    fn a_lone_small_moon_keeps_keplers_period() {
        // Around a planet with no major moons, a small moon follows Kepler's
        // orbit, bent only by the Sun's tide (2 × 10⁻⁷ of the planet's pull
        // at 5 AU): after one period it must be back within 0.001°.
        let (gm_planet, a) = (1.267e17, 4.217e8);
        let (r, v) = periapsis_state(5.2 * AU, 0.0, GM_SUN + gm_planet);
        let top = System::new(vec![
            Body::new("Sun", GM_SUN, 7e8),
            Body::new("system", gm_planet, 7e7).at(r).moving(v),
        ]);
        let planet = Body::new("planet", gm_planet, 7e7).at(r).moving(v);
        let mut h = Hierarchy::new(
            top,
            Box::new(Newtonian),
            vec![MoonSystem::new(1, vec![planet])],
        );
        let (mr, mv) = periapsis_state(a, 0.0, gm_planet);
        h.moon_systems[0].set_small_moons(vec![(
            Body::new("small", 0.0, 1e4).at(mr).moving(mv),
            false,
        )]);
        h.advance(kepler_period(a, gm_planet));
        let (planet, _) = h.absolute(0, 0);
        let (moon, _) = h.absolute_small(0, 0);
        let angle = mr.angle_between(moon - planet).to_degrees();
        assert!(angle < 1e-3, "small moon off by {angle}° after one period");
    }

    #[test]
    fn a_follower_on_mercurys_orbit_precesses_as_relativity_says() {
        // A massless body on Mercury's orbit, following a lone Sun: its
        // perihelion must turn by 6πGM/(c²a(1 − e²)) per orbit, the same
        // 43″ per century as Mercury's (validation_mercury.rs). Measured
        // over 10 orbits from the Laplace–Runge–Lenz vector; the bound is
        // the 0.01% the Mercury test holds.
        let (a, e) = (0.387_098_93 * AU, 0.205_630_69);
        let top = System::new(vec![Body::new("Sun", GM_SUN, 7e8)]);
        let mut h = Hierarchy::new(top, Box::new(Newtonian), Vec::new());
        let (r, v) = periapsis_state(a, e, GM_SUN);
        h.add_followers(vec![(
            Body::new("follower", 0.0, 1.0).at(r).moving(v),
            None,
        )]);
        let perihelion = |h: &Hierarchy| {
            let b = h.follower(0);
            let lrl =
                b.velocity.cross(b.position.cross(b.velocity)) - GM_SUN * b.position.normalize();
            lrl.y.atan2(lrl.x)
        };
        let period = kepler_period(a, GM_SUN);
        let start = perihelion(&h);
        h.advance(10.0 * period);
        let turned = perihelion(&h) - start;
        let theory = 10.0 * 6.0 * std::f64::consts::PI * GM_SUN / (C * C * a * (1.0 - e * e));
        println!("perihelion turned {turned:.6e} rad, theory {theory:.6e} rad");
        assert!((turned / theory - 1.0).abs() < 1e-4);
    }

    #[test]
    fn a_small_moon_at_l4_stays_in_its_tadpole() {
        // A small moon sharing a major moon's orbit, 60° ahead of it, sits
        // at the Lagrange point L4. The major moon's pull keeps it there
        // (Telesto and Calypso do this with Tethys). In the restricted
        // three-body problem such a moon can't come closer than 23.9° to
        // the major moon or pass behind it (Murray & Dermott 1999, §3.9).
        // Following the major moon's interpolated path must preserve that.
        let mut h = star_planet_moon();
        let moons = &h.moon_systems[0];
        let (planet, moon) = (&moons.system.bodies[0], &moons.system.bodies[1]);
        let turn = DMat3::from_rotation_z(60f64.to_radians());
        let (r, v) = (
            moon.position - planet.position,
            moon.velocity - planet.velocity,
        );
        let start = Body::new("trojan", 0.0, 1e4).at(turn * r).moving(turn * v);
        let period = kepler_period(4.217e8, moons.system.total_gm());
        h.moon_systems[0].set_small_moons(vec![(start, true)]);
        let (mut closest, mut farthest) = (f64::MAX, 0f64);
        for _ in 0..200 {
            h.advance(period / 2.0);
            let (planet, _) = h.absolute(0, 0);
            let (moon, _) = h.absolute(0, 1);
            let (trojan, _) = h.absolute_small(0, 0);
            let angle = (moon - planet).angle_between(trojan - planet).to_degrees();
            closest = closest.min(angle);
            farthest = farthest.max(angle);
        }
        println!("angle from the major moon over 100 orbits: {closest:.3}° to {farthest:.3}°");
        assert!(closest > 23.9 && farthest < 180.0);
    }

    #[test]
    fn system_stays_centered_on_its_host() {
        let mut h = star_planet_moon();
        h.advance(30.0 * DAY);
        let moons = &h.moon_systems[0];
        assert!(
            moons.system.barycenter().length() < 1e3,
            "barycenter drifted"
        );
        assert_eq!(moons.system.time(), h.time());
    }

    #[test]
    fn moon_orbits_with_keplers_period_without_oblateness() {
        // With no bulge, the only thing bending the moon's orbit away from
        // Kepler's is the Sun's tide, about 2 × 10⁻⁷ of the planet's pull
        // at 5 AU: after one period the moon must be back within 0.001°.
        let mut h = star_planet_moon();
        let moons = &h.moon_systems[0];
        let period = kepler_period(4.217e8, moons.system.total_gm());
        let start = moons.system.bodies[1].position - moons.system.bodies[0].position;
        h.advance(period);
        let moons = &h.moon_systems[0];
        let end = moons.system.bodies[1].position - moons.system.bodies[0].position;
        let angle = start.angle_between(end).to_degrees();
        assert!(angle < 1e-3, "moon off by {angle}° after one period");
    }
}
