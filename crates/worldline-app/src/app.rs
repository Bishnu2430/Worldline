//! The Worldline window: the 3D view plus its control panels.

use std::f64::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use eframe::egui::collapsing_header::CollapsingState;
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, TextureId, pos2, vec2};
use eframe::egui_wgpu::RenderState;
use glam::Mat3;
use worldline_core::DVec3;
use worldline_core::constants::{AU, DAY, JULIAN_YEAR, SOLAR_LUMINOSITY};
use worldline_core::magnetosphere::standoff;
use worldline_core::sunlight::{irradiance, light_time};
use worldline_render::{Atmosphere, Rings, View};

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::details;
use crate::gpu::{Globe, GpuGlobes};
use crate::sandbox::{self, Launch, Preset};
use crate::simulation::Removal;
use crate::simulation::Simulation;
use crate::theme;
use crate::view::{self, OnScreen, ViewOptions};
use worldline_data::SmallBodyKind;

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
    /// The body a press in the view adds, if adding; otherwise a drag looks
    /// around.
    tool: Option<Preset>,
    /// A body being placed, while its launch is dragged.
    launch: Option<Launch>,
    /// A message in the top bar, and when it appeared.
    status: Option<(String, Instant)>,
    /// Open the selected body's branch of the list and scroll to it, once
    /// it has been picked somewhere other than the list.
    reveal: bool,
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
            launch: None,
            status: None,
        };
        app.simulation.paused = start.paused;
        if let Some((kind, au)) = &start.add {
            app.add_on_circle(kind, *au);
        }
        app.simulation.advance_by(start.advance_years * JULIAN_YEAR);
        if let Some(name) = &start.focus {
            match app.simulation.bodies.iter().position(|b| &b.name == name) {
                Some(i) => app.fly_to(i, start.zoom_radii.unwrap_or(FOCUS_DISTANCE_RADII)),
                None => eprintln!("worldline: no body named `{name}` to focus on"),
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
    fn fly_to(&mut self, index: usize, radii: f64) {
        let radius = self.simulation.bodies[index].radius;
        self.flight = Some(Flight {
            from_target: self.camera.target,
            from_distance: self.camera.distance,
            to_distance: if radius > 0.0 {
                radii * radius
            } else {
                UNKNOWN_SIZE_DISTANCE
            },
            start: Instant::now(),
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
        let t = (flight.start.elapsed().as_secs_f64() / FLIGHT_SECONDS).min(1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        self.camera.target = flight.from_target.lerp(target, eased);
        // Zoom evenly in scale, so going from 4 AU to 25,000 km looks smooth.
        let (from, to) = (flight.from_distance.ln(), flight.to_distance.ln());
        self.camera.distance = (from + (to - from) * eased).exp();
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
                state
                    .show_header(ui, |ui| {
                        entry(ui, i);
                        ui.label(
                            RichText::new(if moons == 1 {
                                "1 moon".to_string()
                            } else {
                                format!("{moons} moons")
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
        egui::Grid::new("selected").num_columns(2).show(ui, |ui| {
            ui.label(key("Mass"));
            if body.gm > 0.0 {
                ui.label(format!("{:.4e} kg", body.mass()));
                ui.end_row();
                ui.label("");
                ui.label(format!("{:.6} Earth masses", body.gm / GM_EARTH));
            } else {
                ui.label(key("not measured"));
            }
            ui.end_row();
            ui.label(key("Radius"));
            ui.label(if body.radius > 0.0 {
                format!("{:.1} km", body.radius / 1e3)
            } else {
                "not measured".to_string()
            });
            ui.end_row();
            if let Some(parent) = parent {
                let planet = &bodies[parent];
                ui.label(key(&format!("Distance from {}", planet.name)));
                ui.label(format!(
                    "{:.0} km",
                    (body.position - planet.position).length() / 1e3
                ));
                ui.end_row();
                ui.label(key(&format!("Speed relative to {}", planet.name)));
                ui.label(format!(
                    "{:.3} km/s",
                    (body.velocity - planet.velocity).length() / 1e3
                ));
                ui.end_row();
            } else if self.selected != SUN {
                ui.label(key("Distance from Sun"));
                ui.label(format!(
                    "{:.4} AU",
                    (body.position - sun.position).length() / AU
                ));
                ui.end_row();
                ui.label(key("Speed relative to Sun"));
                ui.label(format!(
                    "{:.2} km/s",
                    (body.velocity - sun.velocity).length() / 1e3
                ));
                ui.end_row();
            }
            self.sun_reach(ui);
            self.magnetism(ui);
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
        ui.add_space(4.0);
        let kind = if self.simulation.is_added(self.selected) {
            details::BodyKind::Added
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
            }
        });
    }

    /// The top bar's sandbox tools: adding bodies, saving and loading.
    fn sandbox_controls(&mut self, ui: &mut egui::Ui) {
        egui::ComboBox::from_id_salt("add")
            .selected_text(self.tool.map_or("Add a body", Preset::label))
            .show_ui(ui, |ui| {
                for preset in Preset::ALL {
                    if ui
                        .selectable_label(self.tool == Some(preset), preset.label())
                        .clicked()
                    {
                        self.tool = Some(preset);
                    }
                }
            });
        if self.tool.is_some() && ui.button("Done").clicked() {
            self.tool = None;
            self.launch = None;
        }
        if ui.button("Save").clicked() {
            let message = match sandbox::save(&self.simulation) {
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

    /// Finds the followed and selected bodies again by name; the Sun if
    /// they're gone.
    fn reattend(&mut self, (focus, selected): (String, String)) {
        self.focus = self.simulation.index_of(&focus).unwrap_or(SUN);
        self.selected = self.simulation.index_of(&selected).unwrap_or(self.focus);
        self.reveal = true;
    }

    /// Adds a body from the command line: `kind` ("earth", "jupiter" or
    /// "sun") on a circular orbit `au` from the Sun, in the ecliptic, on
    /// the far side of the Sun from where the x axis points.
    fn add_on_circle(&mut self, kind: &str, au: f64) {
        let preset = match kind {
            "earth" => Preset::Earth,
            "sun" => Preset::Sun,
            _ => Preset::Jupiter,
        };
        let sun = self.simulation.bodies[SUN].position;
        let position = sun - DVec3::X * au * AU;
        self.place(Launch {
            preset,
            position,
            around: SUN,
            drag: position,
        });
    }

    /// Adds the body being launched.
    fn place(&mut self, launch: Launch) {
        let names = self.attended_names();
        let template = launch.preset.body(&self.simulation);
        let velocity = launch.velocity(&self.simulation, self.camera.distance, template.gm);
        let name = template.name.clone();
        self.simulation
            .add_body(template.at(launch.position).moving(velocity));
        self.reattend(names);
        self.selected = self.simulation.index_of(&name).unwrap_or(self.selected);
        self.status = Some((format!("Added {name}"), Instant::now()));
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
        let r = (body.position - bodies[SUN].position).length();
        let pressure = self.simulation.flow_pressure_at_1au * (AU / r).powi(2);
        ui.label(key("Magnetopause"));
        ui.label(format!(
            "{:.1} radii sunward (pressure balance)",
            standoff(dipole, pressure) / dipole.radius
        ));
        ui.end_row();
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

        if let Some(preset) = self.tool {
            self.adding(ui, &response, viewport, preset);
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
    fn adding(&mut self, ui: &egui::Ui, response: &egui::Response, viewport: Rect, preset: Preset) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.launch.take().is_none() {
                self.tool = None;
            }
            return;
        }
        let around = if self.simulation.bodies[self.focus].gm > 0.0 {
            self.focus
        } else {
            SUN
        };
        let plane = self.camera.target.z;
        let here = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .and_then(|p| self.camera.point_on_plane(p, viewport, plane));
        if response.drag_started()
            && let Some(position) = here
        {
            self.launch = Some(Launch {
                preset,
                position,
                around,
                drag: position,
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
                preset,
                position,
                around,
                drag: position,
            });
        }
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
        let sun = bodies[SUN].position;
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
                    sun_direction: (sun - body.position).normalize_or_zero().as_vec3(),
                    emissive: item.index == SUN,
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
        self.simulation.update(real_dt, PHYSICS_BUDGET);
        self.aim_camera();

        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::right("inspector")
            .default_size(290.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.scene(ui, frame));

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
    fn durations_read_naturally() {
        assert_eq!(duration(52.7), "52.7 s");
        assert_eq!(duration(496.68), "8 min 16.7 s");
        assert_eq!(duration(4.0 * 3600.0 + 9.4 * 60.0), "4 h 9 min");
    }
}
