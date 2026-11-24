//! The app capturing its own frames (steps 8.8a and 8.8b). A capture run
//! opens its window off the screen, without taking the keyboard and mouse
//! (as `--offscreen` does), and saves frames as the GPU drew them: what's
//! saved is exactly what a person would see, without the screen around it.
//!
//! - `--smoke-test frame.png` (step 8.8a), the clean-machine check: lets
//!   time run for 60 frames, saves the last and closes, saying what it
//!   found. The release workflow runs it on a fresh Windows machine with
//!   only the downloaded zip. It passes when the frame was drawn and the
//!   physics ran: the 3D view isn't one flat color, as it would be if
//!   nothing reached it but the color it's cleared to, and the
//!   simulation's clock moved on. Neither is a number to tune: a frame
//!   either has a picture in it or it doesn't, and time either ran or it
//!   didn't.
//! - `--capture shot.png --at 4` (step 8.8b): saves the frame 4 s in.
//! - `--record frames --at 2 --for 6 --fps 15`: saves the frames from 2 s
//!   to 8 s, 15 a second, as frames/frame-00000.png and on.
//!
//! While a scenario's film plays, `--at` counts the film's own time, which
//! stands still while the film waits (for the eclipse finder, say, which
//! searches in real time): so a capture is the same moment of the film
//! however fast the computer is. Otherwise it counts the app's time.
//!
//! Stills and recordings run on a fixed clock: each frame is 1/fps of a
//! second of the app's time (1/30 for a still), however long the computer
//! takes to draw and save it. So a recording plays at the pace a person
//! would see, and the same command gives the same frames. The smoke test
//! keeps real time, as a first launch does.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use eframe::egui::{self, ColorImage, Rect};

/// Frames the smoke test draws before the one it saves: a second at 60 a
/// second, time for the first frame's work (textures, the sky) to be
/// behind it.
const SMOKE_FRAMES: u64 = 60;

/// A still's fixed clock: 30 frames a second.
const STILL_FPS: f64 = 30.0;

/// Frames to wait for the GPU's copies past the last asked for, before
/// giving up: they arrive one frame after they're asked for.
const PATIENCE: u64 = 600;

/// Whether the capture succeeded, once it has finished, for the program's
/// exit code.
pub static PASSED: OnceLock<bool> = OnceLock::new();

/// What a capture run saves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    /// The clean-machine check.
    Smoke,
    /// One frame, `at` seconds of the app's time in.
    Still { at: f64 },
    /// `fps` frames a second from `at` seconds in, for `length` seconds.
    Film { at: f64, length: f64, fps: f64 },
}

/// A capture under way.
#[derive(Debug)]
pub struct Capture {
    kind: Kind,
    /// The PNG to save (a smoke test or a still), or the folder for a
    /// recording's frames.
    path: PathBuf,
    /// Frames drawn so far.
    frames: u64,
    /// Frames asked of the GPU, and saved.
    asked: u64,
    saved: u64,
    /// The frame the last request was made in.
    last_asked: u64,
    /// When the first frame was drawn, s from the program's start.
    began: Option<f64>,
    /// The simulation's time at the first frame, s.
    time: Option<f64>,
    /// What the frames saved showed: the fewest colors any had in its 3D
    /// view.
    fewest_colors: Option<usize>,
    /// Why it failed, if it did.
    failure: Option<String>,
    /// Finished: the window is closing, and may draw once more meanwhile.
    done: bool,
}

/// What a frame shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    /// How many different colors the 3D view's pixels have.
    pub colors: usize,
    /// The spread of their brightness, 0 to 1: the standard deviation of
    /// their luma.
    pub spread: f64,
}

impl Capture {
    pub fn new(kind: Kind, path: PathBuf) -> Self {
        Self {
            kind,
            path,
            frames: 0,
            asked: 0,
            saved: 0,
            last_asked: 0,
            began: None,
            time: None,
            fewest_colors: None,
            failure: None,
            done: false,
        }
    }

    /// The fixed step of the app's clock, s a frame, if this run keeps
    /// one.
    pub fn step(&self) -> Option<f64> {
        match self.kind {
            Kind::Smoke => None,
            Kind::Still { .. } => Some(1.0 / STILL_FPS),
            Kind::Film { fps, .. } => Some(1.0 / fps),
        }
    }

    /// Frames this run saves.
    fn wanted(&self) -> u64 {
        match self.kind {
            Kind::Smoke | Kind::Still { .. } => 1,
            Kind::Film { length, fps, .. } => (length * fps).round().max(1.0) as u64,
        }
    }

    /// Whether the frame drawn now, the `frame`th (0 the first), is to be
    /// saved, and as which: its number among those saved. `film` is how
    /// far into a playing film it is, s. A recording, once begun, saves
    /// every frame to its end.
    pub fn wants(&self, frame: u64, film: Option<f64>) -> Option<u64> {
        if self.asked >= self.wanted() {
            return None;
        }
        // The app's time at this frame is frame × step, unless a film's
        // own time counts.
        let time = |step: f64| film.unwrap_or(frame as f64 * step);
        // A little slack keeps 2.0 from coming out 1.9999 a frame late.
        let due = match self.kind {
            Kind::Smoke => frame + 1 >= SMOKE_FRAMES,
            Kind::Still { at } => time(1.0 / STILL_FPS) >= at - 1e-9,
            Kind::Film { at, fps, .. } => self.asked > 0 || time(1.0 / fps) >= at - 1e-9,
        };
        due.then_some(self.asked)
    }

    /// One frame of the run, after it's drawn. `view` is where the 3D view
    /// was drawn, `film` how far into a playing film it is, s, `time` the
    /// simulation's time, s, and `gpu` what draws it. Frames it saves are
    /// asked of the GPU now and arrive a frame later; when the last has,
    /// it reports and closes the window.
    pub fn frame(
        &mut self,
        ctx: &egui::Context,
        view: Rect,
        film: Option<f64>,
        time: f64,
        gpu: &str,
    ) {
        if self.done {
            return;
        }
        let now = crate::since_start();
        let began = *self.began.get_or_insert(now);
        let simulated = time - *self.time.get_or_insert(time);
        if let Some(number) = self.wants(self.frames, film) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(
                number,
            )));
            self.asked += 1;
            self.last_asked = self.frames;
        }
        self.frames += 1;
        ctx.request_repaint();

        let arrived: Vec<(u64, std::sync::Arc<ColorImage>)> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|event| match event {
                    egui::Event::Screenshot {
                        image, user_data, ..
                    } => {
                        let number = user_data
                            .data
                            .as_ref()
                            .and_then(|d| d.downcast_ref::<u64>())
                            .copied()?;
                        Some((number, image.clone()))
                    }
                    _ => None,
                })
                .collect()
        });
        let pixels = view * ctx.pixels_per_point();
        for (number, image) in arrived {
            let seen = look(&image, pixels);
            self.fewest_colors = Some(
                self.fewest_colors
                    .map_or(seen.colors, |c| c.min(seen.colors)),
            );
            let path = match self.kind {
                Kind::Film { .. } => self.path.join(format!("frame-{number:05}.png")),
                _ => self.path.clone(),
            };
            if let Err(e) = save(&image, &path) {
                self.failure
                    .get_or_insert(format!("couldn't save {}: {e}", path.display()));
            }
            self.saved += 1;
            if matches!(self.kind, Kind::Smoke) {
                self.report_smoke(seen, image.size, now - began, began, simulated, gpu);
            }
        }

        let waited = self.asked > 0 && self.frames > self.last_asked + PATIENCE;
        if self.saved >= self.wanted() || waited {
            if waited {
                self.failure.get_or_insert(format!(
                    "{} of {} frames arrived from the GPU",
                    self.saved,
                    self.wanted()
                ));
            }
            self.finish(simulated, gpu);
            self.done = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    /// The smoke test's report on what it saw.
    fn report_smoke(
        &mut self,
        seen: Seen,
        size: [usize; 2],
        seconds: f64,
        began: f64,
        simulated: f64,
        gpu: &str,
    ) {
        println!(
            "Worldline {} · drawn by {gpu} · {} frames in {seconds:.1} s, the first after {began:.2} s · {}×{} pixels",
            env!("CARGO_PKG_VERSION"),
            self.frames,
            size[0],
            size[1],
        );
        println!(
            "the 3D view: {} colors, brightness spread {:.3} ({}) · the simulation ran {:.1} days ({})",
            seen.colors,
            seen.spread,
            if seen.colors > 1 {
                "drawn"
            } else {
                "one flat color: NOT DRAWN"
            },
            simulated / 86_400.0,
            if simulated > 0.0 { "ran" } else { "DIDN'T RUN" },
        );
        if seen.colors <= 1 {
            self.failure
                .get_or_insert("the 3D view wasn't drawn".to_string());
        }
        if simulated <= 0.0 {
            self.failure
                .get_or_insert("the simulation didn't run".to_string());
        }
    }

    /// Says what was saved, and whether it all went right.
    fn finish(&mut self, simulated: f64, gpu: &str) {
        if self.fewest_colors.is_some_and(|c| c <= 1) {
            self.failure
                .get_or_insert("a frame's 3D view was one flat color".to_string());
        }
        match self.kind {
            Kind::Smoke => {}
            Kind::Still { at } => println!(
                "saved the frame {at} s in to {} · drawn by {gpu}",
                self.path.display()
            ),
            Kind::Film { at, length, fps } => println!(
                "saved {} frames, {fps} a second from {at} s for {length} s, to {} · drawn by {gpu} · the simulation ran {:.1} days",
                self.saved,
                self.path.display(),
                simulated / 86_400.0
            ),
        }
        if matches!(self.kind, Kind::Smoke) && self.failure.is_none() {
            println!("saved the frame to {}", self.path.display());
        }
        match &self.failure {
            None => println!("{} passed", self.name()),
            Some(why) => println!("{} FAILED: {why}", self.name()),
        }
        let _ = PASSED.set(self.failure.is_none());
    }

    fn name(&self) -> &'static str {
        match self.kind {
            Kind::Smoke => "smoke test",
            Kind::Still { .. } => "capture",
            Kind::Film { .. } => "recording",
        }
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
pub fn save(image: &ColorImage, path: &Path) -> Result<(), String> {
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
        assert!((left.spread - 0.5).abs() < 1e-12, "{}", left.spread);
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
        let folder = std::env::temp_dir().join("worldline-capture-test");
        let path = folder.join("frame.png");
        save(&image, &path).unwrap();
        let back = image::open(&path).unwrap().to_rgba8();
        assert_eq!(back.dimensions(), (3, 2));
        assert_eq!(back.get_pixel(1, 1).0, [110, 160, 255, 255]);
        std::fs::remove_dir_all(folder).unwrap();
    }

    /// The frames a capture asks for, among the first `frames`, as
    /// (frame, its number among those saved), as if each request were
    /// made in turn.
    fn asked(kind: Kind, frames: u64) -> Vec<(u64, u64)> {
        asked_in_film(kind, frames, |_| None)
    }

    /// The same, with a film's time at each frame.
    fn asked_in_film(
        kind: Kind,
        frames: u64,
        film: impl Fn(u64) -> Option<f64>,
    ) -> Vec<(u64, u64)> {
        let mut capture = Capture::new(kind, PathBuf::new());
        let mut asked = Vec::new();
        for frame in 0..frames {
            if let Some(number) = capture.wants(frame, film(frame)) {
                asked.push((frame, number));
                capture.asked += 1;
            }
        }
        asked
    }

    #[test]
    fn a_smoke_test_saves_its_sixtieth_frame() {
        assert_eq!(asked(Kind::Smoke, 200), vec![(59, 0)]);
        assert_eq!(Capture::new(Kind::Smoke, PathBuf::new()).step(), None);
    }

    #[test]
    fn a_still_is_the_frame_at_its_time_on_a_fixed_clock() {
        // 2 s at 30 frames a second: the 61st frame, at 60/30 = 2 s.
        assert_eq!(asked(Kind::Still { at: 2.0 }, 500), vec![(60, 0)]);
        // From the start, the first frame.
        assert_eq!(asked(Kind::Still { at: 0.0 }, 5), vec![(0, 0)]);
        let still = Capture::new(Kind::Still { at: 2.0 }, PathBuf::new());
        assert_eq!(still.step(), Some(1.0 / 30.0));
    }

    #[test]
    fn a_recording_is_every_frame_from_its_start_for_its_length() {
        let film = Kind::Film {
            at: 2.0,
            length: 3.0,
            fps: 15.0,
        };
        let asked = asked(film, 1000);
        // 3 s at 15 frames a second is 45 frames, from the 31st (30/15 =
        // 2 s), each the next, numbered from 0.
        assert_eq!(asked.len(), 45);
        assert_eq!(asked.first(), Some(&(30, 0)));
        assert_eq!(asked.last(), Some(&(74, 44)));
        assert!(
            asked
                .windows(2)
                .all(|w| w[1].0 == w[0].0 + 1 && w[1].1 == w[0].1 + 1)
        );
        assert_eq!(Capture::new(film, PathBuf::new()).step(), Some(1.0 / 15.0));
    }

    #[test]
    fn in_a_film_the_films_own_time_counts_and_a_recording_runs_on_through_a_wait() {
        // A film that waits from frame 30 to frame 129 (it stands at 2 s),
        // then runs on 1/15 s a frame.
        let film = |frame: u64| {
            let frames = if frame < 30 {
                frame
            } else {
                frame.saturating_sub(100).max(30)
            };
            Some(frames as f64 / 15.0)
        };
        let still = asked_in_film(Kind::Still { at: 3.0 }, 1000, film);
        // 3 s of the film is 15 frames past the wait's end, at frame 145,
        // where the app's own time alone (frame 90) would have been early.
        assert_eq!(still, vec![(145, 0)]);
        let recording = Kind::Film {
            at: 1.0,
            length: 2.0,
            fps: 15.0,
        };
        // Begun at 1 s, frame 15, it saves 30 frames in a row through the
        // wait: they show the film waiting, as it does.
        let asked = asked_in_film(recording, 1000, film);
        assert_eq!(asked.len(), 30);
        assert_eq!(asked.first(), Some(&(15, 0)));
        assert_eq!(asked.last(), Some(&(44, 29)));
    }
}
