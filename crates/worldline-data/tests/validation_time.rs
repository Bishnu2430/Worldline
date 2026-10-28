//! Validation: the computer's clock turned into the simulation's time, and
//! Earth turning as it really does (step 5.1).
//!
//! - TT − UTC is 69.184 s since 2017 (IERS Bulletin C 72: UTC − TAI =
//!   −37 s; TT = TAI + 32.184 s).
//! - Where the Sun is overhead, from Earth's orientation by the IERS's
//!   UT1 and the simulation's geometry, against NASA JPL Horizons, on
//!   2025-01-01 and 2026-10-28.
//!
//! `cargo test -p worldline-data --test validation_time -- --nocapture`

use worldline_core::DVec3;
use worldline_core::constants::DAY;
use worldline_core::rotation::earth_prime_meridian;
use worldline_core::timescales::tdb_minus_tt;
use worldline_data::{
    Snapshot, planets_2026_10_28, rotation_model, solar_system, sun_overhead, time_scales,
};

#[test]
fn tt_runs_69_184_seconds_ahead_of_utc() {
    // Every day since 2017-01-01 0h UTC, to a Julian Date's resolution in
    // double precision (47 µs).
    let scales = time_scales();
    for jd_utc in [2_457_754.5, 2_460_676.5, 2_461_341.5] {
        let tdb = scales.tdb(jd_utc);
        let tt_minus_utc = (tdb - jd_utc) * DAY - tdb_minus_tt(jd_utc);
        println!("JD {jd_utc} UTC: TT − UTC = {tt_minus_utc:.6} s");
        assert!((tt_minus_utc - 69.184).abs() < 1e-4);
    }
    // And UT1: 2026-10-28, ΔT = 32.184 + 37 − (−0.0517246), predicted.
    let delta_t = scales.delta_t(2_461_341.5);
    println!("ΔT on 2026-10-28: {delta_t:.7} s");
    assert!((delta_t - 69.235_724_6).abs() < 1e-9);
}

/// Where the Sun is overhead (longitude east, latitude; degrees) at
/// Julian Date `jd_tdb`, given where the Sun and Earth are in `snapshot`,
/// with Earth turned by `meridian` (None: the IAU model's own).
fn overhead(snapshot: &Snapshot, jd_tdb: f64, meridian: Option<f64>) -> (f64, f64) {
    let (earth, sun) = (
        snapshot.body("Earth").unwrap(),
        snapshot.body("Sun").unwrap(),
    );
    let toward_sun = (sun.position - earth.position).normalize();
    let model = rotation_model("Earth").unwrap();
    let orientation = match meridian {
        Some(w) => model.orientation_with_meridian(jd_tdb, w),
        None => model.orientation(jd_tdb),
    };
    let local: DVec3 = orientation.body_to_ecliptic.transpose() * toward_sun;
    (
        local.y.atan2(local.x).to_degrees(),
        local.z.asin().to_degrees(),
    )
}

#[test]
fn the_sun_is_overhead_where_jpl_puts_it() {
    // Horizons: the Sun overhead at longitude α − GAST, latitude δ, from its
    // apparent right ascension and declination and Greenwich apparent
    // sidereal time. Within what the model leaves out: aberration (20.5″)
    // and nutation (in right ascension at most 15.8″, in declination 9.2″)
    // shift where the Sun appears; the origin of the Earth Rotation Angle
    // lies off the ICRF's 0h by under an arcsecond: 0.013° in all, so
    // within 0.015°.
    let scales = time_scales();
    let model = rotation_model("Earth").unwrap();
    for (jd_tt, ra, dec, gast) in sun_overhead() {
        let snapshot = if jd_tt < 2_461_000.0 {
            solar_system()
        } else {
            planets_2026_10_28()
        };
        assert_eq!(snapshot.epoch_jd_tdb, jd_tt);
        let jd_ut1 = scales.ut1(jd_tt);
        let meridian = earth_prime_meridian(jd_ut1, model.pole(jd_tt).0);
        let (longitude, latitude) = overhead(&snapshot, jd_tt, Some(meridian));
        let (iau, _) = overhead(&snapshot, jd_tt, None);
        let expected = (ra - 15.0 * gast + 180.0).rem_euclid(360.0) - 180.0;
        let off = (longitude - expected + 180.0).rem_euclid(360.0) - 180.0;
        let iau_off = (iau - expected + 180.0).rem_euclid(360.0) - 180.0;
        println!(
            "JD {jd_tt} TT: Sun overhead at {latitude:.4}°, {longitude:.4}° (JPL: {dec:.4}°, {expected:.4}°): {:.4}° and {off:.4}° off; the IAU model in TDB alone {iau_off:.3}° off",
            latitude - dec
        );
        assert!(off.abs() < 0.015 && (latitude - dec).abs() < 0.015);
    }
}
