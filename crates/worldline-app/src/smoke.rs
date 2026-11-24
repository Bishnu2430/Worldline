//! The clean-machine check (step 8.8a): `--smoke-test PATH` opens the
//! solar system as a first launch would, lets time run for a few dozen
//! frames, saves the last one to PATH as a PNG and closes, saying what it
//! found. The release workflow runs it on a fresh Windows machine that has
//! only the downloaded zip, to show the program starts there and draws.
//!
//! It passes when the frame was drawn and the physics ran:
//! - the 3D view isn't one flat color, as it would be if nothing reached
//!   the screen but the color it's cleared to;
//! - the simulation's clock moved on from the snapshot.
//!
//! Neither is a number to tune: a frame either has a picture in it or it
//! doesn't, and time either ran or it didn't.

use std::path::PathBuf;
use std::sync::OnceLock;

use eframe::egui::{self, ColorImage, Rect};

/// Frames to draw before the one that's saved: a second at 60 a second,
/// time for the first frame's work (textures, the sky) to be behind it.
const FRAMES: u32 = 60;

/// Whether the check passed, once it has finished, for the program's exit
/// code.
pub static PASSED: OnceLock<bool> = OnceLock::new();

/// A smoke test under way.
#[derive(Debug)]
pub struct Smoke {
    path: PathBuf,
    frames: u32,
    asked: bool,
    began: Option<f64>,
    /// The simulation's time at the first frame, s.
    time: Option<f64>,
}

/// What the saved frame shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    /// How many different colors the 3D view's pixels have.
    pub colors: usize,
    /// The spread of their brightness, 0 to 1: the standard deviation of
    /// their luma.
    pub spread: f64,
}

impl Smoke {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            frames: 0,
            asked: false,
            began: None,
            time: None,
        }
    }

    /// One frame of the test. `view` is where the 3D view was drawn,
    /// `time` the simulation's time, s, and `gpu` what draws it. When the
    /// frame it asked for arrives, the test reports, saves it and closes
    /// the window.
    pub fn frame(&mut self, ctx: &egui::Context, view: Rect, time: f64, gpu: &str) {
        let now = crate::since_start();
        let began = *self.began.get_or_insert(now);
        let simulated = time - *self.time.get_or_insert(time);
        self.frames += 1;
        ctx.request_repaint();
        if self.frames >= FRAMES && !self.asked {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            self.asked = true;
        }
        let image = ctx.input(|i| {
            i.raw.events.iter().find_map(|event| match event {
                egui::Event::Screenshot { image, .. } => Some(image.clone()),
                _ => None,
            })
        });
        let Some(image) = image else {
            return;
        };
        let pixels = view * ctx.pixels_per_point();
        let seen = look(&image, pixels);
        let saved = save(&image, &self.path);
        let drawn = seen.colors > 1;
        let ran = simulated > 0.0;
        let passed = drawn && ran && saved.is_ok();
        println!(
            "Worldline {} · drawn by {gpu} · {} frames in {:.1} s, the first after {:.2} s · {}×{} pixels",
            env!("CARGO_PKG_VERSION"),
            self.frames,
            now - began,
            began,
            image.size[0],
            image.size[1],
        );
        println!(
            "the 3D view: {} colors, brightness spread {:.3} ({}) · the simulation ran {:.1} days ({})",
            seen.colors,
            seen.spread,
            if drawn {
                "drawn"
            } else {
                "one flat color: NOT DRAWN"
            },
            simulated / 86_400.0,
            if ran { "ran" } else { "DIDN'T RUN" },
        );
        match saved {
            Ok(()) => println!("saved the frame to {}", self.path.display()),
            Err(e) => println!("COULDN'T SAVE the frame to {}: {e}", self.path.display()),
        }
        println!("smoke test {}", if passed { "passed" } else { "FAILED" });
        let _ = PASSED.set(passed);
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// The colors and brightness of the image's pixels inside `area`, in
/// pixels.
pub fn look(image: &ColorImage, area: Rect) -> Seen {
    let [width, height] = image.size;
    let x0 = (area.min.x.max(0.0) as usize).min(width);
    let x1 = (area.max.x.max(0.0) as usize).min(width);
    let y0 = (area.min.y.max(0.0) as usize).min(height);
    let y1 = (area.max.y.max(0.0) as usize).min(height);
    let mut colors = std::collections::HashSet::new();
    // Welford's running mean and sum of squared differences from it: a
    // flat frame's spread comes out exactly 0, with no difference of two
    // nearly equal sums to round.
    let (mut mean, mut squares, mut n) = (0.0, 0.0, 0.0);
    for y in y0..y1 {
        for pixel in &image.pixels[y * width + x0..y * width + x1] {
            colors.insert(pixel.to_array());
            // Rec. 709 luma, of the stored (sRGB) values.
            let luma = (0.2126 * f64::from(pixel.r())
                + 0.7152 * f64::from(pixel.g())
                + 0.0722 * f64::from(pixel.b()))
                / 255.0;
            n += 1.0;
            let step = luma - mean;
            mean += step / n;
            squares += step * (luma - mean);
        }
    }
    Seen {
        colors: colors.len(),
        spread: if n > 0.0 { (squares / n).sqrt() } else { 0.0 },
    }
}

/// The image as a PNG.
pub fn save(image: &ColorImage, path: &std::path::Path) -> Result<(), String> {
    let [width, height] = image.size;
    let bytes: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_array()).collect();
    let buffer = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .ok_or("the frame's size doesn't match its pixels")?;
    if let Some(folder) = path.parent().filter(|f| !f.as_os_str().is_empty()) {
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    }
    buffer.save(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::{Color32, pos2};

    #[test]
    fn a_flat_frame_has_one_color_and_no_spread() {
        let image = ColorImage::new([40, 30], vec![Color32::from_rgb(9, 11, 16); 40 * 30]);
        let seen = look(&image, Rect::from_min_max(pos2(0.0, 0.0), pos2(40.0, 30.0)));
        assert_eq!(seen.colors, 1);
        assert_eq!(seen.spread, 0.0);
    }

    #[test]
    fn a_drawn_frame_has_colors_and_spread_only_where_it_looks() {
        let mut image = ColorImage::new([40, 30], vec![Color32::BLACK; 40 * 30]);
        // Half the left half white: the right half stays flat.
        for y in 0..15 {
            for x in 0..20 {
                image.pixels[y * 40 + x] = Color32::WHITE;
            }
        }
        let left = look(&image, Rect::from_min_max(pos2(0.0, 0.0), pos2(20.0, 30.0)));
        assert_eq!(left.colors, 2);
        // Half the pixels at 1 and half at 0: a standard deviation of 1/2.
        assert!((left.spread - 0.5).abs() < 1e-9, "{}", left.spread);
        let right = look(
            &image,
            Rect::from_min_max(pos2(20.0, 0.0), pos2(40.0, 30.0)),
        );
        assert_eq!(right.colors, 1);
        // An area past the image's edge is cut to it.
        let past = look(
            &image,
            Rect::from_min_max(pos2(30.0, 20.0), pos2(90.0, 90.0)),
        );
        assert_eq!(past.colors, 1);
    }

    #[test]
    fn a_frame_saves_as_a_png_that_reads_back() {
        let mut image = ColorImage::new([3, 2], vec![Color32::BLACK; 6]);
        image.pixels[4] = Color32::from_rgb(110, 160, 255);
        let folder = std::env::temp_dir().join("worldline-smoke-test");
        let path = folder.join("frame.png");
        save(&image, &path).unwrap();
        let back = image::open(&path).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (3, 2));
        assert_eq!(back.get_pixel(1, 1).0, [110, 160, 255, 255]);
        std::fs::remove_dir_all(folder).unwrap();
    }
}
