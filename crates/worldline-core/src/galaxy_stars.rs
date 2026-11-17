//! The galaxy's stars as a swarm of tracers (step 7.2): sampled from the
//! stellar parts of McMillan's (2017) mass model, moving in its potential,
//! and pulled by any massive body. They pull on nothing: the model's smooth
//! potential is already their mass. See `docs/physics/galaxy-stars.md`.
//!
//! **Places** follow each component's density exactly. **Velocities** come
//! from the axisymmetric Jeans equations for a distribution function of
//! energy and angular momentum about the axis alone, f(E, L_z) (Binney &
//! Tremaine 2008, ch. 4): then σ_R = σ_z = σ, and
//! - ρσ²(R, z) = ∫_z^∞ ρ ∂Φ/∂z′ dz′,
//! - ρ⟨v_φ²⟩ = ρσ² + R ∂(ρσ²)/∂R + Rρ ∂Φ/∂R,
//!
//! with ρ the component's density and Φ the whole galaxy's potential. How
//! ⟨v_φ²⟩ splits into rotation and dispersion is free: Worldline takes
//! σ_φ = σ, all the rest as rotation (Satoh's 1980 k = 1). Each velocity is
//! drawn from a Gaussian with those moments, an approximation: a real f(E,
//! L_z) isn't Gaussian.

use std::f64::consts::TAU;

use glam::DVec3;

use crate::milky_way::{Disk, KILOPARSEC, MassModel, Potential, Spheroid};

/// A star of the swarm, in galactocentric axes: the center at the origin, z
/// toward the north galactic pole, x from the Sun toward the center.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    /// m.
    pub position: DVec3,
    /// m/s.
    pub velocity: DVec3,
    /// Which population it was drawn from, as an index into the sampler's
    /// components; [`MEASURED`] for one placed from a catalogue.
    pub population: u8,
}

/// The population of a star placed from a catalogue rather than drawn.
pub const MEASURED: u8 = u8::MAX;

/// A part of the galaxy's stars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Component {
    /// A disk, ρ = Σ(R) h(z).
    Disk(Disk),
    /// A spheroid: the bulge.
    Spheroid(Spheroid),
}

impl Component {
    /// Its name.
    pub fn name(&self) -> &'static str {
        match self {
            Component::Disk(d) => d.name,
            Component::Spheroid(s) => s.name,
        }
    }

    /// Its density at cylindrical radius `r` and height `z` (m), kg/m³.
    pub fn density(&self, r: f64, z: f64) -> f64 {
        match self {
            Component::Disk(d) => d.density(r, z),
            Component::Spheroid(s) => s.density_at(r, z),
        }
    }

    /// Its whole mass, kg.
    pub fn mass(&self) -> f64 {
        match self {
            Component::Disk(d) => d.mass(),
            Component::Spheroid(s) => s.mass_within(1e3 * s.scale.max(s.cutoff.unwrap_or(0.0))),
        }
    }
}

/// The stellar parts of a mass model: its bulge and its thin and thick
/// disks, by name. The gas disks and the dark halo hold no stars.
pub fn stellar_components(model: &MassModel) -> Vec<Component> {
    let disks = model
        .disks
        .iter()
        .filter(|d| !d.name.contains("gas"))
        .map(|d| Component::Disk(*d));
    let bulge = model
        .spheroids
        .iter()
        .filter(|s| s.name == "bulge")
        .map(|s| Component::Spheroid(*s));
    disks.chain(bulge).collect()
}

/// A small, fast, deterministic random generator: SplitMix64 (Steele, Lea &
/// Flood 2014), so that a seed always draws the same stars.
#[derive(Debug, Clone)]
pub struct Random(u64);

impl Random {
    /// A generator started from `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in (0, 1].
    pub fn uniform(&mut self) -> f64 {
        ((self.next_u64() >> 11) + 1) as f64 / (1u64 << 53) as f64
    }

    /// A standard normal deviate (Box–Muller).
    pub fn gaussian(&mut self) -> f64 {
        (-2.0 * self.uniform().ln()).sqrt() * (TAU * self.uniform()).cos()
    }
}

/// Where the Jeans tables are solved: ln R from 1 pc to 100 kpc, and
/// z = a sinh u from the plane to 100 kpc, with a = 10 pc so that the plane
/// is finely resolved.
const R_INNER: f64 = 1e-3 * KILOPARSEC;
const R_OUTER: f64 = 100.0 * KILOPARSEC;
const R_NODES: usize = 400;
const Z_SCALE: f64 = 1e-2 * KILOPARSEC;
const Z_OUTER: f64 = 100.0 * KILOPARSEC;
const Z_NODES: usize = 500;

/// One component's Jeans solution on the grid: σ² and ⟨v_φ²⟩, m²/s².
struct Jeans {
    sigma2: Vec<f64>,
    vphi2: Vec<f64>,
}

/// The grid's nodes: ln R, and u with z = a sinh u.
fn grid() -> (Vec<f64>, Vec<f64>) {
    let (lo, hi) = (R_INNER.ln(), R_OUTER.ln());
    let lnr = (0..R_NODES)
        .map(|i| lo + (hi - lo) * i as f64 / (R_NODES - 1) as f64)
        .collect();
    let top = (Z_OUTER / Z_SCALE).asinh();
    let u = (0..Z_NODES)
        .map(|j| top * j as f64 / (Z_NODES - 1) as f64)
        .collect();
    (lnr, u)
}

/// Runs `f` on 0..n, spread over the computer's cores.
fn parallel<T: Send>(n: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
    if n < 64 || threads == 1 {
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
            .flat_map(|h| h.join().expect("a thread finished"))
            .collect()
    })
}

impl Jeans {
    /// Solves `component`'s Jeans equations in the potential whose
    /// derivatives on the grid are `forces` (∂Φ/∂R, ∂Φ/∂z at each node).
    fn solve(component: &Component, forces: &[(f64, f64)], lnr: &[f64], u: &[f64]) -> Self {
        let du = u[1] - u[0];
        // ρσ² by the vertical equation, integrated down from the top by the
        // trapezoid rule in u, where dz = a cosh u du.
        let rows: Vec<(Vec<f64>, Vec<f64>)> = parallel(R_NODES, |i| {
            let r = lnr[i].exp();
            let rho: Vec<f64> = u
                .iter()
                .map(|&u| component.density(r, Z_SCALE * u.sinh()))
                .collect();
            let integrand: Vec<f64> = (0..Z_NODES)
                .map(|j| rho[j] * forces[i * Z_NODES + j].1 * Z_SCALE * u[j].cosh())
                .collect();
            let mut pressure = vec![0.0; Z_NODES];
            for j in (0..Z_NODES - 1).rev() {
                pressure[j] = pressure[j + 1] + 0.5 * du * (integrand[j] + integrand[j + 1]);
            }
            (rho, pressure)
        });
        let dlnr = lnr[1] - lnr[0];
        let mut sigma2 = vec![0.0; R_NODES * Z_NODES];
        let mut vphi2 = vec![0.0; R_NODES * Z_NODES];
        for i in 0..R_NODES {
            let r = lnr[i].exp();
            for j in 0..Z_NODES {
                let (rho, p) = (rows[i].0[j], rows[i].1[j]);
                if rho <= 0.0 || p <= 0.0 {
                    continue;
                }
                // ∂(ρσ²)/∂ln R by central differences, one-sided at the ends.
                let (a, b) = (i.saturating_sub(1), (i + 1).min(R_NODES - 1));
                let slope = (rows[b].1[j] - rows[a].1[j]) / ((b - a) as f64 * dlnr);
                let k = i * Z_NODES + j;
                sigma2[k] = p / rho;
                vphi2[k] = (sigma2[k] + slope / rho + r * forces[k].0).max(0.0);
            }
        }
        Self { sigma2, vphi2 }
    }
}

/// Draws stars from a galaxy's stellar components, in equilibrium in its
/// potential as the Jeans equations give it.
pub struct Sampler<'a> {
    potential: &'a Potential,
    components: Vec<Component>,
    masses: Vec<f64>,
    jeans: Vec<Jeans>,
    lnr: Vec<f64>,
    u: Vec<f64>,
    /// For each spheroid, its mass's cumulative distribution in the
    /// spheroidal radius m: (ln m, fraction within).
    spheroid_cdf: Vec<Vec<(f64, f64)>>,
}

impl<'a> Sampler<'a> {
    /// Solves the Jeans equations of `components` in `potential`.
    pub fn new(potential: &'a Potential, components: Vec<Component>) -> Self {
        let (lnr, u) = grid();
        let forces: Vec<(f64, f64)> = parallel(R_NODES * Z_NODES, |k| {
            let (i, j) = (k / Z_NODES, k % Z_NODES);
            let (_, dr, dz) = potential.evaluate(lnr[i].exp(), Z_SCALE * u[j].sinh());
            (dr, dz)
        });
        let jeans = components
            .iter()
            .map(|c| Jeans::solve(c, &forces, &lnr, &u))
            .collect();
        let masses = components.iter().map(Component::mass).collect();
        let spheroid_cdf = components
            .iter()
            .map(|c| match c {
                Component::Spheroid(s) => spheroid_cdf(s),
                Component::Disk(_) => Vec::new(),
            })
            .collect();
        Self {
            potential,
            components,
            masses,
            jeans,
            lnr,
            u,
            spheroid_cdf,
        }
    }

    /// The components, in order.
    pub fn components(&self) -> &[Component] {
        &self.components
    }

    /// Each component's mass, kg.
    pub fn masses(&self) -> &[f64] {
        &self.masses
    }

    /// The potential the stars move in.
    pub fn potential(&self) -> &Potential {
        self.potential
    }

    /// σ² and ⟨v_φ²⟩ of component `c` at (R, z), m²/s², interpolated.
    pub fn moments(&self, c: usize, r: f64, z: f64) -> (f64, f64) {
        let first = self.lnr[0];
        let dlnr = self.lnr[1] - first;
        let x = ((r.max(R_INNER).ln() - first) / dlnr).clamp(0.0, (R_NODES - 1) as f64 - 1e-9);
        let y = ((z.abs() / Z_SCALE).asinh() / (self.u[1] - self.u[0]))
            .clamp(0.0, (Z_NODES - 1) as f64 - 1e-9);
        let (i, j) = (x as usize, y as usize);
        let (fx, fy) = (x - i as f64, y - j as f64);
        let at = |table: &[f64]| {
            let k = i * Z_NODES + j;
            (1.0 - fx) * (1.0 - fy) * table[k]
                + fx * (1.0 - fy) * table[k + Z_NODES]
                + (1.0 - fx) * fy * table[k + 1]
                + fx * fy * table[k + Z_NODES + 1]
        };
        let jeans = &self.jeans[c];
        (at(&jeans.sigma2), at(&jeans.vphi2))
    }

    /// A place drawn from component `c`'s density: (R, z), m.
    fn place(&self, c: usize, random: &mut Random) -> (f64, f64) {
        match &self.components[c] {
            Component::Disk(d) => {
                // Σ R ∝ R e^(−R/R_d): a gamma distribution of shape 2.
                let r = -d.scale_length * (random.uniform() * random.uniform()).ln();
                let z = -d.scale_height * random.uniform().ln();
                let side = if random.uniform() < 0.5 { -1.0 } else { 1.0 };
                (r, side * z)
            }
            Component::Spheroid(s) => {
                // The spheroidal radius from its mass's distribution, then a
                // direction uniform on the sphere, squashed by q.
                let cdf = &self.spheroid_cdf[c];
                let want = random.uniform();
                let k = cdf
                    .partition_point(|&(_, f)| f < want)
                    .clamp(1, cdf.len() - 1);
                let ((u0, f0), (u1, f1)) = (cdf[k - 1], cdf[k]);
                let m = (u0 + (u1 - u0) * (want - f0) / (f1 - f0).max(1e-300)).exp();
                let cos = 2.0 * random.uniform() - 1.0;
                (m * (1.0 - cos * cos).sqrt(), m * s.axis_ratio * cos)
            }
        }
    }

    /// One star of component `c`.
    pub fn draw(&self, c: usize, random: &mut Random) -> Star {
        let (r, z) = self.place(c, random);
        let phi = TAU * random.uniform();
        let (sin, cos) = phi.sin_cos();
        let (sigma2, vphi2) = self.moments(c, r, z);
        let sigma = sigma2.sqrt();
        // All of ⟨v_φ²⟩ beyond σ² is rotation; where there's none, the
        // dispersion takes all of ⟨v_φ²⟩.
        let (mean, spread) = if vphi2 > sigma2 {
            ((vphi2 - sigma2).sqrt(), sigma)
        } else {
            (0.0, vphi2.sqrt())
        };
        let (v_r, v_z) = (sigma * random.gaussian(), sigma * random.gaussian());
        let v_phi = mean + spread * random.gaussian();
        // The galaxy turns clockwise seen from its north pole: at the Sun,
        // on the −x side, toward +y.
        let radial = DVec3::new(cos, sin, 0.0);
        let along = DVec3::new(sin, -cos, 0.0);
        Star {
            position: DVec3::new(r * cos, r * sin, z),
            velocity: radial * v_r + along * v_phi + DVec3::Z * v_z,
            population: c as u8,
        }
    }

    /// `count` stars from all the components, each in proportion to its
    /// mass.
    pub fn sample(&self, count: usize, seed: u64) -> Vec<Star> {
        let total: f64 = self.masses.iter().sum();
        let mut random = Random::new(seed);
        let mut stars = Vec::with_capacity(count);
        let mut drawn = 0usize;
        let mut share = 0.0;
        for (c, mass) in self.masses.iter().enumerate() {
            share += mass / total;
            let until = if c + 1 == self.masses.len() {
                count
            } else {
                (share * count as f64).round() as usize
            };
            for _ in drawn..until {
                stars.push(self.draw(c, &mut random));
            }
            drawn = until;
        }
        stars
    }
}

/// A spheroid's mass within spheroidal radius m, as a fraction of its whole,
/// tabulated in ln m from a millionth of its scale to a thousand times its
/// cutoff.
fn spheroid_cdf(s: &Spheroid) -> Vec<(f64, f64)> {
    let lo = (1e-6 * s.scale).ln();
    let hi = (1e3 * s.scale.max(s.cutoff.unwrap_or(0.0))).ln();
    let n = 20_000;
    let step = (hi - lo) / n as f64;
    let mut out = Vec::with_capacity(n + 1);
    let mut total = 0.0;
    let integrand = |u: f64| {
        let m = u.exp();
        s.density_at(m, 0.0) * m * m * m
    };
    out.push((lo, 0.0));
    for i in 0..n {
        let (a, b) = (lo + step * i as f64, lo + step * (i + 1) as f64);
        // Simpson's rule on each piece.
        total += step / 6.0 * (integrand(a) + 4.0 * integrand(0.5 * (a + b)) + integrand(b));
        out.push((b, total));
    }
    for p in &mut out {
        p.1 /= total;
    }
    out
}

/// A massive body the stars feel, moving along a cubic through its places
/// and velocities at the start and end of an interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Perturber {
    /// GM, m³/s².
    pub gm: f64,
    /// Its radius, m: a star reaching it is swallowed.
    pub radius: f64,
    /// Place and velocity at the interval's start, galactocentric.
    pub start: (DVec3, DVec3),
    /// And at its end.
    pub end: (DVec3, DVec3),
}

impl Perturber {
    /// Where it is and how it moves at fraction `s` of an interval `h` (s)
    /// long: the cubic Hermite through both ends.
    pub fn at(&self, s: f64, h: f64) -> (DVec3, DVec3) {
        if h <= 0.0 {
            return self.end;
        }
        let ((x0, v0), (x1, v1)) = (self.start, self.end);
        let (s2, s3) = (s * s, s * s * s);
        let x = x0 * (2.0 * s3 - 3.0 * s2 + 1.0)
            + v0 * (h * (s3 - 2.0 * s2 + s))
            + x1 * (3.0 * s2 - 2.0 * s3)
            + v1 * (h * (s3 - s2));
        let v = (x0 * (6.0 * s2 - 6.0 * s) + x1 * (6.0 * s - 6.0 * s2)) / h
            + v0 * (3.0 * s2 - 4.0 * s + 1.0)
            + v1 * (3.0 * s2 - 2.0 * s);
        (x, v)
    }
}

/// The fraction of a star's dynamical time, √(r/|a|), a step may take in
/// the galaxy's field: about 300 steps an orbit.
pub const GALAXY_STEP: f64 = 0.02;

/// The fraction of the time to cross the distance to a perturber, or of
/// its dynamical time there, a step may take.
pub const ENCOUNTER_STEP: f64 = 0.02;

/// Carries `stars` from time 0 to `h` (s) through `potential`, pulled by
/// `perturbers`, each by leapfrog (kick–drift–kick) steps of its own.
/// Returns which were swallowed by a perturber, by index.
pub fn advance(
    stars: &mut [Star],
    potential: &Potential,
    perturbers: &[Perturber],
    h: f64,
) -> Vec<usize> {
    advance_with(stars, potential, perturbers, h, GALAXY_STEP)
}

/// [`advance`], with steps of `eta` of each star's dynamical time.
pub fn advance_with(
    stars: &mut [Star],
    potential: &Potential,
    perturbers: &[Perturber],
    h: f64,
    eta: f64,
) -> Vec<usize> {
    if h <= 0.0 {
        return Vec::new();
    }
    let carried: Vec<(Star, bool)> = parallel(stars.len(), |k| {
        let mut star = stars[k];
        let swallowed = carry(&mut star, potential, perturbers, h, eta);
        (star, swallowed)
    });
    let mut gone = Vec::new();
    for (k, (star, swallowed)) in carried.into_iter().enumerate() {
        stars[k] = star;
        if swallowed {
            gone.push(k);
        }
    }
    gone
}

/// The pull on a star at `x` at time `t`, and the longest step its
/// surroundings allow there. None if a perturber has swallowed it.
fn pull(
    x: DVec3,
    v: DVec3,
    t: f64,
    potential: &Potential,
    perturbers: &[Perturber],
    h: f64,
    eta: f64,
) -> Option<(DVec3, f64)> {
    let galaxy = potential.acceleration(x);
    let mut longest = eta * (x.length() / galaxy.length().max(1e-300)).sqrt();
    let mut a = galaxy;
    for p in perturbers {
        let (at, moving) = p.at(t / h, h);
        let d = at - x;
        let r = d.length();
        if r <= p.radius {
            return None;
        }
        a += d * (p.gm / (r * r * r));
        let crossing = r / (moving - v).length().max(1e-300);
        let dynamical = (r * r * r / p.gm).sqrt();
        longest = longest.min(ENCOUNTER_STEP * crossing.min(dynamical));
    }
    Some((a, longest))
}

/// Carries one star; returns whether it was swallowed.
fn carry(
    star: &mut Star,
    potential: &Potential,
    perturbers: &[Perturber],
    h: f64,
    eta: f64,
) -> bool {
    let mut t = 0.0;
    let Some((mut a, mut longest)) = pull(
        star.position,
        star.velocity,
        t,
        potential,
        perturbers,
        h,
        eta,
    ) else {
        return true;
    };
    while t < h {
        let dt = longest.min(h - t);
        star.velocity += a * (0.5 * dt);
        star.position += star.velocity * dt;
        t = if dt >= h - t { h } else { t + dt };
        match pull(
            star.position,
            star.velocity,
            t,
            potential,
            perturbers,
            h,
            eta,
        ) {
            Some((next, step)) => (a, longest) = (next, step),
            None => return true,
        }
        star.velocity += a * (0.5 * dt);
    }
    false
}

/// How [`equilibrium`] builds its swarm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Recipe {
    /// Orbits drawn for every star kept: enough that few are kept twice.
    pub orbits_per_star: usize,
    /// How long the drawn orbits are let mix before they're watched, s.
    pub settle: f64,
    /// How long each is watched, s: the time its place is averaged over.
    pub window: f64,
    /// How often its place is recorded, s.
    pub record: f64,
    /// Steps of this fraction of each star's dynamical time.
    pub eta: f64,
}

impl Recipe {
    /// Three orbits a star; a galactic year at the Sun's radius to settle;
    /// four to watch, recorded every million years, in steps of a twentieth
    /// of the dynamical time (about 120 an orbit, enough to say where an
    /// orbit spends its time).
    pub fn standard() -> Self {
        let year = 365.25 * 86_400.0;
        Self {
            orbits_per_star: 3,
            settle: 216e6 * year,
            window: 864e6 * year,
            record: 1e6 * year,
            eta: 0.05,
        }
    }
}

/// The bins orbits are weighted in: cylindrical radius and height, kpc.
const WEIGHT_R: [f64; 36] = [
    0.0, 0.125, 0.25, 0.375, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 5.0,
    5.5, 6.0, 6.5, 7.0, 7.5, 8.0, 8.5, 9.0, 9.5, 10.0, 11.0, 12.0, 13.0, 14.0, 15.5, 17.0, 18.5,
    20.0, 25.0,
];
const WEIGHT_Z: [f64; 16] = [
    0.0, 0.025, 0.05, 0.1, 0.15, 0.225, 0.3, 0.45, 0.6, 0.8, 1.0, 1.25, 1.5, 2.0, 2.5, 3.5,
];

/// A place's weighting bin.
fn weight_bin(x: DVec3) -> usize {
    let r = x.x.hypot(x.y) / KILOPARSEC;
    let z = x.z.abs() / KILOPARSEC;
    let i = WEIGHT_R.partition_point(|&e| e <= r) - 1;
    let j = WEIGHT_Z.partition_point(|&e| e <= z) - 1;
    i * WEIGHT_Z.len() + j
}

/// `count` stars in equilibrium in the sampler's potential, matching each
/// component's density: Schwarzschild's (1979) method of superposing
/// orbits (Binney & Tremaine 2008, ch. 4).
/// 1. Orbits are drawn from the Jeans equations, near equilibrium, a few
///    for every star wanted.
/// 2. Each is let mix for `settle`, then watched for `window`, its place
///    recorded every `record`: where it spends its time.
/// 3. Each is weighted so that, all together, the time they spend in each
///    bin is the component's mass there (Richardson 1972; Lucy 1974).
/// 4. Stars are drawn from the orbits by weight, each at a random moment of
///    its window.
///
/// An orbit's weight depends on nothing that changes along it, and a star
/// is as likely to be at any moment of it: so the swarm stays as it is.
/// Returns the stars and how many were drawn from an orbit already used.
pub fn equilibrium(
    sampler: &Sampler,
    count: usize,
    seed: u64,
    recipe: Recipe,
) -> (Vec<Star>, usize) {
    let mut random = Random::new(seed);
    let total: f64 = sampler.masses.iter().sum();
    let bins = WEIGHT_R.len() * WEIGHT_Z.len();
    let mut out = Vec::with_capacity(count);
    let mut repeats = 0;
    let mut drawn = 0usize;
    let mut share = 0.0;
    for (c, component) in sampler.components.iter().enumerate() {
        share += sampler.masses[c] / total;
        let until = if c + 1 == sampler.components.len() {
            count
        } else {
            (share * count as f64).round() as usize
        };
        let wanted = until - drawn;
        drawn = until;
        if wanted == 0 {
            continue;
        }
        // 1. The orbits, and three moments in each one's window to keep.
        let n = wanted * recipe.orbits_per_star;
        let mut orbits: Vec<Star> = (0..n).map(|_| sampler.draw(c, &mut random)).collect();
        let records = (recipe.window / recipe.record).round() as usize;
        let moments: Vec<[usize; 3]> = (0..n)
            .map(|_| {
                let mut pick = || ((random.uniform() * records as f64) as usize).min(records - 1);
                [pick(), pick(), pick()]
            })
            .collect();
        // 2. Settle, then watch.
        advance_with(
            &mut orbits,
            sampler.potential,
            &[],
            recipe.settle,
            recipe.eta,
        );
        let mut visits: Vec<Vec<(u16, u16)>> = vec![Vec::new(); n];
        let mut kept: Vec<[Star; 3]> = vec![[orbits[0]; 3]; n];
        for k in 0..records {
            advance_with(
                &mut orbits,
                sampler.potential,
                &[],
                recipe.record,
                recipe.eta,
            );
            for (o, star) in orbits.iter().enumerate() {
                let b = weight_bin(star.position) as u16;
                match visits[o].iter_mut().find(|v| v.0 == b) {
                    Some(v) => v.1 += 1,
                    None => visits[o].push((b, 1)),
                }
                for (m, &moment) in moments[o].iter().enumerate() {
                    if moment == k {
                        kept[o][m] = *star;
                    }
                }
            }
        }
        // 3. The weights: what each bin should hold, and Richardson–Lucy.
        let target: Vec<f64> = (0..bins)
            .map(|b| {
                let (i, j) = (b / WEIGHT_Z.len(), b % WEIGHT_Z.len());
                let edge = |edges: &[f64], k: usize| {
                    (
                        edges[k] * KILOPARSEC,
                        edges.get(k + 1).map_or(f64::INFINITY, |e| e * KILOPARSEC),
                    )
                };
                mass_in(component, edge(&WEIGHT_R, i), edge(&WEIGHT_Z, j))
            })
            .collect();
        let target_sum: f64 = target.iter().sum();
        let target: Vec<f64> = target.iter().map(|t| t / target_sum).collect();
        let mut weights = vec![1.0 / n as f64; n];
        let fraction = |v: &(u16, u16)| f64::from(v.1) / records as f64;
        for _ in 0..200 {
            let mut model = vec![0.0; bins];
            for (o, v) in visits.iter().enumerate() {
                for visit in v {
                    model[visit.0 as usize] += weights[o] * fraction(visit);
                }
            }
            for (o, v) in visits.iter().enumerate() {
                let factor: f64 = v
                    .iter()
                    .map(|visit| {
                        let b = visit.0 as usize;
                        fraction(visit) * target[b] / model[b].max(1e-300)
                    })
                    .sum();
                weights[o] *= factor;
            }
        }
        // 4. Stars by weight, systematically, each at one of its moments.
        let sum: f64 = weights.iter().sum();
        let offset = random.uniform();
        let mut cumulative = 0.0;
        let mut next = 0usize;
        for o in 0..n {
            cumulative += weights[o] / sum * wanted as f64;
            let mut used = 0;
            while next < wanted && (next as f64 + offset) < cumulative {
                if used > 0 {
                    repeats += 1;
                }
                out.push(Star {
                    population: c as u8,
                    ..kept[o][used % 3]
                });
                used += 1;
                next += 1;
            }
        }
        while next < wanted {
            out.push(Star {
                population: c as u8,
                ..kept[n - 1][0]
            });
            next += 1;
        }
    }
    (out, repeats)
}

/// The mass of `component` in the cylindrical shell between `r` = (R₁, R₂)
/// and the slabs |z| between `z` = (z₁, z₂) (m), kg: exactly for a disk,
/// by Gauss–Legendre quadrature for a spheroid.
pub fn mass_in(component: &Component, r: (f64, f64), z: (f64, f64)) -> f64 {
    match component {
        Component::Disk(d) => {
            let within = |x: f64| {
                if x.is_infinite() {
                    1.0
                } else {
                    let x = x / d.scale_length;
                    1.0 - (1.0 + x) * (-x).exp()
                }
            };
            let above = |z: f64| {
                if z.is_infinite() {
                    0.0
                } else {
                    (-z / d.scale_height).exp()
                }
            };
            d.mass() * (within(r.1) - within(r.0)) * (above(z.0) - above(z.1))
        }
        Component::Spheroid(s) => {
            // Both slabs, so twice one side; each range in pieces on a
            // stretched variable, with the infinite ends cut where the
            // density is gone.
            let far = 50.0 * s.cutoff.unwrap_or(1e3 * s.scale);
            let (r0, r1) = (r.0, r.1.min(far));
            let (z0, z1) = (z.0, z.1.min(far));
            if r1 <= r0 || z1 <= z0 {
                return 0.0;
            }
            let rule = crate::milky_way::gauss_legendre(16);
            let pieces = 16;
            // sinh-stretched coordinates, fine near zero where the bulge's
            // core is.
            let scale = s.scale;
            let stretch = |lo: f64, hi: f64| ((lo / scale).asinh(), (hi / scale).asinh());
            let (ur0, ur1) = stretch(r0, r1);
            let (uz0, uz1) = stretch(z0, z1);
            let mut total = 0.0;
            for i in 0..pieces {
                let (a, b) = (
                    ur0 + (ur1 - ur0) * i as f64 / pieces as f64,
                    ur0 + (ur1 - ur0) * (i + 1) as f64 / pieces as f64,
                );
                for &(xr, wr) in &rule {
                    let ur = 0.5 * (a + b) + 0.5 * (b - a) * xr;
                    let rr = scale * ur.sinh();
                    let jr = 0.5 * (b - a) * wr * scale * ur.cosh();
                    for k in 0..pieces {
                        let (c, d) = (
                            uz0 + (uz1 - uz0) * k as f64 / pieces as f64,
                            uz0 + (uz1 - uz0) * (k + 1) as f64 / pieces as f64,
                        );
                        for &(xz, wz) in &rule {
                            let uz = 0.5 * (c + d) + 0.5 * (d - c) * xz;
                            let zz = scale * uz.sinh();
                            let jz = 0.5 * (d - c) * wz * scale * uz.cosh();
                            total += s.density_at(rr, zz) * TAU * rr * jr * jz;
                        }
                    }
                }
            }
            2.0 * total
        }
    }
}

/// The bins a swarm's density is checked in: cylindrical radius and height
/// above or below the plane, kpc, the last of each running to infinity.
pub const CHECK_R: [f64; 18] = [
    0.0, 0.25, 0.5, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 12.0, 14.0, 17.0, 20.0,
];
/// See [`CHECK_R`].
pub const CHECK_Z: [f64; 8] = [0.0, 0.05, 0.15, 0.3, 0.6, 1.0, 1.5, 2.5];

/// One bin of a [`Fit`]: its radius and height ranges (kpc), and the stars
/// seen in it and expected.
pub type Bin = ((f64, f64), (f64, f64), usize, f64);

/// How well a swarm matches its components' density.
#[derive(Debug, Clone, PartialEq)]
pub struct Fit {
    /// Pearson's χ² over the bins expecting 10 stars or more, the rest
    /// pooled into one.
    pub chi2: f64,
    /// Its degrees of freedom.
    pub dof: usize,
    /// What χ² exceeds by chance once in a thousand times, by Wilson &
    /// Hilferty's (1931) approximation.
    pub limit: f64,
    /// The bins furthest off.
    pub furthest: Vec<Bin>,
}

impl Fit {
    /// Whether the swarm matches: χ² under its limit.
    pub fn passes(&self) -> bool {
        self.chi2 < self.limit
    }
}

/// How well `stars` (populations indexing `components`; measured ones left
/// out) match the density each component's mass should give them in the
/// [`CHECK_R`] × [`CHECK_Z`] bins.
pub fn density_fit(stars: &[Star], components: &[Component]) -> Fit {
    let edge =
        |edges: &[f64], k: usize| (edges[k], edges.get(k + 1).copied().unwrap_or(f64::INFINITY));
    let bins = CHECK_R.len() * CHECK_Z.len();
    let mut seen = vec![0usize; bins];
    let mut counts = vec![0usize; components.len()];
    for s in stars
        .iter()
        .filter(|s| (s.population as usize) < components.len())
    {
        counts[s.population as usize] += 1;
        let r = s.position.x.hypot(s.position.y) / KILOPARSEC;
        let z = s.position.z.abs() / KILOPARSEC;
        let i = CHECK_R.partition_point(|&e| e <= r) - 1;
        let j = CHECK_Z.partition_point(|&e| e <= z) - 1;
        seen[i * CHECK_Z.len() + j] += 1;
    }
    let mut expected = vec![0.0; bins];
    for (c, n) in components.iter().zip(&counts) {
        let total = c.mass();
        for (b, e) in expected.iter_mut().enumerate() {
            let (r, z) = (
                edge(&CHECK_R, b / CHECK_Z.len()),
                edge(&CHECK_Z, b % CHECK_Z.len()),
            );
            let m = mass_in(
                c,
                (r.0 * KILOPARSEC, r.1 * KILOPARSEC),
                (z.0 * KILOPARSEC, z.1 * KILOPARSEC),
            );
            *e += *n as f64 * m / total;
        }
    }
    let (mut chi2, mut used) = (0.0, 0usize);
    let (mut rest_seen, mut rest_expected) = (0usize, 0.0);
    let mut furthest = Vec::new();
    for (b, (&o, &e)) in seen.iter().zip(&expected).enumerate() {
        if e >= 10.0 {
            chi2 += (o as f64 - e).powi(2) / e;
            used += 1;
            let (r, z) = (
                edge(&CHECK_R, b / CHECK_Z.len()),
                edge(&CHECK_Z, b % CHECK_Z.len()),
            );
            furthest.push((r, z, o, e));
        } else {
            rest_seen += o;
            rest_expected += e;
        }
    }
    if rest_expected > 0.0 {
        chi2 += (rest_seen as f64 - rest_expected).powi(2) / rest_expected;
        used += 1;
    }
    let sigma = |x: &Bin| ((x.2 as f64 - x.3) / x.3.sqrt()).abs();
    furthest.sort_by(|a, b| sigma(b).total_cmp(&sigma(a)));
    furthest.truncate(3);
    let dof = used - 1;
    let k = dof as f64;
    // The standard normal's 99.9th percentile.
    let z = 3.090;
    let limit = k * (1.0 - 2.0 / (9.0 * k) + z * (2.0 / (9.0 * k)).sqrt()).powi(3);
    Fit {
        chi2,
        dof,
        limit,
        furthest,
    }
}

impl std::fmt::Display for Fit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "χ² = {:.1} over {} degrees of freedom (limit {:.1}); furthest:",
            self.chi2, self.dof, self.limit
        )?;
        let end = |x: f64| {
            if x.is_infinite() {
                "∞".to_string()
            } else {
                x.to_string()
            }
        };
        for ((r0, r1), (z0, z1), seen, expected) in &self.furthest {
            write!(
                f,
                " R {r0}–{} kpc, |z| {z0}–{} kpc: {seen} for {expected:.0} ({:+.1}σ);",
                end(*r1),
                end(*z1),
                (*seen as f64 - expected) / expected.sqrt()
            )?;
        }
        Ok(())
    }
}

/// How far a star of the swarm spreads its mass when it's drawn (step
/// 7.4), m: the size of the volume its mass fills at the model's density
/// there, (m/ρ★)^(1/3), ρ★ the density of `components` at `position`
/// (galactocentric), for a star of mass `mass` (kg). No more than the
/// widest disk's scale length: far out, where the model's stars thin out to
/// nothing, a star would otherwise spread across the galaxy.
pub fn spread(components: &[Component], mass: f64, position: DVec3) -> f64 {
    let (r, z) = (position.x.hypot(position.y), position.z);
    let density: f64 = components.iter().map(|c| c.density(r, z)).sum();
    let widest = components
        .iter()
        .filter_map(|c| match c {
            Component::Disk(d) => Some(d.scale_length),
            Component::Spheroid(_) => None,
        })
        .fold(0.0, f64::max);
    if density <= 0.0 {
        return widest;
    }
    (mass / density).cbrt().min(widest)
}

/// A star's energy per unit mass in `potential`, J/kg.
pub fn energy(star: &Star, potential: &Potential) -> f64 {
    let x = star.position;
    0.5 * star.velocity.length_squared() + potential.potential(x.x.hypot(x.y), x.z)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::milky_way::{mcmillan_2017, milky_way};

    #[test]
    fn a_star_spreads_over_the_volume_its_mass_fills() {
        let components = stellar_components(&mcmillan_2017());
        let suns = 1.819e6 * crate::constants::GM_SUN / crate::constants::G;
        let pc = crate::constants::PARSEC;
        // At the Sun, about 0.06 Suns per cubic parsec: 1.8 million Suns
        // fill a cube 310 pc across.
        let here = DVec3::new(8.2 * KILOPARSEC, 0.0, 20.0 * pc);
        let h = spread(&components, suns, here);
        let density: f64 = components
            .iter()
            .map(|c| c.density(8.2 * KILOPARSEC, 20.0 * pc))
            .sum();
        assert!((h.powi(3) * density / suns - 1.0).abs() < 1e-12);
        assert!((300.0 * pc..330.0 * pc).contains(&h), "{}", h / pc);
        // Far out, the thick disk's scale length at most.
        let far = DVec3::new(0.0, 60.0 * KILOPARSEC, 20.0 * KILOPARSEC);
        assert!((spread(&components, suns, far) / KILOPARSEC - 3.02134).abs() < 1e-9);
    }

    #[test]
    fn the_random_numbers_are_uniform_and_normal() {
        let mut random = Random::new(7);
        let n = 200_000;
        let (mut sum, mut sum2, mut gsum, mut gsum2) = (0.0, 0.0, 0.0, 0.0);
        for _ in 0..n {
            let u = random.uniform();
            assert!(u > 0.0 && u <= 1.0);
            sum += u;
            sum2 += u * u;
            let g = random.gaussian();
            gsum += g;
            gsum2 += g * g;
        }
        let n = n as f64;
        // Means and variances within five of their standard errors.
        assert!((sum / n - 0.5).abs() < 5.0 * (1.0 / 12.0 / n).sqrt());
        assert!((sum2 / n - 1.0 / 3.0).abs() < 5.0 * 0.3 / n.sqrt());
        assert!((gsum / n).abs() < 5.0 / n.sqrt());
        assert!((gsum2 / n - 1.0).abs() < 5.0 * 2f64.sqrt() / n.sqrt());
    }

    #[test]
    fn the_stellar_components_are_the_bulge_and_the_star_disks() {
        let names: Vec<&str> = stellar_components(&mcmillan_2017())
            .iter()
            .map(Component::name)
            .collect();
        assert_eq!(names, ["thin disk", "thick disk", "bulge"]);
    }

    #[test]
    fn in_the_plane_at_the_sun_the_thin_disk_turns_near_the_circular_speed() {
        // In the plane of the thin disk at the Sun, ⟨v_φ²⟩ is the circular
        // speed squared less the asymmetric drift's terms, so close to it
        // and below it; and σ there is tens of km/s.
        let potential = milky_way();
        let sampler = Sampler::new(potential, stellar_components(&mcmillan_2017()));
        let r = 8.2 * KILOPARSEC;
        let (sigma2, vphi2) = sampler.moments(0, r, 0.0);
        let vc = potential.circular_speed(r);
        println!(
            "thin disk at 8.2 kpc: σ = {:.1} km/s, √⟨v_φ²⟩ = {:.1} km/s, v_c = {:.1} km/s",
            sigma2.sqrt() / 1e3,
            vphi2.sqrt() / 1e3,
            vc / 1e3
        );
        assert!(vphi2.sqrt() < vc && vphi2.sqrt() > 0.9 * vc);
        assert!(sigma2.sqrt() > 5e3 && sigma2.sqrt() < 60e3);
    }

    #[test]
    fn a_star_keeps_its_energy() {
        // An eccentric orbit through the disk, for 20 dynamical times at the
        // Sun: leapfrog keeps the energy to its steps' error, about
        // (2π × 0.02)² = 1.6 × 10⁻² of the kinetic energy at worst within an
        // orbit, with no drift.
        let potential = milky_way();
        let r = 8.2 * KILOPARSEC;
        let vc = potential.circular_speed(r);
        let mut star = Star {
            position: DVec3::new(-r, 0.0, 0.1 * KILOPARSEC),
            velocity: DVec3::new(40e3, 0.8 * vc, 20e3),
            population: 0,
        };
        let start = energy(&star, potential);
        let year = 365.25 * 86400.0;
        for _ in 0..40 {
            advance(std::slice::from_mut(&mut star), potential, &[], 10e6 * year);
        }
        let drift = (energy(&star, potential) - start).abs() / (0.5 * vc * vc);
        println!("energy after 400 million years: off by {drift:.1e} of ½v_c²");
        assert!(drift < 1e-3);
    }
}
