//! Smoothed particle hydrodynamics (SPH): a self-gravitating fluid as
//! particles, for planets and stars that deform (milestone 4). Each
//! particle carries a mass, an internal energy and a smoothing length; the
//! fluid's density, pressure and forces are sums over its neighbours,
//! smoothed by a kernel. See `docs/physics/sph.md`.
//!
//! The formulation is Price's (2012, *J. Comput. Phys.* 231, 759),
//! derived from a Lagrangian, so momentum, angular momentum and energy are
//! kept up to the time steps' error:
//! - the M4 cubic spline kernel (his eq. 6): W = w(|r|/h)/h³ with
//!   w(q) = (1/π)[¼(2 − q)³ − (1 − q)³] for q < 1, (1/π)¼(2 − q)³ for
//!   1 ≤ q < 2, and 0 beyond;
//! - each smoothing length set by the density there, h = η (m/ρ)^(1/3)
//!   with η = 1.2 (eq. 10; about 58 neighbours), solved together with
//!   the density sum by Newton's method;
//! - pressure forces with the "grad-h" terms Ω (eqs. 27 and 30) and the
//!   matching internal energy equation (eq. 35);
//! - Monaghan's (1992) artificial viscosity (eq. 98), β = 2α, ε = 0.01,
//!   to capture shocks, with its heating; α is each particle's own, held
//!   near 0.1 in smooth flow and raised toward 1 where the gas is
//!   compressed, by Morris & Monaghan's (1997) switch (eq. 105);
//! - gravity softened by the same kernel (Price & Monaghan 2007, *MNRAS*
//!   374, 1347): each particle's mass spread out as the kernel, so b pulls
//!   on a with G m_b M(r/h)/r², M(q) the kernel's mass within q, averaged
//!   over the two smoothing lengths.
//!
//! The gas is ideal, P = (γ − 1) ρ u. Steps are leapfrog
//! (kick–drift–kick), all particles together.

use std::collections::HashMap;
use std::f64::consts::PI;

use glam::DVec3;

use crate::constants::G;

/// Smoothing length per mean particle spacing (Price 2012 recommends 1.2
/// for the B-spline kernels).
pub const ETA: f64 = 1.2;

/// Artificial viscosity's parameters (Monaghan 1992, as Price 2012 eq. 98
/// gives them, α = 1 and β = 2 at most), and Morris & Monaghan's (1997)
/// switch: α decays to its least, 0.1, over h/(σc) with σ = 0.1 (Price
/// 2012, eq. 105), and grows where the gas is compressed.
const ALPHA_MAX: f64 = 1.0;
const ALPHA_MIN: f64 = 0.1;
const SIGMA_DECAY: f64 = 0.1;
const EPSILON: f64 = 0.01;

/// The fractions of the sound-crossing and force times a step may take
/// (Courant and force conditions, as in Price et al. 2018, *PASA* 35,
/// e031: 0.3 and 0.25).
const COURANT: f64 = 0.3;
const FORCE: f64 = 0.25;

/// The kernel's shape, w(q), with its 3D normalization 1/π.
pub fn kernel(q: f64) -> f64 {
    if q < 1.0 {
        (0.25 * (2.0 - q).powi(3) - (1.0 - q).powi(3)) / PI
    } else if q < 2.0 {
        0.25 * (2.0 - q).powi(3) / PI
    } else {
        0.0
    }
}

/// dw/dq.
pub fn kernel_slope(q: f64) -> f64 {
    if q < 1.0 {
        (-0.75 * (2.0 - q).powi(2) + 3.0 * (1.0 - q).powi(2)) / PI
    } else if q < 2.0 {
        -0.75 * (2.0 - q).powi(2) / PI
    } else {
        0.0
    }
}

/// The fraction of a particle's mass, spread out as the kernel, within
/// q = r/h of it: M(q) = ∫ 4πq′² w(q′) dq′.
pub fn kernel_mass(q: f64) -> f64 {
    if q < 1.0 {
        q.powi(3) * (4.0 / 3.0 - 1.2 * q * q + 0.5 * q.powi(3))
    } else if q < 2.0 {
        8.0 / 3.0 * q.powi(3) - 3.0 * q.powi(4) + 1.2 * q.powi(5) - q.powi(6) / 6.0 - 1.0 / 15.0
    } else {
        1.0
    }
}

/// The softened potential of a particle of unit G m at distance q = r/h,
/// in units of 1/h, negated: φ = −G m ψ(q)/h, with ψ = M(q)/q + ∫_q^2
/// 4πq′w(q′) dq′, so that −dφ/dr = −G m M(q)/r². At the center ψ = 7/5
/// (Price & Monaghan 2007), beyond 2h it is 1/q, the point mass's.
pub fn kernel_potential(q: f64) -> f64 {
    let outside = if q < 1.0 {
        1.4 - 2.0 * q * q + 1.5 * q.powi(4) - 0.6 * q.powi(5)
    } else if q < 2.0 {
        1.6 - 4.0 * q * q + 4.0 * q.powi(3) - 1.5 * q.powi(4) + 0.2 * q.powi(5)
    } else {
        return 1.0 / q;
    };
    let inside = if q > 0.0 { kernel_mass(q) / q } else { 0.0 };
    inside + outside
}

/// A fluid particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    /// Position, m.
    pub position: DVec3,
    /// Velocity, m/s.
    pub velocity: DVec3,
    /// Mass, kg.
    pub mass: f64,
    /// Internal energy per unit mass, J/kg.
    pub energy: f64,
    /// Smoothing length, m.
    pub h: f64,
    /// Density there, kg/m³, from the sum over its neighbours.
    pub density: f64,
    /// Its artificial viscosity parameter α.
    pub viscosity: f64,
}

/// What the forces on one particle are.
#[derive(Debug, Clone, Copy, Default)]
struct Force {
    acceleration: DVec3,
    heating: f64,
    /// The largest |μ| of the viscosity with its neighbours (for the time
    /// step).
    mu: f64,
    /// How fast its viscosity parameter changes: Morris & Monaghan's
    /// dα/dt = −(α − α_min)/τ + max(0, −∇·v)(α_max − α), τ = h/(σc), the
    /// last factor (Rosswog et al. 2000) keeping α under α_max.
    switch: f64,
}

/// A self-gravitating fluid of SPH particles.
pub struct Fluid {
    /// Its particles.
    pub particles: Vec<Particle>,
    /// The adiabatic index, P = (γ − 1) ρ u.
    pub gamma: f64,
    /// Time, s.
    pub time: f64,
    /// Ω, the grad-h term, at each particle.
    omega: Vec<f64>,
    /// The forces at each particle, as last worked out.
    forces: Vec<Force>,
}

/// Runs `f` on 0..n, spread over the computer's cores.
fn parallel<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if n < 256 || threads == 1 {
        return (0..n).map(f).collect();
    }
    let chunk = n.div_ceil(threads);
    let f = &f;
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..n)
            .step_by(chunk)
            .map(|start| {
                scope.spawn(move || (start..(start + chunk).min(n)).map(f).collect::<Vec<_>>())
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("an SPH thread finished"))
            .collect()
    })
}

/// The particles by cell of a grid, to find neighbours.
struct Grid {
    cell: f64,
    cells: HashMap<(i64, i64, i64), Vec<usize>>,
}

impl Grid {
    fn new(particles: &[Particle], cell: f64) -> Self {
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in particles.iter().enumerate() {
            cells
                .entry(Self::key(p.position, cell))
                .or_default()
                .push(i);
        }
        Self { cell, cells }
    }

    fn key(x: DVec3, cell: f64) -> (i64, i64, i64) {
        let k = (x / cell).floor();
        (k.x as i64, k.y as i64, k.z as i64)
    }

    /// Every particle in the 27 cells around `x`: all within one cell's
    /// width of it, and more.
    fn near(&self, x: DVec3) -> impl Iterator<Item = usize> + '_ {
        let (i, j, k) = Self::key(x, self.cell);
        (-1..=1)
            .flat_map(move |a| {
                (-1..=1).flat_map(move |b| (-1..=1).map(move |c| (i + a, j + b, k + c)))
            })
            .filter_map(|key| self.cells.get(&key))
            .flatten()
            .copied()
    }
}

impl Fluid {
    /// A fluid of `particles` with adiabatic index `gamma`, its densities
    /// and forces worked out.
    pub fn new(particles: Vec<Particle>, gamma: f64) -> Self {
        let n = particles.len();
        let mut fluid = Self {
            particles,
            gamma,
            time: 0.0,
            omega: vec![1.0; n],
            forces: vec![Force::default(); n],
        };
        fluid.update_density();
        let velocities: Vec<DVec3> = fluid.particles.iter().map(|p| p.velocity).collect();
        fluid.update_forces(&velocities);
        fluid
    }

    /// The pressure of particle `a`.
    pub fn pressure(&self, a: usize) -> f64 {
        let p = &self.particles[a];
        (self.gamma - 1.0) * p.density * p.energy
    }

    /// The sound speed at particle `a`, √(γP/ρ).
    pub fn sound_speed(&self, a: usize) -> f64 {
        (self.gamma * (self.gamma - 1.0) * self.particles[a].energy)
            .max(0.0)
            .sqrt()
    }

    /// Sets each particle's smoothing length and density together, h =
    /// η (m/ρ)^(1/3) with ρ the kernel sum at that h, by Newton's method
    /// on f(h) = Σ m W(h) − m (η/h)³ to 10⁻⁶ of h; and its Ω.
    pub fn update_density(&mut self) {
        loop {
            let reach = self.particles.iter().map(|p| p.h).fold(0.0, f64::max);
            let grid = Grid::new(&self.particles, 2.0 * reach);
            let particles = &self.particles;
            let solved: Vec<(f64, f64, f64, bool)> = parallel(particles.len(), |a| {
                let p = &particles[a];
                let near: Vec<usize> = grid.near(p.position).collect();
                let sums = |h: f64| {
                    let (mut rho, mut dh) = (0.0, 0.0);
                    for &b in &near {
                        let q = (p.position - particles[b].position).length() / h;
                        if q < 2.0 {
                            let m = particles[b].mass;
                            rho += m * kernel(q) / h.powi(3);
                            dh -= m * (3.0 * kernel(q) + q * kernel_slope(q)) / h.powi(4);
                        }
                    }
                    (rho, dh)
                };
                let mut h = p.h.min(reach);
                for _ in 0..50 {
                    let (rho, dh) = sums(h);
                    let wanted = p.mass * (ETA / h).powi(3);
                    let slope = dh + 3.0 * wanted / h;
                    let next = (h - (rho - wanted) / slope)
                        .clamp(0.5 * h, 2.0 * h)
                        .min(reach);
                    let done = (next - h).abs() < 1e-6 * h;
                    h = next;
                    if done {
                        break;
                    }
                }
                let (rho, dh) = sums(h);
                // Held at the grid's reach with too little density for it:
                // it wants to be larger.
                let held = h >= reach * (1.0 - 1e-9) && rho < p.mass * (ETA / h).powi(3);
                (h, rho, 1.0 + h / (3.0 * rho) * dh, held)
            });
            // Smoothing lengths that want to grow past the grid's reach are
            // tried again, larger, on a coarser grid.
            let grew = solved.iter().any(|s| s.3);
            for (p, (h, rho, _, held)) in self.particles.iter_mut().zip(&solved) {
                p.h = if *held { 1.5 * h } else { *h };
                p.density = *rho;
            }
            if !grew {
                self.omega = solved.iter().map(|s| s.2).collect();
                return;
            }
        }
    }

    /// Works out every particle's acceleration and heating, for velocities
    /// `velocities`: pressure (Price 2012 eq. 30), viscosity (eq. 98) and
    /// softened gravity; heating by eq. 35 and the viscosity's.
    fn update_forces(&mut self, velocities: &[DVec3]) {
        let reach = self.particles.iter().map(|p| p.h).fold(0.0, f64::max);
        let grid = Grid::new(&self.particles, 2.0 * reach);
        let n = self.particles.len();
        let pressure: Vec<f64> = (0..n).map(|a| self.pressure(a)).collect();
        let sound: Vec<f64> = (0..n).map(|a| self.sound_speed(a)).collect();
        let (particles, omega) = (&self.particles, &self.omega);
        self.forces = parallel(n, |a| {
            let p = &particles[a];
            let own = pressure[a] / (omega[a] * p.density * p.density);
            let mut force = Force::default();
            // −∇·v times ρΩ: dρ/dt (Price 2012, from eq. 35).
            let mut compression = 0.0;
            for b in grid.near(p.position) {
                if b == a {
                    continue;
                }
                let o = &particles[b];
                let d = p.position - o.position;
                let r = d.length();
                if r >= 2.0 * p.h.max(o.h) {
                    continue;
                }
                let unit = d / r;
                let grad = |h: f64| unit * (kernel_slope(r / h) / h.powi(4));
                let (ga, gb) = (grad(p.h), grad(o.h));
                let theirs = pressure[b] / (omega[b] * o.density * o.density);
                force.acceleration -= o.mass * (own * ga + theirs * gb);
                let v = velocities[a] - velocities[b];
                force.heating += own * o.mass * v.dot(ga);
                compression += o.mass * v.dot(ga);
                // Viscosity, for particles closing in.
                let closing = v.dot(d);
                if closing < 0.0 {
                    let h = 0.5 * (p.h + o.h);
                    let mu = h * closing / (r * r + EPSILON * h * h);
                    let rho = 0.5 * (p.density + o.density);
                    let c = 0.5 * (sound[a] + sound[b]);
                    let alpha = 0.5 * (p.viscosity + o.viscosity);
                    let pi = (-alpha * c * mu + 2.0 * alpha * mu * mu) / rho;
                    let mean = 0.5 * (ga + gb);
                    force.acceleration -= o.mass * pi * mean;
                    force.heating += 0.5 * o.mass * pi * v.dot(mean);
                    force.mu = force.mu.max(mu.abs());
                }
            }
            // Gravity, from every particle.
            for (b, o) in particles.iter().enumerate() {
                if b == a {
                    continue;
                }
                let d = p.position - o.position;
                let r = d.length();
                let inside = 0.5 * (kernel_mass(r / p.h) + kernel_mass(r / o.h));
                force.acceleration -= d * (G * o.mass * inside / r.powi(3));
            }
            let converging = (compression / (omega[a] * p.density)).max(0.0);
            let decay = SIGMA_DECAY * sound[a] / p.h;
            force.switch =
                -(p.viscosity - ALPHA_MIN) * decay + converging * (ALPHA_MAX - p.viscosity);
            force
        });
    }

    /// The time step: the Courant and force conditions, the smallest over
    /// the particles (Monaghan 1992's form, with Price et al. 2018's
    /// coefficients).
    pub fn time_step(&self) -> f64 {
        (0..self.particles.len())
            .map(|a| {
                let p = &self.particles[a];
                let c = self.sound_speed(a);
                let f = &self.forces[a];
                let courant =
                    COURANT * p.h / (c + 1.2 * p.viscosity * (c + 2.0 * f.mu)).max(1e-300);
                let force = FORCE * (p.h / f.acceleration.length().max(1e-300)).sqrt();
                courant.min(force)
            })
            .fold(f64::INFINITY, f64::min)
    }

    /// One leapfrog step (kick–drift–kick) of the time step; returns it.
    pub fn step(&mut self) -> f64 {
        let dt = self.time_step();
        let half = 0.5 * dt;
        for (p, f) in self.particles.iter_mut().zip(&self.forces) {
            p.velocity += half * f.acceleration;
            p.energy = (p.energy + half * f.heating).max(0.0);
            p.viscosity = (p.viscosity + half * f.switch).clamp(ALPHA_MIN, ALPHA_MAX);
            p.position += dt * p.velocity;
        }
        self.update_density();
        // The forces at the step's end, for velocities and energies
        // predicted there from the last forces.
        let velocities: Vec<DVec3> = self
            .particles
            .iter()
            .zip(&self.forces)
            .map(|(p, f)| p.velocity + half * f.acceleration)
            .collect();
        let energies: Vec<f64> = self.particles.iter().map(|p| p.energy).collect();
        for (p, f) in self.particles.iter_mut().zip(&self.forces) {
            p.energy = (p.energy + half * f.heating).max(0.0);
        }
        self.update_forces(&velocities);
        for ((p, f), u) in self.particles.iter_mut().zip(&self.forces).zip(&energies) {
            p.velocity += half * f.acceleration;
            p.energy = (u + half * f.heating).max(0.0);
            p.viscosity = (p.viscosity + half * f.switch).clamp(ALPHA_MIN, ALPHA_MAX);
        }
        self.time += dt;
        dt
    }

    /// Each particle's acceleration (m/s²) and heating (W/kg), as last
    /// worked out.
    pub fn accelerations(&self) -> Vec<(DVec3, f64)> {
        self.forces
            .iter()
            .map(|f| (f.acceleration, f.heating))
            .collect()
    }

    /// The kinetic energy, J.
    pub fn kinetic(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| 0.5 * p.mass * p.velocity.length_squared())
            .sum()
    }

    /// The internal (thermal) energy, J.
    pub fn thermal(&self) -> f64 {
        self.particles.iter().map(|p| p.mass * p.energy).sum()
    }

    /// The gravitational energy, J: the softened potential of every pair,
    /// averaged over their two smoothing lengths.
    pub fn gravitational(&self) -> f64 {
        let particles = &self.particles;
        parallel(particles.len(), |a| {
            let p = &particles[a];
            particles[a + 1..]
                .iter()
                .map(|o| {
                    let r = (p.position - o.position).length();
                    let psi =
                        0.5 * (kernel_potential(r / p.h) / p.h + kernel_potential(r / o.h) / o.h);
                    -G * p.mass * o.mass * psi
                })
                .sum::<f64>()
        })
        .into_iter()
        .sum()
    }

    /// The total energy, J.
    pub fn energy(&self) -> f64 {
        self.kinetic() + self.thermal() + self.gravitational()
    }

    /// Lets the fluid settle into equilibrium at constant entropy, P = Kρ^γ:
    /// for `duration` s, its motion damped away with time constant `damping`
    /// and each particle's energy set to K ρ^(γ−1)/(γ − 1) after every
    /// step. Then it is set at rest.
    pub fn settle(&mut self, k: f64, duration: f64, damping: f64) {
        let start = self.time;
        let isentropic = |fluid: &mut Self| {
            let gamma = fluid.gamma;
            for p in &mut fluid.particles {
                p.energy = k * p.density.powf(gamma - 1.0) / (gamma - 1.0);
            }
        };
        isentropic(self);
        while self.time - start < duration {
            let dt = self.step();
            let keep = (-dt / damping).exp();
            for p in &mut self.particles {
                p.velocity *= keep;
            }
            isentropic(self);
        }
        for p in &mut self.particles {
            p.velocity = DVec3::ZERO;
        }
        let velocities = vec![DVec3::ZERO; self.particles.len()];
        self.update_forces(&velocities);
    }

    /// The radius around the center of mass inside which `fraction` of
    /// the mass lies, m.
    pub fn mass_radius(&self, fraction: f64) -> f64 {
        let total: f64 = self.particles.iter().map(|p| p.mass).sum();
        let center = self
            .particles
            .iter()
            .map(|p| p.position * p.mass)
            .sum::<DVec3>()
            / total;
        let mut radii: Vec<(f64, f64)> = self
            .particles
            .iter()
            .map(|p| ((p.position - center).length(), p.mass))
            .collect();
        radii.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut inside = 0.0;
        for (r, m) in radii {
            inside += m;
            if inside >= fraction * total {
                return r;
            }
        }
        0.0
    }
}

/// The density of an n = 1 polytrope of `mass` and `radius` at `r` (Lane–
/// Emden; e.g. Chandrasekhar 1939, *An Introduction to the Study of Stellar
/// Structure*, ch. IV): ρ_c sin ξ/ξ with ξ = πr/R and ρ_c = πM/(4R³).
pub fn polytrope_density(mass: f64, radius: f64, r: f64) -> f64 {
    let xi = PI * r / radius;
    let central = PI * mass / (4.0 * radius.powi(3));
    if xi < 1e-8 {
        central
    } else if xi >= PI {
        0.0
    } else {
        central * xi.sin() / xi
    }
}

/// The fraction of an n = 1 polytrope's mass within `r`: (sin ξ − ξ cos ξ)/π.
pub fn polytrope_mass(radius: f64, r: f64) -> f64 {
    let xi = (PI * r / radius).min(PI);
    (xi.sin() - xi * xi.cos()) / PI
}

/// An n = 1 polytrope (P = Kρ², γ = 2) of `mass` and `radius` as about
/// `count` SPH particles of equal mass, at rest. Every n = 1 polytrope of
/// a given K has the same radius, R = π√(K/2πG), whatever its mass, so
/// K = 2GR²/π; Jupiter is close to one.
///
/// The particles start on a cubic lattice in a sphere, moved out along
/// their radius so the mass within each radius is the polytrope's; each
/// gets the energy K ρ (u = P/((γ − 1)ρ)). They are not yet in SPH's own
/// equilibrium: [`Fluid::settle`] that first.
pub fn polytrope(mass: f64, radius: f64, count: usize) -> (Fluid, f64) {
    let (particles, k) = polytrope_particles(mass, radius, count);
    (Fluid::new(particles, 2.0), k)
}

/// [`polytrope`]'s particles and K, without working out their densities
/// and forces (for a fluid stepped elsewhere, such as on the GPU).
pub fn polytrope_particles(mass: f64, radius: f64, count: usize) -> (Vec<Particle>, f64) {
    let k = 2.0 * G * radius * radius / PI;
    // The lattice spacing that puts about `count` points in a unit sphere.
    let spacing = (4.0 * PI / (3.0 * count as f64)).cbrt();
    let n = (1.0 / spacing).ceil() as i64 + 1;
    let mut points = Vec::new();
    for i in -n..=n {
        for j in -n..=n {
            for l in -n..=n {
                let x = DVec3::new(i as f64 + 0.5, j as f64 + 0.5, l as f64 + 0.5) * spacing;
                if x.length() < 1.0 {
                    points.push(x);
                }
            }
        }
    }
    let m = mass / points.len() as f64;
    let particles = points
        .iter()
        .map(|x| {
            // The radius within which the polytrope holds the fraction x³
            // of its mass, by halving.
            let fraction = x.length().powi(3);
            let (mut lo, mut hi) = (0.0, radius);
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if polytrope_mass(radius, mid) < fraction {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let r = 0.5 * (lo + hi);
            let rho =
                polytrope_density(mass, radius, r).max(1e-3 * polytrope_density(mass, radius, 0.0));
            Particle {
                position: x.normalize_or_zero() * r,
                velocity: DVec3::ZERO,
                mass: m,
                energy: k * rho,
                h: ETA * (m / rho).cbrt(),
                density: rho,
                viscosity: ALPHA_MIN,
            }
        })
        .collect();
    (particles, k)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kernel_holds_its_mass_and_its_potential_matches_its_pull() {
        // ∫ 4πq² w dq over its support is 1, and M(q) is that integral.
        let n = 20_000;
        let mut inside = 0.0;
        for k in 0..n {
            let q = 2.0 * (k as f64 + 0.5) / n as f64;
            inside += 4.0 * PI * q * q * kernel(q) * 2.0 / n as f64;
            let next = 2.0 * (k as f64 + 1.0) / n as f64;
            if (k + 1) % 2000 == 0 {
                assert!(
                    (inside - kernel_mass(next)).abs() < 1e-7,
                    "M({next}) = {} against {inside}",
                    kernel_mass(next)
                );
            }
        }
        assert!((kernel_mass(2.0) - 1.0).abs() < 1e-14);
        // −dψ/dq = M(q)/q², and ψ is 7/5 at the center, 1/q from 2 out.
        for q in [0.1, 0.5, 0.99, 1.01, 1.5, 1.99, 2.5] {
            let step = 1e-6;
            let slope = (kernel_potential(q + step) - kernel_potential(q - step)) / (2.0 * step);
            assert!(
                (slope + kernel_mass(q) / (q * q)).abs() < 1e-8,
                "at q = {q}"
            );
        }
        assert!((kernel_potential(0.0) - 1.4).abs() < 1e-15);
        assert!((kernel_potential(2.0) - 0.5).abs() < 1e-14);
        // w′ is the slope of w.
        for q in [0.2, 0.8, 1.3, 1.9] {
            let slope = (kernel(q + 1e-6) - kernel(q - 1e-6)) / 2e-6;
            assert!((slope - kernel_slope(q)).abs() < 1e-8);
        }
    }
}
