//! What Worldline shows up close for each body, and what kind of knowledge
//! each detail is: measured data, a physics model, or a visual aid.

use eframe::egui::Color32;
use worldline_data::ObjectKind;

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
    detail(
        "Sunlight: IAU luminosity, within 0.4 W/m² of the 1360.8 measured at 1 AU",
        Kind::Measured,
    ),
    detail(
        "Light travel time: distance over c, plus gravity's (Shapiro) delay",
        Kind::Model,
    ),
    detail(
        "Solar wind's field: Parker spiral, with 2025's measured average wind",
        Kind::Model,
    ),
    detail(
        "Heliosphere: measured where Voyager 1 and 2 crossed its boundaries",
        Kind::Measured,
    ),
    detail(
        "Heliosphere's shape elsewhere: a symmetric flow model; the tail is unmeasured",
        Kind::Visual,
    ),
];

const ADDED: &[Detail] = &[
    detail(
        "Added in the sandbox, with the mass and size of a solar-system body",
        Kind::Model,
    ),
    ADDED_PATH,
    ADDED_UNFELT,
];

const ADDED_PATH: Detail = detail(
    "Its path: relativistic gravity with the Sun and planets, which it pulls on in turn",
    Kind::Model,
);

const ADDED_UNFELT: Detail = detail(
    "The belts and small moons on fixed orbits don't feel it",
    Kind::Visual,
);

/// For a real object from the notable-objects catalog, added in the
/// sandbox.
fn catalog_details(kind: ObjectKind) -> Vec<Detail> {
    let mut list = vec![
        detail(
            "Mass and size: a real object's published values (sources above)",
            Kind::Measured,
        ),
        ADDED_PATH,
    ];
    list.extend(match kind {
        ObjectKind::BlackHole => [
            detail(
                "Drawn as its shadow seen from afar, a dark disk √27 GM/c² across: how light bends around it comes in step 3.1",
                Kind::Visual,
            ),
            detail(
                "Inside the horizon, physics is unknown: whatever reaches it is absorbed",
                Kind::Model,
            ),
        ],
        ObjectKind::NeutronStar => [
            detail("No surface map: a flat, glowing color", Kind::Visual),
            detail(
                "Its interior is known only through models of nuclear matter (step 2.7)",
                Kind::Model,
            ),
        ],
        ObjectKind::WhiteDwarf | ObjectKind::Star => [
            detail("No surface map: a flat, glowing color", Kind::Visual),
            detail(
                "It shines, but the planets stay lit by the Sun, unless it takes the Sun's place",
                Kind::Visual,
            ),
        ],
    });
    list.push(ADDED_UNFELT);
    list
}

const EARTH: &[Detail] = &[
    detail(
        "Surface and city lights: maps from NASA data",
        Kind::Measured,
    ),
    detail("Clouds: one snapshot, not live weather", Kind::Visual),
    detail("Blue haze and limb glow: Rayleigh scattering", Kind::Model),
    detail("Shape: 0.34% flattened (IAU)", Kind::Measured),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
    detail(
        "Magnetopause: pressure balance of the IGRF dipole with the measured wind",
        Kind::Model,
    ),
    detail(
        "Radiation belts: measured extent, drawn along dipole field lines",
        Kind::Visual,
    ),
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
    detail(
        "Magnetopause: the dipole alone gives about 19 radii; plasma from Enceladus inflates the real one to 22–27",
        Kind::Model,
    ),
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
    detail(
        "Magnetopause: the dipole alone gives about 41 radii; plasma from Io inflates the real one to 63–92",
        Kind::Model,
    ),
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
    /// A body added in the sandbox: a copy of a solar-system body, or a
    /// real object of the given kind from the catalog.
    Added(Option<ObjectKind>),
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
        BodyKind::Added(None) => return ADDED.to_vec(),
        BodyKind::Added(Some(kind)) => return catalog_details(kind),
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
