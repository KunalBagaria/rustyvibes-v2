//! Mechvibes packs: `config.json` plus either one sprite sliced by
//! `[start_ms, duration_ms]` (`"key_define_type": "single"`) or one file per
//! key (`"multiple"`). Keys are libuiohook scan codes.

use std::collections::HashMap;
use std::path::Path;

use rvpack::{keys, mechvibes};
use serde_json::{Map, Value};

use super::RawPack;
use crate::{decode::decode_file, dsp};

/// Slice starts closer than this belong to the same recording.
const NEAR_DUPLICATE_MS: f64 = 50.0;
/// An onset this close before a define's start still belongs to that define.
const ONSET_TOLERANCE_MS: f64 = 10.0;
/// Gap left before the next key's onset when a slice is shortened.
const ONSET_MARGIN_MS: f64 = 2.0;

/// Clip indices of the sounds a pack defines, by Mechvibes code.
#[derive(Default)]
struct Defined {
    press: HashMap<u16, usize>,
    /// From `"N-up"` defines.
    release: HashMap<u16, usize>,
}

pub fn load(dir: &Path) -> crate::Result<RawPack> {
    let config_path = dir.join("config.json");
    let text = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("{}: {e}", config_path.display()))?;
    let config: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", config_path.display()))?;
    let defines = config
        .get("defines")
        .and_then(Value::as_object)
        .ok_or_else(|| format!("{}: missing \"defines\"", config_path.display()))?;

    let mut pack = RawPack::new(0);
    let defined = match config.get("key_define_type").and_then(Value::as_str).unwrap_or("single") {
        "single" => load_sprite(dir, &config, defines, &mut pack)?,
        _ => Defined { press: load_files(dir, defines, &mut pack)?, release: HashMap::new() },
    };
    if pack.clips.is_empty() {
        return Err(format!("{}: no sounds defined", dir.display()));
    }
    let fallback = defined.press.get(&mechvibes::CODE_A).copied().unwrap_or(0);
    for key in keys::KEYS {
        let codes = mechvibes::candidates(key.code);
        let clip =
            codes.iter().find_map(|code| defined.press.get(code).copied()).unwrap_or(fallback);
        pack.press[usize::from(key.code)] = Some(clip);
        // Mechvibes plays "N-up" when key N is released, without fallbacks: a similar
        // key's release could double up with a press slice that already holds one.
        pack.release[usize::from(key.code)] =
            codes.first().and_then(|code| defined.release.get(code).copied());
    }
    Ok(pack)
}

fn load_sprite(
    dir: &Path,
    config: &Value,
    defines: &Map<String, Value>,
    pack: &mut RawPack,
) -> crate::Result<Defined> {
    let sound = config
        .get("sound")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: missing \"sound\"", dir.display()))?;
    let path = dir.join(sound);
    let mut audio = decode_file(&path)?;
    dsp::dc_block(&mut audio.samples, audio.rate);
    let to_index = |ms: f64| ((ms / 1000.0) * f64::from(audio.rate)).round() as usize;

    // (code, is a key-up define, start ms, duration ms)
    let ranges: Vec<(u16, bool, f64, f64)> = defines
        .iter()
        .filter_map(|(key, define)| {
            let (code, up) = key.strip_suffix("-up").map_or((key.as_str(), false), |c| (c, true));
            let range = define.as_array()?;
            Some((code.parse().ok()?, up, range.first()?.as_f64()?, range.get(1)?.as_f64()?))
        })
        .collect();
    // Slices must not run into the next key's recording (one upstream key is 1194 ms
    // instead of ~194 ms; others end just after the next key's press): see `slice_end`.
    // Key-up defines are part of their own key's recording, so they are not neighbours.
    let mut starts: Vec<f64> = ranges.iter().filter(|r| !r.1).map(|r| r.2).collect();
    starts.sort_by(f64::total_cmp);
    starts.dedup();
    let onsets = onsets_ms(&audio.samples, audio.rate);

    let mut defined = Defined::default();
    let mut slices: HashMap<(usize, usize), usize> = HashMap::new();
    for (code, up, start, duration) in ranges {
        let end_ms = slice_end(start, duration, &starts, &onsets);
        let begin = to_index(start).min(audio.samples.len());
        let end = to_index(end_ms).min(audio.samples.len());
        if end <= begin {
            continue;
        }
        let index = match slices.get(&(begin, end)) {
            Some(&index) => index,
            None => {
                let what = path.display().to_string();
                let index = pack.push(audio.rate, audio.samples[begin..end].to_vec(), &what)?;
                slices.insert((begin, end), index);
                index
            }
        };
        if up {
            defined.release.insert(code, index);
        } else {
            defined.press.insert(code, index);
        }
    }
    Ok(defined)
}

/// Where a slice should end (ms). As defined, unless it overlaps the next distinct
/// define *and* has a sound of its own before that define: then it ends just before the
/// next key's actual onset, keeping its whole decay (and any release click recorded
/// before that) without the neighbour's press. A slice with no onset of its own points
/// at the next key's sound, as some upstream defines do; Mechvibes plays it that way, so
/// it is left alone. Starts within `NEAR_DUPLICATE_MS` are one recording defined twice.
fn slice_end(start: f64, duration: f64, starts: &[f64], onsets: &[f64]) -> f64 {
    let end = start + duration;
    let Some(next) = starts.iter().copied().find(|&s| s > start + NEAR_DUPLICATE_MS) else {
        return end;
    };
    if next >= end {
        return end;
    }
    let owns_a_sound =
        onsets.iter().any(|&t| t >= start - ONSET_TOLERANCE_MS && t < next - ONSET_TOLERANCE_MS);
    if !owns_a_sound {
        return end;
    }
    let next_onset =
        onsets.iter().copied().find(|&t| t >= next - ONSET_TOLERANCE_MS).unwrap_or(next);
    end.min(next_onset - ONSET_MARGIN_MS)
}

/// Times (ms) where a new sound starts: the 1 ms peak envelope jumps at least 15 dB above
/// its quietest point in the previous 10 ms while within 35 dB of the loudest moment.
/// (A key pressed during the previous key's decay rises only 15–19 dB out of it.)
/// Onsets closer than 20 ms are merged.
fn onsets_ms(samples: &[f32], rate: u32) -> Vec<f64> {
    let block = (rate as usize / 1000).max(1);
    let envelope: Vec<f32> =
        samples.chunks(block).map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs()))).collect();
    let loudest = envelope.iter().copied().fold(0.0f32, f32::max);
    let floor = loudest * dsp::db_to_lin(-35.0);
    let jump = dsp::db_to_lin(15.0);
    let ms_per_block = block as f64 * 1000.0 / f64::from(rate);
    let mut onsets: Vec<f64> = Vec::new();
    for i in 1..envelope.len() {
        let quiet = envelope[i.saturating_sub(10)..i].iter().copied().fold(f32::INFINITY, f32::min);
        if envelope[i] >= floor && envelope[i] >= quiet.max(1e-6) * jump {
            let t = i as f64 * ms_per_block;
            if onsets.last().is_none_or(|&last| t - last > 20.0) {
                onsets.push(t);
            }
        }
    }
    onsets
}

fn load_files(
    dir: &Path,
    defines: &Map<String, Value>,
    pack: &mut RawPack,
) -> crate::Result<HashMap<u16, usize>> {
    let mut by_code = HashMap::new();
    let mut files: HashMap<&str, usize> = HashMap::new();
    for (code, define) in defines {
        let Ok(code) = code.parse::<u16>() else { continue }; // skips "14-up" style keys
        let Some(file) = define.as_str() else { continue };
        let index = match files.get(file) {
            Some(&index) => index,
            None => {
                let path = dir.join(file);
                let mut audio = decode_file(&path)?;
                dsp::dc_block(&mut audio.samples, audio.rate);
                let index = pack.push(audio.rate, audio.samples, &path.display().to_string())?;
                files.insert(file, index);
                index
            }
        };
        by_code.insert(code, index);
    }
    Ok(by_code)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{click_i16, temp_dir, write_wav};
    use rvpack::keys::code;

    const RATE: u32 = 44_100;

    fn ms(m: usize) -> usize {
        m * RATE as usize / 1000
    }

    #[test]
    fn sprite_pack_slices_dedupes_and_falls_back() {
        let dir = temp_dir("mechvibes-sprite");
        let mut sprite = vec![0i16; ms(700)];
        for (at, amplitude) in [(100, 0.3), (300, 0.6), (500, 0.9)] {
            let click = click_i16(0, ms(50), amplitude);
            sprite[ms(at)..ms(at) + click.len()].copy_from_slice(&click);
        }
        write_wav(&dir.join("sound.wav"), RATE, 1, &sprite);
        std::fs::write(
            dir.join("config.json"),
            r#"{"key_define_type":"single","sound":"sound.wav","defines":{
                "30":[100,50],"57":[300,50],"28":[500,50],"3675":[100,50],"999":null}}"#,
        )
        .unwrap();

        let pack = load(&dir).unwrap();
        assert_eq!(pack.rate, RATE);
        assert_eq!(pack.clips.len(), 3, "identical slices are shared");
        let key = |c: u8| pack.press[usize::from(c)].unwrap();
        let (a, space, ret) = (key(code::A), key(code::SPACE), key(code::RETURN));
        assert!(a != space && space != ret && a != ret);
        assert_eq!(key(code::RIGHT_COMMAND), a, "right ⌘ → left ⌘ (same slice as A)");
        assert_eq!(key(0x0C), a, "Q is undefined → letter A");
        assert_eq!(pack.clips[a].len(), ms(50));
        assert!(pack.release.iter().all(Option::is_none));
    }

    #[test]
    fn key_up_defines_become_release_sounds_for_that_key_only() {
        let dir = temp_dir("mechvibes-up");
        let mut sprite = vec![0i16; ms(800)];
        for at in [0, 150, 400] {
            let click = click_i16(0, ms(60), 0.5);
            sprite[ms(at)..ms(at) + click.len()].copy_from_slice(&click);
        }
        write_wav(&dir.join("sound.wav"), RATE, 1, &sprite);
        std::fs::write(
            dir.join("config.json"),
            r#"{"key_define_type":"single","sound":"sound.wav","defines":{
                "30":[0,100],"30-up":[150,80],"57":[400,200]}}"#,
        )
        .unwrap();
        let pack = load(&dir).unwrap();
        let up = pack.release[usize::from(code::A)].expect("A has a key-up define");
        assert_eq!(pack.clips[up].len(), ms(80));
        assert_eq!(pack.clips[pack.press[usize::from(code::A)].unwrap()].len(), ms(100));
        assert_eq!(pack.release[usize::from(code::SPACE)], None, "no 57-up define");
        assert_eq!(pack.release[usize::from(code::S)], None, "no fallback to A's release");
    }

    #[test]
    fn slice_end_keeps_slices_that_overlap_nothing() {
        let starts = [0.0, 200.0, 600.0, 1_000.0];
        assert_eq!(slice_end(600.0, 300.0, &starts, &[0.0, 200.0, 600.0, 1_000.0]), 900.0);
        assert_eq!(slice_end(1_000.0, 250.0, &starts, &[1_000.0]), 1_250.0, "last slice");
    }

    #[test]
    fn slice_end_stops_just_before_the_next_keys_actual_onset() {
        // The next key is defined at 1400 but its sound starts at 1460.
        let starts = [1_000.0, 1_400.0];
        let onsets = [1_000.0, 1_460.0];
        assert_eq!(slice_end(1_000.0, 1_194.0, &starts, &onsets), 1_458.0);
    }

    #[test]
    fn slice_end_leaves_a_slice_without_its_own_onset_as_defined() {
        // Like upstream F4: defined 80 ms before the next key, whose sound it captures.
        let starts = [2_000.0, 2_080.0];
        assert_eq!(slice_end(2_000.0, 200.0, &starts, &[2_090.0]), 2_200.0);
    }

    #[test]
    fn slice_end_treats_nearby_starts_as_one_recording() {
        let starts = [1_700.0, 1_702.0];
        assert_eq!(slice_end(1_700.0, 120.0, &starts, &[1_701.0]), 1_820.0);
    }

    #[test]
    fn onsets_find_each_click_but_not_its_decay() {
        let mut samples = vec![0.0f32; 1_000 * 44];
        for at_ms in [100usize, 400, 750] {
            for (i, s) in click_i16(0, ms(150), 0.6).iter().enumerate() {
                samples[at_ms * 44 + i] += f32::from(*s) / 32_768.0;
            }
        }
        let found = onsets_ms(&samples, 44_000);
        assert_eq!(found.len(), 3, "{found:?}");
        for (got, want) in found.iter().zip([100.0, 400.0, 750.0]) {
            assert!((got - want).abs() <= 1.0, "{found:?}");
        }
    }

    #[test]
    fn slices_end_at_the_next_recording_but_keep_their_full_length_otherwise() {
        let dir = temp_dir("mechvibes-overlap");
        let mut sprite = vec![0i16; ms(2_000)];
        for at in [0, 200, 400, 600, 1_000, 1_400, 1_700] {
            let click = click_i16(0, ms(100), 0.5);
            sprite[ms(at)..ms(at) + click.len()].copy_from_slice(&click);
        }
        write_wav(&dir.join("sound.wav"), RATE, 1, &sprite);
        std::fs::write(
            dir.join("config.json"),
            r#"{"key_define_type":"single","sound":"sound.wav","defines":{
                "30":[0,100],"31":[200,100],"32":[400,100],
                "57":[600,300],
                "33":[1000,1194],"34":[1400,100],
                "35":[1700,120],"36":[1702,120]}}"#,
        )
        .unwrap();
        let pack = load(&dir).unwrap();
        let len = |mac: u8| pack.clips[pack.press[usize::from(mac)].unwrap()].len();
        assert_eq!(len(code::SPACE), ms(300), "a long slice that overlaps nothing is kept");
        let f = len(0x03) as i64;
        assert!((f - ms(398) as i64).abs() <= ms(1) as i64, "F ends just before G's press: {f}");
        assert_eq!(len(0x04), ms(120), "H and J start 2 ms apart: the same recording");
        assert_eq!(len(code::J), ms(120));
    }

    #[test]
    fn multiple_file_pack_shares_repeated_files() {
        let dir = temp_dir("mechvibes-multi");
        write_wav(&dir.join("a.wav"), RATE, 1, &click_i16(10, 2_000, 0.5));
        write_wav(&dir.join("space.wav"), RATE, 1, &click_i16(10, 3_000, 0.5));
        std::fs::write(
            dir.join("config.json"),
            r#"{"key_define_type":"multiple","defines":{
                "30":"a.wav","31":"a.wav","57":"space.wav","14-up":"missing.wav"}}"#,
        )
        .unwrap();
        let pack = load(&dir).unwrap();
        assert_eq!(pack.clips.len(), 2);
        assert_eq!(pack.press[usize::from(code::A)], pack.press[usize::from(code::S)]);
        assert_ne!(pack.press[usize::from(code::A)], pack.press[usize::from(code::SPACE)]);
    }
}
