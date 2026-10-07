//! Validation: the swarm of massless particles feels every massive body.
//!
//! A black hole streaking past a ring of asteroids must kick each one by
//! what the impulse approximation predicts: a body of mass M passing at
//! speed V along a straight line, at closest distance b, changes a still
//! particle's velocity toward the line by 2GM/(bV) for an infinite path;
//! over a path from z = −Z to +Z, by 2GM/(bV) · Z/√(Z² + b²) (Binney &
//! Tremaine 2008, *Galactic Dynamics*, §8.2). The Sun is kicked too, so in
//! the Sun's frame the particle's kick is the difference of the two.

use worldline_core::constants::{AU, GM_SUN, SOLAR_RADIUS};
use worldline_core::gravity::Newtonian;
use worldline_core::hierarchy::Hierarchy;
use worldline_core::swarm::Particle;
use worldline_core::{Body, DVec3, System};

/// The impulse a straight path from z = −Z to +Z along the line x = x₀,
/// y = 0 gives a still body at `at` (in the plane z = 0).
fn impulse(gm: f64, speed: f64, x0: f64, z: f64, at: DVec3) -> DVec3 {
    let toward = DVec3::new(x0 - at.x, -at.y, 0.0);
    let b = toward.length();
    toward / b * (2.0 * gm / (b * speed) * z / (z * z + b * b).sqrt())
}

/// The Sun, a black hole of mass `gm` crossing the ecliptic 3 AU out at
/// `speed`, from 10 AU below to 10 AU above, and a ring of asteroids 2.5 AU
/// from the Sun on circular orbits. Returns each asteroid's velocity
/// relative to the Sun when the hole has passed.
fn ring_after_flyby(gm: f64, speed: f64) -> (Vec<(DVec3, DVec3)>, f64) {
    let (x0, z) = (3.0 * AU, 10.0 * AU);
    let hole = Body::new("Hole", gm, 0.0)
        .at(DVec3::new(x0, 0.0, -z))
        .moving(DVec3::new(0.0, 0.0, speed));
    let mut h = Hierarchy::new(
        System::new(vec![Body::new("Sun", GM_SUN, SOLAR_RADIUS), hole]),
        Box::new(Newtonian),
        Vec::new(),
    );
    let r = 2.5 * AU;
    let v = (GM_SUN / r).sqrt();
    h.add_particles((0..72).map(|k| {
        let angle = k as f64 * 5f64.to_radians();
        let (s, c) = angle.sin_cos();
        Particle {
            position: DVec3::new(r * c, r * s, 0.0),
            velocity: DVec3::new(-v * s, v * c, 0.0),
            id: k,
        }
    }));
    let duration = 2.0 * z / speed;
    h.advance(duration);
    h.sync_swarm();
    let sun = &h.top.bodies[0];
    let ring = h
        .swarm()
        .particles
        .iter()
        .map(|p| (p.position - sun.position, p.velocity - sun.velocity))
        .collect();
    (ring, duration)
}

#[test]
fn a_black_hole_flyby_kicks_the_asteroids_as_the_impulse_approximation_says() {
    // A hole of 10 Suns at a tenth of the speed of light: it crosses in
    // 1.2 days, during which the asteroids, at 18.8 km/s, move 2 × 10⁹ m,
    // a small fraction of their distance to it. A twin run without the
    // hole's mass isolates its kicks.
    let (gm, speed) = (10.0 * GM_SUN, 2.997_924_58e7);
    let (with, duration) = ring_after_flyby(gm, speed);
    let (without, _) = ring_after_flyby(0.0, speed);
    let (x0, z) = (3.0 * AU, 10.0 * AU);
    let sun_kick = impulse(gm, speed, x0, z, DVec3::ZERO);
    let orbital = (GM_SUN / (2.5 * AU)).sqrt();
    // The impulse approximation takes each body as still while it is
    // kicked, at its place when the hole passes closest. It leaves out how
    // far the body moves during the kick, relative to the distance it is
    // kicked from: b/V of time at speed v over b, or the asteroid's speed
    // over the hole's, 6.3 × 10⁻⁴. Allow twice that.
    let allowed = 2.0 * orbital / speed;
    let mut worst: f64 = 0.0;
    let mut largest: f64 = 0.0;
    for (k, ((r, v), (_, v0))) in with.iter().zip(&without).enumerate() {
        // Where the asteroid is when the hole passes closest, halfway
        // through: it has moved on along its circle by ω z/V.
        let angle = k as f64 * 5f64.to_radians() + orbital / (2.5 * AU) * (z / speed);
        let at = DVec3::new(2.5 * AU * angle.cos(), 2.5 * AU * angle.sin(), 0.0);
        let predicted = impulse(gm, speed, x0, z, at) - sun_kick;
        let measured = *v - *v0;
        let error = (measured - predicted).length() / predicted.length();
        worst = worst.max(error);
        largest = largest.max(measured.length());
        assert!(r.length() > 2.0 * AU, "asteroid {k} stayed in the ring");
    }
    println!(
        "a hole of 10 Suns at 0.1c, crossing in {:.2} days: kicks up to {:.3} m/s; worst difference from the impulse approximation {:.1e} (allowed {:.1e})",
        duration / 86_400.0,
        largest,
        worst,
        allowed
    );
    assert!(worst < allowed);
}

#[test]
fn a_black_hole_swallows_the_asteroids_it_passes_through() {
    // A hole of a million Suns, its horizon 0.02 AU, dropped at rest in the
    // middle of the ring: the asteroids it reaches fall in and are
    // swallowed; momentum aside, none may be lost any other way.
    let gm = 1e6 * GM_SUN;
    let hole = Body::new("Hole", gm, 2.0 * gm / (299_792_458.0f64).powi(2)).at(DVec3::new(
        2.5 * AU,
        0.0,
        0.0,
    ) + DVec3::new(
        0.0,
        0.0,
        0.3 * AU,
    ));
    let mut h = Hierarchy::new(
        System::new(vec![Body::new("Sun", GM_SUN, SOLAR_RADIUS), hole]),
        Box::new(Newtonian),
        Vec::new(),
    );
    let r = 2.5 * AU;
    let v = (GM_SUN / r).sqrt();
    h.add_particles((0..72).map(|k| {
        let angle = k as f64 * 5f64.to_radians();
        let (s, c) = angle.sin_cos();
        Particle {
            position: DVec3::new(r * c, r * s, 0.0),
            velocity: DVec3::new(-v * s, v * c, 0.0),
            id: k,
        }
    }));
    h.advance(30.0 * 86_400.0);
    h.sync_swarm();
    let swallowed: usize = h.take_swallowed().iter().map(|s| s.ids.len()).sum();
    println!(
        "after 30 days: {swallowed} of 72 asteroids swallowed, {} left",
        h.swarm().len()
    );
    assert_eq!(swallowed + h.swarm().len(), 72);
    assert!(swallowed > 0);
}
