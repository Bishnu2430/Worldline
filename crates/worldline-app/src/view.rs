//! Drawing the 3D scene: reference rings, orbit trails, bodies and labels.

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, vec2};
use worldline_core::DVec3;
use worldline_core::constants::AU;

use crate::camera::Camera;
use crate::simulation::Simulation;

/// Which layers to draw.
#[derive(Debug, Clone, Copy)]
pub struct ViewOptions {
    pub trails: bool,
    pub grid: bool,
    pub labels: bool,
}

/// A body as drawn on screen, kept for picking with the mouse.
#[derive(Debug, Clone, Copy)]
pub struct DrawnBody {
    pub index: usize,
    pub center: Pos2,
    pub radius: f32,
}

/// Bodies smaller than this on screen are drawn at this radius so they
/// stay visible. True sizes are used whenever they're larger.
const MIN_RADIUS: f32 = 3.0;
const SUN_MIN_RADIUS: f32 = 5.0;

/// Screen positions farther than this from the viewport are treated as off
/// screen, so nearly-behind-camera points don't produce huge coordinates.
const MAX_SCREEN_OFFSET: f32 = 1e6;

/// Bodies closer than this on screen share one label.
const LABEL_CLEARANCE: f32 = 14.0;

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
        _ => Color32::WHITE,
    }
}

/// Draws the scene. Returns where each visible body was drawn, and whether
/// any body was enlarged to the minimum size.
pub fn draw(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    simulation: &Simulation,
    options: ViewOptions,
    selected: usize,
) -> (Vec<DrawnBody>, bool) {
    let screen = |point: DVec3| {
        camera
            .project(point, viewport)
            .filter(|p| (p.position - viewport.center()).length() < MAX_SCREEN_OFFSET)
    };

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

    let bodies = &simulation.system.bodies;
    if options.trails {
        for (trail, body) in simulation.trails.iter().zip(bodies) {
            let points: Vec<_> = trail.points().chain([body.position]).map(screen).collect();
            draw_trail(painter, &points, body_color(&body.name));
        }
    }

    // Far bodies first, so near ones are drawn on top.
    let mut visible: Vec<_> = bodies
        .iter()
        .enumerate()
        .filter_map(|(i, body)| screen(body.position).map(|p| (i, p)))
        .collect();
    visible.sort_by(|a, b| b.1.depth.total_cmp(&a.1.depth));

    let mut drawn = Vec::with_capacity(visible.len());
    let mut enlarged = false;
    for (i, projection) in visible {
        let body = &bodies[i];
        let color = body_color(&body.name);
        let true_radius = (body.radius * projection.points_per_meter) as f32;
        let min_radius = if i == 0 { SUN_MIN_RADIUS } else { MIN_RADIUS };
        enlarged |= true_radius < min_radius;
        let radius = true_radius.max(min_radius);
        let center = projection.position;
        if body.name == "Sun" {
            for (scale, alpha) in [(3.0, 18), (2.0, 35)] {
                let glow = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
                painter.circle_filled(center, radius * scale, glow);
            }
        }
        painter.circle_filled(center, radius, color);
        if i == selected {
            painter.circle_stroke(center, radius + 4.0, Stroke::new(1.5, Color32::WHITE));
        }
        drawn.push(DrawnBody {
            index: i,
            center,
            radius,
        });
    }

    if options.labels {
        // The selected body is labeled first, then the most massive ones. A
        // label is skipped if its body sits on top of one already labeled,
        // like the Moon next to Earth when zoomed out.
        let mut order: Vec<&DrawnBody> = drawn.iter().collect();
        order.sort_by(|a, b| {
            (b.index == selected)
                .cmp(&(a.index == selected))
                .then(bodies[b.index].gm.total_cmp(&bodies[a.index].gm))
        });
        let mut labeled: Vec<Pos2> = Vec::new();
        for d in order {
            if labeled
                .iter()
                .any(|p| (*p - d.center).length() < LABEL_CLEARANCE)
            {
                continue;
            }
            labeled.push(d.center);
            painter.text(
                d.center + vec2(d.radius + 5.0, 0.0),
                Align2::LEFT_CENTER,
                &bodies[d.index].name,
                FontId::proportional(13.0),
                Color32::from_gray(215),
            );
        }
    }
    (drawn, enlarged)
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
fn draw_trail(painter: &Painter, points: &[Option<crate::camera::Projection>], color: Color32) {
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
fn draw_path(painter: &Painter, points: &[Option<crate::camera::Projection>], stroke: Stroke) {
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
}
