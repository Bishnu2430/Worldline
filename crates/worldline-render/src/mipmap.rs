//! Mipmaps: successively halved copies of a texture, so a planet a few
//! pixels across samples a properly averaged image instead of sparkling.

/// One level of a mipmap chain: width, height and sRGB RGBA8 pixels.
pub struct Level {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Builds the levels below a full-size sRGB RGBA8 image, down to 1×1.
/// Each 2×2 block is averaged in linear light, so shrinking doesn't darken
/// colors the way averaging raw sRGB values would.
pub fn chain(width: u32, height: u32, rgba: &[u8]) -> Vec<Level> {
    assert_eq!(
        rgba.len(),
        (width * height * 4) as usize,
        "image size mismatch"
    );
    let mut linear: Vec<f32> = rgba
        .chunks(4)
        .flat_map(|p| {
            [
                to_linear(p[0]),
                to_linear(p[1]),
                to_linear(p[2]),
                f32::from(p[3]) / 255.0,
            ]
        })
        .collect();
    let (mut w, mut h) = (width, height);
    let mut levels = Vec::new();
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0.0; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let sample = |sx: u32, sy: u32| {
                        linear[((sy.min(h - 1) * w + sx.min(w - 1)) * 4 + c) as usize]
                    };
                    let sum = sample(2 * x, 2 * y)
                        + sample(2 * x + 1, 2 * y)
                        + sample(2 * x, 2 * y + 1)
                        + sample(2 * x + 1, 2 * y + 1);
                    next[((y * nw + x) * 4 + c) as usize] = sum / 4.0;
                }
            }
        }
        let rgba = next
            .chunks(4)
            .flat_map(|p| {
                [
                    to_srgb(p[0]),
                    to_srgb(p[1]),
                    to_srgb(p[2]),
                    (p[3] * 255.0).round() as u8,
                ]
            })
            .collect();
        levels.push(Level {
            width: nw,
            height: nh,
            rgba,
        });
        linear = next;
        (w, h) = (nw, nh);
    }
    levels
}

/// sRGB-encoded byte to linear light (IEC 61966-2-1).
fn to_linear(value: u8) -> f32 {
    let c = f32::from(value) / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to an sRGB-encoded byte.
fn to_srgb(linear: f32) -> u8 {
    let c = linear.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_halve_down_to_one_pixel() {
        let sizes: Vec<_> = chain(8, 4, &[0; 8 * 4 * 4])
            .iter()
            .map(|l| (l.width, l.height))
            .collect();
        assert_eq!(sizes, [(4, 2), (2, 1), (1, 1)]);
    }

    #[test]
    fn averaging_happens_in_linear_light() {
        // Half black, half white averages to 50% light, which sRGB encodes
        // as 188, not the naive 128.
        let rgba = [0, 0, 0, 255, 255, 255, 255, 255];
        let level = &chain(2, 1, &rgba)[0];
        assert_eq!(&level.rgba, &[188, 188, 188, 255]);
    }

    #[test]
    fn a_flat_color_stays_the_same() {
        let rgba: Vec<u8> = [90, 150, 255, 255].repeat(16);
        for level in chain(4, 4, &rgba) {
            assert!(level.rgba.chunks(4).all(|p| p == [90, 150, 255, 255]));
        }
    }
}
