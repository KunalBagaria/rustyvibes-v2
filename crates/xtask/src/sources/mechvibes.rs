//! Mechvibes packs: `config.json` plus either one sprite sliced by
//! `[start_ms, duration_ms]` (`"key_define_type": "single"`) or one file per
//! key (`"multiple"`). Keys are libuiohook scan codes.

use std::collections::HashMap;
use std::path::Path;

use rvpack::{keys, mechvibes};
use serde_json::{Map, Value};

use super::RawPack;
use crate::{decode::decode_file, dsp};

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
    let by_code = match config.get("key_define_type").and_then(Value::as_str).unwrap_or("single") {
        "single" => load_sprite(dir, &config, defines, &mut pack)?,
        _ => load_files(dir, defines, &mut pack)?,
    };
    if pack.clips.is_empty() {
        return Err(format!("{}: no sounds defined", dir.display()));
    }
    let fallback = by_code.get(&mechvibes::CODE_A).copied().unwrap_or(0);
    for key in keys::KEYS {
        let clip = mechvibes::candidates(key.code)
            .iter()
            .find_map(|code| by_code.get(code).copied())
            .unwrap_or(fallback);
        pack.press[usize::from(key.code)] = Some(clip);
    }
    Ok(pack)
}

fn load_sprite(
    dir: &Path,
    config: &Value,
    defines: &Map<String, Value>,
    pack: &mut RawPack,
) -> crate::Result<HashMap<u16, usize>> {
    let sound = config
        .get("sound")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{}: missing \"sound\"", dir.display()))?;
    let path = dir.join(sound);
    let mut audio = decode_file(&path)?;
    dsp::dc_block(&mut audio.samples, audio.rate);
    let to_index = |ms: f64| ((ms / 1000.0) * f64::from(audio.rate)).round() as usize;

    let ranges: Vec<(u16, f64, f64)> = defines
        .iter()
        .filter_map(|(code, define)| {
            let code = code.parse::<u16>().ok()?;
            let range = define.as_array()?;
            Some((code, range.first()?.as_f64()?, range.get(1)?.as_f64()?))
        })
        .collect();
    // Guard against config typos (one upstream key is 1194 ms instead of ~194 ms,
    // which would play several neighbouring keys): cap every slice at 1.5× the
    // pack's median slice length.
    let median = dsp::median(ranges.iter().map(|r| r.2 as f32).collect());
    let max_duration = f64::from(median) * 1.5;

    let mut by_code = HashMap::new();
    let mut slices: HashMap<(usize, usize), usize> = HashMap::new();
    for (code, start, duration) in ranges {
        let duration = duration.min(max_duration);
        let begin = to_index(start).min(audio.samples.len());
        let end = to_index(start + duration).min(audio.samples.len());
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
        by_code.insert(code, index);
    }
    Ok(by_code)
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
    fn absurdly_long_slices_are_capped() {
        let dir = temp_dir("mechvibes-cap");
        write_wav(&dir.join("sound.wav"), RATE, 1, &click_i16(0, ms(2_000), 0.5));
        std::fs::write(
            dir.join("config.json"),
            r#"{"key_define_type":"single","sound":"sound.wav","defines":{
                "30":[0,100],"31":[200,100],"32":[400,100],"33":[600,1194]}}"#,
        )
        .unwrap();
        let pack = load(&dir).unwrap();
        let f = pack.press[0x03].unwrap(); // the F key (Mechvibes code 33)
        assert_eq!(pack.clips[f].len(), ms(150), "capped at 1.5 × the 100 ms median");
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
