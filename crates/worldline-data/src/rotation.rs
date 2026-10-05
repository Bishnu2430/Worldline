//! IAU rotation models for the bodies in the bundled snapshots.

use std::collections::HashMap;
use std::sync::OnceLock;

use worldline_core::rotation::{PeriodicTerm, RotationModel};

use crate::kernel;

/// The bundled kernel, parsed once.
fn kernel() -> &'static HashMap<String, Vec<f64>> {
    static KERNEL: OnceLock<HashMap<String, Vec<f64>>> = OnceLock::new();
    KERNEL.get_or_init(|| {
        kernel::parse(include_str!("../data/rotation-pck00011.tpc"))
            .expect("bundled rotation kernel is valid")
    })
}

/// NAIF id of the body itself (not its system barycenter) for each body
/// in the solar-system snapshots.
fn naif_body_id(name: &str) -> Option<u32> {
    Some(match name {
        "Sun" => 10,
        "Mercury" => 199,
        "Venus" => 299,
        "Earth" => 399,
        "Moon" => 301,
        "Mars" => 499,
        "Jupiter" => 599,
        "Saturn" => 699,
        "Uranus" => 799,
        "Neptune" => 899,
        "Pluto" => 999,
        _ => return None,
    })
}

/// The IAU rotation model for a body in the solar-system snapshot, from
/// NASA NAIF's pck00011 kernel. `None` for bodies without one.
pub fn rotation_model(name: &str) -> Option<RotationModel> {
    let id = naif_body_id(name)?;
    let values = kernel();
    let get = |key: String| values.get(&key).cloned().unwrap_or_default();
    let three = |key: String| -> [f64; 3] {
        let v = get(key);
        std::array::from_fn(|i| v.get(i).copied().unwrap_or(0.0))
    };

    let ra = get(format!("BODY{id}_NUT_PREC_RA"));
    let dec = get(format!("BODY{id}_NUT_PREC_DEC"));
    let pm = get(format!("BODY{id}_NUT_PREC_PM"));
    let count = ra.len().max(dec.len()).max(pm.len());

    // Periodic-term angles belong to the planet's system: 3 for the Earth
    // and Moon, 5 for Jupiter, and so on. Each angle is a polynomial of
    // degree MAX_PHASE_DEGREE (1 unless the kernel says otherwise).
    let system = id / 100;
    let angles = get(format!("BODY{system}_NUT_PREC_ANGLES"));
    let degree = get(format!("BODY{system}_MAX_PHASE_DEGREE"))
        .first()
        .map_or(1, |&d| d as usize);
    let per_angle = degree + 1;
    assert!(
        angles.len() >= count * per_angle,
        "BODY{id} has {count} periodic terms but only {} angles",
        angles.len() / per_angle
    );

    let amplitude = |list: &[f64], i: usize| list.get(i).copied().unwrap_or(0.0);
    let terms = (0..count)
        .map(|i| PeriodicTerm {
            angle: angles[i * per_angle..(i + 1) * per_angle].to_vec(),
            ra: amplitude(&ra, i),
            dec: amplitude(&dec, i),
            pm: amplitude(&pm, i),
        })
        .filter(|t| t.ra != 0.0 || t.dec != 0.0 || t.pm != 0.0)
        .collect();

    Some(RotationModel {
        pole_ra: three(format!("BODY{id}_POLE_RA")),
        pole_dec: three(format!("BODY{id}_POLE_DEC")),
        prime_meridian: three(format!("BODY{id}_PM")),
        terms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_snapshot_body_has_a_model() {
        for body in crate::solar_system().bodies {
            let model = rotation_model(&body.name).unwrap();
            assert!(model.spin_rate() != 0.0, "{} doesn't spin", body.name);
        }
    }

    #[test]
    fn values_match_the_kernel() {
        let earth = rotation_model("Earth").unwrap();
        assert_eq!(earth.prime_meridian, [190.147, 360.985_623_5, 0.0]);
        assert_eq!(earth.pole_dec, [90.0, -0.557, 0.0]);
        // The Moon has 13 periodic terms in pck00011, all nonzero in at
        // least one of RA, Dec or PM.
        let moon = rotation_model("Moon").unwrap();
        assert_eq!(moon.terms.len(), 13);
        assert_eq!(moon.prime_meridian[2], -1.4e-12);
        // Mars uses quadratic angles.
        let mars = rotation_model("Mars").unwrap();
        assert!(mars.terms.iter().all(|t| t.angle.len() == 3));
    }
}
