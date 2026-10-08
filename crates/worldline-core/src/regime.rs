//! Which physics a body's situation calls for: the dimensionless numbers
//! that decide it, and whether the model in use covers them. See
//! `docs/ARCHITECTURE.md` and `docs/physics/collisions.md`.

use crate::Body;
use crate::constants::C;

/// How strong gravity is where a body is, and how fast it moves, relative
/// to what holds it most strongly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Regime {
    /// ε = GM / (r c²): 0 in flat space, ½ at a black hole's horizon.
    pub epsilon: f64,
    /// v / c, its speed relative to that body.
    pub beta: f64,
}

/// Whether a model covers a situation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Validity {
    /// Within range: what the model leaves out is below the integrator's
    /// own tolerance.
    Within,
    /// It holds, but what it leaves out shows: up to about 1% of the force.
    Approximate,
    /// Out of range: the situation needs physics the model doesn't have.
    Beyond,
}

impl Regime {
    /// The regime of `body` relative to `attractor`.
    pub fn of(body: &Body, attractor: &Body) -> Self {
        let r = (body.position - attractor.position).length();
        Self {
            epsilon: attractor.gm / (r * C * C),
            beta: (body.velocity - attractor.velocity).length() / C,
        }
    }

    /// Whether post-Newtonian equations that keep every term through order
    /// `kept` cover it (1 for Einstein–Infeld–Hoffmann, 2 with the pairwise
    /// 2PN terms). The first order left out is of relative size
    /// max(ε, β²)^(kept + 1). Within range while that is below IAS15's
    /// tolerance of 10⁻⁹, approximate while it is below 1%, beyond after:
    /// - kept 1: ε and β² under 3 × 10⁻⁵ within, under 0.1 approximate;
    /// - kept 2: under 10⁻³ within, under 0.2 approximate.
    ///
    /// Beyond that, gravity is strong (near a neutron star or black hole)
    /// and needs higher orders or full general relativity.
    pub fn validity(&self, kept: i32) -> Validity {
        let left_out = self.epsilon.max(self.beta * self.beta).powi(kept + 1);
        if left_out < 1e-9 {
            Validity::Within
        } else if left_out < 1e-2 {
            Validity::Approximate
        } else {
            Validity::Beyond
        }
    }

    /// Whether the first post-Newtonian equations (Einstein–Infeld–
    /// Hoffmann) cover it: [`Regime::validity`] with one order kept.
    pub fn post_newtonian(&self) -> Validity {
        self.validity(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DVec3;
    use crate::constants::{AU, GM_SUN, SOLAR_RADIUS};

    #[test]
    fn the_solar_system_is_well_within_first_order() {
        let sun = Body::new("Sun", GM_SUN, SOLAR_RADIUS);
        // Mercury at perihelion: ε = 3.2 × 10⁻⁸.
        let mercury = Body::new("Mercury", 2.2e13, 2.44e6)
            .at(DVec3::new(0.3075 * AU, 0.0, 0.0))
            .moving(DVec3::new(0.0, 58.98e3, 0.0));
        let regime = Regime::of(&mercury, &sun);
        assert!(
            (regime.epsilon - 3.21e-8).abs() < 0.01e-8,
            "{}",
            regime.epsilon
        );
        assert_eq!(regime.post_newtonian(), Validity::Within);
        // Grazing the Sun's surface at 618 km/s: ε = 2.1 × 10⁻⁶, still well
        // within.
        let comet = Body::new("Comet", 0.0, 0.0)
            .at(DVec3::new(SOLAR_RADIUS, 0.0, 0.0))
            .moving(DVec3::new(-618e3, 0.0, 0.0));
        assert_eq!(Regime::of(&comet, &sun).post_newtonian(), Validity::Within);
        // 10 km from a neutron star of 1.4 Sun masses: ε = 0.21, beyond.
        let star = Body::new("Neutron star", 1.4 * GM_SUN, 1.2e4);
        let close = Body::new("Close", 0.0, 0.0).at(DVec3::new(1e4, 0.0, 0.0));
        assert_eq!(Regime::of(&close, &star).post_newtonian(), Validity::Beyond);
    }

    #[test]
    fn second_order_reaches_closer_in() {
        // The Hulse–Taylor pair at periastron, ε ≈ 5 × 10⁻⁶, is within range
        // either way. At ε = 10⁻⁴ what first order leaves out (10⁻⁸) shows,
        // but not what 2PN leaves out (10⁻¹²). At ε = 10⁻², a pair a few
        // hundred km apart, even 2PN's leftovers show (10⁻⁶); at ε = 0.3
        // they pass 1%.
        let regime = |epsilon: f64| Regime { epsilon, beta: 0.0 };
        assert_eq!(regime(5e-6).validity(1), Validity::Within);
        assert_eq!(regime(5e-6).validity(2), Validity::Within);
        assert_eq!(regime(1e-4).validity(1), Validity::Approximate);
        assert_eq!(regime(1e-4).validity(2), Validity::Within);
        assert_eq!(regime(1e-2).validity(2), Validity::Approximate);
        assert_eq!(regime(0.3).validity(2), Validity::Beyond);
        // The thresholds match the documented ones for first order.
        assert_eq!(regime(2.9e-5).post_newtonian(), Validity::Within);
        assert_eq!(regime(3.3e-5).post_newtonian(), Validity::Approximate);
        assert_eq!(regime(0.099).post_newtonian(), Validity::Approximate);
        assert_eq!(regime(0.101).post_newtonian(), Validity::Beyond);
    }
}
