//! The eclipse finder: a copy of the simulation run ahead on its own
//! thread, its Sun, Earth and Moon sampled every 6 hours and searched for
//! eclipses as it goes, each kept as a save of the moment it is greatest,
//! to go to. See `docs/physics/eclipses.md`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};

use worldline_core::constants::{DAY, JULIAN_YEAR};
use worldline_core::eclipses::{Eclipse, Ephemeris};

use crate::simulation::Simulation;

/// How far ahead the finder looks.
pub const YEARS: f64 = 5.0;

/// How often the Sun, Earth and Moon are sampled: the Moon then
/// interpolates to about 10 m (see [`Ephemeris`]).
const EVERY: f64 = 6.0 * 3600.0;

/// How far the copy runs between searches, and reports how far it has
/// got.
const CHUNK: f64 = 30.0 * DAY;

/// An eclipse found ahead.
pub struct Found {
    /// At its greatest, s on the simulation's clock.
    pub eclipse: Eclipse,
    /// When, as a Julian Date (TDB).
    pub jd_tdb: f64,
    /// The simulation at that moment, as a save.
    pub save: String,
}

enum Message {
    Found(Found),
    /// Searched up to this Julian Date (TDB).
    Searched(f64),
    Done,
}

/// A search under way, or finished.
pub struct Finder {
    /// The eclipses found so far, in order of time.
    pub found: Vec<Found>,
    /// Where the search started and where it ends, Julian Dates (TDB).
    pub from: f64,
    pub until: f64,
    /// How far it has searched.
    pub searched: f64,
    pub done: bool,
    receiver: Receiver<Message>,
    stop: Arc<AtomicBool>,
}

impl Finder {
    /// Starts searching the next [`YEARS`] of `simulation`, from now.
    pub fn start(simulation: &mut Simulation) -> Self {
        let save = simulation.save();
        let from = simulation.julian_date();
        let until = from + YEARS * JULIAN_YEAR / DAY;
        let (sender, receiver) = channel();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        std::thread::spawn(move || {
            let Ok(mut runner) = Simulation::load(&save, DAY) else {
                let _ = sender.send(Message::Done);
                return;
            };
            let end = runner.time_at(until);
            let jd = |sim: &Simulation, t: f64| sim.julian_date() + (t - sim.time()) / DAY;
            // The saves at the start of this chunk and the last, to replay
            // from; and the last chunk's final samples, so a minimum near
            // its end has neighbours on both sides.
            let (mut previous, mut tail) = (save, Ephemeris::default());
            let mut last = f64::NEG_INFINITY;
            while runner.time() < end && !stopped.load(Ordering::Relaxed) {
                let start = runner.time();
                let start_save = runner.save();
                let chunk_end = (start + CHUNK).min(end);
                let chunk = runner.ephemeris(chunk_end, EVERY);
                let mut ephemeris = tail;
                ephemeris.times.extend(&chunk.times);
                ephemeris.states.extend(&chunk.states);
                // Each eclipse once: those of the day before this chunk's
                // end wait for the next, with neighbours after them.
                let accept = if chunk_end < end {
                    chunk_end - DAY
                } else {
                    f64::INFINITY
                };
                for eclipse in ephemeris.eclipses() {
                    let t = eclipse.time();
                    if t <= last + DAY || t >= accept {
                        continue;
                    }
                    last = t;
                    let from = if t >= start { &start_save } else { &previous };
                    let Ok(mut replay) = Simulation::load(from, DAY) else {
                        continue;
                    };
                    replay.advance_by(t - replay.time());
                    let found = Found {
                        eclipse,
                        jd_tdb: jd(&runner, t),
                        save: replay.save(),
                    };
                    if sender.send(Message::Found(found)).is_err() {
                        return;
                    }
                }
                let keep = chunk.times.partition_point(|&s| s < chunk_end - 2.0 * DAY);
                tail = Ephemeris {
                    times: chunk.times[keep..chunk.times.len() - 1].to_vec(),
                    states: chunk.states[keep..chunk.states.len() - 1].to_vec(),
                };
                previous = start_save;
                if sender
                    .send(Message::Searched(runner.julian_date()))
                    .is_err()
                {
                    return;
                }
            }
            let _ = sender.send(Message::Done);
        });
        Self {
            found: Vec::new(),
            from,
            until,
            searched: from,
            done: false,
            receiver,
            stop,
        }
    }

    /// Takes in what the search has found since the last call.
    pub fn poll(&mut self) {
        while let Ok(message) = self.receiver.try_recv() {
            match message {
                Message::Found(found) => self.found.push(found),
                Message::Searched(jd) => self.searched = jd,
                Message::Done => self.done = true,
            }
        }
    }
}

impl Drop for Finder {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::eclipses::{
        EARTH_EQUATORIAL_RADIUS, EarthShadow, MoonShadow, State, apparent,
    };

    #[test]
    #[ignore = "slow: runs the solar system ahead to its first eclipse"]
    fn going_to_an_eclipse_lands_on_its_greatest() {
        // The finder's first find from the 2025 snapshot is the total lunar
        // eclipse of 2025 March 14. Its save, loaded (what "Go to" does),
        // is that moment, and the shadow measured on the loaded
        // simulation's own Sun, Earth and Moon gives the same γ, as far as
        // the ephemeris interpolates the Moon: (6 h)⁴/384 × ω⁴ r, about 11
        // m, for its month's ω and 384,000 km.
        let mut sim = Simulation::solar_system(DAY);
        let mut finder = Finder::start(&mut sim);
        while finder.found.is_empty() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            finder.poll();
            assert!(!finder.done, "nothing found");
        }
        let found = &finder.found[0];
        let loaded = Simulation::load(&found.save, DAY).unwrap();
        let state = |name: &str| {
            let body = &loaded.bodies[loaded.index_of(name).unwrap()];
            State {
                position: body.position,
                velocity: body.velocity,
            }
        };
        let (sun, moon) = apparent(state("Sun"), state("Earth"), state("Moon"));
        let (gamma, measured) = match found.eclipse {
            Eclipse::Solar { gamma, .. } => {
                let shadow = MoonShadow::new(sun, moon);
                (gamma, shadow.gamma().copysign(shadow.y))
            }
            Eclipse::Lunar { gamma, .. } => (gamma, EarthShadow::new(sun, moon).gamma),
        };
        let omega = std::f64::consts::TAU / (27.321_661 * DAY);
        let bound = (6.0 * 3600.0f64).powi(4) / 384.0 * omega.powi(4) * 3.844e8;
        println!(
            "{} TDB: γ {gamma:+.7} found, {measured:+.7} loaded: {:.1} m apart (bound {bound:.1} m)",
            crate::calendar::to_the_second(found.jd_tdb),
            (gamma - measured).abs() * EARTH_EQUATORIAL_RADIUS
        );
        assert!((loaded.julian_date() - found.jd_tdb).abs() * DAY < 1e-3);
        assert!(matches!(found.eclipse, Eclipse::Lunar { .. }));
        assert!((gamma - measured).abs() * EARTH_EQUATORIAL_RADIUS < bound);
    }
}
