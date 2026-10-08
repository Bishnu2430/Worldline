//! A camera that orbits a target point.
//!
//! All the math is in double precision, relative to the camera, and only
//! the final screen coordinates become single precision. So a planet
//! 30 AU away renders as steadily as one next to the camera.

use std::f64::consts::FRAC_PI_2;

use eframe::egui::{Pos2, Rect};
use worldline_core::DVec3;

/// Where a point lands on screen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    /// Screen position, in points.
    pub position: Pos2,
    /// Distance in front of the camera, in m.
    pub depth: f64,
    /// How many screen points one meter spans at this depth.
    pub points_per_meter: f64,
}

/// A perspective camera looking at `target` from a point on a sphere
/// around it. "Up" is ecliptic north (+z).
#[derive(Debug, Clone, PartialEq)]
pub struct Camera {
    /// The point the camera looks at and orbits, in m.
    pub target: DVec3,
    /// Angle around the vertical axis, in radians.
    pub yaw: f64,
    /// Angle above the ecliptic plane, in radians.
    pub pitch: f64,
    /// Distance from the target, in m.
    pub distance: f64,
    /// Vertical field of view, in radians.
    pub fov_y: f64,
}

impl Camera {
    /// Farthest the camera can pull back: about 6,700 AU.
    /// About 300 kpc: room to see the solar system and the galactic
    /// center, 8.3 kpc away, together.
    pub const MAX_DISTANCE: f64 = 1e22;
    /// Radians of rotation per point dragged.
    const DRAG_SENSITIVITY: f64 = 0.005;
    /// Zoom factor per point scrolled, as an exponent.
    const ZOOM_SENSITIVITY: f64 = 0.002;
    /// Keeps the camera from flipping over the poles.
    const MAX_PITCH: f64 = FRAC_PI_2 - 0.01;

    /// The camera's position, in m.
    pub fn eye(&self) -> DVec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        self.target
            + self.distance * DVec3::new(cos_pitch * cos_yaw, cos_pitch * sin_yaw, sin_pitch)
    }

    /// Unit vectors pointing forward, right and up from the camera's view.
    pub fn basis(&self) -> (DVec3, DVec3, DVec3) {
        let forward = (self.target - self.eye()).normalize();
        let right = forward.cross(DVec3::Z).normalize();
        let up = right.cross(forward);
        (forward, right, up)
    }

    /// Projects a point onto the screen area `viewport`. Returns `None` for
    /// points behind the camera.
    pub fn project(&self, point: DVec3, viewport: Rect) -> Option<Projection> {
        let (forward, right, up) = self.basis();
        let relative = point - self.eye();
        let depth = relative.dot(forward);
        if depth <= 1e-9 * self.distance {
            return None;
        }
        let focal = f64::from(viewport.height()) / 2.0 / (self.fov_y / 2.0).tan();
        let points_per_meter = focal / depth;
        let center = viewport.center();
        let x = f64::from(center.x) + relative.dot(right) * points_per_meter;
        let y = f64::from(center.y) - relative.dot(up) * points_per_meter;
        Some(Projection {
            position: Pos2::new(x as f32, y as f32),
            depth,
            points_per_meter,
        })
    }

    /// The point under screen position `pointer` (in `viewport`) on the
    /// horizontal plane at height `z` (m): where the line of sight through
    /// it meets the plane. `None` if it never does (looking along or away
    /// from the plane).
    pub fn point_on_plane(&self, pointer: Pos2, viewport: Rect, z: f64) -> Option<DVec3> {
        let (forward, right, up) = self.basis();
        let focal = f64::from(viewport.height()) / 2.0 / (self.fov_y / 2.0).tan();
        let center = viewport.center();
        let direction = forward + right * (f64::from(pointer.x - center.x) / focal)
            - up * (f64::from(pointer.y - center.y) / focal);
        let eye = self.eye();
        let along = (z - eye.z) / direction.z;
        (along.is_finite() && along > 0.0).then(|| eye + direction * along)
    }

    /// Rotates the camera around its target by a drag of (dx, dy) points.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= f64::from(dx) * Self::DRAG_SENSITIVITY;
        self.pitch = (self.pitch + f64::from(dy) * Self::DRAG_SENSITIVITY)
            .clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
    }

    /// Moves the camera toward (positive `scroll`) or away from its target,
    /// staying at least `min_distance` away.
    pub fn zoom(&mut self, scroll: f32, min_distance: f64) {
        self.distance *= (-f64::from(scroll) * Self::ZOOM_SENSITIVITY).exp();
        self.clamp_distance(min_distance);
    }

    /// Keeps the distance between `min_distance` and [`Self::MAX_DISTANCE`].
    pub fn clamp_distance(&mut self, min_distance: f64) {
        self.distance = self.distance.clamp(min_distance, Self::MAX_DISTANCE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eframe::egui::vec2;

    fn camera() -> Camera {
        Camera {
            target: DVec3::new(1e11, 0.0, 0.0),
            yaw: -FRAC_PI_2, // camera on the −y side, looking toward +y
            pitch: 0.0,
            distance: 1e10,
            fov_y: 0.8,
        }
    }

    fn viewport() -> Rect {
        Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))
    }

    #[test]
    fn target_lands_in_the_center() {
        let p = camera().project(camera().target, viewport()).unwrap();
        assert!((p.position - viewport().center()).length() < 1e-3);
        assert!((p.depth - 1e10).abs() < 1.0);
    }

    #[test]
    fn right_is_right_and_up_is_up() {
        let cam = camera();
        let right = cam
            .project(cam.target + DVec3::new(1e9, 0.0, 0.0), viewport())
            .unwrap();
        let up = cam
            .project(cam.target + DVec3::new(0.0, 0.0, 1e9), viewport())
            .unwrap();
        assert!(right.position.x > viewport().center().x);
        assert!(
            up.position.y < viewport().center().y,
            "screen y grows downward"
        );
    }

    #[test]
    fn points_behind_the_camera_are_hidden() {
        let cam = camera();
        assert!(
            cam.project(cam.eye() - DVec3::new(0.0, 1e9, 0.0), viewport())
                .is_none()
        );
    }

    #[test]
    fn size_on_screen_falls_off_with_distance() {
        let cam = camera();
        let near = cam.project(cam.target, viewport()).unwrap();
        let far = cam
            .project(cam.target + DVec3::new(0.0, 1e10, 0.0), viewport())
            .unwrap();
        assert!((near.points_per_meter / far.points_per_meter - 2.0).abs() < 1e-9);
    }

    #[test]
    fn far_away_points_project_precisely() {
        // A target 30 AU out, nudged 1 km sideways, still moves on screen
        // by exactly the expected amount: no single-precision jitter.
        let mut cam = camera();
        cam.target = DVec3::new(4.5e12, 0.0, 0.0);
        cam.distance = 1e6;
        let a = cam.project(cam.target, viewport()).unwrap();
        let b = cam
            .project(cam.target + DVec3::new(1e3, 0.0, 0.0), viewport())
            .unwrap();
        let expected = 1e3 * a.points_per_meter;
        assert!(((b.position.x - a.position.x) as f64 - expected).abs() < 1e-3);
    }

    #[test]
    fn zoom_stays_in_range() {
        let mut cam = camera();
        cam.zoom(1e6, 5e9);
        assert_eq!(cam.distance, 5e9);
        cam.zoom(-1e6, 5e9);
        assert_eq!(cam.distance, Camera::MAX_DISTANCE);
    }

    #[test]
    fn a_point_on_the_plane_projects_back_where_it_was_picked() {
        let camera = Camera {
            target: DVec3::new(1e11, -2e10, 3e9),
            yaw: 0.7,
            pitch: 0.5,
            distance: 4e11,
            fov_y: 0.8,
        };
        let viewport = Rect::from_min_size(Pos2::ZERO, vec2(1200.0, 800.0));
        let pointer = Pos2::new(830.0, 210.0);
        let point = camera.point_on_plane(pointer, viewport, 3e9).unwrap();
        assert!((point.z - 3e9).abs() < 1e-3);
        let back = camera.project(point, viewport).unwrap().position;
        assert!((back - pointer).length() < 1e-3, "{back:?}");
        // A camera level with the plane sees it edge-on: no point to pick.
        let level = Camera {
            pitch: 0.0,
            ..camera
        };
        assert!(
            level
                .point_on_plane(Pos2::new(600.0, 100.0), viewport, 3e9)
                .is_none()
        );
    }
}
