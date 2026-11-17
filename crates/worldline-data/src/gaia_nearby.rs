//! Gaia's stars near the Sun (step 7.2): every Gaia DR3 star within 50 pc
//! with a full measured motion, for the galaxy's swarm.
//!
//! `tools/fetch_gaia_nearby_stars.py` regenerates the data. See
//! `docs/physics/galaxy-stars.md`.

use worldline_core::galaxy_stars::{MEASURED, Star};
use worldline_core::{DMat3, DVec3};

use crate::neighbors::{catalog_state, equatorial_to_ecliptic, neighbors};

/// The Julian year Gaia DR3's places and motions are for.
pub const GAIA_DR3_EPOCH: f64 = 2016.0;

/// A star as Gaia DR3 measured it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaiaStar {
    /// Its Gaia DR3 source identifier.
    pub source_id: u64,
    /// Right ascension and declination (ICRS), degrees, at 2016.0.
    pub ra: f64,
    /// Declination, degrees.
    pub dec: f64,
    /// Parallax, mas.
    pub parallax: f64,
    /// Proper motion in right ascension (times cos δ) and declination,
    /// mas/yr.
    pub proper_motion: (f64, f64),
    /// Radial velocity, m/s, positive away.
    pub radial_velocity: f64,
}

/// Julian Date of a Julian year.
fn julian_date(year: f64) -> f64 {
    2_451_545.0 + (year - 2000.0) * 365.25
}

/// Every star in `gaia-nearby-stars.csv`, as Gaia measured it.
pub fn gaia_nearby_stars() -> Vec<GaiaStar> {
    include_str!("../data/gaia-nearby-stars.csv")
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let n = |i: usize| f[i].parse::<f64>().expect("a measured number");
            GaiaStar {
                source_id: f[0].parse().expect("a source id"),
                ra: n(1),
                dec: n(2),
                parallax: n(3),
                proper_motion: (n(4), n(5)),
                radial_velocity: n(6) * 1e3,
            }
        })
        .collect()
}

impl GaiaStar {
    /// Where it is and how it moves at Gaia's epoch, in equatorial axes
    /// from the solar system's barycenter (m, m/s): 1/ϖ away along its
    /// direction. Its parallax is known to better than a tenth, where 1/ϖ
    /// is a fair estimate of the distance (Bailer-Jones 2015).
    pub fn measured_state(&self) -> (DVec3, DVec3) {
        catalog_state(
            self.ra,
            self.dec,
            self.parallax,
            self.proper_motion,
            self.radial_velocity,
        )
    }

    /// Where it is at Julian Date `jd_tdb`, in the simulation's frame from
    /// the solar system's barycenter, carried in a straight line from
    /// 2016.0. Over the nine years to the simulation's start, the galaxy's
    /// tide, strongest across the disk, ν²z with ν² = 4πGρ ≈ 5.7 × 10⁻³⁰
    /// s⁻² for 0.1 Suns per cubic parsec, bends that by ½ν²t² of its
    /// distance: 2 × 10⁻¹³.
    pub fn at(&self, jd_tdb: f64) -> (DVec3, DVec3) {
        let (position, velocity) = self.measured_state();
        let since = (jd_tdb - julian_date(GAIA_DR3_EPOCH)) * 86_400.0;
        let turn = equatorial_to_ecliptic();
        (turn * (position + velocity * since), turn * velocity)
    }
}

/// Gaia's nearby stars that aren't already bodies of their own among the
/// nearest and brightest (step 7.1): those within 1′ of one of them on
/// the sky, at Gaia's epoch, and within a tenth of its distance, are left
/// out. Proxima Centauri, Barnard's Star, 61 Cygni and the others are
/// simulated once, as bodies.
pub fn gaia_swarm_stars() -> Vec<GaiaStar> {
    let jd = julian_date(GAIA_DR3_EPOCH);
    let bodies: Vec<DVec3> = neighbors(jd).iter().map(|n| n.body.position).collect();
    let arcminute = (1.0_f64 / 60.0).to_radians();
    gaia_nearby_stars()
        .into_iter()
        .filter(|s| {
            let x = s.at(jd).0;
            !bodies.iter().any(|b| {
                x.angle_between(*b) < arcminute
                    && (x.length() - b.length()).abs() < 0.1 * b.length()
            })
        })
        .collect()
}

/// Gaia's nearby stars for the galaxy's swarm, galactocentric at Julian
/// Date `jd_tdb` (see [`crate::GalacticFrame`]): each carried from 2016.0
/// and put around the galaxy's center with the solar system's barycenter
/// at `barycenter`, turned by `to_galactic`. Their population is
/// [`MEASURED`].
pub fn gaia_swarm(jd_tdb: f64, to_galactic: DMat3, barycenter: (DVec3, DVec3)) -> Vec<Star> {
    gaia_swarm_stars()
        .iter()
        .map(|s| {
            let (x, v) = s.at(jd_tdb);
            Star {
                position: barycenter.0 + to_galactic * x,
                velocity: barycenter.1 + to_galactic * v,
                population: MEASURED,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stars_read_as_gaia_gives_them() {
        let stars = gaia_nearby_stars();
        assert_eq!(stars.len(), 21_484);
        let first = stars[0];
        assert_eq!(first.source_id, 123_424_475_948_672);
        assert_eq!(first.parallax, 22.499346672897467);
        assert_eq!(first.radial_velocity, 34.41306 * 1e3);
        assert!(stars.iter().all(|s| s.parallax > 20.0));
    }

    #[test]
    fn the_nearest_stars_are_left_to_step_7_1() {
        let apart = gaia_swarm_stars();
        let ids: Vec<u64> = apart.iter().map(|s| s.source_id).collect();
        // Proxima Centauri and Barnard's Star, by their Gaia DR3 sources.
        for id in [5_853_498_713_190_525_696, 4_472_832_130_942_575_872] {
            assert!(gaia_nearby_stars().iter().any(|s| s.source_id == id));
            assert!(!ids.contains(&id));
        }
    }
}
