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
//! The top level sees each system as a point mass at its barycenter, which
//! is exact apart from tiny tidal coupling terms (relative size ~ (r/R)²
//! times the moons' share of the mass). See `docs/physics/moons.md`.

use glam::DVec3;

use crate::gravity::{Gravity, Newtonian, SynchronousFigure, TesseralField, ZonalField};
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
            integrator: Ias15::new(),
        }
    }

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
        }
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
        let taken = self
            .integrator
            .step(&mut self.top, self.gravity.as_ref(), max_dt);
        let t1 = self.top.time();

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
            let gravity = MoonSystemGravity {
                zonal: moons.zonal.as_ref(),
                tesseral: moons.tesseral.as_ref(),
                figures: &moons.figures,
                perturbers,
                t0,
                t1,
            };
            let interval = t1 - moons.system.time();
            advance(&mut moons.system, &gravity, &mut moons.integrator, interval);
            moons.system.set_time(t1);
        }
        taken
    }

    /// Advances everything by exactly `duration` seconds.
    pub fn advance(&mut self, duration: f64) {
        let end = self.time() + duration;
        while self.time() < end {
            let remaining = end - self.time();
            self.step(remaining);
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
    perturbers: Vec<Perturber>,
    t0: f64,
    t1: f64,
}

impl MoonSystemGravity<'_> {
    /// A perturber's position at `time`, by cubic Hermite interpolation
    /// from its positions and velocities at both ends of the step.
    fn position(&self, p: &Perturber, time: f64) -> DVec3 {
        let h = self.t1 - self.t0;
        if h <= 0.0 {
            return p.end.0;
        }
        let s = ((time - self.t0) / h).clamp(0.0, 1.0);
        let (s2, s3) = (s * s, s * s * s);
        p.start.0 * (2.0 * s3 - 3.0 * s2 + 1.0)
            + p.start.1 * (h * (s3 - 2.0 * s2 + s))
            + p.end.0 * (-2.0 * s3 + 3.0 * s2)
            + p.end.1 * (h * (s3 - s2))
    }
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

        // Tides: each outside body pulls on every member, minus its pull on
        // the barycenter (the origin), which the top level already handles.
        for p in &self.perturbers {
            let r = self.position(p, time);
            let on_barycenter = r * (p.gm / r.length().powi(3));
            for (body, a) in bodies.iter().zip(out.iter_mut()) {
                let d = r - body.position;
                *a += d * (p.gm / d.length().powi(3)) - on_barycenter;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
