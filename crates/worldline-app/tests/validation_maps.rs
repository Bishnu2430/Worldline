//! Validation: the spacecraft maps sit where the IAU puts things.
//!
//! - Landmarks from the IAU Gazetteer of Planetary Nomenclature land on the
//!   right terrain: bright ones brighter than the map's average and than
//!   the same spot on a map turned halfway round (the mistake a map's
//!   center longitude invites), dark ones darker.
//! - Iapetus's leading side, the one facing its direction of motion in
//!   the simulation, is its dark one (Cassini Regio).
//! - Pluto and Charon turn the faces the maps call longitude 0° to each
//!   other, so Sputnik Planitia (Pluto's "heart") faces away from Charon.
//!
//! The maps use the renderer's layout: equirectangular, 2048 x 1024,
//! longitude 0° in the middle, east to the right, north up.
//!
//! `cargo test --release -p worldline-app --test validation_maps -- --nocapture`

use std::f64::consts::{FRAC_PI_2, PI};

use worldline_core::DVec3;
use worldline_data::{MoonSystemData, moon_systems, rotation_model, solar_system};

const WIDTH: usize = 2048;
const HEIGHT: usize = 1024;

/// One pixel of the maps, in rad.
const PIXEL: f64 = 2.0 * PI / WIDTH as f64;

/// A map's brightness, one value (0–255) per pixel.
struct Map(Vec<u8>);

fn map(file: &str) -> Map {
    let path = format!("{}/assets/textures/{file}", env!("CARGO_MANIFEST_DIR"));
    let image = image::open(&path).expect("bundled map").to_luma8();
    assert_eq!((image.width(), image.height()), (2048, 1024), "{file}");
    Map(image.into_raw())
}

/// Unit vector at latitude `lat` and east longitude `lon`, in rad.
fn unit(lat: f64, lon: f64) -> DVec3 {
    DVec3::new(lat.cos() * lon.cos(), lat.cos() * lon.sin(), lat.sin())
}

/// Latitude and east longitude of a direction, in rad.
fn lat_lon(v: DVec3) -> (f64, f64) {
    let v = v.normalize();
    (v.z.asin(), v.y.atan2(v.x))
}

impl Map {
    /// Mean brightness over the cap of angular radius `radius` around
    /// (`lat`, `lon`), each pixel counted by its area; all angles in rad.
    /// With `photographed_only`, black (unphotographed) pixels are left
    /// out.
    fn cap_mean(&self, lat: f64, lon: f64, radius: f64, photographed_only: bool) -> f64 {
        let center = unit(lat, lon);
        let (mut sum, mut weight) = (0.0, 0.0);
        for j in 0..HEIGHT {
            let row_lat = FRAC_PI_2 - (j as f64 + 0.5) * PI / HEIGHT as f64;
            if (row_lat - lat).abs() > radius {
                continue;
            }
            for i in 0..WIDTH {
                let value = self.0[j * WIDTH + i];
                if photographed_only && value == 0 {
                    continue;
                }
                let col_lon = -PI + (i as f64 + 0.5) * PIXEL;
                if unit(row_lat, col_lon).dot(center) >= radius.cos() {
                    let area = row_lat.cos();
                    sum += area * f64::from(value);
                    weight += area;
                }
            }
        }
        sum / weight
    }

    /// Mean brightness over everything photographed.
    fn photographed_mean(&self) -> f64 {
        self.cap_mean(0.0, 0.0, PI, true)
    }
}

/// A body's moon system from the snapshot, and one of its bodies' state
/// relative to the planet's center, in the simulation's frame.
fn relative_state(host: &str, name: &str) -> (DVec3, DVec3) {
    let systems: Vec<MoonSystemData> = moon_systems();
    let system = systems
        .iter()
        .find(|s| s.host == host)
        .expect("moon system");
    let planet = &system.bodies[0];
    let body = system.bodies.iter().find(|b| b.name == name).expect("moon");
    (
        body.position - planet.position,
        body.velocity - planet.velocity,
    )
}

/// `direction` (in the simulation's frame) in `body`'s IAU body-fixed
/// frame at the snapshot: latitude and east longitude, in rad.
fn body_fixed(body: &str, direction: DVec3) -> (f64, f64) {
    let frame = rotation_model(body)
        .expect("IAU rotation model")
        .orientation(solar_system().epoch_jd_tdb)
        .body_to_ecliptic;
    lat_lon(frame.transpose() * direction)
}

#[test]
fn landmarks_land_on_the_right_terrain() {
    // (body, map, landmark, center latitude and east longitude in degrees
    // and diameter in km from the IAU Gazetteer (planetocentric), the radius
    // of the map's sphere in km from its label, bright or dark).
    let landmarks = [
        (
            "Pluto",
            "2k_pluto.png",
            "Sputnik Planitia",
            19.51,
            178.69,
            1492.0,
            1188.3,
            true,
        ),
        (
            "Ceres",
            "2k_ceres.png",
            "Cerealia Facula",
            19.7,
            239.6,
            14.0,
            470.0,
            true,
        ),
        (
            "Io",
            "2k_io.png",
            "Loki Patera",
            13.0083,
            51.2136,
            226.57471,
            1821.46,
            false,
        ),
        (
            "Titan",
            "2k_titan.png",
            "Xanadu",
            -15.0,
            260.0,
            3400.0,
            2575.0,
            true,
        ),
        (
            "Iapetus",
            "2k_iapetus.png",
            "Roncevaux Terra",
            37.0,
            120.5,
            1284.0,
            736.0,
            true,
        ),
    ];
    for (body, file, landmark, lat, lon, diameter, radius, bright) in landmarks {
        let map = map(file);
        let (lat, lon) = (f64::to_radians(lat), f64::to_radians(lon));
        // A cap half the landmark's radius, well inside it.
        let cap = diameter / radius / 4.0;
        let here = map.cap_mean(lat, lon, cap, false);
        let turned = map.cap_mean(lat, lon + PI, cap, false);
        let average = map.photographed_mean();
        println!(
            "{body}, {landmark} ({}): {here:.1} on the map, {turned:.1} if the map were turned halfway round, map average {average:.1}",
            if bright { "bright" } else { "dark" }
        );
        if bright {
            assert!(here > average && here > turned, "{body}: {landmark}");
        } else {
            assert!(here < average && here < turned, "{body}: {landmark}");
        }
    }
}

#[test]
fn iapetus_leads_with_its_dark_side() {
    // Iapetus turns once per orbit, so one side always leads, and that side
    // is the dark one (Cassini Regio): the trailing side is about 10 times
    // brighter (Spencer & Denk 2010). The mosaic evens brightness out to
    // show terrain, so on the map the difference is smaller, but it must
    // point the same way.
    let (_, velocity) = relative_state("Saturn", "Iapetus");
    let (lat, lon) = body_fixed("Iapetus", velocity);
    let map = map("2k_iapetus.png");
    let leading = map.cap_mean(lat, lon, FRAC_PI_2, true);
    let trailing = map.cap_mean(-lat, lon + PI, FRAC_PI_2, true);
    println!(
        "Iapetus heads toward {:.1}° N, {:.1}° W (the IAU's leading side is centered on 90° W); \
         leading half {leading:.1}, trailing half {trailing:.1}: {:.2} times as bright",
        lat.to_degrees(),
        (-lon.to_degrees()).rem_euclid(360.0),
        leading / trailing
    );
    assert!(leading < trailing);
}

#[test]
fn pluto_and_charon_face_each_other_at_longitude_zero() {
    // Both are tidally locked, and the IAU defines each one's longitude 0°
    // as the point facing the other; New Horizons' maps are made that way.
    // The maps land in the right place to within one of their pixels if the
    // rotation models put the facing points within a pixel of 0°. (NAIF's
    // pck00011 values miss by 1.5°; this test caught that, and Worldline
    // uses the New Horizons team's corrected ones.)
    let (charon, _) = relative_state("Pluto", "Charon");
    for (body, direction) in [("Pluto", charon), ("Charon", -charon)] {
        let (lat, lon) = body_fixed(body, direction);
        let off = unit(lat, lon).angle_between(DVec3::X);
        println!(
            "{body} faces {} at {:.3}° N, {:.3}° E: {:.3}° from longitude 0° (a map pixel is {:.3}°)",
            if body == "Pluto" { "Charon" } else { "Pluto" },
            lat.to_degrees(),
            lon.to_degrees(),
            off.to_degrees(),
            PIXEL.to_degrees()
        );
        assert!(off < PIXEL, "{body}");
    }
    // The point facing directly away from Charon lies in Sputnik Planitia,
    // inside the extent the IAU Gazetteer gives it (22.18° S to 49.61° N,
    // 153.08° to 197.66° E): near the tidal axis, where a heavy basin
    // settles (Nimmo et al. 2016; Keane et al. 2016).
    let (lat, lon) = body_fixed("Pluto", -charon);
    let (lat, lon) = (lat.to_degrees(), lon.to_degrees().rem_euclid(360.0));
    let center = unit(19.51f64.to_radians(), 178.69f64.to_radians());
    println!(
        "Pluto faces away from Charon at {lat:.2}° N, {lon:.2}° E, {:.1}° from Sputnik Planitia's center",
        unit(lat.to_radians(), lon.to_radians())
            .angle_between(center)
            .to_degrees()
    );
    assert!((-22.1813..=49.6135).contains(&lat) && (153.0848..=197.6598).contains(&lon));
}
