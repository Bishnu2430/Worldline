//! Every other scenario's film (step 8.5c), and the double pulsar's. As in
//! `scripts.rs`, a card's numbers are published (read from the data, with
//! their sources) or measured from the simulation as it plays; history is
//! cited to the paper that made it; what never happened is said to be
//! imagined. The tests below work out each figure a caption rounds from the
//! data and check the caption says it.

use super::film::{Card, Line, Look, Pinned, Script, Shot};
use super::*;
use worldline_core::orbit::periapsis_vector;
use worldline_data::{BinaryOrbit, binary_orbits, notable_object};

/// A shot's camera on the bodies `at` (none: the body followed).
const fn look(
    at: &'static [&'static str],
    distance: (f64, f64),
    yaw: (f64, f64),
    pitch: (f64, f64),
) -> Look {
    Look {
        at,
        distance,
        yaw,
        pitch,
        facing_sun: false,
    }
}

/// A card at the view's left, with no body joined to it.
const fn card(title: &'static str, lines: &'static [Line], source: &'static str) -> Card {
    Card {
        at: Pinned::Left,
        title,
        lines,
        source,
    }
}

/// A shot with no trails, orbits, start or wait: most of them.
const fn shot(
    name: &'static str,
    seconds: f64,
    speed: usize,
    look: Look,
    caption: &'static str,
    cards: &'static [Card],
) -> Shot {
    Shot {
        name,
        seconds,
        speed,
        look,
        caption,
        cards,
        trails: false,
        orbits: false,
        begin: None,
        hold: None,
    }
}

/// Nothing more to do at the end.
fn nothing(_: &mut WorldlineApp) {}

/// Two bodies by name.
fn two<'a>(app: &'a WorldlineApp, a: &str, b: &str) -> Option<(&'a Body, &'a Body)> {
    let sim = &app.simulation;
    Some((&sim.bodies[sim.index_of(a)?], &sim.bodies[sim.index_of(b)?]))
}

/// A pair's wave and speed now: the gravitational wave's frequency, twice
/// the orbit's, √(GM/r³)/π for their orbit as if it were a circle at their
/// distance, and how fast they pass each other.
fn wave_now(app: &WorldlineApp, a: &str, b: &str) -> Option<String> {
    let (p, q) = two(app, a, b)?;
    let r = (p.position - q.position).length();
    let f = ((p.gm + q.gm) / r.powi(3)).sqrt() / PI;
    let v = (p.velocity - q.velocity).length();
    Some(format!(
        "Now: {} apart, passing at {:.0}% of light's speed; their wave at {f:.0} Hz",
        length(r),
        100.0 * v / C
    ))
}

/// A published mass in Suns, with its uncertainty as published.
fn suns(name: &str) -> Option<String> {
    let mass = notable_object(name)?.mass;
    Some(match mass.plus_minus {
        Some((plus, minus)) if plus == minus => format!("{} ± {plus} Suns", mass.value),
        Some((plus, minus)) => format!("{} +{plus} −{minus} Suns", mass.value),
        None => format!("{} Suns", mass.value),
    })
}

/// A published distance, in megaparsecs or parsecs as published.
fn published_distance(name: &str) -> Option<f64> {
    match notable_object(name)?.distance? {
        worldline_data::Distance::Meters(m) => Some(m),
        worldline_data::Distance::Redshift(_) => None,
    }
}

// Mercury.

fn mercury_turned(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(format!(
        "Turned {:.1}″ since the film began, over {:.0} years",
        watch.turned().to_degrees() * 3600.0,
        watch.watched_for() / JULIAN_YEAR
    ))
}

fn mercury_theory(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(format!(
        "General relativity: {:.2}″ a century, 6πGM/c²a(1 − e²) each orbit",
        watch.theory * 100.0 * JULIAN_YEAR * 180.0 / PI * 3600.0
    ))
}

fn mercury_measured(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(match watch.rate() {
        Some((rate, span)) => format!(
            "Measured here: {:.2}″ a century, over {:.0} years",
            rate * 100.0 * JULIAN_YEAR * 180.0 / PI * 3600.0,
            span / JULIAN_YEAR
        ),
        None => "Measured here: measuring…".to_string(),
    })
}

const MERCURY_AT: &[&str] = &["Mercury", "Sun"];

pub static MERCURY: Script = Script {
    shots: &[
        Shot {
            orbits: true,
            ..shot(
                "A turning orbit",
                14.0,
                CENTURY,
                look(MERCURY_AT, (2.2e11, 1.6e11), (250.0, 270.0), (60.0, 80.0)),
                "Mercury's orbit isn't still: its closest point to the Sun creeps round. In 1859 \
                 Urbain Le Verrier found that the other planets' pulls explain all of the turning \
                 but 38 seconds of arc a century.",
                &[card(
                    "Mercury's perihelion",
                    &[
                        Line::Text(
                            "Its orbit's closest point to the Sun turns a little each orbit",
                        ),
                        Line::Live(mercury_turned),
                    ],
                    "Le Verrier 1859, Comptes Rendus 49, 379",
                )],
            )
        },
        Shot {
            orbits: true,
            ..shot(
                "Einstein's answer",
                16.0,
                CENTURY,
                look(MERCURY_AT, (1.6e11, 1.6e11), (270.0, 285.0), (80.0, 82.0)),
                "In 1915 Einstein showed that the Sun's curving of spacetime turns the orbit by \
                 just that much: 43 seconds of arc a century. Here the other planets are taken \
                 away, so what turns it is relativity alone.",
                &[card(
                    "Relativity's turning",
                    &[Line::Live(mercury_theory), Line::Live(mercury_turned)],
                    "Einstein 1915, Sitzungsberichte der Preussischen Akademie der Wissenschaften, 831",
                )],
            )
        },
        Shot {
            orbits: true,
            ..shot(
                "Measured here",
                16.0,
                CENTURY,
                look(MERCURY_AT, (1.6e11, 1.9e11), (285.0, 300.0), (82.0, 70.0)),
                "A century a second, the simulation measures the turning as it plays: the same \
                 43 seconds of arc a century, the first test general relativity passed.",
                &[card(
                    "The turning, measured",
                    &[Line::Live(mercury_measured), Line::Live(mercury_theory)],
                    "Fitted by least squares to the orbit's long axis, every frame",
                )],
            )
        },
    ],
    end: nothing,
    end_note: "the inspector keeps measuring the turning",
};

// GW150914.

const HEAVIER_HOLE: &str = "GW150914 heavier hole";
const LIGHTER_HOLE: &str = "GW150914 lighter hole";
const HOLES: &[&str] = &[HEAVIER_HOLE, LIGHTER_HOLE];

fn holes_masses(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "{} and {}",
        suns(HEAVIER_HOLE)?,
        suns(LIGHTER_HOLE)?
    ))
}

fn holes_now(app: &WorldlineApp) -> Option<String> {
    Some(
        wave_now(app, HEAVIER_HOLE, LIGHTER_HOLE)
            .unwrap_or_else(|| "Now: one black hole".to_string()),
    )
}

/// The mass radiated: the two holes' masses less the final hole's, Suns.
fn radiated_suns() -> Option<f64> {
    let mass = |name: &str| notable_object(name).map(|o| o.mass.value);
    Some(mass(HEAVIER_HOLE)? + mass(LIGHTER_HOLE)? - mass("GW150914 final hole")?)
}

fn final_hole(_: &WorldlineApp) -> Option<String> {
    let hole = notable_object("GW150914 final hole")?;
    Some(format!(
        "The black hole they made: {}{}",
        suns("GW150914 final hole")?,
        hole.spin.map_or(String::new(), |(a, _)| format!(
            ", spinning at {a} of the most a hole can"
        ))
    ))
}

fn radiated(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "Radiated as gravitational waves: {:.1} Suns' worth of energy",
        radiated_suns()?
    ))
}

pub static GW150914: Script = Script {
    shots: &[
        shot(
            "Two black holes",
            5.0,
            REAL_TIME,
            look(HOLES, (1.4e7, 1.0e7), (200.0, 230.0), (40.0, 30.0)),
            "Two black holes, 36 and 31 times the Sun's mass, circle each other a few thousand \
             kilometers apart, in a galaxy 440 megaparsecs away.",
            &[card(
                "GW150914",
                &[Line::Live(holes_masses), Line::Live(holes_now)],
                "LIGO/Virgo GWTC-1: Abbott et al. 2019, Phys. Rev. X 9, 031040",
            )],
        ),
        shot(
            "The last orbits",
            4.0,
            REAL_TIME,
            look(HOLES, (1.0e7, 4.0e6), (230.0, 260.0), (30.0, 20.0)),
            "Gravitational waves carry off their orbit's energy: they fall together faster and \
             faster, the wave's pitch rising.",
            &[card(
                "Spiraling in",
                &[Line::Live(holes_now)],
                "Measured from the simulation as it plays",
            )],
        ),
        shot(
            "One black hole",
            8.0,
            REAL_TIME,
            look(
                &[HEAVIER_HOLE, LIGHTER_HOLE, "GW150914 final hole"],
                (4.0e6, 6.0e6),
                (260.0, 290.0),
                (20.0, 25.0),
            ),
            "They merge into one black hole of 63 Suns. The missing three Suns' worth of mass \
             left as gravitational waves.",
            &[card(
                "The merger",
                &[Line::Live(final_hole), Line::Live(radiated)],
                "LIGO/Virgo GWTC-1: Abbott et al. 2019",
            )],
        ),
        shot(
            "Heard on Earth",
            10.0,
            REAL_TIME,
            look(
                &[HEAVIER_HOLE, LIGHTER_HOLE, "GW150914 final hole"],
                (6.0e6, 2.0e7),
                (290.0, 320.0),
                (25.0, 35.0),
            ),
            "On 14 September 2015 their waves reached Earth and LIGO heard them: the first \
             gravitational waves ever detected. The 2017 Nobel Prize in Physics followed.",
            &[card(
                "The first detection",
                &[
                    Line::Text("Heard by LIGO's two detectors, in Hanford and Livingston"),
                    Line::Text("The waves window plays their chirp, as the detectors recorded it"),
                ],
                "Abbott et al. 2016, Phys. Rev. Lett. 116, 061102",
            )],
        ),
    ],
    end: nothing,
    end_note: "the gravitational-waves window plays their chirp",
};

// GW170817.

const HEAVIER_STAR: &str = "GW170817 heavier star";
const LIGHTER_STAR: &str = "GW170817 lighter star";
const STARS: &[&str] = &[HEAVIER_STAR, LIGHTER_STAR];

fn stars_masses(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "{} and {}",
        suns(HEAVIER_STAR)?,
        suns(LIGHTER_STAR)?
    ))
}

fn stars_now(app: &WorldlineApp) -> Option<String> {
    Some(wave_now(app, HEAVIER_STAR, LIGHTER_STAR).unwrap_or_else(|| "Now: merged".to_string()))
}

pub static GW170817: Script = Script {
    shots: &[
        shot(
            "Two neutron stars",
            30.0,
            REAL_TIME,
            look(STARS, (2.5e6, 2.0e6), (200.0, 225.0), (40.0, 30.0)),
            "Two neutron stars, 1.46 and 1.27 times the Sun's mass, circle each other ten times \
             a second, in the galaxy NGC 4993, 40 megaparsecs away.",
            &[card(
                "GW170817",
                &[Line::Live(stars_masses), Line::Live(stars_now)],
                "LIGO/Virgo GWTC-1: Abbott et al. 2019",
            )],
        ),
        shot(
            "Three minutes",
            60.0,
            REAL_TIME,
            look(STARS, (2.0e6, 1.5e6), (225.0, 285.0), (30.0, 25.0)),
            "In real time: for nearly three minutes, gravitational waves draw them closer, and \
             the orbit quickens.",
            &[card(
                "Spiraling in",
                &[Line::Live(stars_now)],
                "Measured from the simulation as it plays",
            )],
        ),
        shot(
            "The merger",
            85.0,
            REAL_TIME,
            look(STARS, (1.5e6, 1.0e6), (285.0, 400.0), (25.0, 20.0)),
            "In the last moments they whirl round hundreds of times a second, then collide.",
            &[card(
                "The last orbits",
                &[Line::Live(stars_now)],
                "Measured from the simulation as it plays",
            )],
        ),
        shot(
            "Light follows",
            16.0,
            REAL_TIME,
            look(STARS, (1.0e6, 3.0e6), (400.0, 430.0), (20.0, 30.0)),
            "On 17 August 2017 the waves reached Earth, and 1.7 seconds later a burst of gamma \
             rays: gravity and light travel at the same speed. Telescopes then found the glow of \
             a kilonova, where heavy elements like gold were forged.",
            &[card(
                "Seen and heard",
                &[
                    Line::Text("The gamma rays, GRB 170817A, came 1.7 s after the merger"),
                    Line::Text("Gravity's speed and light's agree to a few parts in 10¹⁵"),
                ],
                "Abbott et al. 2017, Phys. Rev. Lett. 119, 161101; ApJL 848, L13",
            )],
        ),
    ],
    end: nothing,
    end_note: "the gravitational-waves window plays their chirp",
};

// A planet meets a black hole.

fn tidal(app: &WorldlineApp) -> Option<String> {
    let sim = &app.simulation;
    let hole = &sim.bodies[sim.index_of("Gaia BH1")?];
    let planet = &sim.bodies[app.focus];
    let reach = worldline_core::tides::tidal_radius(planet.radius, planet.gm, hole.gm);
    let now = (planet.position - hole.position).length();
    Some(format!(
        "Its tidal radius, R (M/m)^(1/3): {}. Now {:.1} times that from the hole",
        length(reach),
        now / reach
    ))
}

fn gaia_bh1(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "Gaia BH1: {}, the nearest known black hole, {:.0} parsecs away",
        suns("Gaia BH1")?,
        published_distance("Gaia BH1")? / PARSEC
    ))
}

pub static PLANET_BLACK_HOLE: Script = Script {
    shots: &[
        shot(
            "A planet falls",
            10.0,
            WEEK,
            look(&[], (4.2e9, 3.0e9), (200.0, 225.0), (35.0, 30.0)),
            "Imagined: a planet like Jupiter falls toward a copy of Gaia BH1, a black hole of \
             9.6 Suns and the nearest known, 480 parsecs from the Sun.",
            &[card(
                "The black hole",
                &[Line::Live(gaia_bh1), Line::Live(tidal)],
                "El-Badry et al. 2023, MNRAS 518, 1057",
            )],
        ),
        shot(
            "Torn",
            16.0,
            WEEK,
            look(&[], (3.0e9, 2.0e9), (225.0, 260.0), (30.0, 25.0)),
            "Close in, the hole pulls harder on the planet's near side than on its far side, \
             more than the planet's own gravity holds it together. Worldline turns it into a \
             fluid and lets the tides pull it apart.",
            &[card(
                "Tides",
                &[
                    Line::Live(tidal),
                    Line::Text(
                        "As a fluid of particles on the GPU (smoothed-particle hydrodynamics)",
                    ),
                ],
                "The tidal radius: Rees 1988, Nature 333, 523",
            )],
        ),
        shot(
            "Debris",
            12.0,
            WEEK,
            look(&[], (2.0e9, 4.0e9), (260.0, 290.0), (25.0, 35.0)),
            "Part of it falls back toward the hole, part is flung away: what astronomers see, \
             far brighter, when a black hole tears apart a star.",
            &[card(
                "A tidal disruption",
                &[Line::Text(
                    "Its debris falls back at a rate going as t^(−5/3), as the fluid's debris window shows",
                )],
                "Rees 1988, Nature 333, 523",
            )],
        ),
    ],
    end: nothing,
    end_note: "the debris plays on",
};

// TON 618 swallows Sagittarius A*.

const GIANTS: &[&str] = &["TON 618", "Sagittarius A*"];

fn giants(_: &WorldlineApp) -> Option<String> {
    let ton = notable_object("TON 618")?.mass.value;
    let sgr = notable_object("Sagittarius A*")?.mass.value;
    Some(format!(
        "TON 618: {:.0} billion Suns. Sagittarius A*: {:.1} million",
        ton / 1e9,
        sgr / 1e6
    ))
}

fn ton_horizon(_: &WorldlineApp) -> Option<String> {
    let ton = notable_object("TON 618")?;
    let radius = 2.0 * ton.gm() / (C * C);
    Some(format!(
        "TON 618's horizon: {:.0} AU across, wider than Neptune's orbit",
        2.0 * radius / AU
    ))
}

fn ton_distance(app: &WorldlineApp) -> Option<String> {
    let (ton, sgr) = two(app, "TON 618", "Sagittarius A*")?;
    let horizon = 2.0 * ton.gm / (C * C);
    let d = (ton.position - sgr.position).length();
    Some(format!(
        "Apart now: {:.0} AU, {:.1} times TON 618's horizon",
        d / AU,
        d / horizon
    ))
}

/// How much heavier TON 618 gets, as a share of its mass.
fn ton_grows() -> Option<f64> {
    Some(notable_object("Sagittarius A*")?.mass.value / notable_object("TON 618")?.mass.value)
}

fn ton_grew(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "Swallowing Sagittarius A*, TON 618 grows by {:.4}% of its mass",
        100.0 * ton_grows()?
    ))
}

pub static TON_618: Script = Script {
    shots: &[
        shot(
            "Giants",
            10.0,
            WEEK,
            look(GIANTS, (3.0e15, 2.4e15), (200.0, 225.0), (35.0, 28.0)),
            "Imagined: TON 618, one of the heaviest black holes known, 66 billion Suns, set \
             beside the Milky Way's own, Sagittarius A*, 4.3 million Suns. In truth TON 618 is \
             a quasar billions of light-years away.",
            &[card(
                "Two black holes",
                &[Line::Live(giants), Line::Live(ton_horizon)],
                "Shemmer et al. 2004; GRAVITY Collaboration 2022",
            )],
        ),
        shot(
            "The fall",
            15.0,
            WEEK,
            look(GIANTS, (2.4e15, 2.0e15), (225.0, 260.0), (28.0, 22.0)),
            "Set off at half the speed that would keep it circling, Sagittarius A* falls toward \
             the giant, its orbit dipping inside the horizon.",
            &[card(
                "Falling",
                &[Line::Live(ton_distance)],
                "Measured from the simulation as it plays",
            )],
        ),
        shot(
            "Swallowed",
            10.0,
            WEEK,
            look(&["TON 618"], (2.0e15, 2.6e15), (260.0, 280.0), (22.0, 30.0)),
            "Gone. A black hole of 4.3 million Suns makes TON 618 just 0.0065% heavier.",
            &[card(
                "One black hole",
                &[Line::Live(giants), Line::Live(ton_grew)],
                "Shemmer et al. 2004; GRAVITY Collaboration 2022",
            )],
        ),
    ],
    end: nothing,
    end_note: "TON 618 plays on",
};

// A white dwarf past its limit.

fn chandrasekhar(_: &WorldlineApp) -> Option<String> {
    let limit = worldline_core::white_dwarf::chandrasekhar_limit(CARBON_OXYGEN) * G / GM_SUN;
    Some(format!(
        "Chandrasekhar's limit: {limit:.3} Suns, the most a white dwarf of carbon and oxygen can weigh"
    ))
}

fn two_dwarfs(_: &WorldlineApp) -> Option<String> {
    let mass = notable_object("Sirius B")?.mass.value;
    Some(format!(
        "Each a copy of Sirius B, {mass} Suns: together {:.3}",
        2.0 * mass
    ))
}

pub static WHITE_DWARF: Script = Script {
    shots: &[
        shot(
            "Two white dwarfs",
            5.0,
            HOUR,
            look(&[], (1.7e10, 1.3e10), (200.0, 220.0), (35.0, 28.0)),
            "Imagined: two copies of Sirius B, each 1.02 times the Sun's mass in a ball about \
             the size of Earth, fall together from 7.5 million km.",
            &[card(
                "Two white dwarfs",
                &[Line::Live(two_dwarfs), Line::Live(chandrasekhar)],
                "Sirius B: Bond et al. 2017; the limit: Chandrasekhar 1931",
            )],
        ),
        shot(
            "Past the limit",
            6.0,
            HOUR,
            look(&[], (1.3e10, 8.0e9), (220.0, 250.0), (28.0, 22.0)),
            "No white dwarf can weigh more than Chandrasekhar's limit: past it, the electrons' \
             resistance to being squeezed gives way. Together these two weigh 2.04 Suns.",
            &[card(
                "Chandrasekhar's limit",
                &[Line::Live(chandrasekhar), Line::Live(two_dwarfs)],
                "Chandrasekhar 1931, ApJ 74, 81",
            )],
        ),
        shot(
            "A type Ia supernova",
            12.0,
            HOUR,
            look(&[], (8.0e9, 2.0e10), (250.0, 280.0), (22.0, 30.0)),
            "They meet and explode: a type Ia supernova, outshining its galaxy for weeks. \
             Supernovae like it, all nearly alike, showed in 1998 that the universe's expansion \
             is speeding up.",
            &[card(
                "Standard candles",
                &[Line::Text(
                    "The 2011 Nobel Prize in Physics: the accelerating universe, from such supernovae",
                )],
                "Riess et al. 1998, AJ 116, 1009; Perlmutter et al. 1999, ApJ 517, 565",
            )],
        ),
    ],
    end: nothing,
    end_note: "the scenario plays on",
};

// An evaporating black hole.

const MINI: &[&str] = &["Mini black hole"];

fn mini_glow(app: &WorldlineApp) -> Option<String> {
    let sim = &app.simulation;
    let hole = &sim.bodies[sim.index_of("Mini black hole")?];
    Some(format!(
        "Now {:.0} tonnes, glowing at {:.1e} K with {:.1e} W",
        hole.gm / G / 1e3,
        worldline_core::hawking::temperature(hole.gm),
        worldline_core::hawking::power(hole.gm)
    ))
}

fn mini_left(app: &WorldlineApp) -> Option<String> {
    let sim = &app.simulation;
    Some(match sim.index_of("Mini black hole") {
        Some(i) => format!(
            "Left to live: {:.2} years",
            worldline_core::hawking::lifetime(sim.bodies[i].gm) / JULIAN_YEAR
        ),
        None => "Evaporated".to_string(),
    })
}

pub static MINI_BLACK_HOLE: Script = Script {
    shots: &[
        shot(
            "A tiny black hole",
            10.0,
            MONTH,
            look(MINI, (6.0e10, 4.0e10), (200.0, 220.0), (30.0, 25.0)),
            "Imagined: a black hole of 100,000 tonnes, the mass of a large ship, circling the \
             Sun just outside Earth's orbit. Its horizon is thousands of times smaller than a \
             proton.",
            &[card(
                "A hypothetical black hole",
                &[Line::Live(mini_glow), Line::Live(mini_left)],
                "Hawking 1974, Nature 248, 30",
            )],
        ),
        shot(
            "It glows",
            12.0,
            MONTH,
            look(MINI, (4.0e10, 3.0e10), (220.0, 250.0), (25.0, 22.0)),
            "In 1974 Stephen Hawking showed that quantum effects make a black hole glow, hotter \
             the smaller it is. Glowing, it loses mass, and grows hotter still.",
            &[card(
                "Hawking radiation",
                &[Line::Live(mini_glow), Line::Live(mini_left)],
                "Hawking 1974, Nature 248, 30",
            )],
        ),
        shot(
            "Gone",
            14.0,
            MONTH,
            look(MINI, (3.0e10, 5.0e10), (250.0, 280.0), (22.0, 30.0)),
            "Its last moments are a blaze of radiation. No such black hole has ever been seen: \
             Hawking radiation is still unobserved.",
            &[card(
                "The end",
                &[Line::Live(mini_left)],
                "Hawking 1974, Nature 248, 30",
            )],
        ),
    ],
    end: nothing,
    end_note: "the solar system plays on",
};

// The next total solar eclipse.

/// Whether the finder has found the eclipse and gone to it.
fn eclipse_reached(app: &WorldlineApp) -> bool {
    !app.go_to_total
}

/// How far ahead the finder has searched.
fn eclipse_search(app: &WorldlineApp) -> Option<String> {
    let finder = app.finder.as_ref()?;
    if !app.go_to_total {
        return Some("Found".to_string());
    }
    let date = to_the_minute(app.simulation.time_scales().utc(finder.searched));
    Some(format!(
        "Searched ahead to {}",
        date.split(' ').next().unwrap_or(&date)
    ))
}

const EARTH_ONLY: &[&str] = &["Earth"];

/// Facing Earth from the Sun's side.
const fn sunward(distance: (f64, f64), yaw: (f64, f64), pitch: (f64, f64)) -> Look {
    Look {
        at: EARTH_ONLY,
        distance,
        yaw,
        pitch,
        facing_sun: true,
    }
}

pub static ECLIPSE: Script = Script {
    shots: &[
        Shot {
            hold: Some(eclipse_reached),
            ..shot(
                "Searching ahead",
                8.0,
                REAL_TIME,
                sunward((6.0e7, 4.0e7), (-20.0, -5.0), (20.0, 10.0)),
                "The eclipse finder runs the Sun, Earth and Moon ahead, looking for the Moon's \
                 shadow falling on Earth.",
                &[card(
                    "Searching",
                    &[
                        Line::Text(
                            "Ahead in time, on the same physics, until the shadow touches Earth; \
                             this shot waits until it has",
                        ),
                        Line::Live(eclipse_search),
                    ],
                    "",
                )],
            )
        },
        shot(
            "12 August 2026",
            12.0,
            REAL_TIME,
            sunward((4.0e7, 2.6e7), (-5.0, 0.0), (10.0, 8.0)),
            "17:46 UTC, 12 August 2026: the Moon's shadow falls on Earth near Greenland, crosses \
             Iceland and ends in Spain at sunset.",
            &[card(
                "A total eclipse",
                &[Line::Text(
                    "Inside the shadow, the Sun is completely covered by the Moon",
                )],
                "Espenak & Meeus 2006, NASA's Five Millennium Canon of Solar Eclipses",
            )],
        ),
        shot(
            "The path",
            10.0,
            HOUR,
            sunward((2.6e7, 3.0e7), (0.0, 5.0), (8.0, 12.0)),
            "An hour a second, the shadow sweeps across the Atlantic in a few seconds.",
            &[card(
                "The shadow's path",
                &[Line::Text(
                    "Its path is drawn as the eclipse finder computed it",
                )],
                "Espenak & Meeus 2006",
            )],
        ),
    ],
    end: nothing,
    end_note: "the eclipse finder's window lists the eclipses ahead",
};

// The Moon-forming impact.

/// The main view runs while the impact plays in its own window.
fn run_time(app: &mut WorldlineApp) {
    app.simulation.paused = false;
}

pub static MOON_IMPACT: Script = Script {
    shots: &[
        Shot {
            begin: Some(run_time),
            ..shot(
                "4.5 billion years ago",
                12.0,
                REAL_TIME,
                look(EARTH_ONLY, (6.0e7, 4.5e7), (200.0, 225.0), (25.0, 20.0)),
                "The young Earth, 4.5 billion years ago. A planet the size of Mars is about to \
                 strike it: the impact plays in its own window.",
                &[card(
                    "The giant impact",
                    &[Line::Text(
                        "In its window: Canup's (2004) impact, as 8,000 particles of rock and iron",
                    )],
                    "Canup 2004, Icarus 168, 433",
                )],
            )
        },
        shot(
            "The impact",
            14.0,
            REAL_TIME,
            look(EARTH_ONLY, (4.5e7, 3.5e7), (225.0, 255.0), (20.0, 15.0)),
            "It hits at a glancing angle. Rock and iron splash out; much falls back, but some is \
             left circling Earth.",
            &[card(
                "A glancing blow",
                &[Line::Text(
                    "The impactor's iron core sinks into Earth's; mostly rock is thrown out",
                )],
                "Canup 2004, Icarus 168, 433",
            )],
        ),
        shot(
            "A Moon from the debris",
            12.0,
            REAL_TIME,
            look(EARTH_ONLY, (3.5e7, 5.0e7), (255.0, 280.0), (15.0, 22.0)),
            "From that disk of debris the Moon gathers: the idea that best explains why the \
             Moon has so little iron.",
            &[card(
                "The giant-impact idea",
                &[Line::Text(
                    "Proposed by Hartmann & Davis (1975) and Cameron & Ward (1976)",
                )],
                "Hartmann & Davis 1975, Icarus 24, 504; Cameron & Ward 1976",
            )],
        ),
    ],
    end: nothing,
    end_note: "the impact's window plays on",
};

// A black hole through the Sun's neighbourhood.

fn visitor(app: &WorldlineApp) -> Option<String> {
    let sim = &app.simulation;
    let hole = &sim.bodies[app.selected];
    let sun = &sim.bodies[SUN];
    let d = (hole.position - sun.position).length();
    let v = (hole.velocity - sun.velocity).length();
    Some(format!(
        "{}: {:.2} parsecs from the Sun, passing at {:.0} km/s",
        hole.name,
        d / PARSEC,
        v / 1e3
    ))
}

const SUN_ONLY: &[&str] = &["Sun"];

pub static BLACK_HOLE_FLYBY: Script = Script {
    shots: &[
        shot(
            "A visitor",
            12.0,
            CENTURY,
            look(SUN_ONLY, (6.3e17, 5.0e17), (200.0, 220.0), (60.0, 55.0)),
            "Imagined: a black hole of 4.3 million Suns, a copy of Sagittarius A*, crosses the \
             Sun's neighbourhood at 1,000 km/s, passing 3 parsecs from the Sun.",
            &[card(
                "A visitor",
                &[Line::Live(visitor)],
                "Its mass: GRAVITY Collaboration 2022",
            )],
        ),
        shot(
            "Closest",
            16.0,
            CENTURY,
            look(SUN_ONLY, (5.0e17, 4.5e17), (220.0, 250.0), (55.0, 50.0)),
            "A century a second: it takes about 3,000 years to pass, pulling on the Sun and \
             the nearby stars as it goes.",
            &[card(
                "Passing",
                &[Line::Live(visitor)],
                "Measured from the simulation as it plays",
            )],
        ),
        shot(
            "Scattered",
            14.0,
            CENTURY,
            look(SUN_ONLY, (4.5e17, 7.0e17), (250.0, 280.0), (50.0, 60.0)),
            "The nearby stars nearest its path are flung onto new orbits, as the impulse it \
             gives each predicts.",
            &[card(
                "Scattered",
                &[Line::Live(visitor)],
                "Measured from the simulation as it plays",
            )],
        ),
    ],
    end: nothing,
    end_note: "the neighbourhood plays on",
};

// The double pulsar.

const PULSAR_A: &str = "PSR J0737-3039A";
const PULSAR_B: &str = "PSR J0737-3039B";
const DOUBLE: &[&str] = &[PULSAR_A, PULSAR_B];

/// The double pulsar's measured orbit (Kramer et al. 2021).
fn double() -> BinaryOrbit {
    binary_orbits()
        .into_iter()
        .find(|o| o.system == "PSR J0737-3039")
        .expect("the orbit is in the catalog")
}

fn a_mass(_: &WorldlineApp) -> Option<String> {
    suns(PULSAR_A)
}

fn b_mass(_: &WorldlineApp) -> Option<String> {
    suns(PULSAR_B)
}

fn a_spin(_: &WorldlineApp) -> Option<String> {
    let f = double().spin_frequency?;
    Some(format!(
        "Turns {f:.2} times a second: a pulse every {:.2} ms",
        1e3 / f
    ))
}

fn double_where(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "In the constellation Puppis, {:.0} parsecs away",
        published_distance(PULSAR_A)? / PARSEC
    ))
}

fn double_orbit(app: &WorldlineApp) -> Option<String> {
    let (p, q) = two(app, PULSAR_A, PULSAR_B)?;
    let (r, v, mu) = (
        p.position - q.position,
        p.velocity - q.velocity,
        p.gm + q.gm,
    );
    let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
    let e = periapsis_vector(r, v, mu).length() / mu;
    Some(format!(
        "Apart now: {}, between {} and {}; passing at {:.0} km/s",
        length(r.length()),
        length(a * (1.0 - e)),
        length(a * (1.0 + e)),
        v.length() / 1e3
    ))
}

fn double_turned(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(format!(
        "Turned {:.2}° since this shot began",
        watch.turned().to_degrees()
    ))
}

fn double_turning(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(match watch.rate() {
        Some((rate, span)) => format!(
            "Measured here: {:.2}° a year, over {:.2} years",
            rate.to_degrees() * JULIAN_YEAR,
            span / JULIAN_YEAR
        ),
        None => "Measured here: measuring…".to_string(),
    })
}

fn double_timed(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "Measured by timing pulsar A: {}° a year",
        double().periastron_advance
    ))
}

fn double_shrink(_: &WorldlineApp) -> Option<String> {
    let orbit = double();
    let (measured, sigma) = orbit.period_derivative;
    let (predicted, _) = orbit.period_derivative_gr;
    Some(format!(
        "Measured: {:.6} ± {:.6} × 10⁻¹² s each second; general relativity, at the same order: \
         {:.6} × 10⁻¹²",
        -measured * 1e12,
        sigma * 1e12,
        -predicted * 1e12
    ))
}

fn double_merging(app: &WorldlineApp) -> Option<String> {
    let index = app.simulation.index_of(PULSAR_A)?;
    let waves = app.simulation.gravitational_waves(index)?;
    Some(format!(
        "They'll collide in {:.0} million years",
        waves.merging / (1e6 * JULIAN_YEAR)
    ))
}

fn double_watch(app: &mut WorldlineApp) {
    app.watch = PerihelionWatch::start(&app.simulation, PULSAR_A, PULSAR_B);
}

fn double_waves(app: &mut WorldlineApp) {
    if let Some(index) = app.simulation.index_of(PULSAR_A) {
        app.selected = index;
        app.open_waves();
    }
}

pub static DOUBLE_PULSAR: Script = Script {
    shots: &[
        Shot {
            trails: true,
            ..shot(
                "Found",
                12.0,
                HOUR,
                look(DOUBLE, (5.0e9, 3.5e9), (200.0, 230.0), (32.0, 24.0)),
                "2003, the Parkes telescope in Australia: a pulsar turning 44 times a second \
                 circles a companion every 2 hours 27 minutes. A year later the companion was \
                 found to be a pulsar too: the first double pulsar.",
                &[card(
                    "PSR J0737−3039A and B",
                    &[Line::Live(double_where)],
                    "Burgay et al. 2003, Nature 426, 531; Lyne et al. 2004, Science 303, 1153",
                )],
            )
        },
        Shot {
            trails: true,
            ..shot(
                "Two pulsars",
                13.0,
                HOUR,
                look(DOUBLE, (3.5e9, 2.6e9), (230.0, 262.0), (24.0, 4.0)),
                "Pulsar A turns 44 times a second, B once every 2.8 seconds. Seen from Earth the \
                 orbit is almost exactly edge-on, so A's beam passes behind B every orbit and is \
                 eclipsed by B's magnetic field.",
                &[
                    Card {
                        at: Pinned::RightOf(PULSAR_A),
                        title: "Pulsar A",
                        lines: &[Line::Live(a_mass), Line::Live(a_spin)],
                        source: "Kramer et al. 2021, Phys. Rev. X 11, 041050, Table IV",
                    },
                    Card {
                        at: Pinned::LeftOf(PULSAR_B),
                        title: "Pulsar B",
                        lines: &[Line::Live(b_mass), Line::Text("Turns once every 2.8 s")],
                        source: "Kramer et al. 2021",
                    },
                ],
            )
        },
        Shot {
            orbits: true,
            begin: Some(double_watch),
            ..shot(
                "Relativity turns it",
                24.0,
                MONTH,
                look(DOUBLE, (3.0e9, 3.0e9), (262.0, 272.0), (62.0, 64.0)),
                "A month a second: the orbit's closest point turns 16.9° a year, four times as \
                 fast as the Hulse–Taylor pulsar's.",
                &[card(
                    "The closest point turns",
                    &[
                        Line::Live(double_turned),
                        Line::Live(double_turning),
                        Line::Live(double_timed),
                        Line::Live(double_orbit),
                    ],
                    "Kramer et al. 2021, Table IV",
                )],
            )
        },
        Shot {
            orbits: true,
            ..shot(
                "Waves shrink it",
                18.0,
                MONTH,
                look(DOUBLE, (3.0e9, 3.6e9), (272.0, 300.0), (64.0, 45.0)),
                "The orbit shrinks by 7 millimetres a day and its period by 39 millionths of a \
                 second a year: the two will collide in 86 million years.",
                &[card(
                    "The orbit shrinks",
                    &[Line::Live(double_shrink), Line::Live(double_merging)],
                    "Kramer et al. 2021, eqs. 44 and 45; the formula: Peters & Mathews 1963",
                )],
            )
        },
        Shot {
            orbits: true,
            ..shot(
                "The best test",
                16.0,
                MONTH,
                look(DOUBLE, (3.6e9, 5.0e9), (300.0, 330.0), (45.0, 35.0)),
                "Timing pulsar A for 16 years, Kramer and colleagues measured that shrinking to \
                 6 parts in 100,000: general relativity passes again, its most precise test of \
                 gravitational waves.",
                &[card(
                    "The test",
                    &[
                        Line::Live(double_shrink),
                        Line::Text(
                            "With the next order of radiation too, measured ÷ predicted: 0.999963 ± 0.000063",
                        ),
                    ],
                    "Kramer et al. 2021, eq. 48",
                )],
            )
        },
    ],
    end: double_waves,
    end_note: "it opened the gravitational-waves window",
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether every line of a shot's cards says something.
    fn cards_speak(app: &WorldlineApp, shot: &Shot) -> Result<(), String> {
        for card in shot.cards {
            for line in card.lines {
                let says = match line {
                    Line::Text(text) => !text.is_empty(),
                    Line::Live(live) => live(app).is_some_and(|s| !s.is_empty()),
                    Line::Plot(plot) => plot(app).is_some_and(|p| p.points.len() > 1),
                };
                if !says {
                    return Err(format!("{}: {}", shot.name, card.title));
                }
            }
        }
        Ok(())
    }

    #[test]
    fn every_scenario_is_a_film_and_each_shot_sets_itself_up() {
        // Every scenario plays as a film. Gone to, each shot sets its speed
        // and its view, and every line of its cards says something (a
        // shot's cards are read as it begins, before any event it shows).
        let mut app = WorldlineApp::new(&crate::StartOptions::default());
        for scenario in &scenarios::SCENARIOS {
            let script = scenario
                .film
                .unwrap_or_else(|| panic!("{}: no film", scenario.key));
            app.play_scenario(scenario);
            assert!(app.film_playing(), "{}", scenario.key);
            for (k, shot) in script.shots.iter().enumerate() {
                app.film_go_to(k);
                app.physics_frame(0.1);
                assert_eq!(
                    app.speed_index, shot.speed,
                    "{}: {}",
                    scenario.key, shot.name
                );
                assert_eq!(
                    app.options.trails, shot.trails,
                    "{}: {}",
                    scenario.key, shot.name
                );
                assert!(!app.options.grid, "{}: the grid hides", scenario.key);
                cards_speak(&app, shot).unwrap_or_else(|e| panic!("{}: {e}", scenario.key));
            }
            println!(
                "{}: {} shots, {:.0} s",
                scenario.key,
                script.shots.len(),
                script.lengths().iter().sum::<f64>()
            );
        }
    }

    #[test]
    fn a_shot_waiting_for_the_eclipse_holds_until_it_is_found() {
        // The eclipse's first shot holds just short of its end while the
        // finder searches, however long; once the eclipse is found and gone
        // to, the film moves on.
        let mut app = WorldlineApp::new(&crate::StartOptions::default());
        let eclipse = scenarios::scenario("eclipse").expect("listed");
        app.play_scenario(eclipse);
        assert!(app.go_to_total);
        let length = ECLIPSE.shots[0].seconds;
        for _ in 0..30 {
            app.physics_frame(1.0);
        }
        let reel = app.film.as_ref().expect("the film").reel;
        assert_eq!(reel.shot, 0);
        assert!(
            reel.into < length && reel.into > length - 0.01,
            "{}",
            reel.into
        );
        // Found (as the finder's window marks it): on to the next shot.
        app.go_to_total = false;
        app.physics_frame(0.1);
        assert_eq!(app.film.as_ref().expect("the film").reel.shot, 1);
    }

    /// Each film played from start to end, half a second a frame, as the
    /// app plays it; the eclipse finder polled as its window polls it. Each
    /// ends, and every card has spoken.
    #[test]
    #[ignore = "plays every film: a few minutes"]
    fn every_film_plays_to_its_end() {
        let mut app = WorldlineApp::new(&crate::StartOptions::default());
        for scenario in &scenarios::SCENARIOS {
            let script = scenario.film.expect("a film");
            app.play_scenario(scenario);
            let total: f64 = script.lengths().iter().sum();
            let began = std::time::Instant::now();
            let mut spoke = vec![false; script.shots.len()];
            while app.film_playing() {
                app.physics_frame(0.5);
                // Waiting for the eclipse finder, leave it the processor, as
                // the scenarios' own test does: it runs the solar system 1.6
                // years ahead, half a minute alone, longer with the other
                // slow tests running beside it.
                if app.go_to_total {
                    std::thread::sleep(std::time::Duration::from_millis(200));
                }
                if app.go_to_total
                    && let Some(finder) = &mut app.finder
                {
                    finder.poll();
                    if let Some(k) = app.finder.as_ref().and_then(first_total_solar) {
                        app.go_to_total = false;
                        app.go_to_eclipse(k);
                    }
                }
                let Some(reel) = app.film.as_ref().map(|f| f.reel) else {
                    break;
                };
                if !reel.ended && !spoke[reel.shot] && reel.into > 1.0 {
                    spoke[reel.shot] = cards_speak(&app, &script.shots[reel.shot]).is_ok();
                }
                assert!(
                    began.elapsed().as_secs_f64() < 900.0,
                    "{}: it never ends",
                    scenario.key
                );
            }
            println!(
                "{}: ended after {:.0} s of film ({total:.0} s written), in {:.1} s",
                scenario.key,
                app.film
                    .as_ref()
                    .map_or(0.0, |f| f.reel.elapsed(&script.lengths())),
                began.elapsed().as_secs_f64()
            );
            assert!(
                app.film.as_ref().is_some_and(|f| f.reel.ended),
                "{}",
                scenario.key
            );
            assert!(spoke.iter().all(|&s| s), "{}: {spoke:?}", scenario.key);
        }
    }

    /// `x` to `figures` significant figures.
    fn rounded(x: f64, figures: i32) -> f64 {
        let scale = 10f64.powi(figures - 1 - x.abs().log10().floor() as i32);
        (x * scale).round() / scale
    }

    fn caption(script: &Script, name: &str) -> &'static str {
        script
            .shots
            .iter()
            .find(|s| s.name == name)
            .expect("a shot")
            .caption
    }

    fn mass(name: &str) -> f64 {
        notable_object(name).expect("in the catalog").mass.value
    }

    #[test]
    fn the_captions_figures_follow_from_the_data() {
        // GW150914.
        assert_eq!(mass(HEAVIER_HOLE).round(), 36.0);
        assert_eq!(mass(LIGHTER_HOLE).round(), 31.0);
        let first = caption(&GW150914, "Two black holes");
        assert!(first.contains("36 and 31 times"));
        let parsecs = published_distance(HEAVIER_HOLE).expect("published") / PARSEC;
        assert_eq!((parsecs / 1e6).round(), 440.0);
        assert!(first.contains("440 megaparsecs"));
        assert_eq!(mass("GW150914 final hole").round(), 63.0);
        let merged = caption(&GW150914, "One black hole");
        assert!(merged.contains("63 Suns"));
        assert_eq!(radiated_suns().expect("published").round(), 3.0);
        assert!(merged.contains("three Suns"));
        // GW170817.
        let stars = caption(&GW170817, "Two neutron stars");
        assert!(stars.contains(&format!(
            "{} and {} times",
            mass(HEAVIER_STAR),
            mass(LIGHTER_STAR)
        )));
        assert_eq!(catalogue::INSPIRAL_START / 2.0, 10.0);
        assert!(stars.contains("ten times a second"));
        let parsecs = published_distance(HEAVIER_STAR).expect("published") / PARSEC;
        assert_eq!((parsecs / 1e6).round(), 40.0);
        assert!(stars.contains("40 megaparsecs"));
        // Gaia BH1.
        assert_eq!(format!("{:.1}", mass("Gaia BH1")), "9.6");
        let planet = caption(&PLANET_BLACK_HOLE, "A planet falls");
        assert!(planet.contains("9.6 Suns"));
        let parsecs = published_distance("Gaia BH1").expect("published") / PARSEC;
        assert_eq!(parsecs.round(), 480.0);
        assert!(planet.contains("480 parsecs"));
        // TON 618.
        assert_eq!((mass("TON 618") / 1e9).round(), 66.0);
        assert_eq!(format!("{:.1}", mass("Sagittarius A*") / 1e6), "4.3");
        assert!(caption(&TON_618, "Giants").contains("66 billion Suns"));
        assert!(caption(&TON_618, "Giants").contains("4.3 million Suns"));
        let grows = ton_grows().expect("published") * 100.0;
        assert_eq!(rounded(grows, 2), 0.0065);
        assert!(caption(&TON_618, "Swallowed").contains("0.0065% heavier"));
        // The white dwarfs: the scenario sets them 0.05 AU apart.
        assert_eq!(format!("{:.2}", mass("Sirius B")), "1.02");
        assert_eq!(rounded(0.05 * AU / 1e9, 2), 7.5);
        assert!(caption(&WHITE_DWARF, "Two white dwarfs").contains("7.5 million km"));
        assert_eq!(format!("{:.2}", 2.0 * mass("Sirius B")), "2.04");
        assert!(caption(&WHITE_DWARF, "Past the limit").contains("2.04 Suns"));
        // The mini black hole: the catalogue's 10⁸ kg.
        let mini = catalogue::catalogue()
            .iter()
            .find(|e| e.label == "Mini black hole")
            .expect("listed");
        let gm = mini
            .bodies(&crate::simulation::Simulation::solar_system(1.0))
            .first()
            .expect("a hole")
            .gm;
        assert_eq!((gm / G / 1e3).round(), 100_000.0);
        assert!(caption(&MINI_BLACK_HOLE, "A tiny black hole").contains("100,000 tonnes"));
        // The double pulsar.
        let orbit = double();
        let minutes = orbit.period / 60.0;
        assert_eq!(
            ((minutes / 60.0).floor(), (minutes % 60.0).round()),
            (2.0, 27.0)
        );
        assert!(caption(&DOUBLE_PULSAR, "Found").contains("every 2 hours 27 minutes"));
        assert_eq!(orbit.spin_frequency.expect("published").round(), 44.0);
        assert!(caption(&DOUBLE_PULSAR, "Found").contains("44 times a second"));
        assert_eq!(format!("{:.1}", orbit.periastron_advance), "16.9");
        let hulse_taylor = binary_orbits()
            .into_iter()
            .find(|o| o.system == "PSR B1913+16")
            .expect("listed");
        assert_eq!(
            (orbit.periastron_advance / hulse_taylor.periastron_advance).round(),
            4.0
        );
        assert!(caption(&DOUBLE_PULSAR, "Relativity turns it").contains("four times as fast"));
        // The separation shrinks at (2/3)(a/P)(dP/dt), a from Kepler's law;
        // the period by dP/dt a year.
        let gm = notable_object(PULSAR_A).expect("listed").gm()
            + notable_object(PULSAR_B).expect("listed").gm();
        let a = (gm * (orbit.period / std::f64::consts::TAU).powi(2)).cbrt();
        let (shrink, _) = orbit.period_derivative;
        let a_day = -(2.0 / 3.0) * a / orbit.period * shrink * DAY * 1e3;
        let a_year = -shrink * JULIAN_YEAR * 1e6;
        let merges = worldline_core::gravitational_waves::merger_time(
            notable_object(PULSAR_A).expect("listed").gm(),
            notable_object(PULSAR_B).expect("listed").gm(),
            orbit.period,
            orbit.eccentricity,
        ) / JULIAN_YEAR
            / 1e6;
        println!(
            "double pulsar: {a_day:.2} mm a day, {a_year:.2} µs a year, merges in {merges:.1} Myr"
        );
        assert_eq!(a_day.round(), 7.0);
        assert_eq!(a_year.round(), 39.0);
        assert_eq!(merges.round(), 86.0);
        let shrinking = caption(&DOUBLE_PULSAR, "Waves shrink it");
        assert!(shrinking.contains("7 millimetres a day"));
        assert!(shrinking.contains("39 millionths"));
        assert!(shrinking.contains("86 million years"));
        let (_, sigma) = orbit.period_derivative;
        assert_eq!(rounded((sigma / shrink).abs(), 1), 6e-5);
        assert!(caption(&DOUBLE_PULSAR, "The best test").contains("6 parts in 100,000"));
    }
}
