//! Worldline: a relativistic universe sandbox.
//!
//! The desktop app. Physics lives in `worldline-core`, real-world data in
//! `worldline-data` and GPU drawing in `worldline-render`; this crate is the
//! window, the 3D view and the controls.
//!
//! Usage: `worldline [--focus BODY] [--paused]`
//! - `--focus Earth` starts with the camera flown in to Earth.
//! - `--paused` starts with time stopped.

mod app;
mod calendar;
mod camera;
mod gpu;
mod simulation;
mod textures;
mod view;

use eframe::egui;

/// Options from the command line.
#[derive(Debug, Default, PartialEq)]
pub struct StartOptions {
    /// Name of the body to fly to at startup.
    pub focus: Option<String>,
    /// Start with time stopped.
    pub paused: bool,
}

impl StartOptions {
    fn parse(args: impl Iterator<Item = String>) -> Self {
        let mut options = Self::default();
        let mut args = args.peekable();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--focus" => options.focus = args.next(),
                "--paused" => options.paused = true,
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
            Ok(Box::new(app::WorldlineApp::new(&start)))
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_focus_and_paused() {
        let args = ["--focus", "Earth", "--paused"]
            .map(String::from)
            .into_iter();
        assert_eq!(
            StartOptions::parse(args),
            StartOptions {
                focus: Some("Earth".into()),
                paused: true
            }
        );
        assert_eq!(
            StartOptions::parse(std::iter::empty()),
            StartOptions::default()
        );
    }
}
