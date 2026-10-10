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

const GALACTIC_CENTER: &[Detail] = &[
    detail(
        "Mass and distance: from the orbits of the stars around it (GRAVITY 2022)",
        Kind::Measured,
    ),
    detail(
        "Where it is on the sky: radio interferometry (Gordon et al. 2023)",
        Kind::Measured,
    ),
    detail(
        "Held still relative to the solar system: the galaxy's mass, which carries the Sun around it every 230 million years, isn't simulated (v2)",
        Kind::Model,
    ),
    detail(
        "The Milky Way's other stars, gas and dark matter between are not shown (v2)",
        Kind::Visual,
    ),
    detail(
        "Drawn as its shadow seen from afar, a dark disk √27 GM/c² across: how light bends around it comes in step 3.1",
        Kind::Visual,
    ),
];

const S_STAR: &[Detail] = &[
    detail(
        "Orbit: published elements (GRAVITY 2022 for S2, S29, S38 and S55; Gillessen et al. 2017 for the rest)",
        Kind::Measured,
    ),
    detail(
        "Its path: Sagittarius A*'s Newtonian pull plus its first relativistic term, which turns S2's orbit 12′ each time round",
        Kind::Model,
    ),
    detail(
        "A dot colored by type: young, hot stars blue-white, cool giants orange; its own mass is too small to matter here",
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
    "Small moons on mean orbits feel it only once it could tear them away",
    Kind::Model,
);

/// A black hole that formed when two spiraled together.
const MERGER_REMNANT: &[Detail] = &[
    detail(
        "Its mass, spin and kick: numerical-relativity fits for two non-spinning black holes on a near-circular orbit (Jiménez-Forteza et al. 2017; González et al. 2007)",
        Kind::Model,
    ),
    detail(
        "It formed where the post-Newtonian description gave out, a few orbits before the real merger: the plunge and merger aren't simulated",
        Kind::Model,
    ),
    detail(
        "Its kick's direction within the orbital plane isn't modeled: taken along the heavier hole's motion",
        Kind::Model,
    ),
    detail(
        "Drawn as its shadow seen from afar, a dark disk √27 GM/c² across: how light bends around it comes in step 3.1",
        Kind::Visual,
    ),
    ADDED_PATH,
];

/// A hypothetical mini black hole.
const MINI_BLACK_HOLE: &[Detail] = &[
    detail(
        "Hypothetical: a primordial black hole, which may have formed in the early universe; none has been found",
        Kind::Model,
    ),
    detail(
        "Evaporates by Hawking radiation, theoretical: never observed. As the textbook estimate has it: a black body at its temperature, as big as its horizon, emitting photons; real ones would also emit neutrinos, gravitons and heavier particles, faster",
        Kind::Model,
    ),
    detail(
        "Drawn as a dot: its horizon is far smaller than a proton",
        Kind::Visual,
    ),
    ADDED_PATH,
];

/// A neutron star that collapsed into a black hole.
const COLLAPSED_STAR: &[Detail] = &[
    detail(
        "Collapsed into a black hole when it passed the heaviest neutron star the SLy equation of state allows; its mass kept, its spin and the matter flung out not modeled",
        Kind::Model,
    ),
    detail(
        "Drawn as its shadow seen from afar, a dark disk √27 GM/c² across: how light bends around it comes in step 3.1",
        Kind::Visual,
    ),
    ADDED_PATH,
];

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
                "If it grows, its size follows the SLy equation of state, one model of nuclear matter among several that fit the measured neutron stars; past SLy's heaviest star it collapses into a black hole",
                Kind::Model,
            ),
        ],
        ObjectKind::WhiteDwarf => [
            detail("No surface map: a flat, glowing color", Kind::Visual),
            detail(
                "Held up by degenerate electrons: if it grows, it shrinks as an ideal carbon–oxygen white dwarf does, and past Chandrasekhar's limit it explodes (its debris isn't simulated)",
                Kind::Model,
            ),
        ],
        ObjectKind::Star => [
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

const PLUTO: &[Detail] = &[detail(
    "Spin and tilt: the New Horizons team's model, keeping longitude 0° toward Charon as the IAU defines it (NAIF's standard values miss by 1.45°)",
    Kind::Measured,
)];

const MARS_MOONS: &[Detail] = &[
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
    detail(
        "Orbit: its planet's oblateness, the other moons, the Sun's tide",
        Kind::Model,
    ),
    detail("Spin and tilt: IAU rotation model", Kind::Measured),
];

const CHARON: &[Detail] = &[
    detail(
        "Orbit: its planet's oblateness, the other moons, the Sun's tide",
        Kind::Model,
    ),
    detail(
        "Spin and tilt: the New Horizons team's model, keeping longitude 0° toward Pluto as the IAU defines it (NAIF's standard values miss by 1.45°)",
        Kind::Measured,
    ),
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

/// For a body whose map isn't bundled.
const NO_MAP: Detail = detail("No global map bundled yet: flat color", Kind::Visual);

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
    /// A black hole that formed when two spiraled together.
    MergerRemnant,
    /// A neutron star from the catalog that collapsed into a black hole.
    CollapsedStar,
    /// A hypothetical mini black hole, evaporating.
    MiniBlackHole,
    /// Sagittarius A* in the galactic center, or (`star`) a star orbiting
    /// it.
    Galactic { star: bool },
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
        BodyKind::Galactic { star: false } => return GALACTIC_CENTER.to_vec(),
        BodyKind::Galactic { star: true } => return S_STAR.to_vec(),
        BodyKind::Added(Some(kind)) => return catalog_details(kind),
        BodyKind::MergerRemnant => return MERGER_REMNANT.to_vec(),
        BodyKind::CollapsedStar => return COLLAPSED_STAR.to_vec(),
        BodyKind::MiniBlackHole => return MINI_BLACK_HOLE.to_vec(),
        BodyKind::SmallBody {
            comet,
            outgassing,
            massive,
        } => {
            return small_body_details(name, comet, outgassing, massive);
        }
        BodyKind::Other | BodyKind::Moon => {}
    }
    let moon = kind == BodyKind::Moon;
    // These lists leave out the map, which depends on the body.
    let without_map = match name {
        "Pluto" => PLUTO,
        "Phobos" | "Deimos" => MARS_MOONS,
        "Io" | "Europa" | "Ganymede" | "Callisto" => GALILEAN_MOONS,
        "Hyperion" => HYPERION,
        "Charon" => CHARON,
        // Earth's Moon's map is in OTHER's list.
        _ if moon && name != "Moon" => MOON,
        _ => {
            return match name {
                "Sun" => SUN,
                "Earth" => EARTH,
                "Saturn" => SATURN,
                "Jupiter" => JUPITER,
                "Venus" => VENUS,
                _ => OTHER,
            }
            .to_vec();
        }
    };
    let mut list = vec![map_detail(name, NO_MAP)];
    list.extend_from_slice(without_map);
    list
}

/// The body's spacecraft map, if it has one, else `fallback`.
fn map_detail(name: &str, fallback: Detail) -> Detail {
    crate::textures::spacecraft_map(name).map_or(fallback, |what| detail(what, Kind::Measured))
}

fn small_body_details(name: &str, comet: bool, outgassing: bool, massive: bool) -> Vec<Detail> {
    let mut list = vec![map_detail(
        name,
        detail("No map: flat color, or a dot", Kind::Visual),
    )];
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
