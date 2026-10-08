//! Moons placed by JPL's mean orbital elements: a cheap, approximate model
//! for where a moon is when nobody is looking closely.
//!
//! JPL's Solar System Dynamics group publishes, for every planetary
//! satellite, the elements of an ellipse fitted to its long-term motion:
//! size and shape (a, e), tilt and node (i, Ω), periapsis (ω), mean anomaly
//! (M) at an epoch, the sidereal period, and the periods at which the
//! periapsis and node precess. Advancing the angles at those steady rates
//! gives the moon's position at any date to within what the averaging
//! leaves out: short-period wobbles, and the slow drift of the rates
//! themselves. See `docs/physics/small-moons.md`.

use std::f64::consts::TAU;

use glam::{DMat3, DVec3};

/// A moon's mean orbit.
#[derive(Debug, Clone, PartialEq)]
pub struct MeanElements {
    /// Rotates vectors from the reference plane's frame (x toward the
    /// plane's ascending node on the ICRF equator, z along the plane's
    /// pole) into the simulation frame.
    pub frame: DMat3,
    /// Simulation time of the elements, in s.
    pub epoch: f64,
    /// Semi-major axis, in m.
    pub a: f64,
    /// Eccentricity.
    pub e: f64,
    /// Inclination to the reference plane, in rad.
    pub inclination: f64,
    /// Longitude of the ascending node at the epoch, in rad.
    pub node: f64,
    /// Argument of periapsis at the epoch, in rad.
    pub periapsis: f64,
    /// Mean anomaly at the epoch, in rad.
    pub mean_anomaly: f64,
    /// Sidereal period: the time to come back to the same direction in
    /// space, in s.
    pub period: f64,
    /// Rate at which the argument of periapsis turns, in rad/s.
    pub periapsis_rate: f64,
    /// Rate at which the node turns, in rad/s.
    pub node_rate: f64,
}

impl MeanElements {
    /// The osculating ellipse through position `r` (m) and velocity `v`
    /// (m/s) relative to a planet with gravitational parameter `mu`
    /// (m³/s²), at simulation time `epoch`, with angles measured in the
    /// reference `frame` (see [`Self::frame`]). The period is Kepler's, and
    /// nothing precesses: set those afterwards to taste.
    ///
    /// Angles that aren't defined (the node of an orbit in the reference
    /// plane, the periapsis of a circle) are measured from the frame's x
    /// axis and the node instead, which places the body the same way.
    pub fn osculating(r: DVec3, v: DVec3, mu: f64, frame: DMat3, epoch: f64) -> Self {
        let (r, v) = (frame.transpose() * r, frame.transpose() * v);
        let h = r.cross(v);
        let normal = h.normalize();
        let inclination = normal.z.clamp(-1.0, 1.0).acos();
        let node_direction = DVec3::Z.cross(normal).try_normalize().unwrap_or(DVec3::X);
        let node = node_direction.y.atan2(node_direction.x);
        let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        let e_vec = ((v.length_squared() - mu / r.length()) * r - r.dot(v) * v) / mu;
        let e = e_vec.length();
        // Angles in the orbit's plane, from the node.
        let across = normal.cross(node_direction);
        let angle = |w: DVec3| w.dot(across).atan2(w.dot(node_direction));
        let periapsis = if e > 1e-12 { angle(e_vec) } else { 0.0 };
        let true_anomaly = angle(r) - periapsis;
        let eccentric = 2.0 * (((1.0 - e) / (1.0 + e)).sqrt() * (true_anomaly / 2.0).tan()).atan();
        Self {
            frame,
            epoch,
            a,
            e,
            inclination,
            node,
            periapsis,
            mean_anomaly: (eccentric - e * eccentric.sin()).rem_euclid(TAU),
            period: TAU * (a.powi(3) / mu).sqrt(),
            periapsis_rate: 0.0,
            node_rate: 0.0,
        }
    }

    /// Precession rates from JPL's precession periods (s), which are listed
    /// without a sign. `None` (or zero, which JPL lists where the angle
    /// isn't defined, as for a circular or uninclined orbit) means no
    /// steady precession: the periapsis of a moon caught in a Kozai cycle
    /// librates rather than circulating.
    ///
    /// The directions come from the physics that drives them. Both a
    /// planet's flattening and the Sun's tide turn the node backwards
    /// for a prograde orbit and forwards for a retrograde one, as −cos i
    /// (Murray & Dermott 1999, eq. 6.249; Kozai 1962). Both turn the
    /// periapsis forwards for the orbits in JPL's list: J2 does so for
    /// inclinations below 63.4° or above 116.6°, and the solar tide does
    /// so whenever the periapsis circulates rather than librates.
    pub fn with_precession_periods(
        mut self,
        periapsis_period: Option<f64>,
        node_period: Option<f64>,
    ) -> Self {
        let rate = |period: Option<f64>| period.filter(|&p| p > 0.0).map_or(0.0, |p| TAU / p);
        self.periapsis_rate = rate(periapsis_period);
        self.node_rate = -self.inclination.cos().signum() * rate(node_period);
        self
    }

    /// Position (m) and velocity (m/s) relative to the planet at
    /// simulation time `time` (s).
    ///
    /// The mean anomaly advances so that the moon's direction around the
    /// planet repeats every sidereal period whatever the precession:
    /// Ṁ = 2π/P − ω̇ − Ω̇ cos-sign(i). The speed follows Kepler's laws with
    /// the effective gravity μ = (2π/P)² a³ that the observed period
    /// implies, so it already includes the planet's flattening.
    pub fn state_at(&self, time: f64) -> (DVec3, DVec3) {
        let dt = time - self.epoch;
        let n = TAU / self.period;
        let along = self.inclination.cos().signum();
        let anomaly_rate = n - self.periapsis_rate - along * self.node_rate;
        let mean_anomaly = (self.mean_anomaly + anomaly_rate * dt).rem_euclid(TAU);
        let periapsis = self.periapsis + self.periapsis_rate * dt;
        let node = self.node + self.node_rate * dt;

        let e = self.e;
        let eccentric = solve_kepler(mean_anomaly, e);
        let (sin_e, cos_e) = eccentric.sin_cos();
        let root = (1.0 - e * e).sqrt();
        let position = DVec3::new(self.a * (cos_e - e), self.a * root * sin_e, 0.0);
        let rate = n / (1.0 - e * cos_e);
        let velocity = DVec3::new(-self.a * sin_e * rate, self.a * root * cos_e * rate, 0.0);

        let orientation = self.frame
            * DMat3::from_rotation_z(node)
            * DMat3::from_rotation_x(self.inclination)
            * DMat3::from_rotation_z(periapsis);
        (orientation * position, orientation * velocity)
    }
}

/// The reference frame of elements given relative to a plane whose pole
/// points at right ascension `ra` and declination `dec` (rad, ICRF), with
/// the node measured from the plane's ascending node on the ICRF equator,
/// expressed in the ecliptic frame with obliquity `obliquity` (rad).
pub fn plane_frame(ra: f64, dec: f64, obliquity: f64) -> DMat3 {
    DMat3::from_rotation_x(-obliquity)
        * DMat3::from_rotation_z(ra + std::f64::consts::FRAC_PI_2)
        * DMat3::from_rotation_x(std::f64::consts::FRAC_PI_2 - dec)
}

/// Solves Kepler's equation E − e sin E = M for the eccentric anomaly, by
/// Newton's method from a starting guess that converges for all e < 1.
pub fn solve_kepler(mean_anomaly: f64, e: f64) -> f64 {
    let mut eccentric = if e < 0.8 {
        mean_anomaly
    } else {
        std::f64::consts::PI
    };
    for _ in 0..50 {
        let step = (eccentric - e * eccentric.sin() - mean_anomaly) / (1.0 - e * eccentric.cos());
        eccentric -= step;
        if step.abs() < 1e-15 {
            break;
        }
    }
    eccentric
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::{DAY, OBLIQUITY_J2000};

    fn circle() -> MeanElements {
        MeanElements {
            frame: DMat3::IDENTITY,
            epoch: 0.0,
            a: 4.218e8,
            e: 0.0,
            inclination: 0.0,
            node: 0.0,
            periapsis: 0.0,
            mean_anomaly: 0.0,
            period: 1.769 * DAY,
            periapsis_rate: 0.0,
            node_rate: 0.0,
        }
    }

    #[test]
    fn keplers_equation_is_solved() {
        for e in [0.0, 0.3, 0.75, 0.95] {
            for m in [0.1, 1.0, 3.0, 5.5] {
                let big_e = solve_kepler(m, e);
                assert!((big_e - e * big_e.sin() - m).abs() < 1e-13, "e {e}, M {m}");
            }
        }
    }

    #[test]
    fn a_precessing_orbit_still_comes_round_once_per_sidereal_period() {
        // Whatever its node and periapsis do, a circular orbit in its
        // reference plane returns to the same direction after one sidereal
        // period, whether it goes round prograde (i = 0) or retrograde
        // (i = 180°).
        for inclination in [0.0, std::f64::consts::PI] {
            let orbit = MeanElements {
                inclination,
                ..circle()
            }
            .with_precession_periods(Some(30.0 * DAY), Some(60.0 * DAY));
            let (start, _) = orbit.state_at(0.0);
            let (end, _) = orbit.state_at(orbit.period);
            let drift = start.angle_between(end);
            assert!(drift < 1e-9, "drift {drift} rad");
            // And a quarter period on, it has gone a quarter turn the
            // right way round.
            let (quarter, _) = orbit.state_at(orbit.period / 4.0);
            let turned = start.cross(quarter).z.signum();
            assert_eq!(turned, inclination.cos().signum());
        }
    }

    #[test]
    fn nodes_regress_for_prograde_orbits_and_advance_for_retrograde() {
        let prograde = MeanElements {
            inclination: 0.5,
            ..circle()
        }
        .with_precession_periods(Some(10.0), Some(10.0));
        let retrograde = MeanElements {
            inclination: 2.6,
            ..circle()
        }
        .with_precession_periods(Some(10.0), Some(10.0));
        assert!(prograde.node_rate < 0.0 && retrograde.node_rate > 0.0);
        assert!(prograde.periapsis_rate > 0.0 && retrograde.periapsis_rate > 0.0);
    }

    #[test]
    fn the_state_matches_an_ellipse_at_periapsis() {
        // At periapsis: distance a(1 − e), speed n a √((1 + e)/(1 − e)).
        let orbit = MeanElements { e: 0.4, ..circle() };
        let (r, v) = orbit.state_at(0.0);
        let n = TAU / orbit.period;
        assert!((r.length() - orbit.a * 0.6).abs() < 1e-6);
        assert!((v.length() - n * orbit.a * (1.4f64 / 0.6).sqrt()).abs() < 1e-9 * v.length());
    }

    #[test]
    fn an_osculating_ellipse_passes_back_through_its_state() {
        // From a state to elements and back must return the same state, for
        // prograde, retrograde, eccentric, circular and tilted orbits, in a
        // tilted reference frame.
        let mu = 3.79e16;
        let frame = plane_frame(0.7, 1.2, OBLIQUITY_J2000);
        for (r, v) in [
            (
                DVec3::new(1.5e8, 2.0e7, 3.0e6),
                DVec3::new(-2.0e3, 1.5e4, 400.0),
            ),
            (
                DVec3::new(-1.0e10, 4.0e9, 2.0e9),
                DVec3::new(300.0, -1.2e3, 500.0),
            ),
            (
                DVec3::new(2.0e8, 0.0, 0.0),
                DVec3::new(0.0, -(mu / 2.0e8f64).sqrt(), 0.0),
            ),
        ] {
            let orbit = MeanElements::osculating(r, v, mu, frame, 100.0);
            let (r2, v2) = orbit.state_at(100.0);
            assert!((r2 - r).length() < 1e-9 * r.length(), "{r} vs {r2}");
            assert!((v2 - v).length() < 1e-9 * v.length(), "{v} vs {v2}");
        }
    }

    #[test]
    fn a_plane_frame_puts_its_pole_where_asked() {
        // The ICRF pole itself, seen from the ecliptic, leans by the
        // obliquity toward +y.
        let frame = plane_frame(0.0, std::f64::consts::FRAC_PI_2, OBLIQUITY_J2000);
        let pole = frame.z_axis;
        assert!((pole.angle_between(DVec3::Z) - OBLIQUITY_J2000).abs() < 1e-12);
        assert!(pole.y > 0.0);
    }
}
