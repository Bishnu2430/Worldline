//! A gravitating body.

use glam::DVec3;

use crate::constants::G;

/// A body, treated as a point mass for gravity.
///
/// Mass is stored as the gravitational parameter μ = GM rather than in
/// kilograms. Orbits measure GM directly, to about 10 significant figures,
/// while G itself is known to only about 5. Storing GM keeps the precise
/// number precise. NASA JPL publishes planetary GMs for the same reason.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// Display name.
    pub name: String,
    /// Gravitational parameter μ = GM, in m³/s².
    pub gm: f64,
    /// Physical radius, in m. Point-mass gravity ignores it; collisions and rendering use it.
    pub radius: f64,
    /// Position, in m.
    pub position: DVec3,
    /// Velocity, in m/s.
    pub velocity: DVec3,
}

impl Body {
    /// Creates a body at rest at the origin from its gravitational parameter
    /// (m³/s²) and radius (m).
    pub fn new(name: impl Into<String>, gm: f64, radius: f64) -> Self {
        assert!(
            gm >= 0.0 && gm.is_finite(),
            "GM must be finite and non-negative"
        );
        assert!(
            radius >= 0.0 && radius.is_finite(),
            "radius must be finite and non-negative"
        );
        Self {
            name: name.into(),
            gm,
            radius,
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
        }
    }

    /// Returns this body moved to `position` (m).
    pub fn at(mut self, position: DVec3) -> Self {
        self.position = position;
        self
    }

    /// Returns this body with velocity `velocity` (m/s).
    pub fn moving(mut self, velocity: DVec3) -> Self {
        self.velocity = velocity;
        self
    }

    /// Mass in kg, derived as GM / G. Carries G's 22 ppm uncertainty.
    pub fn mass(&self) -> f64 {
        self.gm / G
    }
}
