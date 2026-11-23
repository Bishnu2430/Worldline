//! The films' scripts (step 8.5a: the Hulse–Taylor pulsar). What a card
//! shows is published, read from the data's CSV files and cited, or
//! measured from the simulation as it plays. The captions round some of
//! those figures, and the tests below check each one against the data.

use super::film::{Card, Line, Look, Pinned, Plot, Script, Shot};
use super::*;
use worldline_core::orbit::periapsis_vector;
use worldline_data::{BinaryOrbit, NotableObject, binary_orbits, notable_object};

const PULSAR: &str = "PSR B1913+16";
const COMPANION: &str = "PSR B1913+16 companion";
const PAIR: &[&str] = &[PULSAR, COMPANION];

/// The pair's measured orbit (Weisberg & Huang 2016).
fn timed() -> BinaryOrbit {
    binary_orbits()
        .into_iter()
        .find(|o| o.system == PULSAR)
        .expect("the orbit is in the catalog")
}

fn object(name: &str) -> NotableObject {
    notable_object(name).expect("in the catalog")
}

/// How the period shrinks, s each second: as measured, as general
/// relativity predicts it, and the ratio of the two with its uncertainty.
fn shrinking() -> (f64, f64, (f64, f64)) {
    let orbit = timed();
    let (measured, sigma) = orbit.period_derivative;
    let predicted = orbit.period_derivative_gr.0;
    (
        measured,
        predicted,
        (measured / predicted, (sigma / predicted).abs()),
    )
}

/// How much earlier than an orbit of fixed size a pair's closest approach
/// comes after `t` s: ½ (Ṗ/P) t². The N-th approach of an orbit whose
/// period is P + Ṗt comes at N P + ½ Ṗ P N², and N P is t.
fn earlier(orbit: &BinaryOrbit, t: f64) -> f64 {
    -0.5 * orbit.period_derivative.0 / orbit.period * t * t
}

/// A distance in km, its thousands set apart.
fn km(meters: f64) -> String {
    format!("{} km", grouped((meters / 1e3).round() as usize))
}

/// The pair in the simulation: the pulsar and its companion.
fn pair(app: &WorldlineApp) -> Option<(&Body, &Body)> {
    let sim = &app.simulation;
    let (i, j) = (sim.index_of(PULSAR)?, sim.index_of(COMPANION)?);
    Some((&sim.bodies[i], &sim.bodies[j]))
}

/// A published mass, as many decimals as its uncertainty's.
fn mass(name: &str) -> Option<String> {
    let mass = object(name).mass;
    let (plus, minus) = mass.plus_minus?;
    let decimals = (-plus.log10()).ceil().max(0.0) as usize;
    (plus == minus).then(|| format!("{:.decimals$} ± {plus} times the Sun's mass", mass.value))
}

fn pulsar_mass(_: &WorldlineApp) -> Option<String> {
    mass(PULSAR)
}

fn companion_mass(_: &WorldlineApp) -> Option<String> {
    mass(COMPANION)
}

fn spin(_: &WorldlineApp) -> Option<String> {
    let f = timed().spin_frequency?;
    Some(format!(
        "Turns {f:.2} times a second: a pulse every {:.2} ms",
        1e3 / f
    ))
}

fn size(_: &WorldlineApp) -> Option<String> {
    let pulsar = object(PULSAR);
    (pulsar.radius_basis == RadiusBasis::Assumed).then(|| {
        format!(
            "About {:.0} km across. Its radius, {:.1} km, is assumed: GW170817's, as none has \
             been measured for it",
            2.0 * pulsar.radius.value / 1e3,
            pulsar.radius.value / 1e3
        )
    })
}

fn period(_: &WorldlineApp) -> Option<String> {
    let orbit = timed();
    Some(format!(
        "Once round every {} days: {:.4} hours",
        orbit.period / DAY,
        orbit.period / 3600.0
    ))
}

fn eccentricity(_: &WorldlineApp) -> Option<String> {
    // Published to seven decimals.
    Some(format!(
        "Eccentricity {:.7}, where a circle's is 0",
        timed().eccentricity
    ))
}

fn apart(app: &WorldlineApp) -> Option<String> {
    let (p, q) = pair(app)?;
    let (r, v, mu) = (
        p.position - q.position,
        p.velocity - q.velocity,
        p.gm + q.gm,
    );
    let a = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
    let e = periapsis_vector(r, v, mu).length() / mu;
    Some(format!(
        "Apart: {}, between {} and {}",
        km(r.length()),
        km(a * (1.0 - e)),
        km(a * (1.0 + e))
    ))
}

fn speed(app: &WorldlineApp) -> Option<String> {
    let (p, q) = pair(app)?;
    let v = (p.velocity - q.velocity).length();
    Some(format!(
        "Passing each other at {:.0} km/s, {:.2}% of light's speed",
        v / 1e3,
        100.0 * v / C
    ))
}

/// Degrees a year, from radians a second.
fn degrees_a_year(rate: f64) -> f64 {
    rate.to_degrees() * JULIAN_YEAR
}

fn turned(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(format!(
        "Turned {:.2}° since this shot began",
        watch.turned().to_degrees()
    ))
}

fn turning_here(app: &WorldlineApp) -> Option<String> {
    let watch = app.watch.as_ref()?;
    Some(match watch.rate() {
        Some((rate, span)) => format!(
            "Measured here: {:.4}° a year, over {:.2} years",
            degrees_a_year(rate),
            span / JULIAN_YEAR
        ),
        None => "Measured here: measuring…".to_string(),
    })
}

/// General relativity's turning for the catalog's masses and the timed
/// orbit, rad/s: 3 (GM/c³)^(2/3) (P/2π)^(−5/3) / (1 − e²) (Robertson 1938;
/// Damour & Deruelle 1986), as step 2.2 checks it.
fn turning_formula() -> f64 {
    let orbit = timed();
    let gm = object(PULSAR).gm() + object(COMPANION).gm();
    let e = orbit.eccentricity;
    3.0 * (gm / C.powi(3)).powf(2.0 / 3.0) * (orbit.period / std::f64::consts::TAU).powf(-5.0 / 3.0)
        / (1.0 - e * e)
}

fn turning_theory(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "General relativity, for these masses and the timed orbit: {:.4}° a year",
        degrees_a_year(turning_formula())
    ))
}

fn turning_timed(_: &WorldlineApp) -> Option<String> {
    Some(format!(
        "Measured by timing the pulses: {}° a year",
        timed().periastron_advance
    ))
}

fn shrink_here(app: &WorldlineApp) -> Option<String> {
    let index = app.simulation.index_of(PULSAR)?;
    let waves = app.simulation.gravitational_waves(index)?;
    Some(format!(
        "General relativity, for the orbit here: the period shortens by {:.4} × 10⁻¹² s every second",
        -waves.shrink * 1e12
    ))
}

fn shrink_timed(_: &WorldlineApp) -> Option<String> {
    let (measured, sigma) = timed().period_derivative;
    Some(format!(
        "Measured: {:.3} ± {:.3} × 10⁻¹², with the galaxy's own pull on the pair taken out",
        -measured * 1e12,
        sigma * 1e12
    ))
}

fn agreement(_: &WorldlineApp) -> Option<String> {
    let (_, _, (ratio, sigma)) = shrinking();
    Some(format!("Measured ÷ predicted: {ratio:.3} ± {sigma:.3}"))
}

fn merging(app: &WorldlineApp) -> Option<String> {
    let index = app.simulation.index_of(PULSAR)?;
    let waves = app.simulation.gravitational_waves(index)?;
    Some(format!(
        "They'll collide in {:.0} million years",
        waves.merging / (1e6 * JULIAN_YEAR)
    ))
}

/// How much earlier the closest approach comes, over 40 years of timing.
fn earlier_plot(_: &WorldlineApp) -> Option<Plot> {
    let orbit = timed();
    let years = 40.0;
    let points = (0..=80)
        .map(|k| {
            let t = years * f64::from(k) / 80.0;
            (t, earlier(&orbit, t * JULIAN_YEAR))
        })
        .collect();
    let thirty = earlier(&orbit, 30.0 * JULIAN_YEAR);
    Some(Plot {
        points,
        x: (years, "years of timing"),
        y: (80.0, "s earlier"),
        mark: Some((30.0, thirty, format!("{thirty:.1} s after 30 years"))),
    })
}

/// Where the camera looks from, at the pair.
const fn at_pair(distance: (f64, f64), yaw: (f64, f64), pitch: (f64, f64)) -> Look {
    Look {
        at: PAIR,
        distance,
        yaw,
        pitch,
    }
}

/// Watches the pulsar's closest approach turn, from now.
fn watch_turning(app: &mut WorldlineApp) {
    app.watch = PerihelionWatch::start(&app.simulation, PULSAR, COMPANION);
}

/// At the end: the waves window, to see and hear the pair's waves.
fn open_the_waves(app: &mut WorldlineApp) {
    if let Some(index) = app.simulation.index_of(PULSAR) {
        app.selected = index;
        app.open_waves();
    }
}

pub static HULSE_TAYLOR: Script = Script {
    shots: &[
        Shot {
            name: "Found",
            seconds: 12.0,
            speed: HOUR,
            look: at_pair((1.6e10, 1.0e10), (200.0, 230.0), (32.0, 24.0)),
            caption: "1974, the Arecibo radio telescope in Puerto Rico. Russell Hulse and Joseph \
                      Taylor find a pulsar whose pulses come faster, then slower, every 7.75 \
                      hours: it is circling something they cannot see.",
            cards: &[Card {
                at: Pinned::Left,
                title: "PSR B1913+16",
                lines: &[
                    Line::Text("The Hulse–Taylor pulsar, in the constellation Aquila"),
                    Line::Text("Found in 1974 with Arecibo's 305 m radio dish"),
                ],
                source: "Hulse & Taylor 1975, ApJ 195, L51",
            }],
            trails: true,
            orbits: false,
            begin: None,
        },
        Shot {
            name: "Two neutron stars",
            seconds: 14.0,
            speed: HOUR,
            look: at_pair((1.0e10, 6.5e9), (230.0, 262.0), (24.0, 16.0)),
            caption: "A pulsar is a neutron star: more mass than the Sun's in a ball the size of \
                      a city, its beam of radio waves sweeping past Earth with every turn. This \
                      one turns 17 times a second, and every pulse is the tick of a clock.",
            cards: &[
                Card {
                    at: Pinned::RightOf(PULSAR),
                    title: "The pulsar",
                    lines: &[Line::Live(pulsar_mass), Line::Live(spin), Line::Live(size)],
                    source: "Mass: Weisberg & Huang 2016, ApJ 829, 55; spin: their Table 1; \
                             radius: Abbott et al. 2018",
                },
                Card {
                    at: Pinned::LeftOf(COMPANION),
                    title: "Its companion",
                    lines: &[
                        Line::Live(companion_mass),
                        Line::Text(
                            "No pulses have been seen from it: by its mass, a second neutron star",
                        ),
                    ],
                    source: "Weisberg & Huang 2016",
                },
            ],
            trails: true,
            orbits: false,
            begin: None,
        },
        Shot {
            name: "The orbit",
            seconds: 16.0,
            speed: HOUR,
            look: at_pair((8.0e9, 7.0e9), (262.0, 290.0), (40.0, 62.0)),
            caption: "Every 7.75 hours they swing from 3.2 million km apart to 750,000 km, where \
                      they pass each other at 900 km/s.",
            cards: &[
                Card {
                    at: Pinned::Left,
                    title: "The orbit, as timed",
                    lines: &[Line::Live(period), Line::Live(eccentricity)],
                    source: "Weisberg & Huang 2016, Table 2",
                },
                Card {
                    at: Pinned::RightOf(PULSAR),
                    title: "Now, in this simulation",
                    lines: &[Line::Live(apart), Line::Live(speed)],
                    source: "Measured from the simulation as it plays",
                },
            ],
            trails: false,
            orbits: true,
            begin: None,
        },
        Shot {
            name: "Relativity turns it",
            seconds: 24.0,
            speed: MONTH,
            look: at_pair((7.0e9, 7.0e9), (290.0, 300.0), (62.0, 64.0)),
            caption: "Time now runs a month a second. Newton's gravity would hold the orbit \
                      still; general relativity turns its closest point round by 4.2° a year, \
                      35,000 times as fast as Mercury's.",
            cards: &[Card {
                at: Pinned::Left,
                title: "The closest point turns",
                lines: &[
                    Line::Live(turned),
                    Line::Live(turning_here),
                    Line::Live(turning_theory),
                    Line::Live(turning_timed),
                    Line::Text(
                        "The masses were found from this turning and one other effect of \
                         relativity, so on its own it can't test the theory. The shrinking can.",
                    ),
                ],
                source: "Timing: Weisberg & Huang 2016; the formula: Robertson 1938",
            }],
            trails: false,
            orbits: true,
            begin: Some(watch_turning),
        },
        Shot {
            name: "Waves shrink it",
            seconds: 18.0,
            speed: MONTH,
            look: at_pair((7.0e9, 8.5e9), (300.0, 330.0), (64.0, 45.0)),
            caption: "Circling, the pair shakes spacetime itself, sending out gravitational \
                      waves. The energy they carry off shrinks the orbit: each year it takes 76 \
                      millionths of a second less to go round, and in 300 million years the two \
                      will collide.",
            cards: &[Card {
                at: Pinned::Left,
                title: "The orbit shrinks",
                lines: &[
                    Line::Live(shrink_here),
                    Line::Live(shrink_timed),
                    Line::Live(agreement),
                    Line::Live(merging),
                ],
                source: "Weisberg & Huang 2016; the formula: Peters & Mathews 1963",
            }],
            trails: false,
            orbits: true,
            begin: None,
        },
        Shot {
            name: "Timed, not seen",
            seconds: 18.0,
            speed: MONTH,
            look: at_pair((8.5e9, 9.5e9), (330.0, 350.0), (45.0, 35.0)),
            caption: "No telescope can see the orbit shrink: it was timed. Against an orbit that \
                      kept its size, each closest approach came a little earlier: 38.5 seconds \
                      early after 30 years, along the curve general relativity predicts.",
            cards: &[Card {
                at: Pinned::Left,
                title: "Timed, not seen",
                lines: &[
                    Line::Plot(earlier_plot),
                    Line::Text(
                        "How much earlier each closest approach comes than in an orbit that \
                         kept its size: after a time t, ½ (dP/dt) t² / P",
                    ),
                    Line::Text("First seen in 1979 (Taylor, Fowler & McCulloch, Nature 277, 437)"),
                    Line::Live(agreement),
                ],
                source: "The curve from the measured period and its rate of change: \
                         Weisberg & Huang 2016",
            }],
            trails: false,
            orbits: true,
            begin: None,
        },
        Shot {
            name: "1993",
            seconds: 14.0,
            speed: HOUR,
            look: at_pair((9.5e9, 2.2e10), (350.0, 380.0), (35.0, 28.0)),
            caption: "In 1993 Hulse and Taylor won the Nobel Prize in Physics for finding the \
                      pulsar. Its shrinking orbit was the first evidence that gravitational waves \
                      exist; in 2015 LIGO heard them directly, from two black holes merging.",
            cards: &[Card {
                at: Pinned::Left,
                title: "What came next",
                lines: &[
                    Line::Text("1993: the Nobel Prize in Physics, for Hulse and Taylor"),
                    Line::Text(
                        "2015: LIGO hears two black holes merge, GW150914, a scenario of its own \
                         here",
                    ),
                    Line::Text(
                        "When the film ends, the gravitational-waves window opens: this pair's \
                         waves, and how they'd sound",
                    ),
                ],
                source: "",
            }],
            trails: false,
            orbits: true,
            begin: None,
        },
    ],
    end: open_the_waves,
    end_note: "it opened the gravitational-waves window",
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// Mercury's perihelion turns 42.98″ a century in general relativity,
    /// as the Mercury scenario measures it (step 8.1).
    const MERCURY_ARCSEC_PER_CENTURY: f64 = 42.98;

    /// Their orbit's size, closest and farthest apart (m), and their speed
    /// past each other at the closest (m/s), from the measured period by
    /// Kepler's third law.
    fn kepler() -> (f64, f64, f64) {
        let orbit = timed();
        let gm = object(PULSAR).gm() + object(COMPANION).gm();
        let a = (gm * (orbit.period / TAU).powi(2)).cbrt();
        let e = orbit.eccentricity;
        let fastest = (gm / a * (1.0 + e) / (1.0 - e)).sqrt();
        (a * (1.0 - e), a * (1.0 + e), fastest)
    }

    /// `x` to `figures` significant figures.
    fn rounded(x: f64, figures: i32) -> f64 {
        let scale = 10f64.powi(figures - 1 - x.abs().log10().floor() as i32);
        (x * scale).round() / scale
    }

    fn caption(name: &str) -> &'static str {
        HULSE_TAYLOR
            .shots
            .iter()
            .find(|s| s.name == name)
            .expect("a shot")
            .caption
    }

    #[test]
    fn the_captions_figures_follow_from_the_data() {
        let orbit = timed();
        let hours = orbit.period / 3600.0;
        let spins = orbit.spin_frequency.expect("published");
        let (closest, farthest, fastest) = kepler();
        let mercury = orbit.periastron_advance * 3600.0 * 100.0 / MERCURY_ARCSEC_PER_CENTURY;
        let (measured, _, _) = shrinking();
        let a_year = -measured * JULIAN_YEAR * 1e6;
        let thirty = earlier(&orbit, 30.0 * JULIAN_YEAR);
        println!(
            "{hours:.4} h; {spins:.3} turns a second; {:.0} to {:.0} km apart, {:.0} km/s at the \
             closest; {:.4}° a year, {mercury:.0} times Mercury's; {a_year:.2} µs less a year; \
             {thirty:.2} s earlier after 30 years",
            closest / 1e3,
            farthest / 1e3,
            fastest / 1e3,
            orbit.periastron_advance
        );
        assert_eq!(format!("{hours:.2}"), "7.75");
        assert!(caption("Found").contains("every 7.75 hours"));
        assert_eq!(spins.round(), 17.0);
        assert!(caption("Two neutron stars").contains("17 times a second"));
        let orbit_caption = caption("The orbit");
        assert!(orbit_caption.contains("Every 7.75 hours"));
        assert_eq!(rounded(farthest, 2), 3.2e9);
        assert!(orbit_caption.contains("3.2 million km"));
        assert_eq!(rounded(closest, 2), 7.5e8);
        assert!(orbit_caption.contains("750,000 km"));
        assert_eq!(rounded(fastest, 2), 9e5);
        assert!(orbit_caption.contains("900 km/s"));
        let turning = caption("Relativity turns it");
        assert_eq!(format!("{:.1}", orbit.periastron_advance), "4.2");
        assert!(turning.contains("4.2° a year"));
        assert_eq!(rounded(mercury, 2), 35_000.0);
        assert!(turning.contains("35,000 times"));
        assert_eq!(a_year.round(), 76.0);
        assert!(caption("Waves shrink it").contains("76 millionths of a second"));
        assert_eq!(format!("{thirty:.1}"), "38.5");
        assert!(caption("Timed, not seen").contains("38.5 seconds early after 30 years"));
    }

    #[test]
    fn the_pair_collides_when_the_caption_says() {
        // From the pair as the scenario sets it up: Peters (1964)'s time to
        // merge, for its orbit there.
        let mut app = WorldlineApp::new(&crate::StartOptions::default());
        app.play_scenario(scenarios::scenario("hulse-taylor").expect("listed"));
        let index = app.simulation.index_of(PULSAR).expect("the pulsar");
        let waves = app
            .simulation
            .gravitational_waves(index)
            .expect("a pair whose waves matter");
        let years = waves.merging / JULIAN_YEAR;
        println!("they merge in {:.1} million years", years / 1e6);
        assert_eq!(rounded(years, 1), 3e8);
        assert!(caption("Waves shrink it").contains("in 300 million years"));
    }

    #[test]
    fn the_formula_for_the_catalogs_masses_gives_the_timed_turning() {
        // Within what rounding the masses to 0.001 Suns allows, as in step
        // 2.2: (2/3)(0.002/2.828) = 4.7 × 10⁻⁴.
        let formula = degrees_a_year(turning_formula());
        let timed = timed().periastron_advance;
        println!("formula {formula:.6}° a year, timed {timed}°");
        assert!((formula / timed - 1.0).abs() < 4.7e-4);
    }

    #[test]
    fn the_measured_shrinking_is_general_relativitys_to_its_uncertainty() {
        let (measured, predicted, (ratio, sigma)) = shrinking();
        println!("measured {measured:e}, predicted {predicted:e}: {ratio:.4} ± {sigma:.4}");
        assert!((ratio - 1.0).abs() < 2.0 * sigma);
        assert_eq!(format!("{ratio:.3} ± {sigma:.3}"), "0.998 ± 0.002");
    }

    fn hulse_taylor() -> WorldlineApp {
        let mut app = WorldlineApp::new(&crate::StartOptions::default());
        app.play_scenario(scenarios::scenario("hulse-taylor").expect("listed"));
        app
    }

    #[test]
    fn the_film_plays_from_start_to_end_with_its_cards() {
        // Played as the app plays it, a quarter of a second a frame: each
        // shot at its speed, the camera on the pair, every card with
        // something to say, and at the end the waves window, said so.
        let mut app = hulse_taylor();
        assert!(app.film_playing());
        let lengths = HULSE_TAYLOR.lengths();
        let total: f64 = lengths.iter().sum();
        let frame = 0.25;
        let mut said = vec![false; lengths.len()];
        let start = app.simulation.time();
        let mut frames = 0;
        while app.film_playing() {
            app.physics_frame(frame);
            frames += 1;
            assert!(f64::from(frames) * frame <= total + frame, "it runs over");
            let reel = app.film.as_ref().expect("the film").reel;
            if reel.ended {
                break;
            }
            let shot = &HULSE_TAYLOR.shots[reel.shot];
            assert_eq!(app.speed_index, shot.speed, "{}", shot.name);
            assert_eq!(app.options.trails, shot.trails, "{}", shot.name);
            // Past the cut's easing, looking at the pair's center of mass,
            // from between the shot's two distances.
            if reel.into > 1.5 {
                let (i, j) = (
                    app.simulation.index_of(PULSAR).expect("the pulsar"),
                    app.simulation.index_of(COMPANION).expect("its companion"),
                );
                let (p, q) = (&app.simulation.bodies[i], &app.simulation.bodies[j]);
                let center = (p.position * p.gm + q.position * q.gm) / (p.gm + q.gm);
                let off = (app.camera.target - center).length();
                assert!(
                    off < 1e-9 * app.camera.distance,
                    "{}: {off} m off",
                    shot.name
                );
                let (near, far) = shot.look.distance;
                let d = app.camera.distance;
                assert!(
                    d >= near.min(far) * (1.0 - 1e-9) && d <= near.max(far) * (1.0 + 1e-9),
                    "{}: {d}",
                    shot.name
                );
            }
            if !said[reel.shot] && reel.into >= shot.seconds / 2.0 {
                said[reel.shot] = true;
                for card in shot.cards {
                    for line in card.lines {
                        let says = match line {
                            Line::Text(text) => !text.is_empty(),
                            Line::Live(live) => live(&app).is_some_and(|s| !s.is_empty()),
                            Line::Plot(plot) => plot(&app).is_some_and(|p| p.points.len() > 1),
                        };
                        assert!(says, "{}: {}", shot.name, card.title);
                    }
                }
            }
        }
        println!(
            "{frames} frames, {:.1} s of film, {:.2} years of the pair",
            f64::from(frames) * frame,
            (app.simulation.time() - start) / JULIAN_YEAR
        );
        assert!(said.iter().all(|&s| s));
        assert!(app.film.as_ref().is_some_and(|f| f.reel.ended));
        assert!(app.waves.is_some());
        let (notice, _) = app.status.as_ref().expect("a notice");
        assert!(notice.contains("gravitational-waves window"), "{notice}");
    }

    #[test]
    fn pausing_time_pauses_the_film_and_skipping_sets_up_the_shot() {
        let mut app = hulse_taylor();
        app.physics_frame(0.5);
        app.play_pause();
        let reel = app.film.as_ref().expect("the film").reel;
        app.physics_frame(0.5);
        let paused = app.film.as_ref().expect("the film").reel;
        assert!(paused.paused);
        assert_eq!(paused.into, reel.into);
        // Skipping to the turning sets its speed and starts watching.
        let k = HULSE_TAYLOR
            .shots
            .iter()
            .position(|s| s.name == "Relativity turns it")
            .expect("a shot");
        app.film_go_to(k);
        assert_eq!(app.speed_index, MONTH);
        assert!(app.watch.as_ref().is_some_and(|w| w.body == PULSAR));
        // Flying elsewhere leaves the film.
        app.focus_on(app.simulation.index_of(COMPANION).expect("its companion"));
        assert!(app.film.is_none());
    }

    /// The turning shot's two years, as the film plays them at 60 frames
    /// a second (half a day a frame), sampled every frame: the turning
    /// measured in the simulation is the one measured by timing, within
    /// two known differences:
    /// - rounding the catalog's masses to 0.001 Suns: the rate goes as
    ///   M^(2/3), so (2/3)(0.002/2.828) = 4.7 × 10⁻⁴, as in step 2.2's
    ///   check of the same orbit;
    /// - the catalogue sizing the orbit by Kepler's law: relativity makes
    ///   its period 1.5 × 10⁻⁴ longer than the measured one (step 2.2), and
    ///   the rate goes as P^(−5/3), so 2.5 × 10⁻⁴ slower.
    ///
    /// The limit is their sum, 7.2 × 10⁻⁴.
    #[test]
    #[ignore = "two years of the pair: about half a minute"]
    fn the_turning_the_film_measures_is_the_timed_one() {
        let mut app = hulse_taylor();
        let k = HULSE_TAYLOR
            .shots
            .iter()
            .position(|s| s.name == "Relativity turns it")
            .expect("a shot");
        app.film_go_to(k);
        let frames = (HULSE_TAYLOR.shots[k].seconds * 60.0) as usize;
        for _ in 0..frames {
            app.simulation.advance_by(SPEEDS[MONTH].1 / 60.0);
            app.watch
                .as_mut()
                .expect("watching")
                .sample(&app.simulation);
        }
        let watch = app.watch.as_ref().expect("watching");
        let (rate, span) = watch.rate().expect("a rate");
        let (here, theory) = (degrees_a_year(rate), degrees_a_year(watch.theory));
        let timed = timed().periastron_advance;
        println!(
            "over {:.2} years: turned {:.3}°, {here:.6}° a year; general relativity {theory:.6}°, \
             timed {timed}°",
            span / JULIAN_YEAR,
            watch.turned().to_degrees()
        );
        worldline_report::margins(
            "film-hulse-taylor",
            "The Hulse–Taylor film: the turning it measures, against the timed one",
        )
        .check(
            "the turning measured in the film's shot, against Weisberg & Huang's timing",
            here / timed - 1.0,
            7.2e-4,
            format!(
                "{here:.5}° a year against {timed}° (limit 7.2 × 10⁻⁴: the masses rounded to \
                 0.001 Suns, and the orbit sized by Kepler's law)"
            ),
        )
        .save(worldline_report::here!());
        assert!((here / timed - 1.0).abs() < 7.2e-4);
    }
}
