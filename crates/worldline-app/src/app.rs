//! The Worldline window: the 3D view plus its control panels.

use std::f64::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, RichText, Sense, vec2};
use worldline_core::DVec3;
use worldline_core::constants::{AU, DAY, JULIAN_YEAR};

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::simulation::Simulation;
use crate::view::{self, ViewOptions, body_color};

/// Simulation speeds the user can pick: simulated time per real second.
const SPEEDS: [(&str, f64); 7] = [
    ("1 hour per second", 3600.0),
    ("1 day per second", DAY),
    ("1 week per second", 7.0 * DAY),
    ("1 month per second", 30.0 * DAY),
    ("1 year per second", JULIAN_YEAR),
    ("10 years per second", 10.0 * JULIAN_YEAR),
    ("100 years per second", 100.0 * JULIAN_YEAR),
];
const DEFAULT_SPEED: usize = 3;

/// Most computer time physics may take per frame. If a high speed needs
/// more, the simulation runs slower than requested and says so.
const PHYSICS_BUDGET: Duration = Duration::from_millis(12);

/// Earth's GM (JPL DE440), for showing masses in Earth masses.
const GM_EARTH: f64 = 3.986_004_355_070_227e14;

const SUN: usize = 0;
const EARTH: usize = 3;

pub struct WorldlineApp {
    simulation: Simulation,
    camera: Camera,
    /// The body the camera follows.
    focus: usize,
    /// The body shown in the inspector.
    selected: usize,
    speed_index: usize,
    options: ViewOptions,
    last_frame: Instant,
}

impl WorldlineApp {
    pub fn new() -> Self {
        Self {
            simulation: Simulation::solar_system(SPEEDS[DEFAULT_SPEED].1),
            camera: initial_camera(),
            focus: SUN,
            selected: EARTH,
            speed_index: DEFAULT_SPEED,
            options: ViewOptions {
                trails: true,
                grid: true,
                labels: true,
            },
            last_frame: Instant::now(),
        }
    }

    fn reset(&mut self) {
        self.simulation = Simulation::solar_system(SPEEDS[self.speed_index].1);
        self.camera = initial_camera();
        self.focus = SUN;
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Worldline").strong().size(16.0));
            ui.separator();
            let label = if self.simulation.paused {
                "Play"
            } else {
                "Pause"
            };
            if ui.button(label).clicked() {
                self.simulation.paused = !self.simulation.paused;
            }
            egui::ComboBox::from_id_salt("speed")
                .selected_text(SPEEDS[self.speed_index].0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in SPEEDS.iter().enumerate() {
                        ui.selectable_value(&mut self.speed_index, i, *name);
                    }
                });
            self.simulation.speed = SPEEDS[self.speed_index].1;
            if ui.button("Reset").clicked() {
                self.reset();
            }
            ui.separator();
            let date = DateTime::from_julian_date(self.simulation.julian_date());
            ui.label(RichText::new(format!("{date} TDB")).monospace().size(15.0));
            if !self.simulation.paused && self.simulation.achieved < 0.99 {
                ui.separator();
                ui.colored_label(
                    Color32::from_rgb(255, 170, 60),
                    format!(
                        "Physics running at {:.0}% of the requested speed",
                        self.simulation.achieved * 100.0
                    ),
                );
            }
        });
    }

    fn inspector(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bodies");
            ui.label(
                RichText::new("Click to select, double-click to follow.")
                    .small()
                    .weak(),
            );
            for (i, body) in self.simulation.system.bodies.iter().enumerate() {
                let text = RichText::new(&body.name).color(body_color(&body.name));
                let response = ui.selectable_label(i == self.selected, text);
                if response.clicked() {
                    self.selected = i;
                }
                if response.double_clicked() {
                    self.selected = i;
                    self.focus = i;
                }
            }
            ui.separator();
            self.selected_body(ui);
            ui.separator();

            ui.heading("Physics");
            egui::Grid::new("physics").num_columns(2).show(ui, |ui| {
                ui.label("Gravity");
                ui.label(self.simulation.gravity().name());
                ui.end_row();
                ui.label("Integrator");
                ui.label(self.simulation.integrator_name());
                ui.end_row();
                ui.label("Data");
                ui.label("NASA JPL Horizons (DE441)");
                ui.end_row();
                ui.label("Start");
                ui.label("2025-01-01 00:00 TDB");
                ui.end_row();
            });
            ui.separator();

            ui.heading("Display");
            ui.checkbox(&mut self.options.trails, "Orbit trails");
            ui.checkbox(&mut self.options.grid, "Distance rings (ecliptic plane)");
            ui.checkbox(&mut self.options.labels, "Labels");
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Distances and positions are to scale. Bodies too small to see \
                     are drawn as dots at least 3 points wide.",
                )
                .small()
                .weak(),
            );
        });
    }

    fn selected_body(&mut self, ui: &mut egui::Ui) {
        let bodies = &self.simulation.system.bodies;
        let body = &bodies[self.selected];
        let sun = &bodies[SUN];
        ui.heading(RichText::new(&body.name).color(body_color(&body.name)));
        egui::Grid::new("selected").num_columns(2).show(ui, |ui| {
            ui.label("Mass");
            ui.label(format!("{:.4e} kg", body.mass()));
            ui.end_row();
            ui.label("");
            ui.label(format!("{:.6} Earth masses", body.gm / GM_EARTH));
            ui.end_row();
            ui.label("Radius");
            ui.label(format!("{:.0} km", body.radius / 1e3));
            ui.end_row();
            if self.selected != SUN {
                ui.label("Distance from Sun");
                ui.label(format!(
                    "{:.4} AU",
                    (body.position - sun.position).length() / AU
                ));
                ui.end_row();
                ui.label("Speed relative to Sun");
                ui.label(format!(
                    "{:.2} km/s",
                    (body.velocity - sun.velocity).length() / 1e3
                ));
                ui.end_row();
            }
        });
        if self.focus == self.selected {
            ui.label(RichText::new("The camera is following this body.").weak());
        } else if ui.button("Follow with camera").clicked() {
            self.focus = self.selected;
        }
    }

    fn scene(&mut self, ui: &mut egui::Ui) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let viewport = response.rect;
        painter.rect_filled(viewport, 0.0, Color32::from_rgb(3, 5, 12));

        if response.dragged() {
            let delta = response.drag_delta();
            self.camera.orbit(delta.x, delta.y);
        }
        let min_distance = (3.0 * self.simulation.system.bodies[self.focus].radius).max(1e3);
        if response.hovered() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll, min_distance);
            }
        }
        self.camera.clamp_distance(min_distance);

        let (drawn, _) = view::draw(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            self.selected,
        );
        if (response.clicked() || response.double_clicked())
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(i) = view::pick(&drawn, pointer)
        {
            self.selected = i;
            if response.double_clicked() {
                self.focus = i;
            }
        }

        painter.text(
            viewport.left_bottom() + vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            "Drag to rotate · scroll to zoom · double-click a body to follow it",
            FontId::proportional(12.0),
            Color32::from_gray(140),
        );
    }
}

impl eframe::App for WorldlineApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let now = Instant::now();
        // Cap the step after a stall (e.g. dragging the window) at 0.1 s.
        let real_dt = now.duration_since(self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        self.simulation.update(real_dt, PHYSICS_BUDGET);
        self.camera.target = self.simulation.system.bodies[self.focus].position;

        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::right("inspector")
            .default_size(290.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.scene(ui));

        if !self.simulation.paused {
            ui.ctx().request_repaint();
        }
    }
}

/// Looking down on the inner solar system at an angle.
fn initial_camera() -> Camera {
    Camera {
        target: DVec3::ZERO,
        yaw: -FRAC_PI_2 - 0.4,
        pitch: 0.6,
        distance: 4.0 * AU,
        fov_y: 0.8,
    }
}
