//! The planets' magnetic dipoles and Earth's radiation belts.
//!
//! `tools/fetch_magnetic_fields.py` regenerates the data. See
//! `docs/physics/magnetospheres.md`.

use worldline_core::magnetosphere::Dipole;

/// A planet's magnetic field model.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanetaryField {
    /// The planet.
    pub planet: String,
    /// The field model it comes from ("IGRF-14 at 2025.0", "JRM09", …).
    pub model: String,
    /// Its dipole.
    pub dipole: Dipole,
    /// Where it was published.
    pub source: String,
}

/// Splits a CSV line whose last field is quoted and may contain commas.
fn fields(line: &str, count: usize) -> Vec<String> {
    line.splitn(count, ',')
        .map(|f| f.trim_matches('"').to_string())
        .collect()
}

/// Every planet with a global magnetic field (Venus and Mars have none).
pub fn planetary_fields() -> Vec<PlanetaryField> {
    include_str!("../data/planetary-dipoles.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f = fields(line, 8);
            let number = |i: usize| f[i].parse::<f64>().expect("published number");
            PlanetaryField {
                planet: f[0].clone(),
                model: f[1].clone(),
                dipole: Dipole {
                    g10: number(3) * 1e-9,
                    g11: number(4) * 1e-9,
                    h11: number(5) * 1e-9,
                    radius: number(2) * 1e3,
                    north_offset: number(6) * 1e3,
                },
                source: f[7].clone(),
            }
        })
        .collect()
}

/// One of a planet's radiation belts, as a range of L: the equatorial
/// distance of a dipole field line, in planetary radii.
#[derive(Debug, Clone, PartialEq)]
pub struct RadiationBelt {
    /// The planet.
    pub planet: String,
    /// "inner" or "outer".
    pub name: String,
    /// Where it starts and ends, in L.
    pub l_range: (f64, f64),
    /// Where its extent is given.
    pub source: String,
}

/// Earth's two radiation belts.
pub fn radiation_belts() -> Vec<RadiationBelt> {
    include_str!("../data/radiation-belts.csv")
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|line| {
            let f = fields(line, 5);
            let number = |i: usize| f[i].parse::<f64>().expect("published number");
            RadiationBelt {
                planet: f[0].clone(),
                name: f[1].clone(),
                l_range: (number(2), number(3)),
                source: f[4].clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_loads() {
        let fields = planetary_fields();
        let planets: Vec<&str> = fields.iter().map(|f| f.planet.as_str()).collect();
        assert_eq!(
            planets,
            ["Mercury", "Earth", "Jupiter", "Saturn", "Uranus", "Neptune"]
        );
        assert!((fields[1].dipole.g10 * 1e9 + 29_350.0).abs() < 1e-9);
        assert!(fields[3].source.starts_with("Cao et al. 2020"));
        // Uranus's Q3 dipole: 0.228 gauss R_U³, tilted 58.6° (Connerney et
        // al. 1987), to the rounding of those figures.
        let uranus = fields[4].dipole;
        assert!((uranus.equatorial_field() / 1e-4 - 0.228).abs() < 0.0005);
        assert!((uranus.tilt().to_degrees() - 58.6).abs() < 0.05);
        let belts = radiation_belts();
        assert_eq!(belts[1].l_range, (3.0, 7.0));
    }
}
