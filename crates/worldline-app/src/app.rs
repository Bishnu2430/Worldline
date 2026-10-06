//! The Worldline window: the 3D view plus its control panels.

use std::f64::consts::FRAC_PI_2;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, TextureId, pos2, vec2};
use eframe::egui_wgpu::RenderState;
use glam::Mat3;
use worldline_core::DVec3;
use worldline_core::constants::{AU, DAY, JULIAN_YEAR};
use worldline_render::{Atmosphere, Rings, View};

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::details;
use crate::gpu::{Globe, GpuGlobes};
use crate::simulation::Simulation;
use crate::view::{self, OnScreen, ViewOptions, body_color};
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
            },
            last_frame: Instant::now(),
            flight: None,
            gpu: None,
        };
        app.simulation.paused = start.paused;
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
            // Each planet with its moons listed beneath it.
            let mut fly_to = None;
            let simulation = &self.simulation;
            let mut entry = |ui: &mut egui::Ui, i: usize| {
                let name = &simulation.bodies[i].name;
                let text = RichText::new(name).color(body_color(name));
                let response = ui.selectable_label(i == self.selected, text);
                if response.clicked() {
                    self.selected = i;
                }
                if response.double_clicked() {
                    fly_to = Some(i);
                }
            };
            for i in 0..simulation.bodies.len() {
                if simulation.parent(i).is_some() || simulation.small_body(i).is_some() {
                    continue;
                }
                entry(ui, i);
                let (small, major): (Vec<usize>, Vec<usize>) = (0..simulation.bodies.len())
                    .filter(|&m| simulation.parent(m) == Some(i))
                    .partition(|&m| simulation.is_small_moon(m));
                if !major.is_empty() || !small.is_empty() {
                    ui.indent(i, |ui| {
                        for m in major {
                            entry(ui, m);
                        }
                        if !small.is_empty() {
                            egui::CollapsingHeader::new(format!("{} small moons", small.len()))
                                .id_salt(("small moons", i))
                                .show(ui, |ui| {
                                    for m in small {
                                        entry(ui, m);
                                    }
                                });
                        }
                    });
                }
            }
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
                egui::CollapsingHeader::new(format!("{label} ({})", members.len()))
                    .id_salt(label)
                    .show(ui, |ui| {
                        for i in members {
                            entry(ui, i);
                        }
                    });
            }
            if let Some(i) = fly_to {
                self.focus_on(i);
            }
            ui.separator();
            self.selected_body(ui);
            ui.separator();

            ui.heading("Physics");
            egui::Grid::new("physics").num_columns(2).show(ui, |ui| {
                ui.label("Gravity");
                ui.label(self.simulation.gravity().name());
                ui.end_row();
                ui.label("Moons");
                ui.label(self.simulation.moon_gravity_name());
                ui.end_row();
                ui.label("Integrator");
                ui.label(self.simulation.integrator_name());
                ui.end_row();
                ui.label("Data");
                ui.label("NASA JPL Horizons (DE441 and satellite ephemerides)");
                ui.end_row();
                ui.label("Start");
                ui.label("2025-01-01 00:00 TDB");
                ui.end_row();
                ui.label("Rotation");
                ui.label("IAU models (NASA NAIF)");
                ui.end_row();
                ui.label("Surfaces");
                ui.label("Solar System Scope, CC BY 4.0");
                ui.end_row();
            });
            ui.separator();

            ui.heading("Display");
            ui.checkbox(
                &mut self.options.trails,
                "Orbit trails (moons: current orbit)",
            );
            ui.checkbox(&mut self.options.grid, "Distance rings (ecliptic plane)");
            ui.checkbox(&mut self.options.labels, "Labels");
            ui.checkbox(
                &mut self.options.spin_axes,
                "Spin axes (dot marks the spin direction)",
            );
            ui.checkbox(&mut self.options.globes, "Textured globes up close");
            let total: usize = self
                .simulation
                .belts
                .iter()
                .map(|b| b.positions.len())
                .sum();
            ui.checkbox(
                &mut self.options.belts,
                format!("Asteroid belt, Trojans and Kuiper belt ({total} real orbits)"),
            );
            ui.checkbox(&mut self.options.dust, "Zodiacal dust");
            ui.add_space(4.0);
            ui.label(
                RichText::new(
                    "Distances, sizes and positions are to scale. Bodies too small to \
                     see are drawn as dots at least 3 points wide. Night sides get a \
                     faint fill light so they don't vanish completely.\n\n\
                     Belts: JPL's orbits (measured), on fixed ellipses around the \
                     Sun (model, about 1/10,000 of their distance off after a year). \
                     Dust: the COBE DIRBE model of Kelsall et al. 1998, drawn as the \
                     sunlight it scatters on a log scale (visual), held at its \
                     0.3 AU brightness farther in, where no probe has measured how \
                     it brightens. Both show when zoomed out past 0.05 AU.",
                )
                .small()
                .weak(),
            );
        });
    }

    fn selected_body(&mut self, ui: &mut egui::Ui) {
        let bodies = &self.simulation.bodies;
        let body = &bodies[self.selected];
        let sun = &bodies[SUN];
        let parent = self.simulation.parent(self.selected);
        ui.heading(RichText::new(&body.name).color(body_color(&body.name)));
        egui::Grid::new("selected").num_columns(2).show(ui, |ui| {
            ui.label("Mass");
            if body.gm > 0.0 {
                ui.label(format!("{:.4e} kg", body.mass()));
                ui.end_row();
                ui.label("");
                ui.label(format!("{:.6} Earth masses", body.gm / GM_EARTH));
            } else {
                ui.label("not measured");
            }
            ui.end_row();
            ui.label("Radius");
            ui.label(if body.radius > 0.0 {
                format!("{:.1} km", body.radius / 1e3)
            } else {
                "not measured".to_string()
            });
            ui.end_row();
            if let Some(parent) = parent {
                let planet = &bodies[parent];
                ui.label(format!("Distance from {}", planet.name));
                ui.label(format!(
                    "{:.0} km",
                    (body.position - planet.position).length() / 1e3
                ));
                ui.end_row();
                ui.label(format!("Speed relative to {}", planet.name));
                ui.label(format!(
                    "{:.3} km/s",
                    (body.velocity - planet.velocity).length() / 1e3
                ));
                ui.end_row();
            } else if self.selected != SUN {
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
            if let Some(model) = self.simulation.rotation(self.selected) {
                let jd = self.simulation.julian_date();
                let hours = model.sidereal_period() / 3600.0;
                ui.label("Day (sidereal)");
                ui.label(if hours < 72.0 {
                    format!("{hours:.3} h")
                } else {
                    format!("{:.2} days", hours / 24.0)
                });
                ui.end_row();
                ui.label("Spin");
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
        let kind = if let Some((kind, outgassing)) = self.simulation.small_body(self.selected) {
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
        if self.focus == self.selected {
            ui.label(RichText::new("The camera is following this body.").weak());
        } else if ui.button("Fly to it").clicked() {
            self.focus_on(self.selected);
        }
    }

    fn scene(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let viewport = response.rect;
        painter.rect_filled(viewport, 0.0, Color32::from_rgb(3, 5, 12));

        if response.dragged() {
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
        let layout = view::layout(
            &self.camera,
            viewport,
            &self.simulation,
            globes_allowed,
            (self.focus, self.selected),
        );
        view::draw_under(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            &layout,
            self.selected,
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
            self.selected,
        );

        if (response.clicked() || response.double_clicked())
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(i) = view::pick(&drawn, pointer)
        {
            self.selected = i;
            if response.double_clicked() {
                self.focus_on(i);
            }
        }

        painter.text(
            viewport.left_bottom() + vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            "Drag to rotate · scroll to zoom · double-click a body to fly to it",
            FontId::proportional(12.0),
            Color32::from_gray(140),
        );
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
                    color: body_color(&body.name),
                    center,
                    radii,
                    orientation,
                    sun_direction: (sun - body.position).normalize_or_zero().as_vec3(),
                    emissive: item.index == SUN,
                    night_glow: if item.index == EARTH { 1.0 } else { 0.0 },
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
