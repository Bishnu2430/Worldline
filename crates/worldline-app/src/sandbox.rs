//! The sandbox tools: placing new bodies and launching them with a drag,
//! and saving and loading. See `docs/app.md`.

use std::path::PathBuf;

use eframe::egui::{Align2, Color32, FontId, Painter, Rect, Stroke};
use worldline_core::orbit::osculating_orbit;
use worldline_core::{Body, DVec3};

use crate::calendar::DateTime;
use crate::camera::Camera;
use crate::simulation::Simulation;

/// The color of bodies added in the sandbox.
pub const SANDBOX_COLOR: Color32 = Color32::from_rgb(255, 136, 196);

/// A body to add, with the mass and size of one of the solar system's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Earth,
    Jupiter,
    Sun,
}

impl Preset {
    pub const ALL: [Preset; 3] = [Preset::Earth, Preset::Jupiter, Preset::Sun];

    /// Its name in the menu.
    pub fn label(self) -> &'static str {
        match self {
            Preset::Earth => "Earth-mass planet",
            Preset::Jupiter => "Jupiter-mass planet",
            Preset::Sun => "Sun-mass star",
        }
    }

    /// The body it copies, as named in the 2025 snapshot.
    fn model(self) -> &'static str {
        match self {
            Preset::Earth => "Earth",
            Preset::Jupiter => "Jupiter",
            Preset::Sun => "Sun",
        }
    }

    /// A new body: the model's GM and radius from the snapshot (Jupiter's
    /// GM includes its moons), under the first free name like "New planet
    /// 2", at rest at the origin.
    pub fn body(self, simulation: &Simulation) -> Body {
        let snapshot = worldline_data::solar_system();
        let model = snapshot
            .bodies
            .iter()
            .find(|b| b.name == self.model())
            .expect("the snapshot has the model");
        let noun = if self == Preset::Sun {
            "star"
        } else {
            "planet"
        };
        let name = (1..)
            .map(|n| format!("New {noun} {n}"))
            .find(|name| simulation.index_of(name).is_none())
            .expect("a free name");
        Body::new(name, model.gm, model.radius)
    }
}

/// A body being placed: where, the body it will orbit if not dragged, and
/// how far the drag has pulled.
#[derive(Debug, Clone, Copy)]
pub struct Launch {
    pub preset: Preset,
    pub position: DVec3,
    /// Index of the body it orbits by default.
    pub around: usize,
    /// Where the drag has reached, on the same plane as `position`.
    pub drag: DVec3,
}

impl Launch {
    /// Its velocity. Placed without a drag, it circles `around`:
    /// prograde, in the ecliptic, at √(G(M + m)/r) relative to it, which
    /// for two bodies is exactly circular. Dragging adds velocity along the
    /// drag: one circular speed for every quarter of the camera's distance.
    pub fn velocity(&self, simulation: &Simulation, camera_distance: f64, gm: f64) -> DVec3 {
        let center = &simulation.bodies[self.around];
        let r = self.position - center.position;
        let speed = ((center.gm + gm) / r.length()).sqrt();
        let along = DVec3::Z.cross(r).normalize_or_zero();
        center.velocity
            + along * speed
            + (self.drag - self.position) * (speed / (0.25 * camera_distance))
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
        let gm = self.preset.body(simulation).gm;
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
        let circular = (mu / r.length()).sqrt();
        let energy = 0.5 * v.length_squared() - mu / r.length();
        painter.text(
            at + eframe::egui::vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            format!(
                "{}: {:.1} km/s around {} (circular {:.1}){}",
                self.preset.label(),
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
pub fn save(simulation: &Simulation) -> Result<PathBuf, String> {
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
