//! The sandbox tools: placing new bodies from the catalogue and launching
//! them with a drag, and saving and loading. See `docs/app.md`.

use std::path::PathBuf;

use eframe::egui::{Align2, Color32, FontId, Painter, Rect, Stroke};
use worldline_core::Body;
use worldline_core::DVec3;
use worldline_core::compact::{horizon_spin, is_black_hole};
use worldline_core::constants::C;
use worldline_core::gravity::kerr::{boyer_lindquist_radius, innermost_stable_orbit};
use worldline_core::gravity::moves_in_spacetime_of;
use worldline_core::orbit::osculating_orbit;

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::catalogue::Entry;
use crate::simulation::Simulation;

/// The color of bodies added in the sandbox.
pub const SANDBOX_COLOR: Color32 = Color32::from_rgb(255, 136, 196);

/// A body being placed: what, where, the body it will orbit if not
/// dragged, and how far the drag has pulled.
#[derive(Debug, Clone, Copy)]
pub struct Launch {
    pub entry: &'static Entry,
    pub position: DVec3,
    /// Index of the body it orbits by default.
    pub around: usize,
    /// Where the drag has reached, on the same plane as `position`.
    pub drag: DVec3,
    /// Start at rest relative to `around` (it falls straight in), instead
    /// of circling it.
    pub at_rest: bool,
}

impl Launch {
    /// Its velocity. Placed without a drag, it circles `around`:
    /// prograde, in the ecliptic, at √(G(M + m)/r) relative to it, which
    /// for two bodies is exactly circular; or, `at_rest`, it starts still
    /// relative to it. Dragging adds velocity along the drag: one circular
    /// speed for every quarter of the camera's distance.
    pub fn velocity(&self, simulation: &Simulation, camera_distance: f64, gm: f64) -> DVec3 {
        let center = &simulation.bodies[self.around];
        let r = self.position - center.position;
        let mu = center.gm + gm;
        let m = mu / (C * C);
        // A body a black hole holds circles in its exact spacetime, at
        // Ω = 1/(r^(3/2) + a) in units of GM/c² and c (Boyer–Lindquist r;
        // prograde, the hole's spin being along z), moving at Ω ẑ × r.
        // Without spin that is √(G(M + m)/r), as for Newton. Inside the
        // innermost stable orbit no circular orbit lasts: it starts at rest
        // and falls in. Launches stay below half the speed of light.
        let held = is_black_hole(center) && moves_in_spacetime_of(&Body::new("", gm, 0.0), center);
        let spin = if held {
            horizon_spin(center.gm, center.radius) * center.gm / mu
        } else {
            0.0
        };
        let radius = boyer_lindquist_radius(mu, spin, r) / m;
        let circular = if !self.at_rest && radius > innermost_stable_orbit(spin, true) {
            DVec3::Z.cross(r) * (C / m / (radius.powf(1.5) + spin))
        } else {
            DVec3::ZERO
        };
        let scale = (mu / r.length()).sqrt().min(C / 6f64.sqrt());
        let relative = circular + (self.drag - self.position) * (scale / (0.25 * camera_distance));
        center.velocity + relative.clamp_length_max(0.5 * C)
    }

    /// Draws the launch: the body's marker, the drag as an arrow, and the
    /// orbit it would follow around `around` if nothing else pulled.
    pub fn draw(
        &self,
        painter: &Painter,
        camera: &Camera,
        viewport: Rect,
        simulation: &Simulation,
    ) {
        let screen = |p: DVec3| camera.project(p, viewport).map(|s| s.position);
        let gm = self.entry.gm();
        let center = &simulation.bodies[self.around];
        let velocity = self.velocity(simulation, camera.distance, gm);
        let (r, v) = (self.position - center.position, velocity - center.velocity);
        let mu = center.gm + gm;
        let stroke = Stroke::new(1.5, SANDBOX_COLOR);
        if let Some(orbit) = osculating_orbit(r, v, mu, 180) {
            let points: Vec<_> = orbit.iter().map(|p| screen(center.position + *p)).collect();
            for pair in points.windows(2) {
                if let [Some(a), Some(b)] = pair {
                    painter.line_segment(
                        [*a, *b],
                        Stroke::new(1.0, SANDBOX_COLOR.gamma_multiply(0.6)),
                    );
                }
            }
        }
        let Some(at) = screen(self.position) else {
            return;
        };
        if let Some(to) = screen(self.drag) {
            painter.arrow(at, to - at, stroke);
        }
        painter.circle_filled(at, 5.0, SANDBOX_COLOR);
        // A black hole's horizon (or a big star), drawn to scale where it
        // will appear: whatever it overlaps falls in at once.
        if let Some(body) = self.entry.bodies(simulation).first()
            && let Some(p) = camera.project(self.position, viewport)
        {
            let radius = (body.radius * p.points_per_meter) as f32;
            if radius > 6.0 {
                let ring = if is_black_hole(body) {
                    "horizon"
                } else {
                    "surface"
                };
                painter.circle_stroke(at, radius, Stroke::new(1.0, SANDBOX_COLOR));
                painter.text(
                    at + eframe::egui::vec2(0.0, radius + 4.0),
                    Align2::CENTER_TOP,
                    ring,
                    FontId::proportional(11.0),
                    SANDBOX_COLOR,
                );
            }
        }
        let circular = (mu / r.length()).sqrt();
        let energy = 0.5 * v.length_squared() - mu / r.length();
        painter.text(
            at + eframe::egui::vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            format!(
                "{}: {:.1} km/s around {} (circular {:.1}){}",
                self.entry.label,
                v.length() / 1e3,
                center.name,
                circular / 1e3,
                if energy >= 0.0 { ", escaping" } else { "" }
            ),
            FontId::proportional(12.0),
            SANDBOX_COLOR,
        );
    }
}

/// Where saves go: the user's application data folder.
pub fn saves_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("Worldline").join("saves")
}

/// Saves the simulation, named after its date, and returns the file.
pub fn save(simulation: &mut Simulation) -> Result<PathBuf, String> {
    let dir = saves_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let date = DateTime::from_julian_date(simulation.julian_date())
        .to_string()
        .replace(':', "-");
    let path = (1..)
        .map(|n| {
            let suffix = if n == 1 {
                String::new()
            } else {
                format!(" ({n})")
            };
            dir.join(format!("{date} TDB{suffix}.worldline"))
        })
        .find(|p| !p.exists())
        .expect("a free name");
    std::fs::write(&path, simulation.save()).map_err(|e| e.to_string())?;
    Ok(path)
}

/// The saves, newest first: their names and files.
pub fn saves() -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(saves_dir()) else {
        return Vec::new();
    };
    let mut found: Vec<(std::time::SystemTime, String, PathBuf)> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "worldline"))
        .filter_map(|p| {
            let modified = p.metadata().ok()?.modified().ok()?;
            Some((modified, p.file_stem()?.to_string_lossy().into_owned(), p))
        })
        .collect();
    found.sort_by_key(|f| std::cmp::Reverse(f.0));
    found
        .into_iter()
        .map(|(_, name, path)| (name, path))
        .collect()
}
