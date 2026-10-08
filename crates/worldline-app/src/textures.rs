//! Surface maps for the bodies, built into the program.
//!
//! The Sun's and planets' are 2K equirectangular maps from Solar System
//! Scope (CC BY 4.0, based on NASA data); `tools/fetch_textures.py`
//! downloads them. The major moons', Pluto's, Charon's and Ceres's are
//! spacecraft mosaics from the USGS Astrogeology Science Center and NASA's
//! Planetary Data System (public domain), shrunk to 2K and placed in each
//! body's IAU coordinates by `tools/fetch_spacecraft_maps.py`. See
//! CREDITS.md.

/// A body's maps, as encoded image files.
pub struct SurfaceMaps {
    /// The sunlit surface.
    pub surface: &'static [u8],
    /// What glows on the night side (Earth's city lights), if anything.
    pub night: Option<&'static [u8]>,
    /// Clouds, white on black, if the body has a cloud map.
    pub clouds: Option<&'static [u8]>,
}

macro_rules! map {
    ($file:literal) => {
        include_bytes!(concat!("../assets/textures/", $file)).as_slice()
    };
}

/// The maps for a body in the solar-system snapshot, if there are any.
pub fn surface_maps(name: &str) -> Option<SurfaceMaps> {
    let surface = match name {
        "Sun" => map!("2k_sun.jpg"),
        "Mercury" => map!("2k_mercury.jpg"),
        // Venus's surface is hidden under clouds; this is what you'd see.
        "Venus" => map!("2k_venus_atmosphere.jpg"),
        "Earth" => map!("2k_earth_daymap.jpg"),
        "Moon" => map!("2k_moon.jpg"),
        "Mars" => map!("2k_mars.jpg"),
        "Jupiter" => map!("2k_jupiter.jpg"),
        "Saturn" => map!("2k_saturn.jpg"),
        "Uranus" => map!("2k_uranus.jpg"),
        "Neptune" => map!("2k_neptune.jpg"),
        "Io" => map!("2k_io.png"),
        "Europa" => map!("2k_europa.png"),
        "Ganymede" => map!("2k_ganymede.png"),
        "Callisto" => map!("2k_callisto.png"),
        "Titan" => map!("2k_titan.png"),
        "Enceladus" => map!("2k_enceladus.png"),
        "Tethys" => map!("2k_tethys.png"),
        "Dione" => map!("2k_dione.png"),
        "Rhea" => map!("2k_rhea.png"),
        "Iapetus" => map!("2k_iapetus.png"),
        "Triton" => map!("2k_triton.png"),
        "Pluto" => map!("2k_pluto.png"),
        "Charon" => map!("2k_charon.png"),
        "Ceres" => map!("2k_ceres.png"),
        _ => return None,
    };
    let night = (name == "Earth").then(|| map!("2k_earth_nightmap.jpg"));
    let clouds = (name == "Earth").then(|| map!("2k_earth_clouds.jpg"));
    Some(SurfaceMaps {
        surface,
        night,
        clouds,
    })
}

/// What a body's spacecraft map shows, as the inspector says it, if the
/// body has one.
pub fn spacecraft_map(name: &str) -> Option<&'static str> {
    Some(match name {
        "Io" | "Europa" | "Ganymede" | "Callisto" => {
            "Surface map: Galileo and Voyager images (USGS), in black and white"
        }
        "Titan" => {
            "Surface map: Cassini's near-infrared view through the haze (USGS); to the eye, Titan is a featureless orange haze"
        }
        "Enceladus" | "Tethys" => "Surface map: Cassini images (USGS), in black and white",
        "Dione" | "Rhea" => {
            "Surface map: Cassini images, gaps filled from Voyager (USGS), in black and white"
        }
        "Iapetus" => {
            "Surface map: Cassini images, gaps filled from Voyager (USGS), in black and white. The mosaic evens out brightness: the real trailing side is about 10 times brighter than the leading one (Spencer & Denk 2010)"
        }
        "Triton" => {
            "Surface map: Voyager 2 (USGS), its orange, violet and ultraviolet images shown as red, green and blue; the north, in darkness during the 1989 flyby, is black"
        }
        "Pluto" | "Charon" => {
            "Surface map: New Horizons (USGS), in black and white; the south, in darkness during the 2015 flyby, is black"
        }
        "Ceres" => "Surface map: Dawn images (DLR, NASA PDS), in black and white",
        _ => return None,
    })
}

/// Decodes a JPEG or PNG into width, height and sRGB RGBA8 pixels.
pub fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let image = image::load_from_memory(bytes)
        .expect("bundled texture is a valid image")
        .to_rgba8();
    (image.width(), image.height(), image.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_map_decodes_at_2k() {
        let planets = [
            "Sun", "Mercury", "Venus", "Earth", "Moon", "Mars", "Jupiter", "Saturn", "Uranus",
            "Neptune",
        ];
        let spacecraft = [
            "Io",
            "Europa",
            "Ganymede",
            "Callisto",
            "Titan",
            "Enceladus",
            "Tethys",
            "Dione",
            "Rhea",
            "Iapetus",
            "Triton",
            "Pluto",
            "Charon",
            "Ceres",
        ];
        for name in planets.into_iter().chain(spacecraft) {
            let maps = surface_maps(name).unwrap();
            for bytes in [Some(maps.surface), maps.night, maps.clouds]
                .into_iter()
                .flatten()
            {
                let (width, height, rgba) = decode(bytes);
                assert_eq!((width, height), (2048, 1024), "{name}");
                assert_eq!(rgba.len(), 2048 * 1024 * 4);
            }
        }
        // Every spacecraft map is described in the inspector.
        for name in spacecraft {
            assert!(spacecraft_map(name).is_some(), "{name}");
        }
        // No published global map of Eris exists.
        assert!(surface_maps("Eris").is_none());
    }
}
