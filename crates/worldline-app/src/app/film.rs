//! Scenarios as films (step 8.5a). A film is a script of shots. Each shot
//! says what the camera looks at, from where and how it moves, how fast
//! time runs, what's said over it, and which cards of information sit
//! beside the bodies. It plays, pauses and skips like a film.
//!
//! The physics is the scenario's own, running as it always does: the film
//! only directs the camera, sets the speed and explains. Every number on a
//! card is published (with its source, from the data's CSV files) or
//! measured from the simulation as it plays.

use super::*;
use worldline_core::orbit::{osculating_orbit, periapsis_vector};

/// A film: its shots, in order, and what's done once the last ends.
pub struct Script {
    pub shots: &'static [Shot],
    /// Done at the end, to explore on with (a window opened, say).
    pub end: fn(&mut WorldlineApp),
    /// What the end does, for the notice it shows.
    pub end_note: &'static str,
}

impl Script {
    /// How long each shot lasts, s.
    pub fn lengths(&self) -> Vec<f64> {
        self.shots.iter().map(|s| s.seconds).collect()
    }
}

/// One shot.
pub struct Shot {
    /// Its name, on the film's bar.
    pub name: &'static str,
    /// How long it lasts, s of the film.
    pub seconds: f64,
    /// How fast time runs: a place in [`SPEEDS`].
    pub speed: usize,
    /// Where the camera looks from.
    pub look: Look,
    /// What's said over it, at the bottom of the view.
    pub caption: &'static str,
    /// Cards of information.
    pub cards: &'static [Card],
    /// Whether the bodies' trails are drawn.
    pub trails: bool,
    /// Whether the orbits of the pair looked at are drawn, as they are at
    /// each moment, with the direction of their closest approach.
    pub orbits: bool,
    /// Done as it begins (watching something, say).
    pub begin: Option<fn(&mut WorldlineApp)>,
}

/// Where the camera looks from during a shot: from its start to its end,
/// eased in and out.
pub struct Look {
    /// The bodies whose center of mass it looks at.
    pub at: &'static [&'static str],
    /// How far from it, m.
    pub distance: (f64, f64),
    /// Its angle around the vertical, degrees.
    pub yaw: (f64, f64),
    /// And above the ecliptic plane, degrees.
    pub pitch: (f64, f64),
}

/// Where a card sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pinned {
    /// In the view's left column.
    Left,
    /// In the left column, with a line to this body.
    LeftOf(&'static str),
    /// In the right column, with a line to this body.
    RightOf(&'static str),
}

/// A card of information.
pub struct Card {
    pub at: Pinned,
    pub title: &'static str,
    pub lines: &'static [Line],
    /// Where its numbers come from.
    pub source: &'static str,
}

/// A line of a card.
pub enum Line {
    /// Written in the script.
    Text(&'static str),
    /// Worked out as it plays, from the data or the simulation; `None`
    /// leaves the line out.
    Live(fn(&WorldlineApp) -> Option<String>),
    /// A small plot, worked out as it plays.
    Plot(fn(&WorldlineApp) -> Option<Plot>),
}

/// A curve on a card.
pub struct Plot {
    pub points: Vec<(f64, f64)>,
    /// The axes' ends and what they measure.
    pub x: (f64, &'static str),
    pub y: (f64, &'static str),
    /// A point picked out, and what it says.
    pub mark: Option<(f64, f64, String)>,
}

/// Where a film is: which shot, how far into it (s), and whether it's
/// paused or over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reel {
    pub shot: usize,
    pub into: f64,
    pub paused: bool,
    pub ended: bool,
}

/// Back within this long of a shot's start goes to the shot before; later,
/// to this one's start, as a player's back button does.
const BACK_GRACE: f64 = 2.0;

impl Reel {
    pub fn start() -> Self {
        Self {
            shot: 0,
            into: 0.0,
            paused: false,
            ended: false,
        }
    }

    /// Plays `dt` s on, through as many cuts as that takes, shots
    /// `lengths` s long. Returns the shot just begun, if a cut came. At the
    /// last shot's end, it ends, there.
    pub fn advance(&mut self, lengths: &[f64], dt: f64) -> Option<usize> {
        if self.paused || self.ended {
            return None;
        }
        self.into += dt;
        let mut began = None;
        while self.into >= lengths[self.shot] {
            if self.shot + 1 == lengths.len() {
                self.into = lengths[self.shot];
                self.ended = true;
                return began;
            }
            self.into -= lengths[self.shot];
            self.shot += 1;
            began = Some(self.shot);
        }
        began
    }

    /// Goes to the start of shot `k`, returning it; past the last, the film
    /// ends.
    pub fn go_to(&mut self, lengths: &[f64], k: usize) -> Option<usize> {
        if k >= lengths.len() {
            self.shot = lengths.len() - 1;
            self.into = lengths[self.shot];
            self.ended = true;
            return None;
        }
        self.shot = k;
        self.into = 0.0;
        self.ended = false;
        Some(k)
    }

    /// Back to this shot's start, or the one before's if this one began
    /// moments ago.
    pub fn back(&mut self, lengths: &[f64]) -> Option<usize> {
        let k = if self.into < BACK_GRACE && !self.ended {
            self.shot.saturating_sub(1)
        } else {
            self.shot
        };
        self.go_to(lengths, k)
    }

    /// How far into the whole film it is, s.
    pub fn elapsed(&self, lengths: &[f64]) -> f64 {
        lengths[..self.shot].iter().sum::<f64>() + self.into
    }
}

/// The camera's angles and distance, the distance as its logarithm so it
/// eases evenly in scale.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Framing {
    yaw: f64,
    pitch: f64,
    ln_distance: f64,
}

impl Framing {
    fn of(camera: &Camera) -> Self {
        Self {
            yaw: camera.yaw,
            pitch: camera.pitch,
            ln_distance: camera.distance.ln(),
        }
    }

    fn apply(self, camera: &mut Camera) {
        camera.yaw = self.yaw;
        camera.pitch = self.pitch.clamp(-HIGHEST, HIGHEST);
        camera.distance = self.ln_distance.exp();
    }

    /// `t` of the way to `to`, turning the shorter way round.
    fn toward(self, to: Self, t: f64) -> Self {
        let turn = (to.yaw - self.yaw + PI).rem_euclid(2.0 * PI) - PI;
        Self {
            yaw: self.yaw + turn * t,
            pitch: self.pitch + (to.pitch - self.pitch) * t,
            ln_distance: self.ln_distance + (to.ln_distance - self.ln_distance) * t,
        }
    }
}

/// The highest the camera looks down from: just short of straight down,
/// as the camera itself allows.
const HIGHEST: f64 = FRAC_PI_2 - 0.01;

/// How long a cut eases from the camera as it was, s.
const CUT: f64 = 1.5;

/// How long the film fades in from black, and captions and cards fade in
/// and out, s.
const FADE: f64 = 0.6;

/// Eased from 0 to 1 and back to rest at both ends.
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// How visible something shown for a shot is, `into` s into it: fading in
/// after `delay` s and out over the shot's last moments.
fn fade(into: f64, seconds: f64, delay: f64) -> f32 {
    ((into - delay) / FADE)
        .min((seconds - into) / FADE)
        .clamp(0.0, 1.0) as f32
}

/// A film playing: its script, where it is, and the camera it directs.
pub struct Film {
    pub title: &'static str,
    pub script: &'static Script,
    pub reel: Reel,
    /// The camera as the film last set it. What the viewer did to it since
    /// (dragging, zooming) is kept as an offset until the next cut, so they
    /// can still look around.
    set: Option<Framing>,
    offset: Framing,
    /// The camera at the last cut, eased away from.
    cut: Option<(DVec3, Framing)>,
    /// The view's grid and trails as they were: the film hides the grid
    /// (its rings, AU across around the Sun, would seem to measure a pair
    /// a thousand times smaller) and sets the trails shot by shot.
    grid: bool,
    trails: bool,
}

/// What the film's bar was asked to do.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Asked {
    Back,
    PlayPause,
    Next,
    GoTo(usize),
    Again,
    Leave,
}

/// The center of mass of the bodies named, if any are in the simulation.
fn center_of(simulation: &Simulation, names: &[&str]) -> Option<DVec3> {
    let (sum, gm) = names
        .iter()
        .filter_map(|name| simulation.index_of(name))
        .map(|i| &simulation.bodies[i])
        .fold((DVec3::ZERO, 0.0), |(sum, gm), b| {
            (sum + b.position * b.gm, gm + b.gm)
        });
    (gm > 0.0).then(|| sum / gm)
}

impl WorldlineApp {
    /// Starts playing `script` as the film of the scenario `title`, from
    /// its first shot.
    pub(super) fn start_film(&mut self, title: &'static str, script: &'static Script) {
        self.film = Some(Film {
            title,
            script,
            reel: Reel::start(),
            set: None,
            offset: Framing::default(),
            cut: None,
            grid: self.options.grid,
            trails: self.options.trails,
        });
        self.options.grid = false;
        self.begin_shot(0);
        // The first shot opens framed as written, out of black.
        if let Some(film) = &mut self.film {
            film.cut = None;
        }
    }

    /// Whether a film is playing (not over).
    pub(super) fn film_playing(&self) -> bool {
        self.film.as_ref().is_some_and(|f| !f.reel.ended)
    }

    /// What's done as shot `k` begins: its speed, what's drawn, its own
    /// start, and the camera eased from where it is.
    fn begin_shot(&mut self, k: usize) {
        let Some(film) = &mut self.film else {
            return;
        };
        let shot: &'static Shot = &film.script.shots[k];
        film.cut = Some((self.camera.target, Framing::of(&self.camera)));
        film.offset = Framing::default();
        film.set = None;
        self.flight = None;
        self.spin.stop();
        self.set_speed(shot.speed);
        self.options.trails = shot.trails;
        if let Some(begin) = shot.begin {
            begin(self);
        }
    }

    /// Plays the film on by `dt` s, before the physics: its pause is time's
    /// (the top bar's run and stop pause it too), and a cut sets up the
    /// shot it cuts to.
    pub(super) fn film_frame(&mut self, dt: f64) {
        let paused = self.simulation.paused;
        let Some(film) = &mut self.film else {
            return;
        };
        film.reel.paused = paused;
        let lengths = film.script.lengths();
        let was_over = film.reel.ended;
        let began = film.reel.advance(&lengths, dt);
        let over = film.reel.ended && !was_over;
        if let Some(k) = began {
            self.begin_shot(k);
        }
        if over {
            self.end_film();
        }
    }

    /// Points the camera as the shot says, after the physics has moved the
    /// bodies: at their center of mass, from its eased angles and
    /// distance, plus what the viewer turned and zoomed since the cut.
    pub(super) fn direct_camera(&mut self) {
        let Some(film) = &mut self.film else {
            return;
        };
        if film.reel.ended {
            return;
        }
        if let Some(set) = film.set {
            let now = Framing::of(&self.camera);
            film.offset.yaw += now.yaw - set.yaw;
            film.offset.pitch += now.pitch - set.pitch;
            film.offset.ln_distance += now.ln_distance - set.ln_distance;
        }
        let shot = &film.script.shots[film.reel.shot];
        let Some(center) = center_of(&self.simulation, shot.look.at) else {
            return;
        };
        let look = &shot.look;
        let t = smooth(film.reel.into / shot.seconds);
        let between = |(a, b): (f64, f64)| a + (b - a) * t;
        let scripted = Framing {
            yaw: between(look.yaw).to_radians() + film.offset.yaw,
            pitch: (between(look.pitch).to_radians() + film.offset.pitch).clamp(-HIGHEST, HIGHEST),
            ln_distance: between((look.distance.0.ln(), look.distance.1.ln()))
                + film.offset.ln_distance,
        };
        let (target, framing) = match film.cut {
            Some((from, framing)) => {
                let t = smooth(film.reel.into / CUT);
                (from.lerp(center, t), framing.toward(scripted, t))
            }
            None => (center, scripted),
        };
        self.camera.target = target;
        framing.apply(&mut self.camera);
        film.set = Some(Framing::of(&self.camera));
    }

    /// Runs the film's last line, and says what it did.
    fn end_film(&mut self) {
        let Some(film) = &self.film else {
            return;
        };
        let (title, end, note) = (film.title, film.script.end, film.script.end_note);
        self.restore_view();
        end(self);
        self.status = Some((
            format!("{title}: the film has ended, and {note}. The scenario plays on."),
            Instant::now(),
        ));
    }

    /// Puts the view's grid and trails back as they were before the film.
    fn restore_view(&mut self) {
        if let Some(film) = &self.film {
            self.options.grid = film.grid;
            self.options.trails = film.trails;
        }
    }

    /// Goes to shot `k`'s start (past the last, the end).
    pub(super) fn film_go_to(&mut self, k: usize) {
        let Some(film) = &mut self.film else {
            return;
        };
        let lengths = film.script.lengths();
        let was_over = film.reel.ended;
        match film.reel.go_to(&lengths, k) {
            Some(k) => self.begin_shot(k),
            None if !was_over => self.end_film(),
            None => {}
        }
    }

    /// The next shot.
    pub(super) fn film_next(&mut self) {
        if let Some(film) = &self.film
            && !film.reel.ended
        {
            self.film_go_to(film.reel.shot + 1);
        }
    }

    /// Back a shot (or to this one's start).
    pub(super) fn film_back(&mut self) {
        let Some(film) = &mut self.film else {
            return;
        };
        let lengths = film.script.lengths();
        if let Some(k) = film.reel.back(&lengths) {
            self.begin_shot(k);
        }
    }

    /// Stops the film directing: the camera is the viewer's, and the
    /// scenario plays on.
    pub(super) fn leave_film(&mut self) {
        let playing = self.film_playing();
        if playing {
            self.restore_view();
        }
        if self.film.take().is_some() && playing {
            self.status = Some((
                "Left the film: the camera is yours, and the scenario plays on".to_string(),
                Instant::now(),
            ));
        }
    }

    /// The film over the view: the orbits it draws, its cards, its caption
    /// and its bar.
    pub(super) fn draw_film(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        viewport: Rect,
    ) {
        let Some(film) = &self.film else {
            return;
        };
        if !film.reel.ended {
            let shot = &film.script.shots[film.reel.shot];
            let into = film.reel.into;
            if shot.orbits {
                self.draw_orbits(
                    painter,
                    viewport,
                    shot.look.at,
                    fade(into, shot.seconds, 0.0),
                );
            }
            let cards = fade(into, shot.seconds, FADE);
            if cards > 0.0 {
                self.draw_cards(painter, viewport, shot.cards, cards);
            }
            let label = format!(
                "{} · {} of {}: {}",
                film.title,
                film.reel.shot + 1,
                film.script.shots.len(),
                shot.name
            );
            draw_caption(
                painter,
                viewport,
                &label,
                shot.caption,
                fade(into, shot.seconds, 0.0),
            );
            // Out of black at the start.
            let elapsed = film.reel.elapsed(&film.script.lengths());
            if elapsed < 2.0 * FADE {
                let dark = 1.0 - (elapsed / (2.0 * FADE)) as f32;
                painter.rect_filled(viewport, 0.0, Color32::BLACK.gamma_multiply(dark));
            }
        }
        let asked = film_bar(ctx, viewport, film);
        match asked {
            Some(Asked::Back) => self.film_back(),
            Some(Asked::PlayPause) => self.play_pause(),
            Some(Asked::Next) => self.film_next(),
            Some(Asked::GoTo(k)) => self.film_go_to(k),
            Some(Asked::Again) => {
                if self.simulation.paused {
                    self.play_pause();
                }
                self.options.grid = false;
                self.film_go_to(0);
            }
            Some(Asked::Leave) => self.leave_film(),
            None => {}
        }
    }

    /// The orbits of the pair `at`, as they are this moment (osculating):
    /// each body's ellipse around their center of mass, and the line to
    /// the first's closest approach. Watched as it turns, the line where it
    /// was when watching began too, and how far it has turned.
    fn draw_orbits(&self, painter: &egui::Painter, viewport: Rect, at: &[&str], fade: f32) {
        let sim = &self.simulation;
        let [a, b] = at else {
            return;
        };
        let (Some(i), Some(j)) = (sim.index_of(a), sim.index_of(b)) else {
            return;
        };
        let (p, q) = (&sim.bodies[i], &sim.bodies[j]);
        let mu = p.gm + q.gm;
        let (r, v) = (p.position - q.position, p.velocity - q.velocity);
        let Some(orbit) = osculating_orbit(r, v, mu, 240) else {
            return;
        };
        let center = (p.position * p.gm + q.position * q.gm) / mu;
        let screen = |x: DVec3| self.camera.project(x, viewport).map(|s| s.position);
        // Each body's share of their separation: the other's mass over
        // theirs.
        let share = q.gm / mu;
        for (scale, index) in [(share, i), (share - 1.0, j)] {
            let points: Vec<_> = orbit
                .iter()
                .filter_map(|&o| screen(center + o * scale))
                .collect();
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(1.4, view::color(sim, index).gamma_multiply(0.75 * fade)),
            ));
        }
        // The first body's long axis: from its farthest point through
        // their center of mass to its closest approach.
        let lrl = periapsis_vector(r, v, mu);
        let e = lrl.length() / mu;
        let a_rel = 1.0 / (2.0 / r.length() - v.length_squared() / mu);
        let axis = |toward: DVec3| {
            let toward = toward.normalize();
            let near = screen(center + toward * (a_rel * (1.0 - e) * share))?;
            let far = screen(center - toward * (a_rel * (1.0 + e) * share))?;
            Some((near, far))
        };
        let Some((near, far)) = axis(lrl) else {
            return;
        };
        // Labelled only when the orbit is big enough on screen to take it.
        let labelled = (near - far).length() > 140.0;
        let font = FontId::proportional(11.5);
        let label = |end: egui::Pos2, from: egui::Pos2, text: String, color: Color32| {
            let out = (end - from).normalized();
            let align = if out.x >= 0.0 {
                Align2::LEFT_CENTER
            } else {
                Align2::RIGHT_CENTER
            };
            painter.text(end + out * 8.0, align, text, font.clone(), color);
        };
        let accent = theme::ACCENT.gamma_multiply(fade);
        painter.line_segment([far, near], egui::Stroke::new(1.6, accent));
        painter.circle_filled(near, 3.0, accent);
        if labelled {
            label(near, far, "closest approach".to_string(), accent);
        }
        if let Some(watch) = self.watch.as_ref().filter(|w| w.body == *a)
            && let Some((then_near, then_far)) = axis(watch.first_direction())
        {
            // Now against then, once both are drawn.
            if labelled {
                label(far, near, "its long axis now".to_string(), accent);
            }
            let muted = theme::MUTED.gamma_multiply(fade);
            painter.add(egui::Shape::dashed_line(
                &[then_far, then_near],
                egui::Stroke::new(1.2, muted),
                5.0,
                4.0,
            ));
            if labelled {
                label(
                    then_far,
                    then_near,
                    format!(
                        "{:.1} years ago: turned {:.1}° since",
                        watch.watched_for() / JULIAN_YEAR,
                        watch.turned().to_degrees()
                    ),
                    muted,
                );
            }
        }
    }

    /// The shot's cards, in columns down the view's left and right sides,
    /// each pinned to a body joined to it by a line.
    fn draw_cards(&self, painter: &egui::Painter, viewport: Rect, cards: &[Card], fade: f32) {
        const WIDTH: f32 = 272.0;
        const PAD: f32 = 12.0;
        let inner = WIDTH - 2.0 * PAD;
        let mut left = viewport.top() + 16.0;
        let mut right = viewport.top() + 16.0;
        for card in cards {
            let title = painter.layout(
                card.title.to_string(),
                theme::strong(13.5),
                theme::TEXT.gamma_multiply(fade),
                inner,
            );
            let mut lines = Vec::new();
            let mut plots = Vec::new();
            for line in card.lines {
                match line {
                    Line::Text(text) => lines.push(text.to_string()),
                    Line::Live(live) => lines.extend(live(self)),
                    Line::Plot(plot) => plots.extend(plot(self)),
                }
            }
            let lines: Vec<_> = lines
                .into_iter()
                .map(|text| {
                    painter.layout(
                        text,
                        FontId::proportional(12.5),
                        theme::TEXT.gamma_multiply(0.92 * fade),
                        inner,
                    )
                })
                .collect();
            let source = (!card.source.is_empty()).then(|| {
                painter.layout(
                    card.source.to_string(),
                    FontId::proportional(10.5),
                    theme::MUTED.gamma_multiply(fade),
                    inner,
                )
            });
            const PLOT: f32 = 120.0;
            let height = PAD
                + title.size().y
                + 6.0
                + lines.iter().map(|g| g.size().y + 4.0).sum::<f32>()
                + plots.len() as f32 * (PLOT + 8.0)
                + source.as_ref().map_or(0.0, |g| g.size().y + 6.0)
                + PAD;
            let (x, column) = match card.at {
                Pinned::Left | Pinned::LeftOf(_) => (viewport.left() + 16.0, &mut left),
                Pinned::RightOf(_) => (viewport.right() - 16.0 - WIDTH, &mut right),
            };
            let rect = Rect::from_min_size(pos2(x, *column), vec2(WIDTH, height));
            *column += height + 10.0;
            // The line to its body, under the card.
            let body = match card.at {
                Pinned::LeftOf(name) | Pinned::RightOf(name) => self.simulation.index_of(name),
                Pinned::Left => None,
            };
            if let Some(projection) = body.and_then(|i| {
                self.camera
                    .project(self.simulation.bodies[i].position, viewport)
            }) && viewport.contains(projection.position)
            {
                let at = projection.position;
                let edge = if matches!(card.at, Pinned::LeftOf(_)) {
                    pos2(rect.right(), rect.top() + 22.0)
                } else {
                    pos2(rect.left(), rect.top() + 22.0)
                };
                let stroke = egui::Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.55 * fade));
                painter.line_segment([edge, at], stroke);
                painter.circle_stroke(at, 7.0, stroke);
            }
            painter.rect(
                rect,
                12.0,
                theme::SURFACE.gamma_multiply(0.9 * fade),
                egui::Stroke::new(1.0, theme::BORDER.gamma_multiply(fade)),
                egui::StrokeKind::Inside,
            );
            let mut y = rect.top() + PAD;
            let title_height = title.size().y;
            painter.galley(pos2(x + PAD, y), title, theme::TEXT);
            y += title_height + 6.0;
            for galley in lines {
                let height = galley.size().y;
                painter.galley(pos2(x + PAD, y), galley, theme::TEXT);
                y += height + 4.0;
            }
            for plot in &plots {
                let area = Rect::from_min_size(pos2(x + PAD, y + 4.0), vec2(inner, PLOT));
                draw_plot(painter, area, plot, fade);
                y += PLOT + 8.0;
            }
            if let Some(galley) = source {
                painter.galley(pos2(x + PAD, y + 2.0), galley, theme::MUTED);
            }
        }
    }
}

/// A plot on a card: its axes from zero, the curve, and its marked point.
fn draw_plot(painter: &egui::Painter, area: Rect, plot: &Plot, fade: f32) {
    let font = FontId::proportional(10.0);
    let muted = theme::MUTED.gamma_multiply(fade);
    // Room for the axes' labels below and to the left.
    let frame = Rect::from_min_max(area.min + vec2(30.0, 4.0), area.max - vec2(4.0, 26.0));
    let at = |(x, y): (f64, f64)| {
        pos2(
            frame.left() + (x / plot.x.0) as f32 * frame.width(),
            frame.bottom() - (y / plot.y.0) as f32 * frame.height(),
        )
    };
    let axis = egui::Stroke::new(1.0, muted);
    painter.line_segment([frame.left_bottom(), frame.right_bottom()], axis);
    painter.line_segment([frame.left_bottom(), frame.left_top()], axis);
    for k in 0..=4 {
        let (x, y) = (plot.x.0 * f64::from(k) / 4.0, plot.y.0 * f64::from(k) / 4.0);
        painter.text(
            at((x, 0.0)) + vec2(0.0, 3.0),
            Align2::CENTER_TOP,
            format!("{x:.0}"),
            font.clone(),
            muted,
        );
        painter.text(
            at((0.0, y)) - vec2(4.0, 0.0),
            Align2::RIGHT_CENTER,
            format!("{y:.0}"),
            font.clone(),
            muted,
        );
    }
    painter.text(
        pos2(frame.center().x, area.bottom()),
        Align2::CENTER_BOTTOM,
        plot.x.1,
        font.clone(),
        muted,
    );
    painter.text(
        frame.left_top() + vec2(6.0, 0.0),
        Align2::LEFT_TOP,
        plot.y.1,
        font.clone(),
        muted,
    );
    let points: Vec<_> = plot.points.iter().map(|&p| at(p)).collect();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(2.0, theme::ACCENT.gamma_multiply(fade)),
    ));
    if let Some((x, y, what)) = &plot.mark {
        let p = at((*x, *y));
        painter.circle_filled(p, 3.5, theme::TEXT.gamma_multiply(fade));
        painter.text(
            p - vec2(6.0, 4.0),
            Align2::RIGHT_BOTTOM,
            what,
            font,
            theme::TEXT.gamma_multiply(fade),
        );
    }
}

/// The caption at the bottom of the view, over the film's bar, with the
/// film and shot named above it.
fn draw_caption(painter: &egui::Painter, viewport: Rect, label: &str, text: &str, fade: f32) {
    if fade <= 0.0 {
        return;
    }
    let width = (viewport.width() - 64.0).clamp(240.0, 760.0);
    let label = painter.layout(
        label.to_string(),
        theme::strong(11.5),
        theme::ACCENT.gamma_multiply(fade),
        width,
    );
    let text = painter.layout(
        text.to_string(),
        FontId::proportional(16.5),
        theme::TEXT.gamma_multiply(fade),
        width,
    );
    let inner = vec2(
        label.size().x.max(text.size().x),
        label.size().y + 6.0 + text.size().y,
    );
    let size = inner + vec2(36.0, 24.0);
    let rect = Rect::from_center_size(
        pos2(
            viewport.center().x,
            viewport.bottom() - BAR_ROOM - size.y / 2.0,
        ),
        size,
    );
    painter.rect_filled(
        rect,
        12.0,
        Color32::from_rgba_unmultiplied(9, 11, 16, 205).gamma_multiply(fade),
    );
    let top = rect.min + vec2(18.0, 12.0);
    let label_height = label.size().y;
    painter.galley(top, label, theme::ACCENT);
    painter.galley(top + vec2(0.0, label_height + 6.0), text, theme::TEXT);
}

/// Room left under a caption for the film's bar, points.
const BAR_ROOM: f32 = 66.0;

/// The film's bar, at the bottom of the view: back, play or pause, next,
/// its shots (one to a segment, as long as the shot; a click goes to it),
/// and leaving it. Over, it offers to play it again.
fn film_bar(ctx: &egui::Context, viewport: Rect, film: &Film) -> Option<Asked> {
    let mut asked = None;
    egui::Area::new(egui::Id::new("film bar"))
        .fixed_pos(pos2(viewport.center().x, viewport.bottom() - 12.0))
        .pivot(Align2::CENTER_BOTTOM)
        .order(egui::Order::Middle)
        .show(ctx, |ui| {
            egui::Frame::new()
                .fill(theme::SURFACE.gamma_multiply(0.94))
                .stroke(egui::Stroke::new(1.0, theme::BORDER))
                .corner_radius(12)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if film.reel.ended {
                            ui.label(RichText::new(format!("{}: the end", film.title)).strong());
                            if ui
                                .add(theme::primary_button("Watch again"))
                                .on_hover_text("Plays the film again from its first shot")
                                .clicked()
                            {
                                asked = Some(Asked::Again);
                            }
                        } else {
                            if ui
                                .button("⏮")
                                .on_hover_text("Back to this shot's start, or the one before (←)")
                                .clicked()
                            {
                                asked = Some(Asked::Back);
                            }
                            let (icon, what) = if film.reel.paused {
                                ("▶", "Play the film, and time (Space)")
                            } else {
                                ("⏸", "Pause the film, and time (Space)")
                            };
                            if ui.button(icon).on_hover_text(what).clicked() {
                                asked = Some(Asked::PlayPause);
                            }
                            if ui.button("⏭").on_hover_text("The next shot (→)").clicked() {
                                asked = Some(Asked::Next);
                            }
                            if let Some(k) = shots(ui, film) {
                                asked = Some(Asked::GoTo(k));
                            }
                            let lengths = film.script.lengths();
                            ui.label(
                                RichText::new(format!(
                                    "{} / {}",
                                    minutes(film.reel.elapsed(&lengths)),
                                    minutes(lengths.iter().sum())
                                ))
                                .monospace()
                                .color(theme::MUTED),
                            );
                        }
                        if ui
                            .button("Leave")
                            .on_hover_text(
                                "Leave the film: the camera is yours, and the scenario plays on",
                            )
                            .clicked()
                        {
                            asked = Some(Asked::Leave);
                        }
                    });
                });
        });
    asked
}

/// Seconds as minutes and seconds: "1:05".
fn minutes(seconds: f64) -> String {
    let whole = seconds.floor() as u64;
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// The film's shots, one to a segment as long as the shot, filled as far
/// as it has played. Returns the shot clicked, if one was.
fn shots(ui: &mut egui::Ui, film: &Film) -> Option<usize> {
    let lengths = film.script.lengths();
    let total: f64 = lengths.iter().sum();
    let (rect, response) = ui.allocate_exact_size(vec2(300.0, 20.0), Sense::click());
    let painter = ui.painter();
    let played = film.reel.elapsed(&lengths) / total;
    let mut start = 0.0;
    let mut hovered = None;
    for (k, &length) in lengths.iter().enumerate() {
        let (a, b) = (start / total, (start + length) / total);
        start += length;
        let x = |f: f64| rect.left() + f as f32 * rect.width();
        let segment = Rect::from_min_max(
            pos2(x(a) + 1.0, rect.center().y - 3.0),
            pos2(x(b) - 1.0, rect.center().y + 3.0),
        );
        painter.rect_filled(segment, 3.0, theme::RAISED);
        let filled = ((played - a) / (b - a)).clamp(0.0, 1.0) as f32;
        if filled > 0.0 {
            let mut done = segment;
            done.set_right(segment.left() + filled * segment.width());
            painter.rect_filled(done, 3.0, theme::ACCENT);
        }
        if let Some(pointer) = response.hover_pos()
            && (x(a)..x(b)).contains(&pointer.x)
        {
            hovered = Some(k);
        }
    }
    if let Some(k) = hovered {
        response
            .clone()
            .on_hover_text(format!("{}: {}", k + 1, film.script.shots[k].name));
    }
    hovered.filter(|_| response.clicked())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LENGTHS: [f64; 3] = [10.0, 5.0, 8.0];

    #[test]
    fn a_reel_plays_through_its_cuts_to_its_end() {
        let mut reel = Reel::start();
        assert_eq!(reel.advance(&LENGTHS, 4.0), None);
        assert_eq!((reel.shot, reel.into), (0, 4.0));
        // One step across a cut lands in the next shot, as far in as it ran
        // past.
        assert_eq!(reel.advance(&LENGTHS, 7.0), Some(1));
        assert_eq!((reel.shot, reel.into), (1, 1.0));
        // A long one crosses two cuts: the shot it lands in is the one
        // begun.
        assert_eq!(reel.advance(&LENGTHS, 6.0), Some(2));
        assert_eq!((reel.shot, reel.into), (2, 2.0));
        assert_eq!(reel.elapsed(&LENGTHS), 17.0);
        // Past the last shot's end, it ends there, and stays.
        assert_eq!(reel.advance(&LENGTHS, 100.0), None);
        assert!(reel.ended);
        assert_eq!(reel.elapsed(&LENGTHS), 23.0);
        assert_eq!(reel.advance(&LENGTHS, 1.0), None);
        assert_eq!(reel.elapsed(&LENGTHS), 23.0);
    }

    #[test]
    fn paused_it_stays_put() {
        let mut reel = Reel::start();
        reel.paused = true;
        assert_eq!(reel.advance(&LENGTHS, 30.0), None);
        assert_eq!(reel.elapsed(&LENGTHS), 0.0);
    }

    #[test]
    fn skipping_goes_to_a_shots_start_and_back_as_a_player_does() {
        let mut reel = Reel::start();
        assert_eq!(reel.go_to(&LENGTHS, 2), Some(2));
        assert_eq!((reel.shot, reel.into), (2, 0.0));
        // Moments into a shot, back goes to the one before.
        reel.advance(&LENGTHS, 1.0);
        assert_eq!(reel.back(&LENGTHS), Some(1));
        // Later into it, to its own start.
        reel.advance(&LENGTHS, 3.0);
        assert_eq!(reel.back(&LENGTHS), Some(1));
        assert_eq!((reel.shot, reel.into), (1, 0.0));
        // Back from the first stays at the first.
        reel.go_to(&LENGTHS, 0);
        assert_eq!(reel.back(&LENGTHS), Some(0));
        // Past the last shot is the end; back from there, the last shot's
        // start.
        assert_eq!(reel.go_to(&LENGTHS, 3), None);
        assert!(reel.ended);
        assert_eq!(reel.back(&LENGTHS), Some(2));
        assert!(!reel.ended);
    }

    #[test]
    fn a_cut_turns_the_shorter_way_round() {
        let from = Framing {
            yaw: 350f64.to_radians(),
            pitch: 0.0,
            ln_distance: 0.0,
        };
        let to = Framing {
            yaw: 10f64.to_radians(),
            pitch: 0.5,
            ln_distance: 2.0,
        };
        let half = from.toward(to, 0.5);
        assert!((half.yaw.to_degrees() - 360.0).abs() < 1e-9);
        assert_eq!((half.pitch, half.ln_distance), (0.25, 1.0));
        let all = from.toward(to, 1.0);
        assert!((all.yaw.to_degrees() - 370.0).abs() < 1e-9);
    }

    #[test]
    fn a_players_time() {
        assert_eq!(minutes(0.0), "0:00");
        assert_eq!(minutes(65.9), "1:05");
        assert_eq!(minutes(116.0), "1:56");
    }

    #[test]
    fn captions_and_cards_fade_in_and_out() {
        assert_eq!(fade(0.0, 10.0, 0.0), 0.0);
        assert_eq!(fade(FADE, 10.0, 0.0), 1.0);
        assert_eq!(fade(5.0, 10.0, FADE), 1.0);
        assert_eq!(fade(10.0, 10.0, 0.0), 0.0);
        assert!(fade(FADE, 10.0, FADE) == 0.0);
    }
}
