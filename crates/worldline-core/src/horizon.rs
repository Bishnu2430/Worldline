//! The sky from a place on Earth (step 5.4): altitude and azimuth, the
//! atmosphere's refraction, and when the Sun and Moon rise and set. See
//! `docs/physics/sky-from-earth.md`.
//!
//! Rise and set are as the U.S. Naval Observatory defines them, for an
//! observer on a level horizon at sea level under average conditions:
//! - the Sun's center is 90.8333° from the zenith: 16′ for its apparent
//!   radius and 34′ for refraction;
//! - the Moon's center is 90.5666° + its apparent radius − its horizontal
//!   parallax from the zenith, worked out from Earth's center.
//!
//! Real rise times vary with the air by a minute or more, as the
//! Observatory says; these are the calculation's times, not a forecast.

use glam::{DMat3, DVec3};

use crate::eclipses::{EARTH_EQUATORIAL_RADIUS, Ephemeris, K_MEAN};
use crate::shadow_on_earth::Place;

/// Where something stands in the sky: radians.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Horizontal {
    /// Above the horizon.
    pub altitude: f64,
    /// From north, through east.
    pub azimuth: f64,
}

/// The directions at a place on Earth, in Worldline's frame: unit vectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalFrame {
    /// Along the ground, toward the north pole.
    pub north: DVec3,
    /// Along the ground, eastward.
    pub east: DVec3,
    /// Straight up: square to the ellipsoid, so at the place's geodetic
    /// latitude.
    pub zenith: DVec3,
}

impl LocalFrame {
    /// At `place`, with Earth oriented by `body_to_ecliptic`.
    pub fn new(place: Place, body_to_ecliptic: DMat3) -> Self {
        let (sin_lat, cos_lat) = place.latitude.sin_cos();
        let (sin_lon, cos_lon) = place.longitude.sin_cos();
        Self {
            north: body_to_ecliptic * DVec3::new(-sin_lat * cos_lon, -sin_lat * sin_lon, cos_lat),
            east: body_to_ecliptic * DVec3::new(-sin_lon, cos_lon, 0.0),
            zenith: body_to_ecliptic * DVec3::new(cos_lat * cos_lon, cos_lat * sin_lon, sin_lat),
        }
    }

    /// Where `direction` stands in this place's sky.
    pub fn horizontal(&self, direction: DVec3) -> Horizontal {
        let unit = direction.normalize();
        Horizontal {
            altitude: unit.dot(self.zenith).clamp(-1.0, 1.0).asin(),
            azimuth: unit
                .dot(self.east)
                .atan2(unit.dot(self.north))
                .rem_euclid(std::f64::consts::TAU),
        }
    }

    /// The direction of a point in this place's sky.
    pub fn direction(&self, horizontal: Horizontal) -> DVec3 {
        let (sin_alt, cos_alt) = horizontal.altitude.sin_cos();
        let (sin_az, cos_az) = horizontal.azimuth.sin_cos();
        (self.north * cos_az + self.east * sin_az) * cos_alt + self.zenith * sin_alt
    }
}

/// How much the air lifts something at true altitude `altitude` (radians),
/// radians: Sæmundsson's (1986) formula (Meeus, *Astronomical Algorithms*,
/// eq. 16.4), R = 1.02′ / tan(h + 10.3 / (h + 5.11)), h in degrees, for
/// 10 °C and 1010 hPa. It lifts the true horizon's −0.57° to about 0, the
/// Observatory's 34′. Below −2° nothing is seen, and the formula's pole at
/// −5.11° is near, so it is held there.
pub fn refraction(altitude: f64) -> f64 {
    let h = altitude.to_degrees().max(-2.0);
    (1.02 / (h + 10.3 / (h + 5.11)).to_radians().tan() / 60.0).to_radians()
}

/// The Sun or the Moon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Luminary {
    /// The Sun.
    Sun,
    /// The Moon.
    Moon,
}

/// The Sun's center's distance from the zenith at rise and set, degrees
/// (the Observatory's figure).
const SUN_ZENITH_DISTANCE: f64 = 90.8333;

/// The Moon's, before its radius and parallax, degrees (likewise): 34′ of
/// refraction.
const MOON_ZENITH_DISTANCE: f64 = 90.5666;

/// The altitude of `luminary`'s center at rise and set, radians, seen from
/// Earth's center, `distance` m away.
pub fn horizon_altitude(luminary: Luminary, distance: f64) -> f64 {
    match luminary {
        Luminary::Sun => (90.0 - SUN_ZENITH_DISTANCE).to_radians(),
        Luminary::Moon => {
            let radius = (K_MEAN * EARTH_EQUATORIAL_RADIUS / distance).asin();
            let parallax = (EARTH_EQUATORIAL_RADIUS / distance).asin();
            (90.0 - MOON_ZENITH_DISTANCE).to_radians() - radius + parallax
        }
    }
}

/// A rise or a set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// When, s on the ephemeris's clock.
    pub time: f64,
    /// Rising, or setting.
    pub rising: bool,
}

/// How often to look for a change of side: a body up (or down) for less
/// than this slips through, which takes a graze of the horizon, at high
/// latitudes only.
const SEARCH_STEP: f64 = 300.0;

/// Every rise and set of `luminary` at `place` within the ephemeris, in
/// order of time, each to 0.01 s. `earth` gives Earth's orientation
/// (`body_to_ecliptic`) at a time.
pub fn rises_and_sets(
    ephemeris: &Ephemeris,
    place: Place,
    luminary: Luminary,
    earth: impl Fn(f64) -> DMat3,
) -> Vec<Crossing> {
    // How far above its rise-and-set altitude it stands.
    let above = |t: f64| {
        let (sun, moon) = ephemeris.apparent(t);
        let seen = match luminary {
            Luminary::Sun => sun,
            Luminary::Moon => moon,
        };
        let frame = LocalFrame::new(place, earth(t));
        frame.horizontal(seen).altitude - horizon_altitude(luminary, seen.length())
    };
    let (first, last) = (
        ephemeris.times[0],
        ephemeris.times[ephemeris.times.len() - 1],
    );
    let mut found = Vec::new();
    let (mut a, mut value) = (first, above(first));
    while a < last {
        let b = (a + SEARCH_STEP).min(last);
        let next = above(b);
        if (value > 0.0) != (next > 0.0) {
            let (mut lo, mut hi) = (a, b);
            let rising = next > 0.0;
            while hi - lo > 0.01 {
                let middle = 0.5 * (lo + hi);
                if (above(middle) > 0.0) == rising {
                    hi = middle;
                } else {
                    lo = middle;
                }
            }
            found.push(Crossing {
                time: 0.5 * (lo + hi),
                rising,
            });
        }
        (a, value) = (b, next);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eclipses::State;
    use std::f64::consts::{FRAC_PI_2, TAU};

    fn place(latitude: f64, longitude: f64) -> Place {
        Place {
            latitude: latitude.to_radians(),
            longitude: longitude.to_radians(),
        }
    }

    #[test]
    fn the_compass_points_where_it_should() {
        // On the equator at Greenwich, Earth unturned: up is +x, north +z,
        // east +y.
        let frame = LocalFrame::new(place(0.0, 0.0), DMat3::IDENTITY);
        assert!((frame.zenith - DVec3::X).length() < 1e-15);
        assert!((frame.north - DVec3::Z).length() < 1e-15);
        assert!((frame.east - DVec3::Y).length() < 1e-15);
        let east = frame.horizontal(DVec3::Y);
        assert!(east.altitude.abs() < 1e-15 && (east.azimuth - FRAC_PI_2).abs() < 1e-15);
        // A place's frame is square, wherever it is, and directions go there
        // and back.
        let frame = LocalFrame::new(place(-33.9, 151.2), DMat3::from_rotation_x(0.4));
        assert!(frame.north.dot(frame.east).abs() < 1e-15);
        assert!(frame.north.dot(frame.zenith).abs() < 1e-15);
        let there = Horizontal {
            altitude: 0.3,
            azimuth: 4.0,
        };
        let back = frame.horizontal(frame.direction(there));
        assert!((back.altitude - there.altitude).abs() < 1e-14);
        assert!((back.azimuth - there.azimuth).abs() < 1e-14);
        // West of north wraps to just under a full turn.
        let wrapped = frame.horizontal(frame.north - frame.east * 0.01);
        assert!(wrapped.azimuth > 6.2 && wrapped.azimuth < TAU);
    }

    #[test]
    fn refraction_is_the_observatorys_34_arcminutes_at_the_horizon() {
        let arcmin = |degrees: f64| refraction(f64::to_radians(degrees)).to_degrees() * 60.0;
        // True altitude −34′ is lifted to the horizon.
        assert!(
            (arcmin(-34.0 / 60.0) - 34.0).abs() < 0.5,
            "{}",
            arcmin(-0.5667)
        );
        // Meeus's table: about 1′ at 45° and nothing at the zenith.
        assert!((arcmin(45.0) - 1.0).abs() < 0.05, "{}", arcmin(45.0));
        assert!(arcmin(90.0).abs() < 0.05, "{}", arcmin(90.0));
    }

    #[test]
    fn the_moon_rises_as_the_observatory_says() {
        // "The center of the Moon at rise or set is geometrically 5 to 10
        // arcminutes above the observer's 'geocentric horizon'."
        for distance in [3.63e8, 3.844e8, 4.05e8] {
            let arcmin = horizon_altitude(Luminary::Moon, distance).to_degrees() * 60.0;
            assert!((5.0..=10.0).contains(&arcmin), "{arcmin}");
        }
        let sun = horizon_altitude(Luminary::Sun, 1.5e11).to_degrees();
        assert!((sun + 0.8333).abs() < 1e-9, "{sun}");
    }

    #[test]
    fn a_still_sun_over_a_turning_earth_rises_and_sets_on_time() {
        // The Sun fixed along +x, Earth turning once in `day` beneath it: on
        // the equator at Greenwich the Sun is overhead at 0, sets 90.8333°
        // of turning later and rises 90.8333° before the next noon.
        let day = 86_400.0;
        let at = |_: f64| State {
            position: DVec3::ZERO,
            velocity: DVec3::ZERO,
        };
        let sun = State {
            position: DVec3::X * 1.496e11,
            velocity: DVec3::ZERO,
        };
        let moon = State {
            position: DVec3::Y * 3.844e8,
            velocity: DVec3::ZERO,
        };
        let ephemeris = Ephemeris {
            times: vec![0.0, day],
            states: vec![[sun, at(0.0), moon], [sun, at(day), moon]],
        };
        let found = rises_and_sets(&ephemeris, place(0.0, 0.0), Luminary::Sun, |t| {
            DMat3::from_rotation_z(TAU * t / day)
        });
        assert_eq!(found.len(), 2);
        let set = day * SUN_ZENITH_DISTANCE / 360.0;
        assert!(!found[0].rising && (found[0].time - set).abs() < 0.02);
        assert!(found[1].rising && (found[1].time - (day - set)).abs() < 0.02);
    }
}
