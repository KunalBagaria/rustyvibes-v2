//! Test helpers: a tiny WAV writer, synthetic clicks and per-test temp dirs.

use std::f32::consts::PI;
use std::path::{Path, PathBuf};

/// Writes 16-bit PCM WAV. `samples` are interleaved when `channels > 1`.
pub fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[i16]) {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + samples.len() * 2);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&channels.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    b.extend_from_slice(&(channels * 2).to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    std::fs::write(path, b).unwrap();
}

/// A fresh, empty directory unique to this test process.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rustyvibes-xtask-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// `lead` zero samples, then a decaying 2 kHz click of `len` samples at 44.1 kHz.
pub fn click_i16(lead: usize, len: usize, amplitude: f32) -> Vec<i16> {
    let mut out = vec![0i16; lead];
    out.extend((0..len).map(|i| {
        let t = i as f32 / 44_100.0;
        let x = amplitude * (-t * 150.0).exp() * (2.0 * PI * 2_000.0 * t).sin();
        (x * 32_767.0) as i16
    }));
    out
}
