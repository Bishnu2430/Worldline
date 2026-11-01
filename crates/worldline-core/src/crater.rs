//! Craters (step 6.3): what an impact small against its target digs, by
//! Holsapple's (1993) scaling laws. See `docs/physics/craters.md`.
//!
//! The impactor stands in as a point source, so the crater depends on one
//! combination of its size and speed. Gravity arrests large craters and
//! strength small ones. Equation numbers are the paper's.

use std::f64::consts::PI;

/// What a surface is made of, as the scaling laws need it. `volume` and
/// `strength` are the paper's Table 1, whose π₂ is 3.22 ga/U²
/// ([`GRAVITY_FACTOR`]), as Schmidt & Housen (1987) define it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ground {
    /// K₁ of the volume law (eq. 19).
    pub volume: f64,
    /// K₁ of the excavation radius law (eq. 20), in the same π₂.
    pub radius: f64,
    /// The coupling exponent: 0.55 for nonporous ground, 0.41 for porous.
    pub mu: f64,
    /// Ȳ: the strength that matches strength-dominated craters, Pa.
    pub strength: f64,
    /// ρ, kg/m³.
    pub density: f64,
}

impl Ground {
    /// Soft rock, like the sediments under Meteor Crater. Density from the
    /// paper's Figure 6. The paper gives rock no radius, so it comes from
    /// Collins et al.'s (2005) eq. 21, after Schmidt & Housen (1987): their
    /// constant 1.161 for a sphere is D = 1.6 (m/ρ)^(1/3) π₂^(−0.22).
    pub const SOFT_ROCK: Self = Self {
        volume: 0.20,
        radius: 0.8,
        mu: 0.55,
        strength: 7.6e6,
        density: 2650.0,
    };

    /// Hard rock, like a planet's crystalline crust. Density from Figure 7.
    pub const HARD_ROCK: Self = Self {
        strength: 18e6,
        ..Self::SOFT_ROCK
    };

    /// Dry soil, as the paper takes for the Moon. Density from Figure 4. Its
    /// eq. 21 gives the radius as 0.69 for π₂ = ga/U², which is 0.842 here.
    pub const DRY_SOIL: Self = Self {
        volume: 0.24,
        radius: 0.842,
        mu: 0.41,
        strength: 0.18e6,
        density: 1700.0,
    };

    /// Ice. The paper has no ice. Holsapple (2022) gives all nonporous
    /// ground the same outcome where gravity rules, so this is rock at
    /// cold ice's density. The strength is a stand-in, which matters only
    /// for craters under a few hundred meters.
    pub const ICE: Self = Self {
        density: 930.0,
        ..Self::HARD_ROCK
    };

    /// The ground of a body of mean density `density`, kg/m³. Above 3000 it
    /// is mostly rock, and below it mostly ice: Worldline's choice.
    pub fn of_density(density: f64) -> Self {
        if density >= 3000.0 {
            Self::HARD_ROCK
        } else {
            Self::ICE
        }
    }

    /// α = 3μ/(2 + μ) (eq. 17).
    fn alpha(&self) -> f64 {
        3.0 * self.mu / (2.0 + self.mu)
    }
}

/// How Table 1's π₂ differs from the paper's ga/U²: footnote 9.
pub const GRAVITY_FACTOR: f64 = 3.22;

/// The crater volume is known to "a factor of perhaps two" (p. 351).
pub const VOLUME_SPREAD: f64 = 2.0;

/// The rim radius over the excavation radius, from rim profiles in dry soil
/// (p. 355). The same in either regime.
const RIM_OVER_EXCAVATION: f64 = 1.3;

/// A simple crater's rim radius over its depth below the rim: 2.55 for
/// lunar craters (Pike 1977, p. 356).
const RIM_OVER_DEPTH: f64 = 2.55;

/// R*: where the Moon's simple craters give way to complex ones, m (p. 362).
const MOON_TRANSITION: f64 = 8.5e3;

/// The Moon's surface gravity, m/s². The transition goes as 1/g (p. 358).
const MOON_GRAVITY: f64 = 1.62;

/// The exponent of a complex crater's enlargement (eq. 28).
const ENLARGEMENT: f64 = 0.079;

/// Below this speed straight down, m/s, the data are sketchy and the point
/// source fails (Holsapple 2022).
pub const SLOWEST: f64 = 1e3;

/// A body hitting the ground.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Strike {
    /// Its mass, kg.
    pub mass: f64,
    /// Its speed, m/s.
    pub speed: f64,
    /// b: the sine of the angle from straight down, as in [`crate::impact`].
    pub impact_parameter: f64,
    /// The target's surface gravity, m/s².
    pub gravity: f64,
}

impl Strike {
    /// The speed straight down, which is what counts (Holsapple 2022), m/s.
    pub fn downward_speed(&self) -> f64 {
        let b = self.impact_parameter;
        self.speed * (1.0 - b * b).max(0.0).sqrt()
    }

    /// The bracket of eqs. 19 and 20: Table 1's π₂, then strength's term. The
    /// paper's π₂ (δ/ρ)^(1/3) is g (3m/4πρ)^(1/3)/U², so only the impactor's
    /// mass counts, not its density: the coupling exponent ν is 1/3.
    /// Returns the two terms.
    fn terms(&self, ground: &Ground) -> (f64, f64) {
        let u2 = self.downward_speed().powi(2);
        let size = (3.0 * self.mass / (4.0 * PI * ground.density)).cbrt();
        let gravity = GRAVITY_FACTOR * self.gravity * size / u2;
        let strength = ground.strength / (ground.density * u2);
        (gravity, strength.powf(0.5 * (2.0 + ground.mu)))
    }
}

/// What an impact digs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crater {
    /// The excavation's radius at the original ground, m.
    pub excavation_radius: f64,
    /// The rim's radius once the crater has settled, m.
    pub rim_radius: f64,
    /// Its floor's depth below the rim, m.
    pub depth: f64,
    /// The volume dug out, m³.
    pub volume: f64,
    /// Whether it collapsed into a complex crater.
    pub complex: bool,
    /// Whether strength stopped it rather than gravity.
    pub strength_stopped: bool,
}

impl Crater {
    /// The factor either way on the rim's radius, for a volume known to
    /// [`VOLUME_SPREAD`]. A complex crater's rim goes as a power above 1 of
    /// its excavation.
    pub fn spread(&self) -> f64 {
        let power = if self.complex {
            1.0 / (1.0 - ENLARGEMENT)
        } else {
            1.0
        };
        VOLUME_SPREAD.powf(power / 3.0)
    }
}

/// What `strike` digs in `ground`: the volume by eq. 19, the excavation by
/// eq. 20, the rim by eq. 22b, and past the transition the collapse by
/// eqs. 28 and 29.
pub fn dig(strike: &Strike, ground: &Ground) -> Crater {
    let (gravity, strength) = strike.terms(ground);
    let bracket = gravity + strength;
    let volume = ground.volume * strike.mass / ground.density * bracket.powf(-ground.alpha());
    let excavation_radius = ground.radius
        * (strike.mass / ground.density).cbrt()
        * bracket.powf(-ground.mu / (2.0 + ground.mu));
    let simple_rim = RIM_OVER_EXCAVATION * excavation_radius;
    let transition = MOON_TRANSITION * MOON_GRAVITY / strike.gravity;
    let complex = simple_rim > transition;
    let (rim_radius, depth) = if complex {
        // R/R_e = 1.32 (R/R*)^0.079, solved for R.
        let rim = (1.32 * excavation_radius * transition.powf(-ENLARGEMENT))
            .powf(1.0 / (1.0 - ENLARGEMENT));
        (rim, 0.313 * transition * (rim / transition).powf(0.301))
    } else {
        (simple_rim, simple_rim / RIM_OVER_DEPTH)
    };
    Crater {
        excavation_radius,
        rim_radius,
        depth,
        volume,
        complex,
        strength_stopped: strength > gravity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EARTH_GRAVITY: f64 = 9.81;

    fn strike(mass: f64, speed: f64) -> Strike {
        Strike {
            mass,
            speed,
            impact_parameter: 0.0,
            gravity: EARTH_GRAVITY,
        }
    }

    #[test]
    fn gravity_rules_large_craters_and_strength_small_ones() {
        let ground = Ground::HARD_ROCK;
        assert!(dig(&strike(1e3, 1e4), &ground).strength_stopped);
        assert!(!dig(&strike(1e12, 1e4), &ground).strength_stopped);
        // Where strength rules, the volume goes as the mass (Table 1).
        let (a, b) = (
            dig(&strike(1.0, 1e4), &ground),
            dig(&strike(2.0, 1e4), &ground),
        );
        assert!((b.volume / a.volume - 2.0).abs() < 2e-3, "{a:?} {b:?}");
    }

    #[test]
    fn a_slanted_impact_digs_less() {
        let ground = Ground::HARD_ROCK;
        let straight = dig(&strike(1e12, 1e4), &ground);
        let slanted = dig(
            &Strike {
                impact_parameter: 0.8,
                ..strike(1e12, 1e4)
            },
            &ground,
        );
        assert!(slanted.rim_radius < straight.rim_radius);
        // Straight down at 6 km/s is the same crater.
        let same = dig(&strike(1e12, 6e3), &ground);
        assert!((slanted.rim_radius / same.rim_radius - 1.0).abs() < 1e-12);
    }

    #[test]
    fn the_transition_goes_as_one_over_gravity() {
        // Earth's: the paper says 3 km across. Scaling the Moon's gives 2.8.
        let across = 2.0 * MOON_TRANSITION * MOON_GRAVITY / EARTH_GRAVITY;
        assert!((across - 2807.0).abs() < 1.0, "{across}");
        let ground = Ground::HARD_ROCK;
        assert!(!dig(&strike(1e8, 1e4), &ground).complex);
        assert!(dig(&strike(1e13, 1e4), &ground).complex);
    }

    #[test]
    fn the_dry_soil_radius_is_the_papers() {
        // 0.69 for ga/U², and in Table 1's π₂: 0.69 × 3.22^(μ/(2 + μ)).
        let ground = Ground::DRY_SOIL;
        let wanted = 0.69 * GRAVITY_FACTOR.powf(ground.mu / (2.0 + ground.mu));
        assert!((ground.radius - wanted).abs() < 5e-4, "{wanted}");
    }

    #[test]
    fn rock_is_rock_and_the_rest_is_ice() {
        assert_eq!(Ground::of_density(5514.0), Ground::HARD_ROCK);
        assert_eq!(Ground::of_density(1860.0), Ground::ICE);
    }
}
