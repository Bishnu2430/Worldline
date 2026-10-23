//! Each body's own clock (step 3.4): the proper time it has lived through
//! since the simulation started, or since it was added, against a clock
//! far away. See `docs/physics/proper-time.md`.

use std::collections::HashMap;

use worldline_core::compact::horizon_spin;
use worldline_core::fields::{kerr_clock_slowdown, moving_clock_slowdown};
use worldline_core::{Body, DVec3};

/// One body's clock.
#[derive(Debug, Clone, PartialEq)]
pub struct Clock {
    /// The far-away clock's time when it started, s.
    pub since: f64,
    /// How far it has fallen behind the far-away clock since then, s:
    /// t − τ, added up so its digits don't drown in t.
    pub behind: f64,
    /// How much more slowly it runs now than the far-away clock, 1 − dτ/dt.
    pub slowdown: f64,
    /// The black hole the far-away clock is at rest with, when one holds
    /// it; otherwise the far-away clock is at rest in its region's frame.
    pub frame: Option<String>,
    /// The far-away clock's time when it was last read, s.
    last: f64,
}

impl Clock {
    /// The far-away time since it started, s.
    pub fn elapsed(&self) -> f64 {
        self.last - self.since
    }

    /// The time it has shown since it started, s.
    pub fn shown(&self) -> f64 {
        self.elapsed() - self.behind
    }
}

/// How to read a body's clock.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    /// Which body.
    pub index: usize,
    /// The velocity of its region's frame (the far-away clock's rest frame
    /// when no hole holds it), m/s.
    pub frame: DVec3,
    /// The black hole that holds it, if one does.
    pub hole: Option<usize>,
}

/// How much more slowly body `reading.index`'s clock runs than a far-away
/// clock: near the hole that holds it, exactly (Kerr), against a clock at
/// rest with the hole; elsewhere, in the weak fields of every other body,
/// against a clock at rest in its region's frame.
pub fn slowdown(bodies: &[Body], reading: Reading) -> Option<f64> {
    let body = &bodies[reading.index];
    match reading.hole {
        Some(h) => {
            let hole = &bodies[h];
            kerr_clock_slowdown(
                hole.gm,
                horizon_spin(hole.gm, hole.radius),
                body.position - hole.position,
                body.velocity - hole.velocity,
            )
        }
        None => moving_clock_slowdown(
            bodies,
            body.position,
            body.velocity - reading.frame,
            Some(reading.index),
        ),
    }
}

/// Every body's clock, by name.
#[derive(Debug, Default)]
pub struct Clocks {
    clocks: HashMap<String, Clock>,
}

impl Clocks {
    /// Reads the clocks of `readings` at far-away time `time`: each has
    /// fallen behind since its last reading by the trapezoid rule on its
    /// slowdown then and now; a new one starts here. A clock where none
    /// can run (inside a horizon) keeps its last rate.
    pub fn tick(&mut self, time: f64, bodies: &[Body], readings: impl Iterator<Item = Reading>) {
        for reading in readings {
            let body = &bodies[reading.index];
            let frame = reading.hole.map(|h| bodies[h].name.clone());
            let now = slowdown(bodies, reading);
            match self.clocks.get_mut(&body.name) {
                Some(clock) => {
                    let now = now.unwrap_or(clock.slowdown);
                    clock.behind += 0.5 * (clock.slowdown + now) * (time - clock.last);
                    clock.slowdown = now;
                    clock.last = time;
                    clock.frame = frame;
                }
                None => {
                    self.clocks.insert(
                        body.name.clone(),
                        Clock {
                            since: time,
                            behind: 0.0,
                            slowdown: now.unwrap_or(0.0),
                            frame,
                            last: time,
                        },
                    );
                }
            }
        }
    }

    /// The clock of the body called `name`, if it has one.
    pub fn get(&self, name: &str) -> Option<&Clock> {
        self.clocks.get(name)
    }

    /// Forgets the clocks of bodies no longer in `bodies`.
    pub fn keep_only(&mut self, bodies: &[Body]) {
        self.clocks
            .retain(|name, _| bodies.iter().any(|b| &b.name == name));
    }
}
