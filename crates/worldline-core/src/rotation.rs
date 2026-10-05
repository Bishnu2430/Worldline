//! How bodies are oriented and spin.
//!
//! Uses the IAU rotation models (Archinal et al. 2018) in the form NASA's
//! SPICE toolkit evaluates them: the north pole's right ascension and
//! declination as polynomials in time, the prime meridian's rotation angle
//! W, and periodic terms for precession, nutation and libration.

use glam::{DMat3, DVec3};

use crate::constants::{DAY, J2000_JD, OBLIQUITY_J2000};

/// One periodic term: an angle that advances with time, and how strongly
/// it moves the pole and the prime meridian.
#[derive(Debug, Clone, PartialEq)]
pub struct PeriodicTerm {
    /// The angle as a polynomial in Julian centuries since J2000, in degrees:
    /// angle = c₀ + c₁T + c₂T² + …
    pub angle: Vec<f64>,
    /// Amplitude added to the pole's right ascension as `ra · sin(angle)`, in degrees.
    pub ra: f64,
    /// Amplitude added to the pole's declination as `dec · cos(angle)`, in degrees.
    pub dec: f64,
    /// Amplitude added to the prime meridian as `pm · sin(angle)`, in degrees.
    pub pm: f64,
}

/// An IAU rotation model for one body.
#[derive(Debug, Clone, PartialEq)]
pub struct RotationModel {
    /// North pole right ascension, degrees: a₀ + a₁T + a₂T² with T in Julian
    /// centuries (TDB) since J2000, in the equatorial J2000 frame.
    pub pole_ra: [f64; 3],
    /// North pole declination, degrees, in the same form.
    pub pole_dec: [f64; 3],
    /// Prime meridian angle W, degrees: w₀ + w₁d + w₂d² with d in days
    /// (TDB) since J2000. Negative w₁ means the body spins retrograde.
    pub prime_meridian: [f64; 3],
    /// Periodic corrections.
    pub terms: Vec<PeriodicTerm>,
}

/// A body's orientation at one moment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orientation {
    /// Rotates vectors from the body-fixed frame (z toward the north pole,
    /// x toward the prime meridian on the equator) into Worldline's frame
    /// (ecliptic and mean equinox of J2000).
    pub body_to_ecliptic: DMat3,
}

impl Orientation {
    /// Direction of the body's IAU north pole.
    pub fn north_pole(&self) -> DVec3 {
        self.body_to_ecliptic.z_axis
    }
}

impl RotationModel {
    /// The body's orientation at Julian Date `jd_tdb`.
    pub fn orientation(&self, jd_tdb: f64) -> Orientation {
        let d = jd_tdb - J2000_JD;
        let t = d / 36_525.0;
        let polynomial = |c: &[f64], x: f64| c.iter().rev().fold(0.0, |acc, &k| acc * x + k);

        let mut ra = polynomial(&self.pole_ra, t);
        let mut dec = polynomial(&self.pole_dec, t);
        let mut w = polynomial(&self.prime_meridian, d);
        for term in &self.terms {
            let (sin, cos) = polynomial(&term.angle, t).to_radians().sin_cos();
            ra += term.ra * sin;
            dec += term.dec * cos;
            w += term.pm * sin;
        }

        // Body-fixed → equatorial J2000: turn by W about the pole, tilt the
        // pole down to declination δ, then swing it round to right
        // ascension α (the equator's ascending node lies at α + 90°).
        let equatorial = DMat3::from_rotation_z((ra + 90.0).to_radians())
            * DMat3::from_rotation_x((90.0 - dec).to_radians())
            * DMat3::from_rotation_z(w.rem_euclid(360.0).to_radians());
        // Equatorial → ecliptic: tilt back by the obliquity.
        Orientation {
            body_to_ecliptic: DMat3::from_rotation_x(-OBLIQUITY_J2000) * equatorial,
        }
    }

    /// Rotation rate in degrees per day. Negative for retrograde rotators
    /// such as Venus and Uranus.
    pub fn spin_rate(&self) -> f64 {
        self.prime_meridian[1]
    }

    /// Sidereal rotation period, in s: one turn relative to the stars.
    pub fn sidereal_period(&self) -> f64 {
        360.0 / self.spin_rate().abs() * DAY
    }

    /// Direction of the body's spin (its angular momentum, by the
    /// right-hand rule). That's the north pole for bodies that spin
    /// prograde and the south pole for retrograde ones.
    pub fn spin_axis(&self, jd_tdb: f64) -> DVec3 {
        self.orientation(jd_tdb).north_pole() * self.spin_rate().signum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn earth_like() -> RotationModel {
        RotationModel {
            pole_ra: [0.0, 0.0, 0.0],
            pole_dec: [90.0, 0.0, 0.0],
            prime_meridian: [190.147, 360.985_623_5, 0.0],
            terms: Vec::new(),
        }
    }

    #[test]
    fn orientation_is_a_proper_rotation() {
        let m = earth_like().orientation(J2000_JD + 1234.5).body_to_ecliptic;
        assert!(
            (m * m.transpose() - DMat3::IDENTITY)
                .abs()
                .to_cols_array()
                .iter()
                .all(|&x| x < 1e-14)
        );
        assert!((m.determinant() - 1.0).abs() < 1e-14);
    }

    #[test]
    fn a_pole_at_the_celestial_pole_is_tilted_by_the_obliquity() {
        // Earth's pole points at Dec 90° in the equatorial frame, so in the
        // ecliptic frame it leans by exactly the obliquity, toward +y.
        let pole = earth_like().orientation(J2000_JD).north_pole();
        let tilt = pole.angle_between(DVec3::Z);
        assert!((tilt - OBLIQUITY_J2000).abs() < 1e-12);
        assert!(pole.y > 0.0);
    }

    #[test]
    fn prime_meridian_turns_once_per_sidereal_day() {
        let model = earth_like();
        let period_days = model.sidereal_period() / DAY;
        let a = model.orientation(J2000_JD + 100.0).body_to_ecliptic.x_axis;
        let b = model
            .orientation(J2000_JD + 100.0 + period_days)
            .body_to_ecliptic
            .x_axis;
        assert!(a.angle_between(b) < 1e-9);
        // 86164.1 s: the sidereal day.
        assert!((model.sidereal_period() - 86_164.1).abs() < 0.1);
    }

    #[test]
    fn periodic_terms_follow_the_spice_convention() {
        // A single term whose angle is 90° at J2000 moves RA by +ra (sine),
        // leaves Dec alone (cosine of 90° is 0), and moves W by +pm (sine).
        let mut model = earth_like();
        model.pole_dec = [60.0, 0.0, 0.0];
        let plain = model.orientation(J2000_JD);
        model.terms.push(PeriodicTerm {
            angle: vec![90.0, 0.0],
            ra: 2.0,
            dec: 5.0,
            pm: 3.0,
        });
        let shifted = model.orientation(J2000_JD);
        let pole_shift = plain
            .north_pole()
            .angle_between(shifted.north_pole())
            .to_degrees();
        // Moving RA by 2° at Dec 60° moves the pole 2° × cos 60° = 1° on the sky.
        assert!((pole_shift - 1.0).abs() < 1e-3, "pole moved {pole_shift}°");
    }
}
