//! Step 8.5b's check of star colors: a blackbody's color worked out from
//! the CIE's colour-matching functions comes out as the CIE publishes it.
//!
//! The CIE's standard illuminant A, the light of a tungsten lamp, is
//! defined as a blackbody: Planck's law with c₂ = 1.435 × 10⁻² m K at
//! 2848 K (CIE 015:2018, §3.1; the second radiation constant as it stood
//! when the illuminant was defined, so 2856 K with today's). Its
//! chromaticity, with the 2° observer, is published as x = 0.44757,
//! y = 0.40745 (CIE 015:2018, Table 1).

use worldline_core::DVec3;
use worldline_core::blackbody::{blackbody, chromaticity, display, planck, tristimulus};
use worldline_data::{cie_1931, star_temperatures};

#[test]
fn illuminant_a_comes_out_as_the_cie_publishes_it() {
    // Summed over the whole table, 360 to 830 nm at 1 nm, as Worldline
    // sums. CIE 015 allows sums over 380 to 780 nm, and at 5 nm steps,
    // too, and the published values come from one of those: they differ
    // from each other in the sixth place. The limit is half the published
    // values' last place, 5 × 10⁻⁶, plus how far the allowed sums spread
    // from Worldline's, worked out here.
    let table = cie_1931();
    let a = |rows: &[[f64; 4]]| {
        chromaticity(tristimulus(rows, |lambda| planck(lambda, 2848.0, 1.435e-2)))
    };
    let (x, y) = a(&table);
    let within = |lo: f64, hi: f64, step: f64| -> Vec<[f64; 4]> {
        table
            .iter()
            .copied()
            .filter(|r| r[0] >= lo && r[0] <= hi && (r[0] - lo) % step == 0.0)
            .collect()
    };
    let others = [
        a(&within(380.0, 780.0, 1.0)),
        a(&within(380.0, 780.0, 5.0)),
        a(&within(360.0, 830.0, 5.0)),
    ];
    let spread = |k: usize| {
        others
            .iter()
            .map(|o| {
                if k == 0 {
                    (o.0 - x).abs()
                } else {
                    (o.1 - y).abs()
                }
            })
            .fold(0.0f64, f64::max)
    };
    let (limit_x, limit_y) = (5e-6 + spread(0), 5e-6 + spread(1));
    println!(
        "illuminant A: x = {x:.6}, y = {y:.6} (published 0.44757, 0.40745); the allowed sums \
         spread {:.1e} and {:.1e}",
        spread(0),
        spread(1)
    );
    worldline_report::margins(
        "star-colors",
        "A blackbody's color from the CIE's colour-matching functions: illuminant A",
    )
    .check(
        "x, against CIE 015:2018's 0.44757",
        x - 0.44757,
        limit_x,
        format!(
            "{x:.6} (limit {limit_x:.1e}: the published rounding, and how far the sums CIE 015 \
             allows spread)"
        ),
    )
    .check(
        "y, against CIE 015:2018's 0.40745",
        y - 0.40745,
        limit_y,
        format!("{y:.6} (limit {limit_y:.1e})"),
    )
    .save(worldline_report::here!());
    assert!((x - 0.44757).abs() < limit_x);
    assert!((y - 0.40745).abs() < limit_y);
}

#[test]
fn hotter_stars_are_bluer() {
    // Along the blackbody colors, x falls as the temperature rises, and
    // the drawn color's blue grows against its red. Printed for each star
    // with a temperature in the data.
    let table = cie_1931();
    let mut stars = star_temperatures();
    stars.sort_by(|a, b| a.kelvin.total_cmp(&b.kelvin));
    let mut last: Option<(f64, f64)> = None;
    for star in &stars {
        let xyz = blackbody(&table, star.kelvin);
        let (x, _) = chromaticity(xyz);
        let rgb = display(xyz);
        println!(
            "{}: {} K, x = {x:.4}, drawn as #{:02x}{:02x}{:02x}",
            star.name,
            star.kelvin,
            (rgb[0] * 255.0).round() as u8,
            (rgb[1] * 255.0).round() as u8,
            (rgb[2] * 255.0).round() as u8
        );
        if let Some((last_x, last_ratio)) = last {
            assert!(x < last_x, "{}", star.name);
            assert!(rgb[2] / rgb[0] > last_ratio, "{}", star.name);
        }
        last = Some((x, rgb[2] / rgb[0]));
    }
    // Far hotter than any star, toward the limit all blackbodies share.
    let (x, y) = chromaticity(blackbody(&table, 1e9));
    println!("1e9 K: x = {x:.4}, y = {y:.4}");
    assert!(x < 0.25 && y < 0.25);
    assert!(display(DVec3::ZERO) == [0.0; 3]);
}
