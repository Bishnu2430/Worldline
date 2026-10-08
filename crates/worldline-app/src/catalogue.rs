//! The catalogue of bodies to add in the sandbox: copies of the solar
//! system's own bodies, and the real objects of the notable-objects catalog
//! (black holes, neutron stars, white dwarfs and nearby stars) with their
//! published masses and sizes. See `docs/app.md`.

use std::sync::OnceLock;

use eframe::egui::Color32;
use worldline_core::constants::{AU, C, GM_SUN, SOLAR_RADIUS};
use worldline_core::gravity::relative_acceleration;
use worldline_core::orbit::periapsis_state;
use worldline_core::{Body, DVec3};
use worldline_data::{BinaryOrbit, NotableObject, ObjectKind, RadiusBasis};

use crate::simulation::Simulation;

/// The catalogue's sections, in the order shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Planets,
    Stars,
    WhiteDwarfs,
    NeutronStars,
    BlackHoles,
    Binaries,
}

impl Group {
    pub const ALL: [Group; 6] = [
        Group::Planets,
        Group::Stars,
        Group::WhiteDwarfs,
        Group::NeutronStars,
        Group::BlackHoles,
        Group::Binaries,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Planets => "Planets",
            Group::Stars => "Stars",
            Group::WhiteDwarfs => "White dwarfs",
            Group::NeutronStars => "Neutron stars",
            Group::BlackHoles => "Black holes",
            Group::Binaries => "Binaries",
        }
    }
}

/// What an entry adds.
#[derive(Debug, Clone)]
pub enum Template {
    /// A copy of one of the solar system's bodies (named as in the 2025
    /// snapshot), under a new name made from `noun`.
    Copy {
        model: &'static str,
        noun: &'static str,
    },
    /// An object from the notable-objects catalog.
    Notable(NotableObject),
    /// A binary from the catalog, on its measured orbit.
    Binary {
        orbit: BinaryOrbit,
        members: Box<[NotableObject; 2]>,
    },
    /// The two bodies of a gravitational-wave event, on a circular orbit
    /// whose wave is at `frequency` (Hz), spiraling in.
    Inspiral {
        members: Box<[NotableObject; 2]>,
        frequency: f64,
    },
}

/// The gravitational-wave events the catalogue offers as merging pairs, by
/// their two bodies' names in the catalog.
const INSPIRALS: [(&str, &str, &str); 2] = [
    ("GW170817", "GW170817 heavier star", "GW170817 lighter star"),
    ("GW150914", "GW150914 heavier hole", "GW150914 lighter hole"),
];

/// Where a catalogue inspiral starts: a wave at 20 Hz, in the band LIGO
/// hears. A choice, not a measurement: by then real pairs have long since
/// become circular.
const INSPIRAL_START: f64 = 20.0;

/// One thing the catalogue can add.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Stable identifier, recorded in saves.
    pub key: String,
    /// Its name in the catalogue.
    pub label: String,
    pub group: Group,
    /// One line on its mass and size.
    pub summary: String,
    pub template: Template,
}

/// Every entry, grouped as in [`Group::ALL`], lightest first in each group.
pub fn catalogue() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let snapshot = worldline_data::solar_system();
        let copy = |model: &'static str, noun: &'static str, label: &str, group: Group| {
            let body = snapshot
                .bodies
                .iter()
                .find(|b| b.name == model)
                .expect("the snapshot has the model");
            Entry {
                key: format!("copy:{model}"),
                label: label.to_string(),
                group,
                summary: format!("{model}'s mass and size, {}", mass_words(body.gm)),
                template: Template::Copy { model, noun },
            }
        };
        let mut entries = vec![
            copy("Earth", "planet", "Earth-mass planet", Group::Planets),
            copy("Neptune", "planet", "Neptune-mass planet", Group::Planets),
            copy("Jupiter", "planet", "Jupiter-mass planet", Group::Planets),
            copy("Sun", "star", "Sun-mass star", Group::Stars),
        ];
        let objects = worldline_data::notable_objects();
        let mut notable: Vec<&NotableObject> = objects.iter().collect();
        notable.sort_by(|a, b| a.mass.value.total_cmp(&b.mass.value));
        for o in notable {
            let group = match o.kind {
                ObjectKind::Star => Group::Stars,
                ObjectKind::WhiteDwarf => Group::WhiteDwarfs,
                ObjectKind::NeutronStar => Group::NeutronStars,
                ObjectKind::BlackHole => Group::BlackHoles,
            };
            entries.push(Entry {
                key: format!("object:{}", o.name),
                label: o.name.clone(),
                group,
                summary: format!("{}, {}", mass_words(o.gm()), size_words(o)),
                template: Template::Notable(o.clone()),
            });
        }
        for orbit in worldline_data::binary_orbits() {
            let members: Vec<NotableObject> = objects
                .iter()
                .filter(|o| o.system.as_deref() == Some(&orbit.system))
                .cloned()
                .collect();
            let Ok(members) = <[NotableObject; 2]>::try_from(members) else {
                continue;
            };
            entries.push(Entry {
                key: format!("binary:{}", orbit.system),
                label: format!("{} pair", orbit.system),
                group: Group::Binaries,
                summary: format!(
                    "{} and {} Suns, orbiting every {:.2} h",
                    members[0].mass.value,
                    members[1].mass.value,
                    orbit.period / 3600.0
                ),
                template: Template::Binary {
                    orbit,
                    members: Box::new(members),
                },
            });
        }
        for (event, first, second) in INSPIRALS {
            let find = |name: &str| objects.iter().find(|o| o.name == name).cloned();
            let (Some(a), Some(b)) = (find(first), find(second)) else {
                continue;
            };
            entries.push(Entry {
                key: format!("inspiral:{event}"),
                label: format!("{event} pair"),
                group: Group::Binaries,
                summary: format!(
                    "{} and {} Suns, spiraling in from a {INSPIRAL_START} Hz wave",
                    a.mass.value, b.mass.value
                ),
                template: Template::Inspiral {
                    members: Box::new([a, b]),
                    frequency: INSPIRAL_START,
                },
            });
        }
        entries.sort_by_key(|e| Group::ALL.iter().position(|g| *g == e.group));
        entries
    })
}

/// The entry with this key, if there is one.
pub fn entry(key: &str) -> Option<&'static Entry> {
    catalogue().iter().find(|e| e.key == key)
}

/// A mass in words: in Earth masses for planets, Suns above.
fn mass_words(gm: f64) -> String {
    if gm < 0.01 * GM_SUN {
        match significant(gm / crate::app::GM_EARTH).as_str() {
            "1" => "1 Earth mass".to_string(),
            n => format!("{n} Earth masses"),
        }
    } else {
        measured_suns(gm / GM_SUN, None)
    }
}

/// A mass in Suns, with its published uncertainty if given, in the same
/// unit: "4.297 ± 0.012 million Suns", "35.6 +4.7/−3.1 Suns", "1 Sun".
pub fn measured_suns(suns: f64, plus_minus: Option<(f64, f64)>) -> String {
    let (scale, word) = if suns >= 1e9 {
        (1e9, " billion")
    } else if suns >= 1e6 {
        (1e6, " million")
    } else {
        (1.0, "")
    };
    // The uncertainty to the same last digit as the value, as published.
    let value = significant(suns / scale);
    let decimals = value.split_once('.').map_or(0, |(_, d)| d.len());
    let digits = |x: f64| format!("{:.decimals$}", x / scale);
    let unit = if value == "1" && word.is_empty() {
        "Sun"
    } else {
        "Suns"
    };
    match plus_minus {
        Some((p, m)) if p == m => format!("{value} ± {}{word} {unit}", digits(p)),
        Some((p, m)) => format!("{value} +{}/−{}{word} {unit}", digits(p), digits(m)),
        None => format!("{value}{word} {unit}"),
    }
}

/// A radius in words, saying what kind of size it is.
fn size_words(o: &NotableObject) -> String {
    let r = length(o.radius.value);
    match o.radius_basis {
        RadiusBasis::Horizon => format!("horizon {r}"),
        RadiusBasis::Measured => format!("radius {r}"),
        RadiusBasis::Estimated => format!("radius {r} (estimated)"),
        RadiusBasis::Assumed => format!("radius {r} (assumed)"),
    }
}

/// A number to four significant figures, without trailing zeros, in
/// words above a million: "4.297 million".
pub fn significant(x: f64) -> String {
    let (x, word) = if x >= 1e9 {
        (x / 1e9, " billion")
    } else if x >= 1e6 {
        (x / 1e6, " million")
    } else {
        (x, "")
    };
    let digits = (3 - x.abs().log10().floor() as i32).clamp(0, 6) as usize;
    let s = format!("{x:.digits$}");
    let s = if s.contains('.') {
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    } else {
        s
    };
    format!("{s}{word}")
}

/// A length in the unit that suits it: AU from a hundredth of one (where
/// it compares with planetary orbits), the Sun's radius for stars (from a
/// tenth of it), else km.
pub fn length(meters: f64) -> String {
    if meters >= 0.01 * AU {
        format!("{} AU", significant(meters / AU))
    } else if meters >= 0.1 * SOLAR_RADIUS {
        format!("{} Sun radii", significant(meters / SOLAR_RADIUS))
    } else {
        format!("{} km", significant(meters / 1e3))
    }
}

/// A distance in light-years (and parsecs, which astronomers use).
pub fn light_years(meters: f64) -> String {
    use worldline_core::constants::{LIGHT_YEAR, PARSEC};
    format!(
        "{} light-years ({} parsecs)",
        significant(meters / LIGHT_YEAR),
        significant(meters / PARSEC)
    )
}

impl Entry {
    /// The total GM of what it adds, in m³/s².
    pub fn gm(&self) -> f64 {
        match &self.template {
            Template::Copy { model, .. } => {
                worldline_data::solar_system()
                    .bodies
                    .iter()
                    .find(|b| b.name == *model)
                    .expect("the snapshot has the model")
                    .gm
            }
            Template::Notable(o) => o.gm(),
            Template::Binary { members, .. } | Template::Inspiral { members, .. } => {
                members[0].gm() + members[1].gm()
            }
        }
    }

    /// What kind of object it is, for the catalog's own objects.
    pub fn kind(&self) -> Option<ObjectKind> {
        match &self.template {
            Template::Copy { .. } => None,
            Template::Notable(o) => Some(o.kind),
            Template::Binary { members, .. } | Template::Inspiral { members, .. } => {
                Some(members[0].kind)
            }
        }
    }

    /// The catalog object behind it, if it is one.
    pub fn object(&self) -> Option<&NotableObject> {
        match &self.template {
            Template::Notable(o) => Some(o),
            _ => None,
        }
    }

    /// The bodies it adds, relative to their center of mass, under names
    /// not yet used in `simulation`. A copy is named like "New planet 2";
    /// a catalog object keeps its name, numbered if it is already there. A
    /// binary's pair starts at its closest approach, on its measured
    /// period and eccentricity, in the plane of the ecliptic (its real
    /// orientation on the sky has no meaning here).
    pub fn bodies(&self, simulation: &Simulation) -> Vec<Body> {
        let free = |names: &[&str]| {
            (1..)
                .map(|n| {
                    names
                        .iter()
                        .map(|name| match n {
                            1 => name.to_string(),
                            n => format!("{name} {n}"),
                        })
                        .collect::<Vec<_>>()
                })
                .find(|names| names.iter().all(|name| simulation.index_of(name).is_none()))
                .expect("a free name")
        };
        match &self.template {
            Template::Copy { model, noun } => {
                let snapshot = worldline_data::solar_system();
                let model = snapshot
                    .bodies
                    .iter()
                    .find(|b| b.name == *model)
                    .expect("the snapshot has the model");
                let name = (1..)
                    .map(|n| format!("New {noun} {n}"))
                    .find(|name| simulation.index_of(name).is_none())
                    .expect("a free name");
                vec![Body::new(name, model.gm, model.radius)]
            }
            Template::Notable(o) => {
                let name = free(&[&o.name]).remove(0);
                vec![Body::new(name, o.gm(), o.radius.value)]
            }
            Template::Binary { orbit, members } => {
                let names = free(&[&members[0].name, &members[1].name]);
                let (m1, m2) = (members[0].gm(), members[1].gm());
                let gm = m1 + m2;
                // Kepler's third law gives the orbit for the measured
                // period (relativity lengthens it by about 10⁻⁴).
                let a = (gm * (orbit.period / std::f64::consts::TAU).powi(2)).cbrt();
                let (r, v) = periapsis_state(a, orbit.eccentricity, gm);
                let (w1, w2) = (m2 / gm, m1 / gm);
                vec![
                    Body::new(names[0].clone(), m1, members[0].radius.value)
                        .at(-r * w1)
                        .moving(-v * w1),
                    Body::new(names[1].clone(), m2, members[1].radius.value)
                        .at(r * w2)
                        .moving(v * w2),
                ]
            }
            Template::Inspiral { members, frequency } => {
                let names = free(&[&members[0].name, &members[1].name]);
                let (m1, m2) = (members[0].gm(), members[1].gm());
                let gm = m1 + m2;
                let nu = m1 * m2 / (gm * gm);
                // The orbit is half the wave's frequency. Its speed is the
                // relativistic equations' circular speed (Newton's would be
                // visibly eccentric this close), v² = r |a·n|, and its size
                // is found alongside: Kepler's law first, then rescaled as
                // r ∝ ω^(−2/3) until the angular rate v/r is the wanted one
                // (relativity slows it by about (3 − ν)/2 · GM/(rc²), 1% at
                // 20 Hz for neutron stars). It already falls inward at the
                // leading-order rate, ṙ = −(64/5) G³m³ν / (r³c⁵).
                let omega = std::f64::consts::PI * frequency;
                let mut r = (gm / (omega * omega)).cbrt();
                let mut speed = (gm / r).sqrt();
                for _ in 0..40 {
                    let x = DVec3::new(r, 0.0, 0.0);
                    let a = relative_acceleration(gm, nu, x, DVec3::new(0.0, speed, 0.0));
                    speed = (r * -(a.newtonian + a.first + a.second).x).sqrt();
                    r *= (speed / (r * omega)).powf(2.0 / 3.0);
                }
                let x = DVec3::new(r, 0.0, 0.0);
                let rdot = -64.0 / 5.0 * gm.powi(3) * nu / (r.powi(3) * C.powi(5));
                let v = DVec3::new(rdot, speed, 0.0);
                let (w1, w2) = (m2 / gm, m1 / gm);
                vec![
                    Body::new(names[0].clone(), m1, members[0].radius.value)
                        .at(-x * w1)
                        .moving(-v * w1),
                    Body::new(names[1].clone(), m2, members[1].radius.value)
                        .at(x * w2)
                        .moving(v * w2),
                ]
            }
        }
    }
}

/// The color a body added from the catalogue is drawn in: by kind for the
/// catalog's objects (black holes get the color of their ring, since the
/// hole itself is dark), the sandbox's pink for copies.
pub fn kind_color(kind: Option<ObjectKind>) -> Color32 {
    match kind {
        Some(ObjectKind::Star) => Color32::from_rgb(255, 232, 196),
        Some(ObjectKind::WhiteDwarf) => Color32::from_rgb(222, 234, 255),
        Some(ObjectKind::NeutronStar) => Color32::from_rgb(150, 205, 255),
        Some(ObjectKind::BlackHole) => Color32::from_rgb(255, 168, 84),
        None => crate::sandbox::SANDBOX_COLOR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::DVec3;

    #[test]
    fn numbers_read_naturally() {
        assert_eq!(significant(4.297e6), "4.297 million");
        assert_eq!(significant(6.606_934e10), "66.07 billion");
        assert_eq!(significant(1.438), "1.438");
        assert_eq!(significant(0.1221), "0.1221");
        assert_eq!(significant(21.2), "21.2");
        assert_eq!(length(1.268e10), "0.08476 AU");
        assert_eq!(length(33_020.0), "33.02 km");
        assert_eq!(
            measured_suns(4.297e6, Some((0.012e6, 0.012e6))),
            "4.297 ± 0.012 million Suns"
        );
        assert_eq!(measured_suns(35.6, Some((4.7, 3.1))), "35.6 +4.7/−3.1 Suns");
        assert_eq!(measured_suns(1.0, None), "1 Sun");
        assert_eq!(
            measured_suns(1.438, Some((0.001, 0.001))),
            "1.438 ± 0.001 Suns"
        );
    }

    #[test]
    fn the_catalogue_offers_every_catalog_object() {
        let entries = catalogue();
        // 4 copies, 20 catalog objects, the Hulse–Taylor pair and the
        // GW170817 and GW150914 pairs.
        assert_eq!(entries.len(), 27);
        let groups: Vec<Group> = entries.iter().map(|e| e.group).collect();
        assert!(groups.windows(2).all(|w| {
            Group::ALL.iter().position(|g| *g == w[0]) <= Group::ALL.iter().position(|g| *g == w[1])
        }));
        let sgr = entry("object:Sagittarius A*").expect("listed");
        assert_eq!(sgr.summary, "4.297 million Suns, horizon 0.08483 AU");
        assert_eq!(sgr.gm(), 4.297e6 * GM_SUN);
        assert_eq!(
            entry("object:Sirius B").unwrap().summary,
            "1.018 Suns, radius 5586 km"
        );
        assert_eq!(
            entry("copy:Earth").unwrap().summary,
            "Earth's mass and size, 1 Earth mass"
        );
    }

    #[test]
    fn an_inspiral_starts_with_its_wave_at_20_hz_and_merges_soon() {
        // GW170817's neutron stars, on a circular orbit whose wave is at
        // 20 Hz: the orbital frequency is 10 Hz, and gravitational waves
        // merge them within minutes (Peters & Mathews' leading-order time
        // from the orbit's Newtonian elements: 2.4 minutes).
        let mut sim = Simulation::solar_system(1.0);
        let pair = entry("inspiral:GW170817").expect("listed");
        assert_eq!(pair.label, "GW170817 pair");
        let bodies = pair.bodies(&sim);
        let x = bodies[0].position - bodies[1].position;
        let v = bodies[0].velocity - bodies[1].velocity;
        let wave = x.cross(v).length() / x.length_squared() / std::f64::consts::PI;
        println!("the wave starts at {wave} Hz");
        // Exact up to rounding.
        assert!((wave / 20.0 - 1.0).abs() < 1e4 * f64::EPSILON, "{wave}");
        let above = DVec3::new(0.0, 0.0, 30.0 * AU);
        let indices: Vec<usize> = bodies
            .into_iter()
            .map(|b| {
                let position = b.position + above;
                sim.add_body(b.at(position), &pair.key)
            })
            .collect();
        let waves = sim.gravitational_waves(indices[0]).expect("a merging pair");
        println!("merges in {:.1} minutes", waves.merging / 60.0);
        assert!(waves.merging < 3.0 * 60.0);
    }

    #[test]
    fn a_binary_starts_at_closest_approach_around_its_center_of_mass() {
        let sim = Simulation::solar_system(1.0);
        let pair = entry("binary:PSR B1913+16").expect("listed");
        let bodies = pair.bodies(&sim);
        assert_eq!(bodies[0].name, "PSR B1913+16");
        assert_eq!(bodies[1].name, "PSR B1913+16 companion");
        let center: DVec3 = bodies.iter().map(|b| b.position * b.gm).sum();
        let momentum: DVec3 = bodies.iter().map(|b| b.velocity * b.gm).sum();
        assert!(center.length() < 1e-6 * bodies[0].position.length() * pair.gm());
        assert!(momentum.length() < 1e-6 * bodies[0].velocity.length() * pair.gm());
        // 1.1 Suns' radii apart at closest approach.
        let gap = (bodies[1].position - bodies[0].position).length() / SOLAR_RADIUS;
        assert!((gap - 1.07).abs() < 0.01, "{gap}");
    }
}
