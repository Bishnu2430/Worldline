//! Light near a spinning black hole: rays (null geodesics) of Kerr
//! spacetime, the shadow they outline, and how a thin disk around the hole
//! looks.
//!
//! Rays are traced in the engine's Kerr–Schild Cartesian coordinates (see
//! [`crate::gravity::kerr`]), where the metric is g = η + f l⊗l and its
//! inverse η − f l l. With p_t = −E, a ray's Hamiltonian is
//!
//! 2H = |p|² − E² − f (E + l·p)² = 0
//!
//! (l·p over the space parts), and Hamilton's equations are
//!
//! dx/dλ = p − f (E + l·p) l,  dp/dλ = ½(E + l·p)² ∇f + f (E + l·p) ∇(l·p).
//!
//! Boyer–Lindquist coordinates, where the equations separate (Carter 1968),
//! break down on the spin axis, which rays passing behind the hole cross;
//! these don't. A ray keeps its energy E, its angular momentum about the
//! spin axis L = x p_y − y p_x, and Carter's constant Q. Units: GM/c² = 1,
//! c = 1, and a the spin, 0 to 1, along +z.
//!
//! The camera is a zero-angular-momentum observer (ZAMO; Bardeen, Press &
//! Teukolsky 1972), turning with the hole's frame dragging; for a hole that
//! doesn't spin, at rest, as in [`crate::lensing`]. Rays are traced back
//! from it, here in double precision by adaptive Dormand–Prince steps.
//!
//! See `docs/physics/lensing.md`.

use std::f64::consts::PI;

use glam::{DMat3, DMat4, DVec3, DVec4};

use crate::constants::{C, PROTON_MASS, STEFAN_BOLTZMANN, THOMSON_CROSS_SECTION};
use crate::gravity::kerr::{Dual, field, innermost_stable_orbit};

/// Δ = r² − 2r + a².
fn delta(a: f64, r: f64) -> f64 {
    r * r - 2.0 * r + a * a
}

/// The Boyer–Lindquist radius of the point `x`, from
/// (x² + y²)/(r² + a²) + z²/r² = 1.
pub fn radius(a: f64, x: DVec3) -> f64 {
    let w = x.length_squared() - a * a;
    (0.5 * w + (0.25 * w * w + a * a * x.z * x.z).sqrt()).sqrt()
}

/// The outer horizon, r₊ = 1 + √(1 − a²).
pub fn horizon(a: f64) -> f64 {
    1.0 + (1.0 - a * a).sqrt()
}

/// The radii of the circular photon orbits in the equatorial plane,
/// turning with the hole and against it: 2(1 + cos(⅔ arccos(∓a)))
/// (Bardeen, Press & Teukolsky 1972, eq. 2.18).
pub fn photon_orbits(a: f64) -> (f64, f64) {
    let orbit = |sign: f64| 2.0 * (1.0 + ((2.0 / 3.0) * (sign * a).acos()).cos());
    (orbit(-1.0), orbit(1.0))
}

/// For a body circling in the equatorial plane at radius `r`, turning with
/// the hole: its angular velocity Ω = 1/(r^(3/2) + a) and u^t =
/// (r^(3/2) + a)/(r^(3/4) √(r^(3/2) − 3r^(1/2) + 2a)) (Bardeen, Press &
/// Teukolsky 1972, eqs. 2.16 and 2.12).
pub fn circular(a: f64, r: f64) -> (f64, f64) {
    let x = r.sqrt();
    let omega = 1.0 / (r * x + a);
    let ut = (r * x + a) / (r.powf(0.75) * (r * x - 3.0 * x + 2.0 * a).sqrt());
    (omega, ut)
}

/// The shift of light from a disk circling at radius `r` to an observer,
/// for a ray of energy `e` and angular momentum `l` (as the observer
/// measures energy 1): g = ν_seen/ν_sent = 1/(u^t (E − ΩL)).
pub fn disk_shift(a: f64, r: f64, e: f64, l: f64) -> f64 {
    let (omega, ut) = circular(a, r);
    1.0 / (ut * (e - omega * l))
}

/// The light a thin disk sends out from each face at radius `r` (units of
/// GM/c²), as a fraction of Ṁc²/(4π(GM/c²)²), Ṁ the rate at which it
/// feeds the hole: the relativistic thin disk of Novikov & Thorne (1973)
/// and Page & Thorne (1974), in the closed form Li (2002,
/// arXiv:astro-ph/0202361, eqs. A1–A3) gives, with x = √r, x₀ the same
/// at the innermost stable orbit and x₁, x₂, x₃ the roots of
/// x³ − 3x + 2a = 0, 2cos(⅓ arccos a ∓ π/3) and −2cos(⅓ arccos a):
///
/// F = 3/(2x⁴(x³ − 3x + 2a)) [x − x₀ − (3a/2) ln(x/x₀)
///     − Σᵢ 3(xᵢ − a)²/(xᵢ(xᵢ − xⱼ)(xᵢ − xₖ)) ln((x − xᵢ)/(x₀ − xᵢ))].
///
/// It is zero at the innermost stable orbit, where the disk ends, and far
/// out 3/(2r³), as without relativity.
pub fn disk_flux(a: f64, r: f64) -> f64 {
    let r_ms = innermost_stable_orbit(a, true);
    if r <= r_ms {
        return 0.0;
    }
    let (x, x0) = (r.sqrt(), r_ms.sqrt());
    let third = a.acos() / 3.0;
    let roots = [
        2.0 * (third - PI / 3.0).cos(),
        2.0 * (third + PI / 3.0).cos(),
        -2.0 * third.cos(),
    ];
    let mut bracket = x - x0 - 1.5 * a * (x / x0).ln();
    for i in 0..3 {
        let (xi, xj, xk) = (roots[i], roots[(i + 1) % 3], roots[(i + 2) % 3]);
        // Without spin x₂ = 0, and its term, of order a, is gone.
        if xi.abs() > 1e-12 {
            bracket -=
                3.0 * (xi - a).powi(2) / (xi * (xi - xj) * (xi - xk)) * ((x - xi) / (x0 - xi)).ln();
        }
    }
    1.5 * bracket / (x.powi(4) * (x.powi(3) - 3.0 * x + 2.0 * a))
}

/// The energy per unit rest energy of a body on a circular orbit at `r`,
/// turning with the hole: E = (1 − 2/r + a r^(−3/2))/√(1 − 3/r + 2a r^(−3/2))
/// (Bardeen, Press & Teukolsky 1972, eq. 2.12).
pub fn orbit_energy(a: f64, r: f64) -> f64 {
    let s = a * r.powf(-1.5);
    (1.0 - 2.0 / r + s) / (1.0 - 3.0 / r + 2.0 * s).sqrt()
}

/// The temperature, K, of a thin disk's faces at radius `r` (units of
/// GM/c²) around a hole of gravitational parameter `gm` (m³/s²) and spin
/// `a`, fed at `eddington` times the Eddington rate: σT⁴ = F
/// ([`disk_flux`]), with Ṁc² = L_Edd/η, L_Edd = 4πGM m_p c/σ_T and
/// η = 1 − E_ms the fraction of its rest energy matter radiates on its way
/// down to the innermost stable orbit.
pub fn disk_temperature(gm: f64, a: f64, eddington: f64, r: f64) -> f64 {
    let luminosity = 4.0 * PI * gm * PROTON_MASS * C / THOMSON_CROSS_SECTION;
    let efficiency = 1.0 - orbit_energy(a, innermost_stable_orbit(a, true));
    let feeding = eddington * luminosity / efficiency;
    let flux = feeding / (4.0 * PI * (gm / (C * C)).powi(2)) * disk_flux(a, r);
    (flux / STEFAN_BOLTZMANN).powf(0.25)
}

/// Bardeen's (1973) photon orbit of radius `r`, between the two of
/// [`photon_orbits`]: the constants of the rays that spiral toward it,
/// ξ = L/E = (r²(3 − r) − a²(r + 1))/(a(r − 1)) and
/// η = Q/E² = r³(4a² − r(r − 3)²)/(a²(r − 1)²). Needs a > 0.
pub fn spherical_orbit(a: f64, r: f64) -> (f64, f64) {
    let xi = (r * r * (3.0 - r) - a * a * (r + 1.0)) / (a * (r - 1.0));
    let eta = r.powi(3) * (4.0 * a * a - r * (r - 3.0).powi(2)) / (a * a * (r - 1.0).powi(2));
    (xi, eta)
}

/// Where a ray of constants ξ = L/E and η = Q/E² appears to an observer
/// far away at inclination `inclination` (rad from the spin axis):
/// α = −ξ/sin i across the projected spin axis and
/// β = √(η + a² cos² i − ξ² cot² i) along it, in units of GM/c² (Bardeen
/// 1973), or `None` if the ray can't reach that inclination.
pub fn seen_from_afar(a: f64, inclination: f64, xi: f64, eta: f64) -> Option<(f64, f64)> {
    let (si, ci) = inclination.sin_cos();
    let beta2 = eta + a * a * ci * ci - xi * xi * ci * ci / (si * si);
    (beta2 >= 0.0).then(|| (-xi / si, beta2.sqrt()))
}

/// The edge of the shadow for an observer far away at inclination
/// `inclination`: the rays of every [`spherical_orbit`], seen
/// [`seen_from_afar`], above the projected equator and then below. Needs
/// a > 0.
pub fn bardeen_shadow(a: f64, inclination: f64, samples: usize) -> Vec<(f64, f64)> {
    let (inner, outer) = photon_orbits(a);
    let upper: Vec<_> = (0..=samples)
        .filter_map(|k| {
            let r = inner + (outer - inner) * k as f64 / samples as f64;
            let (xi, eta) = spherical_orbit(a, r);
            seen_from_afar(a, inclination, xi, eta)
        })
        .collect();
    let lower: Vec<_> = upper.iter().rev().map(|&(x, y)| (x, -y)).collect();
    upper.into_iter().chain(lower).collect()
}

/// Carter's constant Q = p_θ² + cos²θ (L²/sin²θ − a²E²) of a ray of energy
/// `e` at `x` with momentum `p`, Kerr–Schild Cartesian. Its θ-momentum is
/// the same as Boyer–Lindquist's, p_θ = ∂x/∂θ · p, and
/// ∂x/∂θ = (x cot θ, y cot θ, −r sin θ).
pub fn carter(a: f64, x: DVec3, p: DVec3, e: f64) -> f64 {
    let r = radius(a, x);
    let (st, ct) = polar_angle(a, r, x).sin_cos();
    let ptheta = ct / st * (x.x * p.x + x.y * p.y) - r * st * p.z;
    let l = x.x * p.y - x.y * p.x;
    ptheta * ptheta + ct * ct * (l * l / (st * st) - a * a * e * e)
}

/// The Boyer–Lindquist angle from the spin axis of the point `x` at
/// radius `r`: sin θ = √(x² + y²)/√(r² + a²) and cos θ = z/r, both kept, so
/// it is exact near the axis too.
fn polar_angle(a: f64, r: f64, x: DVec3) -> f64 {
    (x.x.hypot(x.y) / (r * r + a * a).sqrt()).atan2(x.z / r)
}

/// A ray's place and momentum (covariant), Kerr–Schild Cartesian.
#[derive(Debug, Clone, Copy)]
struct State {
    x: DVec3,
    p: DVec3,
}

impl State {
    fn add(self, k: State, h: f64) -> State {
        State {
            x: self.x + h * k.x,
            p: self.p + h * k.p,
        }
    }
}

/// Hamilton's equations for a ray of energy `e`.
fn rates(a: f64, e: f64, s: State) -> State {
    let (f, l) = field(s.x, 1.0, a);
    let lp = s.p.x * l[1] + s.p.y * l[2] + s.p.z * l[3];
    let k = e + lp.value;
    let slope = |d: Dual| DVec3::from_array(d.slope);
    State {
        x: s.p - f.value * k * DVec3::new(l[1].value, l[2].value, l[3].value),
        p: 0.5 * k * k * slope(f) + f.value * k * slope(lp),
    }
}

/// 2H, which is 0 along a ray.
#[cfg(test)]
fn hamiltonian(a: f64, e: f64, s: State) -> f64 {
    let (f, l) = field(s.x, 1.0, a);
    let lp = s.p.x * l[1].value + s.p.y * l[2].value + s.p.z * l[3].value;
    s.p.length_squared() - e * e - f.value * (e + lp).powi(2)
}

/// Where the camera is and how it's turned: a zero-angular-momentum
/// observer at `place`, with its frame's axes given as world directions
/// (the hole at the origin, its spin along +z, Kerr–Schild Cartesian
/// coordinates as in [`crate::gravity::kerr`]).
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    /// The hole's spin, 0 to 1.
    pub a: f64,
    /// Where it is, in units of GM/c².
    pub place: DVec3,
    /// Its Boyer–Lindquist radius.
    pub r: f64,
    /// Its Boyer–Lindquist angle from the spin axis.
    pub theta: f64,
    /// The observer's axes along r, θ and φ, as world directions.
    pub axes: [DVec3; 3],
}

impl Camera {
    /// A camera at `place` (Kerr–Schild Cartesian, in units of GM/c²)
    /// near a hole of spin `a`.
    ///
    /// On the spin axis itself, where the frame's φ has no direction, it
    /// stands 10⁻⁹ of its distance off the axis, which no image can show.
    pub fn at(a: f64, place: DVec3) -> Self {
        let off = 1e-9 * place.length();
        let place = if place.x.hypot(place.y) < off {
            DVec3::new(off, 0.0, place.z)
        } else {
            place
        };
        let r = radius(a, place);
        let theta = polar_angle(a, r, place);
        let [along_r, along_theta, along_phi] = Self::jacobian(a, r, theta, place);
        // ∂_r at fixed Boyer–Lindquist time and φ, in world directions:
        // Kerr–Schild's φ runs ahead by ∫ a dr/Δ.
        let radial = along_r + a / delta(a, r) * along_phi;
        let e_r = radial.normalize();
        let e_theta = (along_theta - along_theta.dot(e_r) * e_r).normalize();
        let e_phi = e_r.cross(e_theta);
        Self {
            a,
            place,
            r,
            theta,
            axes: [e_r, e_theta, e_phi],
        }
    }

    /// ∂x/∂r, ∂x/∂θ and ∂x/∂φ at `x` for x + iy = (r + ia) sin θ e^(iφ),
    /// z = r cos θ: (sin θ cos φ, sin θ sin φ, cos θ), (x cot θ, y cot θ,
    /// −r sin θ) and (−y, x, 0).
    fn jacobian(a: f64, r: f64, theta: f64, x: DVec3) -> [DVec3; 3] {
        let (st, ct) = theta.sin_cos();
        let ra = r * r + a * a;
        [
            DVec3::new((r * x.x + a * x.y) / ra, (r * x.y - a * x.x) / ra, ct),
            DVec3::new(x.x * ct / st, x.y * ct / st, -r * st),
            DVec3::new(-x.y, x.x, 0.0),
        ]
    }

    /// The ray reaching the camera from world direction `look`, the
    /// observer measuring its energy as `unit`: (E, p). It is linear in
    /// `unit` and `look` together.
    fn ray(&self, unit: f64, look: DVec3) -> DVec4 {
        let (a, r) = (self.a, self.r);
        // It travels the other way from where the camera looks.
        let n = -DVec3::new(
            look.dot(self.axes[0]),
            look.dot(self.axes[1]),
            look.dot(self.axes[2]),
        );
        let (st, ct) = self.theta.sin_cos();
        let d = delta(a, r);
        let rho = (r * r + a * a * ct * ct).sqrt();
        let big_a = (r * r + a * a).powi(2) - a * a * d * st * st;
        // The ZAMO's lapse and frame-dragging rate.
        let alpha = rho * (d / big_a).sqrt();
        let omega = 2.0 * a * r / big_a;
        // Boyer–Lindquist components, from the observer's frame
        // (√Δ/ρ ∂_r, ∂_θ/ρ, ρ/(√A sin θ) ∂_φ).
        let l = big_a.sqrt() * st / rho * n.z;
        let e = unit * alpha + omega * l;
        let pr = rho / d.sqrt() * n.x;
        let ptheta = rho * n.y;
        // Kerr–Schild's time and φ run ahead of Boyer–Lindquist's by
        // ∫ 2r dr/Δ and ∫ a dr/Δ, which adds (2rE − aL)/Δ to p_r.
        let pr = pr + (2.0 * r * e - a * l) / d;
        let [jr, jt, jp] = Self::jacobian(a, r, self.theta, self.place);
        let jacobian = DMat3::from_cols(jr, jt, jp);
        let p = jacobian.transpose().inverse() * DVec3::new(pr, ptheta, l);
        DVec4::new(e, p.x, p.y, p.z)
    }

    /// The ray that reaches the camera from the world direction `look`
    /// (unit; where the camera looks), the observer measuring its energy as
    /// 1: its energy E and momentum p.
    pub fn arriving(&self, look: DVec3) -> (f64, DVec3) {
        let v = self.ray(1.0, look);
        (v.x, DVec3::new(v.y, v.z, v.w))
    }

    /// [`Camera::arriving`] as a matrix, for the GPU:
    /// (E, p) = M (1, look).
    pub fn launch(&self) -> DMat4 {
        DMat4::from_cols(
            self.ray(1.0, DVec3::ZERO),
            self.ray(0.0, DVec3::X),
            self.ray(0.0, DVec3::Y),
            self.ray(0.0, DVec3::Z),
        )
    }
}

/// A ray crossing the disk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiskHit {
    /// Where, as Boyer–Lindquist radius.
    pub r: f64,
    /// The shift of its light, ν_seen/ν_sent.
    pub g: f64,
}

/// What a ray traced back from the camera found.
#[derive(Debug, Clone, PartialEq)]
pub struct Traced {
    /// Where on the sky it came from (a world direction), or `None` if it
    /// came out of the hole.
    pub sky: Option<DVec3>,
    /// The disk crossings on the way, nearest the camera first.
    pub disk: Vec<DiskHit>,
    /// Its energy (the camera measuring it as 1).
    pub energy: f64,
    /// Its angular momentum about the spin axis.
    pub angular: f64,
    /// Its Carter constant.
    pub carter: f64,
    /// How many steps it took.
    pub steps: usize,
}

/// One Dormand–Prince 5(4) step of `h` (negative: back in time) from `s`,
/// whose rates are `k1`: the new state, its rates, and the error estimate.
fn dormand_prince(a: f64, e: f64, s: State, k1: State, h: f64) -> (State, State, State) {
    let f = |s: State| rates(a, e, s);
    let k2 = f(s.add(k1, h / 5.0));
    let k3 = f(s.add(k1, h * 3.0 / 40.0).add(k2, h * 9.0 / 40.0));
    let k4 = f(s
        .add(k1, h * 44.0 / 45.0)
        .add(k2, -h * 56.0 / 15.0)
        .add(k3, h * 32.0 / 9.0));
    let k5 = f(s
        .add(k1, h * 19372.0 / 6561.0)
        .add(k2, -h * 25360.0 / 2187.0)
        .add(k3, h * 64448.0 / 6561.0)
        .add(k4, -h * 212.0 / 729.0));
    let k6 = f(s
        .add(k1, h * 9017.0 / 3168.0)
        .add(k2, -h * 355.0 / 33.0)
        .add(k3, h * 46732.0 / 5247.0)
        .add(k4, h * 49.0 / 176.0)
        .add(k5, -h * 5103.0 / 18656.0));
    let next = s
        .add(k1, h * 35.0 / 384.0)
        .add(k3, h * 500.0 / 1113.0)
        .add(k4, h * 125.0 / 192.0)
        .add(k5, -h * 2187.0 / 6784.0)
        .add(k6, h * 11.0 / 84.0);
    let k7 = f(next);
    // The fifth-order result less the embedded fourth-order one.
    let zero = State {
        x: DVec3::ZERO,
        p: DVec3::ZERO,
    };
    let error = zero
        .add(k1, h * (35.0 / 384.0 - 5179.0 / 57600.0))
        .add(k3, h * (500.0 / 1113.0 - 7571.0 / 16695.0))
        .add(k4, h * (125.0 / 192.0 - 393.0 / 640.0))
        .add(k5, h * (-2187.0 / 6784.0 + 92097.0 / 339200.0))
        .add(k6, h * (11.0 / 84.0 - 187.0 / 2100.0))
        .add(k7, -h / 40.0);
    (next, k7, error)
}

/// The most steps a ray is followed for. Only rays within rounding of the
/// edge of the shadow circle the hole long enough to need more; they are
/// counted as captured.
const MAX_STEPS: usize = 100_000;

/// How much farther than the camera rays are followed out: past R, a ray
/// passing the hole at b bends by under GM b/(c²R²) more, and b is at most
/// about the camera's distance r, so by under 10⁻¹⁰/r rad.
const FAR: f64 = 1e5;

/// Traces back the ray the camera sees along world direction `look`, to
/// the sky or into the hole, noting where it crosses a disk in the
/// equatorial plane from `disk.0` to `disk.1` (Boyer–Lindquist radii).
/// `tolerance` bounds each step's relative error.
///
/// A ray moving inward inside the innermost photon orbit can't turn back
/// out, so it is captured there, before the coordinates' trouble at the
/// past horizon; one moving outward past [`FAR`] times the camera's
/// distance escapes.
pub fn trace(camera: &Camera, look: DVec3, disk: Option<(f64, f64)>, tolerance: f64) -> Traced {
    let a = camera.a;
    let (e, p) = camera.arriving(look);
    let mut s = State { x: camera.place, p };
    let mut traced = Traced {
        sky: None,
        disk: Vec::new(),
        energy: e,
        angular: s.x.x * p.y - s.x.y * p.x,
        carter: carter(a, s.x, p, e),
        steps: 0,
    };
    let far = FAR * camera.r;
    let (inner, _) = photon_orbits(a);
    let mut r = camera.r;
    let mut k = rates(a, e, s);
    // Back in time: negative steps, first a hundredth of the distance.
    let mut h = -0.01 * r / k.x.length();
    while traced.steps < MAX_STEPS {
        let (next, k_next, error) = dormand_prince(a, e, s, k, h);
        let size =
            (error.x.length() / s.x.length()).max(error.p.length() / s.p.length()) / tolerance;
        if size > 1.0 {
            h *= (0.9 * size.powf(-0.2)).max(0.1);
            continue;
        }
        traced.steps += 1;
        if let Some((from, to)) = disk
            && s.x.z * next.x.z <= 0.0
            && s.x.z != 0.0
        {
            let hit = crossing(a, e, (s, k), (next, k_next), h);
            let r = (hit.x.x * hit.x.x + hit.x.y * hit.x.y - a * a).sqrt();
            if (from..=to).contains(&r) {
                traced.disk.push(DiskHit {
                    r,
                    g: disk_shift(a, r, e, traced.angular),
                });
            }
        }
        let r_next = radius(a, next.x);
        let inward = r_next < r;
        (s, k, r) = (next, k_next, r_next);
        if inward && r < inner {
            return traced;
        }
        if !inward && r > far {
            // It came from where its motion points away from.
            traced.sky = Some(-k.x.normalize());
            return traced;
        }
        h *= (0.9 * size.max(1e-10).powf(-0.2)).min(5.0);
    }
    traced
}

/// Where a step from `start` to `end` (each with its rates), of `h`,
/// crosses the equatorial plane: first on the cubic through both ends,
/// then by Newton's method with steps of the integrator itself, so the
/// place is as good as the ray.
fn crossing(a: f64, e: f64, start: (State, State), end: (State, State), h: f64) -> State {
    let ((s, ds), (n, dn)) = (start, end);
    let cubic = |t: f64| {
        let (t2, t3) = (t * t, t * t * t);
        s.x.z * (2.0 * t3 - 3.0 * t2 + 1.0)
            + ds.x.z * h * (t3 - 2.0 * t2 + t)
            + n.x.z * (3.0 * t2 - 2.0 * t3)
            + dn.x.z * h * (t3 - t2)
    };
    let (mut lo, mut hi) = (0.0, 1.0);
    let above = s.x.z > 0.0;
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if (cubic(mid) > 0.0) == above {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (mut at, _, _) = dormand_prince(a, e, s, ds, 0.5 * (lo + hi) * h);
    for _ in 0..3 {
        let rate = rates(a, e, at);
        (at, _, _) = dormand_prince(a, e, at, rate, -at.x.z / rate.x.z);
    }
    at
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photon_orbits_and_the_circular_orbit_are_bardeens() {
        // No spin: both photon orbits at 3; maximal spin: 1 and 4.
        let (p, q) = photon_orbits(0.0);
        assert!((p - 3.0).abs() < 1e-12 && (q - 3.0).abs() < 1e-12);
        let (p, q) = photon_orbits(1.0);
        assert!((p - 1.0).abs() < 1e-9 && (q - 4.0).abs() < 1e-9);
        // No spin, r = 6: u^t = 1/√(1 − 3/r) = √2.
        let (omega, ut) = circular(0.0, 6.0);
        assert!((ut - 2f64.sqrt()).abs() < 1e-12 && (omega - 6f64.powf(-1.5)).abs() < 1e-15);
    }

    #[test]
    fn the_camera_frame_is_spherical_without_spin() {
        let camera = Camera::at(0.0, DVec3::new(3.0, 4.0, 0.0));
        assert!((camera.r - 5.0).abs() < 1e-12);
        assert!((camera.axes[0] - DVec3::new(0.6, 0.8, 0.0)).length() < 1e-12);
        assert!((camera.axes[1] - DVec3::new(0.0, 0.0, -1.0)).length() < 1e-12);
    }

    /// Looks spread around the sky.
    fn looks() -> impl Iterator<Item = DVec3> {
        (0..40).map(|k| {
            let z = 1.0 - (2.0 * k as f64 + 1.0) / 40.0;
            let phi = 2.4 * k as f64;
            let s = (1.0 - z * z).sqrt();
            DVec3::new(s * phi.cos(), s * phi.sin(), z)
        })
    }

    #[test]
    fn rays_leave_the_camera_as_light() {
        // H = 0 to rounding: |p|² and E² are of order 1 here, so 10⁻¹⁴
        // allows for a few dozen roundings. The matrix gives the same rays.
        for a in [0.0, 0.5, 0.99] {
            for place in [
                DVec3::new(30.0, -4.0, 9.0),
                DVec3::new(2.5, 1.0, -0.7),
                DVec3::new(0.0, 0.0, 12.0),
            ] {
                let camera = Camera::at(a, place);
                for look in looks() {
                    let (e, p) = camera.arriving(look);
                    let h = hamiltonian(a, e, State { x: camera.place, p });
                    assert!(h.abs() < 1e-14 * (e * e + p.length_squared()), "H = {h:e}");
                    let m = camera.launch() * DVec4::new(1.0, look.x, look.y, look.z);
                    assert!((m - DVec4::new(e, p.x, p.y, p.z)).abs().max_element() < 1e-12);
                }
            }
        }
    }

    #[test]
    fn rays_keep_their_energy_angular_momentum_and_carters_constant() {
        // Hamilton's equations conserve H, L and Q. Each step's error is
        // checked to be under 10⁻¹² of the state, so over 200 steps each
        // drifts by under 2 × 10⁻¹⁰ of its size.
        let a = 0.9;
        let camera = Camera::at(a, DVec3::new(8.0, 3.0, 5.0));
        let (e, p) = camera.arriving(DVec3::new(-0.8, 0.1, -0.5).normalize());
        let mut s = State { x: camera.place, p };
        let (l0, q0) = (s.x.x * p.y - s.x.y * p.x, carter(a, s.x, p, e));
        let mut k = rates(a, e, s);
        for _ in 0..200 {
            let (next, k_next, error) = dormand_prince(a, e, s, k, -0.02);
            assert!(
                error.x.length() < 1e-12 * s.x.length() && error.p.length() < 1e-12 * s.p.length()
            );
            (s, k) = (next, k_next);
        }
        let l = s.x.x * s.p.y - s.x.y * s.p.x;
        let q = carter(a, s.x, s.p, e);
        println!(
            "after 200 steps to r = {:.2}: H {:.1e}, L {l0} → {l}, Q {q0} → {q}",
            radius(a, s.x),
            hamiltonian(a, e, s)
        );
        assert!(hamiltonian(a, e, s).abs() < 2e-10 * (e * e + s.p.length_squared()));
        assert!((l - l0).abs() < 2e-10 * l0.abs().max(1.0));
        assert!((q - q0).abs() < 2e-10 * q0.abs().max(1.0));
    }
}
