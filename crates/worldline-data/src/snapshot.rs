//! Snapshots: the state of a set of real bodies at one moment.

use std::fmt;

use worldline_core::{Body, DVec3, System};

/// The column header every snapshot file must have.
const HEADER: &str = "name,naif_id,gm_km3_s2,radius_km,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s";

/// Unit conversions from the file's km-based units to SI.
const KM: f64 = 1e3;
const KM3: f64 = 1e9;

/// The state of a set of real bodies at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// The moment, as a Julian Date in Barycentric Dynamical Time (TDB),
    /// the time scale JPL's ephemerides use.
    pub epoch_jd_tdb: f64,
    /// The bodies in SI units, positioned relative to the solar system barycenter.
    pub bodies: Vec<Body>,
}

impl Snapshot {
    /// Parses the CSV format written by `tools/fetch_solar_system.py`.
    ///
    /// Lines starting with `#` are comments. The line `# epoch_jd_tdb: <value>`
    /// is required. Units in the file are km, km/s and km³/s².
    pub fn parse(text: &str) -> Result<Self, ParseError> {
        let mut epoch = None;
        let mut header_seen = false;
        let mut bodies = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line_number = index + 1;
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(comment) = line.strip_prefix('#') {
                if let Some(value) = comment.trim().strip_prefix("epoch_jd_tdb:") {
                    epoch = Some(parse_number(value, line_number)?);
                }
                continue;
            }
            if !header_seen {
                if line != HEADER {
                    return Err(ParseError::new(
                        line_number,
                        format!("expected header `{HEADER}`"),
                    ));
                }
                header_seen = true;
                continue;
            }
            bodies.push(parse_body(line, line_number)?);
        }
        let epoch_jd_tdb =
            epoch.ok_or_else(|| ParseError::new(0, "missing `# epoch_jd_tdb:` line"))?;
        Ok(Self {
            epoch_jd_tdb,
            bodies,
        })
    }

    /// A system ready to simulate, with its clock at zero at this snapshot's epoch.
    pub fn system(&self) -> System {
        System::new(self.bodies.clone())
    }

    /// The body with this name, if present.
    pub fn body(&self, name: &str) -> Option<&Body> {
        self.bodies.iter().find(|b| b.name == name)
    }
}

fn parse_body(line: &str, line_number: usize) -> Result<Body, ParseError> {
    let fields: Vec<&str> = line.split(',').map(str::trim).collect();
    if fields.len() != 10 {
        return Err(ParseError::new(
            line_number,
            format!("expected 10 fields, found {}", fields.len()),
        ));
    }
    let number = |i: usize| parse_number(fields[i], line_number);
    // fields[1] is the NAIF id, kept in the file for provenance.
    let (gm, radius) = (number(2)? * KM3, number(3)? * KM);
    if gm < 0.0 || radius < 0.0 {
        return Err(ParseError::new(
            line_number,
            "GM and radius must not be negative",
        ));
    }
    let position = DVec3::new(number(4)?, number(5)?, number(6)?) * KM;
    let velocity = DVec3::new(number(7)?, number(8)?, number(9)?) * KM;
    Ok(Body::new(fields[0], gm, radius)
        .at(position)
        .moving(velocity))
}

fn parse_number(text: &str, line_number: usize) -> Result<f64, ParseError> {
    let text = text.trim();
    match text.parse::<f64>() {
        Ok(value) if value.is_finite() => Ok(value),
        _ => Err(ParseError::new(
            line_number,
            format!("`{text}` is not a finite number"),
        )),
    }
}

/// A problem in a snapshot file.
#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    /// The 1-based line number, or 0 if the problem isn't on one line.
    pub line: usize,
    /// What went wrong.
    pub message: String,
}

impl ParseError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solar_system;

    #[test]
    fn bundled_solar_system_loads() {
        let snapshot = solar_system();
        assert_eq!(snapshot.epoch_jd_tdb, 2_460_676.5);
        let names: Vec<&str> = snapshot.bodies.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Sun", "Mercury", "Venus", "Earth", "Moon", "Mars", "Jupiter", "Saturn", "Uranus",
                "Neptune", "Pluto"
            ]
        );
        // DE440's solar GM, converted from km³/s² to m³/s².
        assert_eq!(snapshot.body("Sun").unwrap().gm, 1.327_124_400_412_794_2e20);
        // Earth is about 1 AU from the Sun.
        let earth = snapshot.body("Earth").unwrap().position;
        let sun = snapshot.body("Sun").unwrap().position;
        let au = (earth - sun).length() / worldline_core::constants::AU;
        assert!((0.98..1.02).contains(&au), "Earth is {au} AU from the Sun");
    }

    #[test]
    fn errors_name_the_line() {
        let text = format!("# epoch_jd_tdb: 1.0\n{HEADER}\nSun,10,1.0,2.0,3.0\n");
        let error = Snapshot::parse(&text).unwrap_err();
        assert_eq!(error.line, 3);
        assert!(error.to_string().contains("expected 10 fields"));

        let error = Snapshot::parse(HEADER).unwrap_err();
        assert!(error.message.contains("epoch_jd_tdb"));
    }
}
