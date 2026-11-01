//! The Moon-forming impact in a window of its own (step 6.4): Canup's (2004)
//! impact as worldline-core's fluid of rock and iron, stepped on the CPU on a
//! thread of its own, frames kept to replay. It happened 4.5 billion years
//! ago, so it stands apart from the solar system of the main view. See
//! `docs/physics/giant-impacts.md`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, Color32, Mesh, Rect, RichText, Sense, Shape, Stroke, vec2};
use worldline_core::giant_impact::{
    Disk, EARTH_MASS, EARTH_MOON_ANGULAR_MOMENTUM, Impact, LUNAR_MASS, from_impactor, is_iron,
    measure,
};
use worldline_core::sph::Fluid;

use crate::theme;

/// How long the impact runs, s: as long as Canup's Figure 2.
const END: f64 = 27.0 * 3600.0;

/// Simulated seconds between frames, and between measures of the disk.
const FRAME: f64 = 300.0;
const MEASURE: f64 = 1800.0;

/// The picture's side, and the width of the results beside it, points.
const CANVAS: f32 = 480.0;
const SIDE: f32 = 300.0;

/// Earth's radius, m: the window's unit of length.
const EARTH_RADIUS: f64 = 6.371e6;

/// A particle as drawn, in Earth radii.
#[derive(Debug, Clone, Copy)]
struct Dot {
    x: f32,
    y: f32,
    z: f32,
    /// Internal energy, MJ/kg.
    energy: f32,
    material: u8,
}

/// The fluid at a moment.
struct Frame {
    /// Hours since the run began.
    hours: f64,
    dots: Vec<Dot>,
}

impl Frame {
    fn of(fluid: &Fluid) -> Self {
        let dots = fluid
            .particles
            .iter()
            .map(|p| {
                let at = (p.position / EARTH_RADIUS).as_vec3();
                Dot {
                    x: at.x,
                    y: at.y,
                    z: at.z,
                    energy: (p.energy / 1e6) as f32,
                    material: p.material,
                }
            })
            .collect();
        Self {
            hours: fluid.time / 3600.0,
            dots,
        }
    }
}

/// What the thread has done so far.
#[derive(Default)]
struct Progress {
    frames: Vec<Frame>,
    disk: Option<Disk>,
    /// What it's doing, in words.
    status: String,
}

/// How particles are colored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Colors {
    Origin,
    Energy,
}

/// The impact under way, and the window onto it.
pub struct ImpactLab {
    progress: Arc<Mutex<Progress>>,
    stop: Arc<AtomicBool>,
    colors: Colors,
    edge_on: bool,
    /// Half the view's width, Earth radii.
    reach: f32,
    /// The frame shown, if not the latest.
    replay: Option<usize>,
}

impl ImpactLab {
    /// Starts the impact with about `count` particles.
    pub fn start(count: usize) -> Self {
        let progress = Arc::new(Mutex::new(Progress {
            status: "Settling the two planets".to_string(),
            ..Progress::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, halted) = (progress.clone(), stop.clone());
        std::thread::spawn(move || run(count, &shared, &halted));
        Self {
            progress,
            stop,
            colors: Colors::Origin,
            edge_on: false,
            reach: 9.0,
            replay: None,
        }
    }

    /// Draws the window. Returns whether it's still open.
    pub fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        egui::Window::new("The Moon-forming impact")
            .open(&mut open)
            .default_pos([24.0, 64.0])
            .show(ctx, |ui| self.contents(ui));
        ctx.request_repaint_after(Duration::from_millis(100));
        open
    }

    fn contents(&mut self, ui: &mut egui::Ui) {
        let shared = self.progress.clone();
        let progress = shared.lock().expect("the lab's thread lives");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.colors, Colors::Origin, "Origin");
            ui.selectable_value(&mut self.colors, Colors::Energy, "Internal energy");
            ui.separator();
            ui.selectable_value(&mut self.edge_on, false, "From above");
            ui.selectable_value(&mut self.edge_on, true, "Edge on");
            ui.separator();
            ui.add(
                egui::Slider::new(&mut self.reach, 2.0..=60.0)
                    .logarithmic(true)
                    .text("Earth radii"),
            );
        });
        let last = progress.frames.len().saturating_sub(1);
        let shown = self.replay.unwrap_or(last).min(last);
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(CANVAS);
                self.picture(ui, progress.frames.get(shown));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let mut at = shown;
                    if ui
                        .add_enabled(
                            last > 0,
                            egui::Slider::new(&mut at, 0..=last.max(1)).show_value(false),
                        )
                        .changed()
                    {
                        self.replay = Some(at);
                    }
                    if self.replay.is_some() && ui.button("Latest").clicked() {
                        self.replay = None;
                    }
                    ui.label(RichText::new(&progress.status).small().color(theme::MUTED));
                });
            });
            ui.vertical(|ui| {
                ui.set_width(SIDE);
                results(ui, progress.disk.as_ref());
                ui.add_space(8.0);
                if self.colors == Colors::Origin {
                    ui.label(
                        RichText::new(
                            "Blue: Earth's rock. Orange: the impactor's rock. White: Earth's iron. Red: the impactor's iron.",
                        )
                        .small()
                        .color(theme::MUTED),
                    );
                }
                ui.label(
                    RichText::new(
                        "Canup's (2004) impact, by the Tillotson equation of state on this computer's processor. In brackets are her figures at 27 hours, from 60,000 particles and a better equation of state. Internal energy runs from 1 to 100 MJ/kg.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
            });
        });
    }

    /// Draws `frame`, with Earth's size for scale.
    fn picture(&self, ui: &mut egui::Ui, frame: Option<&Frame>) {
        let (response, painter) = ui.allocate_painter(vec2(CANVAS, CANVAS), Sense::hover());
        let rect = response.rect;
        painter.rect_filled(rect, 0.0, Color32::from_rgb(6, 8, 14));
        let Some(frame) = frame else {
            return;
        };
        let mut mesh = Mesh::default();
        let scale = 0.5 * CANVAS / self.reach;
        for dot in &frame.dots {
            let down = if self.edge_on { dot.z } else { dot.y };
            let at = rect.center() + vec2(dot.x, -down) * scale;
            if rect.contains(at) {
                let size = Rect::from_center_size(at, vec2(2.5, 2.5));
                mesh.add_colored_rect(size, color(dot, self.colors));
            }
        }
        painter.add(Shape::mesh(mesh));
        painter.circle_stroke(
            rect.center(),
            scale,
            Stroke::new(1.0, Color32::from_white_alpha(40)),
        );
        painter.text(
            rect.left_top() + vec2(8.0, 6.0),
            egui::Align2::LEFT_TOP,
            format!("{:.1} hours", frame.hours),
            egui::FontId::monospace(14.0),
            Color32::WHITE,
        );
    }
}

impl Drop for ImpactLab {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Runs the impact until it's over or the window closes.
fn run(count: usize, progress: &Mutex<Progress>, stop: &AtomicBool) {
    let start = Impact::CANONICAL.set_up(count);
    let mut fluid = start.fluid;
    let (mut next_frame, mut next_measure) = (0.0, 0.0);
    while !stop.load(Ordering::Relaxed) {
        if fluid.time >= next_frame {
            let frame = Frame::of(&fluid);
            let mut shared = progress.lock().expect("the window lives");
            shared.status = format!(
                "{} particles, {:.1} of 27 hours",
                fluid.particles.len(),
                frame.hours
            );
            shared.frames.push(frame);
            next_frame += FRAME;
        }
        if fluid.time >= next_measure {
            let disk = measure(&fluid.particles);
            progress.lock().expect("the window lives").disk = Some(disk);
            next_measure += MEASURE;
        }
        if fluid.time >= END {
            progress.lock().expect("the window lives").status =
                "Finished: drag to replay".to_string();
            return;
        }
        fluid.step();
    }
}

/// A particle's color.
fn color(dot: &Dot, colors: Colors) -> Color32 {
    match colors {
        Colors::Origin => match (is_iron(dot.material), from_impactor(dot.material)) {
            (false, false) => Color32::from_rgb(86, 148, 255),
            (false, true) => Color32::from_rgb(255, 166, 72),
            (true, false) => Color32::from_rgb(226, 230, 240),
            (true, true) => Color32::from_rgb(255, 72, 60),
        },
        // 1 to 100 MJ/kg, cool to hot.
        Colors::Energy => {
            let t = (dot.energy.max(1e-3).log10() / 2.0).clamp(0.0, 1.0);
            let (low, middle, high) = (
                [60.0, 20.0, 110.0],
                [240.0, 110.0, 40.0],
                [255.0, 245.0, 200.0],
            );
            let (from, to, s) = if t < 0.5 {
                (low, middle, 2.0 * t)
            } else {
                (middle, high, 2.0 * t - 1.0)
            };
            let mix = |k: usize| (from[k] + (to[k] - from[k]) * s) as u8;
            Color32::from_rgb(mix(0), mix(1), mix(2))
        }
    }
}

/// The disk as measured so far, beside Canup's figures for her §4.1 run.
fn results(ui: &mut egui::Ui, disk: Option<&Disk>) {
    let Some(disk) = disk else {
        return;
    };
    let moons = |kg: f64| format!("{:.2} lunar masses", kg / LUNAR_MASS);
    let rows = [
        ("In orbit", moons(disk.disk), "1.62"),
        ("Past the Roche limit", moons(disk.beyond_roche), "0.92"),
        ("Escaping", moons(disk.escaping), "0.41"),
        (
            "Earth",
            format!("{:.3} Earth masses", disk.planet / EARTH_MASS),
            "0.994",
        ),
        (
            "Its day",
            format!("{:.1} hours", disk.day / 3600.0),
            "4.6 hours",
        ),
        (
            "Disk's spin",
            format!(
                "{:.2} of the Earth–Moon system's",
                disk.disk_angular_momentum / EARTH_MOON_ANGULAR_MOMENTUM
            ),
            "0.31",
        ),
        ("The moon it would make", moons(disk.moon), "1.38"),
    ];
    egui::Grid::new("impact results")
        .num_columns(3)
        .show(ui, |ui| {
            for (name, value, hers) in rows {
                ui.label(RichText::new(name).color(theme::MUTED));
                ui.label(value);
                ui.label(RichText::new(format!("({hers})")).color(theme::MUTED));
                ui.end_row();
            }
        });
}
