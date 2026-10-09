//! What an observer at Earth would record from a pair's gravitational
//! waves: the waveform, from a copy of the pair run ahead with the same
//! gravity, and the same wave as sound.

use std::f64::consts::{PI, TAU};

use worldline_core::compact::is_black_hole;
use worldline_core::constants::C;
use worldline_core::gravitational_waves::strain;
use worldline_core::gravity::{Relativistic, moves_in_spacetime_of, reaction_converges};
use worldline_core::integrator::{Ias15, advance};
use worldline_core::merger::{Ringdown, remnant, ringdown};
use worldline_core::{Body, DVec3, System};

/// Waveform samples per orbit.
const SAMPLES_PER_ORBIT: f64 = 64.0;

/// The longest recording, in orbits: enough for two neutron stars to merge
/// from 20 Hz, and for a slow pair to fill the sound.
pub const MAX_ORBITS: f64 = 3000.0;

/// Audio sample rate, Hz.
pub const RATE: u32 = 44_100;

/// The longest sound, s.
const MAX_SOUND: f64 = 20.0;

/// A waveform recorded at the observer.
pub struct Recording {
    /// Time of each sample, s from now.
    pub times: Vec<f64>,
    /// h₊ at each sample.
    pub plus: Vec<f64>,
    /// The wave's frequency there, twice the orbit's, Hz.
    pub frequency: Vec<f64>,
    /// How far the observer is, m.
    pub distance: f64,
    /// The pair's total gravitational parameter G(m₁ + m₂), m³/s².
    pub gm: f64,
    /// The pair's symmetric mass ratio ν = m₁m₂/(m₁ + m₂)².
    pub nu: f64,
    /// Whether the lighter moves in the heavier's exact spacetime (see
    /// `worldline_core::gravity::moves_in_spacetime_of`).
    pub held: bool,
    /// Whether the pair reached the end of its post-Newtonian inspiral
    /// before the recording ended: a few orbits before it merges.
    pub reached_end: bool,
    /// For two black holes that reached it: the final hole, whose
    /// ringdown ends the recording.
    pub final_hole: Option<FinalHole>,
}

/// The hole two black holes merge into, as numerical relativity's fits
/// say, and how it rings down.
pub struct FinalHole {
    /// Its gravitational parameter, m³/s².
    pub gm: f64,
    /// Its spin.
    pub spin: f64,
    /// Its fundamental ringdown.
    pub tone: Ringdown,
    /// When its ringdown starts in the recording: where the inspiral ends,
    /// s from now.
    pub start: f64,
}

/// Runs a copy of the pair `a`, `b` ahead with Worldline's post-Newtonian
/// gravity, by itself, and records the strain an observer at `observer`
/// sees (the leading-order quadrupole waveform), for `max_orbits` orbits
/// or until the post-Newtonian description gives out, a few orbits before
/// the merger: when the radiation reaction stops converging (its 3.5PN
/// correction outgrows its leading term, near GM/rc² ≈ 0.11), when the
/// pair reaches the innermost stable orbit of a test body,
/// x = (G m Ω / c³)^(2/3) = 1/6, or when they touch. Run on past that, the
/// equations can pump the orbit eccentric or fling the pair apart.
///
/// Two black holes that get there go on to merge, and the recording ends
/// with the final hole ringing down: its fundamental tone, fading over ten
/// damping times. The plunge and merger between, where the real wave is
/// loudest, aren't modeled; the ringdown starts where the inspiral ends, at
/// its strength and phase.
pub fn record(a: &Body, b: &Body, observer: DVec3, max_orbits: f64) -> Recording {
    let gm = a.gm + b.gm;
    let mut system = System::new(vec![a.clone(), b.clone()]);
    let center = (a.position * a.gm + b.position * b.gm) / gm;
    let toward = observer - center;
    let distance = toward.length();
    let touching = a.radius + b.radius;
    // A light body in a black hole's exact spacetime circles down to the
    // innermost stable orbit: the post-Newtonian reaction doesn't apply.
    let held = moves_in_spacetime_of(a, b) || moves_in_spacetime_of(b, a);
    let gravity = Relativistic::default();
    let mut ias = Ias15::new();
    let mut recording = Recording {
        times: Vec::new(),
        plus: Vec::new(),
        frequency: Vec::new(),
        distance,
        gm,
        nu: a.gm * b.gm / (gm * gm),
        held,
        reached_end: false,
        final_hole: None,
    };
    let mut orbits = 0.0;
    loop {
        let (p, q) = (&system.bodies[0], &system.bodies[1]);
        let x = p.position - q.position;
        let v = p.velocity - q.velocity;
        let omega = x.cross(v).length() / x.length_squared();
        recording.times.push(system.time());
        recording
            .plus
            .push(strain(&system.bodies, toward, distance).plus);
        recording.frequency.push(omega / PI);
        let parameter = (gm * omega / C.powi(3)).powf(2.0 / 3.0);
        if parameter >= 1.0 / 6.0
            || x.length() <= touching
            || (!held && !reaction_converges(&system.bodies[0], &system.bodies[1]))
        {
            recording.reached_end = true;
            break;
        }
        if orbits >= max_orbits {
            break;
        }
        advance(
            &mut system,
            &gravity,
            &mut ias,
            TAU / omega / SAMPLES_PER_ORBIT,
        );
        orbits += 1.0 / SAMPLES_PER_ORBIT;
    }
    if recording.reached_end && is_black_hole(a) && is_black_hole(b) {
        ring_down(&mut recording, a.gm, b.gm);
    }
    recording
}

/// Ends the recording of black holes of gravitational parameters `gm1`
/// and `gm2` with the final hole's ringdown, continuing the inspiral's
/// last strength and phase.
fn ring_down(recording: &mut Recording, gm1: f64, gm2: f64) {
    let fit = remnant(gm1, gm2);
    let gm = (gm1 + gm2) * (1.0 - fit.radiated);
    let tone = ringdown(gm, fit.spin);
    let n = recording.plus.len();
    let last_orbit = n.saturating_sub(SAMPLES_PER_ORBIT as usize);
    let envelope = recording.plus[last_orbit..]
        .iter()
        .fold(0.0f64, |m, h| m.max(h.abs()));
    let (start, h) = (recording.times[n - 1], recording.plus[n - 1]);
    let rising = n > 1 && h > recording.plus[n - 2];
    let mut phase = (h / envelope).clamp(-1.0, 1.0).acos();
    if rising {
        phase = -phase;
    }
    let omega = TAU * tone.frequency;
    let dt = 1.0 / (tone.frequency * SAMPLES_PER_ORBIT);
    let samples = (10.0 * tone.damping / dt).ceil() as usize;
    for k in 1..=samples {
        let t = k as f64 * dt;
        recording.times.push(start + t);
        recording
            .plus
            .push(envelope * (-t / tone.damping).exp() * (omega * t + phase).cos());
        recording.frequency.push(tone.frequency);
    }
    recording.final_hole = Some(FinalHole {
        gm,
        spin: fit.spin,
        tone,
        start,
    });
}

/// The wave frequency (Hz) above which the simulated chirp is only rough:
/// where the first term it leaves out, the tail (+4π x^(3/2) in the
/// sweep), outgrows the last one it keeps, the first post-Newtonian
/// correction (−(743/336 + 11ν/4) x), with x = (πGmf/c³)^(2/3). Below it
/// the simulated sweep is right to about the tail's size; above it the
/// series no longer describes the sweep. `gm` is G(m₁ + m₂) and `nu` the
/// symmetric mass ratio.
pub fn rough_above(gm: f64, nu: f64) -> f64 {
    // 4π x^(3/2) = k x at x^(1/2) = k/(4π), and πGmf/c³ = x^(3/2).
    let k = 743.0 / 336.0 + 11.0 / 4.0 * nu;
    (k / (4.0 * PI)).powi(3) * C.powi(3) / (PI * gm)
}

/// A recording turned into sound.
pub struct Sound {
    /// Mono samples at [`RATE`], peaking at ±0.8.
    pub samples: Vec<f32>,
    /// How many times faster than real time it plays (1 for real time).
    pub speedup: f64,
}

/// The wave as sound. If its frequencies are already in the range we hear
/// well (20 Hz to 4 kHz), it plays in real time, as the strain itself
/// would sound; otherwise it is sped up (or slowed) so its highest
/// frequency sounds at 400 Hz. At most the last 20 s are kept.
pub fn sound(recording: &Recording) -> Sound {
    let (low, high) = recording
        .frequency
        .iter()
        .fold((f64::INFINITY, 0.0f64), |(lo, hi), &f| {
            (lo.min(f), hi.max(f))
        });
    let speedup = if low >= 20.0 && high <= 4000.0 {
        1.0
    } else {
        400.0 / high
    };
    let end = *recording.times.last().expect("a recording");
    let seconds = (end / speedup).min(MAX_SOUND);
    let start = end - seconds * speedup;
    let count = (seconds * f64::from(RATE)) as usize;
    let mut samples: Vec<f32> = Vec::with_capacity(count);
    let mut k = recording.times.partition_point(|&t| t < start).max(1);
    for i in 0..count {
        let t = start + i as f64 / f64::from(RATE) * speedup;
        while k < recording.times.len() - 1 && recording.times[k] < t {
            k += 1;
        }
        let (t0, t1) = (recording.times[k - 1], recording.times[k]);
        let (h0, h1) = (recording.plus[k - 1], recording.plus[k]);
        let s = ((t - t0) / (t1 - t0)).clamp(0.0, 1.0);
        samples.push((h0 + s * (h1 - h0)) as f32);
    }
    let peak = samples.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let fade = (0.01 * f64::from(RATE)) as usize;
    let n = samples.len();
    for (i, s) in samples.iter_mut().enumerate() {
        let edge = i.min(n - 1 - i);
        let gain = if edge < fade {
            edge as f32 / fade as f32
        } else {
            1.0
        };
        *s = if peak > 0.0 {
            0.8 * *s / peak * gain
        } else {
            0.0
        };
    }
    Sound { samples, speedup }
}

/// The sound as a 16-bit mono WAV file.
pub fn wav(sound: &Sound) -> Vec<u8> {
    let data = (sound.samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data.to_le_bytes());
    for s in &sound.samples {
        out.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::constants::{AU, GM_SUN, PARSEC};
    use worldline_core::gravity::relative_acceleration;
    use worldline_core::orbit::two_body_system;

    /// Two neutron stars on a circular orbit whose wave is near
    /// `frequency`, at the circular speed of the relativistic equations
    /// (a Newtonian circle would be visibly eccentric this close).
    fn neutron_stars(frequency: f64) -> (Body, Body) {
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let gm = gm1 + gm2;
        let nu = gm1 * gm2 / (gm * gm);
        let omega = PI * frequency;
        let r = (gm / (omega * omega)).cbrt();
        let x = DVec3::new(r, 0.0, 0.0);
        let mut speed = (gm / r).sqrt();
        for _ in 0..20 {
            let a = relative_acceleration(gm, nu, x, DVec3::new(0.0, speed, 0.0));
            speed = (r * -(a.newtonian + a.first + a.second).x).sqrt();
        }
        let v = DVec3::new(0.0, speed, 0.0);
        (
            Body::new("one", gm1, 1.2e4)
                .at(x * gm2 / gm)
                .moving(v * gm2 / gm),
            Body::new("two", gm2, 1.2e4)
                .at(-x * gm1 / gm)
                .moving(-v * gm1 / gm),
        )
    }

    #[test]
    fn neutron_stars_chirp_to_their_last_orbits_and_play_in_real_time() {
        // From a 200 Hz wave, two neutron stars spiral in within a second:
        // the recording reaches the end of the post-Newtonian inspiral, the
        // frequency and the strain rise steadily, and the sound plays in
        // real time at the same frequencies (its zero crossings match the
        // wave's).
        let (a, b) = neutron_stars(200.0);
        let observer = DVec3::new(0.0, 0.0, 40e6 * PARSEC);
        let recording = record(&a, &b, observer, MAX_ORBITS);
        assert!(recording.reached_end);
        let n = recording.times.len();
        let (f0, f1) = (recording.frequency[0], recording.frequency[n - 1]);
        let low = recording
            .frequency
            .iter()
            .fold(f64::INFINITY, |m, &f| m.min(f));
        println!(
            "{:.3} s, {f0:.0} Hz to {f1:.0} Hz (lowest {low:.0} Hz), {} samples",
            recording.times[n - 1],
            n
        );
        assert!(f1 > 3.0 * f0);
        // A chirp: the frequency never falls by more than an orbit's
        // wobble, and stays above where it started.
        assert!(low > 0.95 * f0);
        // The strain grows as f^(2/3) at leading order; the next order is
        // x = (π G m f / c³)^(2/3) of it, about 0.1 at the end.
        let peak = |h: &[f64]| h.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        let growth = peak(&recording.plus[n - 200..]) / peak(&recording.plus[..200]);
        let expected = (f1 / f0).powf(2.0 / 3.0);
        let x = (PI * (a.gm + b.gm) * f1 / C.powi(3)).powf(2.0 / 3.0);
        println!("strain grows {growth:.3} times; f^(2/3) says {expected:.3} (x = {x:.3})");
        assert!((growth / expected - 1.0).abs() < x);

        let sound = sound(&recording);
        assert_eq!(sound.speedup, 1.0);
        let crossings = |values: &mut dyn Iterator<Item = f64>| {
            let mut count: usize = 0;
            let mut last = 0.0f64;
            for v in values {
                if v != 0.0 && last != 0.0 && (v < 0.0) != (last < 0.0) {
                    count += 1;
                }
                if v != 0.0 {
                    last = v;
                }
            }
            count
        };
        let wave = crossings(&mut recording.plus.iter().copied());
        let heard = crossings(&mut sound.samples.iter().map(|&s| f64::from(s)));
        println!("zero crossings: wave {wave}, sound {heard}");
        assert!(wave.abs_diff(heard) <= 2);
    }

    #[test]
    fn the_chirp_turns_rough_where_the_tail_outgrows_the_first_correction() {
        // Two neutron stars of 1.4 and 1.3 Suns: the tail catches up with
        // the first correction near 290 Hz; for GW150914's black holes,
        // 25 times heavier, below 20 Hz, so their whole recorded chirp is
        // rough.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let gm = gm1 + gm2;
        let nu = gm1 * gm2 / (gm * gm);
        let f = rough_above(gm, nu);
        let x = (PI * gm * f / C.powi(3)).powf(2.0 / 3.0);
        let tail = 4.0 * PI * x.powf(1.5);
        let kept = (743.0 / 336.0 + 11.0 / 4.0 * nu) * x;
        println!(
            "neutron stars: rough above {f:.1} Hz (x = {x:.4}: tail {tail:.5}, first correction {kept:.5})"
        );
        assert!((tail / kept - 1.0).abs() < 1e4 * f64::EPSILON);
        let (gm1, gm2) = (35.6 * GM_SUN, 30.6 * GM_SUN);
        let holes = rough_above(gm1 + gm2, gm1 * gm2 / ((gm1 + gm2) * (gm1 + gm2)));
        println!("GW150914's black holes: rough above {holes:.1} Hz");
        assert!(holes < 20.0);
    }

    #[test]
    fn black_holes_ring_down_after_their_inspiral() {
        // GW150914's holes from a 30 Hz wave: within a second the
        // inspiral ends, and the final hole rings down at its fundamental
        // tone, which the recording's zero crossings must show (two per
        // cycle, within one at each end), joined on to the inspiral without
        // a jump larger than one sample's change at the ringdown's pace.
        let (a, b) = worldline_core::gravity::circular_pair(
            Body::new("one", 35.6 * GM_SUN, 2.0 * 35.6 * GM_SUN / (C * C)),
            Body::new("two", 30.6 * GM_SUN, 2.0 * 30.6 * GM_SUN / (C * C)),
            30.0,
        );
        let recording = record(&a, &b, DVec3::new(0.0, 0.0, 400e6 * PARSEC), MAX_ORBITS);
        let hole = recording.final_hole.as_ref().expect("a ringdown");
        let fit = remnant(a.gm, b.gm);
        assert_eq!(
            hole.tone,
            ringdown((a.gm + b.gm) * (1.0 - fit.radiated), fit.spin)
        );
        let first = recording.times.partition_point(|&t| t <= hole.start);
        let ring = &recording.plus[first..];
        let crossings = ring
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        let span = recording.times[recording.times.len() - 1] - hole.start;
        let expected = 2.0 * hole.tone.frequency * span;
        println!(
            "final hole {:.2} Suns, spin {:.3}: rings at {:.1} Hz for {:.1} ms, {crossings} zero crossings ({expected:.1} expected)",
            hole.gm / GM_SUN,
            hole.spin,
            hole.tone.frequency,
            span * 1e3
        );
        assert!((crossings as f64 - expected).abs() <= 2.0);
        // The ringdown starts at the inspiral's last strength, its peak over
        // the last orbit; a sample later it can have changed by at most
        // that times ω dt = 2π/64.
        let last_orbit = &recording.plus[first - SAMPLES_PER_ORBIT as usize..first];
        let strength = last_orbit.iter().fold(0.0f64, |m, h| m.max(h.abs()));
        let step = TAU / SAMPLES_PER_ORBIT;
        assert!((ring[0] - recording.plus[first - 1]).abs() <= strength * step);
    }

    #[test]
    fn a_slow_pair_is_sped_up_into_hearing() {
        // Two neutron stars a million kilometers apart: their wave is at a
        // millihertz. The sound is sped up so the highest frequency sounds at
        // 400 Hz, and lasts at most 20 s.
        let (gm1, gm2) = (1.4 * GM_SUN, 1.3 * GM_SUN);
        let s = two_body_system(
            Body::new("one", gm1, 1.2e4),
            Body::new("two", gm2, 1.2e4),
            1e9,
            0.3,
        );
        let recording = record(&s.bodies[0], &s.bodies[1], DVec3::new(AU, 0.0, 0.0), 300.0);
        assert!(!recording.reached_end);
        let sound = sound(&recording);
        let high = recording.frequency.iter().fold(0.0f64, |m, &f| m.max(f));
        assert!((sound.speedup * high - 400.0).abs() < 1e-9);
        assert!(sound.samples.len() as f64 <= 20.0 * f64::from(RATE) + 1.0);
        println!(
            "sped up {:.2e} times, {:.1} s of sound",
            sound.speedup,
            sound.samples.len() as f64 / f64::from(RATE)
        );
    }

    #[test]
    fn the_wav_file_is_well_formed() {
        let sound = Sound {
            samples: vec![0.0, 0.5, -0.5, 1.0],
            speedup: 1.0,
        };
        let bytes = wav(&sound);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..16], b"WAVEfmt ");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(bytes.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), RATE);
        assert_eq!(i16::from_le_bytes([bytes[50], bytes[51]]), 32767);
    }
}
