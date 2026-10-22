//! The Milky Way's gravity, as a smooth mass model: McMillan's (2017) best
//! fit, an axisymmetric bulge, thin and thick stellar disks, atomic (H I)
//! and molecular (H₂) gas disks and a dark-matter halo. Its stars aren't
//! simulated or drawn one by one: the model is their averaged mass.
//!
//! Its potential is found as McMillan's own code (GalPot) finds it,
//! following Dehnen & Binney (1998). A disk's thin layer would need
//! hundreds of multipoles, so each disk ρ = Σ(R) h(z) gets a potential
//! written down directly, 4πG Σ(r) H(z) with r the spherical radius and
//! H″ = h. What that misses is smooth: it joins the bulge and halo in a
//! multipole expansion, integrated on a grid in radius.
//!
//! Sources:
//! - McMillan (2017), *MNRAS* 465, 76, and its best-fitting model's
//!   parameters, `pot/PJM16_best.Tpot`, published with the paper's code at
//!   github.com/PaulMcMillan-Astro/GalPot.
//! - Dehnen & Binney (1998), *MNRAS* 294, 429, section 2.3.
//!
//! See `docs/physics/milky-way.md`.

use std::f64::consts::{LN_2, PI};
use std::sync::OnceLock;

use glam::DVec3;

use crate::constants::{G, PARSEC, SOLAR_MASS};
use crate::gravity::Gravity;
use crate::integrator::{Ias15, Integrator};
use crate::{Body, System};

/// A kiloparsec, in m.
pub const KILOPARSEC: f64 = 1e3 * PARSEC;

/// A disk, ρ(R, z) = Σ(R) h(z): surface density Σ(R) = Σ₀ exp(−R_m/R − R/R_d),
/// with a central hole of scale R_m (none if 0), and a vertical profile of
/// scale height z_d, exponential, exp(−|z|/z_d)/(2z_d), or isothermal,
/// sech²(z/2z_d)/(4z_d) (McMillan 2017, eqs. 3 and 4).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Disk {
    /// What it is ("thin disk").
    pub name: &'static str,
    /// Σ₀, in kg/m².
    pub surface_density: f64,
    /// R_d, in m.
    pub scale_length: f64,
    /// z_d, in m.
    pub scale_height: f64,
    /// Whether the vertical profile is sech² (the gas) rather than
    /// exponential.
    pub isothermal: bool,
    /// R_m, the central hole's scale, in m: 0 for none.
    pub hole: f64,
}

impl Disk {
    /// Σ(R) and its first two derivatives, at cylindrical radius `r` (m).
    fn surface(&self, r: f64) -> (f64, f64, f64) {
        let inv = 1.0 / self.scale_length;
        if self.hole <= 0.0 {
            let s = self.surface_density * (-r * inv).exp();
            return (s, -s * inv, s * inv * inv);
        }
        if r <= 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let s = self.surface_density * (-self.hole / r - r * inv).exp();
        let d1 = self.hole / (r * r) - inv;
        let d2 = -2.0 * self.hole / (r * r * r);
        (s, s * d1, s * (d1 * d1 + d2))
    }

    /// Its surface density at cylindrical radius `r` (m), in kg/m².
    pub fn surface_density_at(&self, r: f64) -> f64 {
        self.surface(r).0
    }

    /// The vertical profile h(z) (1/m, integrating to 1), and H(z) and
    /// H′(z), its double integral from the plane: H″ = h, H(0) = H′(0) = 0.
    fn vertical(&self, z: f64) -> (f64, f64, f64) {
        let zd = self.scale_height;
        let x = z.abs() / zd;
        if self.isothermal {
            let y = 0.5 * x;
            let sech = 1.0 / y.cosh();
            // ln cosh y, without overflow far from the plane.
            let ln_cosh = y + (-2.0 * y).exp().ln_1p() - LN_2;
            (
                sech * sech / (4.0 * zd),
                zd * ln_cosh,
                0.5 * y.tanh() * z.signum(),
            )
        } else {
            let e = (-x).exp();
            (
                e / (2.0 * zd),
                0.5 * zd * ((-x).exp_m1() + x),
                0.5 * (1.0 - e) * z.signum(),
            )
        }
    }

    /// Its density at cylindrical radius `r` and height `z` (m), in kg/m³.
    pub fn density(&self, r: f64, z: f64) -> f64 {
        self.surface(r).0 * self.vertical(z).0
    }

    /// What the multipole expansion has to take: the density less what the
    /// analytic potential 4πG Σ(s) H(z) accounts for (s the spherical
    /// radius): ρ − ∇²[Σ(s)H(z)] = [Σ(R) − Σ(s)] h − Σ″(s) H − 2Σ′(s)(H +
    /// zH′)/s. Smooth where ρ is thin.
    fn residual(&self, r: f64, z: f64) -> f64 {
        let s = r.hypot(z);
        let (h, big_h, dh) = self.vertical(z);
        let (sigma, d1, d2) = self.surface(s);
        let mut rho = (self.surface(r).0 - sigma) * h - d2 * big_h;
        if s > 0.0 {
            rho -= 2.0 * d1 * (big_h + z * dh) / s;
        }
        rho
    }

    /// The analytic part of its potential, 4πG Σ(s) H(z), and that part's
    /// derivatives along R and z.
    fn analytic(&self, r: f64, z: f64) -> (f64, f64, f64) {
        let s = r.hypot(z);
        if s == 0.0 {
            return (0.0, 0.0, 0.0);
        }
        let (_, big_h, dh) = self.vertical(z);
        let (sigma, d1, _) = self.surface(s);
        let k = 4.0 * PI * G;
        (
            k * sigma * big_h,
            k * d1 * r / s * big_h,
            k * (d1 * z / s * big_h + sigma * dh),
        )
    }

    /// Its whole mass, 2π∫Σ(R) R dR, in kg.
    pub fn mass(&self) -> f64 {
        // On ln R, where the integrand is a smooth bump.
        let lo = (1e-4 * self.scale_length).ln();
        let hi = (100.0 * self.scale_length.max(self.hole)).ln();
        let intervals = 2000;
        let step = (hi - lo) / intervals as f64;
        let rule = gauss_legendre(4);
        let mut total = 0.0;
        for i in 0..intervals {
            for &(x, w) in &rule {
                let u = lo + step * (i as f64 + 0.5 * (1.0 + x));
                let r = u.exp();
                total += 0.5 * step * w * self.surface(r).0 * r * r;
            }
        }
        2.0 * PI * total
    }
}

/// A spheroid, ρ = ρ₀ x^(−γ) (1 + x)^(γ−β) exp(−(m/r_cut)²), with
/// m² = R² + z²/q² and x = m/r₀: the bulge (McMillan 2017, eq. 1) and the
/// dark-matter halo (eq. 5).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spheroid {
    /// What it is ("bulge").
    pub name: &'static str,
    /// ρ₀, in kg/m³.
    pub density: f64,
    /// q, the ratio of its vertical to radial size: 1 for a sphere.
    pub axis_ratio: f64,
    /// γ, how steeply the density rises toward the center.
    pub inner_slope: f64,
    /// β, how steeply it falls far out.
    pub outer_slope: f64,
    /// r₀, the scale radius, in m.
    pub scale: f64,
    /// r_cut, where the density is cut off, in m; `None` for no cutoff.
    pub cutoff: Option<f64>,
}

impl Spheroid {
    /// Its density at cylindrical radius `r` and height `z` (m), in kg/m³.
    pub fn density_at(&self, r: f64, z: f64) -> f64 {
        let m = r.hypot(z / self.axis_ratio);
        let x = m / self.scale;
        let mut rho = self.density
            * x.powf(-self.inner_slope)
            * (1.0 + x).powf(self.inner_slope - self.outer_slope);
        if let Some(cut) = self.cutoff {
            rho *= (-(m / cut).powi(2)).exp();
        }
        rho
    }

    /// Its mass within the spheroidal radius `m` (m), in kg: 4πq∫ρ m² dm.
    pub fn mass_within(&self, m: f64) -> f64 {
        let lo = (1e-6 * self.scale).ln();
        let intervals = 4000;
        let step = (m.ln() - lo) / intervals as f64;
        let rule = gauss_legendre(4);
        let mut total = 0.0;
        for i in 0..intervals {
            for &(x, w) in &rule {
                let s = (lo + step * (i as f64 + 0.5 * (1.0 + x))).exp();
                total += 0.5 * step * w * self.density_at(s, 0.0) * s.powi(3);
            }
        }
        4.0 * PI * self.axis_ratio * total
    }
}

/// A galaxy's mass: its disks and spheroids.
#[derive(Debug, Clone, PartialEq)]
pub struct MassModel {
    /// Its disks, ρ = Σ(R) h(z).
    pub disks: Vec<Disk>,
    /// Its bulge and halo.
    pub spheroids: Vec<Spheroid>,
}

impl MassModel {
    /// Density at cylindrical radius `r` and height `z` (m), in kg/m³.
    pub fn density(&self, r: f64, z: f64) -> f64 {
        self.disks.iter().map(|d| d.density(r, z)).sum::<f64>()
            + self
                .spheroids
                .iter()
                .map(|s| s.density_at(r, z))
                .sum::<f64>()
    }

    /// The density the multipole expansion takes: the spheroids, and what
    /// the disks' analytic parts leave over.
    fn expanded(&self, r: f64, z: f64) -> f64 {
        self.disks.iter().map(|d| d.residual(r, z)).sum::<f64>()
            + self
                .spheroids
                .iter()
                .map(|s| s.density_at(r, z))
                .sum::<f64>()
    }
}

/// McMillan's (2017) best-fitting Milky Way, with the full-precision
/// parameters of `PJM16_best.Tpot` (its Table 3 rounds them to three
/// figures).
pub fn mcmillan_2017() -> MassModel {
    let msun_per_kpc2 = SOLAR_MASS / (KILOPARSEC * KILOPARSEC);
    let msun_per_kpc3 = msun_per_kpc2 / KILOPARSEC;
    let disk = |name, sigma: f64, rd: f64, zd: f64, isothermal, hole: f64| Disk {
        name,
        surface_density: sigma * msun_per_kpc2,
        scale_length: rd * KILOPARSEC,
        scale_height: zd * KILOPARSEC,
        isothermal,
        hole: hole * KILOPARSEC,
    };
    MassModel {
        disks: vec![
            disk("thin disk", 8.95679e8, 2.49955, 0.3, false, 0.0),
            disk("thick disk", 1.83444e8, 3.02134, 0.9, false, 0.0),
            disk("atomic gas (H I) disk", 5.31319e7, 7.0, 0.085, true, 4.0),
            disk("molecular gas (H₂) disk", 2.17995e9, 1.5, 0.045, true, 12.0),
        ],
        spheroids: vec![
            Spheroid {
                name: "bulge",
                density: 9.8351e10 * msun_per_kpc3,
                axis_ratio: 0.5,
                inner_slope: 0.0,
                outer_slope: 1.8,
                scale: 0.075 * KILOPARSEC,
                cutoff: Some(2.1 * KILOPARSEC),
            },
            Spheroid {
                name: "dark-matter halo",
                density: 8.53702e6 * msun_per_kpc3,
                axis_ratio: 1.0,
                inner_slope: 1.0,
                outer_slope: 3.0,
                scale: 19.5725 * KILOPARSEC,
                cutoff: None,
            },
        ],
    }
}

/// How finely a [`Potential`] is solved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Resolution {
    /// The highest multipole kept (even: the galaxy is symmetric about its
    /// plane).
    pub l_max: usize,
    /// Radial grid nodes per factor of ten in radius.
    pub per_decade: usize,
}

impl Default for Resolution {
    /// GalPot's own number of multipoles, 80 (thin gas disks need them),
    /// on a grid finer than GalPot's: doubling either changes the force at
    /// the Sun by under 10⁻⁵. See `docs/physics/milky-way.md`.
    fn default() -> Self {
        Self {
            l_max: 80,
            per_decade: 100,
        }
    }
}

/// The most multipoles a [`Potential`] keeps.
const MAX_L: usize = 128;

/// The radial grid runs from a parsec, well inside anything the model is
/// meant for, to a gigaparsec. Out there only the halo is left, falling as
/// a power law whose slope still steepens by 2r_s/r: extrapolating it from
/// the last node misjudges the depth inside by about (r_s/r)², here 10⁻⁹.
const GRID_INNER: f64 = 1e-3 * KILOPARSEC;
const GRID_OUTER: f64 = 1e6 * KILOPARSEC;

/// Points of the composite rule for cos θ on [0, 1]: crowded near the
/// plane, where the disks' leftovers are thin.
const ANGULAR_PIECES: [(f64, f64, usize); 3] = [(0.0, 0.02, 16), (0.02, 0.15, 24), (0.15, 1.0, 48)];

/// A galaxy's potential, solved: its disks' analytic parts, and the
/// multipoles of the rest on a grid in ln r. Between nodes, each multipole
/// is a cubic in ln r through its values and exact slopes, and the forces
/// are the exact gradient of that potential, so orbits conserve energy.
#[derive(Debug, Clone)]
pub struct Potential {
    disks: Vec<Disk>,
    l_max: usize,
    /// ln r at the first node, and the spacing.
    u_first: f64,
    step: f64,
    /// For each multipole l = 2k: at each node, f_l = a_l + b_l, where the
    /// l-th term of the potential is −4πG f_l P_l(cos θ)/(2l + 1), and its
    /// slope df_l/d ln r.
    values: Vec<Vec<f64>>,
    slopes: Vec<Vec<f64>>,
}

impl Potential {
    /// Solves `model`'s potential at the default resolution.
    pub fn new(model: &MassModel) -> Self {
        Self::with_resolution(model, Resolution::default())
    }

    /// Solves `model`'s potential at `resolution`.
    ///
    /// With ρ_l(r) = (2l + 1) ∫₀¹ ρ(r, μ) P_l(μ) dμ (μ = cos θ), the l-th
    /// term is −4πG P_l(μ)/(2l + 1) [a_l(r) + b_l(r)], where
    /// a_l = r^(−l−1) ∫₀^r ρ_l r′^(l+2) dr′ and b_l = r^l ∫_r^∞ ρ_l r′^(1−l) dr′.
    /// Both are carried node to node in ln r (outward and inward), each
    /// interval by 4-point Gauss–Legendre, which keeps the steep powers of r
    /// exact.
    pub fn with_resolution(model: &MassModel, resolution: Resolution) -> Self {
        let l_max = resolution.l_max.min(MAX_L) & !1;
        let terms = l_max / 2 + 1;
        let step = std::f64::consts::LN_10 / resolution.per_decade as f64;
        let u_first = GRID_INNER.ln();
        let nodes = ((GRID_OUTER.ln() - u_first) / step).ceil() as usize + 1;

        // The angular rule, with every P_l at its points.
        let mut angular = Vec::new();
        for (lo, hi, n) in ANGULAR_PIECES {
            for (x, w) in gauss_legendre(n) {
                let mu = lo + 0.5 * (hi - lo) * (1.0 + x);
                // Every P_l by the recurrence; the even ones are kept.
                let mut all = vec![1.0, mu];
                for l in 1..l_max {
                    let n = l as f64;
                    all.push(((2.0 * n + 1.0) * mu * all[l] - n * all[l - 1]) / (n + 1.0));
                }
                let p: Vec<f64> = all.into_iter().step_by(2).take(terms).collect();
                angular.push((mu, 0.5 * (hi - lo) * w, p));
            }
        }
        // ρ_l r² at radius e^u, for every l.
        let multipoles = |u: f64| -> Vec<f64> {
            let r = u.exp();
            let mut out = vec![0.0; terms];
            for (mu, w, p) in &angular {
                let rho = w * model.expanded(r * (1.0 - mu * mu).sqrt(), r * mu);
                for (o, pl) in out.iter_mut().zip(p) {
                    *o += rho * pl;
                }
            }
            for (k, o) in out.iter_mut().enumerate() {
                *o *= (4 * k + 1) as f64 * r * r;
            }
            out
        };

        // Each interval's Gauss–Legendre points: how far past the interval's
        // start, and ρ_l r² there times the weight. Intervals are shared out
        // among the cores.
        let rule = gauss_legendre(4);
        let interval = |i: usize| -> Vec<(f64, Vec<f64>)> {
            rule.iter()
                .map(|&(x, w)| {
                    let s = 0.5 * step * (1.0 + x);
                    let g = multipoles(u_first + step * i as f64 + s);
                    (s, g.into_iter().map(|g| 0.5 * step * w * g).collect())
                })
                .collect()
        };
        let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
        let chunk = (nodes - 1).div_ceil(threads);
        let mut inner: Vec<Vec<(f64, Vec<f64>)>> = Vec::with_capacity(nodes - 1);
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..nodes - 1)
                .step_by(chunk)
                .map(|first| {
                    let interval = &interval;
                    scope.spawn(move || {
                        (first..(first + chunk).min(nodes - 1))
                            .map(interval)
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for handle in handles {
                inner.extend(handle.join().expect("a solving thread finished"));
            }
        });

        let mut a = vec![vec![0.0; nodes]; terms];
        let mut b = vec![vec![0.0; nodes]; terms];
        // Inside the first node only the halo's cusp matters (the l = 0
        // term): a power law, ρ r² ∝ r^q, gives a₀ = ρ₀r²/(q + 1).
        let first = multipoles(u_first);
        let second = multipoles(u_first + step);
        let q = (second[0] / first[0]).ln() / step;
        if first[0] > 0.0 && q + 1.0 > 0.0 {
            a[0][0] = first[0] / (q + 1.0);
        }
        // Beyond the last node only the halo's tail is left, likewise:
        // b₀ = ρ₀r²/(−q).
        let last = multipoles(u_first + step * (nodes - 1) as f64);
        let before = multipoles(u_first + step * (nodes - 2) as f64);
        let q = (last[0] / before[0]).ln() / step;
        if last[0] > 0.0 && q < 0.0 {
            b[0][nodes - 1] = last[0] / -q;
        }
        for k in 0..terms {
            let l = (2 * k) as f64;
            for i in 0..nodes - 1 {
                let carried: f64 = inner[i]
                    .iter()
                    .map(|(s, g)| g[k] * (-(l + 1.0) * (step - s)).exp())
                    .sum();
                a[k][i + 1] = (-(l + 1.0) * step).exp() * a[k][i] + carried;
            }
            for i in (0..nodes - 1).rev() {
                let carried: f64 = inner[i].iter().map(|(s, g)| g[k] * (-l * s).exp()).sum();
                b[k][i] = (-l * step).exp() * b[k][i + 1] + carried;
            }
        }
        // df/d ln r = −(l + 1) a_l + l b_l: the density terms cancel.
        let values = (0..terms)
            .map(|k| a[k].iter().zip(&b[k]).map(|(a, b)| a + b).collect())
            .collect();
        let slopes = (0..terms)
            .map(|k| {
                let l = (2 * k) as f64;
                a[k].iter()
                    .zip(&b[k])
                    .map(|(a, b)| -(l + 1.0) * a + l * b)
                    .collect()
            })
            .collect();
        Self {
            disks: model.disks.clone(),
            l_max,
            u_first,
            step,
            values,
            slopes,
        }
    }

    /// The potential Φ (J/kg, zero far away) at cylindrical radius `r` and
    /// height `z` (m), and its derivatives ∂Φ/∂R and ∂Φ/∂z.
    pub fn evaluate(&self, r: f64, z: f64) -> (f64, f64, f64) {
        let mut total = (0.0, 0.0, 0.0);
        for disk in &self.disks {
            let (p, dr, dz) = disk.analytic(r, z);
            total.0 += p;
            total.1 += dr;
            total.2 += dz;
        }
        let s = r.hypot(z).max(GRID_INNER);
        let nodes = self.values[0].len();
        let u = s.ln();
        let i = (((u - self.u_first) / self.step).floor() as usize).min(nodes - 2);
        let t = ((u - self.u_first) / self.step - i as f64).min(1.0);
        // The cubic Hermite basis and its slopes.
        let (t2, t3) = (t * t, t * t * t);
        let basis = [
            2.0 * t3 - 3.0 * t2 + 1.0,
            t3 - 2.0 * t2 + t,
            3.0 * t2 - 2.0 * t3,
            t3 - t2,
        ];
        let slope = [
            6.0 * t2 - 6.0 * t,
            3.0 * t2 - 4.0 * t + 1.0,
            6.0 * t - 6.0 * t2,
            3.0 * t2 - 2.0 * t,
        ];
        let mu = if r.hypot(z) > 0.0 { z / s } else { 0.0 };
        // P_l(μ) by the recurrence, and P′_{l+1} = P′_{l−1} + (2l + 1) P_l.
        let mut p = [0.0; MAX_L + 2];
        let mut dp = [0.0; MAX_L + 2];
        (p[0], p[1], dp[1]) = (1.0, mu, 1.0);
        for l in 1..self.l_max {
            let n = l as f64;
            p[l + 1] = ((2.0 * n + 1.0) * mu * p[l] - n * p[l - 1]) / (n + 1.0);
            dp[l + 1] = dp[l - 1] + (2.0 * n + 1.0) * p[l];
        }
        let h = self.step;
        let (mut phi, mut d_u, mut d_mu) = (0.0, 0.0, 0.0);
        for (k, (f, g)) in self.values.iter().zip(&self.slopes).enumerate() {
            let l = 2 * k;
            let value = basis[0] * f[i]
                + basis[1] * h * g[i]
                + basis[2] * f[i + 1]
                + basis[3] * h * g[i + 1];
            let rate = (slope[0] * f[i]
                + slope[1] * h * g[i]
                + slope[2] * f[i + 1]
                + slope[3] * h * g[i + 1])
                / h;
            let c = -4.0 * PI * G / (2 * l + 1) as f64;
            phi += c * value * p[l];
            d_u += c * rate * p[l];
            d_mu += c * value * dp[l];
        }
        total.0 += phi;
        if r.hypot(z) >= GRID_INNER {
            let s3 = s * s * s;
            total.1 += d_u * r / (s * s) - d_mu * z * r / s3;
            total.2 += d_u * z / (s * s) + d_mu * r * r / s3;
        }
        total
    }

    /// The potential at cylindrical radius `r` and height `z` (m), in J/kg.
    pub fn potential(&self, r: f64, z: f64) -> f64 {
        self.evaluate(r, z).0
    }

    /// The pull per unit mass there, (−∂Φ/∂R, −∂Φ/∂z), in m/s².
    pub fn force(&self, r: f64, z: f64) -> (f64, f64) {
        let (_, dr, dz) = self.evaluate(r, z);
        (-dr, -dz)
    }

    /// The acceleration at `position` (m, Cartesian, the galaxy's center at
    /// the origin and z toward its north pole), in m/s².
    pub fn acceleration(&self, position: DVec3) -> DVec3 {
        let r = position.x.hypot(position.y);
        let (fr, fz) = self.force(r, position.z);
        if r > 0.0 {
            DVec3::new(fr * position.x / r, fr * position.y / r, fz)
        } else {
            DVec3::new(0.0, 0.0, fz)
        }
    }

    /// The speed of a circular orbit at radius `r` (m) in the plane, in
    /// m/s.
    pub fn circular_speed(&self, r: f64) -> f64 {
        (r * -self.force(r, 0.0).0).max(0.0).sqrt()
    }
}

/// McMillan's (2017) Milky Way, solved once and shared.
pub fn milky_way() -> &'static Potential {
    static POTENTIAL: OnceLock<Potential> = OnceLock::new();
    POTENTIAL.get_or_init(|| Potential::new(&mcmillan_2017()))
}

/// Gauss–Legendre points and weights on [−1, 1], by Newton's method on
/// P_n from Tricomi's first guesses.
pub fn gauss_legendre(n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let mut x = (PI * (i as f64 + 0.75) / (n as f64 + 0.5)).cos();
            let mut dp = 1.0;
            for _ in 0..100 {
                let (mut p0, mut p1) = (1.0, x);
                for k in 2..=n {
                    let p2 = ((2 * k - 1) as f64 * x * p1 - (k - 1) as f64 * p0) / k as f64;
                    p0 = p1;
                    p1 = p2;
                }
                dp = n as f64 * (x * p1 - p0) / (x * x - 1.0);
                let dx = p1 / dp;
                x -= dx;
                if dx.abs() < 1e-15 {
                    break;
                }
            }
            (x, 2.0 / ((1.0 - x * x) * dp * dp))
        })
        .collect()
}

/// A galaxy's pull on test bodies: positions are galactocentric.
struct Field(&'static Potential);

impl Gravity for Field {
    fn name(&self) -> &'static str {
        "Milky Way mass model"
    }

    fn accelerations(&self, _time: f64, bodies: &[Body], out: &mut [DVec3]) {
        for (body, a) in bodies.iter().zip(out) {
            *a = self.0.acceleration(body.position);
        }
    }
}

/// A test body's path through a galaxy, the solar system's barycenter in
/// the app: integrated by IAS15 in galactocentric coordinates (the center
/// at the origin, z toward the north galactic pole).
#[derive(Debug, Clone)]
pub struct GalacticOrbit {
    potential: &'static Potential,
    system: System,
    integrator: Ias15,
}

impl GalacticOrbit {
    /// Starts at `position` (m) with `velocity` (m/s), at time 0.
    pub fn new(potential: &'static Potential, position: DVec3, velocity: DVec3) -> Self {
        Self {
            potential,
            system: System::new(vec![
                Body::new("barycenter", 0.0, 0.0)
                    .at(position)
                    .moving(velocity),
            ]),
            integrator: Ias15::new(),
        }
    }

    /// The same, its clock set to `time` (s): for a state saved then.
    pub fn at_time(mut self, time: f64) -> Self {
        self.system.set_time(time);
        self
    }

    /// Its clock, in s.
    pub fn time(&self) -> f64 {
        self.system.time()
    }

    /// Where it is, in m.
    pub fn position(&self) -> DVec3 {
        self.system.bodies[0].position
    }

    /// How fast it moves, in m/s.
    pub fn velocity(&self) -> DVec3 {
        self.system.bodies[0].velocity
    }

    /// Its energy per unit mass, v²/2 + Φ, in J/kg.
    pub fn energy(&self) -> f64 {
        let p = self.position();
        0.5 * self.velocity().length_squared() + self.potential.potential(p.x.hypot(p.y), p.z)
    }

    /// Carries it forward to `time` (s); never backward.
    pub fn advance_to(&mut self, time: f64) {
        let field = Field(self.potential);
        while self.system.time() < time {
            let left = time - self.system.time();
            if self.integrator.step(&mut self.system, &field, left) >= left {
                self.system.set_time(time);
            }
        }
    }

    /// Where it will be over the next `duration` (s), at `samples` evenly
    /// spaced moments from now (m).
    pub fn path(&self, duration: f64, samples: usize) -> Vec<DVec3> {
        let mut ahead = self.clone();
        let start = self.time();
        (0..=samples)
            .map(|i| {
                ahead.advance_to(start + duration * i as f64 / samples as f64);
                ahead.position()
            })
            .collect()
    }

    /// Follows a copy of it for `duration` (s), sampled every `step` (s)
    /// and refined between samples on the cubic through each pair of
    /// positions and velocities, which is good to (step/period)⁴.
    pub fn survey(&self, duration: f64, step: f64) -> Survey {
        let mut orbit = self.clone();
        // The cubic through two samples, and its rate, at fraction s.
        let hermite = |x0: DVec3, v0: DVec3, x1: DVec3, v1: DVec3, s: f64| {
            let (s2, s3) = (s * s, s * s * s);
            let x = x0 * (2.0 * s3 - 3.0 * s2 + 1.0)
                + v0 * step * (s3 - 2.0 * s2 + s)
                + x1 * (3.0 * s2 - 2.0 * s3)
                + v1 * step * (s3 - s2);
            let v = (x0 * (6.0 * s2 - 6.0 * s)
                + v0 * step * (3.0 * s2 - 4.0 * s + 1.0)
                + x1 * (6.0 * s - 6.0 * s2)
                + v1 * step * (3.0 * s2 - 2.0 * s))
                / step;
            (x, v)
        };
        // Where in [0, 1] g, which changes sign there, is zero.
        let root = |g: &dyn Fn(f64) -> f64| {
            let (mut lo, mut hi) = (0.0, 1.0);
            let below = g(0.0) < 0.0;
            for _ in 0..60 {
                let mid = 0.5 * (lo + hi);
                if (g(mid) < 0.0) == below {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        };
        // How fast the distance from the axis grows, times that distance.
        let outward = |x: DVec3, v: DVec3| x.x * v.x + x.y * v.y;
        let start = orbit.time();
        let (mut x0, mut v0) = (orbit.position(), orbit.velocity());
        let mut survey = Survey {
            r_min: x0.truncate().length(),
            r_max: x0.truncate().length(),
            z_max: x0.z.abs(),
            radial_period: None,
            vertical_period: None,
            azimuthal_period: 0.0,
        };
        let (mut pericenters, mut crossings) = (Vec::new(), Vec::new());
        let mut turned = 0.0;
        let steps = (duration / step).round().max(1.0) as usize;
        for i in 1..=steps {
            let t0 = start + (i - 1) as f64 * step;
            orbit.advance_to(start + i as f64 * step);
            let (x1, v1) = (orbit.position(), orbit.velocity());
            let at = |s: f64| hermite(x0, v0, x1, v1, s);
            if outward(x0, v0).signum() != outward(x1, v1).signum() {
                let s = root(&|s| {
                    let (x, v) = at(s);
                    outward(x, v)
                });
                let r = at(s).0.truncate().length();
                if outward(x0, v0) < 0.0 {
                    survey.r_min = survey.r_min.min(r);
                    let further = at(s).0.truncate().angle_to(x0.truncate()).abs();
                    pericenters.push((t0 + s * step, turned + further));
                } else {
                    survey.r_max = survey.r_max.max(r);
                }
            }
            if v0.z.signum() != v1.z.signum() {
                let s = root(&|s| at(s).1.z);
                survey.z_max = survey.z_max.max(at(s).0.z.abs());
            }
            if x0.z < 0.0 && x1.z >= 0.0 {
                crossings.push(t0 + root(&|s| at(s).0.z) * step);
            }
            turned += x1.truncate().angle_to(x0.truncate()).abs();
            (x0, v0) = (x1, v1);
        }
        let period = |times: &[f64]| {
            (times.len() > 1)
                .then(|| (times[times.len() - 1] - times[0]) / (times.len() - 1) as f64)
        };
        let times: Vec<f64> = pericenters.iter().map(|p| p.0).collect();
        survey.radial_period = period(&times);
        survey.vertical_period = period(&crossings);
        // The mean rate of turning, over whole radial periods where there
        // are any: the rate varies along the orbit.
        survey.azimuthal_period = std::f64::consts::TAU
            * match (pericenters.first(), pericenters.last()) {
                (Some(a), Some(b)) if b.1 > a.1 => (b.0 - a.0) / (b.1 - a.1),
                _ => duration / turned,
            };
        survey
    }
}

/// What an orbit does over a stretch of time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Survey {
    /// Its least distance from the galaxy's axis, in m.
    pub r_min: f64,
    /// Its greatest distance from the axis, in m.
    pub r_max: f64,
    /// How far it gets from the midplane, in m.
    pub z_max: f64,
    /// How long from one closest approach to the center to the next, in s,
    /// if it made two.
    pub radial_period: Option<f64>,
    /// How long from one upward crossing of the midplane to the next, in s,
    /// if it made two.
    pub vertical_period: Option<f64>,
    /// How long it takes to go once around the center, on average over
    /// whole radial periods, in s.
    pub azimuthal_period: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gauss_legendre_integrates_polynomials_exactly() {
        // n points integrate x^(2n−2) over [−1, 1], 2/(2n − 1), exactly.
        for n in [4, 16, 48] {
            let sum: f64 = gauss_legendre(n)
                .iter()
                .map(|&(x, w)| w * x.powi(2 * n as i32 - 2))
                .sum();
            assert!((sum * (2 * n - 1) as f64 / 2.0 - 1.0).abs() < 1e-13, "{n}");
        }
    }

    #[test]
    fn each_disks_profiles_are_derivatives_of_each_other() {
        // H″ = h, and Σ′ and Σ″, by central differences of step d, whose
        // error is of order (d/L)² = 10⁻⁸ of what they measure.
        for disk in mcmillan_2017().disks {
            let zd = disk.scale_height;
            for z in [0.3, 1.0, 3.0].map(|x| x * zd) {
                let d = 1e-4 * zd;
                let (h, big_h, dh) = disk.vertical(z);
                let (_, below, _) = disk.vertical(z - d);
                let (_, above, _) = disk.vertical(z + d);
                assert!(((above - below) / (2.0 * d) / dh - 1.0).abs() < 1e-7);
                assert!(((above - 2.0 * big_h + below) / (d * d) / h - 1.0).abs() < 1e-6);
            }
            let rd = disk.scale_length;
            for r in [0.5, 2.0, 5.0].map(|x| x * rd) {
                let (s, ds, d2s) = disk.surface(r);
                // Near a hole Σ changes over much less than R_d.
                let local = rd.min(r).min((s / ds).abs());
                let d = 1e-4 * local;
                let (below, ..) = disk.surface(r - d);
                let (above, ..) = disk.surface(r + d);
                assert!(((above - below) / (2.0 * d) - ds).abs() < 1e-7 * ds.abs().max(s / local));
                assert!(
                    ((above - 2.0 * s + below) / (d * d) - d2s).abs()
                        < 1e-6 * d2s.abs().max(s / (local * local))
                );
            }
        }
    }

    #[test]
    fn the_force_is_the_potentials_gradient() {
        let potential = milky_way();
        for (r, z) in [(8.2, 0.02), (1.0, 0.5), (20.0, 3.0)] {
            let (r, z) = (r * KILOPARSEC, z * KILOPARSEC);
            let d = 1e-5 * r;
            let (fr, fz) = potential.force(r, z);
            let dr = (potential.potential(r + d, z) - potential.potential(r - d, z)) / (2.0 * d);
            let dz = (potential.potential(r, z + d) - potential.potential(r, z - d)) / (2.0 * d);
            let scale = fr.hypot(fz);
            assert!((fr + dr).abs() < 1e-6 * scale && (fz + dz).abs() < 1e-6 * scale);
        }
    }
}
