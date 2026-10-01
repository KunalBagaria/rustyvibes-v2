//! Converts catalog entries into `.rvpack` files: RawPack → trimmed,
//! normalised, dithered PackData, with the spec's invariants checked before
//! anything is written.

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use rvpack::{ClipRef, KEY_SLOTS, Meta, PackData, keys};

use crate::catalog::{self, Entry, SourceKind};
use crate::dsp;
use crate::sources::{self, RawPack};

/// Summary of one converted pack.
#[derive(Debug)]
pub struct Report {
    pub id: String,
    pub clips: usize,
    pub bytes: usize,
    pub seconds: f32,
    pub gain_db: f32,
    pub max_lead_ms: f32,
    pub max_peak_db: f32,
    pub keys: usize,
    pub has_release: bool,
}

/// `cargo xtask packs`
pub fn run(root: &Path) -> crate::Result<()> {
    let started = Instant::now();
    let catalog = root.join("assets/soundpacks/catalog.json");
    let reports = build_all(&catalog, &root.join("target/packs"))?;
    println!(
        "{:<26} {:>5} {:>8} {:>7} {:>8} {:>8} {:>9} {:>5} {:>7}",
        "pack", "clips", "size", "audio", "gain", "lead", "peak", "keys", "release"
    );
    for r in &reports {
        println!(
            "{:<26} {:>5} {:>6}KB {:>6.1}s {:>+6.1}dB {:>6.2}ms {:>6.1}dBFS {:>5} {:>7}",
            r.id,
            r.clips,
            r.bytes / 1024,
            r.seconds,
            r.gain_db,
            r.max_lead_ms,
            r.max_peak_db,
            r.keys,
            if r.has_release { "yes" } else { "no" }
        );
    }
    let total: usize = reports.iter().map(|r| r.bytes).sum();
    println!(
        "{} packs, {:.1} MB, built in {:.1}s → target/packs",
        reports.len(),
        total as f64 / 1e6,
        started.elapsed().as_secs_f32()
    );
    Ok(())
}

/// Converts every catalog entry into `out_dir/<id>.rvpack` and removes stale packs.
pub fn build_all(catalog_path: &Path, out_dir: &Path) -> crate::Result<Vec<Report>> {
    let catalog = catalog::load(catalog_path)?;
    std::fs::create_dir_all(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let listing = std::fs::read_dir(out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    for item in listing.flatten() {
        let path = item.path();
        let stale = path.extension().is_some_and(|e| e == "rvpack")
            && !catalog.entries.iter().any(|c| path.file_stem().is_some_and(|s| *s == *c.id));
        if stale {
            std::fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
    }

    let mut reports = Vec::new();
    for entry in &catalog.entries {
        let raw = match entry.kind {
            SourceKind::Mechvibes => sources::mechvibes::load(&entry.dir)?,
            SourceKind::Kbsim => sources::kbsim::load(&entry.dir)?,
        };
        let (pack, gain_db) = finish(entry, entry.id == catalog.default, raw)?;
        let report = check(&pack, gain_db)?;
        let bytes = pack.to_bytes().map_err(|e| format!("{}: {e}", entry.id))?;
        let path = out_dir.join(format!("{}.rvpack", entry.id));
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        reports.push(Report { bytes: bytes.len(), ..report });
    }
    Ok(reports)
}

/// Cleans up the raw clips and assembles the pack. Returns it with the gain applied (dB).
pub fn finish(entry: &Entry, is_default: bool, raw: RawPack) -> crate::Result<(PackData, f32)> {
    let rate = raw.rate;
    let params = dsp::TrimParams::default();
    // Keep only clips that some key uses; silent clips are dropped.
    let used: BTreeSet<usize> = raw.press.iter().chain(&raw.release).flatten().copied().collect();
    let mut remap: Vec<Option<usize>> = vec![None; raw.clips.len()];
    let mut clips: Vec<Vec<f32>> = Vec::new();
    for &index in &used {
        if let Some(trimmed) = dsp::trim_clip(&raw.clips[index], rate, &params) {
            remap[index] = Some(clips.len());
            clips.push(trimmed);
        }
    }
    if clips.is_empty() {
        return Err(format!("{}: every clip is silent", entry.id));
    }
    if clips.len() > usize::from(u16::MAX) {
        return Err(format!("{}: too many clips", entry.id));
    }
    let fallback = raw.press[usize::from(keys::code::A)].and_then(|i| remap[i]).unwrap_or(0);

    let mut is_press = vec![false; clips.len()];
    let mut press = [ClipRef::NONE; KEY_SLOTS];
    let mut release = [ClipRef::NONE; KEY_SLOTS];
    for slot in 0..KEY_SLOTS {
        if let Some(index) = raw.press[slot] {
            let clip = remap[index].unwrap_or(fallback);
            is_press[clip] = true;
            press[slot] = ClipRef::single(clip as u16);
        }
        if let Some(clip) = raw.release[slot].and_then(|i| remap[i]) {
            release[slot] = ClipRef::single(clip as u16);
        }
    }
    let gain_db = dsp::normalize(&mut clips, &is_press, rate, dsp::TARGET_DB, dsp::CEILING_DB);
    let mut seed = fnv1a(entry.id.as_bytes()) | 1;
    let clips = clips.iter().map(|clip| dsp::to_i16(clip, &mut seed)).collect();
    let meta = Meta {
        id: entry.id.clone(),
        name: entry.name.clone(),
        variant: entry.variant.clone(),
        category: entry.category.clone(),
        color: entry.color.clone(),
        credit: entry.credit.clone(),
        order: entry.order,
        is_default,
    };
    Ok((PackData { sample_rate: rate, meta, press, release, clips }, gain_db))
}

/// Checks the invariants promised by the spec and summarises the pack.
pub fn check(pack: &PackData, gain_db: f32) -> crate::Result<Report> {
    let id = &pack.meta.id;
    let rate = pack.sample_rate as f32;
    let (mut max_lead_ms, mut max_peak, mut samples) = (0.0f32, 0i32, 0usize);
    for clip in &pack.clips {
        let peak = clip.iter().map(|&s| i32::from(s).abs()).max().unwrap_or(0);
        let threshold = ((peak as f32 * dsp::db_to_lin(-45.0)) as i32).max(2);
        let lead = clip.iter().position(|&s| i32::from(s).abs() >= threshold).unwrap_or(0);
        max_lead_ms = max_lead_ms.max(lead as f32 * 1000.0 / rate);
        max_peak = max_peak.max(peak);
        samples += clip.len();
    }
    let max_peak_db = dsp::lin_to_db(max_peak as f32 / 32_767.0);
    let mapped = keys::KEYS.iter().filter(|k| !pack.press[usize::from(k.code)].is_none()).count();
    if max_lead_ms > 1.0 {
        return Err(format!("{id}: a clip starts {max_lead_ms:.2} ms late (limit 1 ms)"));
    }
    if max_peak_db > dsp::CEILING_DB + 0.1 {
        return Err(format!(
            "{id}: a clip peaks at {max_peak_db:.2} dBFS (ceiling {} dBFS)",
            dsp::CEILING_DB
        ));
    }
    if mapped != keys::KEYS.len() {
        return Err(format!("{id}: only {mapped} of {} keys have a sound", keys::KEYS.len()));
    }
    Ok(Report {
        id: id.clone(),
        clips: pack.clips.len(),
        bytes: 0,
        seconds: samples as f32 / rate,
        gain_db,
        max_lead_ms,
        max_peak_db,
        keys: mapped,
        has_release: pack.has_release(),
    })
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811C_9DC5_u32, |h, &b| (h ^ u32::from(b)).wrapping_mul(0x0100_0193))
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;
    use std::path::PathBuf;

    fn entry() -> Entry {
        Entry {
            id: "test".into(),
            name: "Test".into(),
            variant: String::new(),
            category: "linear".into(),
            color: "#000000".into(),
            credit: "unit test".into(),
            kind: SourceKind::Kbsim,
            dir: PathBuf::new(),
            order: 0,
        }
    }

    fn click(amplitude: f32) -> Vec<f32> {
        let mut c = vec![0.0; 1_000];
        c.extend((0..4_000).map(|i| {
            let t = i as f32 / 44_100.0;
            amplitude * (-t * 120.0).exp() * (2.0 * PI * 1_800.0 * t).sin()
        }));
        c
    }

    #[test]
    fn finish_trims_normalises_and_maps_every_key() {
        let mut raw = RawPack::new(44_100);
        let a = raw.push(44_100, click(0.2), "a").unwrap();
        let silent = raw.push(44_100, vec![0.0; 500], "silent").unwrap();
        let up = raw.push(44_100, click(0.05), "up").unwrap();
        raw.push(44_100, click(0.9), "unused").unwrap();
        for key in keys::KEYS {
            raw.press[usize::from(key.code)] = Some(a);
            raw.release[usize::from(key.code)] = Some(up);
        }
        raw.press[usize::from(keys::code::SPACE)] = Some(silent);

        let (pack, _) = finish(&entry(), true, raw).unwrap();
        assert_eq!(pack.clips.len(), 2, "silent and unused clips are dropped");
        assert_eq!(
            pack.press[usize::from(keys::code::SPACE)],
            pack.press[usize::from(keys::code::A)],
            "a silent clip falls back to A"
        );
        assert!(pack.meta.is_default);
        let report = check(&pack, 0.0).unwrap();
        assert!(report.max_lead_ms <= 1.0);
        assert!(report.max_peak_db <= dsp::CEILING_DB + 0.1);
        assert!(report.has_release);
        assert_eq!(report.keys, keys::KEYS.len());
    }

    #[test]
    fn check_rejects_late_onsets() {
        let mut raw = RawPack::new(44_100);
        let a = raw.push(44_100, click(0.3), "a").unwrap();
        for key in keys::KEYS {
            raw.press[usize::from(key.code)] = Some(a);
        }
        let (mut pack, gain) = finish(&entry(), false, raw).unwrap();
        let mut late = vec![0i16; 200];
        late.extend_from_slice(&pack.clips[0]);
        pack.clips[0] = late;
        let err = check(&pack, gain).unwrap_err();
        assert!(err.contains("late"), "{err}");
    }

    #[test]
    #[ignore = "decodes the full catalog; run: cargo test -p xtask --profile xtask -- --ignored"]
    fn shipped_catalog_converts() {
        let root = crate::workspace_root();
        let out = crate::testutil::temp_dir("packs");
        let reports = build_all(&root.join("assets/soundpacks/catalog.json"), &out).unwrap();
        assert_eq!(reports.len(), 21);
        assert!(reports.iter().all(|r| r.max_lead_ms <= 1.0));
    }
}
