//! What Worldline shows up close for each body, and what kind of knowledge
//! each detail is: measured data, a physics model, or a visual aid.

use eframe::egui::Color32;

/// Where a detail comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Observed by instruments: spacecraft maps, occultations, orbits.
    Measured,
    /// Computed from a physics model.
    Model,
    /// A visual aid or simplification, not physics.
    Visual,
}

impl Kind {
    pub fn tag(self) -> &'static str {
        match self {
            Kind::Measured => "measured",
            Kind::Model => "model",
            Kind::Visual => "visual",
        }
    }

    pub fn color(self) -> Color32 {
        match self {
            Kind::Measured => Color32::from_rgb(110, 200, 140),
            Kind::Model => Color32::from_rgb(120, 170, 255),
            Kind::Visual => Color32::from_rgb(220, 170, 90),
        }
    }
}

/// One detail shown for a body.
pub struct Detail {
    pub what: &'static str,
    pub kind: Kind,
}

const fn detail(what: &'static str, kind: Kind) -> Detail {
    Detail { what, kind }
}

const SUN: &[Detail] = &[
    detail(
        "Surface map: stylized colors (the real Sun is white), static",
        Kind::Visual,
    ),
    detail("Limb darkening: Eddington approximation", Kind::Model),
    detail(
        "Corona: Baumbach brightness model, boosted ~10⁵× (only seen in eclipses)",
        Kind::Model,
    ),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const EARTH: &[Detail] = &[
    detail(
        "Surface and city lights: maps from NASA data",
        Kind::Measured,
    ),
    detail("Clouds: one snapshot, not live weather", Kind::Visual),
    detail("Blue haze and limb glow: Rayleigh scattering", Kind::Model),
    detail("Shape: 0.34% flattened (IAU)", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const SATURN: &[Detail] = &[
    detail(
        "Rings: Cassini radio occultation (2005), 10 km bins",
        Kind::Measured,
    ),
    detail(
        "Ring transparency: optical depth along the slanted path",
        Kind::Model,
    ),
    detail(
        "Ring brightness: simplified particle-layer model",
        Kind::Model,
    ),
    detail("Shadows: Saturn on its rings, rings on Saturn", Kind::Model),
    detail("Shape: 9.8% flattened (IAU)", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const JUPITER: &[Detail] = &[
    detail(
        "Cloud bands and Great Red Spot: map from NASA data",
        Kind::Measured,
    ),
    detail(
        "Great Red Spot's place on the map is fixed; the real one drifts",
        Kind::Visual,
    ),
    detail("Shape: 6.5% flattened (IAU)", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const VENUS: &[Detail] = &[
    detail(
        "Cloud tops: what you'd see; the surface is hidden",
        Kind::Measured,
    ),
    detail(
        "Spin and tilt: IAU rotation model (retrograde)",
        Kind::Measured,
    ),
];

const PLUTO: &[Detail] = &[
    detail("No global map bundled yet: flat color", Kind::Visual),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const OTHER: &[Detail] = &[
    detail("Surface map from NASA data", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

/// The details Worldline shows for a body when you look closely.
pub fn details(name: &str) -> &'static [Detail] {
    match name {
        "Sun" => SUN,
        "Earth" => EARTH,
        "Saturn" => SATURN,
        "Jupiter" => JUPITER,
        "Venus" => VENUS,
        "Pluto" => PLUTO,
        _ => OTHER,
    }
}
