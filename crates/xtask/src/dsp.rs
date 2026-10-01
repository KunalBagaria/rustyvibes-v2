//! Clip clean-up applied at build time: DC removal, onset/tail trimming with
//! short fades, loudness normalisation and dithered 16-bit quantisation.

use std::f32::consts::PI;

/// Loudness target: median attack-window RMS of a pack's press clips.
pub const TARGET_DB: f32 = -30.0;
/// No clip may peak above this after normalisation.
pub const CEILING_DB: f32 = -1.0;
/// Length of the attack window used to measure loudness.
pub const ATTACK_MS: f32 = 60.0;

pub fn db_to_lin(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub fn lin_to_db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

pub fn peak(x: &[f32]) -> f32 {
    x.iter().fold(0.0, |m, s| m.max(s.abs()))
}

/// One-pole DC blocker with its corner near 20 Hz.
pub fn dc_block(x: &mut [f32], rate: u32) {
    let r = 1.0 - 2.0 * PI * 20.0 / rate as f32;
    let (mut prev_in, mut prev_out) = (0.0f32, 0.0f32);
    for s in x.iter_mut() {
        let out = *s - prev_in + r * prev_out;
        prev_in = *s;
        prev_out = out;
        *s = out;
    }
}

/// Thresholds and fade lengths for [`trim_clip`].
#[derive(Clone, Copy, Debug)]
pub struct TrimParams {
    /// Absolute floor below which a sample never counts as the onset (dBFS).
    pub floor_db: f32,
    /// Onset threshold relative to the clip peak (dB).
    pub onset_rel_db: f32,
    /// Tail threshold relative to the clip peak (dB).
    pub tail_rel_db: f32,
    /// Audio kept before the onset (ms), faded in linearly.
    pub preroll_ms: f32,
    /// Raised-cosine fade-out length (ms).
    pub fade_out_ms: f32,
}

impl Default for TrimParams {
    fn default() -> Self {
        TrimParams {
            floor_db: -55.0,
            onset_rel_db: -45.0,
            tail_rel_db: -50.0,
            preroll_ms: 0.5,
            fade_out_ms: 6.0,
        }
    }
}

/// Removes leading and trailing silence and applies short fades so the clip
/// starts on its transient. Returns `None` when the clip is silent.
pub fn trim_clip(x: &[f32], rate: u32, params: &TrimParams) -> Option<Vec<f32>> {
    let pk = peak(x);
    let floor = db_to_lin(params.floor_db);
    if pk < floor {
        return None;
    }
    let onset_threshold = floor.max(pk * db_to_lin(params.onset_rel_db));
    let tail_threshold = pk * db_to_lin(params.tail_rel_db);
    let onset = x.iter().position(|s| s.abs() >= onset_threshold)?;
    let last = x.iter().rposition(|s| s.abs() >= tail_threshold)?;
    let to_samples = |ms: f32| (ms / 1000.0 * rate as f32).round() as usize;

    let start = onset.saturating_sub(to_samples(params.preroll_ms));
    let fade_out = to_samples(params.fade_out_ms);
    let end = (last + 1 + fade_out).min(x.len());
    let mut out = x[start..end].to_vec();

    let fade_in = onset - start;
    for (i, s) in out.iter_mut().take(fade_in).enumerate() {
        *s *= i as f32 / fade_in as f32;
    }
    let n = out.len();
    let fade_out = fade_out.min(n);
    for i in 0..fade_out {
        let t = (i as f32 + 0.5) / fade_out as f32;
        out[n - fade_out + i] *= 0.5 * (1.0 + (PI * t).cos());
    }
    Some(out)
}

/// RMS level (dBFS) of the first `ms` milliseconds of `x`.
pub fn attack_rms_db(x: &[f32], rate: u32, ms: f32) -> f32 {
    let n = ((ms / 1000.0 * rate as f32) as usize).min(x.len());
    if n == 0 {
        return lin_to_db(0.0);
    }
    let energy = x[..n].iter().map(|s| s * s).sum::<f32>() / n as f32;
    lin_to_db(energy.sqrt())
}

pub fn median(mut values: Vec<f32>) -> f32 {
    if values.is_empty() {
        return 0.0;
    }
    values.sort_by(f32::total_cmp);
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) { (values[mid - 1] + values[mid]) / 2.0 } else { values[mid] }
}

/// Applies one gain to every clip so the median attack loudness of the press
/// clips (`is_press`) hits `target_db`, then attenuates individual clips whose
/// peak would exceed `ceiling_db`. Returns the pack gain in dB.
pub fn normalize(
    clips: &mut [Vec<f32>],
    is_press: &[bool],
    rate: u32,
    target_db: f32,
    ceiling_db: f32,
) -> f32 {
    let level = |c: &Vec<f32>| attack_rms_db(c, rate, ATTACK_MS);
    let mut levels: Vec<f32> =
        clips.iter().zip(is_press).filter(|(_, press)| **press).map(|(c, _)| level(c)).collect();
    if levels.is_empty() {
        levels = clips.iter().map(level).collect();
    }
    let gain_db = target_db - median(levels);
    let gain = db_to_lin(gain_db);
    let ceiling = db_to_lin(ceiling_db);
    for clip in clips.iter_mut() {
        let projected = peak(clip) * gain;
        let g = if projected > ceiling { gain * ceiling / projected } else { gain };
        clip.iter_mut().for_each(|s| *s *= g);
    }
    gain_db
}

/// Quantises to 16-bit with triangular (TPDF) dither; `seed` advances.
pub fn to_i16(x: &[f32], seed: &mut u32) -> Vec<i16> {
    x.iter()
        .map(|&s| {
            let dither = unit(seed) - unit(seed);
            (s * 32_767.0 + dither).round().clamp(-32_768.0, 32_767.0) as i16
        })
        .collect()
}

fn unit(state: &mut u32) -> f32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    (x >> 8) as f32 / 16_777_216.0
}
#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;
    // Explicit levels keep these tests about `normalize`, not the shipped target.
    const TEST_TARGET_DB: f32 = -22.0;
    const TEST_CEILING_DB: f32 = -1.0;

    fn samples(ms: f32) -> usize {
        (ms / 1000.0 * RATE as f32) as usize
    }

    /// `lead_ms` of silence, a 2 kHz burst decaying by ~52 dB over `decay_ms`, then `tail_ms` of silence.
    fn click(lead_ms: f32, decay_ms: f32, tail_ms: f32, amplitude: f32) -> Vec<f32> {
        let mut x = vec![0.0; samples(lead_ms)];
        let tau = decay_ms / 6.0 / 1000.0;
        x.extend((0..samples(decay_ms)).map(|i| {
            let t = i as f32 / RATE as f32;
            amplitude * (-t / tau).exp() * (2.0 * PI * 2_000.0 * t).sin()
        }));
        x.extend(vec![0.0; samples(tail_ms)]);
        x
    }

    #[test]
    fn trim_removes_leading_silence_but_keeps_preroll() {
        let x = click(20.0, 80.0, 30.0, 0.8);
        let y = trim_clip(&x, RATE, &TrimParams::default()).unwrap();
        let threshold = peak(&y) * db_to_lin(-45.0);
        let onset = y.iter().position(|s| s.abs() >= threshold).unwrap();
        assert_eq!(onset, 22, "0.5 ms of pre-roll at 44.1 kHz");
        assert!(y.len() + samples(19.0) < x.len(), "lead silence removed");
    }

    #[test]
    fn trim_fades_both_ends_to_silence() {
        let y = trim_clip(&click(20.0, 80.0, 30.0, 0.8), RATE, &TrimParams::default()).unwrap();
        assert!(y[0].abs() < 1e-6);
        assert!(y.last().unwrap().abs() < 1e-3);
    }

    #[test]
    fn trim_fades_a_clip_that_is_cut_off() {
        // A sustained tone that runs to the very end of the slice.
        let x: Vec<f32> = (0..samples(50.0))
            .map(|i| 0.5 * (2.0 * PI * 1_000.0 * i as f32 / RATE as f32).sin())
            .collect();
        let y = trim_clip(&x, RATE, &TrimParams::default()).unwrap();
        assert!(y.last().unwrap().abs() < 1e-3);
        assert_eq!(y.len(), x.len(), "the onset is at sample 1, inside the pre-roll");
    }

    #[test]
    fn silent_clip_is_dropped() {
        assert!(trim_clip(&vec![1e-4; 1_000], RATE, &TrimParams::default()).is_none());
        assert!(trim_clip(&[], RATE, &TrimParams::default()).is_none());
    }

    #[test]
    fn normalize_hits_the_target_median() {
        let mut clips: Vec<Vec<f32>> =
            [0.1, 0.5, 1.0].iter().map(|&a| click(0.0, 80.0, 0.0, a)).collect();
        normalize(&mut clips, &[true, true, true], RATE, TEST_TARGET_DB, TEST_CEILING_DB);
        let levels: Vec<f32> = clips.iter().map(|c| attack_rms_db(c, RATE, ATTACK_MS)).collect();
        assert!((median(levels) - TEST_TARGET_DB).abs() < 0.05);
    }

    #[test]
    fn normalize_limits_hot_clips_individually() {
        let quiet = click(0.0, 80.0, 0.0, 0.3);
        let mut spike = vec![0.0; samples(80.0)];
        spike[10] = 1.0;
        let mut clips = vec![quiet.clone(), quiet.clone(), spike];
        let gain_db =
            normalize(&mut clips, &[true, true, true], RATE, TEST_TARGET_DB, TEST_CEILING_DB);
        let gain = db_to_lin(gain_db);
        assert!(peak(&clips[2]) <= db_to_lin(TEST_CEILING_DB) + 1e-6);
        for (a, b) in quiet.iter().zip(&clips[0]) {
            assert!((a * gain - b).abs() < 1e-6, "quiet clips get exactly the pack gain");
        }
    }

    #[test]
    fn release_clips_follow_the_press_gain() {
        let press = click(0.0, 80.0, 0.0, 0.4);
        let release = click(0.0, 40.0, 0.0, 0.05);
        let mut clips = vec![press.clone(), release.clone()];
        let gain =
            db_to_lin(normalize(&mut clips, &[true, false], RATE, TEST_TARGET_DB, TEST_CEILING_DB));
        assert!((clips[1][100] - release[100] * gain).abs() < 1e-6);
        assert!((clips[0][100] - press[100] * gain).abs() < 1e-6);
    }

    #[test]
    fn dc_block_removes_offset() {
        let mut x = vec![0.5; RATE as usize];
        dc_block(&mut x, RATE);
        let tail = &x[x.len() - 1_000..];
        assert!(tail.iter().sum::<f32>().abs() / 1_000.0 < 1e-3);
    }

    #[test]
    fn dither_is_small_unbiased_and_clamped() {
        let mut seed = 1;
        let q = to_i16(&vec![0.0; 10_000], &mut seed);
        assert!(q.iter().all(|&s| (-1..=1).contains(&s)));
        let mean = q.iter().map(|&s| f32::from(s)).sum::<f32>() / q.len() as f32;
        assert!(mean.abs() < 0.05);
        assert_eq!(to_i16(&[1.5, -1.5], &mut seed), vec![32_767, -32_768]);
    }

    #[test]
    fn median_of_even_and_odd_lists() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(median(vec![]), 0.0);
    }
}
