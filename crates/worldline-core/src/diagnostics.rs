//! Conserved quantities, used to check that the simulation is honest.
//!
//! In an isolated system, total energy, linear momentum and angular momentum
//! never change. A numerical method that lets them drift is wrong, so the
//! validation tests watch them.

use glam::DVec3;

use crate::System;
use crate::constants::G;

/// Total Newtonian energy (kinetic plus gravitational potential), in J.
pub fn newtonian_energy(system: &System) -> f64 {
    // Work with μ = GM throughout and divide by G once at the end, so the
    // imprecise G only scales the result and never mixes into the sum.
    let bodies = &system.bodies;
    let mut kinetic = 0.0;
    let mut potential = 0.0;
    for (i, bi) in bodies.iter().enumerate() {
        kinetic += 0.5 * bi.gm * bi.velocity.length_squared();
        for bj in &bodies[i + 1..] {
            potential -= bi.gm * bj.gm / (bj.position - bi.position).length();
        }
    }
    (kinetic + potential) / G
}

/// Total linear momentum Σ m v, in kg m/s.
pub fn linear_momentum(system: &System) -> DVec3 {
    system
        .bodies
        .iter()
        .map(|b| b.gm * b.velocity)
        .sum::<DVec3>()
        / G
}

/// Total angular momentum about the origin, Σ m (r × v), in kg m²/s.
pub fn angular_momentum(system: &System) -> DVec3 {
    system
        .bodies
        .iter()
        .map(|b| b.gm * b.position.cross(b.velocity))
        .sum::<DVec3>()
        / G
}
