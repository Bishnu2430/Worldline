//! Time scales (step 5.1): from the computer's clock (UTC) to the
//! simulation's time (TDB), and Earth's rotation (UT1). See
//! `docs/physics/time.md`.
//!
//! Following Kaplan (2005), *USNO Circular 179*, chapter 2:
//! - TAI = UTC + ΔAT, ΔAT the leap seconds so far (eq. 2.1);
//! - TT = TAI + 32.184 s (eq. 2.4);
//! - TDB = TT plus periodic terms under 2 ms (eq. 2.6);
//! - UT1 = UTC + (UT1 − UTC), from the IERS (eq. 2.7), and
//!   ΔT = TT − UT1 = 32.184 s + ΔAT − (UT1 − UTC) (eq. 2.9);
//! - the Earth Rotation Angle from UT1 (eq. 2.11).
//!
//! Dates are Julian Dates in the time scale named; `jd_utc` counts UTC
//! days of 86,400 s, as computers do.

use std::f64::consts::TAU;

use crate::constants::{DAY, J2000_JD};

/// TT − TAI, s (Kaplan 2005 eq. 2.4).
pub const TT_MINUS_TAI: f64 = 32.184;

/// Julian Date = Modified Julian Date + this.
pub const MJD_OFFSET: f64 = 2_400_000.5;

/// The Julian Date (UTC) the computer's clock counts from, 1970-01-01 0h.
pub const UNIX_EPOCH_JD: f64 = 2_440_587.5;

/// The Julian Date of a Gregorian calendar date and `seconds` into it:
/// Meeus, *Astronomical Algorithms*, 2nd ed., eq. 7.1.
pub fn julian_date(year: i32, month: u32, day: u32, seconds: f64) -> f64 {
    let (y, m) = if month <= 2 {
        (f64::from(year - 1), f64::from(month + 12))
    } else {
        (f64::from(year), f64::from(month))
    };
    let a = (y / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y + 4716.0)).floor() + (30.6001 * (m + 1.0)).floor() + f64::from(day) + b - 1524.5
        + seconds / DAY
}

/// The Julian Date (UTC) `seconds` after the computer's epoch: Unix time
/// leaves leap seconds out, so it counts UTC days of 86,400 s.
pub fn jd_from_unix(seconds: f64) -> f64 {
    UNIX_EPOCH_JD + seconds / DAY
}

/// TDB − TT, s, at Julian Date `jd_tt` (TT): the truncated series of
/// Kaplan (2005) eq. 2.6, after Fairhead & Bretagnon (1990), good to about
/// 10 µs from 1600 to 2200.
pub fn tdb_minus_tt(jd_tt: f64) -> f64 {
    let t = (jd_tt - J2000_JD) / 36_525.0;
    0.001_657 * (628.3076 * t + 6.2401).sin()
        + 0.000_022 * (575.3385 * t + 4.2970).sin()
        + 0.000_014 * (1256.6152 * t + 6.1969).sin()
        + 0.000_005 * (606.9777 * t + 4.0212).sin()
        + 0.000_005 * (52.9691 * t + 0.4444).sin()
        + 0.000_002 * (21.3299 * t + 5.5431).sin()
        + 0.000_010 * t * (628.3076 * t + 4.2490).sin()
}

/// The Earth Rotation Angle at Julian Date `jd_ut1` (UT1), radians in
/// [0, 2π): θ = 0.7790572732640 + 0.00273781191135448 D_U + frac(JD(UT1))
/// turns, D_U = JD(UT1) − 2451545.0 (Kaplan 2005 eq. 2.11).
pub fn earth_rotation_angle(jd_ut1: f64) -> f64 {
    let days = jd_ut1 - J2000_JD;
    let turns = 0.779_057_273_264_0 + 0.002_737_811_911_354_48 * days + jd_ut1.fract();
    turns.rem_euclid(1.0) * TAU
}

/// Where UT1 − UTC comes from, at a moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ut1Source {
    /// The IERS's measured value.
    Measured,
    /// The IERS's prediction.
    Predicted,
    /// Outside the IERS's table: the nearest value in it, held.
    Held,
}

/// The tables that link UTC to the other time scales: leap seconds and
/// UT1 − UTC, from the IERS.
#[derive(Debug, Clone, PartialEq)]
pub struct TimeScales {
    /// From each Julian Date (UTC) on, TAI − UTC (s), in order.
    leap_seconds: Vec<(f64, f64)>,
    /// For each day (Julian Date, UTC), UT1 − TAI (s): continuous across
    /// leap seconds, unlike UT1 − UTC; and whether it is predicted.
    ut1: Vec<(f64, f64, bool)>,
}

impl TimeScales {
    /// From `leap_seconds`, each Julian Date (UTC) from which TAI − UTC
    /// (s) holds, and `ut1_minus_utc`, each day's (Julian Date, UTC) UT1 −
    /// UTC (s) and whether it is predicted; both in order.
    pub fn new(leap_seconds: Vec<(f64, f64)>, ut1_minus_utc: &[(f64, f64, bool)]) -> Self {
        let mut scales = Self {
            leap_seconds,
            ut1: Vec::new(),
        };
        scales.ut1 = ut1_minus_utc
            .iter()
            .map(|&(jd, dut1, predicted)| (jd, dut1 - scales.tai_minus_utc(jd), predicted))
            .collect();
        scales
    }

    /// TAI − UTC (s), the leap seconds so far, at Julian Date `jd_utc`.
    /// Before the table's first entry, its first value.
    pub fn tai_minus_utc(&self, jd_utc: f64) -> f64 {
        self.leap_seconds
            .iter()
            .rev()
            .find(|(from, _)| *from <= jd_utc)
            .or(self.leap_seconds.first())
            .map_or(0.0, |(_, seconds)| *seconds)
    }

    /// The Julian Date (TDB) at Julian Date `jd_utc` (UTC).
    pub fn tdb(&self, jd_utc: f64) -> f64 {
        let jd_tt = jd_utc + (self.tai_minus_utc(jd_utc) + TT_MINUS_TAI) / DAY;
        jd_tt + tdb_minus_tt(jd_tt) / DAY
    }

    /// The Julian Date (UTC) at Julian Date `jd_tdb` (TDB): [`Self::tdb`]
    /// undone, by iterating it.
    pub fn utc(&self, jd_tdb: f64) -> f64 {
        let mut jd_utc = jd_tdb - (TT_MINUS_TAI + self.tai_minus_utc(jd_tdb)) / DAY;
        for _ in 0..3 {
            jd_utc += jd_tdb - self.tdb(jd_utc);
        }
        jd_utc
    }

    /// UT1 − UTC (s) at Julian Date `jd_utc`, interpolated between the
    /// IERS's days, and where it comes from. Outside the table, the
    /// nearest day's value, held.
    pub fn ut1_minus_utc(&self, jd_utc: f64) -> (f64, Ut1Source) {
        let tai = self.tai_minus_utc(jd_utc);
        let (Some(first), Some(last)) = (self.ut1.first(), self.ut1.last()) else {
            return (0.0, Ut1Source::Held);
        };
        if jd_utc <= first.0 || jd_utc >= last.0 {
            let held = if jd_utc <= first.0 { first } else { last };
            let on_the_day = jd_utc == held.0;
            let source = match (on_the_day, held.2) {
                (false, _) => Ut1Source::Held,
                (true, true) => Ut1Source::Predicted,
                (true, false) => Ut1Source::Measured,
            };
            return (held.1 + tai, source);
        }
        let k = self.ut1.partition_point(|row| row.0 <= jd_utc);
        let (a, b) = (self.ut1[k - 1], self.ut1[k]);
        let f = (jd_utc - a.0) / (b.0 - a.0);
        let source = if a.2 || b.2 {
            Ut1Source::Predicted
        } else {
            Ut1Source::Measured
        };
        (a.1 + f * (b.1 - a.1) + tai, source)
    }

    /// The first day the IERS's table of UT1 − UTC holds (Julian Date,
    /// UTC).
    pub fn ut1_known_from(&self) -> f64 {
        self.ut1.first().map_or(0.0, |row| row.0)
    }

    /// The last day the IERS's table of UT1 − UTC reaches (Julian Date,
    /// UTC), and the last it measured rather than predicted.
    pub fn ut1_known_until(&self) -> (f64, f64) {
        let last = self.ut1.last().map_or(0.0, |row| row.0);
        let measured = self
            .ut1
            .iter()
            .rev()
            .find(|row| !row.2)
            .map_or(0.0, |row| row.0);
        (last, measured)
    }

    /// ΔT = TT − UT1 (s) at Julian Date `jd_utc` (Kaplan 2005 eq. 2.9).
    pub fn delta_t(&self, jd_utc: f64) -> f64 {
        TT_MINUS_TAI + self.tai_minus_utc(jd_utc) - self.ut1_minus_utc(jd_utc).0
    }

    /// The Julian Date (UT1) at Julian Date `jd_tdb` (TDB).
    pub fn ut1(&self, jd_tdb: f64) -> f64 {
        let jd_utc = self.utc(jd_tdb);
        jd_utc + self.ut1_minus_utc(jd_utc).0 / DAY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scales() -> TimeScales {
        // A leap second at the end of 2016 (TAI − UTC 36 → 37 s), and UT1 −
        // UTC jumping by +1 s across it, as it does.
        let new_year = 2_457_754.5;
        TimeScales::new(
            vec![(2_457_204.5, 36.0), (new_year, 37.0)],
            &[
                (new_year - 2.0, -0.40, false),
                (new_year - 1.0, -0.41, false),
                (new_year, 0.58, false),
                (new_year + 1.0, 0.57, true),
            ],
        )
    }

    #[test]
    fn calendar_dates_to_julian_dates() {
        // J2000.0, and Meeus's example 7.a: 1957 Oct 4.81 → 2436116.31.
        assert_eq!(julian_date(2000, 1, 1, 43_200.0), J2000_JD);
        assert!((julian_date(1957, 10, 4, 0.81 * DAY) - 2_436_116.31).abs() < 1e-9);
        assert_eq!(julian_date(2025, 1, 1, 0.0), 2_460_676.5);
    }

    #[test]
    fn the_computers_clock_counts_from_1970() {
        assert_eq!(jd_from_unix(0.0), 2_440_587.5);
        // 2000-01-01 12:00 UTC.
        assert_eq!(jd_from_unix(946_728_000.0), J2000_JD);
    }

    #[test]
    fn earth_turns_from_its_angle_at_j2000() {
        // θ(J2000.0 UT1) = 0.7790572732640 turns (280.46°), and one UT1 day
        // later 1.00273781191135448 turns more.
        let theta = earth_rotation_angle(J2000_JD);
        assert!((theta / TAU - 0.779_057_273_264_0).abs() < 1e-12);
        let next = earth_rotation_angle(J2000_JD + 1.0);
        let turned = (next - theta).rem_euclid(TAU) / TAU;
        assert!(
            (turned - 0.002_737_811_911_354_48).abs() < 1e-11,
            "{turned}"
        );
    }

    #[test]
    fn tdb_runs_within_two_milliseconds_of_tt() {
        // The terms add to at most 1.731 ms (with T ≤ 1).
        let largest = (0..10_000)
            .map(|k| tdb_minus_tt(J2000_JD + 3.65 * f64::from(k)).abs())
            .fold(0.0, f64::max);
        assert!(largest < 1.731e-3 && largest > 1.6e-3, "{largest}");
    }

    #[test]
    fn utc_to_tdb_and_back() {
        let scales = scales();
        // Across the leap second: TT − UTC is 68.184 s, then 69.184 s. To
        // a Julian Date's resolution in double precision, 47 µs now.
        let resolution = 2_457_754.5 * f64::EPSILON * DAY;
        let before = 2_457_754.5 - 0.5;
        let after = 2_457_754.5 + 0.5;
        for (jd, leaps) in [(before, 36.0), (after, 37.0)] {
            let tdb = scales.tdb(jd);
            let tt_minus_utc = (tdb - jd) * DAY - tdb_minus_tt(jd);
            assert!(
                (tt_minus_utc - (32.184 + leaps)).abs() < resolution,
                "{tt_minus_utc}"
            );
            assert!(((scales.utc(tdb) - jd) * DAY).abs() < resolution);
        }
    }

    #[test]
    fn ut1_runs_on_smoothly_across_a_leap_second() {
        let scales = scales();
        let new_year = 2_457_754.5;
        // UT1 − TAI slides by 0.01 s a day, through the leap second too:
        // halfway through the last day of 2016, UT1 − UTC is −0.415 s, and
        // halfway through the first of 2017, +0.575 s.
        let (dut1, source) = scales.ut1_minus_utc(new_year - 0.5);
        assert!((dut1 + 0.415).abs() < 1e-12, "{dut1}");
        assert_eq!(source, Ut1Source::Measured);
        let (dut1, source) = scales.ut1_minus_utc(new_year + 0.5);
        assert!((dut1 - 0.575).abs() < 1e-12, "{dut1}");
        assert_eq!(source, Ut1Source::Predicted);
        // Past the table, held; ΔT = 32.184 + 37 − 0.57.
        let (dut1, source) = scales.ut1_minus_utc(new_year + 10.0);
        assert!((dut1 - 0.57).abs() < 1e-12 && source == Ut1Source::Held);
        assert!((scales.delta_t(new_year + 10.0) - 68.614).abs() < 1e-12);
        assert_eq!(scales.ut1_known_until(), (new_year + 1.0, new_year));
    }
}
