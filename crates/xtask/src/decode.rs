//! Decodes Vorbis, MP3 and WAV files to mono `f32` with symphonia.

use std::fs::File;
use std::path::Path;

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

/// Mono audio.
#[derive(Clone, Debug)]
pub struct Audio {
    pub rate: u32,
    pub samples: Vec<f32>,
}

/// Decodes the default audio track of `path`, averaging channels to mono.
pub fn decode_file(path: &Path) -> crate::Result<Audio> {
    let fail = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
    let file = File::open(path).map_err(|e| fail(&e))?;
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), MetadataOptions::default())
        .map_err(|e| fail(&e))?;
    let track = format.default_track(TrackType::Audio).ok_or_else(|| fail(&"no audio track"))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| fail(&"not an audio track"))?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| fail(&e))?;

    let mut rate = params.sample_rate.unwrap_or(0);
    let mut mono = Vec::new();
    let mut interleaved: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(e) => return Err(fail(&e)),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(e) => return Err(fail(&e)),
        };
        let channels = buffer.spec().channels().count().max(1);
        rate = buffer.spec().rate();
        interleaved.clear();
        buffer.copy_to_vec_interleaved(&mut interleaved);
        mono.extend(
            interleaved
                .chunks_exact(channels)
                .map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
    if rate == 0 || mono.is_empty() {
        return Err(fail(&"decoded no audio"));
    }
    Ok(Audio { rate, samples: mono })
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{click_i16, temp_dir, write_wav};

    #[test]
    fn decodes_wav_and_downmixes_to_mono() {
        let dir = temp_dir("decode");
        let path = dir.join("stereo.wav");
        // Frames: (0.5, 0.0), (0.0, 0.5), (-0.5, -0.5)
        write_wav(&path, 22_050, 2, &[16_384, 0, 0, 16_384, -16_384, -16_384]);
        let audio = decode_file(&path).unwrap();
        assert_eq!(audio.rate, 22_050);
        assert_eq!(audio.samples.len(), 3);
        assert!((audio.samples[0] - 0.25).abs() < 1e-4);
        assert!((audio.samples[1] - 0.25).abs() < 1e-4);
        assert!((audio.samples[2] + 0.5).abs() < 1e-4);
    }

    #[test]
    fn decodes_mono_wav_sample_for_sample() {
        let dir = temp_dir("decode-mono");
        let path = dir.join("click.wav");
        let click = click_i16(10, 2_000, 0.5);
        write_wav(&path, 44_100, 1, &click);
        let audio = decode_file(&path).unwrap();
        assert_eq!(audio.rate, 44_100);
        assert_eq!(audio.samples.len(), click.len());
        for (decoded, &original) in audio.samples.iter().zip(&click) {
            assert!((decoded - f32::from(original) / 32_768.0).abs() < 1e-6);
        }
    }

    #[test]
    fn missing_file_names_the_path() {
        let err = decode_file(Path::new("/nonexistent/x.wav")).unwrap_err();
        assert!(err.contains("/nonexistent/x.wav"), "{err}");
    }
}
