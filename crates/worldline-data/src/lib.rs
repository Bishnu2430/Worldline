//! Real-world data for Worldline.
//!
//! Every value comes from a published source, and the data files record
//! their own provenance. `tools/fetch_solar_system.py` regenerates them.

mod snapshot;

pub use snapshot::{ParseError, Snapshot};

/// The solar system at 2025-01-01 00:00:00 TDB: the Sun, the eight planets,
/// the Moon and Pluto, from NASA JPL Horizons (ephemeris DE441) with JPL
/// DE440 gravitational parameters.
pub fn solar_system() -> Snapshot {
    Snapshot::parse(include_str!("../data/solar-system-2025-01-01.csv"))
        .expect("bundled solar-system data is valid")
}
