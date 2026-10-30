//! The references for the sky from a place on Earth (step 5.4): the U.S.
//! Naval Observatory's rise and set of the Sun and Moon in 2026, and where
//! NASA JPL Horizons puts the Sun, Moon and planets. See
//! `tools/fetch_sky.py`.

use worldline_core::timescales::julian_date;

const RISE_SET: &str = include_str!("../data/rise-set-2026.csv");

/// A city in the references.
#[derive(Debug, Clone, PartialEq)]
pub struct Site {
    /// Its key in the tables.
    pub key: String,
    /// Its name.
    pub name: String,
    /// Latitude, degrees north.
    pub latitude: f64,
    /// Longitude, degrees east.
    pub longitude: f64,
}

/// A rise or set in the Observatory's tables.
#[derive(Debug, Clone, PartialEq)]
pub struct RiseSet {
    /// The city's key.
    pub site: String,
    /// "Sun" or "Moon".
    pub body: String,
    /// Rising, or setting.
    pub rising: bool,
    /// When, a Julian Date in Universal Time, to the minute.
    pub jd_ut: f64,
}

/// Where Horizons puts a body in a city's sky.
#[derive(Debug, Clone, PartialEq)]
pub struct SkyPoint {
    /// The city's key.
    pub site: String,
    /// When, a Julian Date in Universal Time.
    pub jd_ut: f64,
    /// The body's name.
    pub body: String,
    /// Apparent azimuth from north through east, degrees.
    pub azimuth: f64,
    /// Apparent elevation without refraction, degrees.
    pub elevation: f64,
}

/// The cities of the references, whose places the Observatory's tables give
/// to the minute of arc.
pub fn sky_sites() -> Vec<Site> {
    RISE_SET
        .lines()
        .filter_map(|line| line.strip_prefix("# site: "))
        .map(|line| {
            let f: Vec<&str> = line.split("; ").collect();
            let angle = |text: &str, label: &str| -> f64 {
                let (degrees, minutes) = text
                    .strip_prefix(label)
                    .and_then(|rest| rest.split_once(' '))
                    .expect("a site's coordinate");
                let degrees: f64 = degrees.parse().expect("degrees");
                let minutes: f64 = minutes.parse().expect("minutes");
                (degrees.abs() + minutes / 60.0).copysign(degrees)
            };
            Site {
                key: f[0].to_string(),
                name: f[1].to_string(),
                latitude: angle(f[2], "latitude "),
                longitude: angle(f[3], "longitude "),
            }
        })
        .collect()
}

/// The Observatory's rises and sets of 2026.
pub fn rise_set_2026() -> Vec<RiseSet> {
    RISE_SET
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let date: Vec<u32> = f[3].split('-').map(|x| x.parse().unwrap()).collect();
            let hours: f64 = f[4][..2].parse().expect("hours");
            let minutes: f64 = f[4][2..].parse().expect("minutes");
            RiseSet {
                site: f[0].to_string(),
                body: f[1].to_string(),
                rising: f[2] == "rise",
                jd_ut: julian_date(
                    date[0] as i32,
                    date[1],
                    date[2],
                    hours * 3600.0 + minutes * 60.0,
                ),
            }
        })
        .collect()
}

/// Where Horizons puts the Sun, Moon and planets.
pub fn sky_from_earth_2026() -> Vec<SkyPoint> {
    include_str!("../data/sky-from-earth-2026.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let number = |i: usize| f[i].parse::<f64>().expect("bundled sky data is valid");
            SkyPoint {
                site: f[0].to_string(),
                jd_ut: number(1),
                body: f[2].to_string(),
                azimuth: number(3),
                elevation: number(4),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_references_load() {
        let sites = sky_sites();
        assert_eq!(sites.len(), 2);
        assert_eq!(sites[0].name, "Washington, DC");
        assert_eq!(sites[0].latitude, 38.0 + 53.0 / 60.0);
        assert_eq!(sites[0].longitude, -(77.0 + 2.0 / 60.0));
        assert_eq!(sites[1].latitude, -(33.0 + 52.0 / 60.0));
        let events = rise_set_2026();
        let count = |site: &str, body: &str, rising: bool| {
            events
                .iter()
                .filter(|e| e.site == site && e.body == body && e.rising == rising)
                .count()
        };
        assert_eq!(count("washington", "Sun", true), 365);
        assert_eq!(count("sydney", "Sun", false), 365);
        // The Moon rises about 50 minutes later each day, so some days it
        // doesn't, in a year of about 353 rises.
        assert!((345..=358).contains(&count("washington", "Moon", true)));
        // New Year's Day's sunrise in Washington: 12:27 UT.
        let first = &events[0];
        assert_eq!(
            first.jd_ut,
            julian_date(2026, 1, 1, 12.0 * 3600.0 + 27.0 * 60.0)
        );
        assert_eq!(sky_from_earth_2026().len(), 2 * 9 * 4);
    }
}
