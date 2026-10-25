//! When a body's tides matter, and where a point body has to become a
//! fluid (step 4.3). See `docs/physics/tides.md`.
//!
//! A body of mass m and radius R at distance D from a mass M is stretched
//! along the line to it. In equilibrium, to first order, its surface rises
//! there by
//!
//!   δR/R = h₂ (M/m)(R/D)³ = h₂ (r_t/D)³,   r_t = R (M/m)^(1/3),
//!
//! r_t being its tidal radius. A fluid body's surface stays level with its
//! own and the tide's potential, so h₂ = 1 + k₂, k₂ its Love number: 3/2
//! for a body of even density, 15/π² − 1 for an n = 1 polytrope.
//!
//! A point leaves that stretch out. It is graded as every model is (see
//! `regime.rs`): approximate while it is under 1%, beyond after. So a body
//! becomes a fluid where its tides would stretch it by 1%, and turns back
//! into a point once they fall to half that.

use std::f64::consts::PI;

/// The tidal stretch at which a point body becomes a fluid: the 1% line
/// past which every model in Worldline is graded beyond its range.
pub const FLUID_STRETCH: f64 = 0.01;

/// The tidal stretch below which a fluid turns back into a point: half the
/// line it became one at, so a body grazing the line doesn't flicker
/// between the two.
pub const POINT_STRETCH: f64 = 0.5 * FLUID_STRETCH;

/// Earth's GM, IAU 2015 Resolution B3's nominal value, m³/s².
pub const GM_EARTH_NOMINAL: f64 = 3.986_004e14;

/// Jupiter's GM, IAU 2015 Resolution B3's nominal value, m³/s².
pub const GM_JUPITER_NOMINAL: f64 = 1.266_865_3e17;

/// The tidal radius of a body of `radius` and gravitational parameter `gm`
/// near one of `gm_other`: R (M/m)^(1/3). Tides of that body tear it apart
/// about there.
pub fn tidal_radius(radius: f64, gm: f64, gm_other: f64) -> f64 {
    radius * (gm_other / gm).cbrt()
}

/// The Love number k₂ of an n = 1 polytrope (P = Kρ²): 15/π² − 1, twice
/// its apsidal constant (15 − π²)/(2π²).
pub fn polytrope_love_number() -> f64 {
    15.0 / (PI * PI) - 1.0
}

/// How much tides stretch a fluid body of displacement Love number `h2`
/// at `distance` from what raises them, `tidal_radius` being its tidal
/// radius there: h₂ (r_t/D)³, as a fraction of its radius.
pub fn stretch(h2: f64, tidal_radius: f64, distance: f64) -> f64 {
    h2 * (tidal_radius / distance).powi(3)
}

/// The distance at which the tides of a body stretch it by `stretch`:
/// r_t (h₂/stretch)^(1/3).
pub fn distance_for(h2: f64, tidal_radius: f64, stretch: f64) -> f64 {
    tidal_radius * (h2 / stretch).cbrt()
}

/// The radius of the region around a body of `gm` at `distance` from one
/// of `gm_other` within which its own gravity holds things: its Hill
/// radius, D (m/3M)^(1/3).
pub fn hill_radius(distance: f64, gm: f64, gm_other: f64) -> f64 {
    distance * (gm / (3.0 * gm_other)).cbrt()
}

/// How long two bodies `distance` apart, moving at `speed` relative to
/// each other, take at least to close to `stop`, if they pull on each
/// other with G(M + m) = `gm`: (D − stop)/(v + √(2G(M + m)/stop)). Falling
/// from D to any r ≥ stop speeds them up to at most √(v² + 2G(M + m)/r),
/// less than that sum. Others' pulls, much weaker where the two are this
/// close, are left out.
pub fn time_to_close(distance: f64, stop: f64, speed: f64, gm: f64) -> f64 {
    (distance - stop).max(0.0) / (speed + (2.0 * gm / stop).sqrt())
}

/// Whether a body of `gm` is a giant planet, whose bulk is fluid: from 10
/// Earth masses, about where a growing planet holds on to a hydrogen and
/// helium envelope (Pollack et al. 1996), to 13 Jupiter masses, where
/// deuterium starts to burn and brown dwarfs begin (the IAU's working
/// definition of a planet, 2003).
pub fn giant_planet(gm: f64) -> bool {
    (10.0 * GM_EARTH_NOMINAL..=13.0 * GM_JUPITER_NOMINAL).contains(&gm)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Love number of a body of density ρ(ξ) and mean density inside ξ
    /// ρ̄(ξ), out to its surface at `surface`, from Radau's equation for
    /// η = d ln ε/d ln r of its level surfaces' flattening ε:
    ///   ξ η′ + 6 (ρ/ρ̄)(η + 1) + η(η − 1) = 6,  η(0) = 0,
    /// and k₂ = (3 − η)/(2 + η) at the surface (Sterne 1939).
    fn radau_love_number(
        density: impl Fn(f64) -> f64,
        mean: impl Fn(f64) -> f64,
        surface: f64,
        start: (f64, f64),
    ) -> f64 {
        let slope = |xi: f64, eta: f64| {
            (6.0 - 6.0 * density(xi) / mean(xi) * (eta + 1.0) - eta * (eta - 1.0)) / xi
        };
        let (mut xi, mut eta) = start;
        let n = 20_000;
        let h = (surface - xi) / n as f64;
        for _ in 0..n {
            let k1 = slope(xi, eta);
            let k2 = slope(xi + 0.5 * h, eta + 0.5 * h * k1);
            let k3 = slope(xi + 0.5 * h, eta + 0.5 * h * k2);
            let k4 = slope(xi + h, eta + h * k3);
            eta += h / 6.0 * (k1 + 2.0 * k2 + 2.0 * k3 + k4);
            xi += h;
        }
        (3.0 - eta) / (2.0 + eta)
    }

    #[test]
    fn radau_gives_the_love_numbers() {
        // Even density: η stays 0, k₂ = 3/2.
        let even = radau_love_number(|_| 1.0, |_| 1.0, 1.0, (1e-3, 0.0));
        assert!((even - 1.5).abs() < 1e-12, "{even}");
        // n = 1: ρ = sin ξ/ξ, ρ̄ = 3(sin ξ − ξ cos ξ)/ξ³, out to ξ = π;
        // near the center η = 2ξ²/35.
        let xi0 = 1e-3;
        let n1 = radau_love_number(
            |x: f64| x.sin() / x,
            |x: f64| 3.0 * (x.sin() - x * x.cos()) / x.powi(3),
            PI,
            (xi0, 2.0 * xi0 * xi0 / 35.0),
        );
        assert!(
            (n1 - polytrope_love_number()).abs() < 1e-9,
            "Radau {n1} against {}",
            polytrope_love_number()
        );
    }

    #[test]
    fn a_giant_planet_becomes_a_fluid_at_5_3_tidal_radii() {
        // h₂ = 15/π² for n = 1; 1% of R at r_t (100 h₂)^(1/3).
        let h2 = 1.0 + polytrope_love_number();
        assert!((h2 - 15.0 / (PI * PI)).abs() < 1e-15);
        let fluid = distance_for(h2, 1.0, FLUID_STRETCH);
        assert!((fluid - 5.336).abs() < 1e-3, "{fluid}");
        assert!((stretch(h2, 1.0, fluid) - FLUID_STRETCH).abs() < 1e-15);
        let point = distance_for(h2, 1.0, POINT_STRETCH);
        assert!((point / fluid - 2f64.cbrt()).abs() < 1e-12);
        // Jupiter near a black hole of 9.6 Suns: r_t is 21.6 of its radii.
        let rt = tidal_radius(6.9911e7, GM_JUPITER_NOMINAL, 9.62 * 1.327_124_4e20);
        assert!((rt / 6.9911e7 - 21.6).abs() < 0.05, "{}", rt / 6.9911e7);
    }

    #[test]
    fn the_giant_planets_are_jupiter_to_neptune() {
        let gm = |earths: f64| earths * GM_EARTH_NOMINAL;
        // Earth, Uranus, Neptune, Saturn, Jupiter, a brown dwarf.
        assert!(!giant_planet(gm(1.0)));
        assert!(giant_planet(gm(14.54)) && giant_planet(gm(17.15)));
        assert!(giant_planet(gm(95.16)) && giant_planet(GM_JUPITER_NOMINAL));
        assert!(!giant_planet(20.0 * GM_JUPITER_NOMINAL));
    }

    #[test]
    fn two_bodies_never_close_faster_than_the_bound() {
        // Falling straight in from rest at D: at r, v = √(2GM(1/r − 1/D)).
        // The time to fall from D to stop always exceeds the bound.
        let (gm, d, stop) = (1.0, 10.0, 2.0);
        // Free-fall time from D to r, by the cycloid: t = √(D³/2GM)
        // (√(x(1 − x)) + arccos √x), x = r/D.
        let fall = |r: f64| {
            let x = r / d;
            (d.powi(3) / (2.0 * gm)).sqrt() * ((x * (1.0 - x)).sqrt() + x.sqrt().acos())
        };
        let actual = fall(stop);
        let bound = time_to_close(d, stop, 0.0, gm);
        assert!(bound < actual, "bound {bound}, actual {actual}");
        assert_eq!(time_to_close(stop, stop, 3.0, gm), 0.0);
    }
}
