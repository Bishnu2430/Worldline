//! What a collision leaves (step 6.1): Leinhardt & Stewart's (2012) scaling
//! laws for collisions between gravity-dominated bodies, from planetesimals
//! to super-Earths. See `docs/physics/collision-outcomes.md`.
//!
//! Everything follows from how the collision's energy compares with the
//! energy that disperses half the mass, Q′*_RD, which depends on the bodies'
//! size, their mass ratio and the impact's angle. Equation numbers are the
//! paper's; the order of the calculation is its Appendix's.

use std::f64::consts::PI;

use crate::constants::G;

/// How hard a kind of body is to disperse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    /// The energy to disperse half of two equal bodies, in units of their
    /// specific gravitational binding energy (eq. 28).
    pub c_star: f64,
    /// How the energy scales with impact speed: 1/3 for pure momentum
    /// scaling, 2/3 for pure energy scaling.
    pub mu_bar: f64,
}

impl Material {
    /// Best fit to small bodies of every make-up: c* = 5 ± 2, μ̄ = 0.37 ±
    /// 0.01.
    pub const SMALL_BODIES: Self = Self {
        c_star: 5.0,
        mu_bar: 0.37,
    };
    /// Best fit to planets, as hydrodynamic bodies: c* = 1.9 ± 0.3, μ̄ =
    /// 0.36 ± 0.01.
    pub const PLANETS: Self = Self {
        c_star: 1.9,
        mu_bar: 0.36,
    };
    /// Best fit to the paper's own simulations of 10-km rubble piles.
    pub const RUBBLE_PILES: Self = Self {
        c_star: 4.3,
        mu_bar: 0.35,
    };

    /// The fit for bodies of combined radius `r_c1` (see
    /// [`combined_radius`]). The paper's small bodies stop near 125 km and
    /// its planets start near 5,600 km, and it sets no boundary between
    /// them. Worldline's, 1,000 km, is about the gap's geometric middle: a
    /// choice, not a result.
    pub fn for_size(r_c1: f64) -> Self {
        if r_c1 >= 1e6 {
            Self::PLANETS
        } else {
            Self::SMALL_BODIES
        }
    }
}

/// ρ₁, the density the combined radius is taken at, kg/m³.
const DENSITY_ONE: f64 = 1000.0;

/// Where the universal law ends and the power law of super-catastrophic
/// disruption begins: Q_R / Q′*_RD, with a tenth of the mass left.
const SUPER_CATASTROPHIC: f64 = 1.8;

/// That power law's exponent, η ≈ −1.5.
const ETA: f64 = -1.5;

/// The slope of the fragments' size distribution, as fitted to the second
/// remnants.
const BETA: f64 = 2.85;

/// Earth's mass, kg: the IAU's nominal GM⊕ over G.
const EARTH_MASS: f64 = 3.986_004e14 / G;

/// The largest super-Earths in the paper's data: 10 Earth masses.
const MOST_MASSIVE: f64 = 10.0 * EARTH_MASS;

/// The radius below which strength, not gravity, holds a body together: the
/// paper's data on gravity-dominated bodies start at 1 km.
const SMALLEST: f64 = 1e3;

/// The steepest mass ratio in the paper's data, 1 : 45,000.
const SMALLEST_RATIO: f64 = 1.0 / 45_000.0;

/// The fastest critical speed in the paper's data, m/s.
const FASTEST: f64 = 80e3;

/// R_C1: the radius of the bodies' whole mass at ρ₁, m.
pub fn combined_radius(mass: f64) -> f64 {
    (3.0 * mass / (4.0 * PI * DENSITY_ONE)).cbrt()
}

/// Q*_RD for two equal bodies of total mass `mass`, J/kg (eq. 28), and the
/// speed that brings it, m/s (eq. 30): the principal disruption curve.
pub fn equal_mass_threshold(mass: f64, material: Material) -> (f64, f64) {
    let r = combined_radius(mass);
    let energy = material.c_star * 0.8 * PI * DENSITY_ONE * G * r * r;
    let speed = (32.0 * PI * material.c_star / 5.0).sqrt() * (DENSITY_ONE * G).sqrt() * r;
    (energy, speed)
}

/// Q*_RD for a head-on impact of mass ratio `gamma`, J/kg: the equal-mass
/// value raised for how much less of the energy a small projectile deposits
/// (eq. 23).
fn head_on_threshold(mass: f64, gamma: f64, material: Material) -> f64 {
    let (equal, _) = equal_mass_threshold(mass, material);
    let unevenness = 0.25 * (gamma + 1.0) * (gamma + 1.0) / gamma;
    equal * unevenness.powf(2.0 / (3.0 * material.mu_bar) - 1.0)
}

/// The largest remnant, as a share of the total mass, for an energy `x`
/// times the disruption energy: the universal law (eq. 5), then past it the
/// power law of super-catastrophic disruption (eq. 44).
pub fn largest_share(x: f64) -> f64 {
    if x < SUPER_CATASTROPHIC {
        -0.5 * (x - 1.0) + 0.5
    } else {
        0.1 / SUPER_CATASTROPHIC.powf(ETA) * x.powf(ETA)
    }
}

/// The second-largest remnant, as a share of the total mass, when the
/// largest has `largest`: the size distribution's next body, with one
/// largest remnant and two second-largest (eq. 37).
pub fn second_share(largest: f64) -> f64 {
    (3.0 - BETA) * (1.0 - largest) / (2.0 * BETA)
}

/// What a collision does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Regime {
    /// Slower than the bodies' mutual escape speed: they become one.
    Merging,
    /// The target gains some of the projectile; the rest flies off.
    PartialAccretion,
    /// A grazing impact: the two meet and part, the target hardly changed.
    HitAndRun,
    /// The target ends up smaller than it began.
    Erosion,
    /// Less than a tenth of the mass stays in one body.
    SuperCatastrophic,
}

/// A collision at the moment of contact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Impact {
    /// The larger body's mass, kg.
    pub target_mass: f64,
    /// The smaller's.
    pub projectile_mass: f64,
    /// The larger body's radius, m.
    pub target_radius: f64,
    /// The smaller's.
    pub projectile_radius: f64,
    /// The relative speed at contact, m/s.
    pub speed: f64,
    /// b: the sine of the angle between the line of centers and the
    /// velocity at contact. 0 head-on, 1 grazing.
    pub impact_parameter: f64,
}

/// What a collision leaves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outcome {
    /// What kind of collision it is.
    pub regime: Regime,
    /// The largest remnant's mass, kg.
    pub largest: f64,
    /// The second-largest's.
    pub second: f64,
    /// Q_R, the collision's energy per unit of total mass, J/kg.
    pub energy: f64,
    /// Q′*_RD, the energy that would disperse half the mass, J/kg.
    pub threshold: f64,
    /// V′*: the speed that would disperse half the mass, m/s.
    pub disruption_speed: f64,
    /// V′_esc: below this they merge, m/s.
    pub merging_speed: f64,
    /// V_erosion: above this the target shrinks, m/s.
    pub erosion_speed: f64,
    /// V_supercat, m/s.
    pub super_catastrophic_speed: f64,
    /// Whether under half the projectile meets the target.
    pub grazing: bool,
}

impl Impact {
    /// The two bodies' total mass, kg.
    pub fn total_mass(&self) -> f64 {
        self.target_mass + self.projectile_mass
    }

    /// γ, the projectile's mass over the target's.
    pub fn mass_ratio(&self) -> f64 {
        self.projectile_mass / self.target_mass
    }

    /// μ, kg.
    fn reduced_mass(&self) -> f64 {
        self.target_mass * self.projectile_mass / self.total_mass()
    }

    /// l: how far the projectile overlaps the target across the line of
    /// flight, m (eq. 7).
    fn overlap(&self) -> f64 {
        (self.target_radius + self.projectile_radius) * (1.0 - self.impact_parameter)
    }

    /// b_crit: past it, the projectile's center misses the target (eq. 6).
    pub fn critical_impact_parameter(&self) -> f64 {
        self.target_radius / (self.target_radius + self.projectile_radius)
    }

    /// α: the share of the projectile that meets the target (eq. 11). The
    /// rest flies past. All of it does when the projectile lies wholly
    /// within the target's outline.
    pub fn interacting_share(&self) -> f64 {
        let (big, r) = (self.target_radius, self.projectile_radius);
        if big > self.impact_parameter * (big + r) + r {
            return 1.0;
        }
        let l = self.overlap();
        (3.0 * r * l * l - l * l * l) / (4.0 * r * r * r)
    }

    /// Q_R: the collision's energy per unit of total mass, J/kg (eq. 1).
    pub fn energy(&self) -> f64 {
        0.5 * self.reduced_mass() * self.speed * self.speed / self.total_mass()
    }

    /// Q′*_RD, J/kg: the head-on value, raised because only the interacting
    /// mass counts (eq. 15).
    pub fn threshold(&self, material: Material) -> f64 {
        let alpha = self.interacting_share();
        let interacting = alpha * self.projectile_mass;
        let reduced = interacting * self.target_mass / (interacting + self.target_mass);
        let head_on = head_on_threshold(self.total_mass(), self.mass_ratio(), material);
        (self.reduced_mass() / reduced).powf(2.0 - 1.5 * material.mu_bar) * head_on
    }

    /// The speed that brings an energy per unit of total mass, m/s (eq. 16).
    fn speed_for(&self, energy: f64) -> f64 {
        (2.0 * energy * self.total_mass() / self.reduced_mass()).sqrt()
    }

    /// V′_esc: the mutual escape speed of the target and what meets it, at
    /// the two bodies' mean density, m/s (eq. 53).
    pub fn merging_speed(&self) -> f64 {
        let volume = |r: f64| 4.0 / 3.0 * PI * r * r * r;
        let density =
            self.total_mass() / (volume(self.target_radius) + volume(self.projectile_radius));
        let mass = self.target_mass + self.interacting_share() * self.projectile_mass;
        let radius = (3.0 * mass / (4.0 * PI * density)).cbrt();
        (2.0 * G * mass / radius).sqrt()
    }

    /// Whether the scaling laws' data cover this collision: bodies held
    /// together by gravity, up to super-Earths, at mass ratios and speeds
    /// the paper reaches.
    pub fn in_range(&self) -> bool {
        self.target_radius >= SMALLEST
            && self.target_mass <= MOST_MASSIVE
            && self.mass_ratio() >= SMALLEST_RATIO
            && self.speed <= FASTEST
    }

    /// What's left of the projectile after a hit-and-run: the part of the
    /// target in its way strikes it, in the reverse impact (eqs. 46–52), kg.
    /// That interacting mass is an order-of-magnitude estimate in the paper,
    /// whose eq. 48 gives it as a volume; it is taken at the target's
    /// density.
    fn surviving_projectile(&self, material: Material) -> f64 {
        let (big, r) = (self.target_radius, self.projectile_radius);
        let l = self.overlap();
        let phi = 2.0 * ((l - r) / r).clamp(-1.0, 1.0).acos();
        let area = r * r * (PI - (phi - phi.sin()) / 2.0);
        let length = 2.0 * (big * big - (big - 0.5 * l).powi(2)).max(0.0).sqrt();
        let density = self.target_mass / (4.0 / 3.0 * PI * big * big * big);
        let striking = (density * area * length).min(self.target_mass);
        if striking <= 0.0 {
            return self.projectile_mass;
        }
        let total = striking + self.projectile_mass;
        let gamma = striking / self.projectile_mass;
        let threshold = head_on_threshold(total, gamma, material);
        let reduced = striking * self.projectile_mass / total;
        let energy = 0.5 * reduced * self.speed * self.speed / total;
        (largest_share(energy / threshold) * total).min(self.projectile_mass)
    }

    /// What this collision leaves, as the paper's Appendix works it out.
    pub fn outcome(&self, material: Material) -> Outcome {
        let total = self.total_mass();
        let energy = self.energy();
        let threshold = self.threshold(material);
        let x = energy / threshold;
        let merging_speed = self.merging_speed();
        // The energies at which the largest remnant is the target's mass,
        // and a tenth of the total, by the universal law.
        let erosion = 2.0 * (self.projectile_mass / total) * threshold;
        let super_catastrophic = SUPER_CATASTROPHIC * threshold;
        let erosion_speed = self.speed_for(erosion);
        let super_catastrophic_speed = self.speed_for(super_catastrophic);
        let grazing = self.impact_parameter > self.critical_impact_parameter();
        let regime = if self.speed < merging_speed {
            Regime::Merging
        } else if self.speed > super_catastrophic_speed {
            Regime::SuperCatastrophic
        } else if self.speed > erosion_speed {
            Regime::Erosion
        } else if grazing {
            Regime::HitAndRun
        } else {
            Regime::PartialAccretion
        };
        let (largest, second) = match regime {
            Regime::Merging => (total, 0.0),
            Regime::HitAndRun => (self.target_mass, self.surviving_projectile(material)),
            _ => {
                let share = largest_share(x);
                (share * total, second_share(share) * total)
            }
        };
        Outcome {
            regime,
            largest,
            second,
            energy,
            threshold,
            disruption_speed: self.speed_for(threshold),
            merging_speed,
            erosion_speed,
            super_catastrophic_speed,
            grazing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two bodies of density 1000 kg/m³, like the paper's rubble piles.
    fn rubble(target_radius: f64, gamma: f64, speed: f64, b: f64) -> Impact {
        let volume = 4.0 / 3.0 * PI * target_radius.powi(3);
        Impact {
            target_mass: 1000.0 * volume,
            projectile_mass: 1000.0 * volume * gamma,
            target_radius,
            projectile_radius: target_radius * gamma.cbrt(),
            speed,
            impact_parameter: b,
        }
    }

    #[test]
    fn the_universal_law_meets_the_power_law() {
        assert_eq!(largest_share(0.0), 1.0);
        assert_eq!(largest_share(1.0), 0.5);
        let below = largest_share(SUPER_CATASTROPHIC - 1e-9);
        let above = largest_share(SUPER_CATASTROPHIC + 1e-9);
        assert!((below - 0.1).abs() < 1e-8 && (above - 0.1).abs() < 1e-8);
        // Above it, the remnant keeps shrinking, more slowly.
        assert!(largest_share(4.0) < 0.1 && largest_share(4.0) > 0.0);
    }

    #[test]
    fn the_interacting_share_runs_from_all_to_none() {
        // Head on, all of it. Just touching, none. The paper's Table 1:
        // equal bodies at b = 0.7 have 0.22 of the projectile meet the
        // target.
        assert_eq!(rubble(1e4, 0.1, 100.0, 0.0).interacting_share(), 1.0);
        assert!(rubble(1e4, 1.0, 100.0, 0.999_999).interacting_share() < 1e-9);
        let alpha = rubble(1e4, 1.0, 100.0, 0.7).interacting_share();
        assert!((alpha - 0.216).abs() < 1e-12, "{alpha}");
        // The critical impact parameter is 1/2 for equal bodies, and
        // approaches 1 for a small projectile.
        assert_eq!(
            rubble(1e4, 1.0, 100.0, 0.0).critical_impact_parameter(),
            0.5
        );
        assert!(rubble(1e4, 1e-3, 100.0, 0.0).critical_impact_parameter() > 0.9);
    }

    #[test]
    fn regimes_follow_each_other_as_the_speed_rises() {
        // Head on, the speed takes it from merging through accretion and
        // erosion to super-catastrophic disruption.
        let regime = |speed: f64| {
            rubble(1e4, 0.25, speed, 0.0)
                .outcome(Material::RUBBLE_PILES)
                .regime
        };
        assert_eq!(regime(5.0), Regime::Merging);
        assert_eq!(regime(20.0), Regime::PartialAccretion);
        assert_eq!(regime(50.0), Regime::Erosion);
        assert_eq!(regime(200.0), Regime::SuperCatastrophic);
        // Grazing, partial accretion gives way to hit-and-run, and the
        // target keeps its mass exactly.
        let glancing = rubble(1e4, 0.25, 50.0, 0.9).outcome(Material::RUBBLE_PILES);
        assert!(glancing.grazing);
        assert_eq!(glancing.regime, Regime::HitAndRun);
        assert_eq!(glancing.largest, rubble(1e4, 0.25, 50.0, 0.9).target_mass);
    }

    #[test]
    fn no_outcome_makes_mass() {
        // The remnants never exceed what collided, at any speed or angle,
        // and a target can't come out of an accretion smaller.
        for k in 0..400 {
            let speed = 2.0 * 1.03f64.powi(k);
            for b in [0.0, 0.3, 0.6, 0.8, 0.95] {
                for gamma in [1e-3, 0.1, 1.0] {
                    let impact = rubble(1e4, gamma, speed, b);
                    let total = impact.total_mass();
                    let outcome = impact.outcome(Material::RUBBLE_PILES);
                    assert!(outcome.largest > 0.0 && outcome.largest <= total);
                    assert!(outcome.second >= 0.0);
                    assert!(outcome.largest + outcome.second <= total * (1.0 + 1e-12));
                    if outcome.regime == Regime::PartialAccretion {
                        assert!(outcome.largest >= impact.target_mass * (1.0 - 1e-12));
                    }
                }
            }
        }
    }

    #[test]
    fn the_range_covers_what_the_data_cover() {
        let earth_on_earth = Impact {
            target_mass: EARTH_MASS,
            projectile_mass: 0.1 * EARTH_MASS,
            target_radius: 6.371e6,
            projectile_radius: 3.4e6,
            speed: 15e3,
            impact_parameter: 0.7,
        };
        assert!(earth_on_earth.in_range());
        let jupiter = Impact {
            target_mass: 318.0 * EARTH_MASS,
            ..earth_on_earth
        };
        assert!(!jupiter.in_range());
        assert!(!rubble(100.0, 0.5, 10.0, 0.0).in_range());
        assert!(!rubble(1e4, 1e-6, 10.0, 0.0).in_range());
        let fast = Impact {
            speed: 200e3,
            ..earth_on_earth
        };
        assert!(!fast.in_range());
    }
}
