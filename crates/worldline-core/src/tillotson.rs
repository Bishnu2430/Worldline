//! The Tillotson (1962) equation of state: rock and iron from solid to
//! vapor, for giant impacts (step 6.4). See `docs/physics/giant-impacts.md`.
//!
//! Pressure is a function of density and internal energy, in three regions
//! (Reinhardt & Stadel 2017, *MNRAS* 467, 4252, Appendix A; equation numbers
//! are theirs). Compressed or cold it is a solid's (eq. A1), expanded and
//! hot a vapor's, tending to an ideal gas (eq. A2), and in between a linear
//! mix of the two (eq. A3). It has no phase changes: a mixed state of melt
//! and vapor is only that interpolation.

/// A material, in SI units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tillotson {
    /// ρ₀: the density at zero pressure, kg/m³.
    pub density: f64,
    /// a: how much internal energy adds to the pressure, always.
    pub a: f64,
    /// b: how much more it adds while the material is near solid.
    pub b: f64,
    /// A: the bulk modulus, Pa.
    pub big_a: f64,
    /// B: the next term in compression, Pa.
    pub big_b: f64,
    /// u₀: the reference energy, J/kg.
    pub u0: f64,
    /// u_s: the energy at which vaporization begins, J/kg.
    pub incipient: f64,
    /// u′_s: the energy of complete vaporization, J/kg.
    pub complete: f64,
    /// α: how fast the vapor's solid term dies away with expansion.
    pub alpha: f64,
    /// β: how fast all that's left of the solid does.
    pub beta: f64,
}

impl Tillotson {
    /// Olivine (Marinova et al. 2011), the closest in the table to the
    /// dunite of Canup's (2004) planets.
    pub const OLIVINE: Self = Self {
        density: 3500.0,
        a: 0.5,
        b: 1.4,
        big_a: 1.31e11,
        big_b: 4.9e10,
        u0: 5.5e8,
        incipient: 4.5e6,
        complete: 1.5e7,
        alpha: 5.0,
        beta: 5.0,
    };

    /// Iron (Benz et al. 1987).
    pub const IRON: Self = Self {
        density: 7860.0,
        a: 0.5,
        b: 1.5,
        big_a: 1.28e11,
        big_b: 1.05e11,
        u0: 9.5e6,
        incipient: 1.42e6,
        complete: 8.45e6,
        alpha: 5.0,
        beta: 5.0,
    };

    /// The solid's pressure and its slopes in ρ and u (eq. A1).
    fn solid(&self, rho: f64, u: f64) -> (f64, f64, f64) {
        let eta = rho / self.density;
        let mu = eta - 1.0;
        let w = u / (self.u0 * eta * eta) + 1.0;
        let thermal = self.a + self.b / w;
        let pressure = thermal * u * rho + self.big_a * mu + self.big_b * mu * mu;
        let by_rho = thermal * u
            + 2.0 * self.b * u * (w - 1.0) / (w * w)
            + (self.big_a + 2.0 * self.big_b * mu) / self.density;
        let by_u = rho * (self.a + self.b / (w * w));
        (pressure, by_rho, by_u)
    }

    /// The vapor's pressure and its slopes in ρ and u (eq. A2).
    fn vapor(&self, rho: f64, u: f64) -> (f64, f64, f64) {
        let eta = rho / self.density;
        let mu = eta - 1.0;
        let z = 1.0 / eta - 1.0;
        let w = u / (self.u0 * eta * eta) + 1.0;
        let (near, far) = ((-self.alpha * z).exp(), (-self.beta * z * z).exp());
        let inner = self.b * u * rho / w + self.big_a * mu * near;
        let pressure = self.a * u * rho + inner * far;
        // dz/dρ = −1/(ηρ).
        let dz = -1.0 / (eta * rho);
        let inner_by_rho = self.b * u / w
            + 2.0 * self.b * u * (w - 1.0) / (w * w)
            + self.big_a * near / self.density
            - self.big_a * mu * near * self.alpha * dz;
        let by_rho = self.a * u + inner_by_rho * far - inner * far * 2.0 * self.beta * z * dz;
        let by_u = rho * (self.a + self.b * far / (w * w));
        (pressure, by_rho, by_u)
    }

    /// The pressure and its slopes in ρ and u, unclipped.
    fn slopes(&self, rho: f64, u: f64) -> (f64, f64, f64) {
        if rho >= self.density || u <= self.incipient {
            self.solid(rho, u)
        } else if u >= self.complete {
            self.vapor(rho, u)
        } else {
            // The mix of eq. A3: linear in u between the two.
            let span = self.complete - self.incipient;
            let (hot, cold) = ((u - self.incipient) / span, (self.complete - u) / span);
            let (pc, rc, uc) = self.solid(rho, u);
            let (pv, rv, uv) = self.vapor(rho, u);
            (
                hot * pv + cold * pc,
                hot * rv + cold * rc,
                hot * uv + cold * uc + (pv - pc) / span,
            )
        }
    }

    /// The pressure (Pa) and sound speed (m/s) at density `rho` and internal
    /// energy `u`. A cold, stretched material would pull: a fluid breaks
    /// into droplets there instead, so the pressure stops at zero (Melosh
    /// 1989, as in Reinhardt & Stadel). The sound speed is √(∂P/∂ρ|ₛ) =
    /// √(∂P/∂ρ + (P/ρ²) ∂P/∂u), which can then be imaginary: it is held at a
    /// tenth of the sound speed at rest, for the time steps and the viscosity
    /// only. That floor is Worldline's choice.
    pub fn state(&self, rho: f64, u: f64) -> (f64, f64) {
        let (pressure, by_rho, by_u) = self.slopes(rho, u);
        let pressure = pressure.max(0.0);
        let sound = by_rho + pressure / (rho * rho) * by_u;
        (pressure, sound.max(0.01 * self.big_a / self.density).sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BOTH: [Tillotson; 2] = [Tillotson::OLIVINE, Tillotson::IRON];

    #[test]
    fn at_rest_there_is_no_pressure_and_sound_goes_as_the_modulus() {
        for m in BOTH {
            let (pressure, sound) = m.state(m.density, 0.0);
            assert_eq!(pressure, 0.0);
            let wanted = (m.big_a / m.density).sqrt();
            assert!((sound / wanted - 1.0).abs() < 1e-14, "{sound} {wanted}");
        }
    }

    #[test]
    fn the_slopes_are_the_pressures_slopes() {
        // Against finite differences, in all three regions.
        for m in BOTH {
            for (eta, share) in [(1.5, 0.1), (0.7, 0.5), (0.5, 1.3), (0.9, 3.0), (0.2, 1.1)] {
                let (rho, u) = (eta * m.density, share * m.complete);
                let (_, by_rho, by_u) = m.slopes(rho, u);
                let (dr, du) = (1e-6 * rho, 1e-6 * u);
                let numeric_rho = (m.slopes(rho + dr, u).0 - m.slopes(rho - dr, u).0) / (2.0 * dr);
                let numeric_u = (m.slopes(rho, u + du).0 - m.slopes(rho, u - du).0) / (2.0 * du);
                assert!(
                    (by_rho / numeric_rho - 1.0).abs() < 1e-6,
                    "{eta} {share}: {by_rho} {numeric_rho}"
                );
                assert!(
                    (by_u / numeric_u - 1.0).abs() < 1e-6,
                    "{eta} {share}: {by_u} {numeric_u}"
                );
            }
        }
    }

    #[test]
    fn the_regions_meet_without_a_jump() {
        // A1 and A2 are the same where the density is ρ₀, and the mix meets
        // each at its ends.
        for m in BOTH {
            let u = 2.0 * m.complete;
            let (above, below) = (
                m.state(m.density * (1.0 + 1e-10), u).0,
                m.state(m.density * (1.0 - 1e-10), u).0,
            );
            assert!((above / below - 1.0).abs() < 1e-5, "{above} {below}");
            let rho = 0.5 * m.density;
            for (u, ends) in [
                (m.incipient, m.solid(rho, m.incipient).0),
                (m.complete, m.vapor(rho, m.complete).0),
            ] {
                let mix = |u: f64| m.slopes(rho, u).0;
                assert!((mix(u * (1.0 + 1e-10)) - ends).abs() < 1e-3 * ends.abs() + 1.0);
                assert!((mix(u * (1.0 - 1e-10)) - ends).abs() < 1e-3 * ends.abs() + 1.0);
            }
        }
    }

    #[test]
    fn a_thin_vapor_is_an_ideal_gas() {
        // At very low density only a u ρ is left: γ = 1 + a (A2).
        for m in BOTH {
            let (rho, u) = (1e-3 * m.density, 3.0 * m.complete);
            let (pressure, sound) = m.state(rho, u);
            assert!((pressure / (m.a * u * rho) - 1.0).abs() < 1e-12);
            let ideal = (m.a * (1.0 + m.a) * u).sqrt();
            assert!((sound / ideal - 1.0).abs() < 1e-3, "{sound} {ideal}");
        }
    }

    #[test]
    fn stretched_cold_rock_has_no_pull() {
        for m in BOTH {
            assert_eq!(m.state(0.5 * m.density, 0.0).0, 0.0);
            assert!(m.state(0.5 * m.density, 0.0).1 > 0.0);
        }
    }
}
