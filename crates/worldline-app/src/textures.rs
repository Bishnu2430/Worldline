//! Surface maps for the bodies, built into the program.
//!
//! 2K equirectangular maps from Solar System Scope (CC BY 4.0, based on
//! NASA data); see CREDITS.md. `tools/fetch_textures.py` downloads them.
//! Pluto has no global map in the set, so it is drawn in a flat color.

/// A body's maps, as encoded image files.
pub struct SurfaceMaps {
    /// The sunlit surface.
    pub surface: &'static [u8],
    /// What glows on the night side (Earth's city lights), if anything.
    pub night: Option<&'static [u8]>,
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
        _ => return None,
    };
    let night = (name == "Earth").then(|| map!("2k_earth_nightmap.jpg"));
    Some(SurfaceMaps { surface, night })
}

/// Decodes a JPEG into width, height and sRGB RGBA8 pixels.
pub fn decode(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)
        .expect("bundled texture is a valid JPEG")
        .to_rgba8();
    (image.width(), image.height(), image.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_map_decodes_at_2k() {
        let names = [
            "Sun", "Mercury", "Venus", "Earth", "Moon", "Mars", "Jupiter", "Saturn", "Uranus",
            "Neptune",
        ];
        for name in names {
            let maps = surface_maps(name).unwrap();
            for bytes in [Some(maps.surface), maps.night].into_iter().flatten() {
                let (width, height, rgba) = decode(bytes);
                assert_eq!((width, height), (2048, 1024), "{name}");
                assert_eq!(rgba.len(), 2048 * 1024 * 4);
            }
        }
        assert!(surface_maps("Pluto").is_none());
    }
}
