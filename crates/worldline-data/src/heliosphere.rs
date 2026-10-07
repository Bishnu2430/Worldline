//! The Sun's reach: a year of solar wind measured at Earth, and where the
//! Voyagers crossed the heliosphere's boundaries.
//!
//! `tools/fetch_solar_wind.py` and `tools/fetch_heliosphere.py` regenerate
//! the data. See `docs/physics/sun-reach.md`.

use worldline_core::DVec3;
use worldline_core::constants::{AU, DAY};
use worldline_core::solar_wind::{Heliosphere, ParkerSpiral};

/// One hour of solar wind at Earth, from NASA's OMNI data. Missing values
/// are `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct SolarWindHour {
    /// The hour it starts, UTC, as `YYYY-MM-DDTHH`.
    pub hour: String,
    /// The interplanetary magnetic field in GSE coordinates (x toward the
    /// Sun, y in the ecliptic opposite to Earth's motion, z to ecliptic
    /// north), in T.
    pub field: Option<DVec3>,
    /// The wind's speed, in m/s.
    pub speed: Option<f64>,
    /// Protons per m³.
    pub density: Option<f64>,
    /// The flow (dynamic) pressure ρv², in Pa, as OMNI computes it,
    /// helium included.
    pub flow_pressure: Option<f64>,
}

/// Every hour of 2025.
pub fn solar_wind_2025() -> Vec<SolarWindHour> {
    include_str!("../data/solar-wind-2025.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let value = |i: usize| (!f[i].is_empty()).then(|| f[i].parse::<f64>().expect("number"));
            let field = match (value(1), value(2), value(3)) {
                (Some(x), Some(y), Some(z)) => Some(DVec3::new(x, y, z) * 1e-9),
                _ => None,
            };
            SolarWindHour {
                hour: f[0].to_string(),
                field,
                speed: value(4).map(|v| v * 1e3),
                density: value(5).map(|n| n * 1e6),
                flow_pressure: value(6).map(|p| p * 1e-9),
            }
        })
        .collect()
}

/// The Parker spiral with 2025's mean solar wind: its mean speed and
/// density at Earth, and the Sun's sidereal rotation rate from its IAU
/// rotation model.
pub fn parker_spiral() -> ParkerSpiral {
    let hours = solar_wind_2025();
    let mean = |values: Vec<f64>| values.iter().sum::<f64>() / values.len() as f64;
    let sun = crate::rotation_model("Sun").expect("the Sun's rotation model");
    ParkerSpiral {
        speed: mean(hours.iter().filter_map(|h| h.speed).collect()),
        rotation_rate: sun.spin_rate().to_radians() / DAY,
        density_at_1au: mean(hours.iter().filter_map(|h| h.density).collect()),
    }
}

/// 2025's average flow pressure of the solar wind at Earth (Pa), helium
/// included.
pub fn mean_flow_pressure() -> f64 {
    let pressures: Vec<f64> = solar_wind_2025()
        .iter()
        .filter_map(|h| h.flow_pressure)
        .collect();
    pressures.iter().sum::<f64>() / pressures.len() as f64
}

/// Which boundary a Voyager crossed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Where the solar wind drops from supersonic to subsonic.
    TerminationShock,
    /// Where the solar wind meets the interstellar gas.
    Heliopause,
}

/// A Voyager crossing one of the heliosphere's boundaries.
#[derive(Debug, Clone, PartialEq)]
pub struct VoyagerCrossing {
    /// "Voyager 1" or "Voyager 2".
    pub spacecraft: String,
    /// Which boundary it crossed.
    pub boundary: Boundary,
    /// The day, as `YYYY-MM-DD`.
    pub date: String,
    /// The distance from the Sun its team published, in m.
    pub published_distance: f64,
    /// Its position relative to the Sun that day, from JPL Horizons, in m.
    pub position: DVec3,
    /// Where the crossing was published.
    pub source: String,
}

const CROSSINGS: &str = include_str!("../data/voyager-crossings.csv");

/// The four crossings, in time order.
pub fn voyager_crossings() -> Vec<VoyagerCrossing> {
    CROSSINGS
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            // The source, last, may itself contain commas.
            let f: Vec<&str> = line.splitn(8, ',').collect();
            let au = |i: usize| f[i].parse::<f64>().expect("number") * AU;
            VoyagerCrossing {
                spacecraft: f[0].to_string(),
                boundary: match f[1] {
                    "termination shock" => Boundary::TerminationShock,
                    "heliopause" => Boundary::Heliopause,
                    other => panic!("unknown boundary `{other}`"),
                },
                date: f[2].to_string(),
                published_distance: au(3),
                position: DVec3::new(au(4), au(5), au(6)),
                source: f[7].to_string(),
            }
        })
        .collect()
}

/// The heliosphere: its nose toward where the interstellar wind comes
/// from (IBEX), and each boundary's shape fitted through the Voyager
/// crossings.
pub fn heliosphere() -> Heliosphere {
    let (lon, lat) = CROSSINGS
        .lines()
        .find_map(|l| l.strip_prefix("# nose_ecliptic_lon_lat_deg:"))
        .and_then(|v| {
            let mut numbers = v.split(['(', ',']).map(|s| s.trim().parse::<f64>());
            Some((numbers.next()?.ok()?, numbers.next()?.ok()?))
        })
        .expect("the data file records the nose");
    let (lon, lat) = (lon.to_radians(), lat.to_radians());
    let nose = DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin());
    let crossings = voyager_crossings();
    let through = |boundary: Boundary| -> Vec<DVec3> {
        crossings
            .iter()
            .filter(|c| c.boundary == boundary)
            .map(|c| c.position)
            .collect()
    };
    Heliosphere {
        nose,
        termination_shock: Heliosphere::fit(nose, &through(Boundary::TerminationShock)),
        heliopause: Heliosphere::fit(nose, &through(Boundary::Heliopause)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_loads() {
        let hours = solar_wind_2025();
        assert_eq!(hours.len(), 8760);
        assert_eq!(hours[0].hour, "2025-01-01T00");
        assert_eq!(hours[0].speed, Some(427e3));
        let crossings = voyager_crossings();
        assert_eq!(crossings.len(), 4);
        assert_eq!(crossings[0].source, "Stone et al. 2005, Science 309, 2017");
        // The interstellar wind comes from 5.16° north of the ecliptic.
        let shell = heliosphere();
        assert!((shell.nose.z.asin().to_degrees() - 5.16).abs() < 1e-9);
    }
}
