//! The sphere every body is drawn with.

use std::f32::consts::{PI, TAU};

/// A unit sphere made of latitude–longitude bands. Vertex positions double
/// as normals and as directions in the body's own frame (z toward the north
/// pole, x toward the prime meridian). Triangles wind counterclockwise seen
/// from outside, so back faces can be culled.
pub fn uv_sphere(longitudes: u32, latitudes: u32) -> (Vec<[f32; 3]>, Vec<u32>) {
    let mut vertices = Vec::with_capacity(((longitudes + 1) * (latitudes + 1)) as usize);
    for i in 0..=latitudes {
        // Colatitude: 0 at the north pole, π at the south pole.
        let (sin_t, cos_t) = (PI * i as f32 / latitudes as f32).sin_cos();
        for j in 0..=longitudes {
            let (sin_p, cos_p) = (TAU * j as f32 / longitudes as f32).sin_cos();
            vertices.push([sin_t * cos_p, sin_t * sin_p, cos_t]);
        }
    }

    let row = longitudes + 1;
    let mut indices = Vec::with_capacity((longitudes * latitudes * 6) as usize);
    for i in 0..latitudes {
        for j in 0..longitudes {
            let top_left = i * row + j;
            let top_right = top_left + 1;
            let bottom_left = top_left + row;
            let bottom_right = bottom_left + 1;
            // Each band quad is two triangles; at the poles one of them
            // collapses to a line and is skipped.
            if i != latitudes - 1 {
                indices.extend([top_left, bottom_left, bottom_right]);
            }
            if i != 0 {
                indices.extend([top_left, bottom_right, top_right]);
            }
        }
    }
    (vertices, indices)
}

/// A square from (−1, −1) to (1, 1) in the z = 0 plane, for rings. Both
/// sides are drawn, so winding doesn't matter.
pub fn quad() -> ([[f32; 3]; 4], [u32; 6]) {
    (
        [
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ],
        [0, 1, 2, 0, 2, 3],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangles_face_outward() {
        let (vertices, indices) = uv_sphere(32, 16);
        for triangle in indices.chunks(3) {
            let [a, b, c] = [0, 1, 2].map(|k| glam::Vec3::from(vertices[triangle[k] as usize]));
            let normal = (b - a).cross(c - a);
            let center = (a + b + c) / 3.0;
            assert!(normal.length() > 0.0, "degenerate triangle {triangle:?}");
            assert!(
                normal.dot(center) > 0.0,
                "triangle {triangle:?} faces inward"
            );
        }
    }

    #[test]
    fn vertices_lie_on_the_unit_sphere() {
        let (vertices, _) = uv_sphere(32, 16);
        for v in vertices {
            assert!((glam::Vec3::from(v).length() - 1.0).abs() < 1e-6);
        }
    }
}
