//! The catalog of notable objects: black holes, neutron stars, white
//! dwarfs and nearby stars, with their published masses and sizes.
//!
//! `tools/write_notable_objects.py` regenerates the data. See
//! `docs/physics/compact-objects.md`.

use worldline_core::Body;
use worldline_core::compact::horizon_radius;
use worldline_core::constants::{GM_SUN, PARSEC, SOLAR_RADIUS};

/// What sort of object it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectKind {
    /// A star burning nuclear fuel.
    Star,
    /// The dense core a Sun-like star leaves, held up by electron
    /// degeneracy pressure.
    WhiteDwarf,
    /// The collapsed core of a massive star, held up by neutron degeneracy
    /// and the strong force.
    NeutronStar,
    /// A black hole.
    BlackHole,
}

/// How an object's size was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadiusBasis {
    /// Measured for this object.
    Measured,
    /// From an empirical relation, not measured directly.
    Estimated,
    /// Not known for this object: another's measurement stands in.
    Assumed,
    /// A black hole's event horizon, computed from its mass and spin.
    Horizon,
}

/// How far away an object is, as published.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Distance {
    /// A distance, in m.
    Meters(f64),
    /// A redshift, for objects so far that their distance depends on the
    /// cosmological model.
    Redshift(f64),
}

/// A published value and its uncertainty, in the same units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measured {
    /// The published value.
    pub value: f64,
    /// How far above and below the value the uncertainty reaches; `None`
    /// if the source gives no interval.
    pub plus_minus: Option<(f64, f64)>,
}

/// An object in the catalog.
#[derive(Debug, Clone, PartialEq)]
pub struct NotableObject {
    /// Its name, as the catalog lists it.
    pub name: String,
    /// What sort of object it is.
    pub kind: ObjectKind,
    /// The system it belongs to ("PSR B1913+16", "GW150914", …), if any.
    pub system: Option<String>,
    /// Mass, in solar masses (nominal GM☉, IAU 2015).
    pub mass: Measured,
    /// Radius, in m: the event horizon for a black hole.
    pub radius: Measured,
    /// How its radius was found.
    pub radius_basis: RadiusBasis,
    /// A black hole's dimensionless spin, and whether it is a measured
    /// value (true) or a lower limit (false).
    pub spin: Option<(f64, bool)>,
    /// How far away it is, if published.
    pub distance: Option<Distance>,
    /// Where it is, in words.
    pub location: String,
    /// Where its values were published.
    pub source: String,
}

impl NotableObject {
    /// Its gravitational parameter GM, in m³/s².
    pub fn gm(&self) -> f64 {
        self.mass.value * GM_SUN
    }

    /// A body with its mass and size, at rest at the origin.
    pub fn body(&self) -> Body {
        Body::new(self.name.clone(), self.gm(), self.radius.value)
    }
}

/// A measured binary orbit.
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryOrbit {
    /// The system, as named in the catalog.
    pub system: String,
    /// Orbital period, in s.
    pub period: f64,
    /// The orbit's eccentricity.
    pub eccentricity: f64,
    /// The measured mean advance of periastron, in degrees per year.
    pub periastron_advance: f64,
    /// Where it was published.
    pub source: String,
}

/// Splits a CSV line, where quoted fields may hold commas.
fn fields(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut quoted = false;
    for c in line.chars() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(String::new()),
            _ => out.last_mut().expect("a field").push(c),
        }
    }
    out
}

fn number(text: &str) -> f64 {
    // Masses published as logarithms, like TON 618's, are written 10^x.
    match text.strip_prefix("10^") {
        Some(exponent) => 10f64.powf(exponent.parse::<f64>().expect("published exponent")),
        None => text.parse().expect("published number"),
    }
}

fn optional(text: &str) -> Option<f64> {
    (!text.is_empty()).then(|| number(text))
}

fn rows(text: &str) -> impl Iterator<Item = Vec<String>> + '_ {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .skip(1)
        .map(fields)
}

/// Every object in the catalog.
pub fn notable_objects() -> Vec<NotableObject> {
    rows(include_str!("../data/notable-objects.csv"))
        .map(|f| {
            assert_eq!(f.len(), 17, "a notable object has 17 fields: {f:?}");
            let kind = match f[1].as_str() {
                "star" => ObjectKind::Star,
                "white dwarf" => ObjectKind::WhiteDwarf,
                "neutron star" => ObjectKind::NeutronStar,
                "black hole" => ObjectKind::BlackHole,
                other => panic!("unknown kind `{other}`"),
            };
            let mass = Measured {
                value: number(&f[3]),
                plus_minus: optional(&f[4]).zip(optional(&f[5])),
            };
            let spin = optional(&f[11]).map(|s| (s, f[12] == "measured"));
            let radius_basis = match f[10].as_str() {
                "measured" => RadiusBasis::Measured,
                "estimated" => RadiusBasis::Estimated,
                "assumed" => RadiusBasis::Assumed,
                "horizon" => RadiusBasis::Horizon,
                other => panic!("unknown radius basis `{other}`"),
            };
            let radius = if radius_basis == RadiusBasis::Horizon {
                // Not spinning, if the spin isn't measured: the largest
                // horizon a hole of its mass can have.
                Measured {
                    value: horizon_radius(mass.value * GM_SUN, spin.map_or(0.0, |s| s.0)),
                    plus_minus: None,
                }
            } else {
                let unit = match f[7].as_str() {
                    "km" => 1e3,
                    "R_sun" => SOLAR_RADIUS,
                    other => panic!("unknown radius unit `{other}`"),
                };
                Measured {
                    value: number(&f[6]) * unit,
                    plus_minus: optional(&f[8])
                        .zip(optional(&f[9]))
                        .map(|(p, m)| (p * unit, m * unit)),
                }
            };
            let distance = optional(&f[13]).map(|d| match f[14].as_str() {
                "pc" => Distance::Meters(d * PARSEC),
                "kpc" => Distance::Meters(d * 1e3 * PARSEC),
                "Mpc" => Distance::Meters(d * 1e6 * PARSEC),
                // A parallax in milliarcseconds: 1000/ϖ parsecs.
                "mas" => Distance::Meters(1e3 / d * PARSEC),
                "z" => Distance::Redshift(d),
                other => panic!("unknown distance unit `{other}`"),
            });
            NotableObject {
                name: f[0].clone(),
                kind,
                system: (!f[2].is_empty()).then(|| f[2].clone()),
                mass,
                radius,
                radius_basis,
                spin,
                distance,
                location: f[15].clone(),
                source: f[16].clone(),
            }
        })
        .collect()
}

/// The object named `name`, if the catalog has it.
pub fn notable_object(name: &str) -> Option<NotableObject> {
    notable_objects().into_iter().find(|o| o.name == name)
}

/// The measured orbits of the catalog's binaries.
pub fn binary_orbits() -> Vec<BinaryOrbit> {
    rows(include_str!("../data/binary-orbits.csv"))
        .map(|f| BinaryOrbit {
            system: f[0].clone(),
            period: number(&f[1]) * 86_400.0,
            eccentricity: number(&f[2]),
            periastron_advance: number(&f[3]),
            source: f[4].clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoted_fields_keep_their_commas() {
        assert_eq!(fields(r#"a,"b, c",,d"#), ["a", "b, c", "", "d"]);
    }

    #[test]
    fn every_row_parses() {
        let objects = notable_objects();
        assert_eq!(objects.len(), 20);
        assert!(
            objects
                .iter()
                .all(|o| o.mass.value > 0.0 && o.radius.value > 0.0)
        );
        assert_eq!(binary_orbits().len(), 1);
    }
}
