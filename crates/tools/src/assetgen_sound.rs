//! Sound synthesizer of `assetgen` (D-034). Makes the game sounds with code.
//!
//! Parts: oscillators, noise, envelopes, one-pole filters. The code uses only `+ - * /`, `floor`,
//! and casts on `f32`. These are exact IEEE operations, so Windows and Linux give the same bytes.
//! The sine is a parabola, not `sin`. The envelopes are polynomials, not `exp`. The WAV writer is
//! our own: 16-bit mono PCM.

use super::rng_for;
use base::rng::Pcg32;

/// Sample rate of every sound.
pub const SOUND_RATE: u32 = 44_100;

/// Peak level of every sound: 20,000 of 32,767 is about -4.3 dBFS. The limit is 23,000 (-3 dBFS).
const PEAK: f32 = 20_000.0;
/// Fade in at the start, in samples (3 ms).
const FADE_IN: usize = 132;
/// Fade out at the end, in samples (6 ms).
const FADE_OUT: usize = 264;

/// A named sound: the name is the file stem.
pub struct Sound {
    /// File stem, for example `alert_info`.
    pub name: &'static str,
    /// 16-bit mono samples at `SOUND_RATE`.
    pub samples: Vec<i16>,
}

// ---------------------------------------------------------------- parts

/// A sine from a parabola. `phase` is in cycles. Result is in -1..1.
fn sine(phase: f32) -> f32 {
    let q = phase - phase.floor();
    let x = q * 2.0;
    if x < 1.0 {
        4.0 * x * (1.0 - x)
    } else {
        let y = x - 1.0;
        -4.0 * y * (1.0 - y)
    }
}

/// A saw wave. `phase` is in cycles. Result is in -1..1.
fn saw(phase: f32) -> f32 {
    let q = phase - phase.floor();
    q * 2.0 - 1.0
}

/// A square wave. Result is -1 or 1.
fn square(phase: f32) -> f32 {
    let q = phase - phase.floor();
    if q < 0.5 { 1.0 } else { -1.0 }
}

/// White noise in -1..1.
fn noise(rng: &mut Pcg32) -> f32 {
    (rng.next_u32() >> 8) as f32 / 8_388_608.0 - 1.0
}

/// A decay from 1 to 0 over `t` in 0..1, with the given power (1 to 3).
fn decay(t: f32, power: u32) -> f32 {
    let r = (1.0 - t).max(0.0);
    let mut v = 1.0;
    for _ in 0..power {
        v *= r;
    }
    v
}

/// A gate with a linear ramp of `ramp` seconds at both ends: 1 inside, 0 outside `[0, len]`.
fn gate(t: f32, len: f32, ramp: f32) -> f32 {
    if t < 0.0 || t > len {
        0.0
    } else {
        (t / ramp).min(1.0).min(((len - t) / ramp).min(1.0))
    }
}

/// A one-pole low-pass filter. `k` in 0..1: a small value is a dark sound.
struct LowPass {
    k: f32,
    y: f32,
}

impl LowPass {
    fn new(k: f32) -> Self {
        Self { k, y: 0.0 }
    }
    fn next(&mut self, x: f32) -> f32 {
        self.y += self.k * (x - self.y);
        self.y
    }
}

/// An oscillator with a changing pitch: it adds the phase step for each sample.
struct Osc {
    phase: f32,
}

impl Osc {
    fn new() -> Self {
        Self { phase: 0.0 }
    }
    /// Advance by one sample at `hz` and return the phase in cycles.
    fn step(&mut self, hz: f32) -> f32 {
        self.phase += hz / SOUND_RATE as f32;
        self.phase -= self.phase.floor();
        self.phase
    }
}

/// Render `seconds` of sound. `f` gets the sample index and the time in seconds. Then fade in,
/// fade out, and scale to `PEAK`.
fn render(seconds: f32, mut f: impl FnMut(usize, f32) -> f32) -> Vec<i16> {
    let n = (seconds * SOUND_RATE as f32) as usize;
    let mut v: Vec<f32> = (0..n).map(|i| f(i, i as f32 / SOUND_RATE as f32)).collect();
    for (i, s) in v.iter_mut().enumerate() {
        if i < FADE_IN {
            *s *= i as f32 / FADE_IN as f32;
        }
        let left = n - 1 - i;
        if left < FADE_OUT {
            *s *= left as f32 / FADE_OUT as f32;
        }
    }
    let top = v.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let scale = if top > 0.0 { PEAK / top } else { 0.0 };
    v.iter()
        .map(|s| {
            let x = s * scale;
            (if x >= 0.0 { x + 0.5 } else { x - 0.5 }) as i16
        })
        .collect()
}

// ---------------------------------------------------------------- sounds

/// Soft two-note chime up: 659 Hz then 880 Hz, 0.2 s each, soft second harmonic. 0.4 s.
fn alert_info() -> Vec<i16> {
    render(0.4, |_, t| {
        let (hz, t0) = if t < 0.2 { (659.0, 0.0) } else { (880.0, 0.2) };
        let local = t - t0;
        let body = sine(hz * local) + 0.25 * sine(2.0 * hz * local);
        body * decay(local / 0.2, 2)
    })
}

/// Two short mid beeps: 740 Hz sine plus third harmonic, 0.14 s each, gap 0.1 s. 0.5 s.
fn alert_warning() -> Vec<i16> {
    render(0.5, |_, t| {
        let a = gate(t, 0.14, 0.006);
        let b = gate(t - 0.24, 0.14, 0.006);
        let w = sine(740.0 * t) + 0.35 * sine(2_220.0 * t);
        w * (a + b)
    })
}

/// Low urgent buzz: three pulses of 0.24 s that fall in pitch (220, 185, 150 Hz). Saw plus a
/// square an octave below, with a 30 Hz tremolo. 0.8 s.
fn alert_danger() -> Vec<i16> {
    let mut hi = Osc::new();
    let mut lo = Osc::new();
    render(0.8, |_, t| {
        let pulse = (t / 0.28) as usize;
        let local = t - pulse as f32 * 0.28;
        let hz = [220.0, 185.0, 150.0][pulse.min(2)];
        let g = gate(local, 0.24, 0.008);
        let tremolo = 0.75 + 0.25 * square(30.0 * t);
        let w = saw(hi.step(hz)) + 0.8 * square(lo.step(hz * 0.5));
        w * g * tremolo
    })
}

/// Soft tick: a 2,000 Hz sine and a bit of noise, cubic decay. 0.05 s.
fn ui_click(seed: u64) -> Vec<i16> {
    let mut rng = rng_for(seed, 0x5001);
    render(0.05, |_, t| {
        let d = decay(t / 0.05, 3);
        (sine(2_000.0 * t) * 0.8 + noise(&mut rng) * 0.25) * d
    })
}

/// Soft thud: low-passed noise plus a 90 Hz to 50 Hz sine. 0.12 s.
fn footstep(seed: u64) -> Vec<i16> {
    let mut rng = rng_for(seed, 0x5002);
    let mut lp = LowPass::new(0.08);
    let mut osc = Osc::new();
    render(0.12, |_, t| {
        let u = t / 0.12;
        let d = decay(u, 2);
        let thump = sine(osc.step(90.0 - 40.0 * u));
        (lp.next(noise(&mut rng)) * 3.0 + thump) * d
    })
}

/// Wood knock: a sine that falls from 210 Hz to 120 Hz, a 2.4x partial, and a short low-passed
/// noise hit. 0.2 s.
fn tool_chop(seed: u64) -> Vec<i16> {
    let mut rng = rng_for(seed, 0x5003);
    let mut lp = LowPass::new(0.3);
    let mut osc = Osc::new();
    let mut osc2 = Osc::new();
    render(0.2, |_, t| {
        let u = t / 0.2;
        let hz = 210.0 - 90.0 * u;
        let tone = sine(osc.step(hz)) + 0.4 * sine(osc2.step(hz * 2.4));
        let hit = lp.next(noise(&mut rng)) * decay(t / 0.02, 2);
        tone * decay(u, 3) + hit * 1.2
    })
}

/// Metal tink on stone: three inharmonic partials (1,900, 2,750, 4,130 Hz) with a fast decay and
/// a 4 ms noise tick. 0.2 s.
fn tool_mine(seed: u64) -> Vec<i16> {
    let mut rng = rng_for(seed, 0x5004);
    render(0.2, |_, t| {
        let u = t / 0.2;
        let ring = sine(1_900.0 * t) * decay(u, 2)
            + 0.6 * sine(2_750.0 * t) * decay(u, 3)
            + 0.35 * sine(4_130.0 * t) * decay(u * 2.0, 2);
        ring + noise(&mut rng) * decay(t / 0.004, 1) * 0.8
    })
}

/// Short rising ding: 784 Hz for 0.12 s, then 1,175 Hz with a 2.76x bell partial. 0.3 s.
fn craft_done() -> Vec<i16> {
    render(0.3, |_, t| {
        let (hz, t0, len) = if t < 0.12 {
            (784.0, 0.0, 0.12)
        } else {
            (1_175.0, 0.12, 0.18)
        };
        let local = t - t0;
        let body = sine(hz * local) + 0.3 * sine(hz * 2.76 * local);
        body * decay(local / len, 2)
    })
}

/// Make every sound of the game from a seed.
pub fn make_all(seed: u64) -> Vec<Sound> {
    let s = |name, samples| Sound { name, samples };
    vec![
        s("alert_info", alert_info()),
        s("alert_warning", alert_warning()),
        s("alert_danger", alert_danger()),
        s("ui_click", ui_click(seed)),
        s("footstep", footstep(seed)),
        s("tool_chop", tool_chop(seed)),
        s("tool_mine", tool_mine(seed)),
        s("craft_done", craft_done()),
    ]
}

// ---------------------------------------------------------------- WAV writer

/// Encode 16-bit mono samples as a WAV file at `SOUND_RATE`.
pub fn encode_wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SOUND_RATE.to_le_bytes());
    out.extend_from_slice(&(SOUND_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(s: &[i16]) -> i32 {
        s.iter().map(|&x| i32::from(x).abs()).max().unwrap_or(0)
    }

    fn find<'a>(all: &'a [Sound], name: &str) -> &'a Sound {
        all.iter().find(|s| s.name == name).expect("sound exists")
    }

    #[test]
    fn wav_header_is_correct() {
        let wav = encode_wav(&[0, 1, -1, 300]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
        assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 44_100);
        assert_eq!(u32::from_le_bytes(wav[28..32].try_into().unwrap()), 88_200);
        assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(u32::from_le_bytes(wav[40..44].try_into().unwrap()), 8);
        assert_eq!(wav.len(), 52);
        assert_eq!(&wav[50..52], &300i16.to_le_bytes());
    }

    #[test]
    fn sounds_are_repeatable() {
        let a = make_all(1);
        let b = make_all(1);
        assert_eq!(a.len(), 8);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(x.samples, y.samples, "{}", x.name);
        }
    }

    #[test]
    fn noise_sounds_depend_on_the_seed() {
        let a = make_all(1);
        let b = make_all(2);
        assert_ne!(find(&a, "footstep").samples, find(&b, "footstep").samples);
    }

    #[test]
    fn every_sound_is_short_quiet_enough_and_click_free() {
        for s in make_all(crate::assetgen::DEFAULT_SEED) {
            assert!(!s.samples.is_empty(), "{}", s.name);
            assert!(
                s.samples.len() <= SOUND_RATE as usize,
                "{} too long",
                s.name
            );
            let p = peak(&s.samples);
            assert!(p <= 23_000, "{} peak {p}", s.name);
            assert!(p >= 10_000, "{} too quiet: {p}", s.name);
            assert!(s.samples[0].abs() < 200, "{} start", s.name);
            assert!(s.samples[s.samples.len() - 1].abs() < 200, "{} end", s.name);
        }
    }

    #[test]
    fn the_three_alerts_differ() {
        let all = make_all(1);
        let (i, w, d) = (
            find(&all, "alert_info"),
            find(&all, "alert_warning"),
            find(&all, "alert_danger"),
        );
        assert_ne!(i.samples, w.samples);
        assert_ne!(w.samples, d.samples);
        assert_ne!(i.samples, d.samples);
        assert!(d.samples.len() > w.samples.len() && w.samples.len() > i.samples.len());
    }

    fn shipped(name: &str) -> Option<Vec<u8>> {
        let path = format!(
            "{}/../../assets/sounds/{name}.wav",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::read(path).ok()
    }

    #[test]
    fn shipped_sounds_match_the_generator() {
        for s in make_all(crate::assetgen::DEFAULT_SEED) {
            assert_eq!(
                shipped(s.name).as_deref(),
                Some(encode_wav(&s.samples).as_slice()),
                "{} differs from the generator: run `cargo run -p tools -- assetgen`",
                s.name
            );
        }
    }
}
