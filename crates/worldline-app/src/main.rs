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
//! - `--paused` starts with time stopped.
//! - `--advance 7` runs the physics 7 years past the 2025-01-01 snapshot
//!   before showing anything.

mod app;
mod calendar;
mod camera;
mod catalogue;
mod details;
mod gpu;
mod sandbox;
mod simulation;
mod textures;
mod theme;
mod view;
mod waves;

use eframe::egui;

/// Options from the command line.
#[derive(Debug, Default, PartialEq)]
pub struct StartOptions {
    /// Name of the body to fly to at startup.
    pub focus: Option<String>,
    /// Camera distance from the focused body, in its radii.
    pub zoom_radii: Option<f64>,
    /// Start with time stopped.
    pub paused: bool,
    /// Start with the catalogue window open.
    pub catalogue: bool,
    /// Years to simulate past the snapshot before starting.
    pub advance_years: f64,
    /// A body to add before starting: which ("earth", "jupiter", "sun" or
    /// a catalogue entry's name, like "Sagittarius A*") and how far from
    /// the Sun, in AU, on a circular orbit.
    pub add: Option<(String, f64)>,
    /// Open the gravitational-wave window for the focused body's pair.
    pub waves: bool,
}

impl StartOptions {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut options = Self::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--focus" => options.focus = args.next(),
                "--paused" => options.paused = true,
                "--catalogue" => options.catalogue = true,
                "--waves" => options.waves = true,
                "--zoom" => match args.next().and_then(|r| r.parse::<f64>().ok()) {
                    Some(radii) if radii > 1.0 => options.zoom_radii = Some(radii),
                    _ => eprintln!("worldline: --zoom needs a number of radii above 1"),
                },
                "--advance" => match args.next().and_then(|y| y.parse::<f64>().ok()) {
                    Some(years) if years >= 0.0 => options.advance_years = years,
                    _ => eprintln!("worldline: --advance needs a number of years, 0 or more"),
                },
                "--add" => match args.next().and_then(|a| {
                    let (kind, au) = a.rsplit_once(':')?;
                    let au = au.parse::<f64>().ok().filter(|au| *au > 0.0)?;
                    (!kind.is_empty()).then(|| (kind.to_string(), au))
                }) {
                    Some(add) => options.add = Some(add),
                    None => eprintln!(
                        "worldline: --add needs a body and a distance in AU, as jupiter:1.5 or \"Gaia BH1:3\""
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
            "--paused",
            "--advance",
            "7.5",
            "--add",
            "Gaia BH1:2.5",
            "--catalogue",
            "--waves",
        ]
        .map(String::from)
        .into_iter();
        assert_eq!(
            StartOptions::parse(args),
            StartOptions {
                focus: Some("Earth".into()),
                zoom_radii: Some(30.0),
                paused: true,
                advance_years: 7.5,
                catalogue: true,
                add: Some(("Gaia BH1".into(), 2.5)),
                waves: true,
            }
        );
        assert_eq!(
            StartOptions::parse(std::iter::empty()),
            StartOptions::default()
        );
    }
}
