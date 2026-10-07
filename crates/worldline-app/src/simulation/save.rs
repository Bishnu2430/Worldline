//! Saving the simulation to text, and loading it back.
//!
//! A save records the moment and every body's state: the top level
//! (relative to the solar system's barycenter), each moon system (relative
//! to its barycenter) and the followers, plus which bodies were added in the
//! sandbox and the catalogue entry each came from. Everything else (gravity
//! fields, rotation, the data behind each
//! body) comes from the bundled data when it loads. Numbers are written in
//! their shortest exact form, so they load back bit for bit.

use std::fmt::Write;

use super::*;

/// The first line of every save. Version 2 records each added body's
/// catalogue entry; version 1 saves load too.
const HEADER: &str = "worldline-save\t2";
const HEADER_1: &str = "worldline-save\t1";

fn body_line(kind: &str, prefix: &str, b: &Body) -> String {
    let (p, v) = (b.position, b.velocity);
    format!(
        "{kind}\t{prefix}{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        b.name, b.gm, b.radius, p.x, p.y, p.z, v.x, v.y, v.z
    )
}

/// A body as saved: name, GM, radius, position, velocity.
struct Saved {
    name: String,
    gm: f64,
    radius: f64,
    position: DVec3,
    velocity: DVec3,
}

fn parse_body(fields: &[&str]) -> Result<Saved, String> {
    if fields.len() != 9 {
        return Err(format!("expected 9 fields for `{}`", fields.join(" ")));
    }
    let numbers: Vec<f64> = fields[1..]
        .iter()
        .map(|f| {
            f.parse::<f64>()
                .map_err(|_| format!("`{f}` is not a number"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Saved {
        name: fields[0].to_string(),
        gm: numbers[0],
        radius: numbers[1],
        position: DVec3::new(numbers[2], numbers[3], numbers[4]),
        velocity: DVec3::new(numbers[5], numbers[6], numbers[7]),
    })
}

impl Simulation {
    /// The simulation's state as text.
    pub fn save(&self) -> String {
        let mut out = String::new();
        let h = &self.hierarchy;
        let _ = writeln!(
            out,
            "# Worldline save. Tab-separated; SI units (m, m/s, m^3/s^2). Top-level bodies and \
             followers relative to the solar system barycenter; moons relative to their system's \
             barycenter."
        );
        let _ = writeln!(out, "{HEADER}");
        let _ = writeln!(out, "epoch_jd_tdb\t{}", self.epoch_jd_tdb);
        let _ = writeln!(out, "time\t{}", self.time());
        for b in &h.top.bodies {
            let _ = writeln!(out, "{}", body_line("top", "", b));
        }
        for moons in &h.moon_systems {
            let host = format!("{}\t", moons.system.bodies[0].name);
            for b in &moons.system.bodies {
                let _ = writeln!(out, "{}", body_line("moon", &host, b));
            }
        }
        for i in 0..h.follower_count() {
            let _ = writeln!(out, "{}", body_line("follower", "", h.follower(i)));
        }
        for (name, entry) in &self.added {
            let _ = writeln!(out, "added\t{name}\t{entry}");
        }
        if let Some(system) = self.detailed {
            let planet = &h.moon_systems[system].system.bodies[0].name;
            let _ = writeln!(out, "detailed\t{planet}");
        }
        out
    }

    /// The simulation a save describes, running at `speed`.
    pub fn load(text: &str, speed: f64) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty());
        if !matches!(lines.next(), Some(HEADER | HEADER_1)) {
            return Err("not a Worldline save (or from a newer version)".to_string());
        }
        let mut sim = Simulation::solar_system(speed);
        let (mut epoch, mut time, mut detailed) = (None, None, None);
        let (mut top, mut moons, mut followers, mut added) =
            (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for line in lines {
            let fields: Vec<&str> = line.split('\t').collect();
            let number = || {
                fields
                    .get(1)
                    .and_then(|f| f.parse::<f64>().ok())
                    .ok_or_else(|| format!("bad line `{line}`"))
            };
            match fields[0] {
                "epoch_jd_tdb" => epoch = Some(number()?),
                "time" => time = Some(number()?),
                "top" => top.push(parse_body(&fields[1..])?),
                "moon" if fields.len() > 2 => {
                    moons.push((fields[1].to_string(), parse_body(&fields[2..])?))
                }
                "follower" => followers.push(parse_body(&fields[1..])?),
                // Version 1 didn't record where added bodies came from.
                "added" if fields.len() == 2 => added.push((fields[1].to_string(), String::new())),
                "added" if fields.len() == 3 => {
                    added.push((fields[1].to_string(), fields[2].to_string()))
                }
                "detailed" if fields.len() == 2 => detailed = Some(fields[1].to_string()),
                _ => return Err(format!("unknown line `{line}`")),
            }
        }
        let (Some(epoch), Some(time)) = (epoch, time) else {
            return Err("the save has no epoch or time".to_string());
        };
        if epoch != sim.epoch_jd_tdb {
            return Err(format!("the save starts at JD {epoch}, not this snapshot"));
        }

        let is_added = |name: &str| added.iter().any(|(n, _): &(String, String)| n == name);
        let h = &mut sim.hierarchy;
        // If something absorbed the Sun, it took the Sun's place as body 0.
        if let Some(first) = top.first()
            && first.name != "Sun"
        {
            if !is_added(&first.name) {
                return Err(format!("unknown body `{}`", first.name));
            }
            h.top.bodies[0] = Body::new(&first.name, first.gm, first.radius);
        }
        // Remove what the save doesn't have, last first so indices hold.
        for k in (1..h.top.bodies.len()).rev() {
            if !top.iter().any(|s| s.name == h.top.bodies[k].name) {
                if let Some(system) = h.moon_systems.iter().position(|m| m.host == k) {
                    sim.small.remove(system);
                }
                h.remove_body(k);
            }
        }
        for i in (0..h.follower_count()).rev() {
            if !followers.iter().any(|s| s.name == h.follower(i).name) {
                h.remove_follower(i);
            }
        }
        // Add the sandbox's bodies, then set every state.
        for saved in &top {
            if !h.top.bodies.iter().any(|b| b.name == saved.name) {
                if !is_added(&saved.name) {
                    return Err(format!("unknown body `{}`", saved.name));
                }
                h.add_body(Body::new(&saved.name, saved.gm, saved.radius));
            }
        }
        for saved in &top {
            let body = h
                .top
                .bodies
                .iter_mut()
                .find(|b| b.name == saved.name)
                .ok_or_else(|| format!("unknown body `{}`", saved.name))?;
            (body.gm, body.radius) = (saved.gm, saved.radius);
            (body.position, body.velocity) = (saved.position, saved.velocity);
        }
        for (host, saved) in &moons {
            let body = h
                .moon_systems
                .iter_mut()
                .find(|m| &m.system.bodies[0].name == host)
                .and_then(|m| m.system.bodies.iter_mut().find(|b| b.name == saved.name))
                .ok_or_else(|| format!("unknown moon `{}`", saved.name))?;
            (body.position, body.velocity) = (saved.position, saved.velocity);
        }
        for saved in &followers {
            let index = (0..h.follower_count())
                .find(|&i| h.follower(i).name == saved.name)
                .ok_or_else(|| format!("unknown body `{}`", saved.name))?;
            let body = h.follower_mut(index);
            (body.position, body.velocity) = (saved.position, saved.velocity);
        }
        h.set_time(time);
        h.restart();
        sim.added = added;
        sim.detailed = None;
        sim.trails.clear();
        sim.bodies.clear();
        sim.reindex();
        sim.refresh();
        // Small moons in detail restart from their mean orbits.
        if let Some(planet) = detailed.and_then(|name| sim.index_of(&name)) {
            sim.focus_detail_on(planet);
        }
        sim.belts_time = f64::NAN;
        sim.refresh_small_moons();
        Ok(sim)
    }
}
