//! Drawing the 3D scene in layers: reference rings and orbit trails at the
//! back, then textured globes (drawn on the GPU, see `gpu.rs`), then dots
//! for bodies too small to see, spin axes and labels on top.

use eframe::egui::{Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, vec2};
use worldline_core::DVec3;
use worldline_core::constants::{AU, C};
use worldline_core::magnetosphere::{field_line_point, magnetopause_radius, shue_1998, standoff};
use worldline_core::orbit::osculating_orbit;
use worldline_core::solar_wind::ParkerSpiral;
use worldline_core::zodiacal::ZodiacalCloud;
use worldline_data::BeltKind;

use crate::camera::{Camera, Projection};
use crate::simulation::Simulation;

/// Which layers to draw.
#[derive(Debug, Clone, Copy)]
pub struct ViewOptions {
    pub trails: bool,
    pub grid: bool,
    pub labels: bool,
    pub spin_axes: bool,
    pub globes: bool,
    pub belts: bool,
    pub dust: bool,
    pub solar_wind: bool,
    pub heliosphere: bool,
    pub magnetospheres: bool,
}

/// A visible body and how it will be drawn.
#[derive(Debug, Clone, Copy)]
pub struct OnScreen {
    pub index: usize,
    pub projection: Projection,
    /// Radius actually drawn, never below the minimum dot size.
    pub radius: f32,
    /// Big enough to draw as a textured globe instead of a dot.
    pub globe: bool,
}

/// A body as drawn on screen, kept for picking with the mouse.
#[derive(Debug, Clone, Copy)]
pub struct DrawnBody {
    pub index: usize,
    pub center: Pos2,
    pub radius: f32,
}

/// What the viewer is attending to: the body the camera follows, the one
/// selected, and where the pointer is over the view, if it is.
#[derive(Debug, Clone, Copy)]
pub struct Attention {
    pub focus: usize,
    pub selected: usize,
    pub pointer: Option<Pos2>,
}

impl Attention {
    /// The planet in focus: the followed planet, or the followed moon's.
    fn planet(&self, simulation: &Simulation) -> usize {
        simulation.parent(self.focus).unwrap_or(self.focus)
    }

    /// Whether body `index` gets its name shown. Zoomed out, names would
    /// bury the view, so only these do: the body followed, the one
    /// selected, the one under the pointer, any body near enough to be
    /// drawn as a globe, and the major moons of the planet in focus (which
    /// appear only once the view resolves their orbits).
    fn names(
        &self,
        simulation: &Simulation,
        index: usize,
        globe: bool,
        hovered: Option<usize>,
    ) -> bool {
        index == self.focus
            || index == self.selected
            || Some(index) == hovered
            || globe
            || (simulation.parent(index) == Some(self.planet(simulation))
                && !simulation.is_small_moon(index))
    }
}

/// How close (points) the pointer must come to a marker to name it.
const HOVER_REACH: f32 = 10.0;

/// Bodies smaller than this on screen are drawn as dots of this radius so
/// they stay visible. At this size and above they become globes.
const MIN_RADIUS: f32 = 3.0;
const SUN_MIN_RADIUS: f32 = 5.0;

/// Screen positions farther than this from the viewport are treated as off
/// screen, so nearly-behind-camera points don't produce huge coordinates.
const MAX_SCREEN_OFFSET: f32 = 1e6;

/// Globes larger than this on screen skip the selection ring.
const SELECTION_RING_MAX_RADIUS: f32 = 20.0;

/// Labels keep at least this much space between them.
const LABEL_GAP: f32 = 2.0;

/// The belts and the zodiacal dust are drawn only while the camera is at
/// least this far from what it looks at: closer in, their markers would
/// scatter across the sky like stars that aren't there.
const BELT_MIN_VIEW: f64 = 0.05 * AU;

/// Size of a belt body's marker, in points.
const BELT_MARKER: f32 = 1.6;

/// Reference rings in the ecliptic plane, in AU.
const RING_RADII_AU: [f64; 8] = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];

/// How long light takes to cross `distance` (m), for a label.
fn light_time_label(distance: f64) -> String {
    let seconds = distance / C;
    if seconds < 3600.0 {
        format!("{:.1} light-minutes", seconds / 60.0)
    } else {
        format!("{:.1} light-hours", seconds / 3600.0)
    }
}

/// Display color for body `index`: by its name, or the sandbox's color if
/// it was added there.
pub fn color(simulation: &Simulation, index: usize) -> Color32 {
    if simulation.is_added(index) {
        crate::sandbox::SANDBOX_COLOR
    } else {
        body_color(&simulation.bodies[index].name)
    }
}

/// Display color for a body.
pub fn body_color(name: &str) -> Color32 {
    match name {
        "Sun" => Color32::from_rgb(255, 214, 102),
        "Mercury" => Color32::from_rgb(169, 163, 155),
        "Venus" => Color32::from_rgb(230, 200, 140),
        "Earth" => Color32::from_rgb(90, 150, 255),
        "Moon" => Color32::from_rgb(200, 200, 200),
        "Mars" => Color32::from_rgb(226, 110, 70),
        "Jupiter" => Color32::from_rgb(216, 180, 130),
        "Saturn" => Color32::from_rgb(230, 205, 140),
        "Uranus" => Color32::from_rgb(150, 215, 225),
        "Neptune" => Color32::from_rgb(90, 120, 240),
        "Pluto" => Color32::from_rgb(200, 170, 140),
        // Moons, in roughly their true tints.
        "Phobos" | "Deimos" => Color32::from_rgb(150, 135, 120),
        "Io" => Color32::from_rgb(235, 215, 110),
        "Europa" => Color32::from_rgb(220, 205, 180),
        "Ganymede" => Color32::from_rgb(175, 160, 145),
        "Callisto" => Color32::from_rgb(130, 115, 100),
        "Titan" => Color32::from_rgb(225, 170, 80),
        "Iapetus" => Color32::from_rgb(170, 150, 120),
        "Triton" => Color32::from_rgb(215, 195, 190),
        "Charon" => Color32::from_rgb(170, 165, 160),
        "Mimas" | "Enceladus" | "Tethys" | "Dione" | "Rhea" | "Hyperion" => {
            Color32::from_rgb(225, 225, 225)
        }
        "Miranda" | "Ariel" | "Umbriel" | "Titania" | "Oberon" => Color32::from_rgb(185, 180, 175),
        _ => small_body_color(name).unwrap_or(Color32::WHITE),
    }
}

/// Colors for the dwarf planets, asteroids and comets, by kind.
fn small_body_color(name: &str) -> Option<Color32> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    use worldline_data::SmallBodyKind;
    static KINDS: OnceLock<HashMap<String, SmallBodyKind>> = OnceLock::new();
    let kinds = KINDS.get_or_init(|| {
        worldline_data::small_bodies()
            .into_iter()
            .map(|b| (b.name, b.kind))
            .collect()
    });
    Some(match kinds.get(name)? {
        SmallBodyKind::DwarfPlanet => Color32::from_rgb(210, 190, 165),
        SmallBodyKind::TransNeptunian => Color32::from_rgb(190, 150, 130),
        SmallBodyKind::Asteroid => Color32::from_rgb(165, 155, 140),
        SmallBodyKind::Comet => Color32::from_rgb(150, 220, 235),
        SmallBodyKind::Interstellar => Color32::from_rgb(230, 140, 220),
    })
}

fn screen(camera: &Camera, viewport: Rect, point: DVec3) -> Option<Projection> {
    camera
        .project(point, viewport)
        .filter(|p| (p.position - viewport.center()).length() < MAX_SCREEN_OFFSET)
}

/// Works out which bodies are visible, how big, and which become globes.
/// Sorted far to near. A moon shows only around the planet in focus (see
/// [`Simulation::in_view`]), and only once the view resolves its orbit:
/// once its distance from its planet, on screen, clears the planet's dot or
/// globe.
pub fn layout(
    camera: &Camera,
    viewport: Rect,
    simulation: &Simulation,
    globes_allowed: bool,
    attention: Attention,
) -> Vec<OnScreen> {
    let (focus, selected) = (attention.focus, attention.selected);
    let bodies = &simulation.bodies;
    let mut visible: Vec<OnScreen> = bodies
        .iter()
        .enumerate()
        .filter(|&(index, _)| simulation.in_view(index, focus, selected))
        .filter_map(|(index, body)| {
            let projection = screen(camera, viewport, body.position)?;
            if let Some(parent) = simulation.parent(index) {
                let planet = &bodies[parent];
                let orbit =
                    (body.position - planet.position).length() * projection.points_per_meter;
                let planet_radius = (planet.radius * projection.points_per_meter) as f32;
                if (orbit as f32) < planet_radius.max(MIN_RADIUS) + 2.0 * MIN_RADIUS {
                    return None;
                }
            }
            let true_radius = (body.radius * projection.points_per_meter) as f32;
            let min_radius = if index == 0 {
                SUN_MIN_RADIUS
            } else {
                MIN_RADIUS
            };
            Some(OnScreen {
                index,
                projection,
                radius: true_radius.max(min_radius),
                globe: globes_allowed && true_radius >= min_radius,
            })
        })
        .collect();
    visible.sort_by(|a, b| b.projection.depth.total_cmp(&a.projection.depth));
    visible
}

/// The back layer: reference rings, orbit trails and the Sun's glow.
/// Small moons, hundreds of them, show their orbit only when `selected`.
pub fn draw_under(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    simulation: &Simulation,
    options: ViewOptions,
    layout: &[OnScreen],
    attention: Attention,
) {
    let (focus, selected) = (attention.focus, attention.selected);
    let screen = |point: DVec3| screen(camera, viewport, point);
    if options.grid {
        let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(110, 130, 170, 45));
        for radius_au in RING_RADII_AU {
            let radius = radius_au * AU;
            let ring = (0..=128).map(|k| {
                let angle = k as f64 / 128.0 * std::f64::consts::TAU;
                DVec3::new(radius * angle.cos(), radius * angle.sin(), 0.0)
            });
            draw_path(painter, &ring.map(screen).collect::<Vec<_>>(), stroke);
            if options.labels
                && let Some(p) = screen(DVec3::new(radius, 0.0, 0.0))
            {
                painter.text(
                    p.position + vec2(4.0, 0.0),
                    Align2::LEFT_CENTER,
                    format!("{radius_au} AU · {}", light_time_label(radius)),
                    FontId::proportional(11.0),
                    Color32::from_rgba_unmultiplied(140, 160, 200, 110),
                );
            }
        }
    }

    if options.magnetospheres {
        draw_magnetosphere(painter, camera, viewport, simulation, attention);
    }
    if options.heliosphere {
        draw_heliosphere(
            painter,
            camera,
            viewport,
            simulation,
            options.labels.then_some(attention.pointer).flatten(),
        );
    }
    if camera.distance >= BELT_MIN_VIEW {
        let sun = simulation.bodies[0].position;
        // The Sun in focus: its wind's spiral field.
        if options.solar_wind && focus == 0 {
            draw_solar_wind(painter, camera, viewport, simulation);
        }
        if options.dust {
            draw_dust(painter, camera, viewport, sun);
        }
        if options.belts {
            draw_belts(painter, camera, viewport, simulation, sun);
        }
    }

    let bodies = &simulation.bodies;
    if options.trails {
        for (index, (trail, body)) in simulation.trails.iter().zip(bodies).enumerate() {
            let points: Vec<_> = match simulation.parent(index) {
                // A moon can circle its planet several times between
                // physics steps, too fast for a recorded trail: it gets its
                // current (osculating) orbit instead, once it is drawn at
                // all (see `layout`).
                Some(parent) => {
                    if !layout.iter().any(|s| s.index == index)
                        || (simulation.is_small_moon(index) && index != selected)
                    {
                        continue;
                    }
                    let planet = &bodies[parent];
                    let Some(orbit) = osculating_orbit(
                        body.position - planet.position,
                        body.velocity - planet.velocity,
                        planet.gm + body.gm,
                        180,
                    ) else {
                        continue;
                    };
                    orbit
                        .into_iter()
                        .map(|p| screen(planet.position + p))
                        .collect()
                }
                None => trail.points().chain([body.position]).map(screen).collect(),
            };
            draw_trail(painter, &points, color(simulation, index));
        }
    }

    // The Sun's corona sits behind its disk.
    if let Some(sun) = layout.iter().find(|s| s.index == 0) {
        draw_corona(painter, sun.projection.position, sun.radius);
    }
}

/// Draws the Parker spiral: field lines rooted every 30° of solar longitude
/// on the source surface, in the Sun's equatorial plane, turning with the
/// Sun. They fade out by 10 AU, where they have wound round about one and a half times.
fn draw_solar_wind(painter: &Painter, camera: &Camera, viewport: Rect, simulation: &Simulation) {
    const LINES: usize = 12;
    const REACH: f64 = 10.0 * AU;
    let Some(model) = simulation.rotation(0) else {
        return;
    };
    let frame = model.orientation(simulation.julian_date()).body_to_ecliptic;
    let sun = simulation.bodies[0].position;
    let wind = &simulation.wind;
    // A step of 2° of lag, but no more than 5% of the distance.
    let lag_step = 2f64.to_radians() * wind.speed / wind.rotation_rate;
    for k in 0..LINES {
        let root = k as f64 * std::f64::consts::TAU / LINES as f64;
        let mut r = ParkerSpiral::SOURCE_SURFACE;
        let mut previous: Option<Pos2> = None;
        while r <= REACH {
            let longitude = root - wind.lag(r);
            let local = DVec3::new(r * longitude.cos(), r * longitude.sin(), 0.0);
            let here = screen(camera, viewport, sun + frame * local).map(|p| p.position);
            if let (Some(a), Some(b)) = (previous, here) {
                let alpha = (90.0 * (1.0 - r / REACH)) as u8;
                painter.line_segment(
                    [a, b],
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(130, 180, 235, alpha)),
                );
            }
            previous = here;
            r += lag_step.min(0.05 * r);
        }
    }
}

/// Draws the magnetosphere of the planet in focus, if it has a global
/// field: the magnetopause, its nose where the planet's field balances the
/// solar wind's pressure and its flanks flaring as Shue et al. measured at
/// Earth, pointed into the wind as the moving planet meets it; and, around
/// Earth, its two radiation belts, traced along dipole field lines.
fn draw_magnetosphere(
    painter: &Painter,
    camera: &Camera,
    viewport: Rect,
    simulation: &Simulation,
    attention: Attention,
) {
    let planet = attention.planet(simulation);
    let Some((dipole, _)) = &simulation.dipoles[planet] else {
        return;
    };
    let bodies = &simulation.bodies;
    let (sun, body) = (&bodies[0], &bodies[planet]);
    let offset = body.position - sun.position;
    let r = offset.length();
    let pressure = simulation.flow_pressure_at_1au * (AU / r).powi(2);
    let nose_distance = standoff(dipole, pressure);
    // Too far out to see it, or so close it would fill the sky.
    if camera.distance > 60.0 * nose_distance || camera.distance < 1.5 * body.radius {
        return;
    }
    let (_, flaring) = shue_1998(pressure * 1e9, 0.0);
    // The wind as the planet meets it: radially out from the Sun at its
    // speed, less the planet's own orbital motion (aberration).
    let wind = offset / r * simulation.wind.speed - (body.velocity - sun.velocity);
    let nose = -wind.normalize();
    let across = nose.any_orthonormal_vector();
    let other = nose.cross(across);
    let stroke = Stroke::new(1.0, Color32::from_rgba_unmultiplied(90, 205, 220, 70));
    let point = |theta: f64, phi: f64| {
        let direction = nose * theta.cos() + (across * phi.cos() + other * phi.sin()) * theta.sin();
        body.position + direction * magnetopause_radius(nose_distance, flaring, theta)
    };
    for ring in 1..=5 {
        let theta = (ring as f64 * 22.0).to_radians();
        let circle: Vec<_> = (0..=72)
            .map(|j| point(theta, j as f64 / 72.0 * std::f64::consts::TAU))
            .map(|p| screen(camera, viewport, p))
            .collect();
        draw_path(painter, &circle, stroke);
    }
    for meridian in 0..12 {
        let phi = meridian as f64 / 12.0 * std::f64::consts::TAU;
        let line: Vec<_> = (0..=40)
            .map(|j| point((j as f64 / 40.0 * 110.0).to_radians(), phi))
            .map(|p| screen(camera, viewport, p))
            .collect();
        draw_path(painter, &line, stroke);
    }
    // Radiation belts: each drawn as its inner and outer field-line shells,
    // around the magnetic axis as it points now.
    let Some(model) = simulation.rotation(planet) else {
        return;
    };
    let frame = model.orientation(simulation.julian_date()).body_to_ecliptic;
    let axis = frame * dipole.moment_direction();
    let (x, y) = (
        axis.any_orthonormal_vector(),
        axis.cross(axis.any_orthonormal_vector()),
    );
    for belt in simulation
        .radiation_belts
        .iter()
        .filter(|b| b.planet == body.name)
    {
        let color = if belt.name == "inner" {
            Color32::from_rgba_unmultiplied(245, 150, 90, 80)
        } else {
            Color32::from_rgba_unmultiplied(140, 175, 255, 80)
        };
        for l in [belt.l_range.0, belt.l_range.1] {
            // The field line meets the surface where cos²λ = 1/L.
            let reach = (1.0 / l).sqrt().acos();
            for k in 0..24 {
                let phi = k as f64 / 24.0 * std::f64::consts::TAU;
                let line: Vec<_> = (0..=32)
                    .map(|j| {
                        let latitude = -reach + 2.0 * reach * j as f64 / 32.0;
                        let local = field_line_point(dipole, l, latitude, phi);
                        body.position + x * local.x + y * local.y + axis * local.z
                    })
                    .map(|p| screen(camera, viewport, p))
                    .collect();
                draw_path(painter, &line, Stroke::new(1.0, color));
            }
        }
    }
}

/// Draws the termination shock and the heliopause as faint wireframes,
/// with the Voyager crossings marked. Only from outside the termination
/// shock: from inside, the shells would surround the camera.
fn draw_heliosphere(
    painter: &Painter,
    camera: &Camera,
    viewport: Rect,
    simulation: &Simulation,
    pointer: Option<Pos2>,
) {
    // The shape is drawn out to 120° from the nose; beyond, the tail is
    // unmeasured.
    const OPEN: f64 = 120.0;
    let shell = &simulation.heliosphere;
    let sun = simulation.bodies[0].position;
    let eye = camera.eye() - sun;
    if eye.length() < shell.termination_shock * shell.shape(eye) {
        return;
    }
    let across = shell.nose.any_orthonormal_vector();
    let other = shell.nose.cross(across);
    let point = |r0: f64, theta: f64, phi: f64| {
        let direction =
            shell.nose * theta.cos() + (across * phi.cos() + other * phi.sin()) * theta.sin();
        sun + direction * r0 * shell.shape(direction)
    };
    for (r0, color) in [
        (
            shell.termination_shock,
            Color32::from_rgba_unmultiplied(235, 160, 90, 60),
        ),
        (
            shell.heliopause,
            Color32::from_rgba_unmultiplied(170, 130, 235, 60),
        ),
    ] {
        let stroke = Stroke::new(1.0, color);
        for ring in 1..=6 {
            let theta = (ring as f64 * OPEN / 6.0).to_radians();
            let circle: Vec<_> = (0..=96)
                .map(|j| point(r0, theta, j as f64 / 96.0 * std::f64::consts::TAU))
                .map(|p| screen(camera, viewport, p))
                .collect();
            draw_path(painter, &circle, stroke);
        }
        for meridian in 0..16 {
            let phi = meridian as f64 / 16.0 * std::f64::consts::TAU;
            let line: Vec<_> = (0..=40)
                .map(|j| point(r0, (j as f64 / 40.0 * OPEN).to_radians(), phi))
                .map(|p| screen(camera, viewport, p))
                .collect();
            draw_path(painter, &line, stroke);
        }
    }
    for crossing in &simulation.crossings {
        let Some(p) = screen(camera, viewport, sun + crossing.position) else {
            continue;
        };
        let color = Color32::from_rgb(250, 235, 160);
        painter.circle_filled(p.position, 3.0, color);
        // Named when the pointer is on it.
        if pointer.is_some_and(|q| (q - p.position).length() < HOVER_REACH) {
            // Each Voyager crossed both boundaries in nearly the same
            // direction: one label above, one below.
            let (what, offset, align) = match crossing.boundary {
                worldline_data::Boundary::TerminationShock => {
                    ("termination shock", vec2(6.0, -2.0), Align2::LEFT_BOTTOM)
                }
                worldline_data::Boundary::Heliopause => {
                    ("heliopause", vec2(6.0, 2.0), Align2::LEFT_TOP)
                }
            };
            painter.text(
                p.position + offset,
                align,
                format!(
                    "{} crossed the {what}, {}, {:.1} AU",
                    crossing.spacecraft,
                    &crossing.date[..4],
                    crossing.published_distance / AU
                ),
                FontId::proportional(11.0),
                color,
            );
        }
    }
}

/// Marker color for each belt.
fn belt_color(kind: BeltKind) -> Color32 {
    match kind {
        BeltKind::AsteroidBelt => Color32::from_rgba_unmultiplied(185, 165, 135, 130),
        BeltKind::JupiterTrojans => Color32::from_rgba_unmultiplied(225, 195, 105, 160),
        BeltKind::KuiperBelt => Color32::from_rgba_unmultiplied(135, 170, 225, 140),
    }
}

/// Draws every belt body as a tiny square, all in one mesh.
fn draw_belts(
    painter: &Painter,
    camera: &Camera,
    viewport: Rect,
    simulation: &Simulation,
    sun: DVec3,
) {
    let mut mesh = Mesh::default();
    let size = vec2(BELT_MARKER, BELT_MARKER);
    for belt in &simulation.belts {
        let color = belt_color(belt.kind);
        for &offset in &belt.positions {
            if let Some(p) = screen(camera, viewport, sun + offset)
                && viewport.contains(p.position)
            {
                mesh.add_colored_rect(Rect::from_center_size(p.position, size), color);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// Inside this distance from the Sun, Helios didn't measure how the
/// zodiacal light brightens (it reached 0.3 AU). The dust goes on inward
/// (it is seen as the Sun's F-corona), but rather than extrapolate the
/// model, the glow is held at its brightness here.
const DUST_MEASURED_FROM: f64 = 0.3 * AU;

/// The glow starts this close to the Sun, about 10 solar radii.
const DUST_INNER_EDGE: f64 = 0.05 * AU;

/// The zodiacal dust's brightness seen face-on, from `DUST_INNER_EDGE` out
/// to the model's 5.2 AU edge: the sunlight the modeled dust scatters along
/// a line through the cloud (see `ZodiacalCloud::scattered_light`).
/// Computed once.
fn dust_profile() -> &'static [(f64, f64)] {
    static PROFILE: std::sync::OnceLock<Vec<(f64, f64)>> = std::sync::OnceLock::new();
    PROFILE.get_or_init(|| {
        let cloud = ZodiacalCloud::kelsall();
        let across = cloud.pole.cross(DVec3::Z).normalize();
        let (inner, outer) = (DUST_INNER_EDGE, cloud.outer_radius);
        (0..=48)
            .map(|k| {
                let rho = inner * (outer / inner).powf(k as f64 / 48.0);
                let above = cloud.center + across * rho + cloud.pole * (6.0 * AU);
                (rho, cloud.scattered_light(above, -cloud.pole))
            })
            .collect()
    })
}

/// Draws the zodiacal dust as a faint glow in its symmetry plane, tilted
/// 2° to the ecliptic. Its brightness spans hundreds of times from 0.3 to
/// 5 AU, so it is shown on a log scale, like the corona: a visual aid with
/// the model's shape.
fn draw_dust(painter: &Painter, camera: &Camera, viewport: Rect, sun: DVec3) {
    const SEGMENTS: usize = 96;
    let cloud = ZodiacalCloud::kelsall();
    let profile = dust_profile();
    let center = sun + cloud.center;
    let across = cloud.pole.cross(DVec3::Z).normalize();
    let along = cloud.pole.cross(across);
    // A thin, transparent disk seen at a slant looks brighter by 1/|cos θ|,
    // up to the cloud's thickness: its density falls by a factor e about
    // 0.3 of the way from the plane to the Sun's distance.
    let view = (camera.eye() - center).normalize();
    let slant = 1.0 / view.dot(cloud.pole).abs().max(0.3);
    let brightest = profile
        .iter()
        .find(|(rho, _)| *rho >= DUST_MEASURED_FROM)
        .map_or(1.0, |p| p.1)
        .ln();
    let faintest = profile[profile.len() - 3].1.ln();
    let mut mesh = Mesh::default();
    let mut drawn = Vec::with_capacity(profile.len() * SEGMENTS);
    for &(rho, light) in profile {
        // Brightest at the measured limit and held there inward (the clamp).
        let level = ((light.ln() - faintest) / (brightest - faintest)).clamp(0.0, 1.0);
        let alpha = (40.0 * level * slant).min(255.0) as u8;
        let color = Color32::from_rgba_unmultiplied(255, 240, 215, alpha);
        for j in 0..SEGMENTS {
            let angle = j as f64 / SEGMENTS as f64 * std::f64::consts::TAU;
            let point = center + (across * angle.cos() + along * angle.sin()) * rho;
            let projected = screen(camera, viewport, point);
            drawn.push(projected.is_some());
            mesh.colored_vertex(projected.map_or(Pos2::ZERO, |p| p.position), color);
        }
    }
    let index = |ring: usize, segment: usize| ring * SEGMENTS + segment % SEGMENTS;
    for ring in 0..profile.len() - 1 {
        for segment in 0..SEGMENTS {
            let quad = [
                index(ring, segment),
                index(ring, segment + 1),
                index(ring + 1, segment + 1),
                index(ring + 1, segment),
            ];
            if quad.iter().all(|&i| drawn[i]) {
                let [a, b, c, d] = quad.map(|i| i as u32);
                mesh.add_triangle(a, b, c);
                mesh.add_triangle(a, c, d);
            }
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// Brightness of the Sun's corona relative to the center of the Sun's disk,
/// at `rho` solar radii from the center: the Baumbach (1937) model of the
/// K-corona, as given in Allen's Astrophysical Quantities.
pub fn corona_brightness(rho: f64) -> f64 {
    1e-6 * (0.0532 * rho.powf(-2.5) + 1.425 * rho.powi(-7) + 2.565 * rho.powi(-17))
}

/// Draws the corona as a soft glow out to 3 solar radii. It's a million
/// times fainter than the disk (that's why it's only seen in total
/// eclipses), so brightness is shown on a log scale: a visual boost, with
/// the shape of the falloff from the Baumbach model.
fn draw_corona(painter: &Painter, center: Pos2, radius: f32) {
    const RINGS: usize = 24;
    const SEGMENTS: usize = 64;
    const OUTER: f64 = 3.0;
    let (bright, faint) = (
        corona_brightness(1.0).log10(),
        corona_brightness(OUTER).log10(),
    );
    let mut mesh = Mesh::default();
    for ring in 0..=RINGS {
        let rho = 1.0 + (OUTER - 1.0) * ring as f64 / RINGS as f64;
        let level = (corona_brightness(rho).log10() - faint) / (bright - faint);
        let alpha = (110.0 * level) as u8;
        let color = Color32::from_rgba_unmultiplied(255, 240, 210, alpha);
        for segment in 0..SEGMENTS {
            let angle = segment as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            let offset = vec2(angle.cos(), angle.sin()) * radius * rho as f32;
            mesh.colored_vertex(center + offset, color);
        }
    }
    let at = |ring: usize, segment: usize| (ring * SEGMENTS + segment % SEGMENTS) as u32;
    for ring in 0..RINGS {
        for segment in 0..SEGMENTS {
            let (a, b) = (at(ring, segment), at(ring, segment + 1));
            let (c, d) = (at(ring + 1, segment), at(ring + 1, segment + 1));
            mesh.add_triangle(a, b, d);
            mesh.add_triangle(a, d, c);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// The front layer: dots for bodies too small to see (unless a globe
/// hides them), the selection ring, spin axes and labels. Returns what was
/// drawn, for picking.
pub fn draw_over(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    simulation: &Simulation,
    options: ViewOptions,
    layout: &[OnScreen],
    attention: Attention,
) -> Vec<DrawnBody> {
    let selected = attention.selected;
    let bodies = &simulation.bodies;
    let mut drawn = Vec::with_capacity(layout.len());
    for item in layout {
        if !item.globe && hidden_behind_globe(item, layout) {
            continue;
        }
        let body = &bodies[item.index];
        let color = color(simulation, item.index);
        let center = item.projection.position;
        if !item.globe {
            painter.circle_filled(center, item.radius, color);
        }
        // Up close it's obvious what's selected, so large globes skip the ring.
        if item.index == selected && item.radius < SELECTION_RING_MAX_RADIUS {
            painter.circle_stroke(
                center,
                item.radius + 4.0,
                Stroke::new(1.5, crate::theme::ACCENT),
            );
        }
        if options.spin_axes
            && let Some(model) = simulation.rotation(item.index)
        {
            let axis = model.spin_axis(simulation.julian_date());
            if item.globe {
                draw_pole_stubs(
                    painter,
                    viewport,
                    camera,
                    body.position,
                    body.radius,
                    axis,
                    item,
                    color,
                );
            } else {
                draw_spin_axis(
                    painter,
                    viewport,
                    camera,
                    body.position,
                    axis,
                    item.radius,
                    item.projection.points_per_meter,
                    color,
                );
            }
        }
        drawn.push(DrawnBody {
            index: item.index,
            center,
            radius: item.radius,
        });
    }

    let hovered = attention.pointer.and_then(|p| pick(&drawn, p));
    if let Some(h) = hovered.filter(|&h| h != selected)
        && let Some(d) = drawn.iter().find(|d| d.index == h)
    {
        painter.circle_stroke(
            d.center,
            d.radius + 4.0,
            Stroke::new(1.0, Color32::from_white_alpha(110)),
        );
    }
    if options.labels {
        // Only some bodies are named (see `Attention::names`): the one
        // under the pointer first, then the selected one, then the most
        // massive. A name is skipped if it would overlap one already placed.
        let globes: Vec<usize> = layout.iter().filter(|s| s.globe).map(|s| s.index).collect();
        let mut order: Vec<&DrawnBody> = drawn
            .iter()
            .filter(|d| attention.names(simulation, d.index, globes.contains(&d.index), hovered))
            .collect();
        let rank = |i: usize| (Some(i) == hovered, i == selected);
        order.sort_by(|a, b| {
            rank(b.index)
                .cmp(&rank(a.index))
                .then(bodies[b.index].gm.total_cmp(&bodies[a.index].gm))
        });
        let color = Color32::from_gray(215);
        let mut placed: Vec<Rect> = Vec::new();
        for d in order {
            let galley = painter.layout_no_wrap(
                bodies[d.index].name.clone(),
                FontId::proportional(13.0),
                color,
            );
            let anchor = d.center + vec2(d.radius + 5.0, 0.0);
            let rect = Align2::LEFT_CENTER.anchor_size(anchor, galley.size());
            if placed.iter().any(|r| r.expand(LABEL_GAP).intersects(rect)) {
                continue;
            }
            placed.push(rect);
            painter.galley(rect.min, galley, color);
        }
    }
    drawn
}

/// Whether a dot sits behind a nearer globe's disk on screen.
fn hidden_behind_globe(item: &OnScreen, layout: &[OnScreen]) -> bool {
    layout.iter().any(|g| {
        g.globe
            && g.projection.depth < item.projection.depth
            && (g.projection.position - item.projection.position).length() < g.radius
    })
}

/// Draws a body's spin axis through it, poking out on both sides. The end
/// the spin points toward (right-hand rule) gets a dot.
#[allow(clippy::too_many_arguments)]
fn draw_spin_axis(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    center: DVec3,
    axis: DVec3,
    radius: f32,
    points_per_meter: f64,
    color: Color32,
) {
    let half_length = f64::from((radius * 1.4).max(12.0)) / points_per_meter;
    let ends = [center - axis * half_length, center + axis * half_length]
        .map(|p| camera.project(p, viewport).map(|p| p.position));
    if let [Some(south), Some(north)] = ends {
        let faint = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 170);
        painter.line_segment([south, north], Stroke::new(1.5, faint));
        painter.circle_filled(north, 2.0, Color32::WHITE);
    }
}

/// For a globe, draws the spin axis only where it sticks out of the poles,
/// hiding any part that passes behind the globe. The end the spin points
/// toward gets a dot.
#[allow(clippy::too_many_arguments)]
fn draw_pole_stubs(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    center: DVec3,
    radius: f64,
    axis: DVec3,
    globe: &OnScreen,
    color: Color32,
) {
    const SAMPLES: usize = 12;
    let faint = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 200);
    let hidden = |p: &Projection| {
        p.depth > globe.projection.depth
            && (p.position - globe.projection.position).length() < globe.radius
    };
    for sign in [1.0, -1.0] {
        let points: Vec<Option<Projection>> = (0..=SAMPLES)
            .map(|k| {
                let r = radius * (1.0 + 0.5 * k as f64 / SAMPLES as f64);
                camera
                    .project(center + axis * (sign * r), viewport)
                    .filter(|p| !hidden(p))
            })
            .collect();
        draw_path(painter, &points, Stroke::new(1.5, faint));
        if sign > 0.0
            && let Some(Some(tip)) = points.last()
        {
            painter.circle_filled(tip.position, 2.5, Color32::WHITE);
        }
    }
}

/// The body under the pointer, if any: the nearest one within reach.
pub fn pick(drawn: &[DrawnBody], pointer: Pos2) -> Option<usize> {
    drawn
        .iter()
        .map(|d| (d, (d.center - pointer).length()))
        .filter(|(d, distance)| *distance <= d.radius.max(6.0) + 4.0)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(d, _)| d.index)
}

/// Draws a trail that fades from transparent (oldest) to bright (newest).
fn draw_trail(painter: &Painter, points: &[Option<Projection>], color: Color32) {
    const CHUNKS: usize = 8;
    let segments = points.len().saturating_sub(1);
    if segments == 0 {
        return;
    }
    for chunk in 0..CHUNKS {
        let start = chunk * segments / CHUNKS;
        let end = (chunk + 1) * segments / CHUNKS;
        let alpha = (20 + 180 * (chunk + 1) / CHUNKS) as u8;
        let stroke = Stroke::new(
            1.2,
            Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha),
        );
        draw_path(painter, &points[start..=end], stroke);
    }
}

/// Draws a polyline, breaking it wherever a point is off screen.
fn draw_path(painter: &Painter, points: &[Option<Projection>], stroke: Stroke) {
    let mut run: Vec<Pos2> = Vec::new();
    for point in points {
        match point {
            Some(p) => run.push(p.position),
            None => flush(painter, &mut run, stroke),
        }
    }
    flush(painter, &mut run, stroke);
}

fn flush(painter: &Painter, run: &mut Vec<Pos2>, stroke: Stroke) {
    if run.len() >= 2 {
        painter.add(Shape::line(std::mem::take(run), stroke));
    }
    run.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on_screen(index: usize, x: f32, depth: f64, radius: f32, globe: bool) -> OnScreen {
        OnScreen {
            index,
            projection: Projection {
                position: Pos2::new(x, 100.0),
                depth,
                points_per_meter: 1.0,
            },
            radius,
            globe,
        }
    }

    #[test]
    fn names_go_only_to_bodies_attended_to() {
        let sim = Simulation::solar_system(1.0);
        let find = |name: &str| sim.bodies.iter().position(|b| b.name == name).unwrap();
        let [sun, earth, mars, moon, jupiter, io, himalia] =
            ["Sun", "Earth", "Mars", "Moon", "Jupiter", "Io", "Himalia"].map(find);
        let names = |focus, selected, index, globe, hovered| {
            let attention = Attention {
                focus,
                selected,
                pointer: None,
            };
            attention.names(&sim, index, globe, hovered)
        };
        // Zoomed out on the Sun with Earth selected: just those two, plus
        // whatever is under the pointer or close enough to be a globe.
        assert!(names(sun, earth, sun, false, None) && names(sun, earth, earth, false, None));
        assert!(!names(sun, earth, mars, false, None) && !names(sun, earth, moon, false, None));
        assert!(names(sun, earth, mars, false, Some(mars)) && names(sun, earth, mars, true, None));
        // Following Jupiter (or one of its moons): its major moons too, but
        // not its small ones unless pointed at.
        for focus in [jupiter, io] {
            assert!(names(focus, focus, io, false, None));
            assert!(!names(focus, focus, himalia, false, None));
            assert!(names(focus, focus, himalia, false, Some(himalia)));
        }
    }

    #[test]
    fn picking_prefers_the_nearest_body_within_reach() {
        let drawn = [
            DrawnBody {
                index: 0,
                center: Pos2::new(100.0, 100.0),
                radius: 3.0,
            },
            DrawnBody {
                index: 1,
                center: Pos2::new(108.0, 100.0),
                radius: 3.0,
            },
        ];
        assert_eq!(pick(&drawn, Pos2::new(106.0, 100.0)), Some(1));
        assert_eq!(pick(&drawn, Pos2::new(101.0, 100.0)), Some(0));
        assert_eq!(pick(&drawn, Pos2::new(300.0, 300.0)), None);
    }

    #[test]
    fn corona_fades_by_three_orders_of_magnitude_by_three_solar_radii() {
        // Baumbach: about 4 × 10⁻⁶ of disk-center brightness at the limb,
        // about 4 × 10⁻⁹ at 3 solar radii.
        assert!((corona_brightness(1.0) - 4.0432e-6).abs() < 1e-9);
        let ratio = corona_brightness(1.0) / corona_brightness(3.0);
        assert!((900.0..1100.0).contains(&ratio), "ratio {ratio}");
    }

    #[test]
    fn a_globe_hides_dots_behind_it_but_not_in_front() {
        let earth = on_screen(3, 100.0, 1e8, 50.0, true);
        let behind = on_screen(4, 120.0, 2e8, 3.0, false);
        let in_front = on_screen(5, 120.0, 5e7, 3.0, false);
        let beside = on_screen(6, 200.0, 2e8, 3.0, false);
        let layout = [earth, behind, in_front, beside];
        assert!(hidden_behind_globe(&behind, &layout));
        assert!(!hidden_behind_globe(&in_front, &layout));
        assert!(!hidden_behind_globe(&beside, &layout));
    }
}
