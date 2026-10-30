//! Standing on Earth (step 5.4): the sky from a latitude and longitude, with
//! the horizon, Sun, Moon and planets, and when the Sun and Moon next rise
//! and set. See `docs/physics/sky-from-earth.md`.

use std::f64::consts::{FRAC_PI_2, PI};
use std::sync::mpsc::{Receiver, channel};

use eframe::egui::{Align2, Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use glam::DMat3;
use worldline_core::DVec3;
use worldline_core::constants::DAY;
use worldline_core::eclipses::{
    EARTH_EQUATORIAL_RADIUS, K_MEAN, SUN_RADIUS, State, seen_from, share_of_sun_covered,
};
use worldline_core::horizon::{Horizontal, LocalFrame, Luminary, refraction, rises_and_sets};
use worldline_core::shadow_on_earth::Place;

use crate::camera::Camera;
use crate::simulation::Simulation;
use crate::view::{body_color, draw_corona};

/// Places to pick, as (name, latitude, longitude), degrees: the two cities
/// checked against the Naval Observatory, and Reykjavík, which stood in the
/// path of 12 August 2026's totality.
pub const PLACES: [(&str, f64, f64); 3] = [
    ("Washington, DC", 38.0 + 53.0 / 60.0, -(77.0 + 2.0 / 60.0)),
    ("Sydney", -(33.0 + 52.0 / 60.0), 151.0 + 12.0 / 60.0),
    ("Reykjavík", 64.1466, -21.9426),
];

/// What the sky shows, as named in the simulation.
const SHOWN: [&str; 9] = [
    "Sun", "Moon", "Mercury", "Venus", "Mars", "Jupiter", "Saturn", "Uranus", "Neptune",
];

/// How far ahead the next rises and sets are looked for: a week, and a day
/// for the next day's events.
const DAYS_AHEAD: f64 = 8.0;

/// A body in the sky.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Seen {
    /// Its place in the simulation's list.
    pub index: usize,
    /// Where it really stands, as seen from the place: light-time,
    /// aberration and parallax included.
    pub true_place: Horizontal,
    /// Where the air lifts it to.
    pub shown: Horizontal,
    /// The direction of `true_place`, unit, in Worldline's frame.
    pub direction: DVec3,
    /// Its apparent radius, radians.
    pub angular_radius: f64,
}

/// The sky from a place.
#[derive(Debug, Clone, PartialEq)]
pub struct Sky {
    /// Where the place is, m.
    pub eye: DVec3,
    /// The directions there.
    pub frame: LocalFrame,
    /// What's in it, above the horizon or not.
    pub bodies: Vec<Seen>,
}

/// Where a place on Earth is, in the simulation now, and the directions
/// there.
pub fn stand(simulation: &Simulation, place: Place) -> Option<(State, LocalFrame)> {
    let earth = &simulation.bodies[simulation.index_of("Earth")?];
    let rotation = simulation
        .earth_orientation(simulation.julian_date())?
        .body_to_ecliptic;
    let observer = State {
        position: earth.position + rotation * place.position(),
        velocity: earth.velocity,
    };
    Some((observer, LocalFrame::new(place, rotation)))
}

/// The sky from `place` now.
pub fn sky_from(simulation: &Simulation, place: Place) -> Option<Sky> {
    let (observer, frame) = stand(simulation, place)?;
    let bodies = SHOWN
        .iter()
        .filter_map(|name| {
            let index = simulation.index_of(name)?;
            let body = &simulation.bodies[index];
            let state = State {
                position: body.position,
                velocity: body.velocity,
            };
            let seen = seen_from(observer, state);
            let true_place = frame.horizontal(seen);
            let radius = match *name {
                "Sun" => SUN_RADIUS,
                "Moon" => K_MEAN * EARTH_EQUATORIAL_RADIUS,
                _ => body.radius,
            };
            Some(Seen {
                index,
                true_place,
                shown: Horizontal {
                    altitude: true_place.altitude + refraction(true_place.altitude),
                    azimuth: true_place.azimuth,
                },
                direction: seen.normalize(),
                angular_radius: (radius / seen.length()).asin(),
            })
        })
        .collect();
    Some(Sky {
        eye: observer.position,
        frame,
        bodies,
    })
}

/// How bright the sky is, 0 to 1, by the Sun's true altitude (radians):
/// dark from the end of nautical twilight, 12° down, full by 6° up. A
/// visual aid: it isn't a model of the sky's light.
fn daylight(altitude: f64) -> f64 {
    let x = ((altitude.to_degrees() + 12.0) / 18.0).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

/// An eight-wind compass name for an azimuth (radians).
pub fn compass(azimuth: f64) -> &'static str {
    const NAMES: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    let k = (azimuth / (PI / 4.0)).round() as i64;
    NAMES[k.rem_euclid(8) as usize]
}

/// Draws the Moon of radius `r` at `center`: its dark disk, then its lit
/// part, bounded by the limb toward the Sun and the terminator, slice by
/// slice across the line toward the Sun (`toward_sun`, unit, on screen). The
/// terminator is the half-ellipse cos(elongation) of the way across, as the
/// Moon is far nearer than the Sun.
fn draw_moon(
    painter: &Painter,
    center: Pos2,
    r: f32,
    toward_sun: eframe::egui::Vec2,
    cos_elongation: f64,
    dark: Option<Color32>,
) {
    const SLICES: usize = 48;
    if let Some(dark) = dark {
        painter.circle_filled(center, r, dark);
    }
    // New: nothing lit faces us.
    if cos_elongation > 1.0 - 1e-5 {
        return;
    }
    let across = toward_sun.rot90();
    let lit = Color32::from_rgb(236, 233, 222);
    let mut mesh = Mesh::default();
    for k in 0..=SLICES {
        let theta = -FRAC_PI_2 + PI * k as f64 / SLICES as f64;
        let (height, width) = theta.sin_cos();
        let limb = toward_sun * width as f32 + across * height as f32;
        let edge = toward_sun * (cos_elongation * width) as f32 + across * height as f32;
        mesh.colored_vertex(center + limb * r, lit);
        mesh.colored_vertex(center + edge * r, lit);
        if k > 0 {
            let v = 2 * k as u32;
            mesh.add_triangle(v - 2, v - 1, v);
            mesh.add_triangle(v - 1, v, v + 1);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// Draws the sky from `place` over the starfield: the daylight, the Sun,
/// Moon and planets where the air lifts them, then the ground, which hides
/// what's below the horizon, and the compass. Disks are at their true size,
/// never under 3 points across for a planet.
pub fn draw(
    painter: &Painter,
    viewport: Rect,
    camera: &Camera,
    simulation: &Simulation,
    place: Place,
    labels: bool,
) {
    const FAR: f64 = 1e15;
    let Some(sky) = sky_from(simulation, place) else {
        return;
    };
    let at = |h: Horizontal| camera.project(sky.eye + sky.frame.direction(h) * FAR, viewport);
    let seen = |name: &str| {
        sky.bodies
            .iter()
            .find(|b| simulation.bodies[b.index].name == name)
    };
    let (sun, moon) = (seen("Sun"), seen("Moon"));
    let covered = match (sun, moon) {
        (Some(s), Some(m)) => share_of_sun_covered(
            s.direction.angle_between(m.direction),
            s.angular_radius,
            m.angular_radius,
        ),
        _ => 0.0,
    };
    let day = sun.map_or(0.0, |s| daylight(s.true_place.altitude)) * (1.0 - covered);

    // The horizon is a level line: the camera doesn't roll.
    let look = camera.standing.map_or(0.0, |s| s.look.azimuth);
    let level = Horizontal {
        altitude: 0.0,
        azimuth: look,
    };
    let horizon = at(level)
        .map_or(viewport.bottom(), |p| p.position.y)
        .clamp(viewport.top(), viewport.bottom());

    if day > 0.0 {
        let alpha = (240.0 * day) as u8;
        let top = Color32::from_rgba_unmultiplied(40, 92, 178, alpha);
        let low = Color32::from_rgba_unmultiplied(168, 203, 236, alpha);
        let mut mesh = Mesh::default();
        mesh.colored_vertex(viewport.left_top(), top);
        mesh.colored_vertex(viewport.right_top(), top);
        mesh.colored_vertex(pos2(viewport.left(), horizon), low);
        mesh.colored_vertex(pos2(viewport.right(), horizon), low);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 2, 3);
        painter.add(Shape::mesh(mesh));
    }

    let scale =
        |p: &crate::camera::Projection, angle: f64| (p.points_per_meter * FAR * angle.tan()) as f32;
    let font = FontId::proportional(12.0);
    for body in &sky.bodies {
        let name = simulation.bodies[body.index].name.as_str();
        if name == "Sun" || name == "Moon" {
            continue;
        }
        let Some(p) = at(body.shown) else {
            continue;
        };
        let color = body_color(name);
        // Planets fade by day, but stay findable.
        let alpha = (255.0 * (1.0 - 0.7 * day)) as u8;
        let dot = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);
        painter.circle_filled(p.position, scale(&p, body.angular_radius).max(1.5), dot);
        if labels {
            painter.text(
                p.position + vec2(5.0, 3.0),
                Align2::LEFT_TOP,
                name,
                font.clone(),
                Color32::from_white_alpha(alpha),
            );
        }
    }
    if let Some(sun) = sun
        && let Some(p) = at(sun.shown)
    {
        let r = scale(&p, sun.angular_radius).max(3.0);
        if covered >= 1.0 {
            // Totality: the corona, seen only now.
            draw_corona(painter, p.position, r);
        } else {
            // A soft glow, fading with the share still uncovered.
            let alpha = (5.0 * (1.0 - covered) + 1.0) as u8;
            let glow = Color32::from_rgba_unmultiplied(255, 236, 196, alpha);
            for k in (1..=24).rev() {
                painter.circle_filled(p.position, r * (1.0 + 0.12 * k as f32), glow);
            }
        }
        painter.circle_filled(p.position, r, Color32::from_rgb(255, 250, 236));
        if labels {
            painter.text(
                p.position + vec2(r + 5.0, 3.0),
                Align2::LEFT_TOP,
                "Sun",
                font.clone(),
                Color32::from_white_alpha(230),
            );
        }
    }
    if let (Some(moon), Some(sun)) = (moon, sun)
        && let Some(p) = at(moon.shown)
    {
        let (_, right, up) = camera.basis();
        let cos_elongation = moon.direction.dot(sun.direction);
        let toward = sun.direction - moon.direction * cos_elongation;
        let on_screen = vec2(toward.dot(right) as f32, -toward.dot(up) as f32).normalized();
        let r = scale(&p, moon.angular_radius).max(3.0);
        // By day its dark part is the sky, unless it stands before the Sun.
        let dark = (covered > 0.0 || day < 0.35).then_some(Color32::from_rgb(10, 11, 14));
        draw_moon(painter, p.position, r, on_screen, cos_elongation, dark);
        if labels {
            painter.text(
                p.position + vec2(r + 5.0, 3.0 - 14.0),
                Align2::LEFT_TOP,
                "Moon",
                font.clone(),
                Color32::from_white_alpha(230),
            );
        }
    }

    let shade = 0.3 + 0.7 * day as f32;
    let ground = Color32::from_rgb(
        (46.0 * shade) as u8,
        (52.0 * shade) as u8,
        (38.0 * shade) as u8,
    );
    let below = Rect::from_min_max(pos2(viewport.left(), horizon), viewport.right_bottom());
    painter.rect_filled(below, 0.0, ground);
    painter.line_segment(
        [
            pos2(viewport.left(), horizon),
            pos2(viewport.right(), horizon),
        ],
        Stroke::new(1.0, Color32::from_white_alpha(70)),
    );
    if labels {
        for k in 0..8 {
            let azimuth = k as f64 * PI / 4.0;
            let point = Horizontal {
                altitude: 0.0,
                azimuth,
            };
            if let Some(p) = at(point) {
                painter.text(
                    pos2(p.position.x, horizon + 4.0),
                    Align2::CENTER_TOP,
                    compass(azimuth),
                    FontId::proportional(13.0),
                    Color32::from_white_alpha(170),
                );
            }
        }
    }
}

/// A rise or set found ahead: when (a Julian Date, TDB), of what, and
/// whether rising.
pub type RiseOrSet = (f64, Luminary, bool);

/// When the Sun and Moon rise and set in the days ahead at a place, from a
/// copy of the simulation run ahead on its own thread.
pub struct Ahead {
    /// The place.
    pub place: Place,
    /// The moment it was asked at, a Julian Date (TDB).
    pub from: f64,
    /// What it found, once it has, in order of time.
    pub times: Option<Vec<RiseOrSet>>,
    receiver: Receiver<Vec<RiseOrSet>>,
}

impl Ahead {
    /// Starts working them out.
    pub fn start(simulation: &mut Simulation, place: Place) -> Self {
        let save = simulation.save();
        let from = simulation.julian_date();
        let (sender, receiver) = channel();
        std::thread::spawn(move || {
            let Ok(mut runner) = Simulation::load(&save, DAY) else {
                return;
            };
            let start = runner.time();
            let ephemeris = runner.ephemeris(start + DAYS_AHEAD * DAY, 6.0 * 3600.0);
            let jd = |t: f64| from + (t - start) / DAY;
            let earth = |t: f64| {
                runner
                    .earth_orientation(jd(t))
                    .map_or(DMat3::IDENTITY, |o| o.body_to_ecliptic)
            };
            let mut times = Vec::new();
            for luminary in [Luminary::Sun, Luminary::Moon] {
                for crossing in rises_and_sets(&ephemeris, place, luminary, earth) {
                    times.push((jd(crossing.time), luminary, crossing.rising));
                }
            }
            times.sort_by(|a, b| a.0.total_cmp(&b.0));
            let _ = sender.send(times);
        });
        Self {
            place,
            from,
            times: None,
            receiver,
        }
    }

    /// Takes in what it found, if it has finished.
    pub fn poll(&mut self) {
        if let Ok(times) = self.receiver.try_recv() {
            self.times = Some(times);
        }
    }

    /// The next rise or set of `luminary` after `jd` (TDB), if known.
    pub fn next(&self, luminary: Luminary, rising: bool, jd: f64) -> Option<f64> {
        self.times
            .as_ref()?
            .iter()
            .find(|e| e.0 > jd && e.1 == luminary && e.2 == rising)
            .map(|e| e.0)
    }

    /// Whether it no longer covers `jd`: from before it, or too near its
    /// end for the next day's events.
    pub fn stale(&self, jd: f64) -> bool {
        self.times.is_some() && (jd < self.from || jd > self.from + DAYS_AHEAD - 2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compass_rose_names_azimuths() {
        let name = |degrees: f64| compass(degrees.to_radians());
        assert_eq!(name(0.0), "N");
        assert_eq!(name(22.0), "N");
        assert_eq!(name(23.0), "NE");
        assert_eq!(name(90.0), "E");
        assert_eq!(name(225.0), "SW");
        assert_eq!(name(359.0), "N");
    }

    #[test]
    fn daylight_runs_from_nautical_twilight_to_6_degrees_up() {
        let day = |degrees: f64| daylight(degrees.to_radians());
        assert_eq!(day(-13.0), 0.0);
        assert_eq!(day(7.0), 1.0);
        assert!(day(-3.0) > 0.0 && day(-3.0) < 1.0);
    }

    #[test]
    fn the_sky_from_a_city_has_the_sun_where_it_should_be() {
        // Noon at Greenwich on the snapshot's day: from the equator at the
        // longitude under the Sun, it stands near the zenith. Worked out
        // from where the Sun is, it can't be far from there.
        let simulation = Simulation::solar_system(DAY);
        let rotation = simulation
            .earth_orientation(simulation.julian_date())
            .unwrap()
            .body_to_ecliptic;
        let earth = &simulation.bodies[simulation.index_of("Earth").unwrap()];
        let sun = &simulation.bodies[simulation.index_of("Sun").unwrap()];
        let under = rotation.transpose() * (sun.position - earth.position);
        let place = Place {
            latitude: under.z.atan2(under.x.hypot(under.y)),
            longitude: under.y.atan2(under.x),
        };
        let sky = sky_from(&simulation, place).unwrap();
        let body = sky
            .bodies
            .iter()
            .find(|b| simulation.bodies[b.index].name == "Sun")
            .unwrap();
        assert!(body.true_place.altitude.to_degrees() > 89.5, "{body:?}");
        // A body in the zenith is hardly lifted; every shown body is
        // at least as high as it really is.
        for seen in &sky.bodies {
            assert!(seen.shown.altitude >= seen.true_place.altitude - 1e-3);
        }
        assert_eq!(sky.bodies.len(), SHOWN.len());
    }
}
