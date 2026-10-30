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
//! - `--fluid Jupiter` models Jupiter as a self-gravitating fluid of SPH
//!   particles on the GPU, as many as it steps smoothly.

mod app;
mod calendar;
mod camera;
mod catalogue;
mod clocks;
mod details;
mod eclipse_finder;
mod fluid;
mod gpu;
mod ground;
mod overlays;
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

fn main() -> eframe::Result {
    let start = StartOptions::parse(std::env::args().skip(1));
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Worldline")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 500.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
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
        ]
        .map(String::from)
        .into_iter();
        assert_eq!(
            StartOptions::parse(args),
            StartOptions {
                focus: Some("Earth".into()),
                zoom_radii: Some(30.0),
                pitch_degrees: Some(-8.0),
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
                    },
                    Addition {
                        kind: "jupiter".into(),
                        au: 2.0,
                        around: Some("Gaia BH1".into()),
                        launch: Some(0.071),
                    },
                ],
                waves: true,
                fluid: Some("Jupiter".into()),
                sky: true,
                stand: Some((38.88, -77.03)),
                look: Some((120.0, 30.0)),
                fov: Some(3.0),
            }
        );
        assert_eq!(
            StartOptions::parse(std::iter::empty()),
            StartOptions::default()
        );
    }
}
