//! The zodiacal cloud: the interplanetary dust that scatters sunlight into
//! the zodiacal light. See `docs/physics/zodiacal-dust.md`.

use glam::DVec3;

use crate::constants::AU;

/// The smooth zodiacal cloud of Kelsall et al., "The COBE Diffuse Infrared
/// Background Experiment search for the cosmic infrared background. II.
/// Model of the interplanetary dust cloud", *ApJ* 508, 44 (1998), fitted to
/// ten months of COBE DIRBE's infrared sky survey:
///
/// n = n₀ R_c^−α f(ζ),  f(ζ) = exp(−β g^γ),
/// g = ζ²/2μ for ζ < μ, and ζ − μ/2 for ζ ≥ μ,
///
/// where R_c is the distance from the cloud's center (slightly off the
/// Sun) and ζ = |Z_c|/R_c the height above its symmetry plane relative to
/// that distance (eqs. 4–7). n is the dust's cross-sectional area per unit
/// volume. The model's dust bands, circumsolar ring and Earth-trailing
/// blob, a few percent of the zodiacal light, are left out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZodiacalCloud {
    /// The cloud's center, relative to the Sun, in m.
    pub center: DVec3,
    /// The pole of its symmetry plane.
    pub pole: DVec3,
    /// n₀, the density 1 AU from the center in the symmetry plane, in m⁻¹
    /// (m² of cross-section per m³).
    pub n0: f64,
    /// The radial power-law index α.
    pub alpha: f64,
    /// β, how fast the density falls away from the symmetry plane.
    pub beta: f64,
    /// γ, the shape of that fall.
    pub gamma: f64,
    /// μ, where the profile turns from rounded (near the plane) to
    /// fan-shaped.
    pub mu: f64,
    /// Kelsall et al. integrate out to 5.2 AU from the Sun, about Jupiter's
    /// orbit; there is no dust beyond it in the model.
    pub outer_radius: f64,
}

impl ZodiacalCloud {
    /// The DIRBE smooth cloud. Full-precision values of the published fit
    /// (rounded in Kelsall et al.'s tables to i = 2.03°, Ω = 77.7°,
    /// α = 1.34, n₀ = 1.13 × 10⁻⁷ AU⁻¹), as distributed with the DIRBE model
    /// and reproduced by ZodiPy (San et al. 2022, *A&A* 666, A107).
    // The published digits, copied exactly, even beyond what f64 holds.
    #[allow(clippy::excessive_precision)]
    pub fn kelsall() -> Self {
        let inclination = 2.033_518_807_239_077_f64.to_radians();
        let node = 77.657_955_554_097_11_f64.to_radians();
        Self {
            center: DVec3::new(
                0.011_887_800_744_346_281,
                0.005_476_506_466_226_377_7,
                -0.002_153_090_802_071_074_4,
            ) * AU,
            // Kelsall et al. eq. 5: Z_c = X′ sin Ω sin i − Y′ cos Ω sin i
            // + Z′ cos i.
            pole: DVec3::new(
                node.sin() * inclination.sin(),
                -node.cos() * inclination.sin(),
                inclination.cos(),
            ),
            n0: 1.134_437_388_142_796e-7 / AU,
            alpha: 1.337_069_670_593_028_1,
            beta: 4.141_500_415_758_663_7,
            gamma: 0.942_061_793_933_580_36,
            mu: 0.188_731_764_890_901_9,
            outer_radius: 5.2 * AU,
        }
    }

    /// The dust's cross-sectional area per unit volume (m⁻¹) at
    /// `position`, relative to the Sun (m).
    pub fn density(&self, position: DVec3) -> f64 {
        if position.length() > self.outer_radius {
            return 0.0;
        }
        let offset = position - self.center;
        let r = offset.length();
        let zeta = (offset.dot(self.pole) / r).abs();
        let g = if zeta < self.mu {
            zeta * zeta / (2.0 * self.mu)
        } else {
            zeta - self.mu / 2.0
        };
        self.n0 * (r / AU).powf(-self.alpha) * (-self.beta * g.powf(self.gamma)).exp()
    }

    /// How much sunlight the dust scatters toward an observer at
    /// `observer` (relative to the Sun, m) looking along `direction`:
    /// ∫ n (1 AU / r)² ds along the line of sight, assuming every grain
    /// scatters equally in all directions. Dimensionless; brightness is
    /// proportional to it.
    pub fn scattered_light(&self, observer: DVec3, direction: DVec3) -> f64 {
        const STEPS: usize = 8000;
        let d = direction.normalize();
        // Where the line of sight is inside the outer radius:
        // |observer + s d|² = R², a quadratic in s.
        let b = observer.dot(d);
        let c = observer.length_squared() - self.outer_radius * self.outer_radius;
        let discriminant = b * b - c;
        if discriminant <= 0.0 {
            return 0.0;
        }
        let root = discriminant.sqrt();
        let (start, end) = ((-b - root).max(0.0), -b + root);
        if end <= start {
            return 0.0;
        }
        // Simpson's rule.
        let h = (end - start) / STEPS as f64;
        let integrand = |s: f64| {
            let x = observer + d * s;
            self.density(x) * AU * AU / x.length_squared()
        };
        let sum: f64 = (0..=STEPS)
            .map(|k| {
                let weight = if k == 0 || k == STEPS {
                    1.0
                } else if k % 2 == 1 {
                    4.0
                } else {
                    2.0
                };
                weight * integrand(start + h * k as f64)
            })
            .sum();
        sum * h / 3.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_density_follows_the_published_form() {
        let cloud = ZodiacalCloud::kelsall();
        // In the symmetry plane, 1 AU from the center: n₀.
        let in_plane = cloud.pole.cross(DVec3::Z).normalize();
        let n = cloud.density(cloud.center + in_plane * AU);
        assert!((n / cloud.n0 - 1.0).abs() < 1e-12, "{n}");
        // Twice as far, 2^−α as dense.
        let ratio = cloud.density(cloud.center + in_plane * 2.0 * AU) / n;
        assert!((ratio / 2f64.powf(-cloud.alpha) - 1.0).abs() < 1e-12);
        // The two branches of g meet at ζ = μ.
        let height = |zeta: f64| {
            let tilt = zeta.asin();
            cloud.center + (in_plane * tilt.cos() + cloud.pole * tilt.sin()) * AU
        };
        let below = cloud.density(height(cloud.mu * (1.0 - 1e-9)));
        let above = cloud.density(height(cloud.mu * (1.0 + 1e-9)));
        assert!((below / above - 1.0).abs() < 1e-6);
        // The symmetry plane is tilted 2.03° to the ecliptic, its ascending
        // node at 77.7°.
        let tilt = cloud.pole.z.acos().to_degrees();
        let node = (-cloud.pole.x).atan2(cloud.pole.y).to_degrees() + 180.0;
        assert!((tilt - 2.0335).abs() < 1e-4 && (node - 77.658).abs() < 1e-3);
        // No dust beyond 5.2 AU.
        assert_eq!(cloud.density(DVec3::X * 5.3 * AU), 0.0);
    }

    #[test]
    fn simpsons_rule_integrates_the_cloud_accurately() {
        // A cloud with α = 2 and no vertical falloff (β = 0), centered on
        // the Sun, has an exact line-of-sight integral: along a line passing
        // b from the Sun, ∫ n₀ AU⁴ (b² + s²)⁻² ds, whose antiderivative is
        // s / (2b²(b² + s²)) + atan(s/b) / (2b³). Simpson's error goes as
        // (h/b)⁴ ≈ 5 × 10⁻¹¹ here; 10⁻⁸ leaves room for rounding.
        let cloud = ZodiacalCloud {
            center: DVec3::ZERO,
            pole: DVec3::Z,
            alpha: 2.0,
            beta: 0.0,
            ..ZodiacalCloud::kelsall()
        };
        let b = 0.5 * AU;
        let half_chord = (cloud.outer_radius.powi(2) - b * b).sqrt();
        let antiderivative =
            |s: f64| s / (2.0 * b * b * (b * b + s * s)) + (s / b).atan() / (2.0 * b.powi(3));
        let exact =
            cloud.n0 * AU.powi(4) * (antiderivative(half_chord) - antiderivative(-half_chord));
        let observer = DVec3::new(-10.0 * AU, b, 0.0);
        let computed = cloud.scattered_light(observer, DVec3::X);
        println!("Simpson / exact - 1 = {:.1e}", computed / exact - 1.0);
        assert!((computed / exact - 1.0).abs() < 1e-8);
    }
}
