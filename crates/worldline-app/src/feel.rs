//! How the app responds (step 8.3): the camera glides on a moment after a
//! drag is let go, keyboard shortcuts for what's done most, and how long
//! each frame takes.

use eframe::egui::{self, Key, Modifiers, Vec2};

/// The camera's turning after a drag: it carries on at the drag's speed,
/// slowing exponentially, so a flick of the pointer spins the view and a
/// drag held still stops it dead.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Spin {
    /// Points a second, as the drag moved the pointer.
    velocity: Vec2,
}

impl Spin {
    /// How quickly it slows: by e every this many seconds. A flick then
    /// carries on for about half a second, and as far as the pointer moved
    /// in this long.
    pub const SETTLE: f32 = 0.16;

    /// Below this speed, points a second, it stops.
    const REST: f32 = 4.0;

    /// The shortest frame a drag's speed is measured over, s: a frame that
    /// comes back at once mustn't read as a fast drag.
    const SHORTEST_FRAME: f32 = 1.0 / 240.0;

    /// While dragging, by `delta` points over `dt` seconds: the speed to
    /// carry on at, mostly this frame's, so holding still before letting go
    /// leaves it at rest.
    pub fn dragged(&mut self, delta: Vec2, dt: f32) {
        let velocity = delta / dt.max(Self::SHORTEST_FRAME);
        self.velocity = self.velocity * 0.4 + velocity * 0.6;
    }

    /// Not dragging: how far to turn the camera this frame, in points as
    /// a drag would, or `None` once it has stopped. Exactly what a speed
    /// falling as e^(−t/SETTLE) covers in `dt`.
    pub fn coast(&mut self, dt: f32) -> Option<Vec2> {
        if self.velocity.length() < Self::REST {
            self.velocity = Vec2::ZERO;
            return None;
        }
        let decay = (-dt / Self::SETTLE).exp();
        let turned = self.velocity * Self::SETTLE * (1.0 - decay);
        self.velocity *= decay;
        Some(turned)
    }

    /// Stops it: something else took the camera (a click, a flight).
    pub fn stop(&mut self) {
        self.velocity = Vec2::ZERO;
    }

    /// Whether it's still turning, so frames must keep coming.
    pub fn moving(&self) -> bool {
        self.velocity.length() >= Self::REST
    }
}

/// What a keyboard shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Space: time runs or stops.
    PlayPause,
    /// + or ]: the next speed up.
    Faster,
    /// − or [: the next speed down.
    Slower,
    /// F: the camera flies to the selected body and follows it.
    FollowSelected,
    /// Esc: stop adding bodies, or close the open windows.
    Escape,
    /// Ctrl+S: save the sandbox.
    Save,
    /// F3: show or hide the frame's timings.
    Timings,
    /// →: a film's next shot.
    NextShot,
    /// ←: back a shot.
    PreviousShot,
    /// F1: open or close the manual.
    Manual,
}

impl Action {
    /// Every shortcut, in the order the manual and the ☰ menu list them.
    pub const ALL: [Action; 10] = [
        Action::PlayPause,
        Action::Faster,
        Action::Slower,
        Action::FollowSelected,
        Action::Escape,
        Action::Save,
        Action::Timings,
        Action::Manual,
        Action::NextShot,
        Action::PreviousShot,
    ];

    /// The key that does it, as the interface writes it.
    pub fn key(self) -> &'static str {
        match self {
            Action::PlayPause => "Space",
            Action::Faster => "+",
            Action::Slower => "−",
            Action::FollowSelected => "F",
            Action::Escape => "Esc",
            Action::Save => "Ctrl+S",
            Action::Timings => "F3",
            Action::NextShot => "→",
            Action::PreviousShot => "←",
            Action::Manual => "F1",
        }
    }

    /// What it does, in a few words.
    pub fn what(self) -> &'static str {
        match self {
            Action::PlayPause => "run or stop time",
            Action::Faster => "faster",
            Action::Slower => "slower",
            Action::FollowSelected => "follow the selected body",
            Action::Escape => "close the windows",
            Action::Save => "save the sandbox",
            Action::Timings => "the frame's timings",
            Action::Manual => "the manual",
            Action::NextShot => "a film's next shot",
            Action::PreviousShot => "back a shot",
        }
    }
}

/// The action a key press does, if any.
pub fn action(key: Key, modifiers: Modifiers) -> Option<Action> {
    let plain = !(modifiers.ctrl || modifiers.command || modifiers.alt);
    match key {
        Key::S if modifiers.command && !modifiers.alt => Some(Action::Save),
        _ if !plain => None,
        Key::Space => Some(Action::PlayPause),
        Key::Plus | Key::Equals | Key::CloseBracket => Some(Action::Faster),
        Key::Minus | Key::OpenBracket => Some(Action::Slower),
        Key::F => Some(Action::FollowSelected),
        Key::Escape => Some(Action::Escape),
        Key::F3 => Some(Action::Timings),
        Key::F1 => Some(Action::Manual),
        Key::ArrowRight => Some(Action::NextShot),
        Key::ArrowLeft => Some(Action::PreviousShot),
        _ => None,
    }
}

/// The shortcuts pressed this frame, unless text is being typed.
pub fn pressed(ctx: &egui::Context) -> Vec<Action> {
    if ctx.egui_wants_keyboard_input() {
        return Vec::new();
    }
    ctx.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| match event {
                egui::Event::Key {
                    key,
                    pressed: true,
                    repeat: false,
                    modifiers,
                    ..
                } => action(*key, *modifiers),
                _ => None,
            })
            .collect()
    })
}

/// How long frames take, as running means, s: from one frame to the next,
/// the physics within it, and the rest of the computer's work on it (the
/// interface and handing the GPU its work).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Timings {
    pub frame: f64,
    pub physics: f64,
    pub interface: f64,
    /// Of the interface, drawing the view (step 8.4).
    pub view: f64,
    /// Of the physics: steps of the solar system a frame, their time, the
    /// galactic center's catching up, carrying the belts to now; and the
    /// longest step lately (step 8.4).
    pub steps: f64,
    pub stepping: f64,
    pub galaxy: f64,
    pub belts: f64,
    pub longest_step: f64,
}

impl Timings {
    /// Each frame counts this much toward the means: about the last
    /// quarter second's worth at 60 frames a second.
    const WEIGHT: f64 = 0.07;

    /// Adds a frame's times.
    pub fn record(&mut self, frame: f64, physics: f64, interface: f64) {
        if self.frame == 0.0 {
            *self = Timings {
                frame,
                physics,
                interface,
                ..*self
            };
            return;
        }
        self.frame += Self::WEIGHT * (frame - self.frame);
        self.physics += Self::WEIGHT * (physics - self.physics);
        self.interface += Self::WEIGHT * (interface - self.interface);
    }

    /// Adds the parts of a frame's physics and the view's drawing: steps
    /// taken, and seconds.
    pub fn record_parts(
        &mut self,
        steps: usize,
        stepping: f64,
        galaxy: f64,
        belts: f64,
        view: f64,
    ) {
        let w = Self::WEIGHT;
        self.steps += w * (steps as f64 - self.steps);
        self.stepping += w * (stepping - self.stepping);
        self.galaxy += w * (galaxy - self.galaxy);
        self.belts += w * (belts - self.belts);
        self.view += w * (view - self.view);
    }

    /// Notes a step's time, keeping the longest of the last few seconds:
    /// it forgets by 2% a frame.
    pub fn record_step(&mut self, longest: f64) {
        self.longest_step = longest.max(0.98 * self.longest_step);
    }

    /// Frames a second.
    pub fn rate(&self) -> f64 {
        if self.frame > 0.0 {
            1.0 / self.frame
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_flick_glides_as_far_as_its_speed_times_the_settling_time() {
        // A drag at 600 points a second, then let go: it carries on for
        // 600 × 0.16 = 96 points in all, as an exponential slowing does,
        // however the frames fall.
        for dt in [1.0 / 30.0, 1.0 / 60.0, 1.0 / 144.0] {
            let mut spin = Spin::default();
            for _ in 0..10 {
                spin.dragged(Vec2::new(600.0 * dt, 0.0), dt);
            }
            let mut glided = 0.0;
            let mut frames = 0;
            while let Some(turn) = spin.coast(dt) {
                glided += turn.x;
                frames += 1;
            }
            let seconds = frames as f32 * dt;
            // It stops once under 4 points a second, which leaves out
            // 4 × 0.16 = 0.64 points.
            assert!((glided - 96.0).abs() < 1.0, "{glided} at {dt}");
            assert!(seconds < 1.0, "glided for {seconds} s");
            assert!(!spin.moving());
        }
    }

    #[test]
    fn holding_still_before_letting_go_leaves_it_at_rest() {
        let mut spin = Spin::default();
        let dt = 1.0 / 60.0;
        for _ in 0..10 {
            spin.dragged(Vec2::new(10.0, 0.0), dt);
        }
        // Six frames held still.
        for _ in 0..6 {
            spin.dragged(Vec2::ZERO, dt);
        }
        assert!(spin.coast(dt).is_none());
    }

    #[test]
    fn shortcuts_need_no_modifiers_but_save() {
        let none = Modifiers::NONE;
        assert_eq!(action(Key::Space, none), Some(Action::PlayPause));
        assert_eq!(action(Key::Plus, none), Some(Action::Faster));
        assert_eq!(action(Key::Minus, none), Some(Action::Slower));
        assert_eq!(action(Key::F, none), Some(Action::FollowSelected));
        assert_eq!(action(Key::Escape, none), Some(Action::Escape));
        assert_eq!(action(Key::ArrowRight, none), Some(Action::NextShot));
        assert_eq!(action(Key::ArrowLeft, none), Some(Action::PreviousShot));
        assert_eq!(action(Key::F1, none), Some(Action::Manual));
        // ALL lists every shortcut once: each one's key, as written, is
        // different.
        let mut keys: Vec<&str> = Action::ALL.iter().map(|a| a.key()).collect();
        keys.dedup();
        assert_eq!(keys.len(), Action::ALL.len());
        for a in Action::ALL {
            // Exhaustive: a new shortcut must be added to ALL.
            match a {
                Action::PlayPause
                | Action::Faster
                | Action::Slower
                | Action::FollowSelected
                | Action::Escape
                | Action::Save
                | Action::Timings
                | Action::Manual
                | Action::NextShot
                | Action::PreviousShot => {}
            }
        }
        assert_eq!(action(Key::S, Modifiers::COMMAND), Some(Action::Save));
        assert_eq!(action(Key::S, none), None);
        // Ctrl with another key is the system's or another program's.
        assert_eq!(action(Key::F, Modifiers::COMMAND), None);
        assert_eq!(action(Key::Space, Modifiers::ALT), None);
    }

    #[test]
    fn timings_average_over_the_last_frames() {
        let mut timings = Timings::default();
        timings.record(0.05, 0.01, 0.02);
        assert_eq!(timings.rate(), 20.0);
        for _ in 0..200 {
            timings.record(1.0 / 60.0, 0.012, 0.003);
        }
        assert!((timings.rate() - 60.0).abs() < 0.01);
        assert!((timings.physics - 0.012).abs() < 1e-6);
    }
}
