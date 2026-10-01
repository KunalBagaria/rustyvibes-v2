//! Voice mixer run by the CoreAudio render callback. Fixed voice pool, no
//! allocation: 32.32 fixed-point playback positions, 4-point cubic Hermite
//! interpolation (sample-rate conversion and pitch variation in one step),
//! a ramped master gain and a soft-knee limiter.

use rvpack::{GUARD_AFTER, GUARD_BEFORE};

use super::Play;

/// Simultaneous voices; the oldest is stolen when a new one needs a slot.
pub const MAX_VOICES: usize = 32;

const ONE: u64 = 1 << 32;
const FRACTION: u64 = ONE - 1;
const I16_SCALE: f32 = 1.0 / 32_768.0;
/// The limiter is transparent below this level (≈ −1.9 dBFS).
const KNEE: f32 = 0.8;

#[derive(Clone, Copy)]
struct Voice {
    samples: &'static [i16],
    pos: u64,
    end: u64,
    step: u64,
    gain_l: f32,
    gain_r: f32,
    /// Output frames left before the voice starts.
    delay: u32,
    /// Start order, for stealing the oldest voice.
    serial: u64,
}

pub struct Mixer {
    out_rate: f64,
    voices: [Option<Voice>; MAX_VOICES],
    serial: u64,
    /// Master gain reached at the end of the previous block.
    gain: f32,
}

impl Mixer {
    pub fn new(out_rate: f64) -> Mixer {
        Mixer { out_rate, voices: [None; MAX_VOICES], serial: 0, gain: 0.0 }
    }

    pub fn set_out_rate(&mut self, out_rate: f64) {
        self.out_rate = out_rate;
    }

    pub fn active_voices(&self) -> usize {
        self.voices.iter().filter(|v| v.is_some()).count()
    }

    /// Starts a voice, stealing the oldest one if every slot is busy.
    pub fn start(&mut self, play: &Play) {
        let audio_len = play.samples.len().saturating_sub(GUARD_BEFORE + GUARD_AFTER);
        if audio_len == 0 {
            return;
        }
        let ratio = f64::from(play.src_rate) / self.out_rate * f64::from(play.pitch);
        let step = ((ratio * ONE as f64).round() as u64).max(1);
        let delay = (f64::from(play.delay_us) * self.out_rate / 1e6).round() as u32;
        self.serial += 1;
        let voice = Voice {
            samples: play.samples,
            pos: (GUARD_BEFORE as u64) << 32,
            end: ((GUARD_BEFORE + audio_len) as u64) << 32,
            step,
            gain_l: play.gain_l,
            gain_r: play.gain_r,
            delay,
            serial: self.serial,
        };
        let slot = self.voices.iter().position(Option::is_none).unwrap_or_else(|| {
            self.voices
                .iter()
                .enumerate()
                .min_by_key(|(_, v)| v.as_ref().map_or(0, |v| v.serial))
                .map_or(0, |(i, _)| i)
        });
        self.voices[slot] = Some(voice);
    }

    /// Overwrites `left`/`right` with the mix, ramping the master gain from the
    /// previous block to `master`. Returns whether any voice is sounding or pending.
    pub fn render(&mut self, left: &mut [f32], right: &mut [f32], master: f32) -> bool {
        let n = left.len().min(right.len());
        let (left, right) = (&mut left[..n], &mut right[..n]);
        left.fill(0.0);
        right.fill(0.0);
        let mut sounding = false;
        for slot in self.voices.iter_mut() {
            let Some(voice) = slot else { continue };
            sounding = true;
            let offset = (voice.delay as usize).min(n);
            voice.delay -= offset as u32;
            if offset == n {
                continue;
            }
            if mix_voice(voice, &mut left[offset..], &mut right[offset..]) {
                *slot = None;
            }
        }
        let from = self.gain;
        self.gain = master;
        if sounding {
            let step = (master - from) / n.max(1) as f32;
            let mut g = from;
            for (l, r) in left.iter_mut().zip(right.iter_mut()) {
                g += step;
                *l = soft_clip(*l * g);
                *r = soft_clip(*r * g);
            }
        }
        sounding
    }
}

/// Adds one voice into the buffers. Returns `true` once the voice has finished.
fn mix_voice(v: &mut Voice, left: &mut [f32], right: &mut [f32]) -> bool {
    let s = v.samples;
    let (gl, gr) = (v.gain_l * I16_SCALE, v.gain_r * I16_SCALE);
    if v.step == ONE && v.pos & FRACTION == 0 {
        // Same rate, no pitch change: copy samples directly.
        let start = (v.pos >> 32) as usize;
        let count = ((v.end >> 32) as usize - start).min(left.len());
        for ((l, r), &x) in left.iter_mut().zip(right.iter_mut()).zip(&s[start..start + count]) {
            let x = f32::from(x);
            *l += x * gl;
            *r += x * gr;
        }
        v.pos = ((start + count) as u64) << 32;
    } else {
        let mut pos = v.pos;
        for (l, r) in left.iter_mut().zip(right.iter_mut()) {
            if pos >= v.end {
                break;
            }
            let i = (pos >> 32) as usize;
            let t = (pos & FRACTION) as f32 * (1.0 / ONE as f32);
            // Guard samples make i - 1 and i + 2 valid for every position before `end`.
            let (xm1, x0, x1, x2) =
                (f32::from(s[i - 1]), f32::from(s[i]), f32::from(s[i + 1]), f32::from(s[i + 2]));
            let c1 = 0.5 * (x1 - xm1);
            let c2 = xm1 - 2.5 * x0 + 2.0 * x1 - 0.5 * x2;
            let c3 = 0.5 * (x2 - xm1) + 1.5 * (x0 - x1);
            let y = ((c3 * t + c2) * t + c1) * t + x0;
            *l += y * gl;
            *r += y * gr;
            pos += v.step;
        }
        v.pos = pos;
    }
    v.pos >= v.end
}

/// Identity below the knee, then a smooth curve that approaches ±1.
fn soft_clip(x: f32) -> f32 {
    let a = x.abs();
    if a <= KNEE {
        return x;
    }
    let over = (a - KNEE) / (1.0 - KNEE);
    x.signum() * (KNEE + (1.0 - KNEE) * over / (1.0 + over))
}
#[cfg(test)]
mod tests {
    use super::*;

    fn guarded(audio: &[i16]) -> &'static [i16] {
        let mut v = vec![0i16; GUARD_BEFORE];
        v.extend_from_slice(audio);
        v.extend(std::iter::repeat_n(0, GUARD_AFTER));
        Box::leak(v.into_boxed_slice())
    }

    fn play(samples: &'static [i16], src_rate: u32) -> Play {
        Play { samples, src_rate, pitch: 1.0, gain_l: 1.0, gain_r: 1.0, delay_us: 0 }
    }

    /// A mixer whose master gain has already settled at 1.0.
    fn mixer(rate: f64) -> Mixer {
        let mut m = Mixer::new(rate);
        m.render(&mut [0.0; 4], &mut [0.0; 4], 1.0);
        m
    }

    fn render(m: &mut Mixer, frames: usize) -> (Vec<f32>, Vec<f32>, bool) {
        let (mut l, mut r) = (vec![0.0; frames], vec![0.0; frames]);
        let sounding = m.render(&mut l, &mut r, 1.0);
        (l, r, sounding)
    }

    #[test]
    fn unity_rate_copies_samples_exactly() {
        let audio: Vec<i16> = (0..100).map(|i| i * 100).collect();
        let mut m = mixer(48_000.0);
        m.start(&play(guarded(&audio), 48_000));
        let (l, r, sounding) = render(&mut m, 128);
        assert!(sounding);
        for i in 0..100 {
            assert_eq!(l[i], f32::from(audio[i]) / 32_768.0);
            assert_eq!(r[i], l[i]);
        }
        assert!(l[100..].iter().all(|&x| x == 0.0));
        assert_eq!(m.active_voices(), 0, "the voice ends after its last sample");
    }

    #[test]
    fn delay_postpones_the_start() {
        // At 1 MHz one microsecond is one frame.
        let mut m = mixer(1_000_000.0);
        let mut p = play(guarded(&[1_000; 10]), 1_000_000);
        p.delay_us = 10;
        m.start(&p);
        let (l, _, _) = render(&mut m, 32);
        assert!(l[..10].iter().all(|&x| x == 0.0));
        assert!(l[10..20].iter().all(|&x| x == 1_000.0 / 32_768.0));
        assert!(l[20..].iter().all(|&x| x == 0.0));
    }

    #[test]
    fn delay_longer_than_a_block_carries_over() {
        let mut m = mixer(1_000_000.0);
        let mut p = play(guarded(&[1_000; 4]), 1_000_000);
        p.delay_us = 200;
        m.start(&p);
        let (l, _, sounding) = render(&mut m, 128);
        assert!(sounding, "a pending voice keeps the stream alive");
        assert!(l.iter().all(|&x| x == 0.0));
        let (l, _, _) = render(&mut m, 128);
        assert!(l[..72].iter().all(|&x| x == 0.0));
        assert_eq!(l[72], 1_000.0 / 32_768.0);
    }

    #[test]
    fn gains_apply_per_channel() {
        let mut m = mixer(48_000.0);
        let mut p = play(guarded(&[16_384; 8]), 48_000);
        p.gain_l = 0.5;
        p.gain_r = 0.25;
        m.start(&p);
        let (l, r, _) = render(&mut m, 8);
        assert_eq!(l[0], 0.25);
        assert_eq!(r[0], 0.125);
    }

    #[test]
    fn half_rate_interpolates_linear_ramps_exactly() {
        // Catmull-Rom interpolation reproduces a linear ramp exactly.
        let audio: Vec<i16> = (0..64).map(|i| i * 200).collect();
        let mut m = mixer(48_000.0);
        m.start(&play(guarded(&audio), 24_000));
        let (l, _, _) = render(&mut m, 128);
        for (k, &x) in l.iter().enumerate().take(120).skip(2) {
            let expected = 200.0 * k as f32 / 2.0 / 32_768.0;
            assert!((x - expected).abs() < 1e-6, "frame {k}: {x} vs {expected}");
        }
    }

    #[test]
    fn constant_signal_survives_pitch_shift() {
        let mut m = mixer(48_000.0);
        let mut p = play(guarded(&[1_000; 200]), 44_100);
        p.pitch = 1.03;
        m.start(&p);
        let (l, _, _) = render(&mut m, 128);
        for (k, &x) in l.iter().enumerate().take(120).skip(2) {
            assert!((x - 1_000.0 / 32_768.0).abs() < 1e-6, "frame {k}");
        }
    }

    #[test]
    fn resampling_to_96k_preserves_duration() {
        let audio = vec![1_000i16; 441]; // 10 ms at 44.1 kHz
        let mut m = mixer(96_000.0);
        m.start(&play(guarded(&audio), 44_100));
        let mut produced = 0;
        loop {
            let (l, _, sounding) = render(&mut m, 64);
            produced += l.iter().filter(|&&x| x != 0.0).count();
            if !sounding {
                break;
            }
        }
        assert!((958..=962).contains(&produced), "{produced} frames for 10 ms at 96 kHz");
    }

    #[test]
    fn steals_oldest_voice_when_full() {
        let mut m = mixer(48_000.0);
        let first = guarded(&[100; 1_000]);
        let rest = guarded(&[10; 1_000]);
        m.start(&play(first, 48_000));
        for _ in 0..MAX_VOICES {
            m.start(&play(rest, 48_000));
        }
        assert_eq!(m.active_voices(), MAX_VOICES);
        let (l, _, _) = render(&mut m, 16);
        let expected = MAX_VOICES as f32 * 10.0 / 32_768.0;
        assert!((l[0] - expected).abs() < 1e-6, "the oldest voice was replaced");
    }

    #[test]
    fn limiter_keeps_output_within_unity() {
        let mut m = mixer(48_000.0);
        let loud = guarded(&[i16::MAX; 256]);
        for _ in 0..MAX_VOICES {
            m.start(&play(loud, 48_000));
        }
        let (l, r, _) = render(&mut m, 256);
        assert!(l.iter().chain(&r).all(|x| x.abs() <= 1.0));
        assert!(l[10] > 0.95, "heavy overdrive approaches full scale");
    }

    #[test]
    fn soft_clip_is_transparent_below_the_knee_and_monotonic() {
        assert_eq!(soft_clip(0.5), 0.5);
        assert_eq!(soft_clip(-0.8), -0.8);
        let mut prev = soft_clip(0.0);
        for i in 1..=4_000 {
            let y = soft_clip(i as f32 / 1_000.0);
            assert!(y >= prev && y < 1.0);
            prev = y;
        }
    }

    #[test]
    fn master_gain_ramps_without_a_jump() {
        let mut m = Mixer::new(48_000.0);
        m.start(&play(guarded(&[16_384; 512]), 48_000));
        let (mut l, mut r) = (vec![0.0; 256], vec![0.0; 256]);
        m.render(&mut l, &mut r, 1.0);
        assert!(l[0] < 0.01, "starts near the previous gain (0)");
        assert!((l[255] - 0.5).abs() < 1e-3, "ends at the new gain");
        assert!(l.windows(2).all(|w| w[1] >= w[0]));
    }

    #[test]
    fn silence_reports_not_sounding() {
        let mut m = mixer(48_000.0);
        let (l, r, sounding) = render(&mut m, 64);
        assert!(!sounding);
        assert!(l.iter().chain(&r).all(|&x| x == 0.0));
    }
}
