//! Drawing the 3D scene in layers: reference rings and orbit trails at the
//! back, then textured globes (drawn on the GPU, see `gpu.rs`), then dots
//! for bodies too small to see, spin axes and labels on top.

use eframe::egui::{Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, vec2};
use worldline_core::DVec3;
use worldline_core::constants::AU;
use worldline_core::orbit::osculating_orbit;

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

/// Reference rings in the ecliptic plane, in AU.
const RING_RADII_AU: [f64; 6] = [1.0, 2.0, 5.0, 10.0, 20.0, 50.0];

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
        _ => Color32::WHITE,
    }
}

fn screen(camera: &Camera, viewport: Rect, point: DVec3) -> Option<Projection> {
    camera
        .project(point, viewport)
        .filter(|p| (p.position - viewport.center()).length() < MAX_SCREEN_OFFSET)
}

/// Works out which bodies are visible, how big, and which become globes.
/// Sorted far to near. A moon is left out until the view resolves its
/// orbit: until its distance from its planet, on screen, clears the
/// planet's dot or globe.
pub fn layout(
    camera: &Camera,
    viewport: Rect,
    simulation: &Simulation,
    globes_allowed: bool,
) -> Vec<OnScreen> {
    let bodies = &simulation.bodies;
    let mut visible: Vec<OnScreen> = bodies
        .iter()
        .enumerate()
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
    selected: usize,
) {
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
                    format!("{radius_au} AU"),
                    FontId::proportional(11.0),
                    Color32::from_rgba_unmultiplied(140, 160, 200, 110),
                );
            }
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
            draw_trail(painter, &points, body_color(&body.name));
        }
    }

    // The Sun's corona sits behind its disk.
    if let Some(sun) = layout.iter().find(|s| s.index == 0) {
        draw_corona(painter, sun.projection.position, sun.radius);
    }
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
    selected: usize,
) -> Vec<DrawnBody> {
    let bodies = &simulation.bodies;
    let mut drawn = Vec::with_capacity(layout.len());
    for item in layout {
        if !item.globe && hidden_behind_globe(item, layout) {
            continue;
        }
        let body = &bodies[item.index];
        let color = body_color(&body.name);
        let center = item.projection.position;
        if !item.globe {
            painter.circle_filled(center, item.radius, color);
        }
        // Up close it's obvious what's selected, so large globes skip the ring.
        if item.index == selected && item.radius < SELECTION_RING_MAX_RADIUS {
            painter.circle_stroke(center, item.radius + 4.0, Stroke::new(1.5, Color32::WHITE));
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

    if options.labels {
        // The selected body is labeled first, then the most massive ones. A
        // label is skipped if it would overlap one already placed, as with
        // a planet's inner moons when zoomed out.
        let mut order: Vec<&DrawnBody> = drawn.iter().collect();
        order.sort_by(|a, b| {
            (b.index == selected)
                .cmp(&(a.index == selected))
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
