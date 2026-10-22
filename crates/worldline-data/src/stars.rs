//! The bright stars, for the sky behind everything: the Yale Bright Star
//! Catalogue, 5th revised edition (Hoffleit & Warren 1991), every star to
//! about magnitude 6.5.
//!
//! `tools/fetch_sky.py` regenerates the data. See
//! `docs/physics/lensing.md`.

use worldline_core::DVec3;

/// A star of the catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct BrightStar {
    /// Its Harvard Revised (HR) number.
    pub hr: u32,
    /// Its Bayer or Flamsteed name, if it has one ("9Alp CMa").
    pub name: String,
    /// Right ascension, J2000, in rad.
    pub ra: f64,
    /// Declination, J2000, in rad.
    pub dec: f64,
    /// Galactic longitude and latitude, as the catalogue gives them, in
    /// degrees: for its position in 1950.
    pub galactic: (f64, f64),
    /// Proper motion in right ascension (times cos δ) and declination, in
    /// rad per Julian year; zero where not given.
    pub proper_motion: (f64, f64),
    /// Visual magnitude.
    pub vmag: f64,
    /// B − V color, if measured.
    pub b_v: Option<f64>,
}

impl BrightStar {
    /// Its direction, in equatorial (ICRS) axes.
    pub fn equatorial(&self) -> DVec3 {
        let (sa, ca) = self.ra.sin_cos();
        let (sd, cd) = self.dec.sin_cos();
        DVec3::new(cd * ca, cd * sa, sd)
    }
}

/// The 9,096 stars of the catalogue that have positions.
pub fn bright_stars() -> Vec<BrightStar> {
    include_str!("../data/bright-stars.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let n = |i: usize| {
                f[i].trim_start_matches('+')
                    .parse::<f64>()
                    .expect("published number")
            };
            let sign = if f[5] == "-" { -1.0 } else { 1.0 };
            BrightStar {
                hr: f[0].parse().expect("an HR number"),
                name: f[1].to_string(),
                ra: ((n(2) + n(3) / 60.0 + n(4) / 3600.0) * 15.0).to_radians(),
                dec: (sign * (n(6) + n(7) / 60.0 + n(8) / 3600.0)).to_radians(),
                galactic: (n(9), n(10)),
                vmag: n(11),
                b_v: (!f[12].is_empty()).then(|| n(12)),
                proper_motion: (
                    if f[13].is_empty() {
                        0.0
                    } else {
                        (n(13) / 3600.0).to_radians()
                    },
                    if f[14].is_empty() {
                        0.0
                    } else {
                        (n(14) / 3600.0).to_radians()
                    },
                ),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equatorial_to_galactic;

    #[test]
    fn the_catalogue_loads() {
        let stars = bright_stars();
        assert_eq!(stars.len(), 9096);
        // Sirius, the brightest.
        let sirius = stars.iter().find(|s| s.hr == 2491).unwrap();
        assert_eq!(sirius.name, "9Alp CMa");
        assert_eq!(sirius.vmag, -1.46);
        assert!(stars.iter().all(|s| s.vmag >= sirius.vmag));
        // Polaris, 0.74° from the north celestial pole.
        let polaris = stars.iter().find(|s| s.hr == 424).unwrap();
        assert!((polaris.dec.to_degrees() - 89.2642).abs() < 1e-4);
    }

    #[test]
    fn its_stars_are_where_its_galactic_coordinates_put_them() {
        // The catalogue's galactic coordinates are for 1950: each star's
        // J2000 position, carried back 50 years by its proper motion and
        // turned into galactic coordinates by the Hipparcos rotation, must
        // land where they say. A wrong rotation would move them all; the
        // median must be within half their last digit, 0.005°.
        let turn = equatorial_to_galactic();
        let mut off: Vec<(f64, u32)> = bright_stars()
            .iter()
            .map(|s| {
                let (pm_ra, pm_dec) = s.proper_motion;
                let then = BrightStar {
                    ra: s.ra - 50.0 * pm_ra / s.dec.cos(),
                    dec: s.dec - 50.0 * pm_dec,
                    ..s.clone()
                };
                let g = turn * then.equatorial();
                let l = g.y.atan2(g.x).to_degrees();
                let b = g.z.asin().to_degrees();
                let dl =
                    ((l - s.galactic.0 + 180.0).rem_euclid(360.0) - 180.0) * b.to_radians().cos();
                (dl.hypot(b - s.galactic.1), s.hr)
            })
            .collect();
        off.sort_by(|a, b| a.0.total_cmp(&b.0));
        let median = off[off.len() / 2].0;
        let beyond: Vec<_> = off.iter().filter(|o| o.0 > 0.005 * 2f64.sqrt()).collect();
        println!(
            "median {median:.4}°; {} of {} stars beyond the rounding of both coordinates, the worst HR {} by {:.4}°",
            beyond.len(),
            off.len(),
            off[off.len() - 1].1,
            off[off.len() - 1].0
        );
        assert!(median < 0.005);
    }
}
