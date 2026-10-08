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
//! 5. **The swarm**: tens of thousands more massless bodies (the belts,
//!    small moons torn from their planets) follow the same recorded paths
//!    with a cheaper integrator of their own, in steps of up to 64 days
//!    (see `swarm.rs`).
//!
//! The top level sees each system as a point mass at its barycenter, which
//! is exact apart from tiny tidal coupling terms (relative size ~ (r/R)²
//! times the moons' share of the mass). See `docs/physics/moons.md` and
//! `docs/physics/small-moons.md`.

use glam::DVec3;

use crate::collision::{contact, first_survives, merge};
use crate::constants::C;
use crate::gravity::{
    Gravity, Newtonian, NonGravitational, SynchronousFigure, TesseralField, ZonalField,
    reaction_converges,
};
use crate::integrator::{Ias15, Integrator, advance};
use crate::kepler::drift;
use crate::mean_elements::MeanElements;
use crate::swarm::{Bodies, CADENCE, Field, Particle, Swallowed, Swarm};
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
    /// The small moons' mean orbits, relative to the planet's center, in
    /// their owner's order, for setting them free when they are torn away
    /// while not simulated in detail. Set by the owner; empty if unknown.
    pub small_mean_orbits: Vec<MeanElements>,
    /// Which small moons are already free (swarm particles now).
    small_free: Vec<bool>,
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
            small_mean_orbits: Vec::new(),
            small_free: Vec::new(),
        }
    }

    /// Whether small moon `index` has been set free (see [`FreedMoon`]).
    pub fn is_small_moon_free(&self, index: usize) -> bool {
        self.small_free.get(index).copied().unwrap_or(false)
    }

    /// Marks small moon `index` as set free, as when loading a save.
    pub fn mark_small_moon_free(&mut self, index: usize) {
        if self.small_free.len() <= index {
            self.small_free.resize(index + 1, false);
        }
        self.small_free[index] = true;
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
    /// Whether bodies that touch merge (see [`Collision`]).
    pub collisions: bool,
    gravity: Box<dyn Gravity + Send + Sync>,
    integrator: Ias15,
    followers: Vec<Follower>,
    /// Collisions since the last [`Self::take_collisions`].
    happened: Vec<Collision>,
    /// Moon systems torn apart since the last [`Self::take_unbound`].
    unbound: Vec<Unbound>,
    /// The massless swarm (see `swarm.rs`).
    swarm: Swarm,
    /// The top level's recorded paths, for the swarm.
    field: Field,
    /// Counts changes to the set of top-level bodies, so the swarm knows
    /// which bodies its recorded paths are of.
    generation: u64,
    /// Swarm particles swallowed since the last [`Self::take_swallowed`].
    swallowed: Vec<Swallowed>,
    /// Whether to record the top level's paths even with no swarm.
    recording: bool,
    /// The top level's recorded state at the end of the last step, and the
    /// generation and time then: the next step's start, if nothing changed.
    last_recorded: Option<(u64, f64, Vec<[DVec3; 3]>)>,
    /// The next swarm label for a freed small moon.
    next_id: u32,
    /// Small moons freed since the last [`Self::take_freed_moons`].
    freed_moons: Vec<FreedMoon>,
}

/// A small moon set free: torn from its planet by another body's tides,
/// alone or with its whole moon system (see [`Unbound`]), or left behind
/// by a planet that was absorbed. It goes on as a swarm particle, under the
/// label `id`.
#[derive(Debug, Clone, PartialEq)]
pub struct FreedMoon {
    /// When, as simulation time in s.
    pub time: f64,
    /// Its planet, by name.
    pub planet: String,
    /// Its place in the planet's list of small moons.
    pub index: usize,
    /// Its label in the swarm.
    pub id: u32,
    /// The body whose tides freed it, or that absorbed its planet.
    pub by: String,
}

/// Swarm labels the hierarchy gives the small moons it frees start here,
/// leaving the ones below for its owner.
pub const FREED_MOON_IDS: u32 = 1 << 31;

/// A planet's moons set free: another body came so close that its tide on
/// a moon (how differently it pulls on the moon and the planet) passed 1/12
/// of the planet's own pull. For a distant body that happens at half the
/// planet's Hill radius, beyond which no prograde moon stays bound for long
/// (Domingos, Winter & Yokoyama 2006, *MNRAS* 373, 1227). A separate frame
/// for the moons no longer works, so its major moons join the top level,
/// where everything pulls on everything, and its small moons go on as swarm
/// particles (see [`FreedMoon`]).
#[derive(Debug, Clone, PartialEq)]
pub struct Unbound {
    /// When, as simulation time in s.
    pub time: f64,
    /// The planet whose moons were freed.
    pub planet: String,
    /// The body whose tides did it.
    pub by: String,
    /// The moon system's index (before it was dissolved).
    pub system: usize,
    /// The major moons, now at the top level.
    pub freed: Vec<String>,
}

/// Two bodies that touched and merged: at the top level, the Sun, planets
/// and anything added (the Sun always survives, otherwise the more massive
/// body), or a massless follower (a comet, a small asteroid) that hit one
/// and was absorbed. Collisions inside moon systems aren't detected yet.
#[derive(Debug, Clone, PartialEq)]
pub struct Collision {
    /// When they touched, as simulation time in s. (They merge at the end of
    /// the step they touched in.)
    pub time: f64,
    /// The body that remains.
    pub survivor: String,
    /// The body that merged into it.
    pub absorbed: String,
    /// Their relative speed at the moment of closest approach, in m/s.
    pub speed: f64,
    /// If the absorbed body was a planet with moons, its moon system's index
    /// (before it was dissolved).
    pub dissolved_system: Option<usize>,
    /// The moons that system's planet left behind, now orbiting freely at
    /// the top level.
    pub freed: Vec<String>,
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
            collisions: true,
            gravity,
            integrator: Ias15::new(),
            followers: Vec::new(),
            happened: Vec::new(),
            unbound: Vec::new(),
            swarm: Swarm::new(0.0),
            field: Field::default(),
            generation: 0,
            swallowed: Vec::new(),
            recording: false,
            last_recorded: None,
            next_id: FREED_MOON_IDS,
            freed_moons: Vec::new(),
        }
    }

    /// The small moons set free since the last call, oldest first.
    pub fn take_freed_moons(&mut self) -> Vec<FreedMoon> {
        std::mem::take(&mut self.freed_moons)
    }

    /// Sets the next label for freed small moons, as when loading a save.
    pub fn set_next_freed_id(&mut self, id: u32) {
        self.next_id = id.max(FREED_MOON_IDS);
    }

    /// Small moon `index` of moon system `k`, in the top level's frame:
    /// as simulated, if its system is in detail; otherwise from its mean
    /// orbit around the planet.
    fn small_moon_state(&self, k: usize, index: usize) -> Option<(DVec3, DVec3)> {
        let moons = &self.moon_systems[k];
        let host = &self.top.bodies[moons.host];
        if index < moons.small_moon_count() {
            let b = moons.small_moon(index);
            return Some((host.position + b.position, host.velocity + b.velocity));
        }
        let orbit = moons.small_mean_orbits.get(index)?;
        let (r, v) = orbit.state_at(self.top.time());
        let planet = &moons.system.bodies[0];
        Some((
            host.position + planet.position + r,
            host.velocity + planet.velocity + v,
        ))
    }

    /// Frees small moons `indices` of moon system `k` into the swarm,
    /// reporting each.
    fn free_small_moons(&mut self, k: usize, indices: &[usize], by: &str) {
        let planet = self.moon_systems[k].system.bodies[0].name.clone();
        let mut particles = Vec::new();
        for &index in indices {
            if self.moon_systems[k].is_small_moon_free(index) {
                continue;
            }
            let Some((position, velocity)) = self.small_moon_state(k, index) else {
                continue;
            };
            let id = self.next_id;
            self.next_id += 1;
            self.moon_systems[k].mark_small_moon_free(index);
            particles.push(Particle {
                position,
                velocity,
                id,
            });
            self.freed_moons.push(FreedMoon {
                time: self.top.time(),
                planet: planet.clone(),
                index,
                id,
                by: by.to_string(),
            });
        }
        if !particles.is_empty() {
            self.add_particles(particles);
        }
    }

    /// Dissolves moon system `k` as if `by` had torn it apart (see
    /// [`Unbound`]), without reporting it: for loading a save made after
    /// that happened. Returns the freed major moons' names.
    pub fn dissolve_system(&mut self, k: usize) -> Vec<String> {
        let freed = self.dissolve(k, "");
        self.freed_moons.clear();
        freed
    }

    /// Advances everything by `duration` seconds, recording the top
    /// level's paths, and returns them: for carrying particles along them
    /// (forward or back) outside the hierarchy.
    pub fn record_paths(&mut self, duration: f64) -> Field {
        self.recording = true;
        self.advance(duration);
        self.recording = false;
        std::mem::take(&mut self.field)
    }

    /// The swarm, as of its own clock (up to [`CADENCE`] behind the top
    /// level's; see [`Self::swarm_positions`]).
    pub fn swarm(&self) -> &Swarm {
        &self.swarm
    }

    /// Adds particles to the swarm, given in the top level's frame at the
    /// current time. The swarm first catches up to now.
    pub fn add_particles(&mut self, particles: impl IntoIterator<Item = Particle>) {
        self.sync_swarm();
        if self.swarm.is_empty() {
            self.swarm.set_time(self.time());
        }
        self.swarm.add(particles);
        let gms: Vec<f64> = self.top.bodies.iter().map(|b| b.gm).collect();
        let states: Vec<(DVec3, DVec3)> = self
            .top
            .bodies
            .iter()
            .map(|b| (b.position, b.velocity))
            .collect();
        self.swarm.assign_centers(self.generation, &gms, &states);
    }

    /// Removes the swarm particles with these labels.
    pub fn remove_particles(&mut self, ids: &[u32]) {
        self.swarm.remove(ids);
    }

    /// Replaces the swarm, as when loading a save: its particles at its
    /// clock, which must not be ahead of the top level's. Recorded paths
    /// start afresh, so the swarm is set to the top level's time.
    pub fn set_swarm(&mut self, swarm: Swarm) {
        self.swarm = swarm;
        self.field = Field::default();
        self.add_particles([]);
    }

    /// Brings the swarm up to the top level's time.
    pub fn sync_swarm(&mut self) {
        let now = self.time();
        let swallowed = self.swarm.advance(&mut self.field, now);
        self.swallowed.extend(swallowed);
    }

    /// The swarm particles swallowed since the last call, oldest first.
    pub fn take_swallowed(&mut self) -> Vec<Swallowed> {
        std::mem::take(&mut self.swallowed)
    }

    /// Where the swarm's particles are now, for drawing: each carried from
    /// the swarm's clock to the top level's along its two-body orbit around
    /// its center. Within the swarm's step of at most 64 days, that leaves
    /// out only the other bodies' pulls.
    pub fn swarm_positions(&self, out: &mut Vec<DVec3>) {
        out.clear();
        let particles = &self.swarm.particles;
        let dt = self.time() - self.swarm.time();
        let raw = |out: &mut Vec<DVec3>| out.extend(particles.iter().map(|p| p.position));
        let Some((generation, centers)) = self.swarm.centers() else {
            return raw(out);
        };
        // The centers' states at the swarm's time, and where each center
        // is among the bodies now (found by name if the bodies have changed
        // since).
        let bodies = &self.top.bodies;
        let (then, now): (Vec<(DVec3, DVec3)>, Vec<Option<usize>>) = if dt <= 0.0 {
            return raw(out);
        } else {
            let Some(then) = self.field.states_of(generation, self.swarm.time()) else {
                return raw(out);
            };
            let now = if generation == self.generation {
                (0..then.len()).map(Some).collect()
            } else {
                let Some(names) = self.field.names_of(generation) else {
                    return raw(out);
                };
                names
                    .iter()
                    .map(|name| bodies.iter().position(|b| &b.name == name))
                    .collect()
            };
            (then, now)
        };
        let place = |(p, &c): (&Particle, &u32)| {
            let c = c as usize;
            match (then.get(c), now.get(c).copied().flatten()) {
                (Some(&(at, moving)), Some(k)) => {
                    let center = &bodies[k];
                    let carried = drift(p.position - at, p.velocity - moving, center.gm, dt);
                    center.position + carried.position
                }
                _ => p.position,
            }
        };
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk = particles.len().div_ceil(threads).max(512);
        std::thread::scope(|scope| {
            let handles: Vec<_> = particles
                .chunks(chunk)
                .zip(centers.chunks(chunk))
                .map(|(ps, cs)| {
                    scope.spawn(move || ps.iter().zip(cs).map(place).collect::<Vec<_>>())
                })
                .collect();
            for handle in handles {
                out.extend(handle.join().expect("a drawing thread finished"));
            }
        });
    }

    /// Every top-level body's position, velocity and acceleration, for the
    /// swarm's recorded paths. Newtonian accelerations: they only shape the
    /// interpolation between the recorded positions and velocities, where
    /// relativity's 10⁻⁸ makes no difference.
    fn field_states(&self) -> Vec<[DVec3; 3]> {
        let mut accelerations = vec![DVec3::ZERO; self.top.bodies.len()];
        Newtonian.accelerations(self.top.time(), &self.top.bodies, &mut accelerations);
        self.top
            .bodies
            .iter()
            .zip(accelerations)
            .map(|(b, a)| [b.position, b.velocity, a])
            .collect()
    }

    /// The collisions since the last call, oldest first.
    pub fn take_collisions(&mut self) -> Vec<Collision> {
        std::mem::take(&mut self.happened)
    }

    /// The moon systems torn apart since the last call, oldest first.
    pub fn take_unbound(&mut self) -> Vec<Unbound> {
        std::mem::take(&mut self.unbound)
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

    /// Adds a body to the top level, given in its frame: it pulls on
    /// everything and everything pulls on it, with the top level's
    /// gravity, and the moon systems feel its tides. Returns its index.
    pub fn add_body(&mut self, body: Body) -> usize {
        self.top.bodies.push(body);
        // The integrator's memory of past steps no longer fits.
        self.integrator = Ias15::new();
        self.generation += 1;
        self.top.bodies.len() - 1
    }

    /// Removes top-level body `index`, and its moon system if it has one.
    /// The Sun (body 0) can't be removed: the followers' relativistic term
    /// and the solar wind are measured from it.
    pub fn remove_body(&mut self, index: usize) {
        assert!(index > 0, "the Sun can't be removed");
        self.top.bodies.remove(index);
        self.moon_systems.retain(|moons| moons.host != index);
        for moons in &mut self.moon_systems {
            if moons.host > index {
                moons.host -= 1;
            }
        }
        self.integrator = Ias15::new();
        self.generation += 1;
    }

    /// Removes follower `index`.
    pub fn remove_follower(&mut self, index: usize) {
        self.followers.remove(index);
    }

    /// Follower `index`, to change its state.
    pub fn follower_mut(&mut self, index: usize) -> &mut Body {
        &mut self.followers[index].system.bodies[0]
    }

    /// Restarts every integrator, after states were set from outside (as
    /// when loading a save): their memory of past steps no longer applies.
    /// Small moons being simulated in detail are dropped; set them again.
    pub fn restart(&mut self) {
        self.integrator = Ias15::new();
        for moons in &mut self.moon_systems {
            moons.integrator = Ias15::new();
            moons.clear_small_moons();
        }
        for follower in &mut self.followers {
            follower.integrator = Ias15::new();
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
        if self.collisions {
            self.absorb_overlaps();
        }
        self.free_torn_moons();
        let t0 = self.top.time();
        let swarm_start =
            (self.recording || !self.swarm.is_empty()).then(|| match self.last_recorded.take() {
                Some((g, time, states)) if g == self.generation && time == t0 => states,
                _ => self.field_states(),
            });
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
            let radii: Vec<f64> = self.top.bodies.iter().map(|b| b.radius).collect();
            let mut hits = Vec::new();
            for (f, follower) in self.followers.iter_mut().enumerate() {
                let gravity = FollowerGravity {
                    track: &track,
                    gms: &gms,
                    forces: follower.forces,
                };
                follower.system.set_time(0.0);
                if let Some(hit) =
                    follow(follower, &gravity, &track, &radii, t1 - t0, self.collisions)
                {
                    hits.push((f, hit));
                }
                follower.system.set_time(t1);
            }
            // Massless, so absorbing one changes no momentum.
            for (f, (k, speed, when)) in hits.into_iter().rev() {
                let absorbed = self.followers.remove(f).system.bodies.remove(0);
                self.happened.push(Collision {
                    time: t0 + when,
                    survivor: self.top.bodies[k].name.clone(),
                    absorbed: absorbed.name,
                    speed,
                    dissolved_system: None,
                    freed: Vec::new(),
                });
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
        if let Some(start) = swarm_start {
            let end = self.field_states();
            self.last_recorded = Some((self.generation, t1, end.clone()));
            let bodies = &self.top.bodies;
            self.field.record(
                self.generation,
                || Bodies {
                    names: bodies.iter().map(|b| b.name.clone()).collect(),
                    gms: bodies.iter().map(|b| b.gm).collect(),
                    radii: bodies.iter().map(|b| b.radius).collect(),
                },
                (t0, start),
                (t1, end),
            );
        }
        if self.collisions {
            self.collide(&before, t1 - t0);
        }
        if !self.swarm.is_empty() && self.top.time() - self.swarm.time() >= CADENCE {
            self.sync_swarm();
        }
        taken
    }

    /// Finds the top-level bodies that touched during the step just taken
    /// (from their states at its start, `before`, and now) and merges the
    /// first pair to touch; any others are found at the next step. Their
    /// integrator shortens its steps as bodies close in, so the ends of a
    /// step bracket the approach well.
    fn collide(&mut self, before: &[(DVec3, DVec3)], h: f64) {
        if h <= 0.0 {
            return;
        }
        let now = |b: &Body| (b.position, b.velocity);
        let relative = |a: (DVec3, DVec3), b: (DVec3, DVec3)| (a.0 - b.0, a.1 - b.1);
        let n = self.top.bodies.len();
        let mut first: Option<(f64, usize, usize, f64)> = None;
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (&self.top.bodies[i], &self.top.bodies[j]);
                if let Some((at, speed)) = contact(
                    relative(before[i], before[j]),
                    relative(now(a), now(b)),
                    h,
                    a.radius + b.radius,
                ) && first.is_none_or(|f| at < f.0)
                {
                    first = Some((at, i, j, speed));
                }
            }
        }
        if let Some((at, i, j, speed)) = first {
            let time = self.top.time() - (1.0 - at) * h;
            self.merge_top(i, j, speed, time);
        }
    }

    /// Merges top-level bodies `i` and `j`, conserving momentum exactly.
    /// A black hole survives any collision; otherwise the more massive body
    /// does. A planet that is absorbed leaves its major moons behind as
    /// top-level bodies (its small moons go with it); a planet that survives
    /// keeps its moons. Whatever absorbs the Sun takes its place as body 0,
    /// which the followers' relativistic term is measured from.
    fn merge_top(&mut self, i: usize, j: usize, speed: f64, time: f64) {
        let (s, a) = if first_survives(&self.top.bodies[i], &self.top.bodies[j]) {
            (i, j)
        } else {
            (j, i)
        };
        // An absorbed planet's moon system dissolves. Its bodies keep their
        // exact states, re-centered on the system's own barycenter, and
        // their GMs are scaled to the top level's figure for the system
        // (the two JPL solutions differ by under 10⁻⁴), so the momentum the
        // top level carried for the system is exactly what they carry.
        let dissolved_system = self.moon_systems.iter().position(|m| m.host == a);
        let by = self.top.bodies[s].name.clone();
        let freed = match dissolved_system {
            Some(k) => self.dissolve(k, &by),
            None => Vec::new(),
        };
        let absorbed = self.top.bodies[a].clone();
        let survivor = self.top.bodies[s].clone();
        let merged = merge(&survivor, &absorbed);
        if let Some(moons) = self.moon_systems.iter_mut().find(|m| m.host == s) {
            // The impact is on the planet itself: it takes the absorbed
            // body's mass and momentum, and the moons keep their states.
            let planet = &moons.system.bodies[0];
            let at = Body {
                position: survivor.position + planet.position,
                velocity: survivor.velocity + planet.velocity,
                ..planet.clone()
            };
            let hit = merge(&at, &absorbed);
            for body in moons.system.bodies.iter_mut().skip(1) {
                body.position += survivor.position - merged.position;
                body.velocity += survivor.velocity - merged.velocity;
            }
            moons.system.bodies[0] = Body {
                position: hit.position - merged.position,
                velocity: hit.velocity - merged.velocity,
                ..hit
            };
            moons.integrator = Ias15::new();
            moons.clear_small_moons();
            self.top.bodies[s] = Body {
                radius: moons.system.bodies[0].radius,
                ..merged
            };
        } else {
            self.top.bodies[s] = merged;
        }
        if a == 0 {
            // The Sun was absorbed: the survivor moves into its place, with
            // its moons if it has any.
            self.top.bodies.swap(0, s);
            for moons in &mut self.moon_systems {
                if moons.host == s {
                    moons.host = 0;
                }
            }
            self.remove_body(s);
        } else {
            self.remove_body(a);
        }
        self.happened.push(Collision {
            time,
            survivor: survivor.name,
            absorbed: absorbed.name,
            speed,
            dissolved_system,
            freed,
        });
    }

    /// Dissolves moon system `k`: its planet takes the host's place at the
    /// top level and its major moons join it there, each keeping its exact
    /// state, re-centered on the system's own barycenter, with GMs scaled
    /// to the top level's figure for the system (the two JPL solutions
    /// differ by under 10⁻⁴), so they carry exactly the momentum the host
    /// did. Its small moons go on as swarm particles, freed by `by`.
    /// Returns the freed major moons' names.
    fn dissolve(&mut self, k: usize, by: &str) -> Vec<String> {
        let count = {
            let moons = &self.moon_systems[k];
            moons.small_moon_count().max(moons.small_mean_orbits.len())
        };
        let all: Vec<usize> = (0..count).collect();
        self.free_small_moons(k, &all, by);
        let moons = self.moon_systems.remove(k);
        let host = self.top.bodies[moons.host].clone();
        let scale = host.gm / moons.system.total_gm();
        let center = moons.system.barycenter();
        let drift = moons.system.barycenter_velocity();
        let mut freed = Vec::new();
        for (n, body) in moons.system.bodies.iter().enumerate() {
            let free = Body {
                gm: body.gm * scale,
                position: host.position + body.position - center,
                velocity: host.velocity + body.velocity - drift,
                ..body.clone()
            };
            if n == 0 {
                self.top.bodies[moons.host] = free;
            } else {
                freed.push(free.name.clone());
                self.top.bodies.push(free);
            }
        }
        self.integrator = Ias15::new();
        self.generation += 1;
        freed
    }

    /// Merges everything that already overlaps: top-level bodies closer
    /// than the sum of their radii, and followers inside a top-level body.
    /// Bodies don't arrive overlapping by moving (contact is found along
    /// each step), but they can be placed so: a black hole dropped into
    /// the solar system swallows at once everything inside its horizon.
    /// Merges top-level bodies that touch, or that have spiraled in past
    /// where post-Newtonian gravity holds (see [`spiraled_in`]), and
    /// followers that touch a top-level body.
    fn absorb_overlaps(&mut self) {
        let time = self.top.time();
        while let Some((i, j)) = (0..self.top.bodies.len())
            .flat_map(|i| (i + 1..self.top.bodies.len()).map(move |j| (i, j)))
            .find(|&(i, j)| {
                let (a, b) = (&self.top.bodies[i], &self.top.bodies[j]);
                (a.position - b.position).length() <= a.radius + b.radius || spiraled_in(a, b)
            })
        {
            let (a, b) = (&self.top.bodies[i], &self.top.bodies[j]);
            let speed = (a.velocity - b.velocity).length();
            self.merge_top(i, j, speed, time);
        }
        for f in (0..self.followers.len()).rev() {
            let body = &self.followers[f].system.bodies[0];
            let inside = self
                .top
                .bodies
                .iter()
                .find(|b| (b.position - body.position).length() <= b.radius + body.radius);
            if let Some(host) = inside {
                let collision = Collision {
                    time,
                    survivor: host.name.clone(),
                    absorbed: body.name.clone(),
                    speed: (host.velocity - body.velocity).length(),
                    dissolved_system: None,
                    freed: Vec::new(),
                };
                self.followers.remove(f);
                self.happened.push(collision);
            }
        }
    }

    /// Frees the moons of any planet that another top-level body has come
    /// close enough to tear them away (see [`Unbound`]). For each major
    /// moon, it compares how differently the body pulls on the moon and on
    /// the planet with the planet's own pull on the moon. Far away (d much
    /// larger than the moon's orbit a), that ratio is 2 (M/m)(a/d)³, and it
    /// reaches 1/12 exactly at half the planet's Hill radius,
    /// a = ½ d (m/3M)^(1/3): the threshold used. Closer in, the exact
    /// difference counts, so a small body passing inside the moons' orbits
    /// frees them only if it truly outpulls the planet.
    fn free_torn_moons(&mut self) {
        let torn = self.moon_systems.iter().enumerate().find_map(|(k, moons)| {
            let host = &self.top.bodies[moons.host];
            let planet = &moons.system.bodies[0];
            let at_planet = host.position + planet.position;
            self.top
                .bodies
                .iter()
                .enumerate()
                .filter(|&(i, b)| i != moons.host && b.gm > 0.0)
                .find(|(_, b)| {
                    let pull = |x: DVec3| {
                        let r = b.position - x;
                        r * (b.gm / r.length().powi(3))
                    };
                    moons.system.bodies.iter().skip(1).any(|moon| {
                        let at_moon = host.position + moon.position;
                        let a = (at_moon - at_planet).length();
                        let tide = (pull(at_moon) - pull(at_planet)).length();
                        tide > planet.gm / (a * a) / 12.0
                    })
                })
                .map(|(_, b)| (k, b.name.clone()))
        });
        if let Some((k, by)) = torn {
            let planet = self.moon_systems[k].system.bodies[0].name.clone();
            let freed = self.dissolve(k, &by);
            self.unbound.push(Unbound {
                time: self.top.time(),
                planet,
                by,
                system: k,
                freed,
            });
            // Any others, at the next step.
            return;
        }
        self.free_torn_small_moons();
    }

    /// Frees any small moon another body has come close enough to tear
    /// from its planet, by the same test as for the major moons (see
    /// [`Unbound`]). The body the planet itself orbits doesn't count: the
    /// small moons' mean orbits already include its pull (the Sun's tide on
    /// Jupiter's farthest moons approaches the threshold, and they stay).
    fn free_torn_small_moons(&mut self) {
        let mut torn: Vec<(usize, Vec<usize>, String)> = Vec::new();
        for (k, moons) in self.moon_systems.iter().enumerate() {
            let count = moons.small_moon_count().max(moons.small_mean_orbits.len());
            if count == 0 {
                continue;
            }
            let host = &self.top.bodies[moons.host];
            let planet = &moons.system.bodies[0];
            let at_planet = host.position + planet.position;
            // The body the planet orbits: the more massive one pulling on
            // it hardest.
            let primary = self
                .top
                .bodies
                .iter()
                .enumerate()
                .filter(|(i, b)| *i != moons.host && b.gm > host.gm)
                .max_by(|a, b| {
                    let pull = |x: &Body| x.gm / (x.position - host.position).length_squared();
                    pull(a.1).total_cmp(&pull(b.1))
                })
                .map(|(i, _)| i);
            let perturbers: Vec<&Body> = self
                .top
                .bodies
                .iter()
                .enumerate()
                .filter(|(i, b)| *i != moons.host && Some(*i) != primary && b.gm > 0.0)
                .map(|(_, b)| b)
                .collect();
            // A body can only tear a moon if its tide could beat the
            // planet's grip at the farthest moon's distance a: the tide
            // (the difference of its pulls on moon and planet) is at most
            // twice its pull at distance d − a.
            let reach = moons
                .small_mean_orbits
                .iter()
                .map(|o| o.a * (1.0 + o.e))
                .fold(0.0, f64::max);
            let grip = planet.gm / (reach * reach) / 12.0;
            let perturbers: Vec<&Body> = perturbers
                .into_iter()
                .filter(|b| {
                    let gap = (b.position - at_planet).length() - reach;
                    gap <= 0.0 || 2.0 * b.gm / (gap * gap) > grip
                })
                .collect();
            if perturbers.is_empty() || reach == 0.0 {
                continue;
            }
            let mut indices = Vec::new();
            let mut by = String::new();
            for index in 0..count {
                if moons.is_small_moon_free(index) {
                    continue;
                }
                let Some((at_moon, _)) = self.small_moon_state(k, index) else {
                    continue;
                };
                let a = (at_moon - at_planet).length();
                let grip = planet.gm / (a * a) / 12.0;
                for b in &perturbers {
                    let pull = |x: DVec3| {
                        let r = b.position - x;
                        r * (b.gm / r.length().powi(3))
                    };
                    if (pull(at_moon) - pull(at_planet)).length() > grip {
                        indices.push(index);
                        by.clone_from(&b.name);
                        break;
                    }
                }
            }
            if !indices.is_empty() {
                torn.push((k, indices, by));
            }
        }
        for (k, indices, by) in torn {
            self.free_small_moons(k, &indices, &by);
        }
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

/// Advances a follower by `duration` on its local clock (which starts at
/// zero), step by step. With `collisions` on, after each step it checks
/// whether the follower touched a top-level body during it (from the
/// bodies' recorded paths), and if so stops there: returns the body's index,
/// the relative speed and the moment of contact on the local clock. Checking each of the follower's own steps, which
/// shorten near a body, catches a comet diving into the Sun even when the
/// top level's step is long.
fn follow(
    follower: &mut Follower,
    gravity: &FollowerGravity,
    track: &Track,
    radii: &[f64],
    duration: f64,
    collisions: bool,
) -> Option<(usize, f64, f64)> {
    while follower.system.time() < duration {
        let start_time = follower.system.time();
        let body = &follower.system.bodies[0];
        let start = (body.position, body.velocity);
        let remaining = duration - start_time;
        let taken = follower
            .integrator
            .step(&mut follower.system, gravity, remaining);
        assert!(taken > 0.0, "a follower took an empty step");
        if taken >= remaining {
            follower.system.set_time(duration);
        }
        if !collisions {
            continue;
        }
        let end_time = follower.system.time();
        let body = &follower.system.bodies[0];
        let end = (body.position, body.velocity);
        let (a, b) = (track.locate(start_time), track.locate(end_time));
        for (k, radius) in radii.iter().enumerate() {
            if let Some((at, speed)) = contact(
                (
                    start.0 - track.position(a, k),
                    start.1 - track.velocity(a, k),
                ),
                (end.0 - track.position(b, k), end.1 - track.velocity(b, k)),
                end_time - start_time,
                radius + body.radius,
            ) {
                return Some((k, speed, start_time + at * (end_time - start_time)));
            }
        }
    }
    None
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

/// Whether a bound pair has spiraled in past where post-Newtonian gravity
/// holds: its radiation reaction no longer converges (see
/// [`reaction_converges`]), a few orbits before the two would touch. Run on,
/// the truncated equations would pump the orbit eccentric or fling the pair
/// apart; instead the pair merges then, as touching bodies do. (What really
/// happens in those last orbits, and the merger, needs numerical
/// relativity: step 2.4.) Only strong fields are checked: the reaction can't
/// stop converging below GM/rc² of about 0.1.
fn spiraled_in(a: &Body, b: &Body) -> bool {
    let gm = a.gm + b.gm;
    let r = (a.position - b.position).length();
    let bound = 0.5 * (a.velocity - b.velocity).length_squared() < gm / r;
    gm > 0.0 && gm / (r * C * C) > 0.01 && bound && !reaction_converges(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DMat3;
    use crate::constants::{AU, DAY, GM_SUN};
    use crate::gravity::{PostNewtonian, relative_acceleration};
    use crate::orbit::{kepler_period, periapsis_state};

    #[test]
    fn a_tight_pair_spirals_in_and_merges_instead_of_flying_apart() {
        // Two neutron stars on a circular orbit whose wave is at about
        // 270 Hz (GM/rc² = 0.05) spiral in within a fraction of a second.
        // Near GM/rc² = 0.1 their radiation reaction stops converging and
        // they merge into one body, momentum kept, instead of being flung
        // apart by the truncated equations. (The merge keeps momentum, as
        // every merge does; see the collision tests.)
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let gm = gm1 + gm2;
        let r = gm / (0.05 * C * C);
        // The circular speed of the relativistic equations (a Newtonian
        // circle would be eccentric this close).
        let nu = gm1 * gm2 / (gm * gm);
        let x = DVec3::new(r, 0.0, 0.0);
        let mut speed = (gm / r).sqrt();
        for _ in 0..20 {
            let a = relative_acceleration(gm, nu, x, DVec3::new(0.0, speed, 0.0));
            speed = (r * -(a.newtonian + a.first + a.second).x).sqrt();
        }
        let v = DVec3::new(0.0, speed, 0.0);
        let top = System::new(vec![
            Body::new("one", gm1, 1.2e4)
                .at(x * gm2 / gm)
                .moving(v * gm2 / gm),
            Body::new("two", gm2, 1.2e4)
                .at(-x * gm1 / gm)
                .moving(-v * gm1 / gm),
        ]);
        let mut h = Hierarchy::new(top, Box::new(PostNewtonian::default()), Vec::new());
        let mut farthest: f64 = 0.0;
        while h.top.bodies.len() == 2 && h.time() < 1.0 {
            h.step(1e-4);
            if let [one, two] = &h.top.bodies[..] {
                farthest = farthest.max((one.position - two.position).length());
            }
        }
        let collisions = h.take_collisions();
        println!(
            "merged after {:.3} s; the pair never got farther apart than {:.0} km (started {:.0} km)",
            h.time(),
            farthest / 1e3,
            r / 1e3
        );
        assert_eq!(h.top.bodies.len(), 1);
        assert_eq!(collisions.len(), 1);
        assert!(farthest < 1.2 * r);
        assert_eq!(h.top.bodies[0].gm, gm);
    }

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
