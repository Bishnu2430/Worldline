//! Giant impacts (step 6.4): planets of rock and iron as fluids, and the
//! impact that left Earth with a disk of debris. See
//! `docs/physics/giant-impacts.md`.
//!
//! The planets are in hydrostatic equilibrium on an isentrope, as SPH
//! particles of the [`crate::tillotson`] equation of state. The impact and
//! the way a disk is measured are Canup's (2004, *Icarus* 168, 433).

use std::f64::consts::PI;

use glam::DVec3;

use crate::constants::G;
use crate::sph::{ETA, Fluid, Particle, lattice};
use crate::tillotson::Tillotson;

/// Earth's mass, kg: the IAU's nominal GM⊕ over G.
pub const EARTH_MASS: f64 = 3.986_004e14 / G;

/// The Moon's mass, kg, as Canup (2004) takes it.
pub const LUNAR_MASS: f64 = 7.35e22;

/// The Earth–Moon system's angular momentum, kg m²/s (Canup 2004).
pub const EARTH_MOON_ANGULAR_MOMENTUM: f64 = 3.5e34;

/// Earth's mean radius, m.
const EARTH_RADIUS: f64 = 6.371e6;

/// Heat capacities at constant volume, J/(kg K): Reinhardt & Stadel's (2017)
/// granite, standing in for olivine, for which they give none, and iron.
/// They only set the starting temperature's energy.
const ROCK_HEAT: f64 = 790.0;
const IRON_HEAT: f64 = 449.0;

/// Radial steps through a planet.
const SHELLS: usize = 4000;

/// Artificial viscosity's largest α: 1.5, as in Canup (2004).
const ALPHA_MAX: f64 = 1.5;

/// The largest smoothing length, m: Earth's radius. Worldline's choice, as
/// the debris is then too thin for its pressure to matter.
const MAX_H: f64 = EARTH_RADIUS;

/// Settling: dynamical times to relax for, and the damping's time constant
/// in them, as for the polytropes in `sph.rs`.
const SETTLE: f64 = 5.0;
const DAMPING_TIME: f64 = 0.3;

/// How far apart the bodies start, in the sum of their radii.
const START: f64 = 1.5;

/// What a particle is made of and where it came from, as an index into
/// [`materials`].
pub const TARGET_ROCK: u8 = 0;
/// See [`TARGET_ROCK`].
pub const TARGET_IRON: u8 = 1;
/// See [`TARGET_ROCK`].
pub const IMPACTOR_ROCK: u8 = 2;
/// See [`TARGET_ROCK`].
pub const IMPACTOR_IRON: u8 = 3;

/// The fluid's materials: rock and iron for each body, so that a particle
/// remembers which it came from.
pub fn materials() -> Vec<Tillotson> {
    vec![
        Tillotson::OLIVINE,
        Tillotson::IRON,
        Tillotson::OLIVINE,
        Tillotson::IRON,
    ]
}

/// Whether a particle of `material` is iron.
pub fn is_iron(material: u8) -> bool {
    material == TARGET_IRON || material == IMPACTOR_IRON
}

/// Whether a particle of `material` came from the impactor.
pub fn from_impactor(material: u8) -> bool {
    material >= IMPACTOR_ROCK
}

/// A point in a planet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shell {
    /// Its radius, m.
    pub radius: f64,
    /// The mass inside it, kg.
    pub inside: f64,
    /// The density there, kg/m³.
    pub density: f64,
    /// The internal energy there, J/kg.
    pub energy: f64,
    /// The pressure there, Pa.
    pub pressure: f64,
    /// Whether it's in the iron core.
    pub iron: bool,
}

/// A planet in hydrostatic equilibrium, from the surface in.
#[derive(Debug, Clone, PartialEq)]
pub struct Structure {
    /// Its radius, m.
    pub radius: f64,
    /// Its profile, outermost first.
    pub shells: Vec<Shell>,
}

/// The state going inward: mass inside, density and internal energy, and
/// their slopes with radius. On an isentrope dP = c² dρ and du = P dρ/ρ².
fn slopes(r: f64, (m, rho, u): (f64, f64, f64), material: &Tillotson) -> (f64, f64, f64) {
    let (pressure, sound) = material.state(rho, u);
    let drho = -rho * G * m / (r * r * sound * sound);
    (4.0 * PI * r * r * rho, drho, pressure / (rho * rho) * drho)
}

/// Integrates a planet inward from `radius`, for `iron` of its `mass` in the
/// core. Returns the shells and whether mass was left at the center.
fn inward(mass: f64, iron: f64, surface: f64, radius: f64) -> (Vec<Shell>, bool) {
    let rock = Tillotson::OLIVINE;
    // The density where the pressure falls to zero.
    let (mut low, mut high) = (0.5 * rock.density, 1.5 * rock.density);
    for _ in 0..80 {
        let middle = 0.5 * (low + high);
        if rock.state(middle, surface).0 > 0.0 {
            high = middle;
        } else {
            low = middle;
        }
    }
    let h = radius / SHELLS as f64;
    let (mut m, mut rho, mut u) = (mass, high, surface);
    let mut core = false;
    let mut shells = Vec::with_capacity(SHELLS);
    for k in 0..SHELLS {
        let r = radius - k as f64 * h;
        let material = if core { Tillotson::IRON } else { rock };
        shells.push(Shell {
            radius: r,
            inside: m,
            density: rho,
            energy: u,
            pressure: material.state(rho, u).0,
            iron: core,
        });
        if k + 1 == SHELLS {
            break;
        }
        // Fourth-order Runge–Kutta, going inward.
        let shift = |y: (f64, f64, f64), k: (f64, f64, f64), by: f64| {
            (y.0 - by * k.0, y.1 - by * k.1, y.2 - by * k.2)
        };
        let y = (m, rho, u);
        let k1 = slopes(r, y, &material);
        let k2 = slopes(r - 0.5 * h, shift(y, k1, 0.5 * h), &material);
        let k3 = slopes(r - 0.5 * h, shift(y, k2, 0.5 * h), &material);
        let k4 = slopes(r - h, shift(y, k3, h), &material);
        let step = |a: f64, b: f64, c: f64, d: f64| h / 6.0 * (a + 2.0 * b + 2.0 * c + d);
        m -= step(k1.0, k2.0, k3.0, k4.0);
        rho -= step(k1.1, k2.1, k3.1, k4.1);
        u -= step(k1.2, k2.2, k3.2, k4.2);
        if m <= 0.0 {
            return (shells, false);
        }
        if !core && m <= iron * mass {
            // Into the core: the same pressure and temperature.
            let wanted = rock.state(rho, u).0;
            u *= IRON_HEAT / ROCK_HEAT;
            let (mut low, mut high) = (0.5 * Tillotson::IRON.density, 4.0 * rock.density.max(rho));
            for _ in 0..80 {
                let middle = 0.5 * (low + high);
                if Tillotson::IRON.state(middle, u).0 > wanted {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            rho = high;
            core = true;
        }
    }
    (shells, true)
}

/// A planet of `mass` kg with the share `iron` of it in its core, on an
/// isentrope from a surface at `surface_temperature` K: its radius found by
/// halving, until the mass runs out at the center.
pub fn structure(mass: f64, iron: f64, surface_temperature: f64) -> Structure {
    let surface = ROCK_HEAT * surface_temperature;
    let (mut low, mut high) = (0.2 * EARTH_RADIUS, 3.0 * EARTH_RADIUS);
    for _ in 0..60 {
        let middle = 0.5 * (low + high);
        if inward(mass, iron, surface, middle).1 {
            low = middle;
        } else {
            high = middle;
        }
    }
    Structure {
        radius: high,
        shells: inward(mass, iron, surface, high).0,
    }
}

/// `structure` as about `count` particles of equal mass, at rest: a cubic
/// lattice in a sphere, each point moved along its radius until the mass
/// inside it is the planet's.
fn particles(structure: &Structure, count: usize, rock: u8, iron: u8, alpha: f64) -> Vec<Particle> {
    let points = lattice(count);
    let shells = &structure.shells;
    let mass = shells[0].inside;
    let m = mass / points.len() as f64;
    points
        .iter()
        .map(|x| {
            let inside = mass * x.length_squared() * x.length();
            let k = shells
                .partition_point(|s| s.inside > inside)
                .min(shells.len() - 1);
            let shell = &shells[k];
            Particle {
                position: x.normalize_or_zero() * shell.radius,
                velocity: DVec3::ZERO,
                mass: m,
                energy: shell.energy,
                h: ETA * (m / shell.density).cbrt(),
                density: shell.density,
                viscosity: alpha,
                material: if shell.iron { iron } else { rock },
            }
        })
        .collect()
}

/// A planet as a settled fluid of about `count` particles: five dynamical
/// times with its motion damped and its energies held, then at rest.
pub fn settled(structure: &Structure, count: usize, rock: u8, iron: u8) -> Fluid {
    let mass = structure.shells[0].inside;
    let time = (structure.radius.powi(3) / (G * mass)).sqrt();
    let body = particles(structure, count, rock, iron, 0.1);
    let mut fluid = Fluid::of_materials(body, materials(), ALPHA_MAX, MAX_H);
    fluid.relax(SETTLE * time, DAMPING_TIME * time);
    fluid
}

/// Two bodies about to collide on a parabola, at rest at infinity, so that
/// they meet at their mutual escape speed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    /// M_T: the two bodies' total mass, kg.
    pub total_mass: f64,
    /// γ: the impactor's share of it.
    pub gamma: f64,
    /// b′: the impact's angular momentum over a grazing one's.
    pub impact_parameter: f64,
    /// The share of each body that is iron.
    pub iron: f64,
    /// Each body's surface temperature, K: Canup's "warm start".
    pub surface_temperature: f64,
}

impl Impact {
    /// The impact of Canup's (2004) §4.1 and Table 1: γ = 0.13 and M_T =
    /// 1.019 M⊕ at b′ = 0.73, 30% iron, with surfaces at 2000 K.
    pub const CANONICAL: Self = Self {
        total_mass: 1.019 * EARTH_MASS,
        gamma: 0.13,
        impact_parameter: 0.73,
        iron: 0.3,
        surface_temperature: 2000.0,
    };

    /// The target's and impactor's structures.
    pub fn bodies(&self) -> (Structure, Structure) {
        let body =
            |share: f64| structure(share * self.total_mass, self.iron, self.surface_temperature);
        (body(1.0 - self.gamma), body(self.gamma))
    }

    /// The two bodies as about `count` particles of about equal mass,
    /// settled and set on their way, the impactor along +x from the target
    /// and the center of mass at rest at the origin. The angular momentum is
    /// along +z.
    pub fn set_up(&self, count: usize) -> Start {
        let (target, impactor) = self.bodies();
        let share = |gamma: f64| (gamma * count as f64).round() as usize;
        let first = settled(&target, share(1.0 - self.gamma), TARGET_ROCK, TARGET_IRON);
        let second = settled(&impactor, share(self.gamma), IMPACTOR_ROCK, IMPACTOR_IRON);
        let reach = target.radius + impactor.radius;
        let escape = (2.0 * G * self.total_mass / reach).sqrt();
        let reduced = self.gamma * (1.0 - self.gamma) * self.total_mass;
        let distance = START * reach;
        // A parabola: energy gives the speed, angular momentum its share
        // across the line between the bodies.
        let across = self.impact_parameter * reach * escape / distance;
        let speed = (2.0 * G * self.total_mass / distance).sqrt();
        let along = (speed * speed - across * across).sqrt();
        let relative = DVec3::new(-along, across, 0.0);
        let at = |fluid: Fluid, position: DVec3, velocity: DVec3| {
            let total: f64 = fluid.particles.iter().map(|p| p.mass).sum();
            let center = fluid
                .particles
                .iter()
                .map(|p| p.position * p.mass)
                .sum::<DVec3>()
                / total;
            fluid.particles.into_iter().map(move |p| Particle {
                position: p.position - center + position,
                velocity,
                ..p
            })
        };
        let x = DVec3::X * distance;
        let mut all: Vec<Particle> = at(first, -x * self.gamma, -relative * self.gamma).collect();
        all.extend(at(
            second,
            x * (1.0 - self.gamma),
            relative * (1.0 - self.gamma),
        ));
        Start {
            fluid: Fluid::of_materials(all, materials(), ALPHA_MAX, MAX_H),
            target_radius: target.radius,
            impactor_radius: impactor.radius,
            escape_speed: escape,
            angular_momentum: self.impact_parameter * reduced * reach * escape,
        }
    }
}

/// Two bodies set on their way.
pub struct Start {
    /// The two as one fluid.
    pub fluid: Fluid,
    /// The target's radius, m.
    pub target_radius: f64,
    /// The impactor's.
    pub impactor_radius: f64,
    /// Their mutual escape speed, the speed they meet at, m/s.
    pub escape_speed: f64,
    /// The impact's angular momentum, kg m²/s.
    pub angular_momentum: f64,
}

/// Earth's mean density, kg/m³, and its moment of inertia constant, as
/// Canup (2004) takes them for the central planet.
const PLANET_DENSITY: f64 = 5500.0;
const INERTIA: f64 = 0.335;

/// The Moon's density, kg/m³, for the Roche limit (Canup 2004).
const MOON_DENSITY: f64 = 3340.0;

/// The share of a disk's mass that escapes as it forms a moon (Canup 2004).
const ESCAPING_SHARE: f64 = 0.05;

/// What an impact left, as Canup (2004, §3.3) measures it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Disk {
    /// The central planet's mass, kg.
    pub planet: f64,
    /// Mass in orbit, kg: bound, with an equivalent circular orbit outside
    /// the planet.
    pub disk: f64,
    /// How many particles that is.
    pub particles: usize,
    /// The part of it outside the Roche limit, kg.
    pub beyond_roche: f64,
    /// Mass on escaping orbits, kg.
    pub escaping: f64,
    /// Iron in orbit, kg.
    pub iron: f64,
    /// Mass in orbit from the impactor, kg.
    pub from_impactor: f64,
    /// Mass in orbit with its periapsis outside the planet, kg: the check on
    /// `disk` the paper uses.
    pub above_periapsis: f64,
    /// The disk's angular momentum, kg m²/s.
    pub disk_angular_momentum: f64,
    /// The planet's and disk's, kg m²/s.
    pub bound_angular_momentum: f64,
    /// The planet's rotation period, s.
    pub day: f64,
    /// The mass of the moon the disk would make, kg: eq. 1.
    pub moon: f64,
}

/// Measures the disk around whatever is left in `particles`: guess the
/// planet's mass, and from it and its spin its equatorial radius; each
/// particle is then escaping, orbiting or in the planet; which gives a
/// better mass and spin. Repeated until nothing changes.
pub fn measure(particles: &[Particle]) -> Disk {
    #[derive(Clone, Copy, PartialEq)]
    enum Fate {
        Planet,
        Disk,
        Escaping,
    }
    // The planet to begin with: the densest half of the density range.
    let densest = particles.iter().map(|p| p.density).fold(0.0, f64::max);
    let mut fates: Vec<Fate> = particles
        .iter()
        .map(|p| {
            if p.density > 0.5 * densest {
                Fate::Planet
            } else {
                Fate::Disk
            }
        })
        .collect();
    let mut planet_mass = 0.0;
    let mut normal = DVec3::Z;
    let mut result = Disk::default();
    for _ in 0..200 {
        let members = || {
            particles
                .iter()
                .zip(&fates)
                .filter(|(_, f)| **f == Fate::Planet)
                .map(|(p, _)| p)
        };
        let mass: f64 = members().map(|p| p.mass).sum();
        let center = members().map(|p| p.position * p.mass).sum::<DVec3>() / mass;
        let drift = members().map(|p| p.velocity * p.mass).sum::<DVec3>() / mass;
        let spin = members()
            .map(|p| (p.position - center).cross(p.velocity - drift) * p.mass)
            .sum::<DVec3>();
        let radius = (3.0 * mass / (4.0 * PI * PLANET_DENSITY)).cbrt();
        let omega = spin.dot(normal).abs() / (INERTIA * mass * radius * radius);
        // Darwin–Radau, for a hydrostatic body of this moment of inertia.
        let q = omega * omega * radius.powi(3) / (G * mass);
        let flattening = 2.5 * q / (1.0 + (2.5 * (1.0 - 1.5 * INERTIA)).powi(2));
        let equator = radius * (1.0 + flattening / 3.0);
        let roche = 2.456 * radius * (PLANET_DENSITY / MOON_DENSITY).cbrt();
        let gm = G * mass;
        let mut next = Disk {
            planet: mass,
            day: std::f64::consts::TAU / omega.max(1e-30),
            ..Disk::default()
        };
        let mut around = spin;
        let fates_next: Vec<Fate> = particles
            .iter()
            .map(|p| {
                let (r, v) = (p.position - center, p.velocity - drift);
                let distance = r.length();
                let along = r.cross(v);
                let lz = along.dot(normal);
                let energy = 0.5 * v.length_squared() - gm / distance;
                if distance > equator && energy >= 0.0 {
                    next.escaping += p.mass;
                    return Fate::Escaping;
                }
                let circular = lz * lz / gm;
                if circular <= equator {
                    return Fate::Planet;
                }
                next.disk += p.mass;
                next.particles += 1;
                next.disk_angular_momentum += lz.abs() * p.mass;
                around += along * p.mass;
                if circular > roche {
                    next.beyond_roche += p.mass;
                }
                if is_iron(p.material) {
                    next.iron += p.mass;
                }
                if from_impactor(p.material) {
                    next.from_impactor += p.mass;
                }
                // Its Kepler orbit's periapsis.
                let semi_latus = along.length_squared() / gm;
                let eccentricity = (1.0 + 2.0 * energy * semi_latus / gm).max(0.0).sqrt();
                if semi_latus / (1.0 + eccentricity) > equator {
                    next.above_periapsis += p.mass;
                }
                Fate::Disk
            })
            .collect();
        next.bound_angular_momentum = around.length();
        next.moon = 1.9 * next.disk_angular_momentum / (gm * roche).sqrt()
            - 1.1 * next.disk
            - 1.9 * ESCAPING_SHARE * next.disk;
        normal = around.normalize_or(DVec3::Z);
        result = next;
        let settled = fates_next == fates && mass == planet_mass;
        fates = fates_next;
        planet_mass = mass;
        if settled {
            break;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_planets_mass_runs_out_at_its_center() {
        let earth = structure(EARTH_MASS, 0.3, 2000.0);
        let last = earth.shells.last().unwrap();
        assert!(last.inside < 1e-6 * EARTH_MASS, "{}", last.inside);
        // Pressure and density rise inward, and the iron starts where 30%
        // of the mass is left.
        assert!(
            earth
                .shells
                .windows(2)
                .all(|s| s[1].pressure >= s[0].pressure)
        );
        let core = earth.shells.iter().find(|s| s.iron).unwrap();
        assert!((core.inside / (0.3 * EARTH_MASS) - 1.0).abs() < 2e-3);
    }

    #[test]
    fn nothing_in_orbit_is_measured_around_a_lone_planet() {
        let earth = structure(0.1 * EARTH_MASS, 0.3, 2000.0);
        let body = particles(&earth, 500, TARGET_ROCK, TARGET_IRON, 0.1);
        let disk = measure(&body);
        assert_eq!(disk.particles, 0);
        assert_eq!(disk.escaping, 0.0);
        assert!((disk.planet / (0.1 * EARTH_MASS) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn a_particle_on_an_orbit_is_measured_as_one() {
        // A planet at rest, one particle on a circle at four times its
        // radius, and one flung away at twice the escape speed.
        let earth = structure(0.1 * EARTH_MASS, 0.3, 2000.0);
        let mut body = particles(&earth, 500, TARGET_ROCK, TARGET_IRON, 0.1);
        let (mass, m) = (body.iter().map(|p| p.mass).sum::<f64>(), body[0].mass);
        let r = 4.0 * (3.0 * mass / (4.0 * PI * PLANET_DENSITY)).cbrt();
        let speed = (G * mass / r).sqrt();
        let orbiting = Particle {
            position: DVec3::X * r,
            velocity: DVec3::Y * speed,
            density: 10.0,
            ..body[0]
        };
        let escaping = Particle {
            position: DVec3::Y * r,
            velocity: DVec3::Y * 2.0 * 2f64.sqrt() * speed,
            density: 10.0,
            ..body[0]
        };
        body.push(orbiting);
        body.push(escaping);
        let disk = measure(&body);
        assert_eq!(disk.particles, 1);
        assert_eq!(disk.disk, m);
        assert_eq!(disk.escaping, m);
        assert_eq!(disk.above_periapsis, m);
    }
}
