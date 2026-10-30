//! The Moon's shadow on Earth (step 5.3): where the axis and the edges of
//! its umbra and penumbra meet the ground, and the path of totality they
//! sweep. See `docs/physics/eclipses.md`.
//!
//! Earth is the WGS 84 ellipsoid (the IERS's GRS80 radius and flattening,
//! the same to 0.1 mm), turned as [`crate::rotation`] turns it. The shadow
//! is the one [`crate::eclipses`] measures: the cones tangent to the Sun
//! and to the Moon's limb, from the apparent Sun and Moon.

use glam::{DMat3, DVec3};

use crate::eclipses::{
    Cone, EARTH_EQUATORIAL_RADIUS, EARTH_FLATTENING, Ephemeris, K_PENUMBRA, K_UMBRA, shadow_cones,
};

/// Earth's mean radius, m (IUGG): for distances along the ground, good to
/// 0.5%.
const MEAN_RADIUS: f64 = 6_371_008.8;

/// A place on Earth: geodetic latitude and longitude (east), radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Place {
    /// Geodetic latitude, north positive, radians.
    pub latitude: f64,
    /// Longitude, east positive, radians.
    pub longitude: f64,
}

fn eccentricity_squared() -> f64 {
    EARTH_FLATTENING * (2.0 - EARTH_FLATTENING)
}

impl Place {
    /// Where it is on the ellipsoid, in Earth's body-fixed frame (m; z
    /// toward the north pole, x toward Greenwich).
    pub fn position(&self) -> DVec3 {
        let e2 = eccentricity_squared();
        let (sin_lat, cos_lat) = self.latitude.sin_cos();
        let (sin_lon, cos_lon) = self.longitude.sin_cos();
        let n = EARTH_EQUATORIAL_RADIUS / (1.0 - e2 * sin_lat * sin_lat).sqrt();
        DVec3::new(
            n * cos_lat * cos_lon,
            n * cos_lat * sin_lon,
            n * (1.0 - e2) * sin_lat,
        )
    }

    /// The place at `p`, a point on the ellipsoid in Earth's body-fixed
    /// frame: there the surface's normal gives tan φ = z / ((1 − e²) ρ),
    /// exactly.
    pub fn at(p: DVec3) -> Self {
        Self {
            latitude: p.z.atan2((1.0 - eccentricity_squared()) * p.x.hypot(p.y)),
            longitude: p.y.atan2(p.x),
        }
    }

    /// The distance to `other` along the ground, m (haversine on a sphere
    /// of Earth's mean radius: good to 0.5%).
    pub fn distance(&self, other: &Place) -> f64 {
        let half = |a: f64, b: f64| ((b - a) / 2.0).sin().powi(2);
        let h = half(self.latitude, other.latitude)
            + self.latitude.cos() * other.latitude.cos() * half(self.longitude, other.longitude);
        2.0 * MEAN_RADIUS * h.sqrt().min(1.0).asin()
    }
}

/// Where the ray from `origin` along `direction` first meets Earth's
/// surface, if it does: all in Earth's body-fixed frame, m. Stretched by
/// 1/(1 − f) along the pole the ellipsoid is a sphere, which the ray meets
/// where a quadratic says; solved from the ray's point nearest the center,
/// so a ray from the Moon loses no digits to its 380,000 km.
pub fn surface_hit(origin: DVec3, direction: DVec3) -> Option<DVec3> {
    let stretch = DVec3::new(1.0, 1.0, 1.0 / (1.0 - EARTH_FLATTENING));
    let (o, d) = (origin * stretch, direction * stretch);
    let along = -o.dot(d) / d.length_squared();
    let nearest = o + d * along;
    let r2 = EARTH_EQUATORIAL_RADIUS * EARTH_EQUATORIAL_RADIUS;
    let miss = nearest.length_squared();
    if miss > r2 {
        return None;
    }
    let t = along - ((r2 - miss) / d.length_squared()).sqrt();
    (t >= 0.0).then(|| origin + direction * t)
}

/// The Moon's shadow on Earth at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundShadow {
    /// Where the shadow's axis meets the ground, if it does.
    pub center: Option<Place>,
    /// The edge of the umbra (or, past its vertex, the antumbra) on the
    /// ground, going round the axis, closed; None where it misses Earth.
    pub umbra: Vec<Option<Place>>,
    /// And of the penumbra.
    pub penumbra: Vec<Option<Place>>,
}

impl GroundShadow {
    /// From the apparent Sun and Moon (m from Earth's center, in
    /// Worldline's frame), and Earth's orientation then
    /// (`body_to_ecliptic`), with `points` around each edge. Each edge is
    /// traced along the cone's lines, from the Moon's limb to the ground:
    /// toward the umbra's vertex and on, and away from the penumbra's.
    pub fn new(sun: DVec3, moon: DVec3, body_to_ecliptic: DMat3, points: usize) -> Self {
        let to_body = body_to_ecliptic.transpose();
        let (sun, moon) = (to_body * sun, to_body * moon);
        let a = EARTH_EQUATORIAL_RADIUS;
        let (penumbra, _) = shadow_cones(sun, moon, K_PENUMBRA * a);
        let (_, umbra) = shadow_cones(sun, moon, K_UMBRA * a);
        let axis = umbra.axis;
        let across = axis.any_orthonormal_vector();
        let up = axis.cross(across);
        let edge = |cone: &Cone, toward_vertex: bool| -> Vec<Option<Place>> {
            let radius = cone.radius((moon - cone.vertex).dot(axis));
            (0..=points)
                .map(|k| {
                    let angle = k as f64 / points as f64 * std::f64::consts::TAU;
                    let limb = moon + (across * angle.cos() + up * angle.sin()) * radius;
                    let line = (cone.vertex - limb).normalize();
                    let direction = if toward_vertex { line } else { -line };
                    surface_hit(limb, direction).map(Place::at)
                })
                .collect()
        };
        Self {
            center: surface_hit(moon, axis).map(Place::at),
            umbra: edge(&umbra, true),
            penumbra: edge(&penumbra, false),
        }
    }
}

/// The path of a solar eclipse's umbra across Earth.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    /// When each point is, s on the ephemeris's clock.
    pub times: Vec<f64>,
    /// Where the shadow's axis meets the ground then: the central line.
    pub central: Vec<Place>,
    /// The northern and southern limits then, where the umbra's edge
    /// reaches farthest either side of the central line, if on Earth.
    pub limits: Vec<[Option<Place>; 2]>,
}

/// How many points trace the umbra's edge for the path's limits: 0.5°
/// apart, which finds the edge's farthest point within (1 − cos 0.5°) of
/// the umbra's radius, under 10 m.
const EDGE_POINTS: usize = 720;

/// Where the shadow's axis meets the ground (in Earth's body-fixed frame,
/// m), from the apparent Sun and Moon and Earth's orientation.
fn axis_hit(sun: DVec3, moon: DVec3, body_to_ecliptic: DMat3) -> Option<DVec3> {
    let to_body = body_to_ecliptic.transpose();
    let (sun, moon) = (to_body * sun, to_body * moon);
    surface_hit(moon, (moon - sun).normalize())
}

/// Where the shadow's axis meets the ground, if it does: from the apparent
/// Sun and Moon (m from Earth's center, in Worldline's frame) and Earth's
/// orientation (`body_to_ecliptic`).
pub fn central_point(sun: DVec3, moon: DVec3, body_to_ecliptic: DMat3) -> Option<Place> {
    axis_hit(sun, moon, body_to_ecliptic).map(Place::at)
}

/// The point of the central line at time `t` (s on the ephemeris's clock)
/// and the northern and southern limits then, if the axis meets Earth.
/// `earth` gives Earth's orientation (`body_to_ecliptic`) at a time.
///
/// The limits are the points of the umbra's edge farthest from the
/// central line, across the way the shadow moves over the ground: where a
/// shadow moving steadily touches the edge of the strip it sweeps.
pub fn moment(
    ephemeris: &Ephemeris,
    t: f64,
    earth: &impl Fn(f64) -> DMat3,
) -> Option<(Place, [Option<Place>; 2])> {
    let hit = |t: f64| {
        let (sun, moon) = ephemeris.apparent(t);
        axis_hit(sun, moon, earth(t))
    };
    let center = hit(t)?;
    // The shadow's way over the ground: the central line a second either
    // side.
    let ahead = hit(t + 1.0).unwrap_or(center);
    let behind = hit(t - 1.0).unwrap_or(center);
    let side = center.normalize().cross(ahead - behind).normalize_or_zero();
    let (sun, moon) = ephemeris.apparent(t);
    let shadow = GroundShadow::new(sun, moon, earth(t), EDGE_POINTS);
    let reach = |sign: f64| {
        shadow
            .umbra
            .iter()
            .flatten()
            .map(|place| (place, sign * (place.position() - center).dot(side)))
            .filter(|(_, d)| *d > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(place, _)| *place)
    };
    // The shadow always moves east across the fundamental plane, so the
    // limit on its left is the one on the plane's north side: the northern
    // limit, as the canon names them, even where the path crosses the pole
    // and it lies farther south.
    Some((Place::at(center), [reach(1.0), reach(-1.0)]))
}

/// The path of the eclipse greatest at `greatest` (s on the ephemeris's
/// clock): every `step` s, both ways from then, for as long as the axis
/// meets Earth (see [`moment`]).
pub fn path(ephemeris: &Ephemeris, greatest: f64, step: f64, earth: impl Fn(f64) -> DMat3) -> Path {
    let (first, last) = (
        ephemeris.times[0],
        ephemeris.times[ephemeris.times.len() - 1],
    );
    let mut found = Vec::new();
    for direction in [-1.0, 1.0] {
        let mut k = if direction < 0.0 { 0.0 } else { 1.0 };
        loop {
            let t = greatest + direction * k * step;
            if t < first || t > last {
                break;
            }
            let Some(here) = moment(ephemeris, t, &earth) else {
                break;
            };
            found.push((t, here));
            k += 1.0;
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    Path {
        times: found.iter().map(|f| f.0).collect(),
        central: found.iter().map(|f| f.1.0).collect(),
        limits: found.iter().map(|f| f.1.1).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_round_trip_through_the_ellipsoid() {
        for (lat, lon) in [(0.0, 0.0), (65.2, -25.2), (-33.9, 151.2), (89.9, 10.0)] {
            let place = Place {
                latitude: f64::to_radians(lat),
                longitude: f64::to_radians(lon),
            };
            let back = Place::at(place.position());
            assert!((back.latitude - place.latitude).abs() < 1e-14);
            assert!((back.longitude - place.longitude).abs() < 1e-14);
        }
        // A degree of latitude is 111.2 km on the mean sphere.
        let a = Place {
            latitude: 0.0,
            longitude: 0.0,
        };
        let b = Place {
            latitude: 1f64.to_radians(),
            longitude: 0.0,
        };
        assert!((a.distance(&b) - 111_195.0).abs() < 1.0);
    }

    #[test]
    fn rays_meet_the_ellipsoid_where_it_is() {
        let a = EARTH_EQUATORIAL_RADIUS;
        let b = a * (1.0 - EARTH_FLATTENING);
        // Straight down onto the equator and onto the pole, from the
        // Moon's distance.
        let equator = surface_hit(DVec3::X * 3.8e8, -DVec3::X).unwrap();
        assert!((equator - DVec3::X * a).length() < 1e-6);
        let pole = surface_hit(DVec3::Z * 3.8e8, -DVec3::Z).unwrap();
        assert!((pole - DVec3::Z * b).length() < 1e-6);
        // Passing just outside, and pointing away: no hit.
        assert!(surface_hit(DVec3::new(3.8e8, a * 1.0001, 0.0), -DVec3::X).is_none());
        assert!(surface_hit(DVec3::X * 3.8e8, DVec3::X).is_none());
    }

    #[test]
    fn a_central_shadow_falls_around_its_axis() {
        // Sun and Moon over the equator at longitude 0, the Moon 370,000
        // km away: the axis meets the ground at (0°, 0°); the umbra is a
        // small ring around it, the penumbra a big one, each edge on Earth
        // all round.
        let sun = DVec3::X * 1.496e11;
        let moon = DVec3::X * 3.7e8;
        let shadow = GroundShadow::new(sun, moon, DMat3::IDENTITY, 36);
        let center = shadow.center.unwrap();
        assert!(center.latitude.abs() < 1e-12 && center.longitude.abs() < 1e-12);
        let radii = |edge: &[Option<Place>]| -> Vec<f64> {
            edge.iter().map(|p| p.unwrap().distance(&center)).collect()
        };
        let umbra = radii(&shadow.umbra);
        let penumbra = radii(&shadow.penumbra);
        // l₂ at the Moon's 370,000 km is about −0.02 Earth radii: an
        // umbra about 100 km across on the plane.
        assert!(umbra.iter().all(|&r| r > 30e3 && r < 80e3), "{umbra:?}");
        assert!(penumbra.iter().all(|&r| r > 3e6), "{penumbra:?}");
    }
}
