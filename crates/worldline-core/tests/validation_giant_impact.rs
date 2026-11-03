//! Step 6.4: planets of rock and iron as fluids, and the Moon-forming
//! impact against Canup's (2004) simulations.

use std::time::Instant;

use worldline_core::constants::G;
use worldline_core::giant_impact::{
    EARTH_MASS, EARTH_MOON_ANGULAR_MOMENTUM, Impact, LUNAR_MASS, TARGET_IRON, TARGET_ROCK, measure,
    settled, structure,
};
use worldline_core::sph::Fluid;

/// Particles in the impact unless `WORLDLINE_PARTICLES` says otherwise.
const PARTICLES: usize = 8000;

fn total_momentum(fluid: &Fluid) -> f64 {
    let sum: worldline_core::DVec3 = fluid.particles.iter().map(|p| p.velocity * p.mass).sum();
    sum.length()
}

fn angular_momentum(fluid: &Fluid) -> worldline_core::DVec3 {
    fluid
        .particles
        .iter()
        .map(|p| p.position.cross(p.velocity) * p.mass)
        .sum()
}

#[test]
fn an_earth_of_rock_and_iron_is_in_balance() {
    let earth = structure(EARTH_MASS, 0.3, 2000.0);
    let center = earth.shells.last().unwrap();
    let core = earth.shells.iter().find(|s| s.iron).unwrap();
    let above = earth.shells.iter().rfind(|s| !s.iron).unwrap();
    println!(
        "radius {:.0} km (Earth 6,371), core {:.0} km (3,480); {:.0} to {:.0} kg/m³ at the core's top, {:.0} GPa (about 5,500 to 9,900, 136); {:.0} GPa at the center (about 364)",
        earth.radius / 1e3,
        core.radius / 1e3,
        above.density,
        core.density,
        core.pressure / 1e9,
        center.pressure / 1e9,
    );
    // Pressure balances gravity at every depth: dP/dr = −ρGm/r², by
    // differences over one shell. Averaging across a shell of thickness h
    // has an error of about (h/r)² of the weight, so that's the limit, and
    // `worst` is the error in units of it. The density jumps at the core.
    // The mass integrates to 3 × 10⁻¹⁰ of Earth's: that crumb, left at the
    // very center, isn't weighed.
    let mut worst: f64 = 0.0;
    for s in earth.shells.windows(2) {
        if s[0].iron != s[1].iron || s[1].radius < 0.01 * earth.radius {
            continue;
        }
        let r = 0.5 * (s[0].radius + s[1].radius);
        let mass = 0.5 * (s[0].inside + s[1].inside) - center.inside;
        let weight = G * mass * 0.5 * (s[0].density + s[1].density) / (r * r);
        let slope = (s[1].pressure - s[0].pressure) / (s[0].radius - s[1].radius);
        let thickness = s[0].radius - s[1].radius;
        worst = worst.max(((slope - weight) / weight).abs() / (thickness / r).powi(2));
    }
    println!(
        "pressure against gravity: off by at most {worst:.2} of (h/r)²; {:.1e} of the mass left over",
        center.inside / EARTH_MASS
    );
    assert!(worst < 1.0, "{worst}");
}

#[test]
#[ignore = "about a minute"]
fn a_settled_planet_stays_settled() {
    let mass = 0.13 * 1.019 * EARTH_MASS;
    let body = structure(mass, 0.3, 2000.0);
    let start = Instant::now();
    let mut fluid = settled(&body, 2000, TARGET_ROCK, TARGET_IRON);
    println!(
        "{} particles settled in {:.1} s, half-mass radius {:.0} km of {:.0} km",
        fluid.particles.len(),
        start.elapsed().as_secs_f64(),
        fluid.mass_radius(0.5) / 1e3,
        body.radius / 1e3
    );
    let energy = fluid.energy();
    let end = fluid.time + 10.0 * 3600.0;
    let mut fastest: f64 = 0.0;
    let mut steps = 0;
    while fluid.time < end {
        fluid.step();
        steps += 1;
        let rms = (fluid
            .particles
            .iter()
            .map(|p| p.velocity.length_squared())
            .sum::<f64>()
            / fluid.particles.len() as f64)
            .sqrt();
        fastest = fastest.max(rms);
    }
    println!(
        "left alone for 10 hours ({steps} steps): r.m.s. speed at most {fastest:.1} m/s; \
         energy kept to {:.1e}; {:.1} s in all",
        (fluid.energy() / energy - 1.0).abs(),
        start.elapsed().as_secs_f64()
    );
    assert!(fastest < 50.0, "{fastest}");
}

#[test]
#[ignore = "minutes"]
fn the_canonical_impact_leaves_the_disk_canup_found() {
    let count = std::env::var("WORLDLINE_PARTICLES")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(PARTICLES);
    let clock = Instant::now();
    let start = Impact::CANONICAL.set_up(count);
    let mut fluid = start.fluid;
    println!(
        "{} particles settled in {:.0} s: target {:.0} km, impactor {:.0} km, meeting at {:.2} km/s, \
         angular momentum {:.3} L_EM",
        fluid.particles.len(),
        clock.elapsed().as_secs_f64(),
        start.target_radius / 1e3,
        start.impactor_radius / 1e3,
        start.escape_speed / 1e3,
        start.angular_momentum / EARTH_MOON_ANGULAR_MOMENTUM,
    );
    let scale = fluid
        .particles
        .iter()
        .map(|p| p.mass * p.velocity.length())
        .sum::<f64>();
    let energy = fluid.energy();
    let spin = angular_momentum(&fluid);
    println!(
        "start: energy {energy:.4e} J, L {:.4} L_EM, about {:.0} steps at {:.1} s",
        spin.length() / EARTH_MOON_ANGULAR_MOMENTUM,
        27.0 * 3600.0 / fluid.time_step(),
        fluid.time_step(),
    );
    let (mut steps, mut next_report) = (0usize, 3.0 * 3600.0);
    let end = 27.0 * 3600.0;
    while fluid.time < end {
        fluid.step();
        steps += 1;
        if fluid.time >= next_report {
            let disk = measure(&fluid.particles);
            println!(
                "{:>4.1} h ({steps} steps, {:.0} s): disk {:.3} M_L in {} particles, escaping {:.3} M_L, \
                 planet {:.4} M_E, dt {:.1} s",
                fluid.time / 3600.0,
                clock.elapsed().as_secs_f64(),
                disk.disk / LUNAR_MASS,
                disk.particles,
                disk.escaping / LUNAR_MASS,
                disk.planet / EARTH_MASS,
                fluid.time_step(),
            );
            next_report += 3.0 * 3600.0;
        }
    }
    let disk = measure(&fluid.particles);
    let energy_off = (fluid.energy() / energy - 1.0).abs();
    let spin_off = (angular_momentum(&fluid) - spin).length() / spin.length();
    println!(
        "after 27 hours, {steps} steps, {:.0} s:",
        clock.elapsed().as_secs_f64()
    );
    println!(
        "  disk {:.3} M_L in {} particles (Canup 1.62 in 2,203), by periapsis {:.3} M_L",
        disk.disk / LUNAR_MASS,
        disk.particles,
        disk.above_periapsis / LUNAR_MASS
    );
    println!(
        "  beyond the Roche limit {:.3} M_L (0.92), escaping {:.3} M_L (0.41), planet {:.4} M_E (0.994)",
        disk.beyond_roche / LUNAR_MASS,
        disk.escaping / LUNAR_MASS,
        disk.planet / EARTH_MASS
    );
    println!(
        "  disk's L {:.3} L_EM (0.31), system's L {:.3} L_EM (1.18), day {:.2} h (4.6)",
        disk.disk_angular_momentum / EARTH_MOON_ANGULAR_MOMENTUM,
        disk.bound_angular_momentum / EARTH_MOON_ANGULAR_MOMENTUM,
        disk.day / 3600.0
    );
    println!(
        "  iron {:.1}% of the disk (5%), from the impactor {:.0}% (82%), moon {:.2} M_L (1.38)",
        100.0 * disk.iron / disk.disk,
        100.0 * disk.from_impactor / disk.disk,
        disk.moon / LUNAR_MASS
    );
    println!(
        "  kept: energy {energy_off:.1e}, angular momentum {spin_off:.1e}, momentum {:.1e}",
        total_momentum(&fluid) / scale
    );
    // Canup's warm starts at γ = 0.13, v = v_esc and 0.723 ≤ b′ ≤ 0.740 span
    // 1.33 to 1.62 M_L, widened by the counting noise of this disk.
    let noise = (disk.particles as f64).sqrt() / disk.particles as f64;
    let mass = disk.disk / LUNAR_MASS;
    println!(
        "  limit {:.2} to {:.2} M_L with the counting noise of {:.1}%",
        1.33 * (1.0 - noise),
        1.62 * (1.0 + noise),
        100.0 * noise
    );
    assert!(mass >= 1.33 * (1.0 - noise) && mass <= 1.62 * (1.0 + noise));
}
