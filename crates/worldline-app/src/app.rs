//! The Worldline window: the 3D view plus its control panels.

use std::f64::consts::{FRAC_PI_2, PI};
use std::time::{Duration, Instant};

use eframe::egui::collapsing_header::CollapsingState;
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, TextureId, pos2, vec2};
use eframe::egui_wgpu::RenderState;
use glam::Mat3;
use worldline_core::constants::{AU, C, DAY, GM_SUN, JULIAN_YEAR, SOLAR_LUMINOSITY};
use worldline_core::gravity::kerr::innermost_stable_orbit;
use worldline_core::magnetosphere::standoff;
use worldline_core::merger::ringdown;
use worldline_core::sunlight::{irradiance, light_time};
use worldline_core::{Body, DVec3};
use worldline_render::{Atmosphere, Rings, View};

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::catalogue::{self, Entry, Group, length, light_years, measured_suns, significant};
use crate::details;
use crate::gpu::{Globe, GpuGlobes};
use crate::sandbox::{self, Launch};
use crate::simulation::Simulation;
use crate::simulation::{Computed, Removal};
use crate::theme;
use crate::view::{self, OnScreen, ViewOptions};
use crate::waves;
use worldline_core::compact::{gravitational_redshift, horizon_spin, is_black_hole};
use worldline_core::regime::Validity;
use worldline_data::{Distance, ObjectKind, RadiusBasis, SmallBodyKind};

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
pub const GM_EARTH: f64 = 3.986_004_355_070_227e14;

/// A label in an inspector grid: what the value on its right is.
fn key(text: &str) -> RichText {
    RichText::new(text).color(theme::MUTED)
}

/// A duration (s) for display: seconds; minutes and seconds; or hours and
/// minutes.
fn duration(seconds: f64) -> String {
    if seconds < 120.0 {
        format!("{seconds:.1} s")
    } else if seconds < 7200.0 {
        format!(
            "{:.0} min {:.1} s",
            (seconds / 60.0).floor(),
            seconds % 60.0
        )
    } else {
        let minutes = (seconds / 60.0).round();
        format!(
            "{:.0} h {:.0} min",
            (minutes / 60.0).floor(),
            minutes % 60.0
        )
    }
}

/// Draws a recording's h₊ against time, one column of the plot per
/// stretch of samples (its highest and lowest), so fast waves stay visible.
fn waveform_plot(ui: &mut egui::Ui, recording: &waves::Recording) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
    let background = ui.visuals().extreme_bg_color;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, background);
    let n = recording.times.len();
    let span = recording.times[n - 1].max(f64::MIN_POSITIVE);
    let peak = recording
        .plus
        .iter()
        .fold(0.0f64, |m, h| m.max(h.abs()))
        .max(f64::MIN_POSITIVE);
    let middle = rect.center().y;
    let half = 0.45 * rect.height();
    painter.hline(
        rect.x_range(),
        middle,
        (1.0, theme::MUTED.gamma_multiply(0.4)),
    );
    if let Some(hole) = &recording.final_hole {
        let x = rect.left() + (hole.start / span) as f32 * rect.width();
        painter.vline(x, rect.y_range(), (1.0, theme::MUTED.gamma_multiply(0.6)));
        painter.text(
            pos2(x - 4.0, rect.top() + 4.0),
            Align2::RIGHT_TOP,
            "ringdown starts here",
            FontId::proportional(11.0),
            theme::MUTED,
        );
    }
    let columns = rect.width().max(1.0) as usize;
    let mut k = 0;
    for column in 0..columns {
        let until = span * (column + 1) as f64 / columns as f64;
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        while k < n && recording.times[k] <= until {
            low = low.min(recording.plus[k]);
            high = high.max(recording.plus[k]);
            k += 1;
        }
        if low > high {
            continue;
        }
        let x = rect.left() + column as f32 + 0.5;
        let y = |h: f64| middle - (h / peak) as f32 * half;
        painter.line_segment(
            [pos2(x, y(high)), pos2(x, y(low).max(y(high) + 1.0))],
            (1.0, theme::ACCENT),
        );
    }
    let font = FontId::proportional(11.0);
    painter.text(
        rect.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        format!("h+ peaks at {peak:.2e}"),
        font.clone(),
        theme::MUTED,
    );
    painter.text(
        rect.left_bottom() + vec2(6.0, -4.0),
        Align2::LEFT_BOTTOM,
        "now",
        font.clone(),
        theme::MUTED,
    );
    painter.text(
        rect.right_bottom() + vec2(-6.0, -4.0),
        Align2::RIGHT_BOTTOM,
        format!("{} later", long_time(span)),
        font,
        theme::MUTED,
    );
}

/// A short time, from nanoseconds to seconds: "76 µs".
fn short_time(seconds: f64) -> String {
    let (value, unit) = if seconds < 1e-6 {
        (seconds * 1e9, "ns")
    } else if seconds < 1e-3 {
        (seconds * 1e6, "µs")
    } else if seconds < 1.0 {
        (seconds * 1e3, "ms")
    } else {
        (seconds, "s")
    };
    format!("{} {unit}", significant(value))
}

/// A long time, from seconds to billions of years: "301 million years".
fn long_time(seconds: f64) -> String {
    let years = seconds / JULIAN_YEAR;
    if seconds < 2.0 * DAY {
        duration(seconds)
    } else if years < 1.0 {
        format!("{} days", significant(seconds / DAY))
    } else {
        format!("{} years", significant(years))
    }
}

/// A frequency in words: "270.7 Hz", or for slow ones "one cycle every
/// 21.4 s".
fn frequency(hz: f64) -> String {
    if hz >= 1.0 {
        format!("{} Hz", significant(hz))
    } else {
        format!("one cycle every {}", long_time(1.0 / hz))
    }
}

/// A duration from nanoseconds to years.
fn any_time(seconds: f64) -> String {
    if seconds < 1.0 {
        short_time(seconds)
    } else {
        long_time(seconds)
    }
}

const SUN: usize = 0;
const EARTH: usize = 3;

/// Where the camera settles when you focus on a body, in body radii: close
/// enough that the globe fills most of the view.
const FOCUS_DISTANCE_RADII: f64 = 4.0;
/// How long the camera takes to fly to a newly focused body.
const FLIGHT_SECONDS: f64 = 1.2;
/// Closest the camera may get to a body's center, in body radii.
const MIN_DISTANCE_RADII: f64 = 1.1;
/// How far away the camera settles on a moon whose size hasn't been
/// measured (it is drawn as a dot).
const UNKNOWN_SIZE_DISTANCE: f64 = 1_000e3;

/// A smooth camera move to a newly focused body.
struct Flight {
    from_target: DVec3,
    from_distance: f64,
    to_distance: f64,
    start: Instant,
    /// How long it lasts, in s: longer when it crosses far more space than
    /// it starts or ends seeing.
    seconds: f64,
}

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
    /// The camera's flight to a newly focused body, while it lasts.
    flight: Option<Flight>,
    /// The GPU renderer for globes, created on the first frame.
    gpu: Option<GpuGlobes>,
    /// What's typed in the body list's search box.
    search: String,
    /// The catalogue entry a press in the view adds, if adding; otherwise
    /// a drag looks around.
    tool: Option<&'static Entry>,
    /// Whether the catalogue window is open.
    catalogue_open: bool,
    /// Whether added bodies start at rest instead of circling.
    start_at_rest: bool,
    /// A body being placed, while its launch is dragged.
    launch: Option<Launch>,
    /// A message in the top bar, and when it appeared.
    status: Option<(String, Instant)>,
    /// Open the selected body's branch of the list and scroll to it, once
    /// it has been picked somewhere other than the list.
    reveal: bool,
    /// The gravitational-wave window's recording, while it is open.
    waves: Option<WaveView>,
    /// The sound output, once something has played.
    audio: Option<(rodio::OutputStream, rodio::Sink)>,
}

/// A pair's recorded gravitational waves, shown in their window.
struct WaveView {
    /// "A and B".
    names: String,
    recording: waves::Recording,
    sound: waves::Sound,
}

impl WorldlineApp {
    pub fn new(start: &crate::StartOptions) -> Self {
        let mut app = Self {
            simulation: Simulation::solar_system(SPEEDS[DEFAULT_SPEED].1),
            camera: initial_camera(),
            focus: SUN,
            selected: EARTH,
            speed_index: DEFAULT_SPEED,
            options: ViewOptions {
                trails: true,
                grid: true,
                labels: true,
                spin_axes: true,
                globes: true,
                belts: true,
                dust: true,
                solar_wind: true,
                heliosphere: true,
                magnetospheres: true,
            },
            last_frame: Instant::now(),
            flight: None,
            gpu: None,
            search: String::new(),
            reveal: false,
            tool: None,
            catalogue_open: false,
            start_at_rest: false,
            launch: None,
            status: None,
            waves: None,
            audio: None,
        };
        app.simulation.paused = start.paused;
        app.catalogue_open = start.catalogue;
        if let Some((kind, au)) = &start.add {
            let around = match &start.around {
                Some(name) => app.simulation.index_of(name).unwrap_or_else(|| {
                    eprintln!("worldline: no body named `{name}` to circle; circling the Sun");
                    SUN
                }),
                None => SUN,
            };
            app.add_on_circle(kind, *au, around);
        }
        let names = app.attended_names();
        app.simulation.advance_by(start.advance_years * JULIAN_YEAR);
        app.after_events(names);
        if let Some(name) = &start.focus {
            match app.simulation.bodies.iter().position(|b| &b.name == name) {
                Some(i) => app.fly_to(i, start.zoom_radii.unwrap_or(FOCUS_DISTANCE_RADII)),
                None => eprintln!("worldline: no body named `{name}` to focus on"),
            }
        }
        if start.waves {
            if app.simulation.gravitational_waves(app.selected).is_some() {
                app.open_waves();
            } else {
                eprintln!("worldline: --waves needs --focus on one body of a pair that will merge");
            }
        }
        app
    }

    fn reset(&mut self) {
        self.simulation = Simulation::solar_system(SPEEDS[self.speed_index].1);
        self.camera = initial_camera();
        self.focus = SUN;
        self.flight = None;
    }

    /// Makes the camera follow a body, flying in close to it.
    fn focus_on(&mut self, index: usize) {
        self.fly_to(index, FOCUS_DISTANCE_RADII);
    }

    /// Makes the camera follow a body, settling `radii` of its radii away.
    /// Sagittarius A* is framed with the stars around it: its horizon is
    /// tiny next to their orbits.
    fn fly_to(&mut self, index: usize, radii: f64) {
        let radius = self.simulation.bodies[index].radius;
        let to_distance = if let Some(reach) = self.simulation.cluster_reach(index) {
            reach
        } else if radius > 0.0 {
            radii * radius
        } else {
            UNKNOWN_SIZE_DISTANCE
        };
        let across = (self.simulation.bodies[index].position - self.camera.target).length();
        let far = across > 10.0 * self.camera.distance.max(to_distance);
        self.flight = Some(Flight {
            from_target: self.camera.target,
            from_distance: self.camera.distance,
            to_distance,
            start: Instant::now(),
            seconds: if far {
                2.5 * FLIGHT_SECONDS
            } else {
                FLIGHT_SECONDS
            },
        });
        // Detail follows focus: the small moons around this body are
        // now computed in full.
        self.simulation.focus_detail_on(index);
        self.focus = index;
        self.selected = index;
        self.reveal = true;
    }

    /// Points the camera at the focused body, partway along a flight if one
    /// is under way.
    fn aim_camera(&mut self) {
        let target = self.simulation.bodies[self.focus].position;
        let Some(flight) = &self.flight else {
            self.camera.target = target;
            return;
        };
        let t = (flight.start.elapsed().as_secs_f64() / flight.seconds).min(1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        self.camera.target = flight.from_target.lerp(target, eased);
        // Zoom evenly in scale, so going from 4 AU to 25,000 km looks smooth.
        // Across far more space than either end shows (the solar system to
        // the galactic center), climb out to see both, then come back in.
        let (from, to) = (flight.from_distance.ln(), flight.to_distance.ln());
        let across = (target - flight.from_target).length() * 1.5;
        let lift = (across.ln() - from.max(to)).max(0.0);
        self.camera.distance =
            (from + (to - from) * eased + 4.0 * eased * (1.0 - eased) * lift).exp();
        if t >= 1.0 {
            self.flight = None;
        }
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
            self.sandbox_controls(ui);
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

    /// The side panel: the bodies in a scrolling tree at the top, and below
    /// it, always in view, the selected body and the settings.
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Bodies");
            ui.label(
                RichText::new(self.simulation.bodies.len().to_string())
                    .small()
                    .color(theme::MUTED),
            );
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search by name")
                .desired_width(f32::INFINITY),
        );
        ui.label(
            RichText::new("Click to select · double-click to follow")
                .small()
                .color(theme::MUTED),
        );
        let list_height = (ui.available_height() * 0.42).clamp(140.0, 440.0);
        egui::ScrollArea::vertical()
            .id_salt("bodies")
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| self.body_list(ui));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("details")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.selected_body(ui);
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Physics")
                    .id_salt("physics section")
                    .show(ui, |ui| self.physics(ui));
                egui::CollapsingHeader::new("Display")
                    .id_salt("display section")
                    .show(ui, |ui| self.display(ui));
            });
    }

    /// The bodies as a tree: each planet folds open to its moons (the small
    /// ones folded once more), then the small bodies by kind. While a search
    /// is typed, a flat list of the matches instead.
    fn body_list(&mut self, ui: &mut egui::Ui) {
        let simulation = &self.simulation;
        let (selected, reveal) = (self.selected, self.reveal);
        let (mut clicked, mut fly_to, mut revealed) = (None, None, false);
        let mut entry = |ui: &mut egui::Ui, i: usize| {
            let name = &simulation.bodies[i].name;
            let response = ui.selectable_label(
                i == selected,
                RichText::new(name).color(view::color(simulation, i)),
            );
            if response.clicked() {
                clicked = Some(i);
            }
            if response.double_clicked() {
                fly_to = Some(i);
            }
            if reveal && i == selected {
                response.scroll_to_me(Some(egui::Align::Center));
                revealed = true;
            }
        };
        let query = self.search.trim().to_lowercase();
        if !query.is_empty() {
            let matches: Vec<usize> = (0..simulation.bodies.len())
                .filter(|&i| simulation.bodies[i].name.to_lowercase().contains(&query))
                .collect();
            if matches.is_empty() {
                ui.label(RichText::new("No body by that name").color(theme::MUTED));
            }
            for i in matches.into_iter().take(200) {
                entry(ui, i);
            }
        } else {
            // The planet whose branch holds the selection.
            let holder = simulation.parent(selected).unwrap_or(selected);
            for i in 0..simulation.bodies.len() {
                if simulation.parent(i).is_some() || simulation.small_body(i).is_some() {
                    continue;
                }
                let (small, major): (Vec<usize>, Vec<usize>) = (0..simulation.bodies.len())
                    .filter(|&m| simulation.parent(m) == Some(i))
                    .partition(|&m| simulation.is_small_moon(m));
                if major.is_empty() && small.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.spacing().indent);
                        entry(ui, i);
                    });
                    continue;
                }
                let id = ui.make_persistent_id(("planet", i));
                let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, false);
                if reveal && holder == i && selected != i {
                    state.set_open(true);
                }
                let moons = major.len() + small.len();
                let noun = if simulation.is_galactic(i) {
                    "star"
                } else {
                    "moon"
                };
                state
                    .show_header(ui, |ui| {
                        entry(ui, i);
                        ui.label(
                            RichText::new(if moons == 1 {
                                format!("1 {noun}")
                            } else {
                                format!("{moons} {noun}s")
                            })
                            .small()
                            .color(theme::MUTED),
                        );
                    })
                    .body(|ui| {
                        for m in major {
                            entry(ui, m);
                        }
                        if !small.is_empty() {
                            let mut fold = egui::CollapsingHeader::new(
                                RichText::new(format!("{} small moons", small.len()))
                                    .color(theme::MUTED),
                            )
                            .id_salt(("small moons", i));
                            if reveal && small.contains(&selected) {
                                fold = fold.open(Some(true));
                            }
                            fold.show(ui, |ui| {
                                for m in small {
                                    entry(ui, m);
                                }
                            });
                        }
                    });
            }
            ui.add_space(4.0);
            // The dwarf planets, asteroids and comets, each kind in a fold.
            for (kind, label) in [
                (SmallBodyKind::DwarfPlanet, "Dwarf planets"),
                (SmallBodyKind::TransNeptunian, "Beyond Neptune"),
                (SmallBodyKind::Asteroid, "Asteroids"),
                (SmallBodyKind::Comet, "Comets"),
                (SmallBodyKind::Interstellar, "Interstellar visitors"),
            ] {
                let members: Vec<usize> = (0..simulation.bodies.len())
                    .filter(|&i| simulation.small_body(i).map(|s| s.0) == Some(kind))
                    .collect();
                let mut fold =
                    egui::CollapsingHeader::new(format!("{label}  ·  {}", members.len()))
                        .id_salt(label);
                if reveal && members.contains(&selected) {
                    fold = fold.open(Some(true));
                }
                fold.show(ui, |ui| {
                    for i in members {
                        entry(ui, i);
                    }
                });
            }
        }
        // Keep trying until the selected entry has been laid out (its branch
        // may still be opening).
        if revealed || !query.is_empty() {
            self.reveal = false;
        }
        if let Some(i) = clicked {
            self.selected = i;
        }
        if let Some(i) = fly_to {
            self.focus_on(i);
        }
    }

    fn physics(&self, ui: &mut egui::Ui) {
        egui::Grid::new("physics").num_columns(2).show(ui, |ui| {
            for (what, value) in [
                ("Gravity", self.simulation.gravity().name()),
                ("Moons", self.simulation.moon_gravity_name()),
                ("Integrator", self.simulation.integrator_name()),
                (
                    "Data",
                    "NASA JPL Horizons (DE441 and satellite ephemerides)",
                ),
                ("Start", "2025-01-01 00:00 TDB"),
                ("Rotation", "IAU models (NASA NAIF)"),
                (
                    "Collisions",
                    "bodies that touch merge: momentum kept, volumes add",
                ),
                ("Surfaces", "Solar System Scope, CC BY 4.0"),
            ] {
                ui.label(key(what));
                ui.label(value);
                ui.end_row();
            }
        });
    }

    fn display(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.options.trails, "Orbit trails")
            .on_hover_text("Moons show their current orbit instead");
        ui.checkbox(&mut self.options.grid, "Distance rings")
            .on_hover_text("In the ecliptic plane, labeled in AU and light-time");
        ui.checkbox(&mut self.options.labels, "Names")
            .on_hover_text(
                "The body you follow, the one selected, the one under the pointer, \
             and the major moons of the planet in focus",
            );
        ui.checkbox(&mut self.options.spin_axes, "Spin axes")
            .on_hover_text("A dot marks the end the spin points toward");
        ui.checkbox(&mut self.options.globes, "Textured globes up close");
        let total: usize = self
            .simulation
            .belts
            .iter()
            .map(|b| b.positions.len())
            .sum();
        ui.checkbox(&mut self.options.belts, "Asteroid and Kuiper belts")
            .on_hover_text(format!(
                "{total} real orbits from JPL (measured), on fixed ellipses around the \
                 Sun (model: about 1/10,000 of their distance off after a year). Shown \
                 when zoomed out past 0.05 AU."
            ));
        ui.checkbox(&mut self.options.dust, "Zodiacal dust")
            .on_hover_text(
                "The COBE DIRBE model of Kelsall et al. 1998, drawn as the sunlight it \
             scatters on a log scale (visual); held at its 0.3 AU brightness farther \
             in, where no probe has measured how it brightens",
            );
        ui.checkbox(&mut self.options.solar_wind, "Solar wind spiral")
            .on_hover_text("Parker's spiral field, with the Sun in focus");
        ui.checkbox(&mut self.options.heliosphere, "Heliosphere")
            .on_hover_text("Its boundaries, seen from outside them");
        ui.checkbox(&mut self.options.magnetospheres, "Magnetospheres")
            .on_hover_text(
                "Around the planet in focus: its magnetopause, from pressure balance with \
             2025's average solar wind; Earth's radiation belts",
            );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Distances, sizes and positions are to scale. Bodies too small to \
                 see are drawn as dots at least 3 points wide.",
            )
            .small()
            .color(theme::MUTED),
        );
    }

    fn selected_body(&mut self, ui: &mut egui::Ui) {
        let bodies = &self.simulation.bodies;
        let body = &bodies[self.selected];
        let sun = &bodies[SUN];
        let parent = self.simulation.parent(self.selected);
        ui.heading(RichText::new(&body.name).color(view::color(&self.simulation, self.selected)));
        self.model_indicator(ui);
        egui::Grid::new("selected").num_columns(2).show(ui, |ui| {
            ui.label(key("Mass"));
            if body.gm > 0.0 {
                ui.label(format!("{:.4e} kg", body.mass()));
                ui.end_row();
                ui.label("");
                if body.gm >= 0.01 * GM_SUN {
                    ui.label(measured_suns(body.gm / GM_SUN, None));
                } else {
                    ui.label(format!("{:.6} Earth masses", body.gm / GM_EARTH));
                }
            } else {
                ui.label(key("not measured"));
            }
            ui.end_row();
            if is_black_hole(body) {
                ui.label(key("Event horizon"));
                ui.label(length(body.radius));
                ui.end_row();
                self.black_hole_rows(ui, body);
            } else {
                ui.label(key("Radius"));
                ui.label(if body.radius > 0.0 {
                    format!("{:.1} km", body.radius / 1e3)
                } else {
                    "not measured".to_string()
                });
            }
            ui.end_row();
            self.catalog_rows(ui);
            if let Some(parent) = parent {
                let planet = &bodies[parent];
                let d = (body.position - planet.position).length();
                ui.label(key(&format!("Distance from {}", planet.name)));
                ui.label(if d > 1e10 {
                    format!("{:.1} AU", d / AU)
                } else {
                    format!("{:.0} km", d / 1e3)
                });
                ui.end_row();
                ui.label(key(&format!("Speed relative to {}", planet.name)));
                ui.label(format!(
                    "{:.3} km/s",
                    (body.velocity - planet.velocity).length() / 1e3
                ));
                ui.end_row();
            } else if self.simulation.is_galactic(self.selected) {
                // Too far for AU: in light-years and parsecs.
                ui.label(key(&format!("Distance from {}", sun.name)));
                ui.label(light_years((body.position - sun.position).length()));
                ui.end_row();
            } else if self.selected != SUN {
                ui.label(key(&format!("Distance from {}", sun.name)));
                ui.label(format!(
                    "{:.4} AU",
                    (body.position - sun.position).length() / AU
                ));
                ui.end_row();
                ui.label(key(&format!("Speed relative to {}", sun.name)));
                ui.label(format!(
                    "{:.2} km/s",
                    (body.velocity - sun.velocity).length() / 1e3
                ));
                ui.end_row();
            }
            if !self.simulation.is_galactic(self.selected) {
                self.sun_reach(ui);
                self.magnetism(ui);
            }
            if let Some(model) = self.simulation.rotation(self.selected) {
                let jd = self.simulation.julian_date();
                let hours = model.sidereal_period() / 3600.0;
                ui.label(key("Day (sidereal)"));
                ui.label(if hours < 72.0 {
                    format!("{hours:.3} h")
                } else {
                    format!("{:.2} days", hours / 24.0)
                });
                ui.end_row();
                ui.label(key("Spin"));
                ui.label(if model.spin_rate() > 0.0 {
                    "prograde"
                } else {
                    "retrograde"
                });
                ui.end_row();
                // Tilt of the spin axis to the orbit: around its planet for
                // a moon, around the Sun for a planet, to the ecliptic for
                // the Sun.
                let orbit_normal = match (self.selected, parent) {
                    (SUN, _) => DVec3::Z,
                    (_, Some(planet)) => orbit_normal(body, &bodies[planet]),
                    _ => orbit_normal(body, sun),
                };
                ui.label(if self.selected == SUN {
                    "Tilt to ecliptic"
                } else {
                    "Axial tilt"
                });
                ui.label(format!(
                    "{:.2}°",
                    model.spin_axis(jd).angle_between(orbit_normal).to_degrees()
                ));
                ui.end_row();
            }
        });
        self.catalog_notes(ui);
        ui.add_space(4.0);
        let kind = if self.simulation.is_galactic(self.selected)
            && !self.simulation.is_added(self.selected)
        {
            details::BodyKind::Galactic {
                star: self.simulation.parent(self.selected).is_some(),
            }
        } else if self.simulation.merger(self.selected).is_some() {
            details::BodyKind::MergerRemnant
        } else if self.simulation.is_added(self.selected) {
            details::BodyKind::Added(self.simulation.entry(self.selected).and_then(Entry::kind))
        } else if let Some((kind, outgassing)) = self.simulation.small_body(self.selected) {
            details::BodyKind::SmallBody {
                comet: matches!(kind, SmallBodyKind::Comet | SmallBodyKind::Interstellar),
                outgassing,
                massive: body.gm > 0.0,
            }
        } else if self.simulation.is_detailed(self.selected) {
            details::BodyKind::SmallMoonInDetail
        } else if self.simulation.is_small_moon(self.selected) {
            details::BodyKind::SmallMoonOnMeanOrbit
        } else if parent.is_some() {
            details::BodyKind::Moon
        } else {
            details::BodyKind::Other
        };
        for detail in details::details(&body.name, kind) {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(detail.kind.tag())
                        .small()
                        .color(detail.kind.color()),
                );
                ui.label(RichText::new(detail.what).small());
            });
        }
        if self.simulation.gravitational_waves(self.selected).is_some()
            && ui
                .button("See and hear its gravitational waves")
                .on_hover_text(
                    "The waves an observer on Earth would record from this pair, from a copy of it run ahead",
                )
                .clicked()
        {
            self.open_waves();
        }
        ui.horizontal(|ui| {
            if self.focus == self.selected {
                ui.label(RichText::new("The camera is following this body.").weak());
            } else if ui.button("Fly to it").clicked() {
                self.focus_on(self.selected);
            }
            let name = self.simulation.bodies[self.selected].name.clone();
            match self.simulation.removal(self.selected) {
                Removal::Allowed { moons } => {
                    let label = match moons {
                        0 => "Remove".to_string(),
                        1 => "Remove with its moon".to_string(),
                        n => format!("Remove with its {n} moons"),
                    };
                    if ui
                        .button(label)
                        .on_hover_text(format!("Take {name} out of the simulation (Delete key)"))
                        .clicked()
                    {
                        self.remove_selected();
                    }
                }
                Removal::Sun => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text("The Sun stays: everything is measured from it");
                }
                Removal::Moon => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text("Moons can't be removed on their own yet");
                }
                Removal::GalacticCenter => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text(
                            "Sagittarius A* stays: its region is measured from it",
                        );
                }
            }
        });
    }

    /// Inspector rows for a black hole: its spin and the tone it rings
    /// with, and for one that formed in a merger, what the merger
    /// radiated, its recoil and, for a real merger's pair, the hole that
    /// was measured. The horizon row is above, and ends its row.
    fn black_hole_rows(&self, ui: &mut egui::Ui, body: &Body) {
        let merger = self.simulation.merger(self.selected);
        // A black hole's spin is in its horizon's size.
        let spin = merger.map_or_else(|| horizon_spin(body.gm, body.radius), |m| m.spin);
        if let Some(m) = merger {
            ui.label(key("Spin"));
            ui.label(format!("{spin:.3} (numerical-relativity fit)"));
            ui.end_row();
            let total = body.gm + m.radiated_gm;
            ui.label(key("Its merger radiated"));
            ui.label(format!(
                "{} Suns ({:.1}%) as gravitational waves",
                significant(m.radiated_gm / GM_SUN),
                100.0 * m.radiated_gm / total
            ));
            ui.end_row();
            ui.label(key("Its kick"));
            ui.label(format!("{} km/s", significant(m.kick.length() / 1e3)));
            ui.end_row();
            if let Some(o) = self
                .simulation
                .entry(self.selected)
                .and_then(Entry::measured_remnant)
            {
                ui.label(key("The real one, measured"));
                ui.label(format!(
                    "{}, spin {}",
                    measured_suns(o.mass.value, o.mass.plus_minus),
                    o.spin
                        .map_or("not measured".to_string(), |s| s.0.to_string())
                ));
                ui.end_row();
            }
        }
        let unit = body.gm / (C * C);
        ui.label(key("Innermost stable orbit"));
        ui.label(if spin < 1e-6 {
            format!("{}: 6 GM/c²", length(6.0 * unit))
        } else {
            format!(
                "{} with its spin, {} against",
                length(innermost_stable_orbit(spin, true) * unit),
                length(innermost_stable_orbit(spin, false) * unit)
            )
        })
        .on_hover_text(
            "Closer in, no circular orbit lasts: a body circling there plunges into the hole (Bardeen, Press & Teukolsky 1972)",
        );
        ui.end_row();
        let tone = ringdown(body.gm, spin);
        ui.label(key("Rings down at"));
        ui.label(format!(
            "{}, fading in {}",
            frequency(tone.frequency),
            any_time(tone.damping)
        ))
        .on_hover_text(
            "The tone a black hole rings with when disturbed, as after a merger (Berti, Cardoso & Will 2006)",
        );
    }

    /// Inspector rows for a real object added from the catalog: its
    /// published mass, size and spin.
    fn catalog_rows(&self, ui: &mut egui::Ui) {
        if self.simulation.merger(self.selected).is_some() {
            return;
        }
        let Some(o) = self.simulation.entry(self.selected).and_then(Entry::object) else {
            return;
        };
        ui.label(key("Published mass"));
        ui.label(measured_suns(o.mass.value, o.mass.plus_minus));
        ui.end_row();
        // A black hole's horizon is shown above, from its mass now.
        let basis = match o.radius_basis {
            RadiusBasis::Horizon => None,
            RadiusBasis::Measured => Some("measured"),
            RadiusBasis::Estimated => Some("estimated"),
            RadiusBasis::Assumed => Some("not measured: assumed"),
        };
        if let Some(basis) = basis {
            ui.label(key("Published radius"));
            ui.label(format!("{} ({basis})", length(o.radius.value)));
            ui.end_row();
        }
        if o.kind == ObjectKind::BlackHole {
            ui.label(key("Spin"));
            ui.label(match o.spin {
                Some((a, true)) => format!("{a}"),
                Some((a, false)) => format!("at least {a}"),
                None => "not measured: taken as none".to_string(),
            });
        } else {
            // Light climbing out of its gravity loses energy.
            ui.label(key("Surface light"));
            ui.label(format!(
                "redshifted {} km/s by gravity",
                significant(gravitational_redshift(o.gm(), o.radius.value) / 1e3)
            ));
        }
        ui.end_row();
    }

    /// Below the rows, for a catalog object: where the real one is, and
    /// where its values were published.
    fn catalog_notes(&self, ui: &mut egui::Ui) {
        let entry = self.simulation.entry(self.selected);
        let object = if self.simulation.merger(self.selected).is_some() {
            entry.and_then(Entry::measured_remnant)
        } else {
            entry.and_then(Entry::object)
        };
        let Some(o) = object else {
            return;
        };
        let place = match o.distance {
            Some(Distance::Meters(d)) => format!("{}, {} away.", o.location, light_years(d)),
            Some(Distance::Redshift(z)) => format!("{}, at redshift {z}.", o.location),
            None => format!("{}.", o.location),
        };
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(format!("The real one: {place}")).small()).wrap());
        ui.add(
            egui::Label::new(
                RichText::new(format!("Source: {}", o.source))
                    .small()
                    .color(theme::MUTED),
            )
            .wrap(),
        );
    }

    /// Which model computes the selected body, how strong gravity is
    /// where it is, and whether the model covers that: green within its
    /// range, amber approximate, red beyond it.
    fn model_indicator(&self, ui: &mut egui::Ui) {
        let simulation = &self.simulation;
        let index = self.selected;
        let held = simulation.held_by(index);
        let held_model = held.map(|h| {
            format!(
                "The exact spacetime of {} (Kerr), as a test body, with gravitational-wave losses at leading order (2.5PN), and Newtonian tides from everything else",
                simulation.bodies[h].name
            )
        });
        let model = match simulation.computed(index) {
            Computed::TopLevel if held_model.is_some() => held_model.as_deref().unwrap_or(""),
            Computed::TopLevel => {
                "Relativistic N-body gravity (Einstein–Infeld–Hoffmann), with each pair's second-order (2PN) terms and gravitational-wave losses (2.5PN and 3.5PN): it pulls on every massive body and they on it"
            }
            Computed::MoonSystem => {
                "Its planet's moon system: Newtonian, the planet's field, tides"
            }
            Computed::SmallMoonInDetail => {
                "Follows the major moons: Newtonian, the planet's field, tides"
            }
            Computed::SmallMoonOnMeanOrbit => "JPL's mean orbit, carried at its mean rates",
            Computed::Follower { forces: false } => {
                "Follows the Sun and planets: Newtonian, plus the Sun's relativistic term"
            }
            Computed::GalacticStar => {
                "Follows Sagittarius A*: its Newtonian pull plus its first relativistic (Schwarzschild) term"
            }
            Computed::Swarm => {
                "A swarm particle: carried on its orbit around what holds it, nudged by every other body (Wisdom–Holman)"
            }
            Computed::Follower { forces: true } => {
                "Follows the Sun and planets: Newtonian, the Sun's relativistic term, outgassing"
            }
        };
        let (green, amber, red) = (
            Color32::from_rgb(110, 205, 140),
            Color32::from_rgb(240, 180, 80),
            Color32::from_rgb(240, 110, 100),
        );
        ui.add_space(4.0);
        ui.label(RichText::new("Model").strong());
        ui.label(RichText::new(model).small());
        let mut notes: Vec<(Color32, String)> = Vec::new();
        if let Some(regime) = simulation.regime(index) {
            // The top level keeps every term through second order; the
            // rest of the models, first.
            let kept = if simulation.computed(index) == Computed::TopLevel {
                2
            } else {
                1
            };
            let validity = match held {
                Some(h) => {
                    let (m, big) = (simulation.bodies[index].gm, simulation.bodies[h].gm);
                    regime.test_body_validity(m * big / ((m + big) * (m + big)))
                }
                None => regime.validity(kept),
            };
            let (color, verdict) = match validity {
                Validity::Within => (green, "within range"),
                Validity::Approximate => (amber, "approximate: higher orders show"),
                Validity::Beyond => (red, "beyond this model: gravity too strong"),
            };
            notes.push((
                color,
                format!(
                    "Gravity GM/rc² = {:.1e}, speed v/c = {:.1e}: {verdict}",
                    regime.epsilon, regime.beta
                ),
            ));
        }
        if simulation.computed(index) == Computed::SmallMoonOnMeanOrbit {
            notes.push((
                amber,
                "Approximate position: fly to its planet to compute it in detail".into(),
            ));
        }
        if let Some(waves) = simulation.gravitational_waves(index) {
            notes.push((
                green,
                format!(
                    "Gravitational waves: its orbit's period shrinks {} a year, and the pair merges in {} (Peters & Mathews, at leading order)",
                    short_time(-waves.shrink * JULIAN_YEAR),
                    long_time(waves.merging)
                ),
            ));
            if let Some(plunge) = waves.plunge {
                notes.push((
                    green,
                    format!(
                        "In the hole's exact spacetime, circling, it reaches the innermost stable orbit and plunges in {}",
                        long_time(plunge)
                    ),
                ));
            }
            if !waves.converging && waves.plunge.is_none() {
                notes.push((
                    red,
                    "Its last orbits: the post-Newtonian radiation reaction no longer converges here, so the motion is beyond this model, and the pair merges".into(),
                ));
            }
        }
        let body = &simulation.bodies[index];
        if is_black_hole(body) {
            notes.push((
                green,
                "Bodies it holds that are at least 35 times lighter move in its exact spacetime (Kerr), spin axis taken along the frame's z axis (the ecliptic's north pole)".into(),
            ));
        }
        if let Some(intruder) = simulation.intruder(index) {
            notes.push((
                amber,
                format!(
                    "{intruder} is inside this moon system: its collisions with moons aren't detected yet"
                ),
            ));
        }
        for (color, text) in notes {
            ui.horizontal_wrapped(|ui| {
                // A painted dot: the interface font has no circle glyph.
                let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, color);
                ui.label(RichText::new(text).small());
            });
        }
        ui.add_space(4.0);
    }

    /// The top bar's sandbox tools: adding bodies, saving and loading.
    fn sandbox_controls(&mut self, ui: &mut egui::Ui) {
        let label = match self.tool {
            Some(entry) => format!("Adding: {}", entry.label),
            None => "Add a body".to_string(),
        };
        if ui
            .selectable_label(self.catalogue_open, label)
            .on_hover_text(
                "Open the catalogue: planets, stars, white dwarfs, neutron stars, black holes",
            )
            .clicked()
        {
            self.catalogue_open = !self.catalogue_open;
        }
        if self.tool.is_some() && ui.button("Done").clicked() {
            self.tool = None;
            self.launch = None;
        }
        if ui.button("Save").clicked() {
            let message = match sandbox::save(&mut self.simulation) {
                Ok(path) => format!("Saved {}", path.display()),
                Err(e) => format!("Couldn't save: {e}"),
            };
            self.status = Some((message, Instant::now()));
        }
        ui.menu_button("Load", |ui| {
            let saves = sandbox::saves();
            if saves.is_empty() {
                ui.label(RichText::new("No saves yet").color(theme::MUTED));
            }
            for (name, path) in saves.into_iter().take(20) {
                if ui.button(name).clicked() {
                    self.load(&path);
                    ui.close();
                }
            }
        });
        if let Some((message, since)) = &self.status {
            if since.elapsed() < Duration::from_secs(6) {
                ui.label(RichText::new(message).small().color(theme::MUTED));
            } else {
                self.status = None;
            }
        }
    }

    /// The names of the followed and selected bodies, to find them again
    /// after the list changes.
    fn attended_names(&self) -> (String, String) {
        let bodies = &self.simulation.bodies;
        (
            bodies[self.focus].name.clone(),
            bodies[self.selected].name.clone(),
        )
    }

    /// After an update: reports any collisions and what happened to the
    /// swarm (bodies swallowed, moons torn away), and finds the followed
    /// and selected bodies (`names`, from before the update) again, moving
    /// on to whatever absorbed them.
    fn after_events(&mut self, names: (String, String)) {
        let events = std::mem::take(&mut self.simulation.events);
        let notices = std::mem::take(&mut self.simulation.notices);
        if events.is_empty() && notices.is_empty() {
            return;
        }
        let survivor = |name: String| {
            events.iter().fold(name, |n, e| {
                if n == e.absorbed {
                    e.survivor.clone()
                } else {
                    n
                }
            })
        };
        self.reattend((survivor(names.0), survivor(names.1)));
        let mut parts = Vec::new();
        if let Some(last) = events.last() {
            let mut message = match &last.merger {
                Some(m) => format!(
                    "{} and {} merged into one black hole: {} Suns radiated as gravitational waves; it spins at {:.2} and recoils at {} km/s",
                    last.absorbed,
                    last.survivor,
                    significant(m.radiated_gm / GM_SUN),
                    m.spin,
                    significant(m.kick.length() / 1e3)
                ),
                None => format!(
                    "{} hit {} at {:.1} km/s and merged",
                    last.absorbed,
                    last.survivor,
                    last.speed / 1e3
                ),
            };
            if !last.freed.is_empty() {
                message += &format!(", freeing {}", last.freed.join(", "));
            }
            if events.len() > 1 {
                message += &format!(" ({} collisions)", events.len());
            }
            parts.push(message);
        }
        parts.extend(notices.last().cloned());
        self.status = Some((parts.join("; "), Instant::now()));
    }

    /// Finds the followed and selected bodies again by name; the Sun if
    /// they're gone.
    fn reattend(&mut self, (focus, selected): (String, String)) {
        self.focus = self.simulation.index_of(&focus).unwrap_or(SUN);
        self.selected = self.simulation.index_of(&selected).unwrap_or(self.focus);
        self.reveal = true;
    }

    /// Adds a body from the command line: `kind` ("earth", "jupiter",
    /// "sun", or any catalogue entry's name, like "Sagittarius A*") on a
    /// circular orbit `au` from body `around` (the Sun, usually), in the
    /// ecliptic, on the far side of it from where the x axis points.
    fn add_on_circle(&mut self, kind: &str, au: f64, around: usize) {
        let key = match kind {
            "earth" => "copy:Earth".to_string(),
            "jupiter" => "copy:Jupiter".to_string(),
            "sun" => "copy:Sun".to_string(),
            name => catalogue::catalogue()
                .iter()
                .find(|e| e.label.eq_ignore_ascii_case(name))
                .map_or_else(String::new, |e| e.key.clone()),
        };
        let Some(entry) = catalogue::entry(&key) else {
            eprintln!("worldline: nothing called `{kind}` in the catalogue");
            return;
        };
        let center = self.simulation.bodies[around].position;
        let position = center - DVec3::X * au * AU;
        self.place(Launch {
            entry,
            position,
            around,
            drag: position,
            at_rest: self.start_at_rest,
        });
    }

    /// Adds what is being launched: its bodies around their center of
    /// mass, which moves at the launch velocity.
    fn place(&mut self, launch: Launch) {
        let names = self.attended_names();
        let entry = launch.entry;
        let mut velocity = launch.velocity(&self.simulation, self.camera.distance, entry.gm());
        // Something heavier than what it circles takes that body around
        // their common center of mass instead of dragging it off: each
        // gets its share of their relative motion.
        let around = &self.simulation.bodies[launch.around];
        let (gm, other) = (entry.gm(), around.gm);
        if gm > other {
            let relative = velocity - around.velocity;
            velocity = around.velocity + relative * (other / (gm + other));
            self.simulation
                .shift_velocity(launch.around, -relative * (gm / (gm + other)));
        }
        let bodies = entry.bodies(&self.simulation);
        let added: Vec<String> = bodies.iter().map(|b| b.name.clone()).collect();
        for body in bodies {
            let (p, v) = (body.position, body.velocity);
            self.simulation.add_body_near(
                body.at(launch.position + p).moving(velocity + v),
                &entry.key,
                launch.around,
            );
        }
        self.reattend(names);
        self.selected = self.simulation.index_of(&added[0]).unwrap_or(self.selected);
        self.reveal = true;
        self.status = Some((format!("Added {}", added.join(" and ")), Instant::now()));
    }

    /// Removes the selected body, if it can be.
    fn remove_selected(&mut self) {
        if !matches!(
            self.simulation.removal(self.selected),
            Removal::Allowed { .. }
        ) {
            return;
        }
        let (focus, removed) = self.attended_names();
        self.simulation.remove(self.selected);
        self.reattend((focus, removed.clone()));
        self.status = Some((format!("Removed {removed}"), Instant::now()));
    }

    /// Replaces the simulation with a saved one.
    fn load(&mut self, path: &std::path::Path) {
        let loaded = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Simulation::load(&text, self.simulation.speed));
        match loaded {
            Ok(mut simulation) => {
                let names = self.attended_names();
                simulation.paused = self.simulation.paused;
                self.simulation = simulation;
                self.flight = None;
                self.launch = None;
                self.reattend(names);
                self.status = Some((
                    format!(
                        "Loaded {}",
                        path.file_stem().unwrap_or_default().to_string_lossy()
                    ),
                    Instant::now(),
                ));
            }
            Err(e) => self.status = Some((format!("Couldn't load: {e}"), Instant::now())),
        }
    }

    /// Inspector rows for the Sun's reach: sunlight and its travel time,
    /// and the solar wind, at the selected body; for the Sun itself, its
    /// output.
    fn sun_reach(&self, ui: &mut egui::Ui) {
        if !self.simulation.has_sun() {
            return;
        }
        let bodies = &self.simulation.bodies;
        let (sun, body) = (&bodies[SUN], &bodies[self.selected]);
        let wind = &self.simulation.wind;
        if self.selected == SUN {
            ui.label(key("Luminosity"));
            ui.label(format!("{SOLAR_LUMINOSITY:.4e} W"));
            ui.end_row();
            ui.label(key("Solar wind at Earth"));
            ui.label(format!(
                "{:.0} km/s, {:.1} protons/cm³ (2025 average)",
                wind.speed / 1e3,
                wind.density_at_1au / 1e6
            ));
            ui.end_row();
            return;
        }
        let offset = body.position - sun.position;
        let r = offset.length();
        ui.label(key("Sunlight"));
        ui.label(format!(
            "{:.1} W/m² ({:.3}× at 1 AU)",
            irradiance(r),
            irradiance(r) / irradiance(AU)
        ));
        ui.end_row();
        // From the Sun's surface, including the Shapiro delay.
        let travel = light_time(sun.gm, offset.normalize() * sun.radius, offset);
        ui.label(key("Sunlight left the Sun"));
        ui.label(format!("{} ago", duration(travel)));
        ui.end_row();
        let shell = &self.simulation.heliosphere;
        if r < shell.termination_shock * shell.shape(offset) {
            ui.label(key("Solar wind"));
            ui.label(format!(
                "{:.0} km/s, {:.3} protons/cm³",
                wind.speed / 1e3,
                wind.density(r) / 1e6
            ));
            ui.end_row();
            let sin_colatitude = self.simulation.rotation(SUN).map_or(1.0, |m| {
                m.spin_axis(self.simulation.julian_date())
                    .cross(offset / r)
                    .length()
            });
            ui.label(key("Its magnetic field"));
            ui.label(format!(
                "{:.1}° from the radial (Parker spiral)",
                wind.angle(r, sin_colatitude).to_degrees()
            ));
            ui.end_row();
        } else {
            ui.label(key("Solar wind"));
            ui.label(if r < shell.heliopause * shell.shape(offset) {
                "slowed in the heliosheath (past the termination shock)"
            } else {
                "none: outside the heliopause, in interstellar space"
            });
            ui.end_row();
        }
    }

    /// Inspector rows for the selected planet's magnetic field and
    /// magnetosphere.
    fn magnetism(&self, ui: &mut egui::Ui) {
        let bodies = &self.simulation.bodies;
        let body = &bodies[self.selected];
        if matches!(body.name.as_str(), "Venus" | "Mars") {
            ui.label(key("Magnetic field"));
            ui.label("no global field: the solar wind meets the upper atmosphere");
            ui.end_row();
            return;
        }
        let Some((dipole, model)) = &self.simulation.dipoles[self.selected] else {
            return;
        };
        ui.label(key("Magnetic field"));
        ui.label(format!(
            "{:.0} nT at the equator, tilted {:.1}° ({model})",
            dipole.equatorial_field() * 1e9,
            dipole.tilt().to_degrees()
        ));
        ui.end_row();
        if self.simulation.has_sun() {
            let r = (body.position - bodies[SUN].position).length();
            let pressure = self.simulation.flow_pressure_at_1au * (AU / r).powi(2);
            ui.label(key("Magnetopause"));
            ui.label(format!(
                "{:.1} radii sunward (pressure balance)",
                standoff(dipole, pressure) / dipole.radius
            ));
            ui.end_row();
        }
        let belts: Vec<String> = self
            .simulation
            .radiation_belts
            .iter()
            .filter(|b| b.planet == body.name)
            .map(|b| format!("{} L {}–{}", b.name, b.l_range.0, b.l_range.1))
            .collect();
        if !belts.is_empty() {
            ui.label(key("Radiation belts"));
            ui.label(belts.join(", "));
            ui.end_row();
        }
    }

    fn scene(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let viewport = response.rect;
        painter.rect_filled(viewport, 0.0, Color32::from_rgb(3, 5, 12));

        if response.dragged() && self.tool.is_none() {
            let delta = response.drag_delta();
            self.camera.orbit(delta.x, delta.y);
        }
        let focus_radius = self.simulation.bodies[self.focus].radius;
        let min_distance = (MIN_DISTANCE_RADII * focus_radius).max(1e3);
        if response.hovered() && self.flight.is_none() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll, min_distance);
            }
        }
        self.camera.clamp_distance(min_distance);

        let render_state = frame.wgpu_render_state();
        let globes_allowed = self.options.globes && render_state.is_some();
        let attention = view::Attention {
            focus: self.focus,
            selected: self.selected,
            pointer: response.hover_pos(),
        };
        let layout = view::layout(
            &self.camera,
            viewport,
            &self.simulation,
            globes_allowed,
            attention,
        );
        view::draw_under(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            &layout,
            attention,
        );
        if let Some(state) = render_state
            && let Some(texture) =
                self.draw_globes(state, ui.ctx().pixels_per_point(), viewport, &layout)
        {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(texture, viewport, uv, Color32::WHITE);
        }
        let drawn = view::draw_over(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            &layout,
            attention,
        );
        if let Some(pointer) = response.hover_pos()
            && view::pick(&drawn, pointer).is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        self.dropping(&response, &painter, viewport);
        if let Some(entry) = self.tool {
            self.adding(ui, &response, viewport, entry);
            if let Some(launch) = &self.launch {
                launch.draw(&painter, &self.camera, viewport, &self.simulation);
            }
        } else if (response.clicked() || response.double_clicked())
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(i) = view::pick(&drawn, pointer)
        {
            self.selected = i;
            self.reveal = true;
            if response.double_clicked() {
                self.focus_on(i);
            }
        }
        if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.remove_selected();
        }

        painter.text(
            viewport.left_bottom() + vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            if self.tool.is_some() {
                "Click to place in a circular orbit · drag to launch · Esc to stop adding"
            } else {
                "Drag to rotate · scroll to zoom · double-click a body to fly to it"
            },
            FontId::proportional(12.0),
            Color32::from_gray(140),
        );
    }

    /// Placing a body: press where it goes (on the plane of the body in
    /// focus, parallel to the ecliptic), drag to launch it, release to add
    /// it. A click without a drag puts it in a circular orbit.
    fn adding(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        viewport: Rect,
        entry: &'static Entry,
    ) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.launch.take().is_none() {
                self.tool = None;
            }
            return;
        }
        let around = self.launch_center();
        let plane = self.camera.target.z;
        let here = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .and_then(|p| self.camera.point_on_plane(p, viewport, plane));
        if response.drag_started()
            && let Some(position) = here
        {
            self.launch = Some(Launch {
                entry,
                position,
                around,
                drag: position,
                at_rest: self.start_at_rest,
            });
        }
        if response.dragged()
            && let (Some(launch), Some(at)) = (&mut self.launch, here)
        {
            launch.drag = at;
        }
        if response.drag_stopped()
            && let Some(launch) = self.launch.take()
        {
            self.place(launch);
        }
        if response.clicked()
            && let Some(position) = here
        {
            self.place(Launch {
                entry,
                position,
                around,
                drag: position,
                at_rest: self.start_at_rest,
            });
        }
    }

    /// What a body orbits when placed without a drag: the body in focus,
    /// if it has mass, or else body 0 (the Sun, or what took its place).
    fn launch_center(&self) -> usize {
        if self.simulation.bodies[self.focus].gm > 0.0 {
            self.focus
        } else {
            self.simulation.region_center(self.focus)
        }
    }

    /// Drag and drop from the catalogue: while an entry is held over the
    /// view, shows where it would go and the orbit it would follow; when
    /// it is let go, adds it there on a circular orbit.
    fn dropping(&mut self, response: &egui::Response, painter: &egui::Painter, viewport: Rect) {
        let plane = self.camera.target.z;
        let position = response
            .ctx
            .pointer_hover_pos()
            .and_then(|p| self.camera.point_on_plane(p, viewport, plane));
        let Some(position) = position else {
            return;
        };
        let launch = |entry: &'static Entry| Launch {
            entry,
            position,
            around: self.launch_center(),
            drag: position,
            at_rest: self.start_at_rest,
        };
        if let Some(entry) = response.dnd_hover_payload::<&'static Entry>() {
            launch(*entry).draw(painter, &self.camera, viewport, &self.simulation);
        }
        if let Some(entry) = response.dnd_release_payload::<&'static Entry>() {
            let launch = launch(*entry);
            self.place(launch);
        }
    }

    /// The catalogue window: everything that can be added, by kind. Drag
    /// an entry into the view, or click it and then click in the view.
    /// Records the waves an observer on Earth would get from the selected
    /// body's pair, and opens their window.
    fn open_waves(&mut self) {
        let index = self.selected;
        let Some(pair) = self.simulation.gravitational_waves(index) else {
            return;
        };
        let (a, b) = (
            self.simulation.bodies[index].clone(),
            self.simulation.bodies[pair.partner].clone(),
        );
        let earth = self
            .simulation
            .index_of("Earth")
            .map_or(DVec3::ZERO, |i| self.simulation.bodies[i].position);
        let recording = waves::record(&a, &b, earth, waves::MAX_ORBITS);
        let sound = waves::sound(&recording);
        self.stop_sound();
        self.waves = Some(WaveView {
            names: format!("{} and {}", a.name, b.name),
            recording,
            sound,
        });
    }

    fn stop_sound(&mut self) {
        if let Some((_, sink)) = &self.audio {
            sink.stop();
        }
    }

    /// Plays the recorded waves' sound.
    fn play_sound(&mut self) {
        let Some(view) = &self.waves else {
            return;
        };
        if self.audio.is_none() {
            match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => match rodio::Sink::try_new(&handle) {
                    Ok(sink) => self.audio = Some((stream, sink)),
                    Err(e) => self.status = Some((format!("No sound: {e}"), Instant::now())),
                },
                Err(e) => self.status = Some((format!("No sound: {e}"), Instant::now())),
            }
        }
        if let Some((_, sink)) = &self.audio {
            sink.stop();
            sink.append(rodio::buffer::SamplesBuffer::new(
                1,
                waves::RATE,
                view.sound.samples.clone(),
            ));
            sink.play();
        }
    }

    /// The window with a pair's gravitational waves: the strain at Earth
    /// over time, what it shows, and its sound.
    fn waves_window(&mut self, ctx: &egui::Context) {
        let Some(view) = &self.waves else {
            return;
        };
        let mut open = true;
        let mut play = false;
        let mut stop = false;
        let mut save = false;
        egui::Window::new(format!("Gravitational waves from {}", view.names))
            .id(egui::Id::new("gravitational waves"))
            .open(&mut open)
            .default_pos(pos2(340.0, 90.0))
            .default_width(660.0)
            .show(ctx, |ui| {
                let r = &view.recording;
                let n = r.times.len();
                let span = r.times[n - 1];
                // The inspiral's last sample, before any ringdown.
                let last = r
                    .final_hole
                    .as_ref()
                    .map_or(n - 1, |hole| r.times.partition_point(|&t| t <= hole.start) - 1);
                let (f0, f1) = (r.frequency[0], r.frequency[last]);
                ui.label(
                    RichText::new(format!(
                        "The strain h+ (the \"plus\" polarization) an observer on Earth would record, {} away: the leading-order (quadrupole) waveform, from a copy of the pair run ahead with Worldline's gravity.",
                        length(r.distance)
                    ))
                    .small(),
                );
                ui.label(
                    RichText::new(format!(
                        "{} of signal: the inspiral, from {} to {}{}.",
                        long_time(span),
                        frequency(f0),
                        frequency(f1),
                        r.final_hole.as_ref().map_or(String::new(), |hole| format!(
                            ", then the final hole's ringdown, {}",
                            frequency(hole.tone.frequency)
                        ))
                    ))
                    .small(),
                );
                // The tail, +4π x^(3/2) in the sweep, with x^(3/2) = πGmf/c³,
                // in percent.
                let tail = 400.0 * PI * PI * r.gm * f0 / C.powi(3);
                let tail = if tail >= 0.1 {
                    format!("{tail:.1}%")
                } else {
                    format!("{tail:.1e}%")
                };
                if r.held {
                    // The orbit is exact; the radiation reaction is the
                    // leading-order one.
                    ui.label(
                        RichText::new(
                            "The lighter moves in the heavier's exact spacetime (Kerr). Approximate: it loses energy at the leading-order (quadrupole) rate; close to the innermost stable orbit general relativity's differs by tens of percent.",
                        )
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                } else {
                    ui.label(
                        RichText::new(format!(
                            "Approximate: the sweep leaves out the tail (the waves scattering off the curved spacetime around the pair), which makes general relativity's {tail} faster at {}, and more at higher frequencies.",
                            frequency(f0)
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                }
                let rough = waves::rough_above(r.gm, r.nu);
                if !r.held && rough <= f1 {
                    let from = if rough <= f0 {
                        "From the start".to_string()
                    } else {
                        format!("Above {}", frequency(rough))
                    };
                    ui.label(
                        RichText::new(format!(
                            "{from}, the tail outgrows the last correction kept (the first post-Newtonian one), so the sweep is only rough there: general relativity's is faster."
                        ))
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                }
                let ending = if r.held {
                    "at the innermost stable orbit, where the plunge begins"
                } else {
                    "where the post-Newtonian description gives out, a few orbits before the merger"
                };
                if let Some(hole) = &r.final_hole {
                    ui.label(
                        RichText::new(format!(
                            "The inspiral ends {ending}. Then the final hole, {} Suns spinning at {:.2} (numerical-relativity fits), rings down at {}, fading in {} (Berti, Cardoso & Will 2006).",
                            significant(hole.gm / GM_SUN),
                            hole.spin,
                            frequency(hole.tone.frequency),
                            any_time(hole.tone.damping)
                        ))
                        .small(),
                    );
                    ui.label(
                        RichText::new(
                            "Not modeled: the plunge and merger between, where the real wave is loudest. The ringdown starts where the inspiral ends, at its strength.",
                        )
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                } else if r.reached_end {
                    ui.label(
                        RichText::new(format!("It ends {ending}."))
                            .small()
                            .color(theme::MUTED),
                    );
                }
                waveform_plot(ui, r);
                ui.horizontal(|ui| {
                    play = ui.button("▶ Play").clicked();
                    stop = ui.button("■ Stop").clicked();
                    save = ui.button("Save as WAV").clicked();
                    let how = if view.sound.speedup == 1.0 {
                        "in real time, as the strain itself would sound".to_string()
                    } else {
                        format!(
                            "{} times faster than real time, so its highest frequency sounds at 400 Hz",
                            significant(view.sound.speedup)
                        )
                    };
                    ui.label(
                        RichText::new(format!(
                            "{:.1} s of sound, {how}",
                            view.sound.samples.len() as f64 / f64::from(waves::RATE)
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                });
            });
        if play {
            self.play_sound();
        }
        if stop {
            self.stop_sound();
        }
        if save && let Some(view) = &self.waves {
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("worldline-gravitational-waves.wav");
            let message = match std::fs::write(&path, waves::wav(&view.sound)) {
                Ok(()) => format!("Saved {}", path.display()),
                Err(e) => format!("Couldn't save {}: {e}", path.display()),
            };
            self.status = Some((message, Instant::now()));
        }
        if !open {
            self.stop_sound();
            self.waves = None;
        }
    }

    fn catalogue_window(&mut self, ctx: &egui::Context) {
        let mut open = self.catalogue_open;
        egui::Window::new("Add a body")
            .open(&mut open)
            .default_pos(pos2(16.0, 64.0))
            .default_width(300.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Drag one into the view, or pick it and click there. \
                         Drag in the view to launch it.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Start").small().color(theme::MUTED));
                    ui.selectable_value(&mut self.start_at_rest, false, "circling")
                        .on_hover_text("On a circular orbit around the body in focus");
                    ui.selectable_value(&mut self.start_at_rest, true, "at rest")
                        .on_hover_text("Still, relative to the body in focus: they fall together");
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        for group in Group::ALL {
                            egui::CollapsingHeader::new(RichText::new(group.label()).strong())
                                .default_open(true)
                                .show(ui, |ui| {
                                    for entry in
                                        catalogue::catalogue().iter().filter(|e| e.group == group)
                                    {
                                        self.catalogue_entry(ui, entry);
                                    }
                                });
                        }
                    });
            });
        self.catalogue_open = open;
    }

    /// One catalogue entry: its name and a line on its mass and size. It
    /// can be dragged into the view, or clicked to pick it.
    fn catalogue_entry(&mut self, ui: &mut egui::Ui, entry: &'static Entry) {
        let picked = self.tool.is_some_and(|t| std::ptr::eq(t, entry));
        let id = egui::Id::new(("catalogue", &entry.key));
        let inner = ui.dnd_drag_source(id, entry, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        3.5,
                        catalogue::kind_color(entry.kind()),
                    );
                    let name = RichText::new(&entry.label);
                    ui.label(if picked {
                        name.color(theme::ACCENT)
                    } else {
                        name
                    });
                });
                ui.label(RichText::new(&entry.summary).small().color(theme::MUTED));
            });
        });
        let response = inner.response.on_hover_ui(|ui| {
            ui.set_max_width(320.0);
            if let Some(o) = entry.object() {
                ui.label(RichText::new(&o.location).strong());
                ui.label(RichText::new(&o.source).small());
            } else {
                ui.label(&entry.summary);
            }
        });
        if response.clicked() {
            self.tool = if picked { None } else { Some(entry) };
            self.launch = None;
        }
        ui.add_space(2.0);
    }

    /// Draws the bodies that are big enough on screen as textured, lit,
    /// rotating globes on the GPU.
    fn draw_globes(
        &mut self,
        state: &RenderState,
        pixels_per_point: f32,
        viewport: Rect,
        layout: &[OnScreen],
    ) -> Option<TextureId> {
        let eye = self.camera.eye();
        let jd = self.simulation.julian_date();
        let bodies = &self.simulation.bodies;
        let light = self.simulation.light_source();
        let globes: Vec<Globe> = layout
            .iter()
            .filter(|item| item.globe)
            .map(|item| {
                let body = &bodies[item.index];
                // Relative to the camera, in double precision, before the
                // GPU's single precision takes over.
                let center = (body.position - eye).as_vec3();
                let orientation = self
                    .simulation
                    .rotation(item.index)
                    .map_or(Mat3::IDENTITY, |m| {
                        m.orientation(jd).body_to_ecliptic.as_mat3()
                    });
                // True shape from the IAU models; mean radius if unknown.
                let radii = worldline_data::triaxial_radii(&body.name)
                    .map_or(DVec3::splat(body.radius), DVec3::from_array)
                    .as_vec3();
                let rings = (body.name == "Saturn").then(|| {
                    let profile = worldline_data::saturn_rings();
                    Rings {
                        inner: profile.inner as f32,
                        outer: profile.outer() as f32,
                    }
                });
                let atmosphere = worldline_data::atmosphere(&body.name).map(|a| Atmosphere {
                    height: a.height as f32,
                    scale_height: a.scale_height as f32,
                    rayleigh: DVec3::from_array(a.rayleigh).as_vec3(),
                });
                Globe {
                    name: body.name.clone(),
                    color: view::color(&self.simulation, item.index),
                    center,
                    radii,
                    orientation,
                    // No light at all if a black hole took the Sun's place.
                    sun_direction: light.map_or(glam::Vec3::ZERO, |l| {
                        (l - body.position).normalize_or_zero().as_vec3()
                    }),
                    emissive: self.simulation.shines(item.index),
                    night_glow: if body.name == "Earth" { 1.0 } else { 0.0 },
                    rings,
                    atmosphere,
                }
            })
            .collect();
        if globes.is_empty() {
            return None;
        }
        // The near clipping plane sits halfway to the closest surface, ring
        // or top of the atmosphere.
        let nearest_surface = globes
            .iter()
            .map(|g| {
                let reach = g.radii.max_element().max(g.rings.map_or(0.0, |r| r.outer))
                    + g.atmosphere.map_or(0.0, |a| a.height);
                g.center.length() - reach
            })
            .fold(f32::INFINITY, f32::min);
        let (forward, _, up) = self.camera.basis();
        let view = View {
            size: [
                (viewport.width() * pixels_per_point).round() as u32,
                (viewport.height() * pixels_per_point).round() as u32,
            ],
            fov_y: self.camera.fov_y as f32,
            forward: forward.as_vec3(),
            up: up.as_vec3(),
            near: (0.5 * nearest_surface).max(1.0),
        };
        let gpu = self.gpu.get_or_insert_with(|| GpuGlobes::new(state));
        gpu.draw(state, view, &globes)
    }
}

impl eframe::App for WorldlineApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let now = Instant::now();
        // Cap the step after a stall (e.g. dragging the window) at 0.1 s.
        let real_dt = now.duration_since(self.last_frame).as_secs_f64().min(0.1);
        self.last_frame = now;
        let names = self.attended_names();
        self.simulation.update(real_dt, PHYSICS_BUDGET);
        self.after_events(names);
        self.aim_camera();

        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::right("inspector")
            .default_size(290.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.scene(ui, frame));
        self.catalogue_window(ui.ctx());
        self.waves_window(ui.ctx());

        if !self.simulation.paused || self.flight.is_some() {
            ui.ctx().request_repaint();
        }
    }
}

/// Direction of a body's orbital angular momentum around `center`.
fn orbit_normal(body: &worldline_core::Body, center: &worldline_core::Body) -> DVec3 {
    (body.position - center.position)
        .cross(body.velocity - center.velocity)
        .normalize()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_and_long_times_read_naturally() {
        assert_eq!(short_time(7.58e-5), "75.8 µs");
        assert_eq!(short_time(0.034), "34 ms");
        assert_eq!(long_time(301e6 * JULIAN_YEAR), "301 million years");
        assert_eq!(long_time(4.2e9 * JULIAN_YEAR), "4.2 billion years");
        assert_eq!(long_time(12.0 * DAY), "12 days");
        assert_eq!(long_time(3.5 * JULIAN_YEAR), "3.5 years");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(52.7), "52.7 s");
        assert_eq!(duration(496.68), "8 min 16.7 s");
        assert_eq!(duration(4.0 * 3600.0 + 9.4 * 60.0), "4 h 9 min");
    }
}
