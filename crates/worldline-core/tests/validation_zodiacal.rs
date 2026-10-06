//! Validation: the zodiacal cloud model against the Helios space probes.
//!
//! Kelsall et al. fitted their model from Earth's orbit, looking out at
//! 64°–124° from the Sun. Helios 1 and 2 flew in to 0.3 AU and measured how
//! the zodiacal light brightens closer to the Sun: as R^−(2.3 ± 0.1), at
//! every elongation from 17.5° to 135° (Leinert et al. 1981, *A&A* 103,
//! 177). A model brightening faster or slower than that would put the dust
//! in the wrong place.

use worldline_core::DVec3;
use worldline_core::constants::AU;
use worldline_core::zodiacal::ZodiacalCloud;

#[test]
fn the_dust_model_brightens_toward_the_sun_as_helios_measured() {
    let cloud = ZodiacalCloud::kelsall();
    println!(
        "{:>10} {:>10} {:>10}",
        "longitude", "elongation", "exponent"
    );
    for longitude in [0.0_f64, 90.0, 180.0, 270.0] {
        let outward = DVec3::new(
            longitude.to_radians().cos(),
            longitude.to_radians().sin(),
            0.0,
        );
        let east = DVec3::Z.cross(outward);
        for elongation in [30.0_f64, 60.0, 90.0, 135.0] {
            // Angle from the Sun's direction (−outward), in the ecliptic.
            let e = elongation.to_radians();
            let direction = -outward * e.cos() + east * e.sin();
            let light = |r: f64| cloud.scattered_light(outward * r * AU, direction);
            let exponent = (light(0.3) / light(1.0)).ln() / (1.0 / 0.3_f64).ln();
            println!("{longitude:>10} {elongation:>10} {exponent:>10.3}");
            assert!(
                (exponent - 2.3).abs() <= 0.1,
                "at {elongation}°: brightness goes as R^-{exponent:.3}"
            );
        }
    }
}
