use glam::DVec3;

/// The extra gravity of a flattened planet beyond a point mass, described
/// by zonal harmonics J_n.
///
/// A spinning planet bulges at its equator, so it pulls harder there. Its
/// gravitational potential is
///
/// Φ = −(μ / r) [1 − Σ_n J_n (R / r)ⁿ P_n(z / r)]
///
/// where z is the height above the equator, R the reference radius the
/// coefficients are given for, and P_n the Legendre polynomials. Only the
/// J_n terms are returned here; the point-mass part comes from Newtonian
/// gravity. Symmetric about the pole: longitude-dependent (tesseral) terms
/// are left out. See `docs/physics/moons.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct ZonalField {
    /// Reference radius the coefficients are defined with, in m.
    pub radius: f64,
    /// (degree n, J_n), unnormalized, for degrees up to 15.
    pub coefficients: Vec<(u32, f64)>,
    /// Unit vector along the planet's north pole: the field's symmetry axis.
    pub pole: DVec3,
}

impl ZonalField {
    /// Acceleration beyond the point-mass pull (m/s²) at `r` (m) from the
    /// center of a planet with gravitational parameter `mu` (m³/s²).
    ///
    /// For each degree, from a = −∇Φ_n with Φ_n = μ J_n Rⁿ P_n(u) / r^(n+1)
    /// and u = z / r:
    /// a_n = μ J_n Rⁿ / r^(n+3) · [((n+1) P_n(u) + u P_n′(u)) r − r P_n′(u) k̂]
    pub fn acceleration(&self, mu: f64, r: DVec3) -> DVec3 {
        // Same formula, written per unit vector:
        // a = μ/r² Σ J_n (R/r)ⁿ [((n+1) P_n + u P_n′) r̂ − P_n′ k̂],
        // with every P_n, P_n′ and (R/r)ⁿ from one pass of the recurrences
        // (this runs at every force evaluation for every moon).
        let mut j = [0.0; 16];
        for &(n, jn) in &self.coefficients {
            j[n as usize] = jn;
        }
        let max_degree = self.coefficients.iter().map(|c| c.0).max().unwrap_or(0) as usize;
        let distance = r.length();
        let unit = r / distance;
        let u = unit.dot(self.pole);
        let ratio = self.radius / distance;
        let (mut along_r, mut along_pole) = (0.0, 0.0);
        let (mut p_prev, mut p, mut dp_prev, mut dp) = (1.0, u, 0.0, 1.0);
        let mut power = ratio;
        for (n, &jn) in j.iter().enumerate().take(max_degree + 1).skip(1) {
            if jn != 0.0 {
                along_r += jn * power * ((n + 1) as f64 * p + u * dp);
                along_pole += jn * power * dp;
            }
            // Step up to degree n + 1.
            let k = n as f64;
            let p_next = ((2.0 * k + 1.0) * u * p - k * p_prev) / (k + 1.0);
            let dp_next = dp_prev + (2.0 * k + 1.0) * p;
            (p_prev, p, dp_prev, dp) = (p, p_next, dp, dp_next);
            power *= ratio;
        }
        (unit * along_r - self.pole * along_pole) * (mu / (distance * distance))
    }

    /// The extra potential (J/kg) matching [`Self::acceleration`].
    pub fn potential(&self, mu: f64, r: DVec3) -> f64 {
        let distance = r.length();
        let u = r.dot(self.pole) / distance;
        self.coefficients
            .iter()
            .map(|&(n, j)| {
                mu * j * self.radius.powi(n as i32) * legendre(n, u).0 / distance.powi(n as i32 + 1)
            })
            .sum()
    }
}

/// Legendre polynomial P_n(u) and its derivative P_n′(u), by the standard
/// recurrences. The derivative recurrence P′_(k+1) = P′_(k−1) + (2k+1) P_k
/// stays finite at the poles (u = ±1), unlike the closed form.
fn legendre(n: u32, u: f64) -> (f64, f64) {
    let (mut p_prev, mut p) = (1.0, u);
    let (mut dp_prev, mut dp) = (0.0, 1.0);
    if n == 0 {
        return (1.0, 0.0);
    }
    for k in 1..n {
        let k = k as f64;
        let p_next = ((2.0 * k + 1.0) * u * p - k * p_prev) / (k + 1.0);
        let dp_next = dp_prev + (2.0 * k + 1.0) * p;
        (p_prev, p) = (p, p_next);
        (dp_prev, dp) = (dp, dp_next);
    }
    (p, dp)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jupiter_like() -> ZonalField {
        ZonalField {
            radius: 71_492e3,
            coefficients: vec![(2, 1.4697e-2), (3, -4.5e-8), (4, -5.866e-4), (6, 3.42e-5)],
            pole: DVec3::new(0.1, -0.2, 1.0).normalize(),
        }
    }

    #[test]
    fn legendre_polynomials_match_their_closed_forms() {
        for u in [-1.0, -0.6, 0.0, 0.3, 1.0] {
            let (p2, dp2) = legendre(2, u);
            assert!((p2 - (3.0 * u * u - 1.0) / 2.0).abs() < 1e-15);
            assert!((dp2 - 3.0 * u).abs() < 1e-15);
            let (p4, dp4) = legendre(4, u);
            assert!((p4 - (35.0 * u.powi(4) - 30.0 * u * u + 3.0) / 8.0).abs() < 1e-14);
            assert!((dp4 - (140.0 * u.powi(3) - 60.0 * u) / 8.0).abs() < 1e-13);
        }
    }

    #[test]
    fn acceleration_is_minus_the_gradient_of_the_potential() {
        // Compare with a central-difference gradient at points around the
        // planet, including over the pole and on the equator.
        let field = jupiter_like();
        let mu = 1.267e17;
        let points = [
            DVec3::new(4.2e8, 0.0, 0.0),
            DVec3::new(-3.0e8, 2.5e8, 1.0e8),
            field.pole * 2.0e8,
            DVec3::new(1.0e8, 1.0e8, -1.0e8),
        ];
        for r in points {
            let h = 1.0;
            let gradient = DVec3::new(
                field.potential(mu, r + DVec3::X * h) - field.potential(mu, r - DVec3::X * h),
                field.potential(mu, r + DVec3::Y * h) - field.potential(mu, r - DVec3::Y * h),
                field.potential(mu, r + DVec3::Z * h) - field.potential(mu, r - DVec3::Z * h),
            ) / (2.0 * h);
            let a = field.acceleration(mu, r);
            assert!(
                (a + gradient).length() < 1e-6 * a.length(),
                "at {r}: {a} vs {}",
                -gradient
            );
        }
    }

    #[test]
    fn an_oblate_planet_pulls_harder_at_its_equator() {
        // J2 > 0: at the equator the extra pull points inward; over the
        // pole it points outward (the pole is farther from the bulge).
        let field = ZonalField {
            radius: 1.0,
            coefficients: vec![(2, 0.01)],
            pole: DVec3::Z,
        };
        assert!(field.acceleration(1.0, DVec3::new(3.0, 0.0, 0.0)).x < 0.0);
        assert!(field.acceleration(1.0, DVec3::new(0.0, 0.0, 3.0)).z > 0.0);
    }
}
