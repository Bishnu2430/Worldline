//! The scenario library (step 8.1): famous events and experiments, each set
//! up in one go from the solar system as it was on 1 January 2025, from
//! the Scenarios window or `--scenario KEY`. Each is built from what the
//! app can already do (the catalogue's bodies, its windows, its views), so
//! it runs on the same physics as everything else.

use super::film::Script;
use super::*;
use worldline_core::orbit::{kepler_period, periapsis_vector, relativistic_advance};

/// One scenario.
pub struct Scenario {
    /// For `--scenario`.
    pub key: &'static str,
    pub title: &'static str,
    /// What you'll see, and what it shows.
    pub about: &'static str,
    /// Sets it up, on a fresh solar system.
    start: fn(&mut WorldlineApp),
    /// Its film, if it plays as one (step 8.5a).
    pub film: Option<&'static Script>,
}

/// Every scenario, in the order the roadmap lists them.
pub const SCENARIOS: [Scenario; 12] = [
    Scenario {
        key: "mercury",
        title: "Mercury's perihelion",
        about: "Everything but the Sun and Mercury is taken away, at a century a second. The orbit's closest point turns by 43 arcseconds a century, general relativity's prediction; the inspector measures it as it plays.",
        start: mercury,
        film: Some(&super::films::MERCURY),
    },
    Scenario {
        key: "hulse-taylor",
        title: "The Hulse–Taylor pulsar",
        about: "A film of two minutes: PSR B1913+16 and its companion on their measured orbit, every 7.75 hours. Relativity turns the orbit, and their waves shrink it, its period by 2.4 trillionths of a second each second, as Hulse and Taylor's timing found (Nobel Prize, 1993). Like the mergers, it runs with the rest of the solar system taken away, so its quick steps don't hold up the planets'.",
        start: hulse_taylor,
        film: Some(&super::scripts::HULSE_TAYLOR),
    },
    Scenario {
        key: "double-pulsar",
        title: "The double pulsar",
        about: "A film of under two minutes: PSR J0737−3039A and B, the first pair of pulsars found, on their measured orbit, every 2.45 hours. Relativity turns their orbit 16.9° a year, four times as fast as the Hulse–Taylor pulsar's, and their waves shrink it, as Kramer et al. (2021) timed to 6 parts in 100,000. Run with the rest of the solar system taken away, like the Hulse–Taylor pair.",
        start: double_pulsar,
        film: Some(&super::films::DOUBLE_PULSAR),
    },
    Scenario {
        key: "gw150914",
        title: "GW150914: two black holes merge",
        about: "The first gravitational waves heard, from 36 and 31 Suns spiraling together, in real time from a 10 Hz wave: they merge nine seconds later. The waves window plays their chirp. The rest of the solar system is taken away, so their many steps a second don't hold up the planets'.",
        start: gw150914,
        film: Some(&super::films::GW150914),
    },
    Scenario {
        key: "gw170817",
        title: "GW170817: two neutron stars merge",
        about: "1.46 and 1.27 Suns, from a 20 Hz wave, in real time: they merge about three minutes later, into a neutron star that collapses into a black hole. The waves window plays their chirp. The rest of the solar system is taken away, so their many steps a second don't hold up the planets'.",
        start: gw170817,
        film: Some(&super::films::GW170817),
    },
    Scenario {
        key: "planet-black-hole",
        title: "A planet meets a black hole",
        about: "A Jupiter falls past Gaia BH1, 9.6 Suns, at half the distance its tides tear it apart at. Near it, it becomes a fluid on the GPU and is pulled apart.",
        start: planet_black_hole,
        film: Some(&super::films::PLANET_BLACK_HOLE),
    },
    Scenario {
        key: "ton-618",
        title: "TON 618 swallows Sagittarius A*",
        about: "One of the heaviest black holes known, 66 billion Suns, dropped 6,600 AU from the Milky Way's 4.3 million. Sagittarius A* falls in within half a year, at a month a second.",
        start: ton_618,
        film: Some(&super::films::TON_618),
    },
    Scenario {
        key: "white-dwarf",
        title: "A white dwarf past its limit",
        about: "Two copies of Sirius B, 1.02 Suns each, fall together from 0.05 AU. Together they're past Chandrasekhar's limit, 1.46 Suns: they explode as a type Ia supernova, half a day in, at an hour a second.",
        start: white_dwarf,
        film: Some(&super::films::WHITE_DWARF),
    },
    Scenario {
        key: "mini-black-hole",
        title: "An evaporating black hole",
        about: "A hypothetical 100,000-tonne black hole, circling the Sun just outside Earth's orbit, evaporates by Hawking radiation: 2.7 years, at a month a second, faster and hotter to the end.",
        start: mini_black_hole,
        film: Some(&super::films::MINI_BLACK_HOLE),
    },
    Scenario {
        key: "eclipse",
        title: "The next total solar eclipse",
        about: "The eclipse finder runs the simulation ahead to the total eclipse of 12 August 2026, over Greenland, Iceland and Spain, and shows the Moon's shadow on Earth.",
        start: eclipse,
        film: Some(&super::films::ECLIPSE),
    },
    Scenario {
        key: "moon-impact",
        title: "The Moon-forming impact",
        about: "Canup's (2004) giant impact, a Mars-sized planet striking the young Earth, as 8,000 particles of rock and iron in a window of its own: it leaves a disk the Moon forms from.",
        start: moon_impact,
        film: Some(&super::films::MOON_IMPACT),
    },
    Scenario {
        key: "black-hole-flyby",
        title: "A black hole through the Sun's neighborhood",
        about: "A copy of Sagittarius A*, 4.3 million Suns, crosses the Sun's neighborhood at 1,000 km/s, passing 3 pc from the Sun, at a century a second. Gaia's nearby stars near its path scatter. The planets are taken away: from parsecs off they're inside a point, and they'd hold the century a second up.",
        start: black_hole_flyby,
        film: Some(&super::films::BLACK_HOLE_FLYBY),
    },
];

/// The scenario with this key.
pub fn scenario(key: &str) -> Option<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.key == key)
}

/// A body's periapsis, watched as it turns: its direction in the orbit's
/// plane, from the Laplace–Runge–Lenz vector, sampled as the simulation
/// runs, and the rate fitted to the samples by least squares.
pub struct PerihelionWatch {
    pub body: String,
    around: String,
    /// The orbit's plane, as it was at the start: its periapsis then, and
    /// a quarter turn on.
    first: DVec3,
    quarter: DVec3,
    /// Time (s) and angle (rad, unwrapped) of each sample.
    samples: Vec<(f64, f64)>,
    /// General relativity's rate for the orbit as it was at the start,
    /// rad/s.
    pub theory: f64,
}

impl PerihelionWatch {
    pub(super) fn start(simulation: &Simulation, body: &str, around: &str) -> Option<Self> {
        let (r, v, mu) = relative(simulation, body, around)?;
        let lrl = periapsis_vector(r, v, mu);
        let first = lrl.normalize();
        let quarter = r.cross(v).normalize().cross(first);
        let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        let e = lrl.length() / mu;
        let mut watch = Self {
            body: body.to_string(),
            around: around.to_string(),
            first,
            quarter,
            samples: Vec::new(),
            theory: relativistic_advance(mu, a, e) / kepler_period(a, mu),
        };
        watch.sample(simulation);
        Some(watch)
    }

    /// Takes a sample, if time has moved on since the last.
    pub fn sample(&mut self, simulation: &Simulation) {
        let time = simulation.time();
        if self.samples.last().is_some_and(|&(t, _)| t >= time) {
            return;
        }
        let Some((r, v, mu)) = relative(simulation, &self.body, &self.around) else {
            return;
        };
        let lrl = periapsis_vector(r, v, mu);
        let mut angle = lrl.dot(self.quarter).atan2(lrl.dot(self.first));
        if let Some(&(_, last)) = self.samples.last() {
            angle += (last - angle + std::f64::consts::PI).div_euclid(std::f64::consts::TAU)
                * std::f64::consts::TAU;
        }
        self.samples.push((time, angle));
    }

    /// The direction of the periapsis when watching began.
    pub fn first_direction(&self) -> DVec3 {
        self.first
    }

    /// How long it has been watched, s.
    pub fn watched_for(&self) -> f64 {
        match (self.samples.first(), self.samples.last()) {
            (Some(first), Some(last)) => last.0 - first.0,
            _ => 0.0,
        }
    }

    /// How far it has turned since, rad.
    pub fn turned(&self) -> f64 {
        match (self.samples.first(), self.samples.last()) {
            (Some(first), Some(last)) => last.1 - first.1,
            _ => 0.0,
        }
    }

    /// Whether it's watched around the Sun (a perihelion), not a star of a
    /// pair (a periastron).
    pub fn around_the_sun(&self) -> bool {
        self.around == "Sun"
    }

    /// The rate fitted to the samples, rad/s, and the time they span (s),
    /// once they span something.
    pub fn rate(&self) -> Option<(f64, f64)> {
        let n = self.samples.len() as f64;
        let span = self.samples.last()?.0 - self.samples.first()?.0;
        if self.samples.len() < 3 || span <= 0.0 {
            return None;
        }
        let tm = self.samples.iter().map(|s| s.0).sum::<f64>() / n;
        let am = self.samples.iter().map(|s| s.1).sum::<f64>() / n;
        let (mut num, mut den) = (0.0, 0.0);
        for &(t, a) in &self.samples {
            num += (t - tm) * (a - am);
            den += (t - tm) * (t - tm);
        }
        Some((num / den, span))
    }
}

/// Body `body`'s place and velocity relative to `around`, and their GM.
fn relative(simulation: &Simulation, body: &str, around: &str) -> Option<(DVec3, DVec3, f64)> {
    let (i, j) = (simulation.index_of(body)?, simulation.index_of(around)?);
    let (b, c) = (&simulation.bodies[i], &simulation.bodies[j]);
    Some((
        b.position - c.position,
        b.velocity - c.velocity,
        b.gm + c.gm,
    ))
}

impl WorldlineApp {
    /// Starts `scenario` afresh: the solar system as it was on 1 January
    /// 2025, what was open closed, then the scenario's own setup, playing.
    pub(super) fn play_scenario(&mut self, scenario: &'static Scenario) {
        self.speed_index = DEFAULT_SPEED;
        self.reset();
        self.live = false;
        self.launch = None;
        self.tool = None;
        self.stop_sound();
        self.waves = None;
        self.eclipses_open = false;
        self.finder = None;
        self.go_to_when_found = None;
        self.go_to_total = false;
        self.watch = None;
        self.impact_lab = None;
        self.sky_open = false;
        self.options.overlays = Overlays::default();
        self.options.trails = true;
        self.selected = SUN;
        self.simulation.paused = false;
        (scenario.start)(self);
        self.simulation.speed = SPEEDS[self.speed_index].1;
        self.playing = Some(scenario.title);
        if let Some(script) = scenario.film {
            self.start_film(scenario.title, script);
        }
        // What it opened, so nothing appears unexplained.
        let mut opened = Vec::new();
        if self.waves.is_some() {
            opened.push("the gravitational-waves window");
        }
        if self.eclipses_open {
            opened.push("the eclipse finder");
        }
        if self.impact_lab.is_some() {
            opened.push("the impact's window");
        }
        if self.sky_open {
            opened.push("the sky window");
        }
        if self.options.overlays.tides {
            opened.push("the tides overlay");
        }
        let message = if self.film.is_some() {
            format!(
                "Playing the film of {}: Space pauses it, ← and → skip a shot, and dragging \
                 still looks around",
                scenario.title
            )
        } else if opened.is_empty() {
            format!("Playing: {}", scenario.title)
        } else {
            format!(
                "Playing: {}; it opened {}",
                scenario.title,
                opened.join(", ")
            )
        };
        self.status = Some((message, Instant::now()));
    }

    fn at_speed(&mut self, index: usize) {
        self.speed_index = index;
        self.simulation.speed = SPEEDS[index].1;
    }

    /// Adds catalogue entry `key`'s first body at `place`, moving at
    /// `velocity`, to the region of body `near`; selects it and returns
    /// its index.
    fn drop_entry(&mut self, key: &str, place: DVec3, velocity: DVec3, near: usize) -> usize {
        let entry = catalogue::entry(key).expect("in the catalogue");
        let body = entry.bodies(&self.simulation).remove(0);
        let index =
            self.simulation
                .add_body_near(body.at(place).moving(velocity), &entry.key, near);
        self.selected = index;
        index
    }

    /// The scenario window: each scenario, what it shows, and a button to
    /// play it.
    pub(super) fn scenarios_window(&mut self, ctx: &egui::Context) {
        let mut open = self.scenarios_open;
        let mut play = None;
        egui::Window::new("Scenarios")
            .open(&mut open)
            .default_pos(pos2(340.0, 70.0))
            .default_width(460.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Each starts afresh from the solar system as it was on 1 January 2025, \
                         replacing what's running, and plays on the same physics as everything \
                         else.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(520.0)
                    .show(ui, |ui| {
                        for scenario in &SCENARIOS {
                            ui.separator();
                            ui.horizontal(|ui| {
                                if ui.button("Play").clicked() {
                                    play = Some(scenario);
                                }
                                ui.label(RichText::new(scenario.title).strong());
                                if scenario.film.is_some() {
                                    theme::chip(ui, "film", theme::ACCENT).on_hover_text(
                                        "Plays as a film: shots, captions and cards, with the \
                                         physics running as always",
                                    );
                                }
                            });
                            ui.label(RichText::new(scenario.about).small());
                        }
                    });
            });
        self.scenarios_open = open;
        if let Some(scenario) = play {
            self.scenarios_open = false;
            self.play_scenario(scenario);
        }
    }

    /// Inspector rows for the body whose perihelion is watched (Mercury's
    /// scenario): how fast it turns, measured, and general relativity's
    /// rate.
    pub(super) fn perihelion_rows(&self, ui: &mut egui::Ui) {
        let Some(watch) = &self.watch else {
            return;
        };
        if self.simulation.bodies[self.selected].name != watch.body {
            return;
        }
        // Mercury's in arcseconds a century; a pair's, turning thousands of
        // times faster, in degrees a year, to three decimals: the rate
        // worked out from one moment's orbit swings in the fourth around a
        // tight pair's orbit (see docs/scenarios.md).
        let (name, unit, scale, decimals) = if watch.around_the_sun() {
            (
                "Perihelion turning",
                "arcseconds a century",
                100.0 * JULIAN_YEAR * 180.0 / PI * 3600.0,
                2,
            )
        } else {
            (
                "Periastron turning",
                "degrees a year",
                JULIAN_YEAR * 180.0 / PI,
                3,
            )
        };
        ui.label(key(name));
        ui.label(match watch.rate() {
            Some((rate, span)) => format!(
                "{:.decimals$} {unit}, measured over {:.1} years",
                rate * scale,
                span / JULIAN_YEAR
            ),
            None => "measuring…".to_string(),
        })
        .on_hover_text(
            "The direction of the orbit's closest point, from the Laplace–Runge–Lenz vector, \
             sampled every frame since watching began, its rate fitted by least squares",
        );
        ui.end_row();
        ui.label(key("General relativity says"));
        ui.label(format!(
            "{:.decimals$} {unit}: 6πGM/c²a(1 – e²) an orbit",
            watch.theory * scale
        ))
        .on_hover_text("Einstein (1915), for the orbit as it was when watching began");
        ui.end_row();
    }
}

/// Takes every body of the solar system's region away but those named in
/// `keep`: planets with their moons, dwarf planets, asteroids and comets.
fn clear_solar_system(app: &mut WorldlineApp, keep: &[&str]) {
    loop {
        let sim = &app.simulation;
        let next = (0..sim.bodies.len()).find(|&i| {
            !keep.contains(&sim.bodies[i].name.as_str())
                && sim.parent(i).is_none()
                && !sim.is_galactic(i)
                && !sim.is_neighbor(i)
                && !sim.is_deep_sky(i)
                && matches!(sim.removal(i), Removal::Allowed { .. })
        });
        match next {
            Some(i) => app.simulation.remove(i),
            None => break,
        }
    }
}

/// Adds a tight pair, `bodies` (relative to their center of mass), under
/// catalogue key `key`, 30 AU above the Sun and at rest relative to it,
/// with the rest of the solar system taken away: its steps, many a
/// second, would hold up all the planets'. Near the Sun a place is good to a
/// millimetre (8 kpc out, at the galactic center, only to 30 km, too coarse
/// for a pair 1,300 km apart). The Sun's tide on it is under 10⁻¹⁰ of their
/// pull on each other. Selects the first body and returns its index.
fn add_alone(app: &mut WorldlineApp, bodies: Vec<Body>, key: &str) -> Option<usize> {
    clear_solar_system(app, &["Sun"]);
    let sun = &app.simulation.bodies[SUN];
    let (place, velocity) = (sun.position + DVec3::Z * 30.0 * AU, sun.velocity);
    let first = bodies.first()?.name.clone();
    for body in bodies {
        let (p, v) = (place + body.position, velocity + body.velocity);
        app.simulation.add_body(body.at(p).moving(v), key);
    }
    let index = app.simulation.index_of(&first)?;
    app.selected = index;
    Some(index)
}

/// Everything else in the solar system goes, so what turns Mercury's
/// perihelion is general relativity alone (the planets' pulls would add
/// 531″ a century).
fn mercury(app: &mut WorldlineApp) {
    clear_solar_system(app, &["Sun", "Mercury"]);
    app.watch = PerihelionWatch::start(&app.simulation, "Mercury", "Sun");
    // Sampled once a frame, half an orbit apart at a century a second, a
    // trail would cut across the orbit.
    app.options.trails = false;
    app.camera.pitch = 80f64.to_radians();
    app.fly_to(SUN, 150.0);
    app.selected = app.simulation.index_of("Mercury").unwrap_or(SUN);
    app.at_speed(CENTURY);
}

fn hulse_taylor(app: &mut WorldlineApp) {
    let entry = catalogue::entry("binary:PSR B1913+16").expect("in the catalogue");
    let bodies = entry.bodies(&app.simulation);
    let Some(pulsar) = add_alone(app, bodies, &entry.key) else {
        return;
    };
    app.fly_to(pulsar, 3e5);
    app.selected = pulsar;
    app.at_speed(HOUR);
}

/// As the Hulse–Taylor pair: alone, 30 AU above the Sun, an hour a second.
fn double_pulsar(app: &mut WorldlineApp) {
    let entry = catalogue::entry("binary:PSR J0737-3039").expect("in the catalogue");
    let bodies = entry.bodies(&app.simulation);
    let Some(pulsar) = add_alone(app, bodies, &entry.key) else {
        return;
    };
    app.fly_to(pulsar, 1e5);
    app.selected = pulsar;
    app.at_speed(HOUR);
}

/// The two holes from a 10 Hz wave.
fn gw150914(app: &mut WorldlineApp) {
    let find = |name: &str| worldline_data::notable_object(name).expect("in the catalog");
    let (a, b) = worldline_core::gravity::circular_pair(
        find("GW150914 heavier hole").body(),
        find("GW150914 lighter hole").body(),
        GW150914_WAVE,
    );
    let Some(first) = add_alone(app, vec![a, b], "inspiral:GW150914") else {
        return;
    };
    app.fly_to(first, 25.0);
    app.selected = first;
    app.at_speed(REAL_TIME);
    app.open_waves();
}

/// GW150914's wave starts at this frequency, Hz: 10 Hz, half the
/// catalogue's, so it takes about nine seconds to merge, not one.
const GW150914_WAVE: f64 = 10.0;

fn gw170817(app: &mut WorldlineApp) {
    let entry = catalogue::entry("inspiral:GW170817").expect("in the catalogue");
    let bodies = entry.bodies(&app.simulation);
    let Some(star) = add_alone(app, bodies, &entry.key) else {
        return;
    };
    app.fly_to(star, 60.0);
    app.selected = star;
    app.at_speed(REAL_TIME);
    app.open_waves();
}

/// As `--add "Gaia BH1:40" --add "jupiter:2@Gaia BH1" --launch 0.071`:
/// from 2 AU at 0.071 of the circular speed, its closest approach is half
/// its tidal radius.
fn planet_black_hole(app: &mut WorldlineApp) {
    app.add_on_circle("Gaia BH1", 40.0, SUN, 1.0);
    let hole = app.selected;
    app.add_on_circle("jupiter", 2.0, hole, 0.071);
    let planet = app.selected;
    app.options.overlays.tides = true;
    app.fly_to(planet, 60.0);
    app.selected = planet;
    app.at_speed(WEEK);
}

/// Dropped at half the circular speed, Sagittarius A*'s orbit around it
/// reaches inside its horizon.
fn ton_618(app: &mut WorldlineApp) {
    let Some(sgr) = app.simulation.index_of("Sagittarius A*") else {
        return;
    };
    app.add_on_circle("TON 618", 6600.0, sgr, 0.5);
    let ton = app.selected;
    app.fly_to(ton, 15.0);
    app.selected = ton;
    app.at_speed(MONTH);
}

/// One copy circling the Sun 30 AU out, the other 0.05 AU above it, at
/// rest relative to it: they fall together in 12 hours.
fn white_dwarf(app: &mut WorldlineApp) {
    app.add_on_circle("Sirius B", 30.0, SUN, 1.0);
    let first = app.selected;
    let (place, velocity) = (
        app.simulation.bodies[first].position,
        app.simulation.bodies[first].velocity,
    );
    app.drop_entry(
        "object:Sirius B",
        place + DVec3::Z * 0.05 * AU,
        velocity,
        first,
    );
    app.fly_to(first, 2500.0);
    app.selected = first;
    app.at_speed(HOUR);
}

/// Circling the Sun 1.02 AU out: around Earth, its short orbit would set
/// the planets' steps.
fn mini_black_hole(app: &mut WorldlineApp) {
    app.add_on_circle("Mini black hole", 1.02, SUN, 1.0);
    let hole = app.selected;
    app.fly_to(SUN, 400.0);
    app.selected = hole;
    app.at_speed(MONTH);
}

fn eclipse(app: &mut WorldlineApp) {
    app.open_eclipses();
    app.go_to_total = true;
}

fn moon_impact(app: &mut WorldlineApp) {
    app.impact_lab = Some(ImpactLab::start(8000));
    app.simulation.paused = true;
}

/// From 3 pc behind and 3 pc beside the Sun, moving past it at 1,000
/// km/s: closest, 3 pc, after 2,900 years. With the planets gone, the Sun
/// moves in the hole's spacetime, as anything does that a black hole 35
/// times heavier pulls on hardest; 3 pc off, that's Newton's to 10⁻⁷.
fn black_hole_flyby(app: &mut WorldlineApp) {
    clear_solar_system(app, &["Sun"]);
    let sun = &app.simulation.bodies[SUN];
    let place = sun.position + DVec3::new(-3.0 * PARSEC, 3.0 * PARSEC, 0.0);
    let velocity = sun.velocity + DVec3::X * 1e6;
    app.drop_entry("object:Sagittarius A*", place, velocity, SUN);
    let hole = app.selected;
    app.camera.pitch = 60f64.to_radians();
    app.fly_to(SUN, 9e8);
    app.selected = hole;
    app.at_speed(CENTURY);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> WorldlineApp {
        WorldlineApp::new(&crate::StartOptions::default())
    }

    /// Plays `frames` frames of a tenth of a second each, as the app does.
    fn play(app: &mut WorldlineApp, frames: usize) {
        for _ in 0..frames {
            app.physics_frame(0.1);
        }
    }

    #[test]
    fn every_scenario_loads_and_plays() {
        let mut keys: Vec<&str> = SCENARIOS.iter().map(|s| s.key).collect();
        keys.dedup();
        assert_eq!(keys.len(), 12);
        let mut figure = worldline_report::margins(
            "scenarios-load",
            "The scenario library: every scenario loads and plays from the menu",
        );
        for scenario in &SCENARIOS {
            let mut app = app();
            app.play_scenario(scenario);
            assert_eq!(app.playing, Some(scenario.title));
            let start = app.simulation.time();
            play(&mut app, 3);
            let moved = app.simulation.time() - start;
            println!(
                "{}: {} bodies, {:.3} s of simulated time in three frames{}",
                scenario.title,
                app.simulation.bodies.len(),
                moved,
                if app.simulation.paused {
                    " (paused)"
                } else {
                    ""
                }
            );
            let plays = app.simulation.paused || moved > 0.0;
            figure = figure.check(
                scenario.title,
                if plays { 0.0 } else { 1.0 },
                0.5,
                format!(
                    "{} bodies; {}",
                    app.simulation.bodies.len(),
                    if app.simulation.paused {
                        "waits paused, as it should".to_string()
                    } else {
                        format!("{moved:.3} s of simulated time in three frames")
                    }
                ),
            );
            assert!(plays, "{}", scenario.key);
        }
        figure.save(worldline_report::here!());
    }

    #[test]
    fn each_scenario_is_set_up_as_it_says() {
        // Mercury and the Sun alone, in the solar system's region.
        let mut a = app();
        a.play_scenario(scenario("mercury").unwrap());
        let solar: Vec<&str> = (0..a.simulation.bodies.len())
            .filter(|&i| {
                a.simulation.parent(i).is_none()
                    && !a.simulation.is_galactic(i)
                    && !a.simulation.is_neighbor(i)
                    && !a.simulation.is_deep_sky(i)
            })
            .map(|i| a.simulation.bodies[i].name.as_str())
            .collect();
        assert_eq!(solar, ["Sun", "Mercury"]);
        let watch = a.watch.as_ref().expect("watching Mercury");
        let arcsec_per_century = 100.0 * JULIAN_YEAR * 180.0 / PI * 3600.0;
        assert!((watch.theory * arcsec_per_century - 42.98).abs() < 0.05);
        // The Hulse–Taylor pair, its waves shrinking its orbit as general
        // relativity says from the measured masses, −2.40263 × 10⁻¹²
        // (Weisberg & Huang 2016), to what rounding the catalog's masses to
        // 0.001 Suns allows, 8.5 × 10⁻⁴ (as in step 2.2).
        let mut a = app();
        a.play_scenario(scenario("hulse-taylor").unwrap());
        let waves = a
            .simulation
            .gravitational_waves(a.selected)
            .expect("a pair");
        println!("Hulse–Taylor: Ṗ = {:.5e}", waves.shrink);
        assert!((waves.shrink / -2.40263e-12 - 1.0).abs() < 8.5e-4);
        // It plays as a film, which opens the waves window at its end.
        assert!(a.film_playing());
        assert!(a.waves.is_none());
        // The planet's closest approach to Gaia BH1: half its tidal
        // radius, R (M/m)^(1/3).
        let mut a = app();
        a.play_scenario(scenario("planet-black-hole").unwrap());
        let planet = a.selected;
        let hole = a.simulation.index_of("Gaia BH1").expect("the copy");
        let (p, h) = (&a.simulation.bodies[planet], &a.simulation.bodies[hole]);
        let (r, v, mu) = (
            p.position - h.position,
            p.velocity - h.velocity,
            p.gm + h.gm,
        );
        let e = periapsis_vector(r, v, mu).length() / mu;
        let semi = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        let closest = semi * (1.0 - e);
        let tidal = p.radius * (h.gm / p.gm).cbrt();
        println!(
            "planet and black hole: closest {:.3} of the tidal radius",
            closest / tidal
        );
        assert!((closest / tidal - 0.5).abs() < 0.05);
        // The black hole's straight path passes 3 pc from the Sun.
        let mut a = app();
        a.play_scenario(scenario("black-hole-flyby").unwrap());
        let hole = &a.simulation.bodies[a.selected];
        let sun = &a.simulation.bodies[SUN];
        let (r, v) = (hole.position - sun.position, hole.velocity - sun.velocity);
        let miss = r.cross(v.normalize()).length() / PARSEC;
        println!("black hole flyby: passes {miss:.3} pc from the Sun");
        assert!((miss - 3.0).abs() < 1e-9);
    }

    /// Mercury and the Sun alone for a century, sampled every ten days:
    /// the perihelion turns as general relativity says. The osculating
    /// periapsis also swings within each orbit, by about GM/c²a ≈ 3 × 10⁻⁸
    /// rad; fitted over 3,650 samples it shifts the rate by under 10⁻⁴ of
    /// itself. The limit allows 10⁻³.
    #[test]
    #[ignore = "a century of the solar system: about a minute"]
    fn mercurys_perihelion_turns_as_general_relativity_says() {
        let mut a = app();
        a.play_scenario(scenario("mercury").unwrap());
        for _ in 0..3653 {
            a.simulation.advance_by(10.0 * DAY);
            a.watch.as_mut().unwrap().sample(&a.simulation);
        }
        let watch = a.watch.as_ref().unwrap();
        let (rate, span) = watch.rate().unwrap();
        let k = 100.0 * JULIAN_YEAR * 180.0 / PI * 3600.0;
        println!(
            "over {:.1} years: {:.4}″ a century, general relativity {:.4}″",
            span / JULIAN_YEAR,
            rate * k,
            watch.theory * k
        );
        worldline_report::margins(
            "scenario-mercury",
            "The Mercury scenario: the inspector's perihelion rate over a century, against general relativity's",
        )
        .check(
            "the fitted rate, against 6πGM/c²a(1 − e²) an orbit",
            rate / watch.theory - 1.0,
            1e-3,
            format!(
                "{:.4}″ a century against {:.4}″ (limit 10⁻³: the osculating swing shifts it under 10⁻⁴)",
                rate * k,
                watch.theory * k
            ),
        )
        .save(worldline_report::here!());
        assert!((rate / watch.theory - 1.0).abs() < 1e-3);
    }

    /// Each scenario that ends in an event, played until it happens. The
    /// two mergers must come when the waves window's recording says their
    /// inspiral ends: it runs the same pair ahead on its own, to the same
    /// end (where the radiation reaction stops converging, or they touch),
    /// so the two agree to within how finely the play is checked.
    #[test]
    #[ignore = "plays each to its end: a few minutes"]
    fn the_scenarios_play_out_as_they_say() {
        let outcome = |key: &str, seconds: f64, done: &dyn Fn(&WorldlineApp) -> bool| {
            let mut a = app();
            a.play_scenario(scenario(key).unwrap());
            let recorded = a.waves.as_ref().map(|w| {
                let r = &w.recording;
                r.final_hole
                    .as_ref()
                    .map_or(r.times.last().copied().unwrap_or(0.0), |h| h.start)
            });
            let start = a.simulation.time();
            let step = seconds / 400.0;
            while !done(&a) && a.simulation.time() - start < seconds {
                a.simulation.advance_by(step);
            }
            let took = a.simulation.time() - start;
            println!(
                "{key}: done after {took:.4e} s{}; {}",
                recorded.map_or(String::new(), |r| format!(
                    ", the recording's inspiral ends at {r:.4e} s"
                )),
                a.simulation.notices.join("; ")
            );
            assert!(done(&a), "{key}");
            if let Some(r) = recorded {
                assert!((took - r).abs() <= step, "{key}: {took} against {r}");
            }
            (took, recorded, step)
        };
        // Each event happens; the mergers when their recordings end.
        let mut figure = worldline_report::margins(
            "scenarios-play-out",
            "The scenarios that end in an event, played until it happens",
        );
        let mut happened = |name: &str,
                            (took, recorded, step): (f64, Option<f64>, f64),
                            unit: f64,
                            units: &str| {
            figure = match recorded {
                Some(r) => figure.clone().check(
                    &format!("{name}: merges when its recorded chirp ends"),
                    took - r,
                    step,
                    format!(
                        "after {:.2} {units}; the recording ends at {:.2} (limit: the play's step, {:.2})",
                        took / unit,
                        r / unit,
                        step / unit
                    ),
                ),
                None => figure.clone().check(
                    &format!("{name}: happens"),
                    0.0,
                    0.5,
                    format!("after {:.2} {units}", took / unit),
                ),
            };
        };
        let merged = |a: &WorldlineApp, name: &str| a.simulation.index_of(name).is_none();
        happened(
            "GW150914",
            outcome("gw150914", 12.0, &|a| merged(a, "GW150914 lighter hole")),
            1.0,
            "s",
        );
        happened(
            "GW170817",
            outcome("gw170817", 300.0, &|a| merged(a, "GW170817 lighter star")),
            1.0,
            "s",
        );
        happened(
            "A white dwarf past its limit explodes as a type Ia supernova",
            outcome("white-dwarf", 2.0 * DAY, &|a| {
                a.simulation.notices.iter().any(|n| n.contains("type Ia"))
            }),
            3600.0,
            "hours",
        );
        happened(
            "An evaporating black hole vanishes",
            outcome("mini-black-hole", 3.0 * JULIAN_YEAR, &|a| {
                merged(a, "Mini black hole")
            }),
            JULIAN_YEAR,
            "years",
        );
        happened(
            "TON 618 swallows Sagittarius A*",
            outcome("ton-618", JULIAN_YEAR, &|a| merged(a, "Sagittarius A*")),
            JULIAN_YEAR,
            "years",
        );
        // The finder's first total solar eclipse, gone to: 12 August 2026,
        // as NASA's Five Millennium Canon has it.
        let mut a = app();
        a.play_scenario(scenario("eclipse").unwrap());
        let started = std::time::Instant::now();
        while a.go_to_total && started.elapsed().as_secs() < 900 {
            std::thread::sleep(std::time::Duration::from_millis(200));
            if let Some(finder) = &mut a.finder {
                finder.poll();
            }
            if let Some(k) = a.finder.as_ref().and_then(first_total_solar) {
                a.go_to_total = false;
                a.go_to_eclipse(k);
            }
        }
        let when = to_the_minute(a.simulation.time_scales().utc(a.simulation.julian_date()));
        println!("eclipse: went to {when} UTC");
        figure
            .check(
                "The eclipse finder goes to 12 August 2026, as NASA's canon has it",
                if when.starts_with("2026-08-12") {
                    0.0
                } else {
                    1.0
                },
                0.5,
                format!("{when} UTC"),
            )
            .save(worldline_report::here!());
        assert!(when.starts_with("2026-08-12"), "{when}");
    }
}
