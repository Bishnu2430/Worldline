//! The tables that turn the computer's clock (UTC) into the simulation's
//! time (TDB) and Earth's rotation (UT1), from the IERS (step 5.1): leap
//! seconds, and UT1 − UTC day by day, measured and then predicted.

use worldline_core::timescales::{MJD_OFFSET, TimeScales};

/// The data rows of a bundled CSV file: its comment lines and header left
/// out, each row split at its commas.
fn rows(text: &str) -> impl Iterator<Item = Vec<&str>> {
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .skip(1)
        .map(|line| line.split(',').collect())
}

/// Leap seconds (IERS Bulletin C 72, as IANA republishes it) and UT1 − UTC
/// (IERS Bulletin A, from 2024-12-01 to a year past the day they were
/// fetched; see `tools/fetch_time.py`).
pub fn time_scales() -> TimeScales {
    let number = |s: &str| s.trim().parse::<f64>().expect("bundled time data is valid");
    let leap_seconds = rows(include_str!("../data/leap-seconds.csv"))
        .map(|row| (number(row[0]) + MJD_OFFSET, number(row[1])))
        .collect();
    let ut1: Vec<(f64, f64, bool)> = rows(include_str!("../data/ut1-utc.csv"))
        .map(|row| {
            (
                number(row[0]) + MJD_OFFSET,
                number(row[1]),
                row[2].trim() == "P",
            )
        })
        .collect();
    TimeScales::new(leap_seconds, &ut1)
}

/// Where the Sun is overhead on Earth by NASA JPL Horizons (with the
/// IERS's Earth orientation): for each moment (Julian Date, TT), the Sun's
/// geocentric apparent right ascension and declination (degrees) and
/// Greenwich apparent sidereal time (hours). See `tools/fetch_today.py`.
pub fn sun_overhead() -> Vec<(f64, f64, f64, f64)> {
    let number = |s: &str| s.trim().parse::<f64>().expect("bundled data is valid");
    rows(include_str!("../data/sun-overhead.csv"))
        .map(|row| {
            (
                number(row[0]),
                number(row[1]),
                number(row[2]),
                number(row[3]),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tables_load() {
        let scales = time_scales();
        // Since 2017-01-01, 37 leap seconds; before, 36.
        assert_eq!(scales.tai_minus_utc(2_457_754.5), 37.0);
        assert_eq!(scales.tai_minus_utc(2_457_754.4), 36.0);
        let (until, measured) = scales.ut1_known_until();
        assert!(until > measured && measured > 2_461_000.0);
        assert_eq!(sun_overhead().len(), 2);
    }
}
