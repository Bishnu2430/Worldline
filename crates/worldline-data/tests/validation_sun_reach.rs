//! Validation: sunlight, light travel time, the solar wind's spiral and the
//! heliosphere's boundaries, against measurements.

use std::collections::BTreeMap;

use worldline_core::constants::{AU, C, DAY, GM_SUN, SOLAR_RADIUS};
use worldline_core::gravity::EinsteinInfeldHoffmann;
use worldline_core::integrator::{Ias15, advance};
use worldline_core::sunlight::{irradiance, light_time, shapiro_delay};
use worldline_core::{DVec3, System};
use worldline_data::{
    Boundary, heliosphere, parker_spiral, solar_system, solar_wind_2025, voyager_crossings,
};

/// Earth's distance from the Sun (m) each day of 2025, simulated with
/// relativistic gravity from JPL's 1 January state.
fn earth_distances() -> Vec<f64> {
    let mut system: System = solar_system().system();
    let index = |s: &System, name: &str| s.bodies.iter().position(|b| b.name == name).unwrap();
    let (sun, earth) = (index(&system, "Sun"), index(&system, "Earth"));
    let mut integrator = Ias15::new();
    (0..365)
        .map(|_| {
            let r = (system.bodies[earth].position - system.bodies[sun].position).length();
            advance(&mut system, &EinsteinInfeldHoffmann, &mut integrator, DAY);
            r
        })
        .collect()
}

#[test]
fn sunlight_at_earth_is_1361_watts_per_square_meter() {
    // NASA's SORCE satellite measured the total solar irradiance at 1 AU
    // as 1360.8 ± 0.5 W/m² at the 2008 solar minimum (Kopp & Lean 2011,
    // Geophys. Res. Lett. 38, L01706). The IAU's luminosity must give that
    // within the measurement's uncertainty.
    let at_1au = irradiance(AU);
    println!("at 1 AU: {at_1au:.2} W/m² (SORCE measured 1360.8 ± 0.5)");
    assert!((at_1au - 1360.8).abs() <= 0.5);
    // Over the year, Earth's distance changes by 3.3%, so its sunlight by
    // 6.9%. Averaged over the simulated year, it must round to the
    // roadmap's 1361.
    let distances = earth_distances();
    let light: Vec<f64> = distances.iter().map(|&r| irradiance(r)).collect();
    let mean = light.iter().sum::<f64>() / light.len() as f64;
    let (brightest, faintest) = (
        light
            .iter()
            .copied()
            .enumerate()
            .fold((0, 0.0), |m, x| if x.1 > m.1 { x } else { m }),
        light
            .iter()
            .copied()
            .enumerate()
            .fold((0, f64::MAX), |m, x| if x.1 < m.1 { x } else { m }),
    );
    println!(
        "2025: mean {mean:.1} W/m²; most {:.1} on day {} (perihelion), least {:.1} on day {} (aphelion)",
        brightest.1,
        brightest.0 + 1,
        faintest.1,
        faintest.0 + 1
    );
    assert!((mean - 1361.0).abs() < 0.5);
}

#[test]
fn light_takes_8_3_minutes_to_reach_earth() {
    // The IAU's light time for 1 AU, τ_A = 499.004 783 836 s, follows
    // from the AU and c, both exact.
    let tau = AU / C;
    println!("1 AU / c = {tau:.9} s");
    assert!((tau - 499.004_783_836).abs() < 1e-9);
    // Averaged over the simulated year, from the Sun's center: the
    // roadmap's 8.3 minutes, to its rounding.
    let distances = earth_distances();
    let mean = distances.iter().map(|r| r / C).sum::<f64>() / distances.len() as f64;
    println!("2025 mean: {mean:.2} s = {:.3} min", mean / 60.0);
    assert!((mean / 60.0 - 8.3).abs() < 0.05);
    // Light actually leaves the Sun's surface, and the Sun's gravity
    // delays it on the way out (the Shapiro delay).
    let surface = DVec3::X * SOLAR_RADIUS;
    let earth = DVec3::X * distances[0];
    println!(
        "on 1 January, from the surface: {:.3} s, of which gravity's delay is {:.1} µs",
        light_time(GM_SUN, surface, earth),
        shapiro_delay(GM_SUN, surface, earth) * 1e6
    );
}

/// The axis (rad, in (−90°, 90°]) of a set of directions given as angles,
/// where an angle and its opposite count the same: half the direction of
/// the mean of the doubled angles.
fn axial_mean(angles: &[f64]) -> f64 {
    let (s, c) = angles.iter().fold((0.0, 0.0), |(s, c), a| {
        (s + (2.0 * a).sin(), c + (2.0 * a).cos())
    });
    s.atan2(c) / 2.0
}

#[test]
fn the_parker_spiral_matches_the_magnetic_field_measured_at_earth() {
    // In GSE coordinates (x toward the Sun, y opposite to Earth's
    // motion), Parker's field points outward and back along the spiral,
    // (−cos ψ, sin ψ), or the reverse, depending on its polarity: so the
    // measured spiral angle is atan2(By, −Bx), with opposite directions
    // counted as one. Each hour, Parker predicts tan ψ = Ω r / v from that
    // hour's measured speed (r = 1 AU; Earth's ±1.7% swing averages out).
    //
    // Consecutive hours aren't independent (the field's structure lasts
    // many hours), so the comparison is made day by day: each day's mean
    // measured axis minus its mean predicted axis. The standard: the
    // year's average difference is within 3 standard errors of zero.
    let wind = parker_spiral();
    let mut days: BTreeMap<String, (Vec<f64>, Vec<f64>)> = BTreeMap::new();
    for hour in solar_wind_2025() {
        let (Some(b), Some(v)) = (hour.field, hour.speed) else {
            continue;
        };
        let day = days.entry(hour.hour[..10].to_string()).or_default();
        day.0.push(b.y.atan2(-b.x));
        day.1.push((wind.rotation_rate * AU / v).atan());
    }
    let differences: Vec<f64> = days
        .values()
        .map(|(measured, predicted)| {
            let d = axial_mean(measured) - axial_mean(predicted);
            // Wrap to (−90°, 90°].
            (d + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::PI)
                - std::f64::consts::FRAC_PI_2
        })
        .collect();
    let n = differences.len() as f64;
    let mean = differences.iter().sum::<f64>() / n;
    let spread = (differences.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
    let standard_error = spread / n.sqrt();
    let all: Vec<f64> = days.values().flat_map(|d| d.0.clone()).collect();
    let predicted: Vec<f64> = days.values().flat_map(|d| d.1.clone()).collect();
    println!(
        "{} days; measured axis {:.1}°, Parker {:.1}°; daily difference {:+.1}° ± {:.1}° (standard error)",
        days.len(),
        axial_mean(&all).to_degrees(),
        axial_mean(&predicted).to_degrees(),
        mean.to_degrees(),
        standard_error.to_degrees()
    );
    println!(
        "model: {:.0} km/s, {:.1} protons/cm³, Sun turns in {:.2} days: spiral crosses 1 AU at {:.1}°",
        wind.speed / 1e3,
        wind.density_at_1au / 1e6,
        std::f64::consts::TAU / wind.rotation_rate / DAY,
        wind.angle(AU, 1.0).to_degrees()
    );
    assert!(mean.abs() < 3.0 * standard_error);
}

#[test]
fn voyager_positions_match_the_published_crossings() {
    // JPL's trajectory puts each Voyager, at noon on the published day,
    // within half a tenth of an AU of the published distance: the
    // precision three of the four are given to.
    for c in voyager_crossings() {
        let r = c.position.length();
        println!(
            "{} {:?} {}: Horizons {:.3} AU, published {} AU ({})",
            c.spacecraft,
            c.boundary,
            c.date,
            r / AU,
            c.published_distance / AU,
            c.source
        );
        assert!((r - c.published_distance).abs() < 0.05 * AU);
    }
    // The drawn boundaries: how far each crossing lies from the fitted
    // shape (the shape is axisymmetric; the real heliosphere isn't).
    let shell = heliosphere();
    for c in voyager_crossings() {
        let nose = match c.boundary {
            Boundary::TerminationShock => shell.termination_shock,
            Boundary::Heliopause => shell.heliopause,
        };
        let drawn = nose * shell.shape(c.position);
        println!(
            "{} {:?}: {:.1}° from the nose; shape {:.1} AU, measured {:.1} AU ({:+.1}%)",
            c.spacecraft,
            c.boundary,
            c.position.normalize().dot(shell.nose).acos().to_degrees(),
            drawn / AU,
            c.position.length() / AU,
            (c.position.length() / drawn - 1.0) * 100.0
        );
    }
    println!(
        "nose distances: termination shock {:.1} AU, heliopause {:.1} AU",
        shell.termination_shock / AU,
        shell.heliopause / AU
    );
}
