//! Worldline: a relativistic universe sandbox.
//!
//! The desktop app. Physics lives in `worldline-core` and real-world data
//! in `worldline-data`; this crate is the window, the 3D view and the
//! controls.

mod app;
mod calendar;
mod camera;
mod simulation;
mod view;

use eframe::egui;

fn main() -> eframe::Result {
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
            Ok(Box::new(app::WorldlineApp::new()))
        }),
    )
}
