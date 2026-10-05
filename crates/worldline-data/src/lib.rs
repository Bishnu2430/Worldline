//! Real-world data for Worldline.
//!
//! Every value comes from a published source, and the data files record
//! their own provenance. Scripts in `tools/` regenerate them.

mod atmosphere;
mod kernel;
mod rings;
mod rotation;
mod snapshot;

pub use atmosphere::{Atmosphere, atmosphere};
pub use rings::{RingFeature, RingProfile, saturn_ring_features, saturn_rings};
pub use rotation::{rotation_model, triaxial_radii};
pub use snapshot::{ParseError, Snapshot};

/// The solar system at 2025-01-01 00:00:00 TDB: the Sun, the eight planets,
/// the Moon and Pluto, from NASA JPL Horizons (ephemeris DE441) with JPL
/// DE440 gravitational parameters.
pub fn solar_system() -> Snapshot {
    Snapshot::parse(include_str!("../data/solar-system-2025-01-01.csv"))
        .expect("bundled solar-system data is valid")
}
