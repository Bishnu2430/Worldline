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
#[derive(Clone, Copy)]
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

const MARS_MOONS: &[Detail] = &[
    detail("No global map bundled yet: flat color", Kind::Visual),
    detail(
        "Orbit: Mars's lumpy gravity (the Tharsis bulge) and oblateness, the Sun's tide",
        Kind::Model,
    ),
    detail(
        "Shape: stretched toward Mars (J2, C22 from JPL), which tugs on the orbit",
        Kind::Model,
    ),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const GALILEAN_MOONS: &[Detail] = &[
    detail("No global map bundled yet: flat color", Kind::Visual),
    detail(
        "Orbit: Jupiter's oblateness, the other moons (Laplace resonance), the Sun's tide",
        Kind::Model,
    ),
    detail(
        "Shape: stretched toward Jupiter (J2, C22 from JPL), which tugs on the orbit",
        Kind::Model,
    ),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const HYPERION: &[Detail] = &[
    detail("No global map bundled yet: flat color", Kind::Visual),
    detail(
        "Orbit: Saturn's oblateness and rings' mass, Titan's pull, the Sun's tide",
        Kind::Model,
    ),
    detail(
        "Tumbles chaotically: no rotation model exists, so it is drawn without spin",
        Kind::Visual,
    ),
];

const MOON: &[Detail] = &[
    detail("No global map bundled yet: flat color", Kind::Visual),
    detail(
        "Orbit: its planet's oblateness, the other moons, the Sun's tide",
        Kind::Model,
    ),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const SMALL_MOON_IN_DETAIL: &[Detail] = &[
    detail(
        "No map: flat color, or a dot if its size isn't measured",
        Kind::Visual,
    ),
    detail(
        "Orbit: computed in detail, following its planet's major moons (its planet is in focus)",
        Kind::Model,
    ),
    detail(
        "Too small to pull on the planet or major moons; small moons with measured masses pull on each other",
        Kind::Model,
    ),
];

const SMALL_MOON_ON_MEAN_ORBIT: &[Detail] = &[
    detail(
        "No map: flat color, or a dot if its size isn't measured",
        Kind::Visual,
    ),
    detail(
        "Orbit: JPL's mean elements, an approximation. Fly to its planet to compute it in detail.",
        Kind::Model,
    ),
];

const OTHER: &[Detail] = &[
    detail("Surface map from NASA data", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

/// What kind of body the inspector is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    /// The Sun, a planet or a dwarf planet.
    Other,
    /// A major moon.
    Moon,
    /// A small moon around the planet in focus, computed in detail.
    SmallMoonInDetail,
    /// A small moon placed by its mean orbit.
    SmallMoonOnMeanOrbit,
    /// A dwarf planet, asteroid or comet.
    SmallBody {
        /// A comet (or interstellar visitor) rather than an asteroid.
        comet: bool,
        /// Whether JPL's orbit includes non-gravitational forces.
        outgassing: bool,
        /// Whether it is heavy enough to pull on the planets.
        massive: bool,
    },
}

/// The details Worldline shows for a body when you look closely.
pub fn details(name: &str, kind: BodyKind) -> Vec<Detail> {
    match kind {
        BodyKind::SmallMoonInDetail => return SMALL_MOON_IN_DETAIL.to_vec(),
        BodyKind::SmallMoonOnMeanOrbit => return SMALL_MOON_ON_MEAN_ORBIT.to_vec(),
        BodyKind::SmallBody {
            comet,
            outgassing,
            massive,
        } => {
            return small_body_details(comet, outgassing, massive);
        }
        BodyKind::Other | BodyKind::Moon => {}
    }
    let moon = kind == BodyKind::Moon;
    match name {
        "Sun" => SUN,
        "Earth" => EARTH,
        "Saturn" => SATURN,
        "Jupiter" => JUPITER,
        "Venus" => VENUS,
        "Pluto" => PLUTO,
        "Phobos" | "Deimos" => MARS_MOONS,
        "Io" | "Europa" | "Ganymede" | "Callisto" => GALILEAN_MOONS,
        "Hyperion" => HYPERION,
        // Earth's Moon has a map; the others don't yet.
        _ if moon && name != "Moon" => MOON,
        _ => OTHER,
    }
    .to_vec()
}

fn small_body_details(comet: bool, outgassing: bool, massive: bool) -> Vec<Detail> {
    let mut list = vec![detail("No map: flat color, or a dot", Kind::Visual)];
    list.push(if massive {
        detail(
            "Orbit: relativistic gravity among the Sun, planets and heaviest asteroids; heavy enough to pull on the planets (JPL DE440's mass)",
            Kind::Model,
        )
    } else {
        detail(
            "Orbit: follows the Sun, planets and heaviest asteroids (Newtonian, plus the Sun's relativistic term); too light to pull on them",
            Kind::Model,
        )
    });
    if outgassing {
        list.push(if comet {
            detail(
                "Outgassing pushes it: JPL's non-gravitational model (Marsden, Sekanina & Yeomans 1973)",
                Kind::Model,
            )
        } else {
            detail(
                "Sunlight warming its surface pushes it (the Yarkovsky effect): JPL's fitted model",
                Kind::Model,
            )
        });
    }
    list.push(detail(
        "Starting state: JPL Horizons small-body solution",
        Kind::Measured,
    ));
    list
}
