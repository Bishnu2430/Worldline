//! Eclipses (step 5.2): the shadows of the Moon and Earth, and an eclipse
//! finder. See `docs/physics/eclipses.md`.
//!
//! As Espenak & Meeus's Five Millennium Canons of Solar and Lunar Eclipses
//! (2006, 2009) define them:
//! - a solar eclipse's greatest moment is when the axis of the Moon's
//!   shadow passes closest to Earth's center; γ is that least distance, in
//!   Earth's equatorial radii, positive north;
//! - a lunar eclipse's is when the Moon's center passes closest to the axis
//!   of Earth's shadow; γ likewise.
//!
//! Positions are apparent ones, seen from Earth's center: where the Sun
//! and Moon were when their light now arriving left them, shifted by the
//! aberration of Earth's motion. That puts the shadows where they
//! physically fall, as they are seen. (The Sun's aberration alone moves
//! the Moon's shadow at Earth by about 40 km, 40 s of an eclipse's time.)

use crate::DVec3;
use crate::constants::{AU, C, OBLIQUITY_J2000};

/// Earth's equatorial radius, m: IERS Conventions 2010, Table 1.2
/// (GRS80).
pub const EARTH_EQUATORIAL_RADIUS: f64 = 6_378_137.0;

/// Earth's flattening, likewise.
pub const EARTH_FLATTENING: f64 = 1.0 / 298.257_222_101;

/// The Moon's radius in Earth equatorial radii, as the solar canon uses it
/// (Espenak & Meeus 2006 §1.5): a mean over its limb's peaks and valleys
/// for the penumbra, and a mean minimum radius for the umbra and
/// antumbra, so eclipses beaded by the limb's valleys count as annular.
pub const K_PENUMBRA: f64 = 0.272_488_0;

/// The umbra's and antumbra's lunar radius (see [`K_PENUMBRA`]).
pub const K_UMBRA: f64 = 0.272_281;

/// The IAU's (1982) mean lunar radius, in Earth equatorial radii: for
/// the Moon's own size in lunar eclipses.
pub const K_MEAN: f64 = 0.272_507_6;

/// The Sun's radius as eclipses see it, m: 959.63″ at 1 AU (Auwers),
/// 695,990 km, as the canons and NASA's Besselian elements use it.
/// Eclipses happen at the Sun's visible edge, a little beyond the
/// photosphere's radius that the IAU's nominal 695,700 km is (2015
/// Resolution B3). With the IAU's radius, the umbra of 2026 August 12
/// comes out 1% bigger than NASA's; with this one, tan f₂ matches NASA's to
/// 10⁻⁷ (step 5.3).
pub const SUN_RADIUS: f64 = AU * (959.63 / 3600.0 * std::f64::consts::PI / 180.0);

/// Danjon's enlargement of Earth's shadow by its atmosphere, which the
/// lunar canon uses: Earth's parallax times 1 + 1/85 − 1/594 ≈ 1.01, the
/// last term for its flattening at latitude 45° (NASA, "Enlargement of
/// Earth's Shadows", eqs. 1-5 and 1-6).
pub const DANJON: f64 = 1.01;

/// A body's position (m) and velocity (m/s), barycentric.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct State {
    /// m.
    pub position: DVec3,
    /// m/s.
    pub velocity: DVec3,
}

/// Earth's north pole, in Worldline's frame (ecliptic and mean equinox of
/// J2000): for which way is north. Precession since 2000 moves it by under
/// a quarter of a degree in this century, nothing to the sign of γ.
fn north() -> DVec3 {
    DVec3::new(0.0, OBLIQUITY_J2000.sin(), OBLIQUITY_J2000.cos())
}

/// The apparent positions (m) of the Sun and the Moon from Earth's center:
/// each where it was when the light now reaching Earth left it (back along
/// its own motion by the light's travel time), turned by the aberration of
/// Earth's motion, u' = u + v/c − u(u·v/c), to first order in v/c (the next
/// order is 10⁻⁸ rad).
pub fn apparent(sun: State, earth: State, moon: State) -> (DVec3, DVec3) {
    (seen_from(earth, sun), seen_from(earth, moon))
}

/// Where `body` appears from `observer`: the vector to it, as [`apparent`]
/// works it out.
pub fn seen_from(observer: State, body: State) -> DVec3 {
    let light_time = (body.position - observer.position).length() / C;
    let then = body.position - body.velocity * light_time - observer.position;
    let u = then.normalize();
    let beta = observer.velocity / C;
    (u + beta - u * u.dot(beta)).normalize() * then.length()
}

/// The share of the Sun's disk the Moon's covers, 0 to 1: disks of angular
/// radii `sun` and `moon`, their centers `separation` apart (all radians;
/// small enough to take as flat circles).
pub fn share_of_sun_covered(separation: f64, sun: f64, moon: f64) -> f64 {
    let d = separation;
    if d >= sun + moon {
        return 0.0;
    }
    if d <= (moon - sun).abs() {
        return if moon >= sun {
            1.0
        } else {
            (moon / sun) * (moon / sun)
        };
    }
    // The lens two overlapping circles make: a sector of each, less the
    // kite between their centers and the points where they cross.
    let half_angle = |r: f64, other: f64| ((d * d + r * r - other * other) / (2.0 * d * r)).acos();
    let (a, b) = (half_angle(sun, moon), half_angle(moon, sun));
    let kite = sun * a.sin() * d;
    (sun * sun * a + moon * moon * b - kite) / (std::f64::consts::PI * sun * sun)
}

/// The kind of a solar eclipse, as the canon names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SolarKind {
    /// The Moon covers the Sun somewhere.
    Total,
    /// The Moon is too small to: a ring of Sun around it.
    Annular,
    /// Total where the shadow falls nearest, annular toward its ends.
    Hybrid,
    /// Only the penumbra touches Earth.
    Partial,
}

/// The kind of a lunar eclipse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LunarKind {
    /// The Moon wholly in Earth's umbra.
    Total,
    /// Partly in the umbra.
    Partial,
    /// Only in the penumbra.
    Penumbral,
}

/// An eclipse at its greatest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Eclipse {
    /// The Moon's shadow on Earth.
    Solar {
        /// When, s on the ephemeris's clock.
        time: f64,
        /// What the Moon makes of the Sun.
        kind: SolarKind,
        /// The shadow axis's least distance from Earth's center, Earth
        /// equatorial radii, positive north.
        gamma: f64,
        /// The share of the Sun's diameter covered where the eclipse is
        /// greatest; for a central one, the Moon's apparent diameter over
        /// the Sun's there.
        magnitude: f64,
    },
    /// Earth's shadow on the Moon.
    Lunar {
        /// When, s on the ephemeris's clock.
        time: f64,
        /// How much of the Moon the umbra covers.
        kind: LunarKind,
        /// The Moon's least distance from the shadow axis, Earth
        /// equatorial radii, positive north.
        gamma: f64,
        /// The share of the Moon's diameter in the penumbra.
        penumbral: f64,
        /// And in the umbra.
        umbral: f64,
    },
}

impl Eclipse {
    /// When it is greatest, s on the ephemeris's clock.
    pub fn time(&self) -> f64 {
        match *self {
            Self::Solar { time, .. } | Self::Lunar { time, .. } => time,
        }
    }
}

/// The Moon's shadow on the fundamental plane, through Earth's center
/// across the shadow's axis, in Earth equatorial radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoonShadow {
    /// Where the axis crosses the plane: east of Earth's center.
    pub x: f64,
    /// And north.
    pub y: f64,
    /// The penumbra's radius.
    pub l1: f64,
    /// The umbra's: negative where the umbra reaches the plane, positive
    /// where only the antumbra does.
    pub l2: f64,
    /// The tangent of the penumbral cone's half-angle.
    pub tan_f1: f64,
    /// And of the umbral cone's.
    pub tan_f2: f64,
    /// The axis's direction, from the Sun past the Moon.
    pub axis: DVec3,
    /// Earth's outline on the plane, flattened: its north semi-axis
    /// (equatorial 1).
    pub rho: f64,
}

impl MoonShadow {
    /// From the apparent positions (m) of the Sun and Moon.
    pub fn new(sun: DVec3, moon: DVec3) -> Self {
        let axis = (moon - sun).normalize();
        let separation = (moon - sun).length();
        let r_penumbra = K_PENUMBRA * EARTH_EQUATORIAL_RADIUS;
        let r_umbra = K_UMBRA * EARTH_EQUATORIAL_RADIUS;
        let f1 = ((SUN_RADIUS + r_penumbra) / separation).asin();
        let f2 = ((SUN_RADIUS - r_umbra) / separation).asin();
        // The Moon's height above the plane, toward the Sun.
        let height = -moon.dot(axis);
        let l1 = (height * f1.tan() + r_penumbra / f1.cos()) / EARTH_EQUATORIAL_RADIUS;
        let l2 = (height * f2.tan() - r_umbra / f2.cos()) / EARTH_EQUATORIAL_RADIUS;
        // The plane's axes: north as Earth's pole projected onto it, east
        // completing them.
        let north_axis = (north() - axis * north().dot(axis)).normalize();
        let east = axis.cross(north_axis);
        let crossing = (moon - axis * moon.dot(axis)) / EARTH_EQUATORIAL_RADIUS;
        // cos²d of the axis's declination, for Earth's outline: ρ₁ =
        // √(1 − e² cos² d).
        let e2 = EARTH_FLATTENING * (2.0 - EARTH_FLATTENING);
        let sin_d = -axis.dot(north());
        Self {
            x: crossing.dot(east),
            y: crossing.dot(north_axis),
            l1,
            l2,
            tan_f1: f1.tan(),
            tan_f2: f2.tan(),
            axis,
            rho: (1.0 - e2 * (1.0 - sin_d * sin_d)).sqrt(),
        }
    }

    /// The axis's distance from Earth's center, Earth equatorial radii.
    pub fn gamma(&self) -> f64 {
        self.x.hypot(self.y)
    }

    /// The eclipse it makes, if any: its kind and the magnitude where it
    /// is greatest. The axis meets the flattened Earth where x² + (y/ρ)² <
    /// 1; there the observer stands ζ above the plane, toward the Moon, and
    /// the umbra's radius is l₂ − ζ tan f₂.
    pub fn eclipse(&self) -> Option<(SolarKind, f64)> {
        let reach = self.x.hypot(self.y / self.rho);
        if reach >= 1.0 + self.l1 {
            return None;
        }
        Some(if reach < 1.0 {
            let zeta = (1.0 - reach * reach).sqrt();
            let l1 = self.l1 - zeta * self.tan_f1;
            let l2 = self.l2 - zeta * self.tan_f2;
            let kind = match (l2 < 0.0, self.l2 < 0.0) {
                (true, true) => SolarKind::Total,
                (true, false) => SolarKind::Hybrid,
                _ => SolarKind::Annular,
            };
            // On the axis, the ratio of the Moon's apparent diameter to
            // the Sun's, as the canon gives a central eclipse's magnitude.
            (kind, (l1 - l2) / (l1 + l2))
        } else {
            // The axis misses Earth: the eclipse is greatest on its limb
            // nearest the axis, on the plane.
            let magnitude = (self.l1 - (reach - 1.0)) / (self.l1 + self.l2);
            let kind = if reach < 1.0 + self.l2.abs() {
                if self.l2 < 0.0 {
                    SolarKind::Total
                } else {
                    SolarKind::Annular
                }
            } else {
                SolarKind::Partial
            };
            (kind, magnitude)
        })
    }
}

/// Earth's shadow where the Moon is, as seen from Earth's center: angles,
/// radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EarthShadow {
    /// The Moon's center's angular distance from the shadow's axis.
    pub sigma: f64,
    /// The penumbra's radius, by Danjon's rule: 1.01 π_moon + s_sun +
    /// π_sun.
    pub penumbra: f64,
    /// The umbra's: 1.01 π_moon − s_sun + π_sun.
    pub umbra: f64,
    /// The Moon's semidiameter.
    pub moon: f64,
    /// The Moon's distance from the axis, Earth equatorial radii, positive
    /// north.
    pub gamma: f64,
}

impl EarthShadow {
    /// From the apparent positions (m) of the Sun and Moon.
    pub fn new(sun: DVec3, moon: DVec3) -> Self {
        let anti_sun = -sun.normalize();
        let (d_sun, d_moon) = (sun.length(), moon.length());
        let parallax_moon = (EARTH_EQUATORIAL_RADIUS / d_moon).asin();
        let parallax_sun = (EARTH_EQUATORIAL_RADIUS / d_sun).asin();
        let sun_radius = (SUN_RADIUS / d_sun).asin();
        let off = moon - anti_sun * moon.dot(anti_sun);
        let north_axis = (north() - anti_sun * north().dot(anti_sun)).normalize();
        let gamma = off.length() / EARTH_EQUATORIAL_RADIUS;
        Self {
            sigma: moon.normalize().dot(anti_sun).clamp(-1.0, 1.0).acos(),
            penumbra: DANJON * parallax_moon + sun_radius + parallax_sun,
            umbra: DANJON * parallax_moon - sun_radius + parallax_sun,
            moon: (K_MEAN * EARTH_EQUATORIAL_RADIUS / d_moon).asin(),
            gamma: gamma.copysign(off.dot(north_axis)),
        }
    }

    /// The shares of the Moon's diameter in the penumbra and umbra:
    /// (R + s − σ)/2s.
    pub fn magnitudes(&self) -> (f64, f64) {
        let share = |radius: f64| (radius + self.moon - self.sigma) / (2.0 * self.moon);
        (share(self.penumbra), share(self.umbra))
    }

    /// The eclipse it makes, if any.
    pub fn eclipse(&self) -> Option<LunarKind> {
        let (penumbral, umbral) = self.magnitudes();
        if umbral >= 1.0 {
            Some(LunarKind::Total)
        } else if umbral > 0.0 {
            Some(LunarKind::Partial)
        } else if penumbral > 0.0 {
            Some(LunarKind::Penumbral)
        } else {
            None
        }
    }
}

/// A shadow's cone, tangent to the Sun and to the body casting it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cone {
    /// Its vertex: beyond the body for the umbra, which narrows to it;
    /// between the body and the Sun for the penumbra, which widens from it.
    pub vertex: DVec3,
    /// The direction away from the Sun.
    pub axis: DVec3,
    /// The tangent of its half-angle.
    pub tan_half_angle: f64,
}

impl Cone {
    /// Its radius `along` m from the vertex, away from the Sun (the umbra's
    /// is on the Sun's side of its vertex: `along` < 0).
    pub fn radius(&self, along: f64) -> f64 {
        along.abs() * self.tan_half_angle
    }
}

/// The penumbra and umbra a body of `radius` (m) at `body` casts from the
/// Sun at `sun`: the cones tangent to both spheres, crossing between them
/// for the penumbra (sin f = (R☉ + r)/d) and not for the umbra (sin f =
/// (R☉ − r)/d). Each vertex is r / sin f from the body's center.
pub fn shadow_cones(sun: DVec3, body: DVec3, radius: f64) -> (Cone, Cone) {
    let axis = (body - sun).normalize();
    let separation = (body - sun).length();
    let cone = |sin_f: f64, side: f64| Cone {
        vertex: body + axis * (side * radius / sin_f),
        axis,
        tan_half_angle: sin_f / (1.0 - sin_f * sin_f).sqrt(),
    };
    (
        cone((SUN_RADIUS + radius) / separation, -1.0),
        cone((SUN_RADIUS - radius) / separation, 1.0),
    )
}

/// The Sun, Earth and Moon sampled at even times, interpolated between:
/// cubic Hermite from the positions and velocities at each end. The Moon
/// sampled every 6 h is good to about 10 m that way.
#[derive(Debug, Clone, Default)]
pub struct Ephemeris {
    /// s, evenly spaced.
    pub times: Vec<f64>,
    /// Each sample's Sun, Earth and Moon.
    pub states: Vec<[State; 3]>,
}

impl Ephemeris {
    /// The Sun, Earth and Moon at time `t`, within the samples.
    pub fn at(&self, t: f64) -> [State; 3] {
        let n = self.times.len();
        let k = self.times.partition_point(|&s| s <= t).clamp(1, n - 1);
        let (t0, t1) = (self.times[k - 1], self.times[k]);
        let h = t1 - t0;
        let s = (t - t0) / h;
        let (a, b) = (self.states[k - 1], self.states[k]);
        std::array::from_fn(|i| {
            let (p0, v0, p1, v1) = (
                a[i].position,
                a[i].velocity * h,
                b[i].position,
                b[i].velocity * h,
            );
            let (s2, s3) = (s * s, s * s * s);
            let position = p0 * (2.0 * s3 - 3.0 * s2 + 1.0)
                + v0 * (s3 - 2.0 * s2 + s)
                + p1 * (-2.0 * s3 + 3.0 * s2)
                + v1 * (s3 - s2);
            let velocity = (p0 * (6.0 * s2 - 6.0 * s)
                + v0 * (3.0 * s2 - 4.0 * s + 1.0)
                + p1 * (-6.0 * s2 + 6.0 * s)
                + v1 * (3.0 * s2 - 2.0 * s))
                / h;
            State { position, velocity }
        })
    }

    /// The apparent Sun and Moon at time `t`.
    pub fn apparent(&self, t: f64) -> (DVec3, DVec3) {
        let [sun, earth, moon] = self.at(t);
        apparent(sun, earth, moon)
    }

    /// Every eclipse within the samples, in order of time, each at its
    /// greatest (to 0.01 s).
    pub fn eclipses(&self) -> Vec<Eclipse> {
        let solar = |t: f64| {
            let (sun, moon) = self.apparent(t);
            (sun.dot(moon) > 0.0).then(|| MoonShadow::new(sun, moon).gamma())
        };
        let lunar = |t: f64| {
            let (sun, moon) = self.apparent(t);
            (sun.dot(moon) < 0.0).then(|| EarthShadow::new(sun, moon).gamma.abs())
        };
        let mut found = Vec::new();
        for t in least(&self.times, solar) {
            let (sun, moon) = self.apparent(t);
            let shadow = MoonShadow::new(sun, moon);
            if let Some((kind, magnitude)) = shadow.eclipse() {
                found.push(Eclipse::Solar {
                    time: t,
                    kind,
                    gamma: shadow.gamma().copysign(shadow.y),
                    magnitude,
                });
            }
        }
        for t in least(&self.times, lunar) {
            let (sun, moon) = self.apparent(t);
            let shadow = EarthShadow::new(sun, moon);
            if let Some(kind) = shadow.eclipse() {
                let (penumbral, umbral) = shadow.magnitudes();
                found.push(Eclipse::Lunar {
                    time: t,
                    kind,
                    gamma: shadow.gamma,
                    penumbral,
                    umbral,
                });
            }
        }
        found.sort_by(|a, b| a.time().total_cmp(&b.time()));
        found
    }
}

/// The times where `f` (None where it doesn't apply) is least: at each
/// sample lower than both neighbours, narrowed by golden-section search
/// between them to 0.01 s.
fn least(times: &[f64], f: impl Fn(f64) -> Option<f64>) -> Vec<f64> {
    let values: Vec<Option<f64>> = times.iter().map(|&t| f(t)).collect();
    let mut minima = Vec::new();
    for k in 1..times.len().saturating_sub(1) {
        let (Some(a), Some(b), Some(c)) = (values[k - 1], values[k], values[k + 1]) else {
            continue;
        };
        if b > a || b > c {
            continue;
        }
        let ratio = 0.5 * (5f64.sqrt() - 1.0);
        let (mut lo, mut hi) = (times[k - 1], times[k + 1]);
        let value = |t: f64| f(t).unwrap_or(f64::INFINITY);
        let (mut x1, mut x2) = (hi - ratio * (hi - lo), lo + ratio * (hi - lo));
        let (mut f1, mut f2) = (value(x1), value(x2));
        while hi - lo > 0.01 {
            if f1 < f2 {
                hi = x2;
                (x2, f2) = (x1, f1);
                x1 = hi - ratio * (hi - lo);
                f1 = value(x1);
            } else {
                lo = x1;
                (x1, f1) = (x2, f2);
                x2 = lo + ratio * (hi - lo);
                f2 = value(x2);
            }
        }
        minima.push(0.5 * (lo + hi));
    }
    minima
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolation_is_exact_for_a_cubic() {
        // x(t) = t³ − t: Hermite from values and slopes reproduces it.
        let path = |t: f64| State {
            position: DVec3::splat(t * t * t - t),
            velocity: DVec3::splat(3.0 * t * t - 1.0),
        };
        let ephemeris = Ephemeris {
            times: vec![0.0, 2.0, 4.0],
            states: [0.0, 2.0, 4.0].map(|t| [path(t); 3]).to_vec(),
        };
        for t in [0.3, 1.7, 3.1] {
            let [s, ..] = ephemeris.at(t);
            assert!((s.position - path(t).position).length() < 1e-12);
            assert!((s.velocity - path(t).velocity).length() < 1e-12);
        }
    }

    #[test]
    fn the_moon_dead_ahead_makes_a_total_or_annular_eclipse() {
        // Sun 1 AU away along x; the Moon 384,400 km along x: the axis
        // through Earth's center, γ = 0. Nearer, the umbra reaches Earth
        // (total); farther, only the antumbra (annular).
        let sun = DVec3::X * 1.495_978_707e11;
        for (distance, expected) in [(3.6e8, SolarKind::Total), (4.06e8, SolarKind::Annular)] {
            let shadow = MoonShadow::new(sun, DVec3::X * distance);
            assert!(shadow.gamma() < 1e-12);
            let (kind, magnitude) = shadow.eclipse().unwrap();
            assert_eq!(kind, expected, "{distance}");
            assert_eq!(magnitude > 1.0, expected == SolarKind::Total);
        }
        // Far off the line: none.
        let off = DVec3::new(3.84e8, 3e7, 0.0);
        assert!(MoonShadow::new(sun, off).eclipse().is_none());
    }

    #[test]
    fn the_moon_at_the_shadows_axis_is_totally_eclipsed() {
        // Full Moon dead opposite the Sun: σ = 0, umbral magnitude
        // (R_u + s)/2s, about 1.84 (the canon's most central eclipse of
        // 2026–2030, γ = 0.0124 on 2029 Jun 26, has 1.8436).
        let shadow = EarthShadow::new(DVec3::X * 1.496e11, -DVec3::X * 3.844e8);
        assert!(shadow.sigma.abs() < 1e-12 && shadow.gamma.abs() < 1e-12);
        let (penumbral, umbral) = shadow.magnitudes();
        assert!(
            (umbral - 1.84).abs() < 0.02 && penumbral > umbral,
            "{umbral}"
        );
        assert_eq!(shadow.eclipse(), Some(LunarKind::Total));
        // Danjon: the umbra's radius 1.01 × 57′ − 16′ + 9″, about 41.6′.
        let arcmin = shadow.umbra.to_degrees() * 60.0;
        assert!((arcmin - 41.6).abs() < 0.3, "{arcmin}");
    }

    #[test]
    fn the_moon_covers_the_sun_by_the_area_they_share() {
        let (sun, moon) = (0.00465, 0.0047);
        assert_eq!(share_of_sun_covered(0.02, sun, moon), 0.0);
        assert_eq!(share_of_sun_covered(0.0, sun, moon), 1.0);
        // Annular: the Moon's area over the Sun's.
        let small = 0.004;
        let ratio = (small / sun) * (small / sun);
        assert_eq!(share_of_sun_covered(0.0, sun, small), ratio);
        // Equal disks, one radius apart: the textbook lens, 2/3 − √3/(2π)
        // of a disk, about 0.391.
        let share = share_of_sun_covered(sun, sun, sun);
        let expected = 2.0 / 3.0 - 3f64.sqrt() / (2.0 * std::f64::consts::PI);
        assert!((share - expected).abs() < 1e-12, "{share}");
        // Just touching, the two formulas meet.
        assert!(share_of_sun_covered(sun + moon - 1e-9, sun, moon) < 1e-6);
    }

    #[test]
    fn the_cones_meet_the_fundamental_plane_as_the_besselian_radii_say() {
        // The cones drawn and the shadow the finder measures are one
        // geometry: where each cone crosses the plane through Earth's
        // center, its radius is l₁ (penumbra) and −l₂ (umbra) Earth radii,
        // to rounding. The Moon's umbra ends d r/(R☉ − r) behind it, about
        // 374,000 km at 1 AU.
        let sun = DVec3::new(1.495_978_707e11, 2e9, 0.0);
        let moon = DVec3::new(3.7e8, 1e6, 2e6);
        let shadow = MoonShadow::new(sun, moon);
        let a = EARTH_EQUATORIAL_RADIUS;
        let (penumbra, _) = shadow_cones(sun, moon, K_PENUMBRA * a);
        let (_, umbra) = shadow_cones(sun, moon, K_UMBRA * a);
        let at_plane = |cone: &Cone| cone.radius(-cone.vertex.dot(cone.axis)) / a;
        assert!((at_plane(&penumbra) - shadow.l1).abs() < 1e-12);
        assert!((at_plane(&umbra) + shadow.l2).abs() < 1e-12);
        let r = K_UMBRA * a;
        let length = (umbra.vertex - moon).length();
        let expected = (moon - sun).length() * r / (SUN_RADIUS - r);
        assert!((length / expected - 1.0).abs() < 1e-6, "{length}");
        assert!((length / 3.74e8 - 1.0).abs() < 0.01);
    }
}
