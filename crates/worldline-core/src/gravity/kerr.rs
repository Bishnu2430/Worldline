//! Motion near a black hole in its exact spacetime: the Kerr solution
//! (Kerr 1963), for a hole of any spin, in Kerr–Schild coordinates. See
//! `docs/physics/kerr.md`.
//!
//! Kerr–Schild coordinates are Cartesian, like the rest of the engine, and
//! stay regular at the horizon and the poles: a body can circle a hole at
//! any inclination and plunge through its horizon without the equations
//! blowing up. The metric is g = η + f l⊗l (Kerr & Schild 1965; Visser
//! 2007, arXiv:0706.0622, section "Kerr–Schild Cartesian coordinates"),
//! with the spin along +z:
//! - r, the Boyer–Lindquist radius, from (x² + y²)/(r² + a²) + z²/r² = 1;
//! - f = 2 M r³ / (r⁴ + a² z²);
//! - l = (1, (r x + a y)/(r² + a²), (r y − a x)/(r² + a²), z/r);
//!
//! with M = GM/c² and a the spin times M. Far away, f → 2GM/(rc²) and the
//! motion becomes Newton's.

use std::ops::{Add, Div, Mul, Neg, Sub};

use glam::DVec3;

use crate::constants::C;

/// A number and its rate of change along three directions: forward-mode
/// automatic differentiation, so the metric's derivatives (and with them
/// the Christoffel symbols) come out exactly, without being written by
/// hand.
#[derive(Debug, Clone, Copy)]
struct Dual {
    value: f64,
    slope: [f64; 3],
}

impl Dual {
    fn constant(value: f64) -> Self {
        Self {
            value,
            slope: [0.0; 3],
        }
    }

    /// The `k`th of three independent variables, at `value`.
    fn variable(value: f64, k: usize) -> Self {
        let mut slope = [0.0; 3];
        slope[k] = 1.0;
        Self { value, slope }
    }

    fn map(self, value: f64, derivative: f64) -> Self {
        Self {
            value,
            slope: self.slope.map(|s| s * derivative),
        }
    }

    fn sqrt(self) -> Self {
        let root = self.value.sqrt();
        self.map(root, 0.5 / root)
    }

    /// The derivative along `u`: Σ uᵢ ∂ᵢ.
    fn along(self, u: DVec3) -> f64 {
        self.slope[0] * u.x + self.slope[1] * u.y + self.slope[2] * u.z
    }
}

impl Add for Dual {
    type Output = Self;
    fn add(self, o: Self) -> Self {
        Self {
            value: self.value + o.value,
            slope: [0, 1, 2].map(|k| self.slope[k] + o.slope[k]),
        }
    }
}

impl Sub for Dual {
    type Output = Self;
    fn sub(self, o: Self) -> Self {
        Self {
            value: self.value - o.value,
            slope: [0, 1, 2].map(|k| self.slope[k] - o.slope[k]),
        }
    }
}

// The product rule: (uv)' = u'v + uv'.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Mul for Dual {
    type Output = Self;
    fn mul(self, o: Self) -> Self {
        Self {
            value: self.value * o.value,
            slope: [0, 1, 2].map(|k| self.slope[k] * o.value + self.value * o.slope[k]),
        }
    }
}

// The quotient rule: (u/v)' = (u' − (u/v) v')/v.
#[allow(clippy::suspicious_arithmetic_impl)]
impl Div for Dual {
    type Output = Self;
    fn div(self, o: Self) -> Self {
        let value = self.value / o.value;
        Self {
            value,
            slope: [0, 1, 2].map(|k| (self.slope[k] - value * o.slope[k]) / o.value),
        }
    }
}

impl Neg for Dual {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            value: -self.value,
            slope: self.slope.map(|s| -s),
        }
    }
}

impl Mul<Dual> for f64 {
    type Output = Dual;
    fn mul(self, o: Dual) -> Dual {
        Dual {
            value: self * o.value,
            slope: o.slope.map(|s| self * s),
        }
    }
}

/// The Kerr–Schild functions at `x` (m), for a hole of mass `m` = GM/c²
/// and Kerr parameter `a` (both m): (f, l₀, l₁, l₂, l₃), with their
/// gradients.
fn field(x: DVec3, m: f64, a: f64) -> (Dual, [Dual; 4]) {
    let (px, py, pz) = (
        Dual::variable(x.x, 0),
        Dual::variable(x.y, 1),
        Dual::variable(x.z, 2),
    );
    let a2 = Dual::constant(a * a);
    let w = px * px + py * py + pz * pz - a2;
    let r2 = 0.5 * w + (0.25 * w * w + a2 * pz * pz).sqrt();
    let r = r2.sqrt();
    let f = 2.0 * m * r * r2 / (r2 * r2 + a2 * pz * pz);
    let ra = r2 + a2;
    let ac = Dual::constant(a);
    let l = [
        Dual::constant(1.0),
        (r * px + ac * py) / ra,
        (r * py - ac * px) / ra,
        pz / r,
    ];
    (f, l)
}

/// The acceleration (m/s², in coordinate time) of a body moving freely in
/// the spacetime of a black hole of gravitational parameter `gm` (m³/s²)
/// and spin `spin` (cJ/(GM²), 0 to 1, about +z), at position `x` (m) and
/// velocity `v` (m/s) relative to it in Kerr–Schild coordinates: the
/// geodesic equation, d²xⁱ/dt² = −Γⁱ_αβ uᵅuᵝ + Γ⁰_αβ uᵅuᵝ dxⁱ/dt, with
/// u = (1, v/c).
///
/// With g = η + f l⊗l the contraction Γ_σαβ uᵅuᵝ is
/// (u·∂)(f l_σ (l·u)) − ½ ∂_σ(f (l·u)²), and the inverse metric is
/// η − f l l with l's index raised by η, so no matrix is inverted.
pub fn geodesic_acceleration(gm: f64, spin: f64, x: DVec3, v: DVec3) -> DVec3 {
    let m = gm / (C * C);
    let (f, l) = field(x, m, spin * m);
    let u = v / C;
    let up = [1.0, u.x, u.y, u.z];
    // l·u and its derivatives (u held fixed).
    let lu = (0..4).fold(Dual::constant(0.0), |s, k| s + up[k] * l[k]);
    let along = |d: Dual| d.along(u);
    // Γ_σαβ uᵅuᵝ for σ = t, x, y, z.
    let first = |s: usize| {
        let directional = along(f) * l[s].value * lu.value
            + f.value * along(l[s]) * lu.value
            + f.value * l[s].value * along(lu);
        let gradient = if s == 0 {
            0.0
        } else {
            let k = s - 1;
            0.5 * (f.slope[k] * lu.value * lu.value + 2.0 * f.value * lu.value * lu.slope[k])
        };
        directional - gradient
    };
    let lower = [first(0), first(1), first(2), first(3)];
    // Raise with g^λσ = η^λσ − f l^λ l^σ, where l^λ = (−l₀, l₁, l₂, l₃).
    let eta = [-1.0, 1.0, 1.0, 1.0];
    let raised: Vec<f64> = (0..4).map(|k| eta[k] * l[k].value).collect();
    let l_dot = (0..4).map(|k| raised[k] * lower[k]).sum::<f64>();
    let contracted = |k: usize| eta[k] * lower[k] - f.value * raised[k] * l_dot;
    let g0 = contracted(0);
    let acceleration = DVec3::new(
        -contracted(1) + g0 * u.x,
        -contracted(2) + g0 * u.y,
        -contracted(3) + g0 * u.z,
    );
    acceleration * (C * C)
}

/// The Boyer–Lindquist radius r of a point `x` (m) near a hole of
/// gravitational parameter `gm` and spin `spin`, in m: the radius the
/// horizon and the innermost stable orbit are given in. It equals |x| for
/// a hole that doesn't spin.
pub fn boyer_lindquist_radius(gm: f64, spin: f64, x: DVec3) -> f64 {
    let a = spin * gm / (C * C);
    let w = x.length_squared() - a * a;
    (0.5 * w + (0.25 * w * w + a * a * x.z * x.z).sqrt()).sqrt()
}

/// The energy per unit rest energy and the angular momentum about the spin
/// axis per unit mass (m²/s) of a body at `x` moving at `v`: E = −u_t and
/// L = u_φ = x u_y − y u_x (times c), which a free body keeps. With
/// u^t = 1/√(−g_μν Uᵘ Uᵛ), U = (1, v/c).
pub fn energy_and_angular_momentum(gm: f64, spin: f64, x: DVec3, v: DVec3) -> (f64, f64) {
    let m = gm / (C * C);
    let (f, l) = field(x, m, spin * m);
    let u = v / C;
    let up = [1.0, u.x, u.y, u.z];
    let eta = [-1.0, 1.0, 1.0, 1.0];
    let lu: f64 = (0..4).map(|k| l[k].value * up[k]).sum();
    // g_μν Uᵛ = η_μν Uᵛ + f l_μ (l·U).
    let lowered: Vec<f64> = (0..4)
        .map(|k| eta[k] * up[k] + f.value * l[k].value * lu)
        .collect();
    let norm: f64 = (0..4).map(|k| lowered[k] * up[k]).sum();
    let ut = 1.0 / (-norm).sqrt();
    let energy = -lowered[0] * ut;
    let angular = (x.x * lowered[2] - x.y * lowered[1]) * ut * C;
    (energy, angular)
}

/// The radius of the innermost stable circular orbit in a hole's equatorial
/// plane, in units of GM/c², for spin `spin` (0 to 1), turning with the
/// hole (`prograde`) or against it: Bardeen, Press & Teukolsky (1972),
/// *ApJ* 178, 347, eq. 2.21. 6 for a hole that doesn't spin; 1 and 9 for
/// one spinning as fast as possible.
pub fn innermost_stable_orbit(spin: f64, prograde: bool) -> f64 {
    let a = spin;
    let z1 = 1.0 + (1.0 - a * a).cbrt() * ((1.0 + a).cbrt() + (1.0 - a).cbrt());
    let z2 = (3.0 * a * a + z1 * z1).sqrt();
    let root = ((3.0 - z1) * (3.0 + z1 + 2.0 * z2)).sqrt();
    if prograde {
        3.0 + z2 - root
    } else {
        3.0 + z2 + root
    }
}

/// How long a body on a circular orbit of radius `r` (m) around a black
/// hole that doesn't spin takes to shrink to the innermost stable orbit,
/// where it plunges, losing energy at the leading-order (quadrupole) rate,
/// in s; `gm` is the pair's G(M + m) and `nu` = m M/(m + M)².
///
/// The orbit shrinks at dr/dt = −(64/5) ν c (GM/rc²)³ (1 − 2GM/rc²)/(1 − 6GM/rc²):
/// the leading-order radiated power, (32/5) ν² c⁵/G (GM/rc²)⁵, set against
/// how the energy of circular orbits in the hole's spacetime changes with
/// r, E = (1 − 2GM/rc²)/√(1 − 3GM/rc²). Integrated, with x = rc²/GM, the
/// time is (5/64ν) (GM/c³) [F(x) − F(6)], where
/// F(x) = x⁴/4 − 4x³/3 − 4x² − 16x − 32 ln(x − 2). Far out it becomes
/// Peters' (5/256) c⁵ r⁴/(G³ M² m); near the innermost orbit the runaway
/// (1 − 6GM/rc²) makes it much shorter. `None` inside 6 GM/c².
pub fn plunge_time(gm: f64, nu: f64, r: f64) -> Option<f64> {
    let x = r * C * C / gm;
    if x <= 6.0 || nu <= 0.0 {
        return None;
    }
    let f = |x: f64| {
        x.powi(4) / 4.0 - 4.0 * x.powi(3) / 3.0 - 4.0 * x * x - 16.0 * x - 32.0 * (x - 2.0).ln()
    };
    Some(5.0 / (64.0 * nu) * gm / C.powi(3) * (f(x) - f(6.0)))
}

/// A circular orbit of Boyer–Lindquist radius `r` (m) in the equatorial
/// plane of a hole of gravitational parameter `gm` and spin `spin`,
/// prograde (counterclockwise seen from +z, with the hole's spin) or
/// retrograde: its position and velocity relative to the hole, starting on
/// the +x side, in Kerr–Schild coordinates. In units of GM/c² and c, the
/// angular speed is Ω = ±1/(r^(3/2) ± a) (Bardeen, Press & Teukolsky
/// 1972), the same in Kerr–Schild time as in Boyer–Lindquist
/// time, since the two differ by a function of r alone.
pub fn circular_orbit(gm: f64, spin: f64, r: f64, prograde: bool) -> (DVec3, DVec3) {
    let m = gm / (C * C);
    let a = spin * m;
    // In units of M (and M/c for time).
    let rm = r / m;
    let sign = if prograde { 1.0 } else { -1.0 };
    let omega = sign / (rm.powf(1.5) + sign * spin) * C / m;
    // On the equator x + iy = (r + ia) e^{iφ}, turning at Ω.
    let position = DVec3::new(r, a, 0.0);
    let velocity = DVec3::new(-a * omega, r * omega, 0.0);
    (position, velocity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GM_SUN;

    #[test]
    fn the_innermost_stable_orbit_spans_one_to_nine() {
        // Bardeen, Press & Teukolsky's limits: 6 GM/c² without spin; with
        // the fastest spin, 1 GM/c² turning with it and 9 against it.
        assert_eq!(innermost_stable_orbit(0.0, true), 6.0);
        assert_eq!(innermost_stable_orbit(0.0, false), 6.0);
        assert!((innermost_stable_orbit(1.0, true) - 1.0).abs() < 1e-12);
        assert!((innermost_stable_orbit(1.0, false) - 9.0).abs() < 1e-12);
    }

    #[test]
    fn far_out_the_plunge_time_is_peters() {
        // At 10⁴ GM/c², relativity's corrections to the inspiral time are
        // of order GM/rc² = 10⁻⁴ (−16/(3x) at first); Peters' leading-order
        // time, (5/256) c⁵ r⁴/(G³ M² m), must hold to below the midpoint
        // between that and order 1, √(GM/rc²) = 10⁻².
        let (gm, nu) = (1e6 * GM_SUN, 1e-3);
        let r = 1e4 * gm / (C * C);
        let peters = 5.0 / 256.0 * C.powi(5) * r.powi(4) / (gm.powi(3) * nu);
        let ratio = plunge_time(gm, nu, r).expect("far out") / peters;
        println!("at 10⁴ GM/c²: plunge time / Peters = {ratio:.6}");
        assert!((ratio - 1.0).abs() < 1e-2);
        assert_eq!(plunge_time(gm, nu, 5.0 * gm / (C * C)), None);
    }
}
