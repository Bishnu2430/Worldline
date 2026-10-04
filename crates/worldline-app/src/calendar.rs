//! Converting Julian Dates to calendar dates.

use std::fmt;

/// A Gregorian calendar date and time, to the minute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl DateTime {
    /// Converts a Julian Date to a Gregorian calendar date, rounded to the
    /// nearest minute. Meeus, *Astronomical Algorithms*, 2nd ed., chapter 7.
    /// Valid from the Gregorian reform, 1582-10-15 (JD 2299160.5), onward.
    pub fn from_julian_date(jd: f64) -> Self {
        // Julian days start at noon; shift by half a day so days start at
        // midnight, and round to the minute before splitting.
        let minutes = ((jd + 0.5) * 1440.0).round() as i64;
        let z = minutes.div_euclid(1440) as f64;
        let minute_of_day = minutes.rem_euclid(1440) as u32;

        let alpha = ((z - 1_867_216.25) / 36_524.25).floor();
        let a = z + 1.0 + alpha - (alpha / 4.0).floor();
        let b = a + 1524.0;
        let c = ((b - 122.1) / 365.25).floor();
        let d = (365.25 * c).floor();
        let e = ((b - d) / 30.6001).floor();

        let day = (b - d - (30.6001 * e).floor()) as u32;
        let month = if e < 14.0 { e - 1.0 } else { e - 13.0 } as u32;
        let year = if month > 2 { c - 4716.0 } else { c - 4715.0 } as i64;
        Self {
            year,
            month,
            day,
            hour: minute_of_day / 60,
            minute: minute_of_day % 60,
        }
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        let cases = [
            (2_460_676.5, "2025-01-01 00:00"),  // Worldline's starting snapshot
            (2_461_041.75, "2026-01-01 06:00"), // one Julian year later
            (2_451_545.0, "2000-01-01 12:00"),  // the J2000 epoch
            (2_436_116.31, "1957-10-04 19:26"), // Sputnik 1 (Meeus example 7.c)
            (2_299_160.5, "1582-10-15 00:00"),  // first day of the Gregorian calendar
        ];
        for (jd, expected) in cases {
            assert_eq!(
                DateTime::from_julian_date(jd).to_string(),
                expected,
                "JD {jd}"
            );
        }
    }

    #[test]
    fn rounding_to_the_minute_carries_into_the_next_day() {
        // 23:59:59.9 rounds to midnight of the next day, not 24:00.
        let almost_midnight = 2_460_676.5 + 1.0 - 0.1 / 86_400.0;
        assert_eq!(
            DateTime::from_julian_date(almost_midnight).to_string(),
            "2025-01-02 00:00"
        );
    }
}
