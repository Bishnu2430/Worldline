//! Worldline: a relativistic universe sandbox.
//!
//! The desktop app. Physics lives in `worldline-core`, real-world data in
//! `worldline-data` and GPU drawing in `worldline-render`; this crate is the
//! window, the 3D view and the controls.
//!
//! Usage: `worldline [--focus BODY] [--zoom RADII] [--paused] [--advance YEARS]`
//! - `--focus Earth` starts with the camera flown in to Earth.
//! - `--zoom 60` puts the camera 60 of the focused body's radii away
//!   (default 4), for example to see Jupiter with its moons.
//! - `--pitch 8` looks from 8° above the ecliptic plane (default 34°),
//!   for example to see a black hole's disk nearly edge on.
//! - `--yaw 180` looks from that direction around the ecliptic, measured
//!   from the x axis, for example to see the side of a planet that was hit.
//! - `--moon-impact 8000` runs the Moon-forming impact with about that many
//!   particles, in a window of its own.
//! - `--overlays tides,clocks` starts with those overlays on: any of
//!   models, clocks, tides, embedding, waves and time (proper time).
//! - `--paused` starts with time stopped.
//! - `--speed hour` runs at one hour per second (also real, for real
//!   time, and day, week, month, year, decade and century; month is the
//!   default).
//! - `--now` starts with the solar system as it is right now, running in
//!   real time with the computer's clock.
//! - `--advance 7` runs the physics 7 years past the 2025-01-01 snapshot
//!   before showing anything.
//! - `--add "Gaia BH1:3"` drops a catalogue body on a circular orbit 3 AU
//!   from the Sun; `--add "TON 618:6600@Sagittarius A*"`, 6,600 AU from
//!   Sagittarius A*.
//! - `--launch 0.5` starts the body the last `--add` adds at half the
//!   circular speed, so it falls in on an ellipse (0 drops it from rest).
//!   `--add` can be given more than once: `--add "Gaia BH1:40" --add
//!   "jupiter:2@Gaia BH1" --launch 0.071` drops a Jupiter 2 AU from a
//!   black hole 40 AU out, to fall past it at half its tidal radius.
//! - `--eclipses` opens the eclipse finder, searching the next 5 years;
//!   `--eclipse 7` goes to the 7th eclipse it finds, once found (from the
//!   2025 snapshot, the total solar eclipse of 12 August 2026).
//! - `--sky` opens the sky window.
//! - `--stand 38.88,-77.03` stands on Earth at that latitude and longitude,
//!   looking at the sky; `--look 120,30` faces azimuth 120°, 30° up, and
//!   `--fov 3` narrows the view to 3° across.
//! - `--aim 30,0.8` sends the body the last `--add` adds straight at what
//!   it circles, to arrive at 30 km/s with impact parameter 0.8 (0 head-on,
//!   1 grazing).
//! - `--fluid Jupiter` models Jupiter as a self-gravitating fluid of SPH
//!   particles on the GPU, as many as it steps smoothly.
//! - `--scenario mercury` plays a scenario from the library (step 8.1):
//!   mercury, hulse-taylor, double-pulsar, gw150914, gw170817,
//!   planet-black-hole, ton-618, white-dwarf, mini-black-hole, eclipse,
//!   moon-impact or black-hole-flyby. It starts afresh, after the options above.
//! - `--scenarios` opens the scenario window.
//! - `--manual` opens the manual (step 8.6).
//! - `--timings` shows the frame's timings over the view (F3 toggles them).
//! - `--quality low`, `medium` or `high` draws at that preset rather than
//!   the one picked for this computer (see `quality.rs`).
//! - `--no-vsync` draws frames as fast as they come, for measuring what
//!   drawing costs.
//! - `--measure SECONDS` measures the frames for that long (the last two
//!   thirds of it), prints their mean timings on one line and closes.
//! - `--smoke-test frame.png` draws the solar system for a few dozen
//!   frames, saves the last to frame.png, says whether it was drawn and
//!   the physics ran, and closes, failing (exit code 1) if not (step 8.8a).
//! - `--capture shot.png --at 4` saves the frame 4 s of the app's time in
//!   and closes; `--record frames --at 2 --for 6 --fps 15` saves frames
//!   from 2 s to 8 s, 15 a second, into the folder frames (step 8.8b).
//!   Both run on a fixed clock: each frame is 1/fps s of the app's time
//!   (1/30 for a still), however long it takes to draw and save. While a
//!   scenario's film plays, `--at` counts the film's own time. Captures
//!   open their window as `--offscreen` does (see `capture.rs`).
//! - `--offscreen` opens the window off the screen and without taking the
//!   keyboard and mouse, for runs that capture frames.
//! - `--software` draws with the processor (Windows' WARP) rather than the
//!   graphics card: slow, for computers whose graphics driver fails.
//!
//! A release build on Windows opens no console window; what it would
//! print shows when its output is redirected (`worldline.exe --smoke-test
//! frame.png | Out-Host`), and an error that stops it opens a message box.

// No console window behind the app in a release build on Windows.
#![cfg_attr(
    all(windows, not(debug_assertions), not(test)),
    windows_subsystem = "windows"
)]

mod app;
mod calendar;
mod camera;
mod capture;
mod catalogue;
mod clocks;
mod compact_view;
mod details;
mod eclipse_finder;
mod feel;
mod fluid;
mod gpu;
mod ground;
mod impact_lab;
mod manual;
mod overlays;
mod quality;
mod sandbox;
mod simulation;
mod sky;
mod textures;
mod theme;
mod view;
mod waves;

use eframe::egui;

/// A body to add before starting.
#[derive(Debug, Default, PartialEq)]
pub struct Addition {
    /// Which: "earth", "jupiter", "sun" or a catalogue entry's name, like
    /// "Sagittarius A*".
    pub kind: String,
    /// How far from what it circles, in AU.
    pub au: f64,
    /// What it circles, if not the Sun.
    pub around: Option<String>,
    /// The share of the circular speed it starts with, if not 1.
    pub launch: Option<f64>,
    /// Sent at what it circles instead, to arrive at this speed (km/s) and
    /// impact parameter.
    pub aim: Option<(f64, f64)>,
}

/// Options from the command line.
#[derive(Debug, Default, PartialEq)]
pub struct StartOptions {
    /// Name of the body to fly to at startup.
    pub focus: Option<String>,
    /// Camera distance from the focused body, in its radii.
    pub zoom_radii: Option<f64>,
    /// Camera height above the ecliptic plane, as an angle in degrees.
    pub pitch_degrees: Option<f64>,
    /// Camera direction around the ecliptic plane, as an angle in degrees.
    pub yaw_degrees: Option<f64>,
    /// Run the Moon-forming impact with about this many particles.
    pub moon_impact: Option<usize>,
    /// Start with time stopped.
    pub paused: bool,
    /// Start now, in real time.
    pub now: bool,
    /// How fast to start: which of the speeds, slowest first.
    pub speed: Option<usize>,
    /// Start with the catalogue window open.
    pub catalogue: bool,
    /// Start with the eclipse finder open, searching.
    pub eclipses: bool,
    /// Go to the eclipse the finder finds in this place (1 the first),
    /// once found.
    pub eclipse: Option<usize>,
    /// Years to simulate past the snapshot before starting.
    pub advance_years: f64,
    /// Bodies to add before starting, in order.
    pub adds: Vec<Addition>,
    /// Open the gravitational-wave window for the focused body's pair.
    pub waves: bool,
    /// Overlays to start with.
    pub overlays: overlays::Overlays,
    /// A body to model as a fluid.
    pub fluid: Option<String>,
    /// Start with the sky window open.
    pub sky: bool,
    /// A place on Earth to stand at, as latitude and longitude in degrees.
    pub stand: Option<(f64, f64)>,
    /// Where to look from there, as azimuth and altitude in degrees.
    pub look: Option<(f64, f64)>,
    /// The field of view there, in degrees.
    pub fov: Option<f64>,
    /// A scenario from the library to play, by its key.
    pub scenario: Option<String>,
    /// Start with the scenario window open.
    pub scenarios: bool,
    /// Start with the manual open.
    pub manual: bool,
    /// Start showing the frame's timings.
    pub timings: bool,
    /// The quality preset to draw at, rather than the one picked for this
    /// computer.
    pub quality: Option<quality::Quality>,
    /// Draw frames as fast as they come, not in step with the screen's
    /// refresh: for measuring what drawing costs.
    pub no_vsync: bool,
    /// Measure the frames for this many seconds, print their timings and
    /// close (step 8.4b).
    pub measure: Option<f64>,
    /// Draw a few dozen frames, save the last here, check it and close
    /// (step 8.8a).
    pub smoke_test: Option<std::path::PathBuf>,
    /// Save one frame here, `at` seconds in, and close (step 8.8b).
    pub capture: Option<std::path::PathBuf>,
    /// Save frames into this folder, from `at` seconds in for
    /// `record_for`, `fps` a second, and close (step 8.8b).
    pub record: Option<std::path::PathBuf>,
    /// When a capture or recording starts, s of the app's time.
    pub at: Option<f64>,
    /// How long a recording lasts, s.
    pub record_for: Option<f64>,
    /// A recording's frames a second.
    pub fps: Option<f64>,
    /// Open the window off the screen, without taking the keyboard and
    /// mouse.
    pub offscreen: bool,
    /// Draw with the processor rather than the graphics card.
    pub software: bool,
}

impl StartOptions {
    /// The capture this run makes, if any: the smoke test, a still or a
    /// recording, the first of them asked for.
    pub fn capture(&self) -> Option<(capture::Kind, std::path::PathBuf)> {
        let at = self.at.unwrap_or(0.0);
        if let Some(path) = &self.smoke_test {
            Some((capture::Kind::Smoke, path.clone()))
        } else if let Some(path) = &self.capture {
            Some((
                capture::Kind::Still {
                    at: self.at.unwrap_or(2.0),
                },
                path.clone(),
            ))
        } else {
            self.record.as_ref().map(|folder| {
                let kind = capture::Kind::Film {
                    at,
                    length: self.record_for.unwrap_or(6.0),
                    fps: self.fps.unwrap_or(15.0),
                };
                (kind, folder.clone())
            })
        }
    }
}

/// Two numbers with a comma between them.
fn pair(text: &str) -> Option<(f64, f64)> {
    let (a, b) = text.split_once(',')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

impl StartOptions {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut options = Self::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--focus" => options.focus = args.next(),
                "--fluid" => options.fluid = args.next(),
                "--sky" => options.sky = true,
                "--scenarios" => options.scenarios = true,
                "--manual" => options.manual = true,
                "--timings" => options.timings = true,
                "--quality" => match args.next().as_deref().and_then(quality::Quality::parse) {
                    Some(q) => options.quality = Some(q),
                    None => eprintln!("--quality takes low, medium or high"),
                },
                "--no-vsync" => options.no_vsync = true,
                "--smoke-test" => match args.next() {
                    Some(path) => options.smoke_test = Some(path.into()),
                    None => eprintln!("worldline: --smoke-test needs a file to save the frame to"),
                },
                "--capture" => match args.next() {
                    Some(path) => options.capture = Some(path.into()),
                    None => eprintln!("worldline: --capture needs a file to save the frame to"),
                },
                "--record" => match args.next() {
                    Some(path) => options.record = Some(path.into()),
                    None => eprintln!("worldline: --record needs a folder to save the frames in"),
                },
                "--at" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                    Some(s) if s >= 0.0 => options.at = Some(s),
                    _ => eprintln!("worldline: --at needs a number of seconds, 0 or more"),
                },
                "--for" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                    Some(s) if s > 0.0 => options.record_for = Some(s),
                    _ => eprintln!("worldline: --for needs a number of seconds above 0"),
                },
                "--fps" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                    Some(f) if (1.0..=60.0).contains(&f) => options.fps = Some(f),
                    _ => eprintln!("worldline: --fps needs frames a second, 1 to 60"),
                },
                "--offscreen" => options.offscreen = true,
                "--software" => options.software = true,
                "--measure" => match args.next().and_then(|s| s.parse::<f64>().ok()) {
                    Some(s) if s > 0.0 => options.measure = Some(s),
                    _ => eprintln!("--measure takes a number of seconds"),
                },
                "--scenario" => match args.next() {
                    Some(key) => options.scenario = Some(key),
                    None => eprintln!("worldline: --scenario needs a scenario's key"),
                },
                "--stand" => match args.next().and_then(|a| pair(&a)) {
                    Some(place) if place.0.abs() <= 90.0 => options.stand = Some(place),
                    _ => eprintln!("worldline: --stand needs latitude,longitude in degrees"),
                },
                "--look" => match args.next().and_then(|a| pair(&a)) {
                    Some(look) => options.look = Some(look),
                    None => eprintln!("worldline: --look needs azimuth,altitude in degrees"),
                },
                "--fov" => match args.next().and_then(|a| a.parse::<f64>().ok()) {
                    Some(degrees) if degrees > 0.0 => options.fov = Some(degrees),
                    _ => eprintln!("worldline: --fov needs a field of view in degrees"),
                },
                "--launch" => match (
                    args.next().and_then(|f| f.parse::<f64>().ok()),
                    options.adds.last_mut(),
                ) {
                    (Some(share), Some(added)) if share >= 0.0 => added.launch = Some(share),
                    _ => eprintln!(
                        "worldline: --launch needs a share of the circular speed, 0 or more, after an --add"
                    ),
                },
                "--aim" => match (args.next().and_then(|a| pair(&a)), options.adds.last_mut()) {
                    (Some(aim), Some(added)) if aim.0 >= 0.0 && aim.1.abs() <= 1.0 => {
                        added.aim = Some(aim);
                    }
                    _ => eprintln!(
                        "worldline: --aim needs a speed in km/s and an impact parameter from 0 to 1, after an --add"
                    ),
                },
                "--paused" => options.paused = true,
                "--now" => options.now = true,
                "--catalogue" => options.catalogue = true,
                "--eclipses" => options.eclipses = true,
                "--eclipse" => match args.next().and_then(|n| n.parse::<usize>().ok()) {
                    Some(n) if n >= 1 => {
                        options.eclipses = true;
                        options.eclipse = Some(n);
                    }
                    _ => eprintln!("worldline: --eclipse needs which eclipse ahead, 1 or more"),
                },
                "--waves" => options.waves = true,
                "--zoom" => match args.next().and_then(|r| r.parse::<f64>().ok()) {
                    Some(radii) if radii > 1.0 => options.zoom_radii = Some(radii),
                    _ => eprintln!("worldline: --zoom needs a number of radii above 1"),
                },
                "--overlays" => {
                    for name in args.next().unwrap_or_default().split(',') {
                        let overlays = &mut options.overlays;
                        match name.trim() {
                            "models" => overlays.models = true,
                            "clocks" => overlays.clocks = true,
                            "tides" => overlays.tides = true,
                            "embedding" => overlays.embedding = true,
                            "waves" => overlays.waves = true,
                            "time" => overlays.proper_time = true,
                            other => eprintln!(
                                "worldline: no overlay `{other}`: models, clocks, tides, embedding, waves or time"
                            ),
                        }
                    }
                }
                "--speed" => {
                    let names = [
                        "real", "hour", "day", "week", "month", "year", "decade", "century",
                    ];
                    let name = args.next().unwrap_or_default();
                    match names.iter().position(|n| *n == name) {
                        Some(i) => options.speed = Some(i),
                        None => eprintln!("worldline: --speed needs one of {}", names.join(", ")),
                    }
                }
                "--pitch" => match args.next().and_then(|d| d.parse::<f64>().ok()) {
                    Some(degrees) if degrees.abs() < 90.0 => options.pitch_degrees = Some(degrees),
                    _ => {
                        eprintln!("worldline: --pitch needs an angle in degrees between -90 and 90")
                    }
                },
                "--moon-impact" => match args.next().and_then(|n| n.parse::<usize>().ok()) {
                    Some(count) if count >= 500 => options.moon_impact = Some(count),
                    _ => eprintln!("worldline: --moon-impact needs 500 particles or more"),
                },
                "--yaw" => match args.next().and_then(|d| d.parse::<f64>().ok()) {
                    Some(degrees) => options.yaw_degrees = Some(degrees),
                    None => eprintln!("worldline: --yaw needs an angle in degrees"),
                },
                "--advance" => match args.next().and_then(|y| y.parse::<f64>().ok()) {
                    Some(years) if years >= 0.0 => options.advance_years = years,
                    _ => eprintln!("worldline: --advance needs a number of years, 0 or more"),
                },
                "--add" => match args.next().and_then(|a| {
                    let (what, around) = match a.split_once('@') {
                        Some((what, around)) => (what.to_string(), Some(around.to_string())),
                        None => (a, None),
                    };
                    let (kind, au) = what.rsplit_once(':')?;
                    let au = au.parse::<f64>().ok().filter(|au| *au > 0.0)?;
                    (!kind.is_empty()).then(|| Addition {
                        kind: kind.to_string(),
                        au,
                        around,
                        launch: None,
                        aim: None,
                    })
                }) {
                    Some(added) => options.adds.push(added),
                    None => eprintln!(
                        "worldline: --add needs a body and a distance in AU, as jupiter:1.5, \"Gaia BH1:3\" or \"TON 618:6600@Sagittarius A*\""
                    ),
                },
                other => eprintln!("worldline: ignoring unknown argument `{other}`"),
            }
        }
        options
    }
}

/// When the program started, for how long its first frame took.
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// Seconds since the program started.
pub fn since_start() -> f64 {
    STARTED
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs_f64()
}

fn main() {
    STARTED.get_or_init(std::time::Instant::now);
    let start = StartOptions::parse(std::env::args().skip(1));
    // Runs that check or measure say what went wrong in their output;
    // anyone else gets a message box, as there may be no console to read.
    let automated = start.capture().is_some() || start.measure.is_some();
    if !automated {
        let printed = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            printed(info);
            failed(&format!("Worldline stopped unexpectedly:\n\n{info}"));
        }));
    }
    let capturing = start.capture().is_some();
    if let Err(e) = run(start) {
        eprintln!("worldline: {e}");
        if !automated {
            failed(&format!(
                "Worldline couldn't start:\n\n{e}\n\nIt draws with the graphics card, through DirectX 12 or Vulkan. Updating its driver may help, or `worldline.exe --software` draws with the processor instead, slowly."
            ));
        }
        std::process::exit(1);
    }
    if capturing && capture::PASSED.get() != Some(&true) {
        std::process::exit(1);
    }
}

/// Says why the program stopped, in a message box on Windows.
fn failed(message: &str) {
    #[cfg(windows)]
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Worldline")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    #[cfg(not(windows))]
    let _ = message;
}

/// Opens the window and runs the app until it closes.
fn run(start: StartOptions) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Worldline")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0])
            .with_icon(theme::icon()),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    let mut options = options;
    // Measured runs open in the same place, on the main screen, so their
    // windows have the same number of pixels.
    if start.measure.is_some() {
        options.viewport = options.viewport.with_position([40.0, 40.0]);
    }
    if start.no_vsync {
        options.wgpu_options.surface.present_mode =
            eframe::egui_wgpu::wgpu::PresentMode::AutoNoVsync;
    }
    // Off the screen and never active, so a run that captures frames
    // doesn't take the keyboard and mouse from whoever is at the computer.
    if start.offscreen || start.capture().is_some() {
        options.viewport = options
            .viewport
            .with_position([-10_000.0, -10_000.0])
            .with_active(false)
            .with_taskbar(false);
    }
    if start.software {
        use eframe::egui_wgpu::{WgpuSetup, wgpu};
        if let WgpuSetup::CreateNew(setup) = &mut options.wgpu_options.wgpu_setup {
            setup.native_adapter_selector = Some(std::sync::Arc::new(|adapters, _| {
                adapters
                    .iter()
                    .find(|a| a.get_info().device_type == wgpu::DeviceType::Cpu)
                    .cloned()
                    .ok_or_else(|| "no adapter draws with the processor here".to_string())
            }));
        }
    }
    eframe::run_native(
        "Worldline",
        options,
        Box::new(|creation| {
            creation.egui_ctx.set_theme(egui::ThemePreference::Dark);
            theme::apply(&creation.egui_ctx);
            Ok(Box::new(app::WorldlineApp::new(&start)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_focus_and_paused() {
        let args = [
            "--focus",
            "Earth",
            "--zoom",
            "30",
            "--pitch",
            "-8",
            "--yaw",
            "20",
            "--moon-impact",
            "2000",
            "--overlays",
            "tides,waves",
            "--paused",
            "--now",
            "--speed",
            "hour",
            "--advance",
            "7.5",
            "--add",
            "Gaia BH1:2.5@Sagittarius A*",
            "--catalogue",
            "--eclipse",
            "7",
            "--waves",
            "--fluid",
            "Jupiter",
            "--sky",
            "--stand",
            "38.88,-77.03",
            "--look",
            "120,30",
            "--fov",
            "3",
            "--add",
            "jupiter:2@Gaia BH1",
            "--launch",
            "0.071",
            "--scenario",
            "gw150914",
            "--scenarios",
            "--timings",
            "--quality",
            "low",
            "--no-vsync",
            "--manual",
            "--measure",
            "20",
            "--smoke-test",
            "frame.png",
            "--offscreen",
            "--software",
            "--capture",
            "shot.png",
            "--record",
            "frames",
            "--at",
            "2.5",
            "--for",
            "4",
            "--fps",
            "12",
        ]
        .map(String::from)
        .into_iter();
        assert_eq!(
            StartOptions::parse(args),
            StartOptions {
                focus: Some("Earth".into()),
                zoom_radii: Some(30.0),
                pitch_degrees: Some(-8.0),
                yaw_degrees: Some(20.0),
                moon_impact: Some(2000),
                overlays: overlays::Overlays {
                    tides: true,
                    waves: true,
                    ..Default::default()
                },
                paused: true,
                now: true,
                speed: Some(1),
                advance_years: 7.5,
                catalogue: true,
                eclipses: true,
                eclipse: Some(7),
                adds: vec![
                    Addition {
                        kind: "Gaia BH1".into(),
                        au: 2.5,
                        around: Some("Sagittarius A*".into()),
                        launch: None,
                        aim: None,
                    },
                    Addition {
                        kind: "jupiter".into(),
                        au: 2.0,
                        around: Some("Gaia BH1".into()),
                        launch: Some(0.071),
                        aim: None,
                    },
                ],
                waves: true,
                fluid: Some("Jupiter".into()),
                sky: true,
                stand: Some((38.88, -77.03)),
                look: Some((120.0, 30.0)),
                fov: Some(3.0),
                scenario: Some("gw150914".into()),
                scenarios: true,
                manual: true,
                timings: true,
                quality: Some(quality::Quality::Low),
                no_vsync: true,
                measure: Some(20.0),
                smoke_test: Some("frame.png".into()),
                offscreen: true,
                software: true,
                capture: Some("shot.png".into()),
                record: Some("frames".into()),
                at: Some(2.5),
                record_for: Some(4.0),
                fps: Some(12.0),
            }
        );
        assert_eq!(
            StartOptions::parse(std::iter::empty()),
            StartOptions::default()
        );
    }

    #[test]
    fn a_capture_is_a_still_or_a_recording_with_its_defaults() {
        let parse = |args: &[&str]| StartOptions::parse(args.iter().map(|a| a.to_string()));
        assert_eq!(parse(&[]).capture(), None);
        assert_eq!(
            parse(&["--capture", "a.png"]).capture(),
            Some((capture::Kind::Still { at: 2.0 }, "a.png".into()))
        );
        assert_eq!(
            parse(&["--record", "f", "--at", "1"]).capture(),
            Some((
                capture::Kind::Film {
                    at: 1.0,
                    length: 6.0,
                    fps: 15.0
                },
                "f".into()
            ))
        );
        assert_eq!(
            parse(&["--capture", "a.png", "--smoke-test", "b.png"]).capture(),
            Some((capture::Kind::Smoke, "b.png".into()))
        );
    }
}
