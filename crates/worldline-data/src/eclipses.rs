//! NASA's eclipses of 2026–2030, from the Five Millennium Canons of Solar
//! and Lunar Eclipses (Espenak & Meeus 2006, 2009): the reference for the
//! eclipse finder (step 5.2). See `tools/fetch_eclipses.py`.

use worldline_core::timescales::julian_date;

/// An eclipse as the canon lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonEclipse {
    /// The moment of greatest eclipse, a Julian Date in Terrestrial
    /// (Dynamical) Time.
    pub jd_td: f64,
    /// ΔT the canon assumed, s.
    pub delta_t: f64,
    /// Its Saros series.
    pub saros: u32,
    /// Its type: T, A, H or P for the Sun, T, P or N for the Moon, then the
    /// canon's qualifiers.
    pub kind: String,
    /// The least distance of the shadow's axis, Earth equatorial radii,
    /// positive north.
    pub gamma: f64,
    /// Its magnitude (solar), or penumbral and umbral magnitudes (lunar).
    pub magnitudes: Vec<f64>,
}

fn parse(text: &str) -> Vec<CanonEclipse> {
    let number = |s: &str| {
        s.trim()
            .parse::<f64>()
            .expect("bundled eclipse data is valid")
    };
    text.lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let date: Vec<u32> = f[0].split('-').map(|x| x.parse().unwrap()).collect();
            let clock: Vec<f64> = f[1].split(':').map(number).collect();
            let seconds = clock[0] * 3600.0 + clock[1] * 60.0 + clock[2];
            CanonEclipse {
                jd_td: julian_date(date[0] as i32, date[1], date[2], seconds),
                delta_t: number(f[2]),
                saros: number(f[3]) as u32,
                kind: f[4].to_string(),
                gamma: number(f[5]),
                magnitudes: f[6..].iter().map(|m| number(m)).collect(),
            }
        })
        .collect()
}

/// The solar eclipses of 2026–2030.
pub fn solar_eclipses_2026_2030() -> Vec<CanonEclipse> {
    parse(include_str!("../data/solar-eclipses-2026-2030.csv"))
}

/// The lunar eclipses of 2026–2030.
pub fn lunar_eclipses_2026_2030() -> Vec<CanonEclipse> {
    parse(include_str!("../data/lunar-eclipses-2026-2030.csv"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_of_each() {
        let (solar, lunar) = (solar_eclipses_2026_2030(), lunar_eclipses_2026_2030());
        assert_eq!((solar.len(), lunar.len()), (12, 12));
        // 2026 Aug 12, 17:47:06 TD.
        assert_eq!(solar[1].jd_td, 2_461_264.5 + 64_026.0 / 86_400.0);
        assert_eq!(lunar[3].kind, "Ne");
        assert_eq!(lunar[3].magnitudes, vec![0.0014, -1.068]);
    }
}
