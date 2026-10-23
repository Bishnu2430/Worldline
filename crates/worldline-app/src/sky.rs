//! The sky behind everything: the Milky Way as ESA's Gaia mission mapped it,
//! and the stars of the Yale Bright Star Catalogue placed exactly, drawn
//! into one equirectangular map in galactic coordinates.
//!
//! Gaia's map: "The colour of the sky from Gaia's Early Data Release 3",
//! ESA/Gaia/DPAC, CC BY-SA 3.0 IGO, acknowledgement A. Moitinho
//! (`tools/fetch_sky.py` downloads it). Shown enlarged, with the bright
//! stars drawn in. How bright each star is drawn, and its color, are a
//! visual approximation: see `docs/physics/lensing.md`.

use worldline_core::constants::OBLIQUITY_J2000;
use worldline_core::{DMat3, DVec3};

/// The sky map's size: twice Gaia's 2,000 × 1,000, so the stars are sharp.
pub const WIDTH: u32 = 4096;
pub const HEIGHT: u32 = 2048;

/// Turns the simulation's directions (ecliptic and equinox of J2000) into
/// galactic ones: to the equator, then by the Hipparcos rotation.
pub fn to_galactic() -> DMat3 {
    worldline_data::equatorial_to_galactic() * DMat3::from_rotation_x(OBLIQUITY_J2000)
}

/// Where a galactic direction falls on the map, in texels from its top
/// left: longitude 0 in the middle, increasing to the left, as sky maps
/// (and Gaia's) are drawn.
pub fn texel(galactic: DVec3) -> (f64, f64) {
    let l = galactic.y.atan2(galactic.x);
    let b = galactic.z.clamp(-1.0, 1.0).asin();
    let u = (0.5 - l / std::f64::consts::TAU).rem_euclid(1.0);
    let v = 0.5 - b / std::f64::consts::PI;
    (u * f64::from(WIDTH), v * f64::from(HEIGHT))
}

/// sRGB-encoded byte to linear light.
fn linear(byte: u8) -> f64 {
    let c = f64::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to an sRGB-encoded byte.
fn encoded(c: f64) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round() as u8
}

/// A blackbody's light at the red, green and blue of a display (610, 550
/// and 465 nm), each relative to a 6,504 K blackbody's, so that one is
/// white: Planck's law, λ⁻⁵/(e^(hc/λkT) − 1) up to a constant.
pub fn blackbody(temperature: f64) -> [f64; 3] {
    let planck = |nm: f64, t: f64| {
        let lambda = nm * 1e-9;
        1.0 / (lambda.powi(5) * (0.014_387_77 / (lambda * t)).exp_m1())
    };
    [610.0, 550.0, 465.0].map(|nm| planck(nm, temperature) / planck(nm, 6504.0))
}

/// A star's color from its B − V: its temperature by Ballesteros's (2012)
/// formula (*EPL* 97, 34008), as a [`blackbody`], brightest channel 1.
fn star_color(b_v: Option<f64>) -> [f64; 3] {
    let Some(bv) = b_v else {
        return [1.0; 3];
    };
    let temperature = 4600.0 * (1.0 / (0.92 * bv + 1.7) + 1.0 / (0.92 * bv + 0.62));
    let rgb = blackbody(temperature);
    let top = rgb.iter().copied().fold(0.0, f64::max);
    rgb.map(|c| c / top)
}

/// The sky map: sRGB RGBA, row by row from the top.
pub fn map() -> (u32, u32, Vec<u8>) {
    let (w, h, gaia) =
        crate::textures::decode(include_bytes!("../assets/textures/gaia_edr3_sky_2k.png"));
    // Enlarged bilinearly.
    let mut rgba = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
    for y in 0..HEIGHT {
        let sy = ((f64::from(y) + 0.5) * f64::from(h) / f64::from(HEIGHT) - 0.5)
            .clamp(0.0, f64::from(h - 1));
        let (y0, fy) = (sy.floor() as u32, sy.fract());
        let y1 = (y0 + 1).min(h - 1);
        for x in 0..WIDTH {
            let sx = (f64::from(x) + 0.5) * f64::from(w) / f64::from(WIDTH) - 0.5;
            let sx = sx.rem_euclid(f64::from(w));
            let (x0, fx) = (sx.floor() as u32 % w, sx.fract());
            let x1 = (x0 + 1) % w;
            let at = |px: u32, py: u32, c: usize| f64::from(gaia[((py * w + px) * 4) as usize + c]);
            let out = ((y * WIDTH + x) * 4) as usize;
            for c in 0..3 {
                let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
                let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
                rgba[out + c] = (top * (1.0 - fy) + bottom * fy).round() as u8;
            }
            rgba[out + 3] = 255;
        }
    }
    // The bright stars, as small spots in linear light: as bright as their
    // flux allows up to a magnitude 2.3 star, which fills its spot.
    let turn = worldline_data::equatorial_to_galactic();
    let sigma = 0.6;
    for star in worldline_data::bright_stars() {
        let galactic = turn * star.equatorial();
        let (cx, cy) = texel(galactic);
        let peak = (8.0 * 10f64.powf(-0.4 * star.vmag)).min(1.0);
        let color = star_color(star.b_v);
        // Near the poles a spot stretches across more texels of the map.
        let stretch = 1.0 / galactic.z.clamp(-1.0, 1.0).acos().sin().max(1.0 / 64.0);
        let (reach_x, reach_y) = (
            (3.0 * sigma * stretch).ceil() as i64,
            (3.0 * sigma).ceil() as i64,
        );
        for dy in -reach_y..=reach_y {
            let py = cy.floor() as i64 + dy;
            if py < 0 || py >= i64::from(HEIGHT) {
                continue;
            }
            for dx in -reach_x..=reach_x {
                let px = (cx.floor() as i64 + dx).rem_euclid(i64::from(WIDTH));
                let (ox, oy) = (
                    (cx.floor() as i64 + dx) as f64 + 0.5 - cx,
                    py as f64 + 0.5 - cy,
                );
                let weight = (-0.5 * ((ox / stretch).powi(2) + oy * oy) / (sigma * sigma)).exp();
                let out = ((py as u32 * WIDTH + px as u32) * 4) as usize;
                for c in 0..3 {
                    rgba[out + c] = encoded(linear(rgba[out + c]) + peak * weight * color[c]);
                }
            }
        }
    }
    (WIDTH, HEIGHT, rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_galactic_center_is_in_the_middle_of_the_map() {
        // Sagittarius A*'s direction, from the simulation's axes, lands in
        // the middle of the map, within a texel.
        let (x, y) = texel(to_galactic() * worldline_data::sgr_a_star_direction());
        println!("Sagittarius A* at texel ({x:.2}, {y:.2})");
        assert!((x - f64::from(WIDTH) / 2.0).abs() < 1.0);
        assert!((y - f64::from(HEIGHT) / 2.0).abs() < 1.0);
    }

    #[test]
    fn hot_stars_are_blue_and_cool_ones_red() {
        // From Spica (B − V = −0.23) through Vega (0.00), the Sun's color
        // (0.65), Arcturus (1.23) to Betelgeuse (1.85), blue gives way to
        // red; a star of the Sun's color is nearly white.
        let colors: Vec<[f64; 3]> = [-0.23, 0.0, 0.65, 1.23, 1.85]
            .map(|bv| star_color(Some(bv)))
            .to_vec();
        for pair in colors.windows(2) {
            assert!(pair[0][2] / pair[0][0] > pair[1][2] / pair[1][0]);
        }
        assert!(colors[2].iter().all(|&c| c > 0.85));
    }
}
