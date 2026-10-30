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

/// A place in NASA's path table, degrees: latitude north, longitude east.
pub type LatLon = (f64, f64);

/// One moment of NASA's path of a total solar eclipse.
#[derive(Debug, Clone, PartialEq)]
pub struct PathRow {
    /// When, a Julian Date in the table's UT.
    pub jd_ut: f64,
    /// The umbra's northern limit, if it is on Earth.
    pub north: Option<LatLon>,
    /// Its southern limit.
    pub south: Option<LatLon>,
    /// The central line.
    pub central: LatLon,
}

/// An eclipse's Besselian elements at their reference time, as NASA
/// publishes them: the Moon's shadow on the fundamental plane, in Earth
/// equatorial radii.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Besselian {
    /// The reference time t0, a Julian Date in dynamical time.
    pub jd_td: f64,
    /// Where the shadow's axis crosses the plane: east.
    pub x: f64,
    /// And north (of the true pole of date).
    pub y: f64,
    /// The penumbra's radius on the plane.
    pub l1: f64,
    /// The umbra's, negative where the umbra reaches the plane.
    pub l2: f64,
    /// The tangent of the penumbral cone's half-angle.
    pub tan_f1: f64,
    /// And of the umbral cone's.
    pub tan_f2: f64,
}

/// NASA's path of a total solar eclipse.
#[derive(Debug, Clone, PartialEq)]
pub struct CanonPath {
    /// The ΔT the table assumed, s: its UT is dynamical time less this.
    pub delta_t: f64,
    /// The eclipse's Besselian elements at t0.
    pub besselian: Besselian,
    /// Every 2 minutes.
    pub rows: Vec<PathRow>,
}

/// "65 10.3N" or "025 12.3W" as degrees, north and east positive.
fn coordinate(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, hemisphere) = text.split_at(text.len().checked_sub(1)?);
    let (degrees, minutes) = number.split_once(' ')?;
    let value = degrees.parse::<f64>().ok()? + minutes.parse::<f64>().ok()? / 60.0;
    Some(if hemisphere == "S" || hemisphere == "W" {
        -value
    } else {
        value
    })
}

/// NASA's path of the total solar eclipse of 2026 August 12 (F. Espenak):
/// the reference for the Moon's shadow on Earth (step 5.3). See
/// `tools/fetch_eclipse_path.py`.
pub fn eclipse_path_2026_08_12() -> CanonPath {
    let text = include_str!("../data/eclipse-path-2026-08-12.csv");
    let delta_t = text
        .lines()
        .find_map(|l| l.strip_prefix("# delta_t_s: "))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|d| d.parse().ok())
        .expect("the path's delta T is given");
    // "# besselian: t0 18:00:00.0 TDT; x 0.475593; y ...; tan_f2 0.0045911 (url)"
    let line = text
        .lines()
        .find_map(|l| l.strip_prefix("# besselian: "))
        .expect("the Besselian elements are given");
    let value = |name: &str| -> f64 {
        line.split("; ")
            .find_map(|part| part.strip_prefix(name)?.strip_prefix(' '))
            .and_then(|v| v.split_whitespace().next()?.parse().ok())
            .expect("a Besselian element")
    };
    let t0: Vec<f64> = line
        .strip_prefix("t0 ")
        .and_then(|rest| rest.split_whitespace().next())
        .expect("t0")
        .split(':')
        .map(|v| v.parse().expect("t0 is a time"))
        .collect();
    let besselian = Besselian {
        jd_td: julian_date(2026, 8, 12, t0[0] * 3600.0 + t0[1] * 60.0 + t0[2]),
        x: value("x"),
        y: value("y"),
        l1: value("l1"),
        l2: value("l2"),
        tan_f1: value("tan_f1"),
        tan_f2: value("tan_f2"),
    };
    let rows = text
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f: Vec<&str> = line.split(',').collect();
            let (h, m) = f[0].split_once(':').expect("a time");
            let seconds = h.parse::<f64>().unwrap() * 3600.0 + m.parse::<f64>().unwrap() * 60.0;
            let place = |i: usize| Some((coordinate(f[i])?, coordinate(f[i + 1])?));
            PathRow {
                jd_ut: julian_date(2026, 8, 12, seconds),
                north: place(1),
                south: place(3),
                central: place(5).expect("the central line is given"),
            }
        })
        .collect();
    CanonPath {
        delta_t,
        besselian,
        rows,
    }
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

    #[test]
    fn the_august_2026_path_loads() {
        let path = eclipse_path_2026_08_12();
        assert_eq!((path.delta_t, path.rows.len()), (71.4, 46));
        // 17:46: the central line at 65°10.3'N, 25°12.3'W.
        let row = &path.rows[22];
        assert_eq!(
            row.jd_ut,
            julian_date(2026, 8, 12, 17.0 * 3600.0 + 46.0 * 60.0)
        );
        assert_eq!(row.central, (65.0 + 10.3 / 60.0, -(25.0 + 12.3 / 60.0)));
        // 18:32: the northern limit has left Earth.
        assert_eq!(path.rows[45].north, None);
        let b = path.besselian;
        assert_eq!(b.jd_td, julian_date(2026, 8, 12, 18.0 * 3600.0));
        assert_eq!((b.l2, b.tan_f2), (-0.008142, 0.0045911));
    }
}
