//! A neutron star up close (step 8.5b), drawn as its own gravity shows it.
//!
//! Light leaving a neutron star climbs out of gravity strong enough to bend
//! it far round: a ray grazing the surface leaves it with impact parameter
//! R/√(1 − 2GM/c²R), so the star looks bigger than it is, and light from
//! behind it reaches the camera, so more than half of its surface shows.
//! Each point of a grid on the surface is drawn where its light arrives,
//! from the exact bending for the camera's own distance
//! (`worldline_core::lensing::surface_angle`); a point whose light can't
//! reach the camera is hidden. A dashed circle shows how big the star would
//! look without gravity.
//!
//! What's measured and what's illustration is listed in the inspector:
//! the grid is drawn only to show the bending, the color is white where
//! the temperature isn't measured, and a pulsar's radio beams are an
//! illustration (their tilt, width and the spin axis's direction are
//! chosen; their sweep, as many times a second as the pulsar turns, is
//! measured). See `docs/physics/lensing.md`.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use eframe::egui::{Align2, Color32, FontId, Painter, Pos2, Rect, Shape, Stroke, vec2};
use worldline_core::DVec3;
use worldline_core::lensing::{arrival_angle, grazing_impact, surface_angle};
use worldline_data::ObjectKind;

use crate::camera::Camera;
use crate::simulation::Simulation;

/// The smallest a neutron star is drawn up close: its true radius on
/// screen, in points. Smaller, it's a dot like anything else.
pub const CLOSE: f32 = 6.0;

/// A pulsar's beams' tilt from its spin axis and half-width, rad: an
/// illustration, not measured for the pulsars here.
const BEAM_TILT: f64 = 45.0 * PI / 180.0;
const BEAM_WIDTH: f64 = 10.0 * PI / 180.0;
/// How far out the beams are drawn, in the star's radii: an illustration.
const BEAM_LENGTH: f64 = 8.0;

/// Whether body `index` is a neutron star drawn up close here rather than
/// as a globe or a dot, its true radius `radius` points on screen.
pub fn drawn_here(simulation: &Simulation, index: usize, radius: f32) -> bool {
    radius >= CLOSE && simulation.kind(index) == Some(ObjectKind::NeutronStar)
}

/// How a star bends its own light toward a camera `distance` m from it:
/// for rays from its line of sight's to its grazing one, their impact
/// parameter (m) and how far round the star each carries light (rad).
pub struct Bending {
    table: Vec<(f64, f64)>,
    gm: f64,
    distance: f64,
}

impl Bending {
    /// For a star of `gm` and `radius` (m) seen from `distance` m; `None`
    /// from inside it, or for a body not bigger than its photon sphere.
    pub fn new(gm: f64, radius: f64, distance: f64) -> Option<Self> {
        if distance <= radius
            || 1.0 - 2.0 * gm / (worldline_core::constants::C.powi(2) * radius) <= 0.0
        {
            return None;
        }
        let grazing = grazing_impact(gm, radius);
        // Denser toward the grazing ray, where ψ changes fastest.
        let table = (0..=160)
            .map(|k| {
                let b = grazing * (FRAC_PI_2 * f64::from(k) / 160.0).sin();
                surface_angle(gm, radius, distance, b).map(|psi| (b, psi))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self {
            table,
            gm,
            distance,
        })
    }

    /// How far round from the line of sight the camera sees: the grazing
    /// ray's ψ, rad.
    pub fn horizon(&self) -> f64 {
        self.table.last().map_or(0.0, |t| t.1)
    }

    /// The share of the surface the camera sees: all of it once the
    /// grazing ray has come round 180° or more (closer than 3.5 GM/c² or
    /// so), some of it twice.
    pub fn shown(&self) -> f64 {
        (1.0 - self.horizon().min(std::f64::consts::PI).cos()) / 2.0
    }

    /// The impact parameter of light from a point `psi` round from the
    /// line of sight (m), if it reaches the camera, by interpolation.
    pub fn impact(&self, psi: f64) -> Option<f64> {
        if psi > self.horizon() {
            return None;
        }
        let k = self.table.partition_point(|t| t.1 < psi).max(1);
        let ((b0, p0), (b1, p1)) = (self.table[k - 1], self.table[k]);
        Some(b0 + (b1 - b0) * ((psi - p0) / (p1 - p0)).clamp(0.0, 1.0))
    }

    /// The angle from the star's center at which the camera sees a ray of
    /// impact parameter `b` arrive, rad.
    pub fn arrival(&self, b: f64) -> f64 {
        arrival_angle(self.gm, self.distance, b)
    }

    /// The grazing ray's impact parameter, m: the star's apparent size.
    pub fn grazing(&self) -> f64 {
        self.table.last().map_or(0.0, |t| t.0)
    }
}

/// Two unit vectors square to `axis` and to each other.
fn square_to(axis: DVec3) -> (DVec3, DVec3) {
    let first = axis.any_orthonormal_vector();
    (first, axis.cross(first))
}

/// A pulsar's spin frequency, Hz, if it's one the data times: the binary
/// orbits list each pair's timed pulsar by the pair's name, the pulsar
/// named as the pair (PSR B1913+16) or as its A (PSR J0737-3039A).
pub fn spin_frequency(simulation: &Simulation, index: usize) -> Option<f64> {
    use std::sync::OnceLock;
    static ORBITS: OnceLock<Vec<worldline_data::BinaryOrbit>> = OnceLock::new();
    let name = simulation.catalog_object(index)?.name;
    ORBITS
        .get_or_init(worldline_data::binary_orbits)
        .iter()
        .find(|o| o.system == name || format!("{}A", o.system) == name)
        .and_then(|o| o.spin_frequency)
}

/// The axis a neutron star is drawn spinning around: its orbit's, if it's
/// bound to something, else ecliptic north. An illustration: the spin
/// axes of the pulsars here aren't in the data.
pub fn spin_axis(simulation: &Simulation, index: usize) -> DVec3 {
    simulation.orbit_axis(index).unwrap_or(DVec3::Z)
}

/// Draws neutron star `index` up close: its disk as big as it looks, a
/// grid on its surface where its bent light shows it, its true size dashed,
/// and a pulsar's beams. `frame_seconds` is how long a frame takes, to
/// tell whether the beams turn slowly enough to draw as they are.
pub fn draw(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    simulation: &Simulation,
    index: usize,
    frame_seconds: f64,
) {
    let body = &simulation.bodies[index];
    let to_eye = camera.eye() - body.position;
    let distance = to_eye.length();
    let Some(bending) = Bending::new(body.gm, body.radius, distance) else {
        return;
    };
    let Some(center) = camera.project(body.position, viewport) else {
        return;
    };
    let toward = to_eye / distance;
    let (_, right, up) = camera.basis();
    let focal = f64::from(viewport.height()) / 2.0 / (camera.fov_y / 2.0).tan();
    // Where on screen a point of the surface, `n` from the center, shows.
    let on_screen = |n: DVec3| -> Option<Pos2> {
        let psi = n.dot(toward).clamp(-1.0, 1.0).acos();
        let b = bending.impact(psi)?;
        let side = n - toward * n.dot(toward);
        let along = side.length();
        let (dx, dy) = if along < 1e-12 {
            (0.0, 0.0)
        } else {
            (side.dot(right) / along, -side.dot(up) / along)
        };
        let r = focal * bending.arrival(b).tan();
        Some(center.position + vec2((dx * r) as f32, (dy * r) as f32))
    };
    let disk = (focal * bending.arrival(bending.grazing()).tan()) as f32;
    let flat = (focal * (body.radius / distance).asin().tan()) as f32;

    let axis = spin_axis(simulation, index);
    let spin = spin_frequency(simulation, index);
    let turns_per_frame = if simulation.paused {
        0.0
    } else {
        spin.unwrap_or(0.0) * simulation.speed * frame_seconds
    };
    let phase = spin.map_or(0.0, |f| TAU * (f * simulation.time()).fract());
    let beams = Beams {
        center: body.position,
        radius: body.radius,
        axis,
        phase,
        swept: turns_per_frame > 0.1,
    };
    // The beam pointing away goes behind the star, the other in front.
    if spin.is_some() {
        beams.draw(painter, viewport, camera, toward, false);
    }

    // A blackbody's color at its temperature if measured; otherwise white
    // (its dot, far off, keeps the color of its kind).
    let color = simulation
        .temperature(index)
        .map_or(Color32::from_gray(238), |t| {
            crate::view::blackbody_color(t.kelvin)
        });
    painter.circle_filled(center.position, disk, color);
    // The grid, every 30°, around the spin axis.
    let (e1, e2) = square_to(axis);
    let grid = Stroke::new(1.0, Color32::from_black_alpha(110));
    let line = |points: Vec<Option<Pos2>>| {
        for run in points.split(Option::is_none) {
            let run: Vec<Pos2> = run.iter().flatten().copied().collect();
            if run.len() > 1 {
                painter.add(Shape::line(run, grid));
            }
        }
    };
    let point = |latitude: f64, longitude: f64| {
        let (sl, cl) = latitude.sin_cos();
        (e1 * longitude.cos() + e2 * longitude.sin()) * cl + axis * sl
    };
    for k in 0..12 {
        let longitude = f64::from(k) * PI / 6.0;
        line(
            (0..=90)
                .map(|j| on_screen(point(-FRAC_PI_2 + PI * f64::from(j) / 90.0, longitude)))
                .collect(),
        );
    }
    for k in -2..=2 {
        let latitude = f64::from(k) * PI / 6.0;
        line(
            (0..=180)
                .map(|j| on_screen(point(latitude, TAU * f64::from(j) / 180.0)))
                .collect(),
        );
    }
    // Its true size, dashed: how big it would look without gravity. It
    // falls inside the disk, so it's drawn dark against it.
    if flat > 12.0 {
        let ring: Vec<Pos2> = (0..=120)
            .map(|k| {
                let a = TAU * f64::from(k) / 120.0;
                center.position + vec2(flat * a.cos() as f32, flat * a.sin() as f32)
            })
            .collect();
        painter.add(Shape::dashed_line(
            &ring,
            Stroke::new(1.5, Color32::from_rgba_unmultiplied(200, 60, 40, 220)),
            6.0,
            4.0,
        ));
        if disk > 60.0 {
            painter.text(
                center.position + vec2(0.0, disk + 6.0),
                Align2::CENTER_TOP,
                format!(
                    "{:.0}% of its surface in view, {:.2} times as big as it is (dashed): its gravity bends its light",
                    100.0 * bending.shown(),
                    bending.grazing() / body.radius
                ),
                FontId::proportional(11.5),
                Color32::from_white_alpha(190),
            );
        }
    }
    if spin.is_some() {
        beams.draw(painter, viewport, camera, toward, true);
    }
}

/// A pulsar's two radio beams, from its magnetic poles: an illustration.
struct Beams {
    center: DVec3,
    radius: f64,
    axis: DVec3,
    /// How far it has turned, rad.
    phase: f64,
    /// Turning too fast to draw where they point: the cone they sweep is
    /// drawn instead.
    swept: bool,
}

impl Beams {
    /// The magnetic axis at turn `phase`.
    fn magnetic(&self, phase: f64) -> DVec3 {
        let (e1, e2) = square_to(self.axis);
        self.axis * BEAM_TILT.cos() + (e1 * phase.cos() + e2 * phase.sin()) * BEAM_TILT.sin()
    }

    /// Draws the beams on the camera's side of the star (`front`) or the
    /// far side.
    fn draw(&self, painter: &Painter, viewport: Rect, camera: &Camera, toward: DVec3, front: bool) {
        let fill = Color32::from_rgba_unmultiplied(150, 200, 255, 38);
        let edge = Stroke::new(1.0, Color32::from_rgba_unmultiplied(150, 200, 255, 120));
        let screen = |x: DVec3| camera.project(x, viewport).map(|p| p.position);
        let length = BEAM_LENGTH * self.radius;
        for pole in [1.0, -1.0] {
            if self.swept {
                // The cone the beam's middle sweeps, round the spin axis.
                let facing = pole * self.axis.dot(toward) > 0.0;
                if facing != front {
                    continue;
                }
                let rim: Vec<Pos2> = (0..=72)
                    .filter_map(|k| {
                        let m = self.magnetic(TAU * f64::from(k) / 72.0) * pole;
                        screen(self.center + m * length)
                    })
                    .collect();
                let width = camera
                    .project(self.center + self.axis * pole * length, viewport)
                    .map_or(2.0, |p| {
                        (length * BEAM_WIDTH.tan() * p.points_per_meter) as f32
                    });
                painter.add(Shape::line(
                    rim.clone(),
                    Stroke::new(width.clamp(1.0, 40.0), fill),
                ));
                for k in 0..8 {
                    let m = self.magnetic(TAU * f64::from(k) / 8.0) * pole;
                    if let (Some(a), Some(b)) = (
                        screen(self.center + m * self.radius),
                        screen(self.center + m * length),
                    ) {
                        painter.line_segment([a, b], Stroke::new(1.0, fill));
                    }
                }
                if pole > 0.0
                    && let Some(top) = rim.iter().min_by(|a, b| a.y.total_cmp(&b.y))
                {
                    painter.text(
                        *top - vec2(0.0, 8.0),
                        Align2::CENTER_BOTTOM,
                        "radio beams sweep these cones each turn (illustration)",
                        FontId::proportional(11.5),
                        Color32::from_rgba_unmultiplied(170, 210, 255, 200),
                    );
                }
            } else {
                let m = self.magnetic(self.phase) * pole;
                if (m.dot(toward) > 0.0) != front {
                    continue;
                }
                let (u, v) = square_to(m);
                let base = self.center + m * length;
                let spread = length * BEAM_WIDTH.tan();
                let mut points: Vec<Pos2> = (0..32)
                    .filter_map(|k| {
                        let a = TAU * f64::from(k) / 32.0;
                        screen(base + (u * a.cos() + v * a.sin()) * spread)
                    })
                    .collect();
                points.extend(screen(self.center + m * self.radius));
                let hull = convex_hull(points);
                if hull.len() > 2 {
                    painter.add(Shape::convex_polygon(hull, fill, edge));
                }
            }
        }
    }
}

/// The convex hull of `points`, counterclockwise (Andrew's monotone chain).
fn convex_hull(mut points: Vec<Pos2>) -> Vec<Pos2> {
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    if points.len() < 3 {
        return points;
    }
    let cross = |o: Pos2, a: Pos2, b: Pos2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut hull: Vec<Pos2> = Vec::with_capacity(points.len() * 2);
    for pass in 0..2 {
        let start = hull.len();
        let ordered: Box<dyn Iterator<Item = &Pos2>> = if pass == 0 {
            Box::new(points.iter())
        } else {
            Box::new(points.iter().rev())
        };
        for &p in ordered {
            while hull.len() >= start + 2
                && cross(hull[hull.len() - 2], hull[hull.len() - 1], p) <= 0.0
            {
                hull.pop();
            }
            hull.push(p);
        }
        hull.pop();
    }
    hull
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::constants::GM_SUN;

    #[test]
    fn the_point_facing_the_camera_is_the_middle_and_the_horizon_is_the_edge() {
        // The Hulse–Taylor pulsar seen from 100 km: the point straight
        // toward the camera arrives along the line of sight, and the
        // farthest point it sees arrives at the disk's edge.
        let (gm, radius) = (1.438 * GM_SUN, 11.9e3);
        let bending = Bending::new(gm, radius, 100e3).expect("outside it");
        assert_eq!(bending.impact(0.0), Some(0.0));
        let edge = bending.impact(bending.horizon()).expect("the grazing ray");
        assert!((edge / bending.grazing() - 1.0).abs() < 1e-12);
        assert!(bending.impact(bending.horizon() + 1e-6).is_none());
        // In between, a point is drawn where the exact bending puts it: the
        // impact parameter that sets where, against the exact one (by
        // bisection), within 10⁻³ of the disk's radius, a tenth of a point
        // on a disk 100 points across. Near the edge ψ changes fast with b,
        // so there the interpolation's ψ can be off more, but not where the
        // point lands.
        for psi in [0.3, 0.9, 1.5, 1.9, 1.98] {
            let b = bending.impact(psi).expect("in view");
            let (mut low, mut high) = (0.0, bending.grazing());
            for _ in 0..60 {
                let mid = 0.5 * (low + high);
                if surface_angle(gm, radius, 100e3, mid).expect("on the surface") < psi {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let off = (b - low).abs() / bending.grazing();
            println!("ψ = {psi}: b off by {off:.1e} of the disk");
            assert!(off < 1e-3, "{psi}: {off}");
        }
        assert!(bending.shown() > 0.5);
    }

    #[test]
    fn the_hull_of_a_square_and_its_middle_is_the_square() {
        let points = vec![
            Pos2::new(0.0, 0.0),
            Pos2::new(1.0, 0.0),
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 1.0),
            Pos2::new(0.5, 0.5),
        ];
        assert_eq!(convex_hull(points).len(), 4);
    }
}
