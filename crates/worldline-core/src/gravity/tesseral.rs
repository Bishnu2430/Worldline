use glam::{DMat3, DVec3};

/// One tesseral term: (degree n, order m, C_nm, S_nm), unnormalized.
pub type TesseralTerm = (u32, u32, f64, f64);

/// Rows in the recurrence tables: terms up to degree `TABLE − 2` are
/// supported (the accelerations need one degree more than the terms).
const TABLE: usize = 10;
type Table = [[f64; TABLE + 1]; TABLE];

/// The recurrence's fixed factors (2n − 1)/(n − m) and (n + m − 1)/(n − m),
/// worked out once, when the program is compiled.
const FACTORS: [[(f64, f64); TABLE]; TABLE] = factors();

const fn factors() -> [[(f64, f64); TABLE]; TABLE] {
    let mut table = [[(0.0, 0.0); TABLE]; TABLE];
    let mut n = 1;
    while n < TABLE {
        let mut m = 0;
        while m < n {
            let gap = (n - m) as f64;
            table[n][m] = ((2 * n - 1) as f64 / gap, (n + m - 1) as f64 / gap);
            m += 1;
        }
        n += 1;
    }
    table
}

/// The part of a planet's gravity that depends on longitude: lumps that
/// turn with the planet, described by tesseral harmonics C_nm, S_nm.
///
/// Mars is the clear case: the Tharsis volcanic plateau makes its equator
/// slightly elliptical (C22, S22), and Phobos and Deimos feel it change as
/// Mars turns beneath them. In the planet's body-fixed frame the potential
/// is
///
/// U = (μ / r) Σ (R / r)ⁿ P_nm(sin φ) [C_nm cos mλ + S_nm sin mλ]
///
/// with latitude φ, longitude λ, unnormalized associated Legendre
/// functions P_nm (no Condon–Shortley phase), and the force is +∇U. The
/// accelerations come from Cunningham's recurrences (Montenbruck & Gill,
/// *Satellite Orbits*, 2000, §3.2.4–3.2.5), which avoid the singularity
/// at the poles. Zonal terms (m = 0) are handled by
/// [`super::ZonalField`], though this field computes them correctly too.
///
/// The planet turns at a steady rate about a fixed pole; over the years a
/// simulation spans, its pole moves by hundredths of a degree. See
/// `docs/physics/moons.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct TesseralField {
    /// Reference radius the coefficients are defined with, in m.
    pub radius: f64,
    /// The terms, unnormalized.
    pub coefficients: Vec<TesseralTerm>,
    /// Rotates body-fixed vectors (z to the north pole, x to the prime
    /// meridian) into the simulation frame at time `epoch`.
    pub frame: DMat3,
    /// Spin rate about the pole, in rad/s (negative for retrograde).
    pub spin_rate: f64,
    /// Simulation time at which `frame` holds, in s.
    pub epoch: f64,
}

impl TesseralField {
    /// The planet's body-fixed frame at `time`.
    pub fn frame_at(&self, time: f64) -> DMat3 {
        self.frame * DMat3::from_rotation_z(self.spin_rate * (time - self.epoch))
    }

    /// Acceleration (m/s²) at `r` (m) from the center of a planet with
    /// gravitational parameter `mu` (m³/s²), at simulation time `time`.
    pub fn acceleration(&self, mu: f64, r: DVec3, time: f64) -> DVec3 {
        self.acceleration_in_frame(mu, r, self.frame_at(time))
    }

    /// Like [`Self::acceleration`], with the planet's body-fixed frame
    /// already worked out (from [`Self::frame_at`]), for several moons at
    /// once.
    pub fn acceleration_in_frame(&self, mu: f64, r: DVec3, frame: DMat3) -> DVec3 {
        frame * self.body_fixed_acceleration(mu, frame.transpose() * r)
    }

    /// Acceleration in the body-fixed frame, at body-fixed position `r`.
    pub fn body_fixed_acceleration(&self, mu: f64, r: DVec3) -> DVec3 {
        let max_degree = self.coefficients.iter().map(|c| c.0).max().unwrap_or(0) as usize;
        let (v, w) = self.cunningham(r, max_degree + 1);
        let mut a = DVec3::ZERO;
        for &(n, m, c, s) in &self.coefficients {
            let (n, m) = (n as usize, m as usize);
            if m == 0 {
                a.x -= c * v[n + 1][1];
                a.y -= c * w[n + 1][1];
            } else {
                // (n − m + 2)! / (n − m)!
                let f = ((n - m + 1) * (n - m + 2)) as f64;
                a.x += 0.5 * (-c * v[n + 1][m + 1] - s * w[n + 1][m + 1])
                    + 0.5 * f * (c * v[n + 1][m - 1] + s * w[n + 1][m - 1]);
                a.y += 0.5 * (-c * w[n + 1][m + 1] + s * v[n + 1][m + 1])
                    + 0.5 * f * (-c * w[n + 1][m - 1] + s * v[n + 1][m - 1]);
            }
            a.z += (n - m + 1) as f64 * (-c * v[n + 1][m] - s * w[n + 1][m]);
        }
        a * (mu / (self.radius * self.radius))
    }

    /// The potential U (J/kg, positive, force = +∇U) of these terms at
    /// body-fixed position `r`.
    pub fn body_fixed_potential(&self, mu: f64, r: DVec3) -> f64 {
        let max_degree = self.coefficients.iter().map(|c| c.0).max().unwrap_or(0) as usize;
        let (v, w) = self.cunningham(r, max_degree);
        let sum: f64 = self
            .coefficients
            .iter()
            .map(|&(n, m, c, s)| c * v[n as usize][m as usize] + s * w[n as usize][m as usize])
            .sum();
        sum * mu / self.radius
    }

    /// Cunningham's V_nm = (R/r)^(n+1) P_nm(sin φ) cos mλ and W_nm (with
    /// sin mλ), for n, m ≤ `degree`, by recurrence. Kept on the stack: this
    /// runs at every force evaluation for every moon.
    fn cunningham(&self, r: DVec3, degree: usize) -> (Table, Table) {
        assert!(
            degree < TABLE,
            "degree {degree} is above the supported {}",
            TABLE - 1
        );
        let mut v = [[0.0; TABLE + 1]; TABLE];
        let mut w = [[0.0; TABLE + 1]; TABLE];
        let r2 = r.length_squared();
        let scale = self.radius / r2;
        let (x, y, z) = (r.x * scale, r.y * scale, r.z * scale);
        let rho = self.radius * scale;
        v[0][0] = self.radius / r2.sqrt();
        for m in 0..=degree {
            if m > 0 {
                // Sectoral terms, from the one below them.
                let k = (2 * m - 1) as f64;
                v[m][m] = k * (x * v[m - 1][m - 1] - y * w[m - 1][m - 1]);
                w[m][m] = k * (x * w[m - 1][m - 1] + y * v[m - 1][m - 1]);
            }
            for n in m + 1..=degree {
                let (a, b) = FACTORS[n][m];
                let (v2, w2) = if n >= m + 2 {
                    (v[n - 2][m], w[n - 2][m])
                } else {
                    (0.0, 0.0)
                };
                v[n][m] = a * z * v[n - 1][m] - b * rho * v2;
                w[n][m] = a * z * w[n - 1][m] - b * rho * w2;
            }
        }
        (v, w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gravity::ZonalField;

    fn mars_like() -> TesseralField {
        TesseralField {
            radius: 3_396e3,
            coefficients: vec![
                (2, 1, 4.8e-10, 2.8e-11),
                (2, 2, -5.463e-5, 3.159e-5),
                (3, 1, 4.11e-6, 2.72e-5),
                (3, 3, 4.89e-6, 3.57e-6),
                (4, 2, -2.13e-7, -2.01e-6),
                (6, 5, 1.34e-9, 1.31e-9),
            ],
            frame: DMat3::IDENTITY,
            spin_rate: 7.088e-5,
            epoch: 0.0,
        }
    }

    #[test]
    fn c22_matches_its_closed_form() {
        // U22 = (μ/r)(R/r)² · 3 cos²φ · (C22 cos 2λ + S22 sin 2λ)
        //     = 3 μ R² (C22 (x² − y²) + 2 S22 x y) / r⁵
        let (c22, s22) = (-5.463e-5, 3.159e-5);
        let field = TesseralField {
            coefficients: vec![(2, 2, c22, s22)],
            ..mars_like()
        };
        let (mu, rr) = (4.2828e13, field.radius);
        let r = DVec3::new(6.0e6, -4.0e6, 3.0e6);
        let closed = 3.0 * mu * rr * rr * (c22 * (r.x * r.x - r.y * r.y) + 2.0 * s22 * r.x * r.y)
            / r.length().powi(5);
        let u = field.body_fixed_potential(mu, r);
        assert!((u - closed).abs() < 1e-12 * closed.abs(), "{u} vs {closed}");
    }

    #[test]
    fn acceleration_is_the_gradient_of_the_potential() {
        let field = mars_like();
        let mu = 4.2828e13;
        for r in [
            DVec3::new(9.4e6, 0.0, 0.0),
            DVec3::new(-1.5e7, 1.8e7, 2.0e6),
            DVec3::new(1.0e6, -2.0e6, 8.0e6),
            DVec3::new(0.0, 0.0, 6.0e6),
        ] {
            let h = 1.0;
            let u = |p: DVec3| field.body_fixed_potential(mu, p);
            let gradient = DVec3::new(
                u(r + DVec3::X * h) - u(r - DVec3::X * h),
                u(r + DVec3::Y * h) - u(r - DVec3::Y * h),
                u(r + DVec3::Z * h) - u(r - DVec3::Z * h),
            ) / (2.0 * h);
            let a = field.body_fixed_acceleration(mu, r);
            assert!(
                (a - gradient).length() < 1e-6 * a.length(),
                "at {r}: {a} vs {gradient}"
            );
        }
    }

    #[test]
    fn zonal_terms_agree_with_the_zonal_field() {
        // With m = 0 and C_n0 = −J_n, both formulations must give the same
        // pull: an independent check of the recurrences.
        let zonal = ZonalField {
            radius: 3_396e3,
            coefficients: vec![(2, 1.9566e-3), (3, 3.15e-5), (4, -1.54e-5), (6, -4.85e-6)],
            pole: DVec3::Z,
        };
        let field = TesseralField {
            coefficients: zonal
                .coefficients
                .iter()
                .map(|&(n, j)| (n, 0, -j, 0.0))
                .collect(),
            ..mars_like()
        };
        let mu = 4.2828e13;
        for r in [
            DVec3::new(9.4e6, 1.0e6, -2.0e6),
            DVec3::new(-3.0e6, 2.0e7, 5.0e6),
        ] {
            let a = field.body_fixed_acceleration(mu, r);
            let b = zonal.acceleration(mu, r);
            assert!((a - b).length() < 1e-12 * b.length(), "{a} vs {b}");
        }
    }

    #[test]
    fn the_field_turns_with_the_planet() {
        // After a quarter turn, the pull at a point is the pull the
        // unturned planet exerts at the point a quarter turn back.
        let field = mars_like();
        let mu = 4.2828e13;
        let r = DVec3::new(9.4e6, 1.0e6, 5.0e5);
        let quarter = std::f64::consts::FRAC_PI_2 / field.spin_rate;
        let turned = field.acceleration(mu, r, quarter);
        let back = DMat3::from_rotation_z(-std::f64::consts::FRAC_PI_2);
        let expected = back.transpose() * field.acceleration(mu, back * r, 0.0);
        assert!((turned - expected).length() < 1e-12 * turned.length());
    }
}
