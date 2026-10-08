//! Real-world data for Worldline.
//!
//! Every value comes from a published source, and the data files record
//! their own provenance. Scripts in `tools/` regenerate them.

mod atmosphere;
mod belts;
mod galactic_center;
mod heliosphere;
mod kernel;
mod magnetospheres;
mod moons;
mod notable;
mod rings;
mod rotation;
mod small_bodies;
mod small_moons;
mod snapshot;

pub use atmosphere::{Atmosphere, atmosphere};
pub use belts::{Belt, BeltKind, BeltSample, belt_particles, belt_samples, belts};
pub use galactic_center::{
    GalacticCenter, SStar, galactic_center, kepler_state, s_stars, sgr_a_star_direction,
    sgr_a_star_sky_position, year_to_time,
};
pub use heliosphere::{
    Boundary, SolarWindHour, VoyagerCrossing, heliosphere, mean_flow_pressure, parker_spiral,
    solar_wind_2025, voyager_crossings,
};
pub use magnetospheres::{PlanetaryField, RadiationBelt, planetary_fields, radiation_belts};
pub use moons::{
    MoonSystemData, moon_figure, moon_systems, parse_moons, ring_mass, solar_system_with_moons,
    tesseral_harmonics, zonal_harmonics,
};
pub use notable::{
    BinaryOrbit, Distance, Measured, NotableObject, ObjectKind, RadiusBasis, binary_orbits,
    notable_object, notable_objects,
};
pub use rings::{RingFeature, RingProfile, saturn_ring_features, saturn_rings};
pub use rotation::{rotation_model, triaxial_radii};
pub use small_bodies::{
    SmallBody, SmallBodyKind, full_solar_system, jpl_halley_perihelion_jd, small_bodies,
    small_bodies_one_year_on,
};
pub use small_moons::{
    ElementsRow, SmallMoon, SolutionZonal, display_name, inner_moon_mass, moon_elements,
    parse_elements, satellites_in_jpls_list, small_moon_states_30_days_on,
    small_moon_zonal_harmonics, small_moons,
};
pub use snapshot::{ParseError, Snapshot};

/// The solar system at 2025-01-01 00:00:00 TDB: the Sun, the eight planets,
/// the Moon and Pluto, from NASA JPL Horizons (ephemeris DE441) with JPL
/// DE440 gravitational parameters.
pub fn solar_system() -> Snapshot {
    Snapshot::parse(include_str!("../data/solar-system-2025-01-01.csv"))
        .expect("bundled solar-system data is valid")
}
