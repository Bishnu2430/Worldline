//! Overlays: data drawn over the view, each from worldline-core's physics
//! and each switched on in the Overlays panel (step 3.3). See
//! `docs/physics/overlays.md`.
//! - **Models:** which model computes each body on screen, with a ring
//!   colored by whether it covers the body's situation.
//! - **Clock rates:** how much slower clocks at rest run, as a heatmap on
//!   the plane through what the camera looks at, square to the view.
//! - **Tides:** how the focused body is stretched and squeezed.
//! - **Embedding:** the shape of space around the focused body.
//! - **Waves:** the crests of gravitational waves from the focused pair.
//! - **Proper time:** the selected body's own clock beside a far-away
//!   one (step 3.4).

use std::f64::consts::{PI, TAU};

use eframe::egui::{Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use worldline_core::compact::{horizon_spin, is_black_hole};
use worldline_core::constants::{C, DAY};
use worldline_core::fields::{
    clock_slowdown, embedding_height, principal_axes, ruler_excess, tidal_tensor,
};
use worldline_core::gravitational_waves::{chirp_gm, frequency_then, lookback, strain_amplitude};
use worldline_core::gravity::kerr::boyer_lindquist_radius;
use worldline_core::kerr_light::{circular, photon_orbits};
use worldline_core::regime::Validity;
use worldline_core::{Body, DVec3};

use crate::camera::Camera;
use crate::catalogue::{length, significant};
use crate::simulation::{Computed, Simulation};
use crate::theme;
use crate::view::OnScreen;

/// Which overlays are on.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Overlays {
    pub models: bool,
    pub clocks: bool,
    pub tides: bool,
    pub embedding: bool,
    pub waves: bool,
    pub proper_time: bool,
}

/// What an overlay says, shown in the view's corner.
pub struct Readout {
    pub title: &'static str,
    pub color: Color32,
    pub lines: Vec<String>,
}

/// What the overlays draw on and from.
pub struct Scene<'a> {
    pub painter: &'a Painter,
    pub viewport: Rect,
    pub camera: &'a Camera,
    pub simulation: &'a Simulation,
    pub layout: &'a [OnScreen],
    pub focus: usize,
    pub selected: usize,
    pub pointer: Option<Pos2>,
}

/// Draws the overlays that are on, and returns what they say.
pub fn draw(scene: &Scene<'_>, overlays: Overlays) -> Vec<Readout> {
    let mut readouts = Vec::new();
    if overlays.clocks {
        readouts.push(clocks(scene));
    }
    if overlays.embedding {
        readouts.extend(embedding(scene));
    }
    if overlays.waves {
        readouts.extend(waves(scene));
    }
    if overlays.tides {
        readouts.extend(tides(scene));
    }
    if overlays.models {
        readouts.push(models(scene));
    }
    if overlays.proper_time {
        readouts.extend(proper_time(scene));
    }
    readouts
}

/// Shows the readouts stacked in the view's top-left corner.
pub fn draw_readouts(painter: &Painter, viewport: Rect, readouts: &[Readout]) {
    let mut top = viewport.top() + 10.0;
    for readout in readouts {
        let title = painter.layout_no_wrap(
            readout.title.to_string(),
            FontId::proportional(13.0),
            readout.color,
        );
        let body = painter.layout(
            readout.lines.join("\n"),
            FontId::proportional(12.0),
            Color32::from_gray(214),
            380.0,
        );
        let size = vec2(
            title.size().x.max(body.size().x) + 16.0,
            title.size().y + body.size().y + 14.0,
        );
        let rect = Rect::from_min_size(pos2(viewport.left() + 10.0, top), size);
        painter.rect_filled(rect, 4.0, Color32::from_black_alpha(175));
        let corner = rect.min + vec2(8.0, 6.0);
        painter.galley(corner, title.clone(), readout.color);
        painter.galley(
            corner + vec2(0.0, title.size().y + 2.0),
            body,
            Color32::WHITE,
        );
        top = rect.bottom() + 6.0;
    }
}

/// The bodies with mass in the region of body `near` (the solar system's
/// or the galactic center's), which shape the fields there: the regions
/// don't feel each other, and the galaxy's pull is left out.
fn massive(simulation: &Simulation, near: usize) -> (Vec<Body>, Vec<usize>) {
    let region = simulation.is_galactic(near);
    simulation
        .bodies
        .iter()
        .enumerate()
        .filter(|&(i, b)| b.gm > 0.0 && simulation.is_galactic(i) == region)
        .map(|(i, b)| (b.clone(), i))
        .unzip()
}

/// A short name for a body's model.
fn model_name(simulation: &Simulation, index: usize) -> &'static str {
    if simulation.held_by(index).is_some() {
        return "a black hole's spacetime";
    }
    match simulation.computed(index) {
        Computed::TopLevel => "relativistic N-body",
        Computed::MoonSystem => "its moon system",
        Computed::SmallMoonInDetail => "following the moons",
        Computed::SmallMoonOnMeanOrbit => "a mean orbit",
        Computed::Follower { .. } => "following the planets",
        Computed::GalacticStar => "following Sagittarius A*",
        Computed::Swarm => "the swarm",
    }
}

/// The model overlay: a ring around each body on screen, green where its
/// model covers its situation, amber where only approximately and red
/// where not (named), and a count of all of them.
fn models(scene: &Scene<'_>) -> Readout {
    let simulation = scene.simulation;
    let mut counts = [0; 3];
    let mut models: Vec<(&str, usize)> = Vec::new();
    let mut flagged = Vec::new();
    for item in scene.layout {
        let Some((regime, validity)) = simulation.validity(item.index) else {
            continue;
        };
        let name = model_name(simulation, item.index);
        match models.iter_mut().find(|(n, _)| *n == name) {
            Some((_, count)) => *count += 1,
            None => models.push((name, 1)),
        }
        let (slot, color, tag) = match validity {
            Validity::Within => (0, theme::WITHIN, ""),
            Validity::Approximate => (1, theme::APPROXIMATE, "approximate"),
            Validity::Beyond => (2, theme::BEYOND, "beyond its model"),
        };
        counts[slot] += 1;
        let center = item.projection.position;
        let width = if slot > 0 { 1.5 } else { 1.0 };
        scene.painter.circle_stroke(
            center,
            item.radius + 4.0,
            Stroke::new(width, color.gamma_multiply(0.8)),
        );
        if slot > 0 {
            scene.painter.text(
                center + vec2(item.radius + 7.0, -item.radius - 7.0),
                Align2::LEFT_BOTTOM,
                tag,
                FontId::proportional(11.0),
                color,
            );
            flagged.push((slot, item.index, regime));
        }
    }
    flagged.sort_by_key(|f| std::cmp::Reverse(f.0));
    models.sort_by_key(|m| std::cmp::Reverse(m.1));
    let mut lines = vec![
        format!(
            "On screen: {} within range, {} approximate, {} beyond their model",
            counts[0], counts[1], counts[2]
        ),
        models
            .iter()
            .map(|(name, count)| format!("{count} {name}"))
            .collect::<Vec<_>>()
            .join(" · "),
    ];
    for (slot, index, regime) in flagged.iter().take(4) {
        lines.push(format!(
            "{}: {}, GM/rc² = {:.1e}, v/c = {:.1e}",
            simulation.bodies[*index].name,
            if *slot == 2 { "beyond" } else { "approximate" },
            regime.epsilon,
            regime.beta
        ));
    }
    Readout {
        title: "Models",
        color: theme::ACCENT,
        lines,
    }
}

/// The heatmap's points, in screen points apart.
const CELL: f32 = 12.0;

/// The clock-rate overlay: how much more slowly a clock at rest runs than
/// one far away, over the plane through what the camera looks at, square
/// to the view. Colored on a log scale across the range in view, or a
/// straight one if that range is narrow; black where no clock can stay at
/// rest.
fn clocks(scene: &Scene<'_>) -> Readout {
    let (bodies, _) = massive(scene.simulation, scene.focus);
    let viewport = scene.viewport;
    let columns = (viewport.width() / CELL).ceil() as usize;
    let rows = (viewport.height() / CELL).ceil() as usize;
    let mut points = Vec::with_capacity((columns + 1) * (rows + 1));
    for j in 0..=rows {
        for i in 0..=columns {
            let at = pos2(
                (viewport.left() + i as f32 * CELL).min(viewport.right()),
                (viewport.top() + j as f32 * CELL).min(viewport.bottom()),
            );
            let point = scene.camera.point_in_view_plane(at, viewport);
            points.push((at, clock_slowdown(&bodies, point)));
        }
    }
    let (low, high) = points
        .iter()
        .filter_map(|p| p.1)
        .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)));
    let logarithmic = high > 10.0 * low && low > 0.0;
    let place = |s: f64| {
        if high <= low {
            0.5
        } else if logarithmic {
            (s / low).ln() / (high / low).ln()
        } else {
            (s - low) / (high - low)
        }
    };
    let mut mesh = Mesh::default();
    for &(at, slowdown) in &points {
        mesh.colored_vertex(
            at,
            slowdown.map_or(Color32::from_black_alpha(220), |s| heat(place(s))),
        );
    }
    let stride = (columns + 1) as u32;
    for j in 0..rows as u32 {
        for i in 0..columns as u32 {
            let k = j * stride + i;
            mesh.add_triangle(k, k + 1, k + stride);
            mesh.add_triangle(k + 1, k + stride + 1, k + stride);
        }
    }
    scene.painter.add(Shape::mesh(mesh));
    let mut lines = vec![
        format!(
            "Clocks at rest run slow, against one far away: from {} (blue) to {} (white), on a {} scale",
            significant(low),
            significant(high),
            if logarithmic { "log" } else { "straight" }
        ),
        format!(
            "On the plane through {}, square to the view. The Milky Way's own pull, the same across the solar system, is left out",
            scene.simulation.bodies[scene.focus].name
        ),
    ];
    if let Some(pointer) = scene.pointer.filter(|p| viewport.contains(*p)) {
        let point = scene.camera.point_in_view_plane(pointer, viewport);
        lines.push(match clock_slowdown(&bodies, point) {
            Some(s) => format!(
                "Under the pointer: slow by {}, {} a day",
                significant(s),
                duration(s * DAY)
            ),
            None => "Under the pointer: nothing can stay at rest here".into(),
        });
    }
    Readout {
        title: "Clock rates",
        color: heat(0.8),
        lines,
    }
}

/// A short span of time, from picoseconds up.
fn duration(seconds: f64) -> String {
    let (value, unit) = [
        (1.0, "s"),
        (1e-3, "ms"),
        (1e-6, "µs"),
        (1e-9, "ns"),
        (1e-12, "ps"),
    ]
    .into_iter()
    .find(|(scale, _)| seconds >= *scale)
    .unwrap_or((1e-12, "ps"));
    format!("{} {unit}", significant(seconds / value))
}

/// The heatmap's color at `t`, 0 to 1: dark blue through green and yellow
/// to white, more opaque as it warms.
fn heat(t: f64) -> Color32 {
    let stops: [(f64, [f64; 3]); 5] = [
        (0.0, [20.0, 40.0, 140.0]),
        (0.3, [30.0, 120.0, 200.0]),
        (0.55, [60.0, 190.0, 120.0]),
        (0.8, [240.0, 200.0, 60.0]),
        (1.0, [255.0, 250.0, 240.0]),
    ];
    let t = t.clamp(0.0, 1.0);
    let k = stops
        .iter()
        .rposition(|s| s.0 <= t)
        .unwrap_or(0)
        .min(stops.len() - 2);
    let (a, b) = (stops[k], stops[k + 1]);
    let f = (t - a.0) / (b.0 - a.0);
    let channel = |i: usize| (a.1[i] + (b.1[i] - a.1[i]) * f) as u8;
    let alpha = (70.0 + 90.0 * t) as u8;
    Color32::from_rgba_unmultiplied(channel(0), channel(1), channel(2), alpha)
}

/// The tide overlay. Around a body: arrows on a ring around it, the pull
/// of everything else there less its pull at the body's center (T δ, the
/// tidal tensor at the center), the pattern that raises tides. Around a
/// black hole: the stretch a small body on the ring would feel along the
/// line to the hole.
fn tides(scene: &Scene<'_>) -> Option<Readout> {
    let simulation = scene.simulation;
    let index = scene.focus;
    let body = &simulation.bodies[index];
    let item = scene.layout.iter().find(|i| i.index == index)?;
    let (bodies, indices) = massive(simulation, index);
    let skip = indices.iter().position(|&i| i == index);
    let hole = is_black_hole(body);
    let ring_points = item.radius + 30.0;
    let ring = f64::from(ring_points) / item.projection.points_per_meter;
    let (_, right, up) = scene.camera.basis();
    let color = Color32::from_rgb(120, 220, 230);
    let center = item.projection.position;
    let onscreen = |v: DVec3| vec2(v.dot(right) as f32, -v.dot(up) as f32);
    let at_center = tidal_tensor(&bodies, body.position, skip);
    let pulls: Vec<(Pos2, DVec3)> = (0..24)
        .map(|k| {
            let angle = TAU * f64::from(k) / 24.0;
            let offset = (right * angle.cos() + up * angle.sin()) * ring;
            let screen = center + onscreen(offset).normalized() * ring_points;
            let pull = if hole {
                // Along the stretch there, both ways.
                let [(stretch, axis), ..] =
                    principal_axes(tidal_tensor(&bodies, body.position + offset, None));
                axis * stretch
            } else {
                at_center * offset
            };
            (screen, pull)
        })
        .collect();
    let longest = pulls
        .iter()
        .map(|(_, p)| onscreen(*p).length())
        .fold(0.0, f32::max)
        .max(f32::MIN_POSITIVE);
    for (at, pull) in &pulls {
        let arrow = onscreen(*pull) / longest * 26.0;
        let stroke = Stroke::new(1.5, color);
        scene.painter.arrow(*at, arrow, stroke);
        if hole {
            scene.painter.arrow(*at, -arrow, stroke);
        }
    }
    let mut lines = Vec::new();
    if hole {
        let [(stretch, _), _, (squeeze, _)] =
            principal_axes(tidal_tensor(&bodies, body.position + right * ring, None));
        lines.push(format!(
            "{} from {}, a body 2 m long is pulled apart by {} m/s² ({} g) and squeezed across by {} m/s²",
            length(ring),
            body.name,
            significant(2.0 * stretch),
            significant(2.0 * stretch / 9.806_65),
            significant(-2.0 * squeeze)
        ));
    } else {
        let [(stretch, axis), _, (squeeze, _)] = principal_axes(at_center);
        // Who raises the stretch: the body whose 2GM/d³ is largest.
        let raiser = bodies
            .iter()
            .enumerate()
            .filter(|(k, _)| Some(*k) != skip)
            .max_by(|(_, a), (_, b)| {
                let tide = |x: &Body| x.gm / (x.position - body.position).length().powi(3);
                tide(a).total_cmp(&tide(b))
            })
            .map(|(_, b)| b.name.clone())
            .unwrap_or_default();
        let toward = scene.camera.basis().0.dot(axis).abs().acos().to_degrees();
        lines.push(format!(
            "{} is stretched by {} /s² along one axis (mostly by {raiser}) and squeezed by {} /s² across it",
            body.name,
            significant(stretch),
            significant(-squeeze)
        ));
        if body.radius > 0.0 {
            lines.push(format!(
                "At its surface: {} m/s² outward along it, {} m/s² inward across",
                significant(stretch * body.radius),
                significant(-squeeze * body.radius)
            ));
        }
        lines.push(format!(
            "The stretch axis is {toward:.0}° from the line of sight; arrows show the pull on the ring, less the pull at the center"
        ));
    }
    Some(Readout {
        title: "Tides",
        color,
        lines,
    })
}

/// The embedding overlay: the focused body's equatorial plane drawn as a
/// surface in flat space whose distances are true (Flamm's paraboloid, and
/// a cap inside a body), sinking below the ecliptic-parallel plane through
/// the body toward its center. Depths too small to see are exaggerated by
/// a stated power of ten, the largest that keeps the depth under 40% of the
/// width drawn.
fn embedding(scene: &Scene<'_>) -> Option<Readout> {
    let body = &scene.simulation.bodies[scene.focus];
    let hole = is_black_hole(body);
    if body.gm <= 0.0 || (body.radius <= 0.0 && !hole) {
        return None;
    }
    let rs = 2.0 * body.gm / (C * C);
    let (inner, outer) = if hole {
        (rs, 10.0 * rs)
    } else {
        (0.0, 6.0 * body.radius)
    };
    let height = |r: f64| embedding_height(body.gm, body.radius, r);
    let depth = height(outer) - height(inner);
    let exaggeration = if depth >= 0.3 * outer {
        1.0
    } else {
        10f64.powf((0.4 * outer / depth).log10().floor())
    };
    let point = |r: f64, angle: f64| {
        body.position + DVec3::new(r * angle.cos(), r * angle.sin(), 0.0)
            - DVec3::Z * (exaggeration * (height(outer) - height(r)))
    };
    let color = Color32::from_rgba_unmultiplied(150, 230, 160, 150);
    let line = |points: &[DVec3], stroke: Stroke| {
        let projected: Vec<Option<Pos2>> = points
            .iter()
            .map(|p| scene.camera.project(*p, scene.viewport).map(|q| q.position))
            .collect();
        for pair in projected.windows(2) {
            if let [Some(a), Some(b)] = pair {
                scene.painter.line_segment([*a, *b], stroke);
            }
        }
    };
    let surface = if hole { rs } else { body.radius };
    for k in 0..=12 {
        let r = inner + (outer - inner) * f64::from(k) / 12.0;
        if r <= 0.0 {
            continue;
        }
        let ring: Vec<DVec3> = (0..=72)
            .map(|j| point(r, TAU * f64::from(j) / 72.0))
            .collect();
        line(&ring, Stroke::new(1.0, color));
    }
    let edge: Vec<DVec3> = (0..=72)
        .map(|j| point(surface, TAU * f64::from(j) / 72.0))
        .collect();
    line(&edge, Stroke::new(2.0, color));
    for j in 0..24 {
        let angle = TAU * f64::from(j) / 24.0;
        let spoke: Vec<DVec3> = (0..=40)
            .map(|k| point(inner + (outer - inner) * f64::from(k) / 40.0, angle))
            .collect();
        line(&spoke, Stroke::new(1.0, color));
    }
    let mut lines = vec![format!(
        "Space around {}: its equatorial plane as a surface in flat space with true distances (Flamm 1916){}",
        body.name,
        if exaggeration > 1.0 {
            format!(", depth drawn × {}", significant(exaggeration))
        } else {
            String::new()
        }
    )];
    if hole {
        let ruler = rs + ruler_excess(body.gm, rs, 2.0 * rs);
        lines.push(format!(
            "From the horizon out to twice its radius, {} in r, a ruler measures {} ({:.4} r_s)",
            length(rs),
            length(ruler),
            ruler / rs
        ));
        lines.push("Drawn as for a hole that doesn't spin".into());
    } else {
        let r = body.radius;
        lines.push(format!(
            "From its surface out to twice its radius, a ruler measures {} more than the {} of flat space",
            length(ruler_excess(body.gm, r, 2.0 * r)),
            length(r)
        ));
        lines.push(format!(
            "Its center sits {} below its surface (inside, as a body of even density)",
            length(height(r) - height(0.0))
        ));
    }
    Some(Readout {
        title: "Embedding diagram",
        color,
        lines,
    })
}

/// The proper-time overlay: a dial beside the selected body with two hands
/// turning once in the same time, its own clock's (bright) and a far-away
/// clock's (faint), from when its clock started. The time for a turn is a
/// power of ten of seconds near ten real seconds' worth at the current
/// speed, so the far-away hand turns about once every ten seconds.
fn proper_time(scene: &Scene<'_>) -> Option<Readout> {
    let simulation = scene.simulation;
    let body = &simulation.bodies[scene.selected];
    let clock = simulation.clocks.get(&body.name)?;
    let item = scene.layout.iter().find(|i| i.index == scene.selected)?;
    let turn = 10f64.powf((10.0 * simulation.speed.max(1.0)).log10().round());
    let center = item.projection.position + vec2(item.radius + 24.0, -item.radius - 24.0);
    let radius = 15.0;
    let color = Color32::from_rgb(255, 214, 120);
    scene
        .painter
        .circle_filled(center, radius, Color32::from_black_alpha(160));
    scene
        .painter
        .circle_stroke(center, radius, Stroke::new(1.0, color.gamma_multiply(0.6)));
    let hand = |time: f64, length: f32, stroke: Stroke| {
        let angle = (time / turn).fract() * TAU - PI / 2.0;
        let tip = center + vec2(angle.cos() as f32, angle.sin() as f32) * length;
        scene.painter.line_segment([center, tip], stroke);
    };
    hand(
        clock.elapsed(),
        radius - 2.0,
        Stroke::new(1.0, Color32::from_gray(150)),
    );
    hand(clock.shown(), radius - 3.0, Stroke::new(2.0, color));
    let against = match &clock.frame {
        Some(hole) => format!("a clock far from {hole}, at rest with it"),
        None => "a clock far away".to_string(),
    };
    let mut lines = vec![
        format!(
            "{}'s own clock (bright hand) against {against} (faint): one turn is {}",
            body.name,
            span(turn)
        ),
        if clock.slowdown < 1e-3 {
            format!("Now it runs slow by {}", significant(clock.slowdown))
        } else {
            format!(
                "Now it runs at {:.6} of the far-away rate",
                1.0 - clock.slowdown
            )
        },
    ];
    if clock.elapsed() > 0.0 {
        lines.push(format!(
            "Since it started, {} behind in {}: {:.10} as fast on average",
            span(clock.behind),
            span(clock.elapsed()),
            clock.shown() / clock.elapsed()
        ));
    }
    // What a circular orbit there would show, for comparison.
    if let Some(h) = simulation.held_by(scene.selected) {
        let hole = &simulation.bodies[h];
        let m = hole.gm / (C * C);
        let spin = horizon_spin(hole.gm, hole.radius);
        let r = boyer_lindquist_radius(hole.gm, spin, body.position - hole.position) / m;
        if r > photon_orbits(spin).0 {
            let (_, ut) = circular(spin, r);
            lines.push(format!(
                "A circular orbit at its radius, {r:.2} GM/c², turning with the hole, would run at {:.6}",
                1.0 / ut
            ));
        }
    }
    Some(Readout {
        title: "Proper time",
        color,
        lines,
    })
}

/// A span of time, from picoseconds to years.
fn span(seconds: f64) -> String {
    let units = [
        (365.25 * DAY, "years"),
        (DAY, "days"),
        (3600.0, "h"),
        (60.0, "min"),
    ];
    match units.into_iter().find(|(scale, _)| seconds >= *scale) {
        Some((scale, unit)) => format!("{} {unit}", significant(seconds / scale)),
        None => duration(seconds),
    }
}

/// The wave overlay: the crests of the gravitational waves the focused
/// pair sends out in its orbit's plane, two arms spiralling out at the
/// speed of light, each a half turn of the orbit apart (circular orbit,
/// leading order). Drawn fainter as 1/R, the waves' own falloff, and not
/// within half a wavelength, where they aren't yet waves.
fn waves(scene: &Scene<'_>) -> Option<Readout> {
    let simulation = scene.simulation;
    let pair = simulation.gravitational_waves(scene.focus)?;
    let (a, b) = (
        &simulation.bodies[scene.focus],
        &simulation.bodies[pair.partner],
    );
    let total = a.gm + b.gm;
    let center = (a.position * a.gm + b.position * b.gm) / total;
    let (r, v) = (a.position - b.position, a.velocity - b.velocity);
    let spin = r.cross(v);
    let axis = spin.normalize();
    let frequency = spin.length() / r.length_squared() / PI;
    let chirp = chirp_gm(a.gm, b.gm);
    let wavelength = C / frequency;
    let first = r.normalize();
    let second = axis.cross(first);
    let reach = 2.0 * (scene.camera.eye() - center).length();
    let color = Color32::from_rgb(230, 150, 240);
    for arm in 0..2 {
        let mut previous: Option<(Pos2, f64)> = None;
        // Half a turn of the orbit back per wavelength; up to 400 crests.
        for step in 0..(400 * 64) {
            let behind = PI * f64::from(step) / 64.0;
            let radius = C * lookback(chirp, frequency, behind);
            if radius > reach {
                break;
            }
            if radius < 0.5 * wavelength {
                previous = None;
                continue;
            }
            let angle = PI * f64::from(arm) - behind;
            let place = center + (first * angle.cos() + second * angle.sin()) * radius;
            let here = scene
                .camera
                .project(place, scene.viewport)
                .map(|p| p.position);
            if let (Some((from, _)), Some(to)) = (previous, here) {
                let fade = (0.5 * wavelength / radius).max(0.12);
                let stroke = Stroke::new(1.5, color.gamma_multiply(fade as f32));
                scene.painter.line_segment([from, to], stroke);
            }
            previous = here.map(|p| (p, radius));
        }
    }
    let observer = scene.camera.eye() - center;
    let distance = observer.length();
    let inclination = axis.dot(observer / distance).clamp(-1.0, 1.0).acos();
    let mut lines = vec![format!(
        "From {} and {}: {} Hz now, {} long, crests spiralling out at the speed of light",
        a.name,
        b.name,
        significant(frequency),
        length(wavelength)
    )];
    if distance > wavelength {
        let then = frequency_then(chirp, frequency, distance / C);
        let (plus, cross) = strain_amplitude(chirp, then, distance, inclination);
        lines.push(format!(
            "At the camera, {} away, {:.0}° from the orbit's axis: strain h+ = {:.2e}, h× = {:.2e}",
            length(distance),
            inclination.to_degrees(),
            plus,
            cross.abs()
        ));
    } else {
        lines.push("The camera is within a wavelength, where they aren't yet waves".into());
    }
    lines.push("As from a circular orbit at its current frequency (leading order)".into());
    Some(Readout {
        title: "Gravitational waves",
        color,
        lines,
    })
}

#[cfg(test)]
mod tests {
    use eframe::egui::{self, LayerId};
    use worldline_core::constants::AU;

    use super::*;
    use crate::view::{self, Attention};

    /// How many shapes the overlays draw on a 1,000 × 700 view of `camera`,
    /// following `focus`, and what they say.
    fn drawn(
        simulation: &Simulation,
        camera: &Camera,
        focus: usize,
        overlays: Overlays,
    ) -> (usize, Vec<Readout>) {
        let ctx = egui::Context::default();
        let mut readouts = Vec::new();
        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
            let viewport = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
            let painter = Painter::new(ui.ctx().clone(), LayerId::background(), viewport);
            let attention = Attention {
                focus,
                selected: focus,
                pointer: None,
                lensed: None,
            };
            let layout = view::layout(camera, viewport, simulation, false, attention);
            let scene = Scene {
                painter: &painter,
                viewport,
                camera,
                simulation,
                layout: &layout,
                focus,
                selected: focus,
                pointer: Some(pos2(600.0, 300.0)),
            };
            readouts = draw(&scene, overlays);
        });
        // No window takes the font texture.
        output.textures_delta.clear();
        (output.shapes.len(), readouts)
    }

    fn looking_at(simulation: &Simulation, index: usize, radii: f64) -> Camera {
        let body = &simulation.bodies[index];
        Camera {
            target: body.position,
            yaw: 0.3,
            pitch: 0.5,
            distance: radii * body.radius,
            fov_y: 0.8,
            standing: None,
        }
    }

    #[test]
    fn each_overlay_draws_only_when_on_and_says_what_it_shows() {
        let mut sim = Simulation::solar_system(DAY);
        let earth = sim.index_of("Earth").expect("Earth");
        let camera = looking_at(&sim, earth, 60.0);
        let (shapes, readouts) = drawn(&sim, &camera, earth, Overlays::default());
        assert_eq!((shapes, readouts.len()), (0, 0));
        // GW150914's holes, 30 AU above the Sun, at a 20 Hz wave.
        let find = |name: &str| worldline_data::notable_object(name).expect("in the catalog");
        let (a, b) = worldline_core::gravity::circular_pair(
            find("GW150914 heavier hole").body(),
            find("GW150914 lighter hole").body(),
            20.0,
        );
        let above = DVec3::new(0.0, 0.0, 30.0 * AU);
        for body in [a, b] {
            let at = body.position + above;
            sim.add_body(body.at(at), "inspiral:GW150914");
        }
        let hole = sim.index_of("GW150914 heavier hole").expect("added");
        for (name, overlays, focus, radii) in [
            (
                "models",
                Overlays {
                    models: true,
                    ..Default::default()
                },
                earth,
                60.0,
            ),
            (
                "clocks",
                Overlays {
                    clocks: true,
                    ..Default::default()
                },
                earth,
                60.0,
            ),
            (
                "tides",
                Overlays {
                    tides: true,
                    ..Default::default()
                },
                earth,
                60.0,
            ),
            (
                "embedding",
                Overlays {
                    embedding: true,
                    ..Default::default()
                },
                earth,
                60.0,
            ),
            (
                "waves",
                Overlays {
                    waves: true,
                    ..Default::default()
                },
                hole,
                600.0,
            ),
            (
                "proper time",
                Overlays {
                    proper_time: true,
                    ..Default::default()
                },
                earth,
                60.0,
            ),
        ] {
            let camera = looking_at(&sim, focus, radii);
            let (shapes, readouts) = drawn(&sim, &camera, focus, overlays);
            println!(
                "{name}: {shapes} shapes; {}: {}",
                readouts[0].title,
                readouts[0].lines.join(" | ")
            );
            assert!(shapes > 0 && readouts.len() == 1, "{name}");
            // Bodies far from the pair are measured from its center of
            // mass, not from one hole whirling at 0.1c: Earth's view has
            // nothing beyond or approximate.
            if name == "models" {
                assert!(readouts[0].lines[0].contains(" 0 approximate, 0 beyond"));
            }
            let (none, _) = drawn(&sim, &camera, focus, Overlays::default());
            assert_eq!(none, 0);
        }
    }

    #[test]
    fn the_readouts_carry_the_hand_calculated_numbers() {
        // Earth's center sits √(r_s R)/2 = 118.9 m below its surface, and a
        // ruler from its surface to twice its radius reads (r_s/2) ln 2 =
        // 3.07 mm long (validation_overlays.rs works these out); at its
        // surface the Moon and the Sun stretch it by about 1.10 and 0.51
        // × 10⁻⁶ m/s², more when they line up, as at the new moon of 30
        // December 2024, two days before the snapshot.
        let sim = Simulation::solar_system(DAY);
        let earth = sim.index_of("Earth").expect("Earth");
        let camera = looking_at(&sim, earth, 60.0);
        let overlays = Overlays {
            embedding: true,
            tides: true,
            ..Default::default()
        };
        let (_, readouts) = drawn(&sim, &camera, earth, overlays);
        let said = |title: &str| {
            readouts
                .iter()
                .find(|r| r.title == title)
                .map(|r| r.lines.join(" "))
                .unwrap_or_default()
        };
        let embedding = said("Embedding diagram");
        println!("{embedding}");
        assert!(embedding.contains("118.9 m below") && embedding.contains("0.003074 m more"));
        let tides = said("Tides");
        println!("{tides}");
        let surface: f64 = tides
            .split("At its surface: ")
            .nth(1)
            .and_then(|s| s.split(' ').next())
            .and_then(|s| s.parse().ok())
            .expect("the surface stretch");
        assert!((1.5e-6..1.7e-6).contains(&surface), "{surface}");
    }
}
