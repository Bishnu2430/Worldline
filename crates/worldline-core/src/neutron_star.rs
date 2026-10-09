//! Neutron stars: how big and how heavy they can be, from an equation of
//! state for nuclear matter and general relativity's equations for a
//! static star. See `docs/physics/neutron-stars.md`.
//!
//! Matter denser than an atomic nucleus isn't fully understood, so no
//! single equation of state is known to be right. Worldline uses SLy
//! (Douchin & Haensel 2001, *A&A* 380, 151), a widely used model consistent
//! with the heaviest measured neutron stars, in the piecewise-polytrope
//! form of Read, Lackey, Owen & Friedman (2009), *Phys. Rev. D* 79, 124032.
//! Where the real equation of state differs, so will the stars: the app
//! says which model it is.

use std::f64::consts::PI;
use std::sync::OnceLock;

use crate::constants::{C, G};

/// One piece of a piecewise polytrope, in SI units: p = K ρ^Γ from
/// rest-mass density `start` (kg/m³) up, with energy density
/// ε = (1 + a) ρ c² + K ρ^Γ/(Γ − 1).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Piece {
    k: f64,
    gamma: f64,
    start: f64,
    a: f64,
}

impl Piece {
    fn pressure(&self, rho: f64) -> f64 {
        self.k * rho.powf(self.gamma)
    }

    fn energy(&self, rho: f64) -> f64 {
        (1.0 + self.a) * rho * C * C + self.pressure(rho) / (self.gamma - 1.0)
    }

    /// The specific enthalpy h = (ε + p)/(ρc²), dimensionless.
    fn enthalpy(&self, rho: f64) -> f64 {
        1.0 + self.a
            + self.gamma / (self.gamma - 1.0) * self.k * rho.powf(self.gamma - 1.0) / (C * C)
    }

    /// The density at specific enthalpy `h`, inverting [`Piece::enthalpy`].
    fn density(&self, h: f64) -> f64 {
        ((h - 1.0 - self.a) * (self.gamma - 1.0) * C * C / (self.gamma * self.k))
            .powf(1.0 / (self.gamma - 1.0))
    }
}

/// A cold equation of state made of polytropic pieces, with pressure and
/// energy density continuous (Read et al. 2009, section II).
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewisePolytrope {
    pieces: Vec<Piece>,
}

/// SLy's crust below nuclear density, four pieces (Read et al. 2009,
/// Table II): K and Γ, with p/c² = K ρ^Γ in g/cm³ for ρ in g/cm³ (their
/// footnote 1: K is given for p/c²). Their table also prints the
/// densities where each piece hands over (2.44034 × 10⁷, 3.78358 × 10¹¹
/// and 2.62780 × 10¹² g/cm³); there the printed pieces' pressures differ
/// by up to 1.2 × 10⁻⁴, so the handovers are recomputed where they agree,
/// as Read et al. define them (pressure continuous), within 2 × 10⁻⁴ of
/// the printed densities.
const SLY_CRUST: [(f64, f64); 4] = [
    (6.80110e-09, 1.58425),
    (1.06186e-06, 1.28733),
    (5.32697e+01, 0.62223),
    (3.99874e-08, 1.35692),
];

impl PiecewisePolytrope {
    /// Read et al.'s parameterization: SLy's crust, then three pieces with
    /// adiabatic indices `gammas` above, joined at the fixed densities
    /// 10^14.7 and 10^15 g/cm³, the pressure at the first being
    /// 10^`log_p1` dyne/cm². The crust meets the first piece where their
    /// pressures agree.
    pub fn read(log_p1: f64, gammas: [f64; 3]) -> Self {
        // To SI: p in Pa is 0.1 p in dyne/cm², and p in dyne/cm² is c² (in
        // cm²/s²) times p/c² in g/cm³; ρ in kg/m³ is 1000 ρ in g/cm³. So
        // K_SI = 0.1 (100 c)² K 1000^(−Γ).
        let si = |k: f64, gamma: f64| 0.1 * (100.0 * C).powi(2) * k * 1000f64.powf(-gamma);
        let (rho1, rho2): (f64, f64) = (10f64.powf(14.7) * 1e3, 1e15 * 1e3);
        let k1 = 0.1 * 10f64.powf(log_p1) / rho1.powf(gammas[0]);
        let k2 = k1 * rho1.powf(gammas[0] - gammas[1]);
        let k3 = k2 * rho2.powf(gammas[1] - gammas[2]);
        let mut kg: Vec<(f64, f64)> = SLY_CRUST.iter().map(|&(k, g)| (si(k, g), g)).collect();
        kg.push((k1, gammas[0]));
        // Each piece hands over where its pressure meets the next's:
        // K_i ρ^Γ_i = K_j ρ^Γ_j.
        let meet = |(ki, gi): (f64, f64), (kj, gj): (f64, f64)| (ki / kj).powf(1.0 / (gj - gi));
        let mut raw: Vec<(f64, f64, f64)> = vec![(kg[0].0, kg[0].1, 0.0)];
        for pair in kg.windows(2) {
            raw.push((pair[1].0, pair[1].1, meet(pair[0], pair[1])));
        }
        raw.push((k2, gammas[1], rho1));
        raw.push((k3, gammas[2], rho2));
        // The energy density's constants, from continuity.
        let mut pieces: Vec<Piece> = Vec::new();
        for (k, gamma, start) in raw {
            let a = match pieces.last() {
                None => 0.0,
                Some(prev) => {
                    prev.energy(start) / (start * C * C)
                        - 1.0
                        - k * start.powf(gamma - 1.0) / ((gamma - 1.0) * C * C)
                }
            };
            pieces.push(Piece { k, gamma, start, a });
        }
        Self { pieces }
    }

    /// SLy (Douchin & Haensel 2001), as fitted by Read et al. (2009, Table
    /// III): log p₁ = 34.384, Γ = 3.005, 2.988, 2.851.
    pub fn sly() -> Self {
        Self::read(34.384, [3.005, 2.988, 2.851])
    }

    fn piece(&self, rho: f64) -> &Piece {
        self.pieces
            .iter()
            .rev()
            .find(|p| rho >= p.start)
            .unwrap_or(&self.pieces[0])
    }

    /// Pressure (Pa) at rest-mass density `rho` (kg/m³).
    pub fn pressure(&self, rho: f64) -> f64 {
        self.piece(rho).pressure(rho)
    }

    /// Energy density (J/m³) at rest-mass density `rho` (kg/m³).
    pub fn energy(&self, rho: f64) -> f64 {
        self.piece(rho).energy(rho)
    }

    /// The specific enthalpy (ε + p)/(ρc²) at the start of each piece:
    /// where the integration's steps should break.
    fn joints(&self) -> Vec<f64> {
        self.pieces
            .iter()
            .skip(1)
            .map(|p| p.enthalpy(p.start))
            .collect()
    }
}

/// Matter a star can be made of, described by its specific enthalpy h.
trait Matter {
    /// Rest-mass density (kg/m³), energy density (J/m³) and pressure (Pa)
    /// at specific enthalpy `h`.
    fn at(&self, h: f64) -> (f64, f64, f64);
    /// The specific enthalpy at rest-mass density `rho`.
    fn enthalpy(&self, rho: f64) -> f64;
    /// Enthalpies where the equation of state has a kink.
    fn joints(&self) -> Vec<f64>;
}

impl Matter for PiecewisePolytrope {
    fn at(&self, h: f64) -> (f64, f64, f64) {
        let piece = self
            .pieces
            .iter()
            .rev()
            .find(|p| h >= p.enthalpy(p.start))
            .unwrap_or(&self.pieces[0]);
        let rho = piece.density(h);
        (rho, piece.energy(rho), piece.pressure(rho))
    }

    fn enthalpy(&self, rho: f64) -> f64 {
        self.piece(rho).enthalpy(rho)
    }

    fn joints(&self) -> Vec<f64> {
        PiecewisePolytrope::joints(self)
    }
}

/// A static star.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    /// Its gravitational parameter G M, m³/s², M its gravitational mass.
    pub gm: f64,
    /// Its radius (areal: its circumference over 2π), m.
    pub radius: f64,
    /// Its rest-mass density at the center, kg/m³.
    pub central_density: f64,
}

/// The static star of `matter` with central rest-mass density `rho_c`:
/// the Tolman–Oppenheimer–Volkoff equations (Tolman 1939, *Phys. Rev.* 55,
/// 364; Oppenheimer & Volkoff 1939, *Phys. Rev.* 55, 374), in the form of
/// Lindblom (1992, *ApJ* 398, 569), with the log of the specific
/// enthalpy, η = ln h, as the variable: from its central value to 0 at the
/// surface,
///
/// dr/dη = −r (r − 2Gm/c²) / (Gm/c² + 4πG r³ p/c⁴), dm/dη = 4π r² (ε/c²) dr/dη.
///
/// Integrated by fourth-order Runge–Kutta, halving steps until each is
/// accurate to 10⁻¹², and breaking at the equation of state's kinks.
fn star(matter: &dyn Matter, rho_c: f64) -> Star {
    let eta_c = matter.enthalpy(rho_c).ln();
    let (_, e_c, p_c) = matter.at(eta_c.exp());
    let rate = |eta: f64, s: [f64; 2]| {
        let (r, m) = (s[0], s[1]);
        let (_, e, p) = matter.at(eta.exp());
        let drdeta = -r * (r - 2.0 * G * m / (C * C))
            / (G * m / (C * C) + 4.0 * PI * G * r.powi(3) * p / C.powi(4));
        [drdeta, 4.0 * PI * r * r * e / (C * C) * drdeta]
    };
    let rk4 = |eta: f64, s: [f64; 2], h: f64| {
        let add = |s: [f64; 2], k: [f64; 2], c: f64| [s[0] + c * k[0], s[1] + c * k[1]];
        let k1 = rate(eta, s);
        let k2 = rate(eta + h / 2.0, add(s, k1, h / 2.0));
        let k3 = rate(eta + h / 2.0, add(s, k2, h / 2.0));
        let k4 = rate(eta + h, add(s, k3, h));
        [
            s[0] + h / 6.0 * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]),
            s[1] + h / 6.0 * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]),
        ]
    };
    // Off the center, by Lindblom's series: r² = 3c⁴ (η_c − η)/(2πG(ε_c + 3p_c)).
    let start = 1e-8 * eta_c;
    let r0 = (3.0 * C.powi(4) * start / (2.0 * PI * G * (e_c + 3.0 * p_c))).sqrt();
    let mut s = [r0, 4.0 / 3.0 * PI * e_c / (C * C) * r0.powi(3)];
    let mut eta = eta_c - start;
    // The stops: each kink below the center, then the surface.
    let mut stops: Vec<f64> = matter
        .joints()
        .into_iter()
        .map(f64::ln)
        .filter(|&j| j < eta && j > 0.0)
        .collect();
    stops.sort_by(|a, b| b.total_cmp(a));
    stops.push(0.0);
    for stop in stops {
        let mut h = -(eta - stop) / 64.0;
        while eta > stop {
            h = h.max(stop - eta);
            let whole = rk4(eta, s, h);
            let half = rk4(eta + h / 2.0, rk4(eta, s, h / 2.0), h / 2.0);
            let error =
                ((half[0] - whole[0]) / half[0]).abs() + ((half[1] - whole[1]) / half[1]).abs();
            if error > 1e-12 && h.abs() > 1e-14 {
                h /= 2.0;
                continue;
            }
            eta = if stop - (eta + h) > -1e-15 * eta_c {
                stop
            } else {
                eta + h
            };
            s = half;
            if error < 1e-12 / 64.0 {
                h *= 2.0;
            }
        }
    }
    Star {
        gm: G * s[1],
        radius: s[0],
        central_density: rho_c,
    }
}

/// The static, non-rotating star of `eos` with central rest-mass density
/// `rho_c` (kg/m³).
pub fn neutron_star(eos: &PiecewisePolytrope, rho_c: f64) -> Star {
    star(eos, rho_c)
}

/// The heaviest static star `eos` allows: the maximum of its mass over
/// central density, found by golden-section search. Denser stars than
/// that are unstable and collapse.
pub fn maximum_mass(eos: &PiecewisePolytrope) -> Star {
    let (mut low, mut high) = ((5e17f64).ln(), (1e19f64).ln());
    let ratio = (5f64.sqrt() - 1.0) / 2.0;
    let mass = |x: f64| neutron_star(eos, x.exp()).gm;
    let (mut a, mut b) = (high - ratio * (high - low), low + ratio * (high - low));
    let (mut ma, mut mb) = (mass(a), mass(b));
    for _ in 0..80 {
        if ma > mb {
            high = b;
            b = a;
            mb = ma;
            a = high - ratio * (high - low);
            ma = mass(a);
        } else {
            low = a;
            a = b;
            ma = mb;
            b = low + ratio * (high - low);
            mb = mass(b);
        }
        if high - low < 1e-9 {
            break;
        }
    }
    neutron_star(eos, (0.5 * (low + high)).exp())
}

/// The radius (m) of the stable static star of `eos` with gravitational
/// parameter `gm`, if there is one: lighter than [`maximum_mass`]. Found by
/// bisection on the central density, below the maximum's, where the mass
/// rises steadily.
pub fn radius_for(eos: &PiecewisePolytrope, gm: f64) -> Option<f64> {
    radius_below(eos, gm, maximum_mass(eos))
}

/// [`radius_for`], given `eos`'s heaviest star.
fn radius_below(eos: &PiecewisePolytrope, gm: f64, heaviest: Star) -> Option<f64> {
    if gm <= 0.0 || gm >= heaviest.gm {
        return None;
    }
    let (mut low, mut high) = ((1e15f64).ln(), heaviest.central_density.ln());
    if neutron_star(eos, low.exp()).gm > gm {
        return None;
    }
    for _ in 0..200 {
        let middle = 0.5 * (low + high);
        if neutron_star(eos, middle.exp()).gm < gm {
            low = middle;
        } else {
            high = middle;
        }
        if high - low < 1e-12 {
            break;
        }
    }
    Some(neutron_star(eos, (0.5 * (low + high)).exp()).radius)
}

/// SLy, built once.
fn sly() -> &'static PiecewisePolytrope {
    static SLY: OnceLock<PiecewisePolytrope> = OnceLock::new();
    SLY.get_or_init(PiecewisePolytrope::sly)
}

/// SLy's heaviest static star, computed once.
pub fn sly_maximum() -> Star {
    static MAX: OnceLock<Star> = OnceLock::new();
    *MAX.get_or_init(|| maximum_mass(sly()))
}

/// The radius (m) of SLy's static star of gravitational parameter `gm`, if
/// it is lighter than [`sly_maximum`].
pub fn sly_radius(gm: f64) -> Option<f64> {
    radius_below(sly(), gm, sly_maximum())
}

/// SLy's prompt-collapse threshold ([`prompt_collapse_threshold`]), as
/// G M (m³/s²), computed once.
pub fn sly_threshold() -> f64 {
    static THRESHOLD: OnceLock<f64> = OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        let r16 = sly_radius(1.6 * crate::constants::GM_SUN).expect("SLy holds 1.6 Suns");
        prompt_collapse_threshold(sly_maximum().gm, r16)
    })
}

/// The total mass (as G M, m³/s²) above which two neutron stars that merge
/// collapse to a black hole at once ("prompt collapse"), rather than first
/// forming a hot, spinning neutron star that collapses later or not at
/// all: M_thres = k M_max, k = −3.606 G M_max/(c² R₁.₆) + 2.380, from
/// merger simulations with 12 equations of state (Bauswein, Baumgarte &
/// Janka 2013, *Phys. Rev. Lett.* 111, 131101; largest miss of k, 0.025).
/// `gm_max` is the heaviest static star's G M, `radius_1_6` the radius of
/// one of 1.6 Suns.
pub fn prompt_collapse_threshold(gm_max: f64, radius_1_6: f64) -> f64 {
    let compactness = gm_max / (C * C * radius_1_6);
    (-3.606 * compactness + 2.380) * gm_max
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::GM_SUN;

    /// Matter of constant density: the star has an exact solution.
    struct Incompressible(f64);

    impl Matter for Incompressible {
        fn at(&self, h: f64) -> (f64, f64, f64) {
            // ε/ρ is constant, so h = 1 + p/(ρc²).
            (self.0, self.0 * C * C, (h - 1.0) * self.0 * C * C)
        }
        fn enthalpy(&self, _rho: f64) -> f64 {
            // Set from outside: see the test.
            unreachable!()
        }
        fn joints(&self) -> Vec<f64> {
            Vec::new()
        }
    }

    #[test]
    fn a_star_of_constant_density_matches_schwarzschilds_interior_solution() {
        // Schwarzschild (1916): a star of constant density ρ and compactness
        // u = GM/(Rc²) has central pressure ρc² (1 − √(1 − 2u))/(3√(1 − 2u) − 1),
        // and mass (4π/3) ρ R³.
        struct Centered(Incompressible, f64);
        impl Matter for Centered {
            fn at(&self, h: f64) -> (f64, f64, f64) {
                self.0.at(h)
            }
            fn enthalpy(&self, _rho: f64) -> f64 {
                self.1
            }
            fn joints(&self) -> Vec<f64> {
                Vec::new()
            }
        }
        let rho = 5e17;
        for h_c in [1.01, 1.1, 1.3] {
            let s = star(&Centered(Incompressible(rho), h_c), rho);
            let u = s.gm / (s.radius * C * C);
            let root = (1.0 - 2.0 * u).sqrt();
            let exact = rho * C * C * (1.0 - root) / (3.0 * root - 1.0);
            let p_c = (h_c - 1.0) * rho * C * C;
            let mass = s.gm / G / (4.0 / 3.0 * PI * rho * s.radius.powi(3));
            println!(
                "h_c = {h_c}: GM/Rc² = {u:.5}, central pressure off Schwarzschild's by {:.1e}, mass off (4π/3)ρR³ by {:.1e}",
                (p_c / exact - 1.0).abs(),
                (mass - 1.0).abs()
            );
            assert!((p_c / exact - 1.0).abs() < 1e-9 && (mass - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn sly_is_continuous_and_holds_about_two_suns() {
        let eos = PiecewisePolytrope::sly();
        for p in &eos.pieces[1..] {
            let (below, above) = (p.start * (1.0 - 1e-12), p.start * (1.0 + 1e-12));
            assert!((eos.pressure(below) / eos.pressure(above) - 1.0).abs() < 1e-9);
            assert!((eos.energy(below) / eos.energy(above) - 1.0).abs() < 1e-9);
        }
        let heaviest = sly_maximum();
        println!(
            "SLy: heaviest {:.4} Suns, {:.3} km, central density {:.3e} kg/m³",
            heaviest.gm / GM_SUN,
            heaviest.radius / 1e3,
            heaviest.central_density
        );
        assert!((heaviest.gm / GM_SUN - 2.05).abs() < 0.005);
    }
}
