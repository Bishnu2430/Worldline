//! The Worldline window: the 3D view plus its control panels.

use std::f64::consts::{FRAC_PI_2, PI};
use std::time::{Duration, Instant};

use eframe::egui::collapsing_header::CollapsingState;
use eframe::egui::{self, Align2, Color32, FontId, Rect, RichText, Sense, TextureId, pos2, vec2};
use eframe::egui_wgpu::RenderState;
use glam::Mat3;
use worldline_core::constants::{AU, C, DAY, GM_SUN, JULIAN_YEAR, SOLAR_LUMINOSITY};
use worldline_core::eclipses::{Eclipse, LunarKind, SolarKind, State, apparent};
use worldline_core::gravity::kerr::innermost_stable_orbit;
use worldline_core::hawking;
use worldline_core::kerr_light::{self, circular, disk_temperature, photon_orbits};
use worldline_core::lensing::BendTable;
use worldline_core::magnetosphere::standoff;
use worldline_core::merger::ringdown;
use worldline_core::milky_way::KILOPARSEC;
use worldline_core::neutron_star::{sly_maximum, sly_radius};
use worldline_core::shadow_on_earth::{Path, central_point};
use worldline_core::sunlight::{irradiance, light_time};
use worldline_core::white_dwarf::{CARBON_OXYGEN, chandrasekhar_limit, radius_for};
use worldline_core::{Body, DVec3};
use worldline_render::{Atmosphere, FluidDraw, KerrDisk, KerrLens, Lens, Rings, SkyView, View};

use crate::calendar::{DateTime, to_the_second};
use crate::camera::Camera;
use crate::catalogue::{self, Entry, Group, length, light_years, measured_suns, significant};
use crate::details;
use crate::eclipse_finder::{self, Finder};
use crate::fluid::{FluidBody, Lab};
use crate::gpu::{Globe, GpuGlobes, GpuSky};
use crate::overlays::{self, Overlays};
use crate::sandbox::{self, Launch};
use crate::simulation::Simulation;
use crate::simulation::{Approach, Computed, Limits, Removal};
use crate::theme;
use crate::view::{self, OnScreen, ViewOptions};
use crate::waves;
use worldline_core::compact::{gravitational_redshift, horizon_spin, is_black_hole};
use worldline_core::regime::Validity;
use worldline_core::tides::{
    FLUID_STRETCH, POINT_STRETCH, distance_for, giant_planet, hill_radius, polytrope_love_number,
    stretch, tidal_radius,
};
use worldline_data::{Distance, ObjectKind, RadiusBasis, SmallBodyKind};
use worldline_render::TidalBody;

/// Simulation speeds the user can pick: simulated time per real second.
const SPEEDS: [(&str, f64); 8] = [
    ("Real time", 1.0),
    ("1 hour per second", 3600.0),
    ("1 day per second", DAY),
    ("1 week per second", 7.0 * DAY),
    ("1 month per second", 30.0 * DAY),
    ("1 year per second", JULIAN_YEAR),
    ("10 years per second", 10.0 * JULIAN_YEAR),
    ("100 years per second", 100.0 * JULIAN_YEAR),
];
const DEFAULT_SPEED: usize = 4;
const REAL_TIME: usize = 0;
const HOUR: usize = 1;

/// Most computer time physics may take per frame while catching up to the
/// present: the 2025 snapshot is 7 s of physics behind late 2026, so at
/// 20 frames a second it takes about as long.
const CATCH_UP_BUDGET: Duration = Duration::from_millis(50);

/// Most computer time physics may take per frame. If a high speed needs
/// more, the simulation runs slower than requested and says so.
pub(crate) const PHYSICS_BUDGET: Duration = Duration::from_millis(12);

/// How long after starting the app waits before finding the GPU's particle
/// budget.
const STARTUP: Duration = Duration::from_secs(2);

/// Earth's GM (JPL DE440), for showing masses in Earth masses.
pub const GM_EARTH: f64 = 3.986_004_355_070_227e14;

/// A label in an inspector grid: what the value on its right is.
fn key(text: &str) -> RichText {
    RichText::new(text).color(theme::MUTED)
}

/// A fraction as a percentage, to two significant figures: "0.42%".
fn percent(fraction: f64) -> String {
    let value = 100.0 * fraction;
    if value >= 10.0 {
        format!("{value:.0}%")
    } else if value >= 1e-3 {
        let decimals = (1.0 - value.log10().floor()) as usize;
        format!("{value:.decimals$}%")
    } else {
        format!("{value:.1e}%")
    }
}

/// A count with its thousands set apart: "16,384".
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A duration (s) for display: seconds; minutes and seconds; or hours and
/// minutes.
fn duration(seconds: f64) -> String {
    if seconds < 120.0 {
        format!("{seconds:.1} s")
    } else if seconds < 7200.0 {
        format!(
            "{:.0} min {:.1} s",
            (seconds / 60.0).floor(),
            seconds % 60.0
        )
    } else {
        let minutes = (seconds / 60.0).round();
        format!(
            "{:.0} h {:.0} min",
            (minutes / 60.0).floor(),
            minutes % 60.0
        )
    }
}

/// Draws debris' fallback rate against time since the closest approach,
/// both on log scales, with a line ∝ t^(−5/3) through the late rates.
fn fallback_plot(ui: &mut egui::Ui, debris: &crate::fluid::Debris) {
    let rates = debris.rates();
    if rates.len() < 2 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
    // In days, and shares of its mass a day.
    let points: Vec<(f64, f64)> = rates
        .iter()
        .map(|&(t, rate)| ((t / DAY).log10(), (rate * DAY).log10()))
        .collect();
    let (x0, x1) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.0), b.max(p.0))
        });
    let (y0, y1) = points
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| {
            (a.min(p.1), b.max(p.1))
        });
    let inner = rect.shrink2(vec2(34.0, 16.0));
    let at = |x: f64, y: f64| {
        pos2(
            inner.left() + ((x - x0) / (x1 - x0).max(1e-9)) as f32 * inner.width(),
            inner.bottom() - ((y - y0) / (y1 - y0).max(1e-9)) as f32 * inner.height(),
        )
    };
    let font = FontId::proportional(10.0);
    for decade in (x0.ceil() as i32)..=(x1.floor() as i32) {
        let p = at(f64::from(decade), y0);
        painter.vline(
            p.x,
            inner.y_range(),
            (1.0, theme::MUTED.gamma_multiply(0.25)),
        );
        painter.text(
            pos2(p.x, rect.bottom() - 2.0),
            Align2::CENTER_BOTTOM,
            format!("{} d", significant(10f64.powi(decade))),
            font.clone(),
            theme::MUTED,
        );
    }
    // Rees's t^(−5/3), through the first rate from LATE times the first
    // return on (or the last).
    let from = (crate::fluid::LATE * debris.times[0] / DAY).log10();
    let anchor = points
        .iter()
        .find(|p| p.0 >= from)
        .or(points.last())
        .copied()
        .unwrap_or(points[0]);
    let guide = |x: f64| anchor.1 - 5.0 / 3.0 * (x - anchor.0);
    painter.extend(egui::Shape::dashed_line(
        &[at(x0, guide(x0)), at(x1, guide(x1))],
        (1.0, theme::MUTED),
        6.0,
        4.0,
    ));
    for &(x, y) in &points {
        painter.circle_filled(at(x, y), 2.5, theme::ACCENT);
    }
    painter.text(
        rect.left_top() + vec2(6.0, 3.0),
        Align2::LEFT_TOP,
        "share of its mass coming back a day (log)   – – t^−5/3",
        font,
        theme::MUTED,
    );
}

/// Draws a recording's h₊ against time, one column of the plot per
/// stretch of samples (its highest and lowest), so fast waves stay visible.
fn waveform_plot(ui: &mut egui::Ui, recording: &waves::Recording) {
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 200.0), Sense::hover());
    let background = ui.visuals().extreme_bg_color;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, background);
    let n = recording.times.len();
    let span = recording.times[n - 1].max(f64::MIN_POSITIVE);
    let peak = recording
        .plus
        .iter()
        .fold(0.0f64, |m, h| m.max(h.abs()))
        .max(f64::MIN_POSITIVE);
    let middle = rect.center().y;
    let half = 0.45 * rect.height();
    painter.hline(
        rect.x_range(),
        middle,
        (1.0, theme::MUTED.gamma_multiply(0.4)),
    );
    if let Some(hole) = &recording.final_hole {
        let x = rect.left() + (hole.start / span) as f32 * rect.width();
        painter.vline(x, rect.y_range(), (1.0, theme::MUTED.gamma_multiply(0.6)));
        painter.text(
            pos2(x - 4.0, rect.top() + 4.0),
            Align2::RIGHT_TOP,
            "ringdown starts here",
            FontId::proportional(11.0),
            theme::MUTED,
        );
    }
    let columns = rect.width().max(1.0) as usize;
    let mut k = 0;
    for column in 0..columns {
        let until = span * (column + 1) as f64 / columns as f64;
        let (mut low, mut high) = (f64::INFINITY, f64::NEG_INFINITY);
        while k < n && recording.times[k] <= until {
            low = low.min(recording.plus[k]);
            high = high.max(recording.plus[k]);
            k += 1;
        }
        if low > high {
            continue;
        }
        let x = rect.left() + column as f32 + 0.5;
        let y = |h: f64| middle - (h / peak) as f32 * half;
        painter.line_segment(
            [pos2(x, y(high)), pos2(x, y(low).max(y(high) + 1.0))],
            (1.0, theme::ACCENT),
        );
    }
    let font = FontId::proportional(11.0);
    painter.text(
        rect.left_top() + vec2(6.0, 4.0),
        Align2::LEFT_TOP,
        format!("h+ peaks at {peak:.2e}"),
        font.clone(),
        theme::MUTED,
    );
    painter.text(
        rect.left_bottom() + vec2(6.0, -4.0),
        Align2::LEFT_BOTTOM,
        "now",
        font.clone(),
        theme::MUTED,
    );
    painter.text(
        rect.right_bottom() + vec2(-6.0, -4.0),
        Align2::RIGHT_BOTTOM,
        format!("{} later", long_time(span)),
        font,
        theme::MUTED,
    );
}

/// A short time, from nanoseconds to seconds: "76 µs". Shorter ones are in
/// seconds, in powers of ten: "2.7e-27 s".
fn short_time(seconds: f64) -> String {
    let (value, unit) = if seconds < 1e-9 {
        (seconds, "s")
    } else if seconds < 1e-6 {
        (seconds * 1e9, "ns")
    } else if seconds < 1e-3 {
        (seconds * 1e6, "µs")
    } else if seconds < 1.0 {
        (seconds * 1e3, "ms")
    } else {
        (seconds, "s")
    };
    format!("{} {unit}", significant(value))
}

/// A long time, from seconds to billions of years: "301 million years".
fn long_time(seconds: f64) -> String {
    let years = seconds / JULIAN_YEAR;
    if seconds < 2.0 * DAY {
        duration(seconds)
    } else if years < 1.0 {
        format!("{} days", significant(seconds / DAY))
    } else {
        format!("{} years", significant(years))
    }
}

/// A frequency in words: "270.7 Hz", or for slow ones "one cycle every
/// 21.4 s".
fn frequency(hz: f64) -> String {
    if hz >= 1.0 {
        format!("{} Hz", significant(hz))
    } else {
        format!("one cycle every {}", long_time(1.0 / hz))
    }
}

/// A duration from nanoseconds to years.
fn any_time(seconds: f64) -> String {
    if seconds < 1.0 {
        short_time(seconds)
    } else {
        long_time(seconds)
    }
}

const SUN: usize = 0;
const EARTH: usize = 3;

/// How bright the sky is shown: its map as stored, dimmed a little so the
/// bodies stand out.
const SKY_EXPOSURE: f32 = 0.8;

/// How the black hole bending the sky bends each ray, from where the camera
/// is (see `worldline_core::lensing::BendTable`).
struct LensTable {
    /// The hole's name, and its GM (it can grow).
    hole: String,
    gm: f64,
    /// The camera's distance from it when the table was worked out, in m.
    distance: f64,
    /// The shadow's edge, and the bending, as the GPU reads them.
    edge: f64,
    bending: Vec<f32>,
    /// From the camera toward the hole, now.
    toward: DVec3,
}

/// Within this many GM/c² of the black hole bending the sky, its rays are
/// traced one by one, spin and all, and a disk can be drawn around it.
/// Farther, the table above serves. On the GPU a ray's direction is good
/// to 6 × 10⁻⁸ rad, so from here it passes the hole within 6 × 10⁻⁴ GM/c²
/// of where it should, a ten-thousandth of the shadow.
const KERR_REACH: f64 = 1e4;

/// Traced rays have escaped once moving outward past this many times the
/// camera's distance: a ray passing at b would bend by under GM b/(c²R²)
/// more, under 10⁻⁴ of the camera's distance in GM/c², a hundredth of a
/// pixel or less.
const KERR_FAR: f64 = 100.0;

/// The accretion disk, which is visual: a thin disk (Novikov & Thorne
/// 1973) fed at a tenth of the Eddington rate, from the innermost stable
/// orbit out to 25 GM/c², fading to clear from 15.
const DISK_EDDINGTON: f64 = 0.1;
const DISK_OUTER: f64 = 25.0;
const DISK_RINGS: usize = 256;
/// How bright its hottest ring is drawn, unshifted, before the display's
/// gentle limit (1 − e^(−x)).
const DISK_BRIGHTNESS: f64 = 4.0;
/// The colors the disk can glow: ln T from 10³ K to 10⁹ K. Each is the tint
/// of a blackbody that hot, as bright as the square root of its total
/// light, σT⁴, relative to one at [`DISK_REFERENCE`]: the scale a picture
/// needs to show the dim outer disk along with the bright inner one.
const DISK_REFERENCE: f64 = 1e4;
const DISK_COLORS: usize = 512;
const LN_T_MIN: f64 = 6.907_755_278_982_137;
const LN_T_MAX: f64 = 20.723_265_836_946_41;

/// The accretion disk drawn around the black hole whose rays are traced,
/// as the GPU reads it, worked out for one hole.
struct DiskTable {
    /// The hole's name, GM (it can grow) and spin.
    hole: String,
    gm: f64,
    spin: f64,
    /// Where it starts: the innermost stable orbit.
    inner: f64,
    /// At radii evenly spaced in ln r out to [`DISK_OUTER`]: ln T, the
    /// orbits' angular velocity Ω and u^t.
    rings: Vec<[f32; 3]>,
    /// Scales the colors so the hottest ring shows [`DISK_BRIGHTNESS`].
    exposure: f32,
}

impl DiskTable {
    fn new(hole: &Body, spin: f64) -> Self {
        let inner = innermost_stable_orbit(spin, true);
        let rings: Vec<[f32; 3]> = (0..DISK_RINGS)
            .map(|k| {
                let r = inner * (DISK_OUTER / inner).powf(k as f64 / (DISK_RINGS - 1) as f64);
                let temperature = disk_temperature(hole.gm, spin, DISK_EDDINGTON, r).max(1.0);
                let (omega, ut) = circular(spin, r);
                [temperature.ln() as f32, omega as f32, ut as f32]
            })
            .collect();
        let hottest = rings
            .iter()
            .map(|r| f64::from(r[0]))
            .fold(0.0, f64::max)
            .exp();
        Self {
            hole: hole.name.clone(),
            gm: hole.gm,
            spin,
            inner,
            rings,
            exposure: (DISK_BRIGHTNESS / (hottest / DISK_REFERENCE).powi(2)) as f32,
        }
    }
}

/// The colors a disk can glow, at ln T evenly spaced from [`LN_T_MIN`] to
/// [`LN_T_MAX`]: a blackbody's tint, brightest channel 1, times the square
/// root of its total light relative to one at [`DISK_REFERENCE`],
/// (T/T_ref)². Light shifted by g looks like a blackbody at gT, so this
/// shows, on that scale, the (gT)⁴ that makes the side coming toward you
/// so much brighter.
fn disk_colors() -> Vec<[f32; 3]> {
    (0..DISK_COLORS)
        .map(|k| {
            let ln_t = LN_T_MIN + (LN_T_MAX - LN_T_MIN) * k as f64 / (DISK_COLORS - 1) as f64;
            let tint = crate::sky::blackbody(ln_t.exp());
            let top = tint.iter().copied().fold(0.0, f64::max);
            let light = (ln_t.exp() / DISK_REFERENCE).powi(2);
            tint.map(|c| (c / top * light) as f32)
        })
        .collect()
}

/// Where the camera settles when you focus on a body, in body radii: close
/// enough that the globe fills most of the view.
const FOCUS_DISTANCE_RADII: f64 = 4.0;
/// How long the camera takes to fly to a newly focused body.
const FLIGHT_SECONDS: f64 = 1.2;
/// Closest the camera may get to a body's center, in body radii.
const MIN_DISTANCE_RADII: f64 = 1.1;
/// How far away the camera settles on a moon whose size hasn't been
/// measured (it is drawn as a dot).
const UNKNOWN_SIZE_DISTANCE: f64 = 1_000e3;

/// A smooth camera move to a newly focused body.
struct Flight {
    from_target: DVec3,
    from_distance: f64,
    to_distance: f64,
    start: Instant,
    /// How long it lasts, in s: longer when it crosses far more space than
    /// it starts or ends seeing.
    seconds: f64,
}

pub struct WorldlineApp {
    simulation: Simulation,
    camera: Camera,
    /// The body the camera follows.
    focus: usize,
    /// The body shown in the inspector.
    selected: usize,
    speed_index: usize,
    options: ViewOptions,
    last_frame: Instant,
    /// The camera's flight to a newly focused body, while it lasts.
    flight: Option<Flight>,
    /// The GPU renderer for globes, created on the first frame.
    gpu: Option<GpuGlobes>,
    /// The sky's GPU pass, created on the first frame.
    sky: Option<GpuSky>,
    /// How the black hole bending the sky bends it, for the camera's
    /// distance from it.
    lens: Option<LensTable>,
    /// The disk around the black hole whose rays are traced, and the
    /// colors it can glow.
    disk: Option<DiskTable>,
    disk_colors: Vec<[f32; 3]>,
    /// The black hole bending the sky, if any.
    lensed: Option<String>,
    /// What's typed in the body list's search box.
    search: String,
    /// The catalogue entry a press in the view adds, if adding; otherwise
    /// a drag looks around.
    tool: Option<&'static Entry>,
    /// Whether the catalogue window is open.
    catalogue_open: bool,
    /// Whether added bodies start at rest instead of circling.
    start_at_rest: bool,
    /// A body being placed, while its launch is dragged.
    launch: Option<Launch>,
    /// A message in the top bar, and when it appeared.
    status: Option<(String, Instant)>,
    /// Open the selected body's branch of the list and scroll to it, once
    /// it has been picked somewhere other than the list.
    reveal: bool,
    /// The gravitational-wave window's recording, while it is open.
    waves: Option<WaveView>,
    /// Whether the eclipse finder's window is open, and its search.
    eclipses_open: bool,
    finder: Option<Finder>,
    /// The eclipse to go to once the finder finds it (0 the first).
    go_to_when_found: Option<usize>,
    /// The sound output, once something has played.
    audio: Option<(rodio::OutputStream, rodio::Sink)>,
    /// The body modeled as a fluid, if any (see `fluid.rs`).
    fluid: Option<FluidBody>,
    /// The body to model as a fluid once the fluid is ready, and whether
    /// by hand (not because tides matter). Time waits for it.
    pending: Option<(String, bool)>,
    /// Readies the fluid bodies are made from (see `fluid.rs`).
    lab: Lab,
    /// The approach that slowed the last update: the body, and the one
    /// whose tides it nears.
    slowed: Option<(String, String)>,
    /// A running mean of the time between frames, s.
    frame_seconds: f64,

    /// The GPU and the graphics interface it's driven through.
    gpu_name: String,
    /// Running with the computer's clock: the simulation's time is now's,
    /// once it has caught up.
    live: bool,
    /// How far behind the computer's clock the simulation is, s, while
    /// live.
    behind: f64,
    /// When the app started.
    started: Instant,
}

/// A pair's recorded gravitational waves, shown in their window.
struct WaveView {
    /// "A and B".
    names: String,
    recording: waves::Recording,
    sound: waves::Sound,
}

impl WorldlineApp {
    pub fn new(start: &crate::StartOptions) -> Self {
        let mut app = Self {
            simulation: Simulation::solar_system(SPEEDS[DEFAULT_SPEED].1),
            camera: initial_camera(),
            focus: SUN,
            selected: EARTH,
            speed_index: DEFAULT_SPEED,
            options: ViewOptions {
                trails: true,
                grid: true,
                labels: true,
                spin_axes: true,
                globes: true,
                belts: true,
                dust: true,
                solar_wind: true,
                heliosphere: true,
                magnetospheres: true,
                shadows: true,
                starfield: true,
                accretion_disk: true,
                overlays: Overlays::default(),
            },
            last_frame: Instant::now(),
            flight: None,
            gpu: None,
            sky: None,
            lens: None,
            disk: None,
            disk_colors: disk_colors(),
            lensed: None,
            search: String::new(),
            reveal: false,
            tool: None,
            catalogue_open: false,
            start_at_rest: false,
            launch: None,
            status: None,
            waves: None,
            eclipses_open: false,
            finder: None,
            go_to_when_found: None,
            audio: None,
            fluid: None,
            pending: None,
            lab: Lab::default(),
            slowed: None,
            frame_seconds: 1.0 / 60.0,
            gpu_name: String::new(),
            live: false,
            behind: 0.0,
            started: Instant::now(),
        };
        app.simulation.paused = start.paused;
        if start.now {
            app.go_live();
        }
        if let Some(speed) = start.speed {
            app.speed_index = speed;
            app.simulation.speed = SPEEDS[speed].1;
        }
        app.catalogue_open = start.catalogue;
        app.options.overlays = start.overlays;
        if let Some(degrees) = start.pitch_degrees {
            app.camera.pitch = degrees.to_radians();
        }
        for added in &start.adds {
            let around = match &added.around {
                Some(name) => app.simulation.index_of(name).unwrap_or_else(|| {
                    eprintln!("worldline: no body named `{name}` to circle; circling the Sun");
                    SUN
                }),
                None => SUN,
            };
            app.add_on_circle(&added.kind, added.au, around, added.launch.unwrap_or(1.0));
        }
        let names = app.attended_names();
        app.simulation.advance_by(start.advance_years * JULIAN_YEAR);
        app.after_events(names);
        if let Some(name) = &start.focus {
            match app.simulation.bodies.iter().position(|b| &b.name == name) {
                Some(i) => app.fly_to(i, start.zoom_radii.unwrap_or(FOCUS_DISTANCE_RADII)),
                None => eprintln!("worldline: no body named `{name}` to focus on"),
            }
        }
        if let Some(name) = &start.fluid {
            match app.simulation.index_of(name) {
                Some(i) if app.can_be_fluid(i) => app.pending = Some((name.clone(), true)),
                Some(_) => eprintln!("worldline: `{name}` can't be modeled as a fluid"),
                None => eprintln!("worldline: no body named `{name}` to model as a fluid"),
            }
        }
        if start.eclipses {
            app.open_eclipses();
            app.go_to_when_found = start.eclipse.map(|n| n - 1);
        }
        if start.waves {
            if app.simulation.gravitational_waves(app.selected).is_some() {
                app.open_waves();
            } else {
                eprintln!("worldline: --waves needs --focus on one body of a pair that will merge");
            }
        }
        app
    }

    /// The solar system as it is now: from the 2025 snapshot, caught up to
    /// the computer's clock, then running in real time with it.
    fn go_live(&mut self) {
        self.reset();
        self.live = true;
        self.speed_index = REAL_TIME;
        self.simulation.speed = SPEEDS[REAL_TIME].1;
        self.simulation.paused = false;
    }

    /// Where the computer's clock is, as the simulation's time (s since
    /// its start): UTC, by the IERS's leap seconds, as TDB.
    fn clock_time(&self) -> f64 {
        let unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64());
        let jd_utc = worldline_core::timescales::jd_from_unix(unix);
        self.simulation
            .time_at(self.simulation.time_scales().tdb(jd_utc))
    }

    fn reset(&mut self) {
        self.simulation = Simulation::solar_system(SPEEDS[self.speed_index].1);
        self.camera = initial_camera();
        self.focus = SUN;
        self.flight = None;
        self.drop_fluid();
    }

    /// Lets go of any fluid, and any body waiting to become one: a new or
    /// loaded simulation starts with every body a point.
    fn drop_fluid(&mut self) {
        self.fluid = None;
        self.pending = None;
    }

    /// Makes the camera follow a body, flying in close to it.
    fn focus_on(&mut self, index: usize) {
        self.fly_to(index, FOCUS_DISTANCE_RADII);
    }

    /// Makes the camera follow a body, settling `radii` of its radii away.
    /// Sagittarius A* is framed with the stars around it: its horizon is
    /// tiny next to their orbits.
    fn fly_to(&mut self, index: usize, radii: f64) {
        let radius = self.simulation.bodies[index].radius;
        let to_distance = if let Some(reach) = self.simulation.cluster_reach(index) {
            reach
        } else if radius > 0.0 {
            radii * radius
        } else {
            UNKNOWN_SIZE_DISTANCE
        };
        let across = (self.simulation.bodies[index].position - self.camera.target).length();
        let far = across > 10.0 * self.camera.distance.max(to_distance);
        self.flight = Some(Flight {
            from_target: self.camera.target,
            from_distance: self.camera.distance,
            to_distance,
            start: Instant::now(),
            seconds: if far {
                2.5 * FLIGHT_SECONDS
            } else {
                FLIGHT_SECONDS
            },
        });
        // Detail follows focus: the small moons around this body are
        // now computed in full.
        self.simulation.focus_detail_on(index);
        self.focus = index;
        self.selected = index;
        self.reveal = true;
    }

    /// Points the camera at the focused body, partway along a flight if one
    /// is under way.
    fn aim_camera(&mut self) {
        let target = self.simulation.bodies[self.focus].position;
        let Some(flight) = &self.flight else {
            self.camera.target = target;
            return;
        };
        let t = (flight.start.elapsed().as_secs_f64() / flight.seconds).min(1.0);
        let eased = t * t * (3.0 - 2.0 * t);
        self.camera.target = flight.from_target.lerp(target, eased);
        // Zoom evenly in scale, so going from 4 AU to 25,000 km looks smooth.
        // Across far more space than either end shows (the solar system to
        // the galactic center), climb out to see both, then come back in.
        let (from, to) = (flight.from_distance.ln(), flight.to_distance.ln());
        let across = (target - flight.from_target).length() * 1.5;
        let lift = (across.ln() - from.max(to)).max(0.0);
        self.camera.distance =
            (from + (to - from) * eased + 4.0 * eased * (1.0 - eased) * lift).exp();
        if t >= 1.0 {
            self.flight = None;
        }
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Worldline").strong().size(16.0));
            ui.separator();
            let label = if self.simulation.paused {
                "Play"
            } else {
                "Pause"
            };
            if ui.button(label).clicked() {
                self.simulation.paused = !self.simulation.paused;
                // Stopped, it no longer keeps up with the clock.
                self.live &= !self.simulation.paused;
            }
            let speed = self.speed_index;
            egui::ComboBox::from_id_salt("speed")
                .selected_text(SPEEDS[self.speed_index].0)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in SPEEDS.iter().enumerate() {
                        ui.selectable_value(&mut self.speed_index, i, *name);
                    }
                });
            self.live &= self.speed_index == speed;
            self.simulation.speed = SPEEDS[self.speed_index].1;
            if ui.button("Reset").clicked() {
                self.reset();
                self.live = false;
            }
            if ui
                .button("Now")
                .on_hover_text(
                    "The solar system as it is right now, running in real time with the computer's clock: from the 2025 snapshot, caught up to the present",
                )
                .clicked()
            {
                self.go_live();
            }
            if ui
                .selectable_label(self.eclipses_open, "Eclipses")
                .on_hover_text(
                    "Find every solar and lunar eclipse of the next 5 years, by running a copy of this simulation ahead",
                )
                .clicked()
            {
                if self.eclipses_open {
                    self.eclipses_open = false;
                } else {
                    self.open_eclipses();
                }
            }
            ui.separator();
            self.sandbox_controls(ui);
            ui.separator();
            self.clock(ui);
            if let Some(note) = self.time_note() {
                ui.separator();
                ui.colored_label(Color32::from_rgb(255, 170, 60), note);
            }
        });
    }

    /// The simulation's time in UTC, to the second, the time scale
    /// people's clocks keep; TDB and how Earth's rotation stands on
    /// hovering. Marked when it is the computer's clock's.
    fn clock(&self, ui: &mut egui::Ui) {
        let utc = self.simulation.julian_date_utc();
        let scales = self.simulation.time_scales();
        let (dut1, source) = scales.ut1_minus_utc(utc);
        let (known, measured) = scales.ut1_known_until();
        let source = match source {
            worldline_core::timescales::Ut1Source::Measured => "measured".to_string(),
            worldline_core::timescales::Ut1Source::Predicted => {
                format!("predicted (measured to {})", &to_the_second(measured)[..10])
            }
            worldline_core::timescales::Ut1Source::Held => format!(
                "held at its last value: the IERS's table runs {} to {}",
                &to_the_second(scales.ut1_known_from())[..10],
                &to_the_second(known)[..10]
            ),
        };
        ui.label(
            RichText::new(format!("{} UTC", to_the_second(utc)))
                .monospace()
                .size(15.0),
        )
        .on_hover_text(format!(
            "{} TDB, the simulation's own time\nΔT = TT − UT1 = {:.3} s, UT1 − UTC = {dut1:+.4} s from the IERS, {source}\nLeap seconds: TAI − UTC = {:.0} s",
            to_the_second(self.simulation.julian_date()),
            scales.delta_t(utc),
            scales.tai_minus_utc(utc)
        ));
        if self.live && self.behind < 1.0 {
            ui.colored_label(theme::ACCENT, "live")
                .on_hover_text("Running with the computer's clock");
        }
    }

    /// The side panel: the bodies in a scrolling tree at the top, and below
    /// it, always in view, the selected body and the settings.
    fn inspector(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading("Bodies");
            ui.label(
                RichText::new(self.simulation.bodies.len().to_string())
                    .small()
                    .color(theme::MUTED),
            );
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search by name")
                .desired_width(f32::INFINITY),
        );
        ui.label(
            RichText::new("Click to select · double-click to follow")
                .small()
                .color(theme::MUTED),
        );
        let list_height = (ui.available_height() * 0.42).clamp(140.0, 440.0);
        egui::ScrollArea::vertical()
            .id_salt("bodies")
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| self.body_list(ui));
        ui.separator();
        egui::ScrollArea::vertical()
            .id_salt("details")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.selected_body(ui);
                ui.add_space(8.0);
                egui::CollapsingHeader::new("Physics")
                    .id_salt("physics section")
                    .show(ui, |ui| self.physics(ui));
                egui::CollapsingHeader::new("Display")
                    .id_salt("display section")
                    .show(ui, |ui| self.display(ui));
                egui::CollapsingHeader::new("Overlays")
                    .id_salt("overlays section")
                    .show(ui, |ui| self.overlay_options(ui));
            });
    }

    /// The bodies as a tree: each planet folds open to its moons (the small
    /// ones folded once more), then the small bodies by kind. While a search
    /// is typed, a flat list of the matches instead.
    fn body_list(&mut self, ui: &mut egui::Ui) {
        let simulation = &self.simulation;
        let (selected, reveal) = (self.selected, self.reveal);
        let (mut clicked, mut fly_to, mut revealed) = (None, None, false);
        let mut entry = |ui: &mut egui::Ui, i: usize| {
            let name = &simulation.bodies[i].name;
            let response = ui.selectable_label(
                i == selected,
                RichText::new(name).color(view::color(simulation, i)),
            );
            if response.clicked() {
                clicked = Some(i);
            }
            if response.double_clicked() {
                fly_to = Some(i);
            }
            if reveal && i == selected {
                response.scroll_to_me(Some(egui::Align::Center));
                revealed = true;
            }
        };
        let query = self.search.trim().to_lowercase();
        if !query.is_empty() {
            let matches: Vec<usize> = (0..simulation.bodies.len())
                .filter(|&i| simulation.bodies[i].name.to_lowercase().contains(&query))
                .collect();
            if matches.is_empty() {
                ui.label(RichText::new("No body by that name").color(theme::MUTED));
            }
            for i in matches.into_iter().take(200) {
                entry(ui, i);
            }
        } else {
            // The planet whose branch holds the selection.
            let holder = simulation.parent(selected).unwrap_or(selected);
            for i in 0..simulation.bodies.len() {
                if simulation.parent(i).is_some() || simulation.small_body(i).is_some() {
                    continue;
                }
                let (small, major): (Vec<usize>, Vec<usize>) = (0..simulation.bodies.len())
                    .filter(|&m| simulation.parent(m) == Some(i))
                    .partition(|&m| simulation.is_small_moon(m));
                if major.is_empty() && small.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(ui.spacing().indent);
                        entry(ui, i);
                    });
                    continue;
                }
                let id = ui.make_persistent_id(("planet", i));
                let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, false);
                if reveal && holder == i && selected != i {
                    state.set_open(true);
                }
                let moons = major.len() + small.len();
                let noun = if simulation.is_galactic(i) {
                    "star"
                } else {
                    "moon"
                };
                state
                    .show_header(ui, |ui| {
                        entry(ui, i);
                        ui.label(
                            RichText::new(if moons == 1 {
                                format!("1 {noun}")
                            } else {
                                format!("{moons} {noun}s")
                            })
                            .small()
                            .color(theme::MUTED),
                        );
                    })
                    .body(|ui| {
                        for m in major {
                            entry(ui, m);
                        }
                        if !small.is_empty() {
                            let mut fold = egui::CollapsingHeader::new(
                                RichText::new(format!("{} small moons", small.len()))
                                    .color(theme::MUTED),
                            )
                            .id_salt(("small moons", i));
                            if reveal && small.contains(&selected) {
                                fold = fold.open(Some(true));
                            }
                            fold.show(ui, |ui| {
                                for m in small {
                                    entry(ui, m);
                                }
                            });
                        }
                    });
            }
            ui.add_space(4.0);
            // The dwarf planets, asteroids and comets, each kind in a fold.
            for (kind, label) in [
                (SmallBodyKind::DwarfPlanet, "Dwarf planets"),
                (SmallBodyKind::TransNeptunian, "Beyond Neptune"),
                (SmallBodyKind::Asteroid, "Asteroids"),
                (SmallBodyKind::Comet, "Comets"),
                (SmallBodyKind::Interstellar, "Interstellar visitors"),
            ] {
                let members: Vec<usize> = (0..simulation.bodies.len())
                    .filter(|&i| simulation.small_body(i).map(|s| s.0) == Some(kind))
                    .collect();
                let mut fold =
                    egui::CollapsingHeader::new(format!("{label}  ·  {}", members.len()))
                        .id_salt(label);
                if reveal && members.contains(&selected) {
                    fold = fold.open(Some(true));
                }
                fold.show(ui, |ui| {
                    for i in members {
                        entry(ui, i);
                    }
                });
            }
        }
        // Keep trying until the selected entry has been laid out (its branch
        // may still be opening).
        if revealed || !query.is_empty() {
            self.reveal = false;
        }
        if let Some(i) = clicked {
            self.selected = i;
        }
        if let Some(i) = fly_to {
            self.focus_on(i);
        }
    }

    fn physics(&self, ui: &mut egui::Ui) {
        egui::Grid::new("physics").num_columns(2).show(ui, |ui| {
            for (what, value) in [
                ("Gravity", self.simulation.gravity().name()),
                ("Moons", self.simulation.moon_gravity_name()),
                ("Integrator", self.simulation.integrator_name()),
                (
                    "Data",
                    "NASA JPL Horizons (DE441 and satellite ephemerides)",
                ),
                ("Start", "2025-01-01 00:00 TDB"),
                ("Rotation", "IAU models (NASA NAIF)"),
                (
                    "Collisions",
                    "bodies that touch merge: momentum kept, volumes add",
                ),
                ("Surfaces", "Solar System Scope, CC BY 4.0"),
            ] {
                ui.label(key(what));
                ui.label(value);
                ui.end_row();
            }
        });
    }

    /// The overlays' switches: data drawn over the view (see
    /// `overlays.rs`).
    fn overlay_options(&mut self, ui: &mut egui::Ui) {
        let overlays = &mut self.options.overlays;
        ui.checkbox(&mut overlays.models, "Models").on_hover_text(
            "Which model computes each body on screen, with a ring around any it covers only \
                 approximately (amber) or not at all (red), from how strong gravity is there \
                 (GM/rc²) and how fast it moves (v/c)",
        );
        ui.checkbox(&mut overlays.clocks, "Clock rates")
            .on_hover_text(
                "How much more slowly a clock at rest runs than one far away, √(1 − 2U/c²): a \
                 heatmap on the plane through what the camera looks at, square to the view. \
                 Exact for one body (Schwarzschild, or Kerr near a spinning hole); for several, \
                 their weak fields added",
            );
        ui.checkbox(&mut overlays.tides, "Tides").on_hover_text(
            "How the body in focus is stretched and squeezed by everything else (Newton's \
                 tidal field); around a black hole, the stretch a small body near it would feel",
        );
        ui.checkbox(&mut overlays.embedding, "Embedding diagram")
            .on_hover_text(
                "The shape of space around the body in focus: its equatorial plane drawn as a \
                 surface whose distances are true (Flamm's paraboloid; inside a body, \
                 Schwarzschild's interior solution for even density), depth exaggerated by a \
                 stated power of ten when too small to see",
            );
        ui.checkbox(&mut overlays.proper_time, "Proper time")
            .on_hover_text(
                "A dial beside the selected body: its own clock's hand (bright) against a \
                 far-away clock's (faint), both turning once in the same time. Near a black hole \
                 the bright hand visibly falls behind, by √(1 − 3GM/rc²) on a circular orbit",
            );
        ui.checkbox(&mut overlays.waves, "Gravitational waves")
            .on_hover_text(
                "When the body in focus is in a pair that radiates: the crests of its \
                 gravitational waves in the orbit's plane, spiralling out at the speed of light \
                 (leading order, as from a circular orbit)",
            );
        ui.add_space(4.0);
        ui.label(
            RichText::new("Each says what it shows in the view's top-left corner.")
                .small()
                .color(theme::MUTED),
        );
    }

    fn display(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.options.starfield, "Starfield")
            .on_hover_text(
                "The Milky Way as ESA's Gaia mapped it (CC BY-SA 3.0 IGO), and the 9,096 stars of \
                 the Yale Bright Star Catalogue at their places: as seen from the solar system. \
                 Around a black hole it is bent as general relativity bends light; how bright \
                 the stars are drawn is visual",
            );
        ui.checkbox(&mut self.options.accretion_disk, "Accretion disk")
            .on_hover_text(
                "Visual only: a thin disk of gas around the black hole bending the sky, within \
                 10,000 GM/c² of it, as Novikov and Thorne's model has it glow if fed at a tenth \
                 of the Eddington rate, out to 25 GM/c². Its light is bent, and shifted by the \
                 disk's motion and the hole's gravity, as general relativity predicts. Color is \
                 the tint of a blackbody at the shifted temperature; brightness, the square root \
                 of its total light, so the dim outer disk shows too. The gas itself isn't \
                 simulated",
            );
        ui.checkbox(&mut self.options.trails, "Orbit trails")
            .on_hover_text("Moons show their current orbit instead");
        ui.checkbox(&mut self.options.grid, "Distance rings")
            .on_hover_text("In the ecliptic plane, labeled in AU and light-time");
        ui.checkbox(&mut self.options.labels, "Names")
            .on_hover_text(
                "The body you follow, the one selected, the one under the pointer, \
             and the major moons of the planet in focus",
            );
        ui.checkbox(&mut self.options.spin_axes, "Spin axes")
            .on_hover_text("A dot marks the end the spin points toward");
        ui.checkbox(&mut self.options.globes, "Textured globes up close");
        let total: usize = self
            .simulation
            .belts
            .iter()
            .map(|b| b.positions.len())
            .sum();
        ui.checkbox(&mut self.options.belts, "Asteroid and Kuiper belts")
            .on_hover_text(format!(
                "{total} real orbits from JPL (measured), on fixed ellipses around the \
                 Sun (model: about 1/10,000 of their distance off after a year). Shown \
                 when zoomed out past 0.05 AU."
            ));
        ui.checkbox(&mut self.options.dust, "Zodiacal dust")
            .on_hover_text(
                "The COBE DIRBE model of Kelsall et al. 1998, drawn as the sunlight it \
             scatters on a log scale (visual); held at its 0.3 AU brightness farther \
             in, where no probe has measured how it brightens",
            );
        ui.checkbox(&mut self.options.solar_wind, "Solar wind spiral")
            .on_hover_text("Parker's spiral field, with the Sun in focus");
        ui.checkbox(&mut self.options.heliosphere, "Heliosphere")
            .on_hover_text("Its boundaries, seen from outside them");
        ui.checkbox(&mut self.options.magnetospheres, "Magnetospheres")
            .on_hover_text(
                "Around the planet in focus: its magnetopause, from pressure balance with \
             2025's average solar wind; Earth's radiation belts",
            );
        ui.checkbox(&mut self.options.shadows, "Shadows of the Moon and Earth")
            .on_hover_text(
                "With Earth or the Moon in focus: each one's umbra (red) and penumbra \
             (amber), the cones tangent to it and the Sun, ringed where they pass the \
             other; Earth's enlarged 1% by its atmosphere (Danjon)",
            );
        ui.add_space(4.0);
        ui.label(
            RichText::new(
                "Distances, sizes and positions are to scale. Bodies too small to \
                 see are drawn as dots at least 3 points wide.",
            )
            .small()
            .color(theme::MUTED),
        );
    }

    fn selected_body(&mut self, ui: &mut egui::Ui) {
        ui.heading(
            RichText::new(&self.simulation.bodies[self.selected].name)
                .color(view::color(&self.simulation, self.selected)),
        );
        self.model_indicator(ui);
        self.fluid_rows(ui);
        let bodies = &self.simulation.bodies;
        let body = &bodies[self.selected];
        let sun = &bodies[SUN];
        let parent = self.simulation.parent(self.selected);
        egui::Grid::new("selected").num_columns(2).show(ui, |ui| {
            ui.label(key("Mass"));
            if body.gm > 0.0 {
                ui.label(format!("{:.4e} kg", body.mass()));
                ui.end_row();
                ui.label("");
                let earths = body.gm / GM_EARTH;
                if body.gm >= 0.01 * GM_SUN {
                    ui.label(measured_suns(body.gm / GM_SUN, None));
                } else if earths >= 1e-3 {
                    ui.label(format!("{earths:.6} Earth masses"));
                } else {
                    ui.label(format!("{} Earth masses", significant(earths)));
                }
            } else {
                ui.label(key("not measured"));
            }
            ui.end_row();
            if is_black_hole(body) {
                ui.label(key("Event horizon"));
                ui.label(length(body.radius));
                ui.end_row();
                self.black_hole_rows(ui, body);
            } else {
                ui.label(key("Radius"));
                ui.label(if body.radius > 0.0 {
                    format!("{:.1} km", body.radius / 1e3)
                } else {
                    "not measured".to_string()
                });
            }
            ui.end_row();
            self.catalog_rows(ui);
            self.clock_rows(ui);
            if let Some(parent) = parent {
                let planet = &bodies[parent];
                let d = (body.position - planet.position).length();
                ui.label(key(&format!("Distance from {}", planet.name)));
                ui.label(if d > 1e10 {
                    format!("{:.1} AU", d / AU)
                } else {
                    format!("{:.0} km", d / 1e3)
                });
                ui.end_row();
                ui.label(key(&format!("Speed relative to {}", planet.name)));
                ui.label(format!(
                    "{:.3} km/s",
                    (body.velocity - planet.velocity).length() / 1e3
                ));
                ui.end_row();
            } else if self.simulation.is_galactic(self.selected) {
                // Too far for AU: in light-years and parsecs.
                ui.label(key(&format!("Distance from {}", sun.name)));
                ui.label(light_years((body.position - sun.position).length()));
                ui.end_row();
            } else if self.selected != SUN {
                ui.label(key(&format!("Distance from {}", sun.name)));
                ui.label(format!(
                    "{:.4} AU",
                    (body.position - sun.position).length() / AU
                ));
                ui.end_row();
                ui.label(key(&format!("Speed relative to {}", sun.name)));
                ui.label(format!(
                    "{:.2} km/s",
                    (body.velocity - sun.velocity).length() / 1e3
                ));
                ui.end_row();
            }
            if !self.simulation.is_galactic(self.selected) {
                self.sun_reach(ui);
                self.galaxy_rows(ui);
                self.magnetism(ui);
            }
            if let Some(model) = self.simulation.rotation(self.selected) {
                let jd = self.simulation.julian_date();
                let hours = model.sidereal_period() / 3600.0;
                ui.label(key("Day (sidereal)"));
                ui.label(if hours < 72.0 {
                    format!("{hours:.3} h")
                } else {
                    format!("{:.2} days", hours / 24.0)
                });
                ui.end_row();
                ui.label(key("Spin"));
                ui.label(if model.spin_rate() > 0.0 {
                    "prograde"
                } else {
                    "retrograde"
                });
                ui.end_row();
                // Tilt of the spin axis to the orbit: around its planet for
                // a moon, around the Sun for a planet, to the ecliptic for
                // the Sun.
                let orbit_normal = match (self.selected, parent) {
                    (SUN, _) => DVec3::Z,
                    (_, Some(planet)) => orbit_normal(body, &bodies[planet]),
                    _ => orbit_normal(body, sun),
                };
                ui.label(if self.selected == SUN {
                    "Tilt to ecliptic"
                } else {
                    "Axial tilt"
                });
                ui.label(format!(
                    "{:.2}°",
                    model.spin_axis(jd).angle_between(orbit_normal).to_degrees()
                ));
                ui.end_row();
            }
        });
        self.catalog_notes(ui);
        ui.add_space(4.0);
        let kind = if self.simulation.is_galactic(self.selected)
            && !self.simulation.is_added(self.selected)
        {
            details::BodyKind::Galactic {
                star: self.simulation.parent(self.selected).is_some(),
            }
        } else if self.simulation.merger(self.selected).is_some() {
            details::BodyKind::MergerRemnant
        } else if self.simulation.kind(self.selected) == Some(ObjectKind::BlackHole)
            && self.simulation.entry(self.selected).and_then(Entry::kind)
                == Some(ObjectKind::NeutronStar)
        {
            details::BodyKind::CollapsedStar
        } else if self
            .simulation
            .entry(self.selected)
            .is_some_and(Entry::is_hypothetical)
        {
            details::BodyKind::MiniBlackHole
        } else if self.simulation.is_added(self.selected) {
            details::BodyKind::Added(self.simulation.kind(self.selected))
        } else if let Some((kind, outgassing)) = self.simulation.small_body(self.selected) {
            details::BodyKind::SmallBody {
                comet: matches!(kind, SmallBodyKind::Comet | SmallBodyKind::Interstellar),
                outgassing,
                massive: body.gm > 0.0,
            }
        } else if self.simulation.is_detailed(self.selected) {
            details::BodyKind::SmallMoonInDetail
        } else if self.simulation.is_small_moon(self.selected) {
            details::BodyKind::SmallMoonOnMeanOrbit
        } else if parent.is_some() {
            details::BodyKind::Moon
        } else {
            details::BodyKind::Other
        };
        for detail in details::details(&body.name, kind) {
            ui.horizontal_wrapped(|ui| {
                ui.label(
                    RichText::new(detail.kind.tag())
                        .small()
                        .color(detail.kind.color()),
                );
                ui.label(RichText::new(detail.what).small());
            });
        }
        if self.simulation.gravitational_waves(self.selected).is_some()
            && ui
                .button("See and hear its gravitational waves")
                .on_hover_text(
                    "The waves an observer on Earth would record from this pair, from a copy of it run ahead",
                )
                .clicked()
        {
            self.open_waves();
        }
        ui.horizontal(|ui| {
            if self.focus == self.selected {
                ui.label(RichText::new("The camera is following this body.").weak());
            } else if ui.button("Fly to it").clicked() {
                self.focus_on(self.selected);
            }
            let name = self.simulation.bodies[self.selected].name.clone();
            match self.simulation.removal(self.selected) {
                Removal::Allowed { moons } => {
                    let label = match moons {
                        0 => "Remove".to_string(),
                        1 => "Remove with its moon".to_string(),
                        n => format!("Remove with its {n} moons"),
                    };
                    if ui
                        .button(label)
                        .on_hover_text(format!("Take {name} out of the simulation (Delete key)"))
                        .clicked()
                    {
                        self.remove_selected();
                    }
                }
                Removal::Sun => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text("The Sun stays: everything is measured from it");
                }
                Removal::Moon => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text("Moons can't be removed on their own yet");
                }
                Removal::GalacticCenter => {
                    ui.add_enabled(false, egui::Button::new("Remove"))
                        .on_disabled_hover_text(
                            "Sagittarius A* stays: its region is measured from it",
                        );
                }
            }
        });
    }

    /// Whether body `index` can be modeled as a fluid: it needs a measured
    /// mass and size, and a black hole has no fluid to model.
    fn can_be_fluid(&self, index: usize) -> bool {
        let body = &self.simulation.bodies[index];
        body.gm > 0.0 && body.radius > 0.0 && !is_black_hole(body)
    }

    /// The bodies whose tides stretch body `index`, strongest first: each
    /// one's index and how much it stretches it, h₂ (r_t/D)³ with an n = 1
    /// polytrope's h₂, as a fraction of its radius. Only bodies in its own
    /// region, with mass.
    fn tides_on(&self, index: usize) -> Vec<(usize, f64)> {
        let bodies = &self.simulation.bodies;
        let body = &bodies[index];
        if body.gm <= 0.0 || body.radius <= 0.0 {
            return Vec::new();
        }
        let h2 = 1.0 + polytrope_love_number();
        let galactic = self.simulation.is_galactic(index);
        let mut tides: Vec<(usize, f64)> = (0..bodies.len())
            .filter(|&j| j != index && bodies[j].gm > 0.0)
            .filter(|&j| self.simulation.is_galactic(j) == galactic)
            .map(|j| {
                let rt = tidal_radius(body.radius, body.gm, bodies[j].gm);
                let d = (bodies[j].position - body.position).length();
                (j, stretch(h2, rt, d))
            })
            .collect();
        tides.sort_by(|a, b| b.1.total_cmp(&a.1));
        tides
    }

    /// Whether body `index` turns into a fluid by itself when tides matter:
    /// a giant planet (its bulk is fluid, and an n = 1 polytrope models
    /// it), with a measured radius.
    fn becomes_fluid(&self, index: usize) -> bool {
        let body = &self.simulation.bodies[index];
        giant_planet(body.gm) && body.radius > 0.0 && !is_black_hole(body)
    }

    /// Where each giant planet must stop to become a fluid: for each body
    /// whose tides could stretch it by 1% before the two touch, that
    /// distance. None while a body is a fluid: one at a time.
    fn approaches(&self) -> Vec<Approach> {
        if self.fluid.is_some() {
            return Vec::new();
        }
        let bodies = &self.simulation.bodies;
        let h2 = 1.0 + polytrope_love_number();
        let mut approaches = Vec::new();
        for i in (0..bodies.len()).filter(|&i| self.becomes_fluid(i)) {
            let galactic = self.simulation.is_galactic(i);
            for j in (0..bodies.len()).filter(|&j| j != i && bodies[j].gm > 0.0) {
                if self.simulation.is_galactic(j) != galactic {
                    continue;
                }
                let (a, b) = (&bodies[i], &bodies[j]);
                let rt = tidal_radius(a.radius, a.gm, b.gm);
                let distance = distance_for(h2, rt, FLUID_STRETCH);
                if distance <= a.radius + b.radius {
                    continue;
                }
                approaches.push(Approach {
                    body: a.name.clone(),
                    by: b.name.clone(),
                    distance,
                });
            }
        }
        approaches
    }

    /// How far this frame's update may go: no further than the fluid's
    /// clock while a body is one, nowhere while one waits to become one,
    /// and otherwise up to where a giant planet's tides would make it one.
    fn limits(&self) -> Limits {
        if let Some(fluid) = &self.fluid {
            Limits {
                approaches: Vec::new(),
                until: Some(fluid.reached()),
            }
        } else if self.pending.is_some() {
            Limits {
                approaches: Vec::new(),
                until: Some(self.simulation.time()),
            }
        } else {
            Limits {
                approaches: self.approaches(),
                until: None,
            }
        }
    }

    /// What the top bar says about time, if it isn't running as asked.
    fn time_note(&self) -> Option<String> {
        if self.live && self.behind >= 1.0 {
            return Some(format!(
                "Catching up to now: {}",
                DateTime::from_julian_date(self.simulation.julian_date_utc())
            ));
        }
        if let Some((name, _)) = &self.pending {
            let doing = self.lab.status().unwrap_or_else(|| "starting".into());
            return Some(format!("{name} is becoming a fluid: {doing}"));
        }
        if let Some(fluid) = &self.fluid {
            let pace = f64::from(fluid.clock.dt) * fluid.units.time / self.frame_seconds;
            return Some(format!(
                "{} is a fluid: time runs at its pace, {} a second",
                fluid.name,
                duration(pace)
            ));
        }
        if self.simulation.paused || self.simulation.achieved >= 0.99 {
            return None;
        }
        Some(match &self.slowed {
            Some((body, by)) => format!("Slowed down: {body} nearing {by}'s tides"),
            None => format!(
                "Physics running at {:.0}% of the requested speed",
                self.simulation.achieved * 100.0
            ),
        })
    }

    /// The selected body as a fluid: how much tides stretch it, the button
    /// that makes it one (or back into a point), and while it is one, its
    /// particles, its clock and how fast the GPU steps it (see
    /// `fluid.rs`).
    fn fluid_rows(&mut self, ui: &mut egui::Ui) {
        if !self.can_be_fluid(self.selected) {
            return;
        }
        let name = self.simulation.bodies[self.selected].name.clone();
        let tag = |ui: &mut egui::Ui, kind: details::Kind, what: &str| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(kind.tag()).small().color(kind.color()));
                ui.label(RichText::new(what).small());
            });
        };
        let giant = self.becomes_fluid(self.selected);
        let tides = self.tides_on(self.selected);
        let fluid = self.fluid.as_ref().filter(|f| f.name == name);
        // How much tides stretch it, for a giant planet, or any body they
        // stretch by 1% or more.
        if let Some(&(by, stretched)) = tides.first()
            && (giant || stretched >= FLUID_STRETCH)
        {
            egui::Grid::new("tides").num_columns(2).show(ui, |ui| {
                ui.label(key("Tides stretch it"));
                ui.label(format!(
                    "by {} of its radius ({})",
                    percent(stretched),
                    self.simulation.bodies[by].name
                ));
                ui.end_row();
            });
            let note = if !giant {
                "A point can't show that, and its matter has no fluid model yet: rock comes with step 6.4"
            } else if fluid.is_some_and(|f| f.debris.is_some()) {
                "Torn apart, it stays a fluid: remove it to let time run as asked again"
            } else if fluid.is_some_and(|f| !f.by_hand) {
                "It stays a fluid until tides stretch it by under 0.5%, with none of it torn away"
            } else if fluid.is_some() || self.fluid.is_none() {
                "Over 1% it becomes a fluid"
            } else {
                "Over 1% it would become a fluid, but another body is one: one at a time"
            };
            ui.label(RichText::new(note).small().color(theme::MUTED));
        }
        if self.pending.as_ref().is_some_and(|(n, _)| *n == name) {
            let doing = self.lab.status().unwrap_or_else(|| "starting".into());
            ui.label(RichText::new(format!("Becoming a fluid: {doing}")).weak());
            return;
        }
        let Some(fluid) = fluid else {
            if self.pending.is_none()
                && ui
                    .button("Model as a fluid (SPH)")
                    .on_hover_text(
                        "Replace its globe with a self-gravitating fluid of as many particles as this GPU steps smoothly; time runs at its pace while it is one",
                    )
                    .clicked()
            {
                self.fluid = None;
                self.pending = Some((name, true));
            }
            return;
        };
        if let Some(debris) = &fluid.debris {
            self.debris_rows(ui, debris);
        }
        let t = fluid.units.time;
        let since = f64::from(fluid.clock.time);
        egui::Grid::new("fluid").num_columns(2).show(ui, |ui| {
            ui.label(key("As a fluid"));
            ui.label(format!(
                "{} SPH particles, on the GPU",
                grouped(fluid.fluid().len())
            ));
            ui.end_row();
            let by: Vec<&str> = tides
                .iter()
                .take(worldline_render::MAX_TIDAL_BODIES)
                .map(|&(j, _)| self.simulation.bodies[j].name.as_str())
                .collect();
            ui.label(key("In the tides of"));
            ui.label(by.join(", "));
            ui.end_row();
            ui.label(key("Dynamical time"));
            ui.label(format!("{} (√(R³/GM))", duration(t)));
            ui.end_row();
            ui.label(key("A fluid for"));
            ui.label(format!(
                "{since:.2} dynamical times ({})",
                duration(since * t)
            ));
            ui.end_row();
            ui.label(key("Each step"));
            ui.label(format!(
                "{:.4} dynamical times ({})",
                fluid.clock.dt,
                duration(f64::from(fluid.clock.dt) * t)
            ));
            ui.end_row();
            ui.label(key("Frame rate"));
            ui.label(format!("{:.0} a second", 1.0 / self.frame_seconds));
            ui.end_row();
            if fluid.clock.outside > 0 {
                ui.label(key("Beyond its Hill sphere"));
                ui.label(format!(
                    "{} of its mass",
                    percent(f64::from(fluid.clock.outside) / fluid.fluid().len() as f64)
                ));
                ui.end_row();
            }
        });
        let trials: Vec<String> = self
            .lab
            .budget
            .trials
            .iter()
            .map(|(n, rate)| format!("{}: {rate:.0}", grouped(*n)))
            .collect();
        if !trials.is_empty() {
            ui.label(
                RichText::new(format!(
                    "This GPU's budget ({}) is the most particles stepped and drawn at {:.0} frames a second or more while time waits, leaving the physics its {} ms of a frame at {}. Frames a second: {}",
                    self.gpu_name,
                    crate::fluid::trial_rate(),
                    PHYSICS_BUDGET.as_millis(),
                    crate::fluid::SMOOTH,
                    trials.join(" · ")
                ))
                .small()
                .color(theme::MUTED),
            );
        }
        tag(
            ui,
            details::Kind::Model,
            "An n = 1 polytrope (P = Kρ², γ = 2): close to Jupiter's and Saturn's structure, a stand-in for other bodies'. Newtonian self-gravity, ideal gas, no spin",
        );
        tag(
            ui,
            details::Kind::Model,
            "Newtonian tides of the 4 bodies pulling hardest. Its center moves as the point did; what its shape does to its orbit is left out",
        );
        tag(
            ui,
            details::Kind::Model,
            "Time runs at the fluid's pace, a step a frame, while it is one",
        );
        tag(
            ui,
            details::Kind::Visual,
            "Each particle drawn as a sphere of half its smoothing length, colored by its internal energy (dark red cool, yellow-white hot); the half facing you cut away to show the inside",
        );
        if fluid.by_hand
            && ui
                .button("Back to a point body")
                .on_hover_text("Draw it as its globe again, and let time run as asked")
                .clicked()
        {
            self.fluid = None;
        }
    }

    /// A torn body's debris: how much is bound to what tore it, when it
    /// comes back and at what rate, against Rees's t^(−5/3).
    fn debris_rows(&self, ui: &mut egui::Ui, debris: &crate::fluid::Debris) {
        let Some(&first) = debris.times.first() else {
            ui.label(RichText::new(format!("None of its debris is bound to {}", debris.by)).weak());
            return;
        };
        ui.label(format!(
            "Its debris: {} of its mass bound to {}, the first back {} after its closest approach",
            percent(debris.bound),
            debris.by,
            any_time(first)
        ));
        ui.label(
            RichText::new(if debris.set {
                "Set as it passed twice the distance it became a fluid at, on its way out"
            } else {
                "Still being torn: worked out again every 2 s until it is twice as far as where it became a fluid"
            })
            .small()
            .color(theme::MUTED),
        );
        fallback_plot(ui, debris);
        if let Some((power, error)) = debris.power() {
            ui.label(format!(
                "From {} times that on, the rate falls as t^−{power:.2} ± {error:.2} (Rees: t^−5/3 = t^−1.67)",
                crate::fluid::LATE
            ));
        }
        ui.horizontal_wrapped(|ui| {
            let kind = details::Kind::Model;
            ui.label(RichText::new(kind.tag()).small().color(kind.color()));
            ui.label(
                RichText::new(format!(
                    "Each piece goes round {} on the orbit it has now (Newtonian), back after its period, as if alone with it",
                    debris.by
                ))
                .small(),
            );
        });
    }

    /// Inspector rows for the selected body's own clock: how much more
    /// slowly it runs now than a clock far away, and how far it has fallen
    /// behind since it started (see `clocks.rs`).
    fn clock_rows(&self, ui: &mut egui::Ui) {
        let body = &self.simulation.bodies[self.selected];
        let Some(clock) = self.simulation.clocks.get(&body.name) else {
            return;
        };
        let against = match &clock.frame {
            Some(hole) => format!("a clock far from {hole}, at rest with it"),
            None => "a clock far away".to_string(),
        };
        ui.label(key("Its clock"));
        ui.label(if clock.slowdown < 1e-3 {
            format!(
                "slow by {} against {against}: {} a day",
                significant(clock.slowdown),
                any_time(clock.slowdown * DAY)
            )
        } else {
            format!(
                "runs at {:.6} of the rate of {against}",
                1.0 - clock.slowdown
            )
        });
        ui.end_row();
        if clock.elapsed() > 0.0 {
            ui.label(key("Fallen behind"));
            ui.label(format!(
                "{} in {}: {:.10} as fast on average",
                any_time(clock.behind),
                any_time(clock.elapsed()),
                clock.shown() / clock.elapsed()
            ));
            ui.end_row();
        }
    }

    /// Inspector rows for a black hole: its spin and the tone it rings
    /// with, and for one that formed in a merger, what the merger
    /// radiated, its recoil and, for a real merger's pair, the hole that
    /// was measured. The horizon row is above, and ends its row.
    fn black_hole_rows(&self, ui: &mut egui::Ui, body: &Body) {
        let merger = self.simulation.merger(self.selected);
        // A black hole's spin is in its horizon's size.
        let spin = merger.map_or_else(|| horizon_spin(body.gm, body.radius), |m| m.spin);
        if let Some(m) = merger {
            ui.label(key("Spin"));
            ui.label(format!("{spin:.3} (numerical-relativity fit)"));
            ui.end_row();
            let total = body.gm + m.radiated_gm;
            ui.label(key("Its merger radiated"));
            ui.label(format!(
                "{} Suns ({:.1}%) as gravitational waves",
                significant(m.radiated_gm / GM_SUN),
                100.0 * m.radiated_gm / total
            ));
            ui.end_row();
            ui.label(key("Its kick"));
            ui.label(format!("{} km/s", significant(m.kick.length() / 1e3)));
            ui.end_row();
            if let Some(o) = self
                .simulation
                .entry(self.selected)
                .and_then(Entry::measured_remnant)
            {
                ui.label(key("The real one, measured"));
                ui.label(format!(
                    "{}, spin {}",
                    measured_suns(o.mass.value, o.mass.plus_minus),
                    o.spin
                        .map_or("not measured".to_string(), |s| s.0.to_string())
                ));
                ui.end_row();
            }
        }
        let unit = body.gm / (C * C);
        ui.label(key("Innermost stable orbit"));
        ui.label(if spin < 1e-6 {
            format!("{}: 6 GM/c²", length(6.0 * unit))
        } else {
            format!(
                "{} with its spin, {} against",
                length(innermost_stable_orbit(spin, true) * unit),
                length(innermost_stable_orbit(spin, false) * unit)
            )
        })
        .on_hover_text(
            "Closer in, no circular orbit lasts: a body circling there plunges into the hole (Bardeen, Press & Teukolsky 1972)",
        );
        ui.end_row();
        ui.label(key("Hawking temperature"));
        ui.label(format!(
            "{} K (theoretical)",
            significant(hawking::temperature(body.gm))
        ))
            .on_hover_text("Quantum effects at the horizon make a black hole glow, hotter the smaller it is (Hawking 1974). Theoretical: Hawking radiation has never been observed");
        ui.end_row();
        ui.label(key("Evaporates"));
        let life = any_time(hawking::lifetime(body.gm));
        if hawking::evaporating(body.gm) {
            ui.label(format!("in {life} (photons only)")).on_hover_text(
                "The textbook estimate: a black body at its temperature, as big as its horizon. Real holes also emit neutrinos, gravitons and, when hot enough, every lighter particle: 13 to 200 times faster (Carr et al. 2010)",
            );
        } else {
            ui.label("not today: colder than the cosmic background")
                .on_hover_text(format!(
                    "It absorbs more of the 2.725 K cosmic microwave background than it radiates. Alone in a colder universe, it would last {life}"
                ));
        }
        ui.end_row();
        let tone = ringdown(body.gm, spin);
        ui.label(key("Rings down at"));
        ui.label(format!(
            "{}, fading in {}",
            frequency(tone.frequency),
            any_time(tone.damping)
        ))
        .on_hover_text(
            "The tone a black hole rings with when disturbed, as after a merger (Berti, Cardoso & Will 2006)",
        );
    }

    /// Inspector rows for a real object added from the catalog: its
    /// published mass, size and spin.
    fn catalog_rows(&self, ui: &mut egui::Ui) {
        if self.simulation.merger(self.selected).is_some() {
            return;
        }
        let Some(o) = self.simulation.entry(self.selected).and_then(Entry::object) else {
            return;
        };
        // A neutron star that collapsed isn't the published star any more.
        if self.simulation.kind(self.selected) != Some(o.kind) {
            return;
        }
        ui.label(key("Published mass"));
        ui.label(measured_suns(o.mass.value, o.mass.plus_minus));
        ui.end_row();
        // A black hole's horizon is shown above, from its mass now.
        let basis = match o.radius_basis {
            RadiusBasis::Horizon => None,
            RadiusBasis::Measured => Some("measured"),
            RadiusBasis::Estimated => Some("estimated"),
            RadiusBasis::Assumed => Some("not measured: assumed"),
        };
        if let Some(basis) = basis {
            ui.label(key("Published radius"));
            ui.label(format!("{} ({basis})", length(o.radius.value)));
            ui.end_row();
        }
        if o.kind == ObjectKind::NeutronStar {
            let body = &self.simulation.bodies[self.selected];
            ui.label(key("Heaviest neutron star"));
            ui.label(format!(
                "{:.3} Suns (SLy equation of state)",
                sly_maximum().gm / GM_SUN
            ))
            .on_hover_text(
                "No static neutron star can be heavier: gravity overwhelms nuclear matter, and it collapses into a black hole (Tolman–Oppenheimer–Volkoff, with the SLy equation of state)",
            );
            ui.end_row();
            ui.label(key("Radius for its mass"));
            match sly_radius(body.gm) {
                Some(radius) => {
                    ui.label(format!("{} (SLy)", length(radius)));
                }
                // PSR J0740+6620: 2.08 ± 0.07 Suns.
                None => {
                    ui.label("none: above SLy's maximum").on_hover_text(
                        "Heavier than SLy allows, though within its measured uncertainty: a stiffer equation of state would fit it better",
                    );
                }
            }
            ui.end_row();
        }
        if o.kind == ObjectKind::WhiteDwarf {
            let body = &self.simulation.bodies[self.selected];
            ui.label(key("Chandrasekhar's limit"));
            ui.label(format!(
                "{:.3} Suns (ideal carbon–oxygen)",
                chandrasekhar_limit(CARBON_OXYGEN) / GM_SUN
            ))
            .on_hover_text(
                "The most a white dwarf can weigh: its electrons, nearing light speed, can't hold up more (Chandrasekhar 1931)",
            );
            ui.end_row();
            if let Some(radius) = radius_for(body.gm, CARBON_OXYGEN) {
                ui.label(key("Ideal radius for its mass"));
                ui.label(format!("{} (cold, carbon–oxygen)", length(radius)));
                ui.end_row();
            }
        }
        if o.kind == ObjectKind::BlackHole {
            ui.label(key("Spin"));
            ui.label(match o.spin {
                Some((a, true)) => format!("{a}"),
                Some((a, false)) => format!("at least {a}"),
                None => "not measured: taken as none".to_string(),
            });
        } else {
            // Light climbing out of its gravity loses energy.
            ui.label(key("Surface light"));
            ui.label(format!(
                "redshifted {} km/s by gravity",
                significant(gravitational_redshift(o.gm(), o.radius.value) / 1e3)
            ));
        }
        ui.end_row();
    }

    /// Below the rows, for a catalog object: where the real one is, and
    /// where its values were published.
    fn catalog_notes(&self, ui: &mut egui::Ui) {
        let entry = self.simulation.entry(self.selected);
        let object = if self.simulation.merger(self.selected).is_some() {
            entry.and_then(Entry::measured_remnant)
        } else {
            entry.and_then(Entry::object)
        };
        let Some(o) = object else {
            return;
        };
        let place = match o.distance {
            Some(Distance::Meters(d)) => format!("{}, {} away.", o.location, light_years(d)),
            Some(Distance::Redshift(z)) => format!("{}, at redshift {z}.", o.location),
            None => format!("{}.", o.location),
        };
        ui.add_space(4.0);
        ui.add(egui::Label::new(RichText::new(format!("The real one: {place}")).small()).wrap());
        ui.add(
            egui::Label::new(
                RichText::new(format!("Source: {}", o.source))
                    .small()
                    .color(theme::MUTED),
            )
            .wrap(),
        );
    }

    /// Which model computes the selected body, how strong gravity is
    /// where it is, and whether the model covers that: green within its
    /// range, amber approximate, red beyond it.
    fn model_indicator(&self, ui: &mut egui::Ui) {
        let simulation = &self.simulation;
        let index = self.selected;
        let held = simulation.held_by(index);
        let held_model = held.map(|h| {
            format!(
                "The exact spacetime of {} (Kerr), as a test body, with gravitational-wave losses at leading order (2.5PN), and Newtonian tides from everything else",
                simulation.bodies[h].name
            )
        });
        let model = match simulation.computed(index) {
            Computed::TopLevel if held_model.is_some() => held_model.as_deref().unwrap_or(""),
            Computed::TopLevel => {
                "Relativistic N-body gravity (Einstein–Infeld–Hoffmann), with each pair's second-order (2PN) terms and gravitational-wave losses (2.5PN and 3.5PN): it pulls on every massive body and they on it"
            }
            Computed::MoonSystem => {
                "Its planet's moon system: Newtonian, the planet's field, tides"
            }
            Computed::SmallMoonInDetail => {
                "Follows the major moons: Newtonian, the planet's field, tides"
            }
            Computed::SmallMoonOnMeanOrbit => "JPL's mean orbit, carried at its mean rates",
            Computed::Follower { forces: false } => {
                "Follows the Sun and planets: Newtonian, plus the Sun's relativistic term"
            }
            Computed::GalacticStar => {
                "Follows Sagittarius A*: its Newtonian pull plus its first relativistic (Schwarzschild) term"
            }
            Computed::Swarm => {
                "A swarm particle: carried on its orbit around what holds it, nudged by every other body (Wisdom–Holman)"
            }
            Computed::Follower { forces: true } => {
                "Follows the Sun and planets: Newtonian, the Sun's relativistic term, outgassing"
            }
        };
        let (green, amber, red) = (theme::WITHIN, theme::APPROXIMATE, theme::BEYOND);
        ui.add_space(4.0);
        ui.label(RichText::new("Model").strong());
        ui.label(RichText::new(model).small());
        let mut notes: Vec<(Color32, String)> = Vec::new();
        if let Some((regime, validity)) = simulation.validity(index) {
            let (color, verdict) = match validity {
                Validity::Within => (green, "within range"),
                Validity::Approximate => (amber, "approximate: higher orders show"),
                Validity::Beyond => (red, "beyond this model: gravity too strong"),
            };
            notes.push((
                color,
                format!(
                    "Gravity GM/rc² = {:.1e}, speed v/c = {:.1e}: {verdict}",
                    regime.epsilon, regime.beta
                ),
            ));
        }
        if simulation.computed(index) == Computed::SmallMoonOnMeanOrbit {
            notes.push((
                amber,
                "Approximate position: fly to its planet to compute it in detail".into(),
            ));
        }
        if let Some(waves) = simulation.gravitational_waves(index) {
            notes.push((
                green,
                format!(
                    "Gravitational waves: its orbit's period shrinks {} a year, and the pair merges in {} (Peters & Mathews, at leading order)",
                    short_time(-waves.shrink * JULIAN_YEAR),
                    long_time(waves.merging)
                ),
            ));
            if let Some(plunge) = waves.plunge {
                notes.push((
                    green,
                    format!(
                        "In the hole's exact spacetime, circling, it reaches the innermost stable orbit and plunges in {}",
                        long_time(plunge)
                    ),
                ));
            }
            if !waves.converging && waves.plunge.is_none() {
                notes.push((
                    red,
                    "Its last orbits: the post-Newtonian radiation reaction no longer converges here, so the motion is beyond this model, and the pair merges".into(),
                ));
            }
        }
        let body = &simulation.bodies[index];
        if is_black_hole(body) {
            notes.push((
                green,
                "Bodies it holds that are at least 35 times lighter move in its exact spacetime (Kerr), spin axis taken along the frame's z axis (the ecliptic's north pole)".into(),
            ));
        }
        if index == SUN && simulation.has_sun() && simulation.galactic_orbit().is_some() {
            notes.push((
                amber,
                "The whole solar system goes around the galaxy's center, in McMillan's (2017) model of the Milky Way's mass: smooth and symmetric about its axis, with no bar, spiral arms or stars one by one".into(),
            ));
        }
        if simulation.is_galactic_center(index) {
            notes.push((
                amber,
                "At the center of the galaxy's mass (McMillan's 2017 model), at rest: its own motion is under about 1 km/s (Reid & Brunthaler 2020). The galaxy's mass isn't felt inside this region: the model puts a tenth of a Sun inside S2's orbit, against its 4.3 million".into(),
            ));
        }
        if let Some(intruder) = simulation.intruder(index) {
            notes.push((
                amber,
                format!(
                    "{intruder} is inside this moon system: its collisions with moons aren't detected yet"
                ),
            ));
        }
        for (color, text) in notes {
            ui.horizontal_wrapped(|ui| {
                // A painted dot: the interface font has no circle glyph.
                let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                ui.painter().circle_filled(dot.center(), 3.5, color);
                ui.label(RichText::new(text).small());
            });
        }
        ui.add_space(4.0);
    }

    /// The top bar's sandbox tools: adding bodies, saving and loading.
    fn sandbox_controls(&mut self, ui: &mut egui::Ui) {
        let label = match self.tool {
            Some(entry) => format!("Adding: {}", entry.label),
            None => "Add a body".to_string(),
        };
        if ui
            .selectable_label(self.catalogue_open, label)
            .on_hover_text(
                "Open the catalogue: planets, stars, white dwarfs, neutron stars, black holes",
            )
            .clicked()
        {
            self.catalogue_open = !self.catalogue_open;
        }
        if self.tool.is_some() && ui.button("Done").clicked() {
            self.tool = None;
            self.launch = None;
        }
        if ui.button("Save").clicked() {
            let message = match sandbox::save(&mut self.simulation) {
                Ok(path) => format!("Saved {}", path.display()),
                Err(e) => format!("Couldn't save: {e}"),
            };
            self.status = Some((message, Instant::now()));
        }
        ui.menu_button("Load", |ui| {
            let saves = sandbox::saves();
            if saves.is_empty() {
                ui.label(RichText::new("No saves yet").color(theme::MUTED));
            }
            for (name, path) in saves.into_iter().take(20) {
                if ui.button(name).clicked() {
                    self.load(&path);
                    ui.close();
                }
            }
        });
        if let Some((message, since)) = &self.status {
            if since.elapsed() < Duration::from_secs(6) {
                ui.label(RichText::new(message).small().color(theme::MUTED));
            } else {
                self.status = None;
            }
        }
    }

    /// The names of the followed and selected bodies, to find them again
    /// after the list changes.
    fn attended_names(&self) -> (String, String) {
        let bodies = &self.simulation.bodies;
        (
            bodies[self.focus].name.clone(),
            bodies[self.selected].name.clone(),
        )
    }

    /// After an update: reports any collisions and what happened to the
    /// swarm (bodies swallowed, moons torn away), and finds the followed
    /// and selected bodies (`names`, from before the update) again, moving
    /// on to whatever absorbed them.
    fn after_events(&mut self, names: (String, String)) {
        let events = std::mem::take(&mut self.simulation.events);
        let notices = std::mem::take(&mut self.simulation.notices);
        if events.is_empty() && notices.is_empty() {
            return;
        }
        let survivor = |name: String| {
            events.iter().fold(name, |n, e| {
                if n == e.absorbed {
                    e.survivor.clone()
                } else {
                    n
                }
            })
        };
        self.reattend((survivor(names.0), survivor(names.1)));
        let mut parts = Vec::new();
        if let Some(last) = events.last() {
            let mut message = match &last.merger {
                Some(m) => format!(
                    "{} and {} merged into one black hole: {} Suns radiated as gravitational waves; it spins at {:.2} and recoils at {} km/s",
                    last.absorbed,
                    last.survivor,
                    significant(m.radiated_gm / GM_SUN),
                    m.spin,
                    significant(m.kick.length() / 1e3)
                ),
                None => format!(
                    "{} hit {} at {:.1} km/s and merged",
                    last.absorbed,
                    last.survivor,
                    last.speed / 1e3
                ),
            };
            if !last.freed.is_empty() {
                message += &format!(", freeing {}", last.freed.join(", "));
            }
            if events.len() > 1 {
                message += &format!(" ({} collisions)", events.len());
            }
            parts.push(message);
        }
        parts.extend(notices.last().cloned());
        self.status = Some((parts.join("; "), Instant::now()));
    }

    /// Finds the followed and selected bodies again by name; the Sun if
    /// they're gone.
    fn reattend(&mut self, (focus, selected): (String, String)) {
        self.focus = self.simulation.index_of(&focus).unwrap_or(SUN);
        self.selected = self.simulation.index_of(&selected).unwrap_or(self.focus);
        self.reveal = true;
    }

    /// The share of the circular speed bodies added start with: none at
    /// rest, all circling.
    fn circling(&self) -> f64 {
        if self.start_at_rest { 0.0 } else { 1.0 }
    }

    /// Adds a body from the command line: `kind` ("earth", "jupiter",
    /// "sun", or any catalogue entry's name, like "Sagittarius A*") `au`
    /// from body `around` (the Sun, usually), in the ecliptic, on the far
    /// side of it from where the x axis points, moving at the share
    /// `circling` of the circular speed there: on a circle at 1, on an
    /// ellipse falling in below.
    fn add_on_circle(&mut self, kind: &str, au: f64, around: usize, circling: f64) {
        let key = match kind {
            "earth" => "copy:Earth".to_string(),
            "jupiter" => "copy:Jupiter".to_string(),
            "sun" => "copy:Sun".to_string(),
            name => catalogue::catalogue()
                .iter()
                .find(|e| e.label.eq_ignore_ascii_case(name))
                .map_or_else(String::new, |e| e.key.clone()),
        };
        let Some(entry) = catalogue::entry(&key) else {
            eprintln!("worldline: nothing called `{kind}` in the catalogue");
            return;
        };
        let center = self.simulation.bodies[around].position;
        let position = center - DVec3::X * au * AU;
        self.place(Launch {
            entry,
            position,
            around,
            drag: position,
            circling,
        });
    }

    /// Adds what is being launched: its bodies around their center of
    /// mass, which moves at the launch velocity.
    fn place(&mut self, launch: Launch) {
        let names = self.attended_names();
        let entry = launch.entry;
        let mut velocity = launch.velocity(&self.simulation, self.camera.distance, entry.gm());
        // Something heavier than what it circles takes that body around
        // their common center of mass instead of dragging it off: each
        // gets its share of their relative motion.
        let around = &self.simulation.bodies[launch.around];
        let (gm, other) = (entry.gm(), around.gm);
        if gm > other {
            let relative = velocity - around.velocity;
            velocity = around.velocity + relative * (other / (gm + other));
            self.simulation
                .shift_velocity(launch.around, -relative * (gm / (gm + other)));
        }
        let bodies = entry.bodies(&self.simulation);
        let added: Vec<String> = bodies.iter().map(|b| b.name.clone()).collect();
        for body in bodies {
            let (p, v) = (body.position, body.velocity);
            self.simulation.add_body_near(
                body.at(launch.position + p).moving(velocity + v),
                &entry.key,
                launch.around,
            );
        }
        self.reattend(names);
        self.selected = self.simulation.index_of(&added[0]).unwrap_or(self.selected);
        self.reveal = true;
        self.status = Some((format!("Added {}", added.join(" and ")), Instant::now()));
    }

    /// Removes the selected body, if it can be.
    fn remove_selected(&mut self) {
        if !matches!(
            self.simulation.removal(self.selected),
            Removal::Allowed { .. }
        ) {
            return;
        }
        let (focus, removed) = self.attended_names();
        self.simulation.remove(self.selected);
        self.reattend((focus, removed.clone()));
        self.status = Some((format!("Removed {removed}"), Instant::now()));
    }

    /// Replaces the simulation with a saved one.
    fn load(&mut self, path: &std::path::Path) {
        let loaded = std::fs::read_to_string(path)
            .map_err(|e| e.to_string())
            .and_then(|text| Simulation::load(&text, self.simulation.speed));
        match loaded {
            Ok(mut simulation) => {
                let names = self.attended_names();
                simulation.paused = self.simulation.paused;
                self.simulation = simulation;
                self.drop_fluid();
                self.live = false;
                self.flight = None;
                self.launch = None;
                self.reattend(names);
                self.status = Some((
                    format!(
                        "Loaded {}",
                        path.file_stem().unwrap_or_default().to_string_lossy()
                    ),
                    Instant::now(),
                ));
            }
            Err(e) => self.status = Some((format!("Couldn't load: {e}"), Instant::now())),
        }
    }

    /// Inspector rows for the Sun: the solar system's orbit around the
    /// galaxy's center, in McMillan's (2017) model of the galaxy's mass.
    fn galaxy_rows(&self, ui: &mut egui::Ui) {
        if self.selected != SUN || !self.simulation.has_sun() {
            return;
        }
        let Some((position, velocity, survey)) = self.simulation.galactic_orbit() else {
            return;
        };
        let source = "McMillan's (2017) model of the Milky Way's mass (bulge, star and gas disks, dark-matter halo), from the Sun's measured place and motion: GRAVITY's distance to Sagittarius A* (2022), Sagittarius A*'s motion across the sky (Reid & Brunthaler 2020), the Sun's toward it (Schönrich, Binney & Dehnen 2010) and its height above the galaxy's midplane (Bennett & Bovy 2019)";
        let myr = 1e6 * JULIAN_YEAR;
        let years = |p: Option<f64>| p.map_or("—".to_string(), |p| format!("{:.0}", p / myr));
        ui.label(key("Around the galaxy"));
        ui.label(format!(
            "{:.3} kpc from its center, {:.1} km/s: once around in {:.0} million years",
            position.truncate().length() / KILOPARSEC,
            velocity.length() / 1e3,
            survey.azimuthal_period / myr
        ))
        .on_hover_text(source);
        ui.end_row();
        ui.label(key("Galactic orbit"));
        ui.label(format!(
            "{:.2} to {:.2} kpc out, in and out every {} million years",
            survey.r_min / KILOPARSEC,
            survey.r_max / KILOPARSEC,
            years(survey.radial_period)
        ))
        .on_hover_text(source);
        ui.end_row();
        ui.label(key("From the galactic plane"));
        ui.label(format!(
            "{:.1} pc {} now; up to {:.0} pc either side, through and back every {} million years",
            position.z.abs() / worldline_core::constants::PARSEC,
            if position.z >= 0.0 { "north" } else { "south" },
            survey.z_max / worldline_core::constants::PARSEC,
            years(survey.vertical_period)
        ))
        .on_hover_text(source);
        ui.end_row();
    }

    /// Inspector rows for the Sun's reach: sunlight and its travel time,
    /// and the solar wind, at the selected body; for the Sun itself, its
    /// output.
    fn sun_reach(&self, ui: &mut egui::Ui) {
        if !self.simulation.has_sun() {
            return;
        }
        let bodies = &self.simulation.bodies;
        let (sun, body) = (&bodies[SUN], &bodies[self.selected]);
        let wind = &self.simulation.wind;
        if self.selected == SUN {
            ui.label(key("Luminosity"));
            ui.label(format!("{SOLAR_LUMINOSITY:.4e} W"));
            ui.end_row();
            ui.label(key("Solar wind at Earth"));
            ui.label(format!(
                "{:.0} km/s, {:.1} protons/cm³ (2025 average)",
                wind.speed / 1e3,
                wind.density_at_1au / 1e6
            ));
            ui.end_row();
            return;
        }
        let offset = body.position - sun.position;
        let r = offset.length();
        ui.label(key("Sunlight"));
        ui.label(format!(
            "{:.1} W/m² ({:.3}× at 1 AU)",
            irradiance(r),
            irradiance(r) / irradiance(AU)
        ));
        ui.end_row();
        // From the Sun's surface, including the Shapiro delay.
        let travel = light_time(sun.gm, offset.normalize() * sun.radius, offset);
        ui.label(key("Sunlight left the Sun"));
        ui.label(format!("{} ago", duration(travel)));
        ui.end_row();
        let shell = &self.simulation.heliosphere;
        if r < shell.termination_shock * shell.shape(offset) {
            ui.label(key("Solar wind"));
            ui.label(format!(
                "{:.0} km/s, {:.3} protons/cm³",
                wind.speed / 1e3,
                wind.density(r) / 1e6
            ));
            ui.end_row();
            let sin_colatitude = self.simulation.rotation(SUN).map_or(1.0, |m| {
                m.spin_axis(self.simulation.julian_date())
                    .cross(offset / r)
                    .length()
            });
            ui.label(key("Its magnetic field"));
            ui.label(format!(
                "{:.1}° from the radial (Parker spiral)",
                wind.angle(r, sin_colatitude).to_degrees()
            ));
            ui.end_row();
        } else {
            ui.label(key("Solar wind"));
            ui.label(if r < shell.heliopause * shell.shape(offset) {
                "slowed in the heliosheath (past the termination shock)"
            } else {
                "none: outside the heliopause, in interstellar space"
            });
            ui.end_row();
        }
    }

    /// Inspector rows for the selected planet's magnetic field and
    /// magnetosphere.
    fn magnetism(&self, ui: &mut egui::Ui) {
        let bodies = &self.simulation.bodies;
        let body = &bodies[self.selected];
        if matches!(body.name.as_str(), "Venus" | "Mars") {
            ui.label(key("Magnetic field"));
            ui.label("no global field: the solar wind meets the upper atmosphere");
            ui.end_row();
            return;
        }
        let Some((dipole, model)) = &self.simulation.dipoles[self.selected] else {
            return;
        };
        ui.label(key("Magnetic field"));
        ui.label(format!(
            "{:.0} nT at the equator, tilted {:.1}° ({model})",
            dipole.equatorial_field() * 1e9,
            dipole.tilt().to_degrees()
        ));
        ui.end_row();
        if self.simulation.has_sun() {
            let r = (body.position - bodies[SUN].position).length();
            let pressure = self.simulation.flow_pressure_at_1au * (AU / r).powi(2);
            ui.label(key("Magnetopause"));
            ui.label(format!(
                "{:.1} radii sunward (pressure balance)",
                standoff(dipole, pressure) / dipole.radius
            ));
            ui.end_row();
        }
        let belts: Vec<String> = self
            .simulation
            .radiation_belts
            .iter()
            .filter(|b| b.planet == body.name)
            .map(|b| format!("{} L {}–{}", b.name, b.l_range.0, b.l_range.1))
            .collect();
        if !belts.is_empty() {
            ui.label(key("Radiation belts"));
            ui.label(belts.join(", "));
            ui.end_row();
        }
    }

    fn scene(&mut self, ui: &mut egui::Ui, frame: &eframe::Frame) {
        let (response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let viewport = response.rect;
        let render_state = frame.wgpu_render_state();
        let sky = render_state
            .filter(|_| self.options.starfield || self.options.accretion_disk)
            .and_then(|state| self.draw_sky(state, ui.ctx().pixels_per_point(), viewport));
        match sky {
            Some(texture) => {
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                painter.image(texture, viewport, uv, Color32::WHITE);
            }
            None => {
                painter.rect_filled(viewport, 0.0, Color32::from_rgb(3, 5, 12));
            }
        }

        if response.dragged() && self.tool.is_none() {
            let delta = response.drag_delta();
            self.camera.orbit(delta.x, delta.y);
        }
        let focus_radius = self.simulation.bodies[self.focus].radius;
        let min_distance = (MIN_DISTANCE_RADII * focus_radius).max(1e3);
        if response.hovered() && self.flight.is_none() {
            let scroll = ui.ctx().input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.camera.zoom(scroll, min_distance);
            }
        }
        self.camera.clamp_distance(min_distance);

        let globes_allowed = self.options.globes && render_state.is_some();
        let attention = view::Attention {
            focus: self.focus,
            selected: self.selected,
            pointer: response.hover_pos(),
            lensed: self
                .lensed
                .as_ref()
                .filter(|_| sky.is_some())
                .and_then(|name| self.simulation.index_of(name)),
        };
        let layout = view::layout(
            &self.camera,
            viewport,
            &self.simulation,
            globes_allowed,
            attention,
        );
        view::draw_under(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            &layout,
            attention,
        );
        if let Some(state) = render_state
            && let Some(texture) =
                self.draw_globes(state, ui.ctx().pixels_per_point(), viewport, &layout)
        {
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            painter.image(texture, viewport, uv, Color32::WHITE);
        }
        if self.options.shadows {
            view::draw_shadow_on_earth(
                &painter,
                viewport,
                &self.camera,
                &self.simulation,
                &layout,
                self.eclipse_path(),
            );
        }
        let drawn = view::draw_over(
            &painter,
            viewport,
            &self.camera,
            &self.simulation,
            self.options,
            &layout,
            attention,
        );
        let readouts = overlays::draw(
            &overlays::Scene {
                painter: &painter,
                viewport,
                camera: &self.camera,
                simulation: &self.simulation,
                layout: &layout,
                focus: self.focus,
                selected: self.selected,
                pointer: response.hover_pos(),
            },
            self.options.overlays,
        );
        overlays::draw_readouts(&painter, viewport, &readouts);
        if let Some(pointer) = response.hover_pos()
            && view::pick(&drawn, pointer).is_some()
        {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        self.dropping(&response, &painter, viewport);
        if let Some(entry) = self.tool {
            self.adding(ui, &response, viewport, entry);
            if let Some(launch) = &self.launch {
                launch.draw(&painter, &self.camera, viewport, &self.simulation);
            }
        } else if (response.clicked() || response.double_clicked())
            && let Some(pointer) = response.interact_pointer_pos()
            && let Some(i) = view::pick(&drawn, pointer)
        {
            self.selected = i;
            self.reveal = true;
            if response.double_clicked() {
                self.focus_on(i);
            }
        }
        if response.hovered() && ui.input(|i| i.key_pressed(egui::Key::Delete)) {
            self.remove_selected();
        }

        painter.text(
            viewport.left_bottom() + vec2(10.0, -10.0),
            Align2::LEFT_BOTTOM,
            if self.tool.is_some() {
                "Click to place in a circular orbit · drag to launch · Esc to stop adding"
            } else {
                "Drag to rotate · scroll to zoom · double-click a body to fly to it"
            },
            FontId::proportional(12.0),
            Color32::from_gray(140),
        );
    }

    /// Placing a body: press where it goes (on the plane of the body in
    /// focus, parallel to the ecliptic), drag to launch it, release to add
    /// it. A click without a drag puts it in a circular orbit.
    fn adding(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        viewport: Rect,
        entry: &'static Entry,
    ) {
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.launch.take().is_none() {
                self.tool = None;
            }
            return;
        }
        let plane = self.camera.target.z;
        let here = response
            .interact_pointer_pos()
            .or(response.hover_pos())
            .and_then(|p| self.camera.point_on_plane(p, viewport, plane));
        // It circles whatever dominates gravity where it's placed, not the
        // body the camera happens to follow.
        let around = here.map_or_else(|| self.launch_center(), |p| self.simulation.dominant_at(p));
        if response.drag_started()
            && let Some(position) = here
        {
            self.launch = Some(Launch {
                entry,
                position,
                around,
                drag: position,
                circling: self.circling(),
            });
        }
        if response.dragged()
            && let (Some(launch), Some(at)) = (&mut self.launch, here)
        {
            launch.drag = at;
        }
        if response.drag_stopped()
            && let Some(launch) = self.launch.take()
        {
            self.place(launch);
        }
        if response.clicked()
            && let Some(position) = here
        {
            self.place(Launch {
                entry,
                position,
                around,
                drag: position,
                circling: self.circling(),
            });
        }
    }

    /// What a body orbits when where it goes isn't known yet: the body in
    /// focus, if it has mass, or else its region's center.
    fn launch_center(&self) -> usize {
        if self.simulation.bodies[self.focus].gm > 0.0 {
            self.focus
        } else {
            self.simulation.region_center(self.focus)
        }
    }

    /// Drag and drop from the catalogue: while an entry is held over the
    /// view, shows where it would go and the orbit it would follow; when
    /// it is let go, adds it there on a circular orbit.
    fn dropping(&mut self, response: &egui::Response, painter: &egui::Painter, viewport: Rect) {
        let plane = self.camera.target.z;
        let position = response
            .ctx
            .pointer_hover_pos()
            .and_then(|p| self.camera.point_on_plane(p, viewport, plane));
        let Some(position) = position else {
            return;
        };
        let around = self.simulation.dominant_at(position);
        let launch = |entry: &'static Entry| Launch {
            entry,
            position,
            around,
            drag: position,
            circling: self.circling(),
        };
        if let Some(entry) = response.dnd_hover_payload::<&'static Entry>() {
            launch(*entry).draw(painter, &self.camera, viewport, &self.simulation);
        }
        if let Some(entry) = response.dnd_release_payload::<&'static Entry>() {
            let launch = launch(*entry);
            self.place(launch);
        }
    }

    /// The catalogue window: everything that can be added, by kind. Drag
    /// an entry into the view, or click it and then click in the view.
    /// Records the waves an observer on Earth would get from the selected
    /// body's pair, and opens their window.
    fn open_waves(&mut self) {
        let index = self.selected;
        let Some(pair) = self.simulation.gravitational_waves(index) else {
            return;
        };
        let (a, b) = (
            self.simulation.bodies[index].clone(),
            self.simulation.bodies[pair.partner].clone(),
        );
        let earth = self
            .simulation
            .index_of("Earth")
            .map_or(DVec3::ZERO, |i| self.simulation.bodies[i].position);
        let recording = waves::record(&a, &b, earth, waves::MAX_ORBITS);
        let sound = waves::sound(&recording);
        self.stop_sound();
        self.waves = Some(WaveView {
            names: format!("{} and {}", a.name, b.name),
            recording,
            sound,
        });
    }

    fn stop_sound(&mut self) {
        if let Some((_, sink)) = &self.audio {
            sink.stop();
        }
    }

    /// Plays the recorded waves' sound.
    fn play_sound(&mut self) {
        let Some(view) = &self.waves else {
            return;
        };
        if self.audio.is_none() {
            match rodio::OutputStream::try_default() {
                Ok((stream, handle)) => match rodio::Sink::try_new(&handle) {
                    Ok(sink) => self.audio = Some((stream, sink)),
                    Err(e) => self.status = Some((format!("No sound: {e}"), Instant::now())),
                },
                Err(e) => self.status = Some((format!("No sound: {e}"), Instant::now())),
            }
        }
        if let Some((_, sink)) = &self.audio {
            sink.stop();
            sink.append(rodio::buffer::SamplesBuffer::new(
                1,
                waves::RATE,
                view.sound.samples.clone(),
            ));
            sink.play();
        }
    }

    /// The path across Earth of the solar eclipse under way, if the finder
    /// has worked it out: from 3 hours before the umbra touches Earth to 3
    /// hours after it leaves.
    fn eclipse_path(&self) -> Option<&Path> {
        let now = self.simulation.time();
        self.finder.as_ref()?.found.iter().find_map(|found| {
            let path = found.path.as_ref()?;
            let (first, last) = (path.times.first()?, path.times.last()?);
            (now > first - 3.0 * 3600.0 && now < last + 3.0 * 3600.0).then_some(path)
        })
    }

    /// Opens the eclipse finder, searching from now if it isn't already.
    fn open_eclipses(&mut self) {
        self.eclipses_open = true;
        if self.finder.is_none() {
            self.finder = Some(Finder::start(&mut self.simulation));
        }
    }

    /// The eclipse finder's window: the eclipses found ahead, as they are
    /// found, each to go to.
    fn eclipses_window(&mut self, ctx: &egui::Context) {
        if !self.eclipses_open {
            return;
        }
        let Some(finder) = &mut self.finder else {
            return;
        };
        finder.poll();
        if !finder.done {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some(k) = self.go_to_when_found
            && (k < finder.found.len() || finder.done)
        {
            self.go_to_when_found = None;
            self.go_to_eclipse(k);
            return;
        }
        let Some(finder) = &self.finder else {
            return;
        };
        let scales = self.simulation.time_scales();
        let utc = |jd_tdb: f64| to_the_second(scales.utc(jd_tdb));
        let (mut open, mut go, mut again) = (true, None, false);
        egui::Window::new("Eclipses")
            .open(&mut open)
            .default_pos(pos2(340.0, 90.0))
            .default_width(560.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(format!(
                        "Every solar and lunar eclipse of the {} years from {} UTC, found by running a copy of this simulation ahead: each greatest where the axis of the Moon's shadow passes closest to Earth's center, or the Moon closest to the axis of Earth's, as NASA's Five Millennium Canon (Espenak & Meeus) defines it. Checked against the canon for 2026–2030: all 24 found, of the same types, the worst 5 s from its times.",
                        eclipse_finder::YEARS,
                        &utc(finder.from)[..16]
                    ))
                    .small(),
                );
                ui.horizontal(|ui| {
                    if finder.done {
                        ui.label(format!("{} found", finder.found.len()));
                    } else {
                        let share = (finder.searched - finder.from) / (finder.until - finder.from);
                        ui.add(egui::ProgressBar::new(share as f32).desired_width(120.0));
                        ui.label(format!(
                            "{} found; searched to {}",
                            finder.found.len(),
                            &utc(finder.searched)[..10]
                        ));
                    }
                    if ui
                        .button("Search again from now")
                        .on_hover_text("For a simulation that has changed since the search began")
                        .clicked()
                    {
                        again = true;
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                    egui::Grid::new("eclipses").num_columns(5).striped(true).show(ui, |ui| {
                        for heading in ["Greatest (UTC)", "Eclipse", "γ", "Magnitude", ""] {
                            ui.label(key(heading));
                        }
                        ui.end_row();
                        for (k, found) in finder.found.iter().enumerate() {
                            let (kind, magnitude, detail) = eclipse_row(&found.eclipse);
                            ui.label(RichText::new(utc(found.jd_tdb)).monospace())
                                .on_hover_text(format!("{} TDB", to_the_second(found.jd_tdb)));
                            ui.label(kind);
                            ui.label(format!("{:+.4}", eclipse_gamma(&found.eclipse)))
                                .on_hover_text("The least distance of the shadow's axis from Earth's center (solar), or of the Moon's center from the axis of Earth's shadow (lunar), in Earth's equatorial radii, positive north");
                            ui.label(format!("{magnitude:.4}")).on_hover_text(detail);
                            if ui.button("Go to").clicked() {
                                go = Some(k);
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        self.eclipses_open = open;
        if again {
            self.finder = Some(Finder::start(&mut self.simulation));
        }
        if let Some(k) = go {
            self.go_to_eclipse(k);
        }
    }

    /// Goes to an eclipse the finder found, at its greatest: paused, an
    /// hour a second once played, with the shadows on and the camera on
    /// Earth (solar) or the Moon (lunar).
    fn go_to_eclipse(&mut self, k: usize) {
        let Some(found) = self.finder.as_ref().and_then(|f| f.found.get(k)) else {
            return;
        };
        let solar = matches!(found.eclipse, Eclipse::Solar { .. });
        let (kind, _, _) = eclipse_row(&found.eclipse);
        let when = to_the_second(self.simulation.time_scales().utc(found.jd_tdb));
        match Simulation::load(&found.save, SPEEDS[HOUR].1) {
            Ok(mut simulation) => {
                simulation.paused = true;
                self.simulation = simulation;
                self.speed_index = HOUR;
                self.drop_fluid();
                self.live = false;
                self.flight = None;
                self.launch = None;
                self.options.shadows = true;
                let (name, radii) = if solar {
                    ("Earth", 3.0)
                } else {
                    ("Moon", 14.0)
                };
                if let Some(i) = self.simulation.index_of(name) {
                    self.fly_to(i, radii);
                }
                // For the Sun, look down on where the shadow's axis meets
                // the ground, or else at Earth from the Moon's side.
                let sim = &self.simulation;
                let find = |name: &str| sim.index_of(name);
                if solar
                    && let (Some(sun), Some(earth), Some(moon)) =
                        (find("Sun"), find("Earth"), find("Moon"))
                {
                    let state = |i: usize| State {
                        position: sim.bodies[i].position,
                        velocity: sim.bodies[i].velocity,
                    };
                    let (sun_seen, moon_seen) = apparent(state(sun), state(earth), state(moon));
                    let toward = sim
                        .earth_orientation(sim.julian_date())
                        .and_then(|o| {
                            let frame = o.body_to_ecliptic;
                            central_point(sun_seen, moon_seen, frame).map(|p| frame * p.position())
                        })
                        .unwrap_or(moon_seen)
                        .normalize();
                    self.camera.yaw = toward.y.atan2(toward.x);
                    self.camera.pitch = toward.z.asin();
                }
                // Out of the view's way; the finder keeps its list.
                self.eclipses_open = false;
                self.status = Some((
                    format!("{kind} eclipse at its greatest, {when} UTC"),
                    Instant::now(),
                ));
            }
            Err(e) => self.status = Some((format!("Couldn't go there: {e}"), Instant::now())),
        }
    }

    /// The window with a pair's gravitational waves: the strain at Earth
    /// over time, what it shows, and its sound.
    fn waves_window(&mut self, ctx: &egui::Context) {
        let Some(view) = &self.waves else {
            return;
        };
        let mut open = true;
        let mut play = false;
        let mut stop = false;
        let mut save = false;
        egui::Window::new(format!("Gravitational waves from {}", view.names))
            .id(egui::Id::new("gravitational waves"))
            .open(&mut open)
            .default_pos(pos2(340.0, 90.0))
            .default_width(660.0)
            .show(ctx, |ui| {
                let r = &view.recording;
                let n = r.times.len();
                let span = r.times[n - 1];
                // The inspiral's last sample, before any ringdown.
                let last = r
                    .final_hole
                    .as_ref()
                    .map_or(n - 1, |hole| r.times.partition_point(|&t| t <= hole.start) - 1);
                let (f0, f1) = (r.frequency[0], r.frequency[last]);
                ui.label(
                    RichText::new(format!(
                        "The strain h+ (the \"plus\" polarization) an observer on Earth would record, {} away: the leading-order (quadrupole) waveform, from a copy of the pair run ahead with Worldline's gravity.",
                        length(r.distance)
                    ))
                    .small(),
                );
                ui.label(
                    RichText::new(format!(
                        "{} of signal: the inspiral, from {} to {}{}.",
                        long_time(span),
                        frequency(f0),
                        frequency(f1),
                        r.final_hole.as_ref().map_or(String::new(), |hole| format!(
                            ", then the final hole's ringdown, {}",
                            frequency(hole.tone.frequency)
                        ))
                    ))
                    .small(),
                );
                // The tail, +4π x^(3/2) in the sweep, with x^(3/2) = πGmf/c³,
                // in percent.
                let tail = 400.0 * PI * PI * r.gm * f0 / C.powi(3);
                let tail = if tail >= 0.1 {
                    format!("{tail:.1}%")
                } else {
                    format!("{tail:.1e}%")
                };
                if r.held {
                    // The orbit is exact; the radiation reaction is the
                    // leading-order one.
                    ui.label(
                        RichText::new(
                            "The lighter moves in the heavier's exact spacetime (Kerr). Approximate: it loses energy at the leading-order (quadrupole) rate; close to the innermost stable orbit general relativity's differs by tens of percent.",
                        )
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                } else {
                    ui.label(
                        RichText::new(format!(
                            "Approximate: the sweep leaves out the tail (the waves scattering off the curved spacetime around the pair), which makes general relativity's {tail} faster at {}, and more at higher frequencies.",
                            frequency(f0)
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                }
                let rough = waves::rough_above(r.gm, r.nu);
                if !r.held && rough <= f1 {
                    let from = if rough <= f0 {
                        "From the start".to_string()
                    } else {
                        format!("Above {}", frequency(rough))
                    };
                    ui.label(
                        RichText::new(format!(
                            "{from}, the tail outgrows the last correction kept (the first post-Newtonian one), so the sweep is only rough there: general relativity's is faster."
                        ))
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                }
                let ending = if r.held {
                    "at the innermost stable orbit, where the plunge begins"
                } else {
                    "where the post-Newtonian description gives out, a few orbits before the merger"
                };
                if let Some(hole) = &r.final_hole {
                    ui.label(
                        RichText::new(format!(
                            "The inspiral ends {ending}. Then the final hole, {} Suns spinning at {:.2} (numerical-relativity fits), rings down at {}, fading in {} (Berti, Cardoso & Will 2006).",
                            significant(hole.gm / GM_SUN),
                            hole.spin,
                            frequency(hole.tone.frequency),
                            any_time(hole.tone.damping)
                        ))
                        .small(),
                    );
                    ui.label(
                        RichText::new(
                            "Not modeled: the plunge and merger between, where the real wave is loudest. The ringdown starts where the inspiral ends, at its strength.",
                        )
                        .small()
                        .color(Color32::from_rgb(240, 180, 80)),
                    );
                } else if r.reached_end {
                    ui.label(
                        RichText::new(format!("It ends {ending}."))
                            .small()
                            .color(theme::MUTED),
                    );
                }
                waveform_plot(ui, r);
                ui.horizontal(|ui| {
                    play = ui.button("▶ Play").clicked();
                    stop = ui.button("■ Stop").clicked();
                    save = ui.button("Save as WAV").clicked();
                    let how = if view.sound.speedup == 1.0 {
                        "in real time, as the strain itself would sound".to_string()
                    } else {
                        format!(
                            "{} times faster than real time, so its highest frequency sounds at 400 Hz",
                            significant(view.sound.speedup)
                        )
                    };
                    ui.label(
                        RichText::new(format!(
                            "{:.1} s of sound, {how}",
                            view.sound.samples.len() as f64 / f64::from(waves::RATE)
                        ))
                        .small()
                        .color(theme::MUTED),
                    );
                });
            });
        if play {
            self.play_sound();
        }
        if stop {
            self.stop_sound();
        }
        if save && let Some(view) = &self.waves {
            let path = std::env::current_dir()
                .unwrap_or_default()
                .join("worldline-gravitational-waves.wav");
            let message = match std::fs::write(&path, waves::wav(&view.sound)) {
                Ok(()) => format!("Saved {}", path.display()),
                Err(e) => format!("Couldn't save {}: {e}", path.display()),
            };
            self.status = Some((message, Instant::now()));
        }
        if !open {
            self.stop_sound();
            self.waves = None;
        }
    }

    fn catalogue_window(&mut self, ctx: &egui::Context) {
        let mut open = self.catalogue_open;
        egui::Window::new("Add a body")
            .open(&mut open)
            .default_pos(pos2(16.0, 64.0))
            .default_width(300.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(
                        "Drag one into the view, or pick it and click there. \
                         Drag in the view to launch it.",
                    )
                    .small()
                    .color(theme::MUTED),
                );
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Start").small().color(theme::MUTED));
                    ui.selectable_value(&mut self.start_at_rest, false, "circling")
                        .on_hover_text("On a circular orbit around the body in focus");
                    ui.selectable_value(&mut self.start_at_rest, true, "at rest")
                        .on_hover_text("Still, relative to the body in focus: they fall together");
                });
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(460.0)
                    .show(ui, |ui| {
                        for group in Group::ALL {
                            egui::CollapsingHeader::new(RichText::new(group.label()).strong())
                                .default_open(true)
                                .show(ui, |ui| {
                                    for entry in
                                        catalogue::catalogue().iter().filter(|e| e.group == group)
                                    {
                                        self.catalogue_entry(ui, entry);
                                    }
                                });
                        }
                    });
            });
        self.catalogue_open = open;
    }

    /// One catalogue entry: its name and a line on its mass and size. It
    /// can be dragged into the view, or clicked to pick it.
    fn catalogue_entry(&mut self, ui: &mut egui::Ui, entry: &'static Entry) {
        let picked = self.tool.is_some_and(|t| std::ptr::eq(t, entry));
        let id = egui::Id::new(("catalogue", &entry.key));
        let inner = ui.dnd_drag_source(id, entry, |ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
                    ui.painter().circle_filled(
                        dot.center(),
                        3.5,
                        catalogue::kind_color(entry.kind()),
                    );
                    let name = RichText::new(&entry.label);
                    ui.label(if picked {
                        name.color(theme::ACCENT)
                    } else {
                        name
                    });
                });
                ui.label(RichText::new(&entry.summary).small().color(theme::MUTED));
            });
        });
        let response = inner.response.on_hover_ui(|ui| {
            ui.set_max_width(320.0);
            if let Some(o) = entry.object() {
                ui.label(RichText::new(&o.location).strong());
                ui.label(RichText::new(&o.source).small());
            } else {
                ui.label(&entry.summary);
            }
        });
        if response.clicked() {
            self.tool = if picked { None } else { Some(entry) };
            self.launch = None;
        }
        ui.add_space(2.0);
    }

    /// Draws the sky on the GPU: the Milky Way and the bright stars, bent
    /// by the black hole whose Einstein ring, √(4GM/c²r) across, is widest
    /// from here, if it spans half a pixel or more.
    fn draw_sky(
        &mut self,
        state: &RenderState,
        pixels_per_point: f32,
        viewport: Rect,
    ) -> Option<TextureId> {
        let mut size = [
            (viewport.width() * pixels_per_point).round() as u32,
            (viewport.height() * pixels_per_point).round() as u32,
        ];
        let eye = self.camera.eye();
        let pixel = self.camera.fov_y / f64::from(size[1].max(1));
        let hole = self
            .simulation
            .bodies
            .iter()
            .filter(|b| is_black_hole(b))
            .map(|b| (b, (b.position - eye).length()))
            .max_by(|a, b| (a.0.gm / a.1).total_cmp(&(b.0.gm / b.1)))
            .filter(|(b, d)| {
                *d > 2.0 * b.gm / (C * C) && (4.0 * b.gm / (C * C * d)).sqrt() >= 0.5 * pixel
            });
        self.lensed = hole.map(|(body, _)| body.name.clone());
        // Close up, the rays are traced, from a camera turning with the
        // hole's frame dragging.
        let mut traced = None;
        match hole {
            Some((body, distance)) if distance * C * C / body.gm <= KERR_REACH => {
                self.lens = None;
                let spin = horizon_spin(body.gm, body.radius);
                let unit = body.gm / (C * C);
                traced = Some((
                    kerr_light::Camera::at(spin, (eye - body.position) / unit),
                    spin,
                ));
                let stale = self
                    .disk
                    .as_ref()
                    .is_none_or(|d| d.hole != body.name || d.gm != body.gm || d.spin != spin);
                if self.options.accretion_disk && stale {
                    self.disk = Some(DiskTable::new(body, spin));
                }
            }
            Some((body, distance)) => {
                // Worked out again once the camera has moved a thousandth
                // of its distance: the shadow's edge moves by as much.
                let stale = self.lens.as_ref().is_none_or(|l| {
                    l.hole != body.name
                        || (l.distance / distance - 1.0).abs() > 1e-3
                        || l.gm != body.gm
                });
                if stale {
                    let table = BendTable::new(body.gm, distance);
                    self.lens = Some(LensTable {
                        hole: body.name.clone(),
                        gm: body.gm,
                        distance,
                        edge: table.edge,
                        bending: table.bending.iter().map(|&b| b as f32).collect(),
                        toward: (body.position - eye).normalize(),
                    });
                }
                if let Some(lens) = &mut self.lens {
                    lens.toward = (body.position - eye).normalize();
                }
            }
            None => self.lens = None,
        }
        // Without the starfield, only a traced hole and its disk are drawn,
        // on black.
        if !self.options.starfield && traced.is_none() {
            return None;
        }
        // Traced rays cost a hundred steps or more each: at half the
        // screen's resolution, a pixel spans about what a texel of the sky
        // map does (1.5 × 10⁻³ rad at the usual field of view), so the sky
        // loses little.
        if traced.is_some() {
            size = size.map(|s| s.div_ceil(2));
        }
        let disk = self
            .disk
            .as_ref()
            .filter(|_| self.options.accretion_disk)
            .map(|d| KerrDisk {
                inner: d.inner as f32,
                outer: DISK_OUTER as f32,
                rings: &d.rings,
                colors: &self.disk_colors,
                ln_t_min: LN_T_MIN as f32,
                ln_t_step: ((LN_T_MAX - LN_T_MIN) / (DISK_COLORS - 1) as f64) as f32,
                exposure: d.exposure,
            });
        let kerr = traced.map(|(camera, spin)| KerrLens {
            camera: camera.place.as_vec3(),
            launch: camera.launch().as_mat4(),
            spin: spin as f32,
            capture: photon_orbits(spin).0 as f32,
            far: (KERR_FAR * camera.r) as f32,
            disk,
        });
        let (forward, _, up) = self.camera.basis();
        let view = SkyView {
            size,
            forward: forward.as_vec3(),
            up: up.as_vec3(),
            fov_y: self.camera.fov_y as f32,
            to_map: crate::sky::to_galactic().as_mat3(),
            exposure: if self.options.starfield {
                SKY_EXPOSURE
            } else {
                0.0
            },
            lens: self.lens.as_ref().map(|l| Lens {
                toward: l.toward.as_vec3(),
                edge: l.edge as f32,
                bending: &l.bending,
                x_min: BendTable::X_MIN as f32,
            }),
            kerr,
        };
        let gpu = self.sky.get_or_insert_with(|| GpuSky::new(state));
        gpu.draw(state, &view)
    }

    /// Draws the bodies that are big enough on screen as textured, lit,
    /// rotating globes on the GPU.
    /// The fluid's part of a frame that took `seconds`. The fluid bodies
    /// are made from is readied when first needed (see `fluid::Lab`); a
    /// body waiting to become a fluid becomes one once it is ready. The
    /// fluid feels the tides of the 4 bodies pulling hardest and steps
    /// while time runs. A giant planet made a fluid by tides turns back
    /// into a point once they stretch it by under 0.5%, if none of it is
    /// beyond its Hill sphere. A fluid whose body is gone goes with it.
    fn update_fluid(&mut self, state: &RenderState, ctx: &egui::Context, seconds: f64) {
        if let Some(fluid) = &self.fluid
            && self.simulation.index_of(&fluid.name).is_none()
        {
            self.fluid = None;
        }
        if self.gpu_name.is_empty() {
            let info = state.adapter.get_info();
            self.gpu_name = format!("{}, {:?}", info.name, info.backend);
        }
        // The trials wait for the app to settle: in its first seconds
        // (loading, the GPU clocking up) and during the camera's flight,
        // frames come slower.
        let settled = self.flight.is_none() && self.started.elapsed() > STARTUP;
        self.lab.advance(state, seconds, settled);
        if self.lab.busy() {
            ctx.request_repaint();
        }
        if let Some((name, by_hand)) = self.pending.clone() {
            self.lab.start();
            match self.simulation.index_of(&name) {
                None => self.pending = None,
                Some(i) => {
                    if let Some(template) = self.lab.template() {
                        let body = &self.simulation.bodies[i];
                        self.fluid = Some(FluidBody::new(
                            state,
                            &name,
                            (body.gm, body.radius),
                            template,
                            self.simulation.time(),
                            by_hand,
                        ));
                        self.pending = None;
                    }
                }
            }
        }
        let Some(i) = self
            .fluid
            .as_ref()
            .and_then(|f| self.simulation.index_of(&f.name))
        else {
            return;
        };
        let tides = self.tides_on(i);
        let bodies = &self.simulation.bodies;
        let body = &bodies[i];
        let strongest = tides.first().copied();
        let fluid = self.fluid.as_mut().expect("the fluid");
        let (l, t) = (fluid.units.length, fluid.units.time);
        // Once torn and past its closest approach to what pulls it
        // hardest, its debris' orbits around that body: worked out until
        // it is twice as far as where it became a fluid, by when they are
        // set (as in the GPU's check), and kept from then on. Later, the
        // first pieces come back round, and a period from the closest
        // approach no longer says when.
        if let Some((j, _)) = strongest {
            let other = &bodies[j];
            let distance = (body.position - other.position).length();
            let passed = fluid.track(&other.name, self.simulation.time(), distance);
            let torn = fluid.clock.outside as usize * 100 > fluid.fluid().len();
            let rt = tidal_radius(body.radius, body.gm, other.gm);
            let h2 = 1.0 + polytrope_love_number();
            let set = distance > 2.0 * distance_for(h2, rt, FLUID_STRETCH);
            let kept = fluid.debris.as_ref().is_some_and(|d| d.set);
            if passed && torn && !kept {
                fluid.measure_debris(
                    state,
                    (&other.name, other.gm),
                    body.position - other.position,
                    body.velocity - other.velocity,
                    set,
                );
            }
        }
        // Its Hill sphere around what pulls it hardest, in its radii.
        let outside = strongest.map_or(f32::MAX, |(j, _)| {
            let d = (bodies[j].position - body.position).length();
            (hill_radius(d, body.gm, bodies[j].gm) / l) as f32
        });
        if !fluid.by_hand
            && strongest.is_none_or(|(_, s)| s < POINT_STRETCH)
            && fluid.clock.outside == 0
        {
            self.status = Some((
                format!("{}'s tides have eased: it is a point again", body.name),
                Instant::now(),
            ));
            self.fluid = None;
            return;
        }
        let pulls: Vec<TidalBody> = tides
            .iter()
            .take(worldline_render::MAX_TIDAL_BODIES)
            .map(|&(j, _)| {
                let other = &bodies[j];
                let d = other.position - body.position;
                // Relative to it, the two pull each other together.
                let a = -d * ((other.gm + body.gm) / d.length().powi(3));
                TidalBody {
                    position: (d / l).as_vec3(),
                    velocity: ((other.velocity - body.velocity) * (t / l)).as_vec3(),
                    acceleration: (a * (t * t / l)).as_vec3(),
                    mass: (other.gm / body.gm) as f32,
                }
            })
            .collect();
        fluid.advance(
            state,
            self.simulation.time(),
            &pulls,
            outside,
            !self.simulation.paused,
        );
    }

    fn draw_globes(
        &mut self,
        state: &RenderState,
        pixels_per_point: f32,
        viewport: Rect,
        layout: &[OnScreen],
    ) -> Option<TextureId> {
        let eye = self.camera.eye();
        let bodies = &self.simulation.bodies;
        let light = self.simulation.light_source();
        let sun_direction = |body: &Body| {
            // No light at all if a black hole took the Sun's place.
            light.map_or(glam::Vec3::ZERO, |l| {
                (l - body.position).normalize_or_zero().as_vec3()
            })
        };
        // The body modeled as a fluid is drawn as its particles instead of
        // its globe; so is a body waiting to become one, as the fluid being
        // readied for it.
        let drawn: Option<(&str, &worldline_render::Fluid, f32)> = match &self.fluid {
            Some(fluid) => Some((&fluid.name, fluid.fluid(), fluid.energy)),
            None => self
                .pending
                .as_ref()
                .zip(self.lab.showing())
                .map(|((name, _), (fluid, energy))| (name.as_str(), fluid, energy)),
        };
        let fluid_index = drawn.and_then(|(name, ..)| self.simulation.index_of(name));
        let fluids: Vec<FluidDraw<'_>> = layout
            .iter()
            .filter(|item| item.globe && Some(item.index) == fluid_index)
            .filter_map(|item| {
                let (_, fluid, energy) = drawn?;
                let body = &bodies[item.index];
                Some(FluidDraw {
                    fluid,
                    center: (body.position - eye).as_vec3(),
                    scale: body.radius as f32,
                    energy,
                    sun_direction: sun_direction(body),
                })
            })
            .collect();
        let globes: Vec<Globe> = layout
            .iter()
            .filter(|item| item.globe && Some(item.index) != fluid_index)
            .map(|item| {
                let body = &bodies[item.index];
                // Relative to the camera, in double precision, before the
                // GPU's single precision takes over.
                let center = (body.position - eye).as_vec3();
                let orientation = self
                    .simulation
                    .orientation(item.index)
                    .map_or(Mat3::IDENTITY, |o| o.body_to_ecliptic.as_mat3());
                // True shape from the IAU models; mean radius if unknown.
                let radii = worldline_data::triaxial_radii(&body.name)
                    .map_or(DVec3::splat(body.radius), DVec3::from_array)
                    .as_vec3();
                let rings = (body.name == "Saturn").then(|| {
                    let profile = worldline_data::saturn_rings();
                    Rings {
                        inner: profile.inner as f32,
                        outer: profile.outer() as f32,
                    }
                });
                let atmosphere = worldline_data::atmosphere(&body.name).map(|a| Atmosphere {
                    height: a.height as f32,
                    scale_height: a.scale_height as f32,
                    rayleigh: DVec3::from_array(a.rayleigh).as_vec3(),
                });
                Globe {
                    name: body.name.clone(),
                    color: view::color(&self.simulation, item.index),
                    center,
                    radii,
                    orientation,
                    sun_direction: sun_direction(body),
                    emissive: self.simulation.shines(item.index),
                    night_glow: if body.name == "Earth" { 1.0 } else { 0.0 },
                    rings,
                    atmosphere,
                }
            })
            .collect();
        if globes.is_empty() && fluids.is_empty() {
            return None;
        }
        // The near clipping plane sits halfway to the closest surface, ring
        // or top of the atmosphere; for a fluid, half again past its radius,
        // where its outermost particles may reach.
        let nearest_surface = globes
            .iter()
            .map(|g| {
                let reach = g.radii.max_element().max(g.rings.map_or(0.0, |r| r.outer))
                    + g.atmosphere.map_or(0.0, |a| a.height);
                g.center.length() - reach
            })
            .chain(fluids.iter().map(|f| f.center.length() - 1.5 * f.scale))
            .fold(f32::INFINITY, f32::min);
        let (forward, _, up) = self.camera.basis();
        let view = View {
            size: [
                (viewport.width() * pixels_per_point).round() as u32,
                (viewport.height() * pixels_per_point).round() as u32,
            ],
            fov_y: self.camera.fov_y as f32,
            forward: forward.as_vec3(),
            up: up.as_vec3(),
            near: (0.5 * nearest_surface).max(1.0),
        };
        let gpu = self.gpu.get_or_insert_with(|| GpuGlobes::new(state));
        gpu.draw(state, view, &globes, &fluids)
    }
}

impl eframe::App for WorldlineApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame).as_secs_f64();
        self.frame_seconds += 0.05 * (elapsed.min(1.0) - self.frame_seconds);
        // Cap the step after a stall (e.g. dragging the window) at 0.1 s.
        let real_dt = elapsed.min(0.1);
        self.last_frame = now;
        // The fluid's steps go to the GPU first, so it works on them while
        // the computer works out the physics and the interface.
        if let Some(state) = frame.wgpu_render_state() {
            self.update_fluid(state, ui.ctx(), elapsed);
        }
        let names = self.attended_names();
        let limits = self.limits();
        let reached = if self.live && !self.simulation.paused {
            let now = self.clock_time();
            self.behind = now - self.simulation.time();
            let budget = if self.behind > 1.0 {
                CATCH_UP_BUDGET
            } else {
                PHYSICS_BUDGET
            };
            let reached = self.simulation.update_to(now, budget, &limits);
            self.behind = now - self.simulation.time();
            reached
        } else {
            self.simulation.update(real_dt, PHYSICS_BUDGET, &limits)
        };
        self.slowed = self.simulation.slowed_by.map(|k| {
            let approach = &limits.approaches[k];
            (approach.body.clone(), approach.by.clone())
        });
        if let Some(k) = reached {
            let approach = &limits.approaches[k];
            self.pending = Some((approach.body.clone(), false));
            self.status = Some((
                format!(
                    "{}'s tides stretch {} by 1%: it becomes a fluid",
                    approach.by, approach.body
                ),
                Instant::now(),
            ));
        }
        // Detail follows focus, and the focus can drift into a planet's
        // neighborhood (or out of it) as it moves.
        self.simulation.focus_detail_on(self.focus);
        self.after_events(names);
        self.aim_camera();

        egui::Panel::top("controls").show(ui, |ui| self.controls(ui));
        egui::Panel::right("inspector")
            .default_size(290.0)
            .show(ui, |ui| self.inspector(ui));
        egui::CentralPanel::no_frame().show(ui, |ui| self.scene(ui, frame));
        self.catalogue_window(ui.ctx());
        self.waves_window(ui.ctx());
        self.eclipses_window(ui.ctx());

        if !self.simulation.paused || self.flight.is_some() {
            ui.ctx().request_repaint();
        }
    }
}

/// An eclipse's kind in words, the magnitude to show (the umbral one for
/// the Moon, unless only the penumbra touches it) and what it means.
fn eclipse_row(eclipse: &Eclipse) -> (&'static str, f64, String) {
    match *eclipse {
        Eclipse::Solar { kind, magnitude, .. } => (
            match kind {
                SolarKind::Total => "Total solar",
                SolarKind::Annular => "Annular solar",
                SolarKind::Hybrid => "Hybrid solar",
                SolarKind::Partial => "Partial solar",
            },
            magnitude,
            "The share of the Sun's diameter the Moon covers where the eclipse is greatest; for a central one, the Moon's apparent diameter over the Sun's there".to_string(),
        ),
        Eclipse::Lunar { kind, penumbral, umbral, .. } => (
            match kind {
                LunarKind::Total => "Total lunar",
                LunarKind::Partial => "Partial lunar",
                LunarKind::Penumbral => "Penumbral lunar",
            },
            if kind == LunarKind::Penumbral { penumbral } else { umbral },
            format!(
                "The share of the Moon's diameter in Earth's umbra: {umbral:.4}; in its penumbra: {penumbral:.4}"
            ),
        ),
    }
}

/// An eclipse's γ.
fn eclipse_gamma(eclipse: &Eclipse) -> f64 {
    match *eclipse {
        Eclipse::Solar { gamma, .. } | Eclipse::Lunar { gamma, .. } => gamma,
    }
}

/// Direction of a body's orbital angular momentum around `center`.
fn orbit_normal(body: &worldline_core::Body, center: &worldline_core::Body) -> DVec3 {
    (body.position - center.position)
        .cross(body.velocity - center.velocity)
        .normalize()
}

/// Looking down on the inner solar system at an angle.
fn initial_camera() -> Camera {
    Camera {
        target: DVec3::ZERO,
        yaw: -FRAC_PI_2 - 0.4,
        pitch: 0.6,
        distance: 4.0 * AU,
        fov_y: 0.8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_keep_two_figures() {
        assert_eq!(percent(0.009713), "0.97%");
        assert_eq!(percent(0.04739), "4.7%");
        assert_eq!(percent(0.000_042), "0.0042%");
        assert_eq!(percent(0.15), "15%");
    }

    #[test]
    fn counts_have_their_thousands_set_apart() {
        assert_eq!(grouped(512), "512");
        assert_eq!(grouped(8144), "8,144");
        assert_eq!(grouped(131_072), "131,072");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }

    #[test]
    fn short_and_long_times_read_naturally() {
        assert_eq!(short_time(7.58e-5), "75.8 µs");
        assert_eq!(short_time(0.034), "34 ms");
        assert_eq!(long_time(301e6 * JULIAN_YEAR), "301 million years");
        assert_eq!(long_time(4.2e9 * JULIAN_YEAR), "4.2 billion years");
        assert_eq!(long_time(12.0 * DAY), "12 days");
        assert_eq!(long_time(3.5 * JULIAN_YEAR), "3.5 years");
    }

    #[test]
    fn durations_read_naturally() {
        assert_eq!(duration(52.7), "52.7 s");
        assert_eq!(duration(496.68), "8 min 16.7 s");
        assert_eq!(duration(4.0 * 3600.0 + 9.4 * 60.0), "4 h 9 min");
    }
}
