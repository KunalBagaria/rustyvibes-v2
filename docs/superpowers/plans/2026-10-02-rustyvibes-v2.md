# Rustyvibes 2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build Rustyvibes 2 — a native, hyper-efficient macOS menu bar app in Rust that plays bundled mechanical-keyboard soundpacks on every key press — and package it as a signed, universal `.app` and `.dmg`.

**Architecture:** A Cargo workspace with three crates. `rvpack` defines the zero-copy `.rvpack` soundpack format and keyboard geometry. `xtask` converts MIT-licensed source recordings into `.rvpack` files at build time (decode, trim, normalise) and bundles/signs the app. `rustyvibes` is the app: a listen-only `CGEventTap` thread turns key transitions into lock-free commands, a CoreAudio AUHAL render callback mixes voices from `mmap`ed packs, a control thread starts/stops the audio unit on demand, and an AppKit (objc2) status-item menu drives settings stored in `NSUserDefaults`.

**Tech Stack:** Rust 2024 (rustc 1.97), objc2 0.6.4 / objc2-foundation 0.3.2 / objc2-app-kit 0.3.2 / block2 0.6.2, libc 0.2, hand-written FFI to AudioToolbox/CoreGraphics/CoreFoundation/ServiceManagement, symphonia 0.6.1 + serde_json (build tool only), Swift (icon rendering script only), macOS `codesign`/`lipo`/`hdiutil`/`iconutil`.

**Spec:** `docs/superpowers/specs/2026-10-02-rustyvibes-v2-design.md`

## Global Constraints

- macOS only; `LSMinimumSystemVersion` and `MACOSX_DEPLOYMENT_TARGET` = `13.0`.
- Bundle id `io.github.kb24x7.rustyvibes`, app name `Rustyvibes`, version `2.0.0`, `LSUIElement` = true.
- Universal binary: `aarch64-apple-darwin` + `x86_64-apple-darwin`.
- Release profile: `opt-level = 3`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
- The runtime app contains no audio decoder and makes no network requests.
- Real-time rule: the CoreAudio render callback never allocates, locks, logs, or calls Objective-C.
- Event-tap callback: no allocation, no locks, no Objective-C.
- Settings keys (NSUserDefaults): `Enabled` (default true), `Pack` (default from catalog: `cherry-mx-brown-pbt`), `Volume` (0.6), `ReleaseSounds` (true), `Variation` (true), `Spatial` (true).
- Audio: 128-frame buffer requested; stop the audio unit after 8 s of silence; 32 voices; volume gain = slider².
- Variation: ±35 cents pitch, ±1.5 dB gain. Spatial: equal-power pan, width 0.4, centred key = unity gain.
- Pack conversion: onset threshold max(−55 dBFS, peak − 45 dB) with 0.5 ms pre-roll; tail threshold peak − 50 dB + 6 ms raised-cosine fade-out; loudness target −22 dBFS (60 ms attack-window RMS, median of press clips); per-clip peak ceiling −1 dBFS; TPDF dither to i16.
- Soundpack sources: `hainguyents13/mechvibes` @ `326252a13e7bef4f1c35d08ef0189b5af6f8ba02`, `tplai/kbsim` @ `ba103f3b0afa9dab80447aa2e7e2ed80b6bd80e4`, both MIT; licences shipped inside the app.
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo fmt --check` must pass at the end of every task.
- Commit after every task with a message ending in `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **A saved soundpack id that no longer exists** (pack renamed/removed in an update) must fall back to the catalog default instead of crashing or going silent — test in Task 6 (`resolve_unknown_id_falls_back_to_default`).
2. **Key mashing / rolling many keys at once** (more simultaneous sounds than voices, bursts that fill the command ring) must steal the oldest voice, drop and count excess commands, never panic, and keep output within [−1, 1] — tests in Task 5 (`steals_oldest_voice_when_full`, `limiter_keeps_output_within_unity`) and Task 6 (`full_ring_drops_and_counts`).
3. **Output devices at unusual sample rates** (96 kHz interfaces, 44.1 kHz headsets) must keep pitch and duration correct — test in Task 5 (`resampling_to_96k_preserves_duration`).
4. **Releasing one modifier while another is held, and keyboards/remote-desktop tools that omit device-dependent flag bits,** must yield exactly one press and one release per modifier — tests in Task 8 (`releasing_left_shift_while_right_held`, `modifiers_without_device_bits_toggle`).
5. **Corrupt or truncated `.rvpack` files** in the packs directory must be skipped while the remaining packs load — test in Task 6 (`load_dir_skips_corrupt_files`).

## File Structure

```
Cargo.toml                         workspace + profiles
.cargo/config.toml                 `cargo xtask` alias, deployment target
crates/rvpack/                     soundpack format + keyboard geometry (no deps)
  src/lib.rs                       re-exports
  src/format.rs                    .rvpack writer (PackData) and zero-copy reader (PackView)
  src/keys.rs                      macOS keycode table: name, row, x-position
  src/mechvibes.rs                 macOS keycode → libuiohook code candidates
crates/xtask/                      build tooling (`cargo xtask …`)
  src/main.rs                      command dispatch
  src/decode.rs                    symphonia → mono f32
  src/dsp.rs                       DC block, trim, fades, loudness, dither
  src/catalog.rs                   assets/soundpacks/catalog.json model
  src/sources/{mod,mechvibes,kbsim}.rs   source-layout adapters → RawPack
  src/packs.rs                     RawPack → PackData → target/packs/*.rvpack (+ invariants)
  src/bundle.rs                    universal build, Info.plist, .app assembly, codesign
  src/dmg.rs                       DMG creation
  src/testutil.rs                  WAV writer + temp dirs for tests
crates/rustyvibes/                 the app
  src/main.rs                      CLI flags (diagnostics) → UI
  src/log.rs                       RUSTYVIBES_LOG-gated stderr logging
  src/engine/{mod,ring,rng,play,mixer}.rs   lock-free ring, RNG, voice mixer, Shared state, Voicer
  src/packs.rs                     mmap + parse + residency of .rvpack files (Library)
  src/audio/{mod,ffi}.rs           AUHAL output, control thread, render callback
  src/input/{mod,ffi,keystate}.rs  CGEventTap thread, key/modifier state machine
  src/settings.rs                  Settings + NSUserDefaults store
  src/runtime.rs                   wires library, engine, audio, input
  src/diagnostics.rs               --selftest, --bench, --tap-test, --snapshot
  src/ui/{mod,delegate,menu,views,icons,onboarding,about,login}.rs   AppKit UI
assets/soundpacks/                 catalog.json, SOURCES.md, original recordings + licences
assets/icon/                       AppIcon.icns, AppIcon-1024.png
scripts/make-icon.swift            renders the app icon
README.md, LICENSE, THIRD_PARTY_NOTICES.md
```

---

### Task 1: Workspace scaffold and the `.rvpack` format

**Files:**
- Create: `Cargo.toml`, `.cargo/config.toml`, `crates/rvpack/Cargo.toml`, `crates/rvpack/src/lib.rs`, `crates/rvpack/src/format.rs`
- Modify: `.gitignore`
- Test: unit tests inside `crates/rvpack/src/format.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 4, 6):
  - `rvpack::KEY_SLOTS: usize = 128`, `GUARD_BEFORE: usize = 2`, `GUARD_AFTER: usize = 3`
  - `struct ClipRef { first: u16, count: u16 }` with `NONE`, `single(u16)`, `is_none()`
  - `struct Meta { id, name, variant, category, color, credit: String, order: u16, is_default: bool }`
  - `struct PackData { sample_rate: u32, meta: Meta, press: [ClipRef; 128], release: [ClipRef; 128], clips: Vec<Vec<i16>> }` with `to_bytes() -> Result<Vec<u8>, FormatError>`, `has_release()`
  - `struct PackView<'a>` with `parse(&'a [u8]) -> Result<Self, FormatError>`, pub fields `sample_rate: u32`, `flags: u16`, `meta: Meta`, methods `has_release()`, `press(u8) -> ClipRef`, `release(u8) -> ClipRef`, `clip_count()`, `clip(usize) -> &'a [i16]`, `clip_with_guards(usize) -> &'a [i16]`, `pcm() -> &'a [i16]`
  - `enum FormatError` (Display + Error)

- [ ] **Step 1: Create the workspace files**

**File:** `Cargo.toml`
```toml
[workspace]
resolver = "3"
members = ["crates/*"]

[workspace.package]
version = "2.0.0"
edition = "2024"
rust-version = "1.85"
license = "MIT"
authors = ["Kunal Bagaria <kb24x7@gmail.com>"]
repository = "https://github.com/kb24x7/rustyvibes"

[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

# Optimised but quick to build: used by `cargo xtask`.
[profile.xtask]
inherits = "release"
lto = false
codegen-units = 16
panic = "unwind"
strip = false
incremental = true
```

**File:** `.cargo/config.toml`
```toml
[alias]
xtask = "run --quiet --package xtask --profile xtask --"

[env]
MACOSX_DEPLOYMENT_TARGET = "13.0"
```

**File:** `.gitignore`
```
/target/
**/*.rs.bk
.DS_Store
```

**File:** `crates/rvpack/Cargo.toml`
```toml
[package]
name = "rvpack"
description = "The Rustyvibes soundpack format and keyboard geometry"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[lints]
workspace = true
```

**File:** `crates/rvpack/src/lib.rs`
```rust
//! The `.rvpack` soundpack format shared by the Rustyvibes app (reader) and its
//! build tooling (writer).

pub mod format;

pub use format::{
    Clip, ClipRef, FormatError, GUARD_AFTER, GUARD_BEFORE, KEY_SLOTS, Meta, PackData, PackView,
};
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/rvpack/src/format.rs`
```rust
//! The `.rvpack` binary soundpack format.

#[cfg(test)]
mod tests {
    use super::*;

    /// Copies bytes into 8-byte aligned storage, like an `mmap`ed file.
    struct Aligned {
        words: Vec<u64>,
        len: usize,
    }

    impl Aligned {
        fn new(bytes: &[u8]) -> Aligned {
            let mut words = vec![0u64; bytes.len().div_ceil(8)];
            for (word, chunk) in words.iter_mut().zip(bytes.chunks(8)) {
                let mut buf = [0u8; 8];
                buf[..chunk.len()].copy_from_slice(chunk);
                *word = u64::from_ne_bytes(buf);
            }
            Aligned { words, len: bytes.len() }
        }

        fn bytes(&self) -> &[u8] {
            // SAFETY: `words` owns at least `len` initialised bytes.
            unsafe { std::slice::from_raw_parts(self.words.as_ptr().cast::<u8>(), self.len) }
        }
    }

    fn sample_pack() -> PackData {
        let mut press = [ClipRef::NONE; KEY_SLOTS];
        let mut release = [ClipRef::NONE; KEY_SLOTS];
        press[0x00] = ClipRef::single(0);
        press[0x31] = ClipRef { first: 1, count: 2 };
        release[0x31] = ClipRef::single(2);
        PackData {
            sample_rate: 44_100,
            meta: Meta {
                id: "test-pack".into(),
                name: "Test Switch".into(),
                variant: "PBT".into(),
                category: "tactile".into(),
                color: "#FF8800".into(),
                credit: "Unit test".into(),
                order: 7,
                is_default: true,
            },
            press,
            release,
            clips: vec![vec![100, -200, 300], vec![1; 10], vec![-5; 4]],
        }
    }

    #[test]
    fn roundtrip_preserves_everything() {
        let pack = sample_pack();
        let bytes = pack.to_bytes().unwrap();
        let buf = Aligned::new(&bytes);
        let view = PackView::parse(buf.bytes()).unwrap();

        assert_eq!(view.sample_rate, 44_100);
        assert_eq!(view.meta, pack.meta);
        assert!(view.has_release());
        assert_eq!(view.press(0x00), ClipRef::single(0));
        assert_eq!(view.press(0x31), ClipRef { first: 1, count: 2 });
        assert_eq!(view.release(0x31), ClipRef::single(2));
        assert_eq!(view.press(0x01), ClipRef::NONE);
        assert_eq!(view.press(200), ClipRef::NONE);
        assert_eq!(view.release(200), ClipRef::NONE);
        assert_eq!(view.clip_count(), 3);
        for (i, clip) in pack.clips.iter().enumerate() {
            assert_eq!(view.clip(i), clip.as_slice());
        }
    }

    #[test]
    fn clips_are_surrounded_by_zero_guards() {
        let bytes = sample_pack().to_bytes().unwrap();
        let buf = Aligned::new(&bytes);
        let view = PackView::parse(buf.bytes()).unwrap();
        for i in 0..view.clip_count() {
            let guarded = view.clip_with_guards(i);
            let audio = view.clip(i);
            assert_eq!(guarded.len(), GUARD_BEFORE + audio.len() + GUARD_AFTER);
            assert!(guarded[..GUARD_BEFORE].iter().all(|&s| s == 0));
            assert_eq!(&guarded[GUARD_BEFORE..GUARD_BEFORE + audio.len()], audio);
            assert!(guarded[GUARD_BEFORE + audio.len()..].iter().all(|&s| s == 0));
        }
    }

    #[test]
    fn pcm_offset_is_16_byte_aligned() {
        let bytes = sample_pack().to_bytes().unwrap();
        let offset = u32::from_le_bytes(bytes[16..20].try_into().unwrap());
        assert_eq!(offset % 16, 0);
    }

    #[test]
    fn pack_without_release_sounds_clears_the_flag() {
        let mut pack = sample_pack();
        pack.release = [ClipRef::NONE; KEY_SLOTS];
        let bytes = pack.to_bytes().unwrap();
        let buf = Aligned::new(&bytes);
        assert!(!PackView::parse(buf.bytes()).unwrap().has_release());
    }

    #[test]
    fn every_truncation_is_rejected() {
        let bytes = sample_pack().to_bytes().unwrap();
        for len in 0..bytes.len() {
            let buf = Aligned::new(&bytes[..len]);
            assert!(PackView::parse(buf.bytes()).is_err(), "accepted {len} bytes");
        }
    }

    #[test]
    fn rejects_bad_magic_and_version() {
        let mut bytes = sample_pack().to_bytes().unwrap();
        bytes[0] = b'X';
        assert_eq!(PackView::parse(Aligned::new(&bytes).bytes()).unwrap_err(), FormatError::BadMagic);

        let mut bytes = sample_pack().to_bytes().unwrap();
        bytes[4] = 9;
        assert_eq!(
            PackView::parse(Aligned::new(&bytes).bytes()).unwrap_err(),
            FormatError::UnsupportedVersion(9)
        );
    }

    #[test]
    fn rejects_clip_ref_past_the_clip_table() {
        let mut bytes = sample_pack().to_bytes().unwrap();
        // press[0].count lives at byte 32 + 2.
        bytes[34] = 9;
        assert_eq!(
            PackView::parse(Aligned::new(&bytes).bytes()).unwrap_err(),
            FormatError::BadClipRef { key: 0 }
        );
    }

    #[test]
    fn rejects_clip_without_guard_room() {
        let mut bytes = sample_pack().to_bytes().unwrap();
        // clip[0].start lives at byte 1056.
        bytes[1056..1060].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(
            PackView::parse(Aligned::new(&bytes).bytes()).unwrap_err(),
            FormatError::BadClip { index: 0 }
        );
    }

    #[test]
    fn rejects_misaligned_pcm() {
        let bytes = sample_pack().to_bytes().unwrap();
        let mut shifted = vec![0u8];
        shifted.extend_from_slice(&bytes);
        let buf = Aligned::new(&shifted);
        assert_eq!(PackView::parse(&buf.bytes()[1..]).unwrap_err(), FormatError::Misaligned);
    }

    #[test]
    fn writer_rejects_invalid_packs() {
        let mut pack = sample_pack();
        pack.meta.name = "two\nlines".into();
        assert_eq!(pack.to_bytes().unwrap_err(), FormatError::BadMeta);

        let mut pack = sample_pack();
        pack.press[5] = ClipRef { first: 2, count: 2 };
        assert_eq!(pack.to_bytes().unwrap_err(), FormatError::BadClipRef { key: 5 });

        let mut pack = sample_pack();
        pack.clips[1].clear();
        assert_eq!(pack.to_bytes().unwrap_err(), FormatError::BadClip { index: 1 });

        let mut pack = sample_pack();
        pack.sample_rate = 0;
        assert_eq!(pack.to_bytes().unwrap_err(), FormatError::BadSampleRate);
    }

    #[test]
    fn metadata_ignores_unknown_keys() {
        let meta = Meta::decode(b"id=x\nfuture=1\norder=3\n").unwrap();
        assert_eq!(meta.id, "x");
        assert_eq!(meta.order, 3);
        assert!(Meta::decode(b"name=no id\n").is_err());
        assert!(Meta::decode(b"id=x\nno equals sign\n").is_err());
    }

    #[test]
    fn random_corruption_never_panics() {
        let original = sample_pack().to_bytes().unwrap();
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        for _ in 0..5_000 {
            let mut bytes = original.clone();
            for _ in 0..4 {
                let r = next();
                // Concentrate on header, maps, clip table and metadata.
                let pos = (r % 1_200.min(bytes.len() as u64)) as usize;
                bytes[pos] = (r >> 40) as u8;
            }
            let buf = Aligned::new(&bytes);
            if let Ok(view) = PackView::parse(buf.bytes()) {
                for k in 0..=255u8 {
                    let _ = (view.press(k), view.release(k));
                }
                for i in 0..view.clip_count() {
                    let _ = view.clip_with_guards(i);
                }
            }
        }
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p rvpack`
Expected: compile errors — `PackData`, `PackView`, `Meta`, `ClipRef`, `FormatError` not found.

- [ ] **Step 4: Implement the format** (prepend to `format.rs`, keeping the test module at the bottom)

**File:** `crates/rvpack/src/format.rs` (implementation part; the file is this code followed by the Step 2 test module)
```rust
//! The `.rvpack` binary soundpack format.
//!
//! Layout (all integers little-endian):
//!
//! | offset | size  | field                                                 |
//! |-------:|------:|-------------------------------------------------------|
//! | 0      | 4     | magic `RVPK`                                          |
//! | 4      | 2     | format version (1)                                    |
//! | 6      | 2     | flags (bit 0: the pack has release sounds)            |
//! | 8      | 4     | sample rate in Hz (mono audio)                        |
//! | 12     | 4     | clip count                                            |
//! | 16     | 4     | PCM byte offset (16-byte aligned)                     |
//! | 20     | 4     | PCM length in `i16` samples                           |
//! | 24     | 4     | metadata byte offset                                  |
//! | 28     | 4     | metadata byte length                                  |
//! | 32     | 512   | press map: 128 × (`first: u16`, `count: u16`)         |
//! | 544    | 512   | release map, same shape                               |
//! | 1056   | 8 × n | clip table: n × (`start: u32`, `len: u32`), samples   |
//! | …      | …     | metadata: UTF-8 `key=value` lines                     |
//! | …      | …     | PCM: mono `i16`                                       |
//!
//! Maps are indexed by macOS virtual keycode. Every clip has at least
//! [`GUARD_BEFORE`] zero samples in front of it and [`GUARD_AFTER`] behind it,
//! so an interpolating reader can look one sample back and two ahead without
//! leaving the PCM block.

use std::fmt;

#[cfg(target_endian = "big")]
compile_error!("rvpack assumes a little-endian host");

/// File magic.
pub const MAGIC: [u8; 4] = *b"RVPK";
/// Current format version.
pub const VERSION: u16 = 1;
/// Number of keycode slots per key map (macOS virtual keycodes are 7-bit).
pub const KEY_SLOTS: usize = 128;
/// Flag bit: the pack contains release sounds.
pub const FLAG_HAS_RELEASE: u16 = 1;
/// Zero samples guaranteed in front of every clip.
pub const GUARD_BEFORE: usize = 2;
/// Zero samples guaranteed behind every clip.
pub const GUARD_AFTER: usize = 3;

/// Zero samples the writer places around clips; covers both guards.
const GAP: usize = 4;
const HEADER_LEN: usize = 32;
const MAP_LEN: usize = KEY_SLOTS * 4;
const PRESS_OFFSET: usize = HEADER_LEN;
const CLIPS_OFFSET: usize = PRESS_OFFSET + 2 * MAP_LEN;
const CLIP_ENTRY_LEN: usize = 8;

/// A run of candidate clips for one key: `count` clips starting at `first`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ClipRef {
    pub first: u16,
    pub count: u16,
}

impl ClipRef {
    /// No sound for this key.
    pub const NONE: ClipRef = ClipRef { first: 0, count: 0 };

    /// Exactly one candidate clip.
    pub const fn single(index: u16) -> ClipRef {
        ClipRef { first: index, count: 1 }
    }

    pub const fn is_none(self) -> bool {
        self.count == 0
    }
}

/// Location of a clip inside the PCM block, in samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    pub start: u32,
    pub len: u32,
}

/// Human-facing description of a pack.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Meta {
    /// Stable identifier, e.g. `cherry-mx-brown-pbt`.
    pub id: String,
    /// Switch name, e.g. `Cherry MX Brown`.
    pub name: String,
    /// Optional variant such as `PBT`; empty when there is none.
    pub variant: String,
    /// `linear`, `tactile` or `clicky`.
    pub category: String,
    /// Stem colour as `#RRGGBB`, used for the menu glyph.
    pub color: String,
    /// Who recorded the sounds, and the licence.
    pub credit: String,
    /// Position in the catalog.
    pub order: u16,
    /// Whether this pack is selected on first launch.
    pub is_default: bool,
}

impl Meta {
    fn encode(&self) -> Result<String, FormatError> {
        if self.id.is_empty() {
            return Err(FormatError::BadMeta);
        }
        let text_fields = [
            ("id", &self.id),
            ("name", &self.name),
            ("variant", &self.variant),
            ("category", &self.category),
            ("color", &self.color),
            ("credit", &self.credit),
        ];
        let mut out = String::new();
        for (key, value) in text_fields {
            if value.contains(['\n', '\r']) {
                return Err(FormatError::BadMeta);
            }
            out.push_str(key);
            out.push('=');
            out.push_str(value);
            out.push('\n');
        }
        out.push_str(&format!("order={}\n", self.order));
        out.push_str(&format!("default={}\n", u8::from(self.is_default)));
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> Result<Meta, FormatError> {
        let text = std::str::from_utf8(bytes).map_err(|_| FormatError::BadMeta)?;
        let mut meta = Meta::default();
        for line in text.lines() {
            let (key, value) = line.split_once('=').ok_or(FormatError::BadMeta)?;
            match key {
                "id" => meta.id = value.to_owned(),
                "name" => meta.name = value.to_owned(),
                "variant" => meta.variant = value.to_owned(),
                "category" => meta.category = value.to_owned(),
                "color" => meta.color = value.to_owned(),
                "credit" => meta.credit = value.to_owned(),
                "order" => meta.order = value.parse().map_err(|_| FormatError::BadMeta)?,
                "default" => meta.is_default = value == "1",
                _ => {} // Unknown keys are ignored so newer writers stay readable.
            }
        }
        if meta.id.is_empty() {
            return Err(FormatError::BadMeta);
        }
        Ok(meta)
    }
}

/// Why a pack could not be written or read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormatError {
    TooShort,
    BadMagic,
    UnsupportedVersion(u16),
    BadSampleRate,
    Misaligned,
    OutOfBounds(&'static str),
    BadClipRef { key: usize },
    BadClip { index: usize },
    BadMeta,
    TooManyClips,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::TooShort => write!(f, "file is too short to be a soundpack"),
            FormatError::BadMagic => write!(f, "not a soundpack (bad magic)"),
            FormatError::UnsupportedVersion(v) => write!(f, "unsupported soundpack version {v}"),
            FormatError::BadSampleRate => write!(f, "invalid sample rate"),
            FormatError::Misaligned => write!(f, "PCM data is not 2-byte aligned in memory"),
            FormatError::OutOfBounds(what) => write!(f, "{what} lies outside the file"),
            FormatError::BadClipRef { key } => write!(f, "key slot {key} refers to a missing clip"),
            FormatError::BadClip { index } => {
                write!(f, "clip {index} is empty or lacks guard samples")
            }
            FormatError::BadMeta => write!(f, "metadata is malformed"),
            FormatError::TooManyClips => write!(f, "too many clips (maximum 65535)"),
        }
    }
}

impl std::error::Error for FormatError {}

/// An owned pack, as produced by the build tooling.
#[derive(Clone, Debug, PartialEq)]
pub struct PackData {
    pub sample_rate: u32,
    pub meta: Meta,
    pub press: [ClipRef; KEY_SLOTS],
    pub release: [ClipRef; KEY_SLOTS],
    /// Clip audio without guard samples; the writer adds them.
    pub clips: Vec<Vec<i16>>,
}

impl PackData {
    pub fn has_release(&self) -> bool {
        self.release.iter().any(|r| !r.is_none())
    }

    /// Serializes the pack.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FormatError> {
        if self.sample_rate == 0 {
            return Err(FormatError::BadSampleRate);
        }
        if self.clips.len() > usize::from(u16::MAX) {
            return Err(FormatError::TooManyClips);
        }
        if let Some(index) = self.clips.iter().position(Vec::is_empty) {
            return Err(FormatError::BadClip { index });
        }
        for (slot, r) in self.press.iter().chain(&self.release).enumerate() {
            if !r.is_none() && usize::from(r.first) + usize::from(r.count) > self.clips.len() {
                return Err(FormatError::BadClipRef { key: slot % KEY_SLOTS });
            }
        }
        let meta = self.meta.encode()?;

        let mut pcm: Vec<i16> = vec![0; GAP];
        let mut table = Vec::with_capacity(self.clips.len());
        for clip in &self.clips {
            table.push(Clip { start: pcm.len() as u32, len: clip.len() as u32 });
            pcm.extend_from_slice(clip);
            pcm.extend(std::iter::repeat_n(0, GAP));
        }
        let meta_offset = CLIPS_OFFSET + table.len() * CLIP_ENTRY_LEN;
        let pcm_offset = (meta_offset + meta.len()).next_multiple_of(16);
        let to_u32 = |n: usize, what: &'static str| {
            u32::try_from(n).map_err(|_| FormatError::OutOfBounds(what))
        };

        let mut out = Vec::with_capacity(pcm_offset + pcm.len() * 2);
        out.extend_from_slice(&MAGIC);
        out.extend_from_slice(&VERSION.to_le_bytes());
        let flags = if self.has_release() { FLAG_HAS_RELEASE } else { 0 };
        out.extend_from_slice(&flags.to_le_bytes());
        out.extend_from_slice(&self.sample_rate.to_le_bytes());
        out.extend_from_slice(&(table.len() as u32).to_le_bytes());
        out.extend_from_slice(&to_u32(pcm_offset, "pcm")?.to_le_bytes());
        out.extend_from_slice(&to_u32(pcm.len(), "pcm")?.to_le_bytes());
        out.extend_from_slice(&to_u32(meta_offset, "metadata")?.to_le_bytes());
        out.extend_from_slice(&to_u32(meta.len(), "metadata")?.to_le_bytes());
        for r in self.press.iter().chain(&self.release) {
            out.extend_from_slice(&r.first.to_le_bytes());
            out.extend_from_slice(&r.count.to_le_bytes());
        }
        for clip in &table {
            out.extend_from_slice(&clip.start.to_le_bytes());
            out.extend_from_slice(&clip.len.to_le_bytes());
        }
        out.extend_from_slice(meta.as_bytes());
        out.resize(pcm_offset, 0);
        for sample in &pcm {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(out)
    }
}

/// A validated, zero-copy view of a serialized pack.
#[derive(Clone, Debug)]
pub struct PackView<'a> {
    pub sample_rate: u32,
    pub flags: u16,
    pub meta: Meta,
    maps: &'a [u8],
    clips: &'a [u8],
    pcm: &'a [i16],
}

impl<'a> PackView<'a> {
    /// Validates `bytes` and returns a view into them. The PCM block must be
    /// 2-byte aligned in memory, which holds for `mmap`ed files.
    pub fn parse(bytes: &'a [u8]) -> Result<PackView<'a>, FormatError> {
        if bytes.len() < CLIPS_OFFSET {
            return Err(FormatError::TooShort);
        }
        if bytes[..4] != MAGIC {
            return Err(FormatError::BadMagic);
        }
        let version = read_u16(bytes, 4);
        if version != VERSION {
            return Err(FormatError::UnsupportedVersion(version));
        }
        let flags = read_u16(bytes, 6);
        let sample_rate = read_u32(bytes, 8);
        if sample_rate == 0 {
            return Err(FormatError::BadSampleRate);
        }
        let clip_count = read_u32(bytes, 12) as usize;
        let pcm_offset = read_u32(bytes, 16) as usize;
        let pcm_samples = read_u32(bytes, 20) as usize;
        let meta_offset = read_u32(bytes, 24) as usize;
        let meta_len = read_u32(bytes, 28) as usize;

        if clip_count > usize::from(u16::MAX) {
            return Err(FormatError::TooManyClips);
        }
        let clips_end = CLIPS_OFFSET + clip_count * CLIP_ENTRY_LEN;
        if clips_end > bytes.len() {
            return Err(FormatError::OutOfBounds("clip table"));
        }
        let meta_end = meta_offset
            .checked_add(meta_len)
            .ok_or(FormatError::OutOfBounds("metadata"))?;
        if meta_offset < clips_end || meta_end > bytes.len() {
            return Err(FormatError::OutOfBounds("metadata"));
        }
        let pcm_end = pcm_samples
            .checked_mul(2)
            .and_then(|n| n.checked_add(pcm_offset))
            .ok_or(FormatError::OutOfBounds("pcm"))?;
        if pcm_offset < meta_end || pcm_end > bytes.len() {
            return Err(FormatError::OutOfBounds("pcm"));
        }
        let pcm_bytes = &bytes[pcm_offset..pcm_end];
        if pcm_bytes.as_ptr().align_offset(std::mem::align_of::<i16>()) != 0 {
            return Err(FormatError::Misaligned);
        }
        // SAFETY: `pcm_bytes` is in bounds and aligned for `i16` (checked above), it holds
        // exactly `pcm_samples * 2` bytes, every bit pattern is a valid `i16`, the host is
        // little-endian like the file, and the borrow keeps the data alive for `'a`.
        let pcm =
            unsafe { std::slice::from_raw_parts(pcm_bytes.as_ptr().cast::<i16>(), pcm_samples) };
        let meta = Meta::decode(&bytes[meta_offset..meta_end])?;

        let view = PackView {
            sample_rate,
            flags,
            meta,
            maps: &bytes[PRESS_OFFSET..CLIPS_OFFSET],
            clips: &bytes[CLIPS_OFFSET..clips_end],
            pcm,
        };
        for slot in 0..2 * KEY_SLOTS {
            let r = view.map_entry(slot);
            if !r.is_none() && usize::from(r.first) + usize::from(r.count) > clip_count {
                return Err(FormatError::BadClipRef { key: slot % KEY_SLOTS });
            }
        }
        for index in 0..clip_count {
            let clip = view.clip_entry(index);
            let (start, len) = (clip.start as usize, clip.len as usize);
            if len == 0 || start < GUARD_BEFORE || start + len + GUARD_AFTER > pcm_samples {
                return Err(FormatError::BadClip { index });
            }
        }
        Ok(view)
    }

    pub fn has_release(&self) -> bool {
        self.flags & FLAG_HAS_RELEASE != 0
    }

    /// Candidate press clips for a macOS keycode.
    pub fn press(&self, keycode: u8) -> ClipRef {
        if usize::from(keycode) < KEY_SLOTS {
            self.map_entry(usize::from(keycode))
        } else {
            ClipRef::NONE
        }
    }

    /// Candidate release clips for a macOS keycode.
    pub fn release(&self, keycode: u8) -> ClipRef {
        if usize::from(keycode) < KEY_SLOTS {
            self.map_entry(KEY_SLOTS + usize::from(keycode))
        } else {
            ClipRef::NONE
        }
    }

    pub fn clip_count(&self) -> usize {
        self.clips.len() / CLIP_ENTRY_LEN
    }

    /// Clip audio without guards. Panics if `index >= clip_count()`.
    pub fn clip(&self, index: usize) -> &'a [i16] {
        let c = self.clip_entry(index);
        let start = c.start as usize;
        &self.pcm[start..start + c.len as usize]
    }

    /// Clip audio with its guards: `GUARD_BEFORE` zeros, the audio, `GUARD_AFTER` zeros.
    pub fn clip_with_guards(&self, index: usize) -> &'a [i16] {
        let c = self.clip_entry(index);
        let start = c.start as usize;
        &self.pcm[start - GUARD_BEFORE..start + c.len as usize + GUARD_AFTER]
    }

    /// The whole PCM block.
    pub fn pcm(&self) -> &'a [i16] {
        self.pcm
    }

    fn map_entry(&self, slot: usize) -> ClipRef {
        ClipRef { first: read_u16(self.maps, slot * 4), count: read_u16(self.maps, slot * 4 + 2) }
    }

    fn clip_entry(&self, index: usize) -> Clip {
        let at = index * CLIP_ENTRY_LEN;
        Clip { start: read_u32(self.clips, at), len: read_u32(self.clips, at + 4) }
    }
}

fn read_u16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn read_u32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p rvpack && cargo clippy -p rvpack --all-targets -- -D warnings && cargo fmt --check`
Expected: 12 tests pass, no warnings, no formatting diff (run `cargo fmt` first if needed).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml .cargo/config.toml .gitignore crates/rvpack
git commit -m "feat(rvpack): zero-copy soundpack format with validation

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Keyboard geometry and Mechvibes key mapping

**Files:**
- Create: `crates/rvpack/src/keys.rs`, `crates/rvpack/src/mechvibes.rs`
- Modify: `crates/rvpack/src/lib.rs`
- Test: unit tests inside both new files

**Interfaces:**
- Consumes: nothing.
- Produces (used by Tasks 4, 6, 7):
  - `rvpack::keys::KeyInfo { code: u8, name: &'static str, row: u8, x: f32 }`
  - `rvpack::keys::KEYS: &[KeyInfo]` (120 entries), `rvpack::keys::info(u8) -> Option<&'static KeyInfo>`
  - `rvpack::keys::code::{A, S, E, R, T, U, J, O, RETURN, TAB, SPACE, DELETE, ESCAPE, COMMAND, RIGHT_COMMAND, SHIFT, RIGHT_SHIFT, CAPS_LOCK, OPTION, RIGHT_OPTION, CONTROL, RIGHT_CONTROL, FUNCTION, KEYPAD_ENTER}: u8`
  - `rvpack::mechvibes::candidates(u8) -> &'static [u16]`, `rvpack::mechvibes::CODE_A: u16 = 30`

- [ ] **Step 1: Write the failing tests**

**File:** `crates/rvpack/src/lib.rs`
```rust
//! The `.rvpack` soundpack format shared by the Rustyvibes app (reader) and its
//! build tooling (writer), plus macOS keyboard geometry.

pub mod format;
pub mod keys;
pub mod mechvibes;

pub use format::{
    Clip, ClipRef, FormatError, GUARD_AFTER, GUARD_BEFORE, KEY_SLOTS, Meta, PackData, PackView,
};
```

**File:** `crates/rvpack/src/keys.rs` (test module; implementation added in Step 3 above it)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_every_physical_key_once() {
        assert_eq!(KEYS.len(), 120);
        let mut seen = [false; 128];
        for key in KEYS {
            assert!(key.code < 128, "{} out of range", key.name);
            assert!(!seen[usize::from(key.code)], "duplicate code {:#04x}", key.code);
            seen[usize::from(key.code)] = true;
            assert!(key.row <= 5, "{} has row {}", key.name, key.row);
            assert!((0.0..=23.0).contains(&key.x), "{} has x {}", key.name, key.x);
        }
    }

    #[test]
    fn rows_match_the_physical_keyboard() {
        assert_eq!(info(code::ESCAPE).unwrap().row, 0);
        assert_eq!(info(code::DELETE).unwrap().row, 1);
        assert_eq!(info(code::TAB).unwrap().row, 2);
        assert_eq!(info(code::A).unwrap().row, 3);
        assert_eq!(info(code::RETURN).unwrap().row, 3);
        assert_eq!(info(code::SHIFT).unwrap().row, 4);
        assert_eq!(info(code::SPACE).unwrap().row, 5);
        assert!(info(0x34).is_none());
    }

    #[test]
    fn letters_run_left_to_right() {
        // Q W E R T Y U I O P
        let qwerty = [0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x10, 0x20, 0x22, 0x1F, 0x23];
        let xs: Vec<f32> = qwerty.iter().map(|&c| info(c).unwrap().x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
        assert!(info(code::A).unwrap().x < info(code::J).unwrap().x);
    }
}
```

**File:** `crates/rvpack/src/mechvibes.rs` (test module; implementation added in Step 3 above it)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{KEYS, code};

    #[test]
    fn every_known_key_has_candidates() {
        for key in KEYS {
            assert!(!candidates(key.code).is_empty(), "{} has no Mechvibes candidates", key.name);
        }
        assert!(candidates(0x34).is_empty());
    }

    #[test]
    fn primary_codes_match_libuiohook() {
        assert_eq!(candidates(code::A)[0], 30);
        assert_eq!(candidates(code::SPACE)[0], 57);
        assert_eq!(candidates(code::RETURN)[0], 28);
        assert_eq!(candidates(code::ESCAPE)[0], 1);
        assert_eq!(candidates(code::DELETE)[0], 14);
        assert_eq!(candidates(0x7E)[0], 57416); // up arrow
        assert_eq!(candidates(code::RIGHT_COMMAND)[0], 3676);
    }

    #[test]
    fn right_modifiers_fall_back_to_left() {
        assert_eq!(candidates(code::RIGHT_COMMAND), &[3676, 3675]);
        assert_eq!(candidates(code::RIGHT_SHIFT), &[54, 42]);
        assert_eq!(candidates(code::RIGHT_OPTION), &[3640, 56]);
        assert_eq!(candidates(code::RIGHT_CONTROL), &[3613, 29]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p rvpack`
Expected: compile errors — `KEYS`, `info`, `code`, `candidates` not found.

- [ ] **Step 3: Implement both modules** (place above each test module)

**File:** `crates/rvpack/src/keys.rs` (implementation part)
```rust
//! Physical key geometry for macOS virtual keycodes (`kVK_*`). Keycodes name
//! key *positions*, independent of the keyboard layout. Positions are in key
//! units on a full-size ANSI keyboard; `row` 0 is the function row and 5 the
//! space-bar row.

/// One physical key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyInfo {
    pub code: u8,
    pub name: &'static str,
    pub row: u8,
    /// Horizontal centre of the key, in key units from the left edge.
    pub x: f32,
}

/// macOS virtual keycodes referred to by name.
pub mod code {
    pub const A: u8 = 0x00;
    pub const S: u8 = 0x01;
    pub const E: u8 = 0x0E;
    pub const R: u8 = 0x0F;
    pub const T: u8 = 0x11;
    pub const O: u8 = 0x1F;
    pub const U: u8 = 0x20;
    pub const J: u8 = 0x26;
    pub const RETURN: u8 = 0x24;
    pub const TAB: u8 = 0x30;
    pub const SPACE: u8 = 0x31;
    pub const DELETE: u8 = 0x33;
    pub const ESCAPE: u8 = 0x35;
    pub const RIGHT_COMMAND: u8 = 0x36;
    pub const COMMAND: u8 = 0x37;
    pub const SHIFT: u8 = 0x38;
    pub const CAPS_LOCK: u8 = 0x39;
    pub const OPTION: u8 = 0x3A;
    pub const CONTROL: u8 = 0x3B;
    pub const RIGHT_SHIFT: u8 = 0x3C;
    pub const RIGHT_OPTION: u8 = 0x3D;
    pub const RIGHT_CONTROL: u8 = 0x3E;
    pub const FUNCTION: u8 = 0x3F;
    pub const KEYPAD_ENTER: u8 = 0x4C;
}

const fn k(code: u8, name: &'static str, row: u8, x: f32) -> KeyInfo {
    KeyInfo { code, name, row, x }
}

/// Every physical key Rustyvibes knows about.
pub const KEYS: &[KeyInfo] = &[
    // Row 0: function row.
    k(0x35, "Escape", 0, 0.5),
    k(0x7A, "F1", 0, 2.5),
    k(0x78, "F2", 0, 3.5),
    k(0x63, "F3", 0, 4.5),
    k(0x76, "F4", 0, 5.5),
    k(0x60, "F5", 0, 7.0),
    k(0x61, "F6", 0, 8.0),
    k(0x62, "F7", 0, 9.0),
    k(0x64, "F8", 0, 10.0),
    k(0x65, "F9", 0, 11.5),
    k(0x6D, "F10", 0, 12.5),
    k(0x67, "F11", 0, 13.5),
    k(0x6F, "F12", 0, 14.5),
    k(0x69, "F13", 0, 15.75),
    k(0x6B, "F14", 0, 16.75),
    k(0x71, "F15", 0, 17.75),
    k(0x6A, "F16", 0, 19.0),
    k(0x40, "F17", 0, 20.0),
    k(0x4F, "F18", 0, 21.0),
    k(0x50, "F19", 0, 22.0),
    k(0x5A, "F20", 0, 22.5),
    k(0x4A, "Mute", 0, 12.5),
    k(0x49, "Volume Down", 0, 13.5),
    k(0x48, "Volume Up", 0, 14.5),
    // Row 1: number row.
    k(0x32, "`", 1, 0.5),
    k(0x0A, "§", 1, 0.5),
    k(0x12, "1", 1, 1.5),
    k(0x13, "2", 1, 2.5),
    k(0x14, "3", 1, 3.5),
    k(0x15, "4", 1, 4.5),
    k(0x17, "5", 1, 5.5),
    k(0x16, "6", 1, 6.5),
    k(0x1A, "7", 1, 7.5),
    k(0x1C, "8", 1, 8.5),
    k(0x19, "9", 1, 9.5),
    k(0x1D, "0", 1, 10.5),
    k(0x1B, "-", 1, 11.5),
    k(0x18, "=", 1, 12.5),
    k(0x5D, "¥", 1, 13.5),
    k(0x33, "Delete", 1, 14.0),
    k(0x72, "Help", 1, 15.75),
    k(0x73, "Home", 1, 16.75),
    k(0x74, "Page Up", 1, 17.75),
    k(0x47, "Keypad Clear", 1, 19.0),
    k(0x51, "Keypad =", 1, 20.0),
    k(0x4B, "Keypad /", 1, 21.0),
    k(0x43, "Keypad *", 1, 22.0),
    // Row 2: top letter row.
    k(0x30, "Tab", 2, 0.75),
    k(0x0C, "Q", 2, 2.0),
    k(0x0D, "W", 2, 3.0),
    k(0x0E, "E", 2, 4.0),
    k(0x0F, "R", 2, 5.0),
    k(0x11, "T", 2, 6.0),
    k(0x10, "Y", 2, 7.0),
    k(0x20, "U", 2, 8.0),
    k(0x22, "I", 2, 9.0),
    k(0x1F, "O", 2, 10.0),
    k(0x23, "P", 2, 11.0),
    k(0x21, "[", 2, 12.0),
    k(0x1E, "]", 2, 13.0),
    k(0x2A, "\\", 2, 14.25),
    k(0x75, "Forward Delete", 2, 15.75),
    k(0x77, "End", 2, 16.75),
    k(0x79, "Page Down", 2, 17.75),
    k(0x59, "Keypad 7", 2, 19.0),
    k(0x5B, "Keypad 8", 2, 20.0),
    k(0x5C, "Keypad 9", 2, 21.0),
    k(0x4E, "Keypad -", 2, 22.0),
    // Row 3: home row.
    k(0x39, "Caps Lock", 3, 0.875),
    k(0x00, "A", 3, 2.25),
    k(0x01, "S", 3, 3.25),
    k(0x02, "D", 3, 4.25),
    k(0x03, "F", 3, 5.25),
    k(0x05, "G", 3, 6.25),
    k(0x04, "H", 3, 7.25),
    k(0x26, "J", 3, 8.25),
    k(0x28, "K", 3, 9.25),
    k(0x25, "L", 3, 10.25),
    k(0x29, ";", 3, 11.25),
    k(0x27, "'", 3, 12.25),
    k(0x24, "Return", 3, 13.875),
    k(0x56, "Keypad 4", 3, 19.0),
    k(0x57, "Keypad 5", 3, 20.0),
    k(0x58, "Keypad 6", 3, 21.0),
    k(0x45, "Keypad +", 3, 22.0),
    // Row 4: bottom letter row.
    k(0x38, "Shift", 4, 1.125),
    k(0x06, "Z", 4, 2.75),
    k(0x07, "X", 4, 3.75),
    k(0x08, "C", 4, 4.75),
    k(0x09, "V", 4, 5.75),
    k(0x0B, "B", 4, 6.75),
    k(0x2D, "N", 4, 7.75),
    k(0x2E, "M", 4, 8.75),
    k(0x2B, ",", 4, 9.75),
    k(0x2F, ".", 4, 10.75),
    k(0x2C, "/", 4, 11.75),
    k(0x5E, "_", 4, 12.75),
    k(0x3C, "Right Shift", 4, 13.625),
    k(0x7E, "Up Arrow", 4, 16.75),
    k(0x53, "Keypad 1", 4, 19.0),
    k(0x54, "Keypad 2", 4, 20.0),
    k(0x55, "Keypad 3", 4, 21.0),
    k(0x4C, "Keypad Enter", 4, 22.0),
    // Row 5: space-bar row (MacBook / Magic Keyboard positions).
    k(0x3F, "Fn", 5, 0.5),
    k(0x3B, "Control", 5, 1.5),
    k(0x3A, "Option", 5, 2.5),
    k(0x37, "Command", 5, 3.75),
    k(0x66, "Eisu", 5, 4.75),
    k(0x31, "Space", 5, 7.0),
    k(0x68, "Kana", 5, 9.25),
    k(0x36, "Right Command", 5, 10.25),
    k(0x3D, "Right Option", 5, 11.25),
    k(0x3E, "Right Control", 5, 12.5),
    k(0x6E, "Menu", 5, 13.5),
    k(0x7B, "Left Arrow", 5, 15.75),
    k(0x7D, "Down Arrow", 5, 16.75),
    k(0x7C, "Right Arrow", 5, 17.75),
    k(0x52, "Keypad 0", 5, 19.5),
    k(0x41, "Keypad .", 5, 21.0),
    k(0x5F, "Keypad ,", 5, 22.0),
];

/// Geometry for a keycode, if it is a known physical key.
pub fn info(code: u8) -> Option<&'static KeyInfo> {
    KEYS.iter().find(|key| key.code == code)
}
```

**File:** `crates/rvpack/src/mechvibes.rs` (implementation part)
```rust
//! Mechvibes soundpack configs name keys by libuiohook scan code. This maps
//! macOS virtual keycodes to candidate codes, most specific first, so a pack
//! that lacks a key can fall back to a similar one.

/// Mechvibes code for the letter A, the last-resort fallback.
pub const CODE_A: u16 = 30;

/// Candidate Mechvibes codes for a macOS keycode; empty for unknown keys.
pub fn candidates(mac: u8) -> &'static [u16] {
    match mac {
        0x00 => &[30],          // A
        0x01 => &[31],          // S
        0x02 => &[32],          // D
        0x03 => &[33],          // F
        0x04 => &[35],          // H
        0x05 => &[34],          // G
        0x06 => &[44],          // Z
        0x07 => &[45],          // X
        0x08 => &[46],          // C
        0x09 => &[47],          // V
        0x0A => &[41, 2],       // §
        0x0B => &[48],          // B
        0x0C => &[16],          // Q
        0x0D => &[17],          // W
        0x0E => &[18],          // E
        0x0F => &[19],          // R
        0x10 => &[21],          // Y
        0x11 => &[20],          // T
        0x12 => &[2],           // 1
        0x13 => &[3],           // 2
        0x14 => &[4],           // 3
        0x15 => &[5],           // 4
        0x16 => &[7],           // 6
        0x17 => &[6],           // 5
        0x18 => &[13, 12],      // =
        0x19 => &[10],          // 9
        0x1A => &[8],           // 7
        0x1B => &[12, 13],      // -
        0x1C => &[9],           // 8
        0x1D => &[11],          // 0
        0x1E => &[27, 26],      // ]
        0x1F => &[24],          // O
        0x20 => &[22],          // U
        0x21 => &[26, 27],      // [
        0x22 => &[23],          // I
        0x23 => &[25],          // P
        0x24 => &[28],          // Return
        0x25 => &[38],          // L
        0x26 => &[36],          // J
        0x27 => &[40, 39],      // '
        0x28 => &[37],          // K
        0x29 => &[39, 40],      // ;
        0x2A => &[43, 28],      // \
        0x2B => &[51, 52],      // ,
        0x2C => &[53, 52],      // /
        0x2D => &[49],          // N
        0x2E => &[50],          // M
        0x2F => &[52, 51],      // .
        0x30 => &[15],          // Tab
        0x31 => &[57],          // Space
        0x32 => &[41, 2],       // `
        0x33 => &[14],          // Delete (backspace)
        0x35 => &[1],           // Escape
        0x36 => &[3676, 3675],  // Right Command
        0x37 => &[3675, 3676],  // Command
        0x38 => &[42, 54],      // Shift
        0x39 => &[58, 15],      // Caps Lock
        0x3A => &[56, 3640],    // Option
        0x3B => &[29, 3613],    // Control
        0x3C => &[54, 42],      // Right Shift
        0x3D => &[3640, 56],    // Right Option
        0x3E => &[3613, 29],    // Right Control
        0x3F => &[3666, 29],    // Fn (sits where Insert is on PC keyboards)
        0x40 => &[101, 88],     // F17
        0x41 => &[83, 52],      // Keypad .
        0x43 => &[55, 9],       // Keypad *
        0x45 => &[78, 13],      // Keypad +
        0x47 => &[69, 1],       // Keypad Clear
        0x48 => &[57392, 88],   // Volume Up
        0x49 => &[57390, 87],   // Volume Down
        0x4A => &[57376, 68],   // Mute
        0x4B => &[3637, 53],    // Keypad /
        0x4C => &[3612, 28],    // Keypad Enter
        0x4E => &[74, 12],      // Keypad -
        0x4F => &[102, 88],     // F18
        0x50 => &[103, 88],     // F19
        0x51 => &[3597, 13],    // Keypad =
        0x52 => &[82, 11],      // Keypad 0
        0x53 => &[79, 2],       // Keypad 1
        0x54 => &[80, 3],       // Keypad 2
        0x55 => &[81, 4],       // Keypad 3
        0x56 => &[75, 5],       // Keypad 4
        0x57 => &[76, 6],       // Keypad 5
        0x58 => &[77, 7],       // Keypad 6
        0x59 => &[71, 8],       // Keypad 7
        0x5A => &[104, 88],     // F20
        0x5B => &[72, 9],       // Keypad 8
        0x5C => &[73, 10],      // Keypad 9
        0x5D => &[125, 43],     // JIS Yen
        0x5E => &[115, 53],     // JIS Underscore
        0x5F => &[126, 83, 51], // JIS Keypad ,
        0x60 => &[63],          // F5
        0x61 => &[64],          // F6
        0x62 => &[65],          // F7
        0x63 => &[61],          // F3
        0x64 => &[66],          // F8
        0x65 => &[67],          // F9
        0x66 => &[3675],        // JIS Eisu (left of space)
        0x67 => &[87, 88],      // F11
        0x68 => &[3676],        // JIS Kana (right of space)
        0x69 => &[91, 88],      // F13
        0x6A => &[99, 88],      // F16
        0x6B => &[92, 88],      // F14
        0x6D => &[68],          // F10
        0x6E => &[3677, 3676],  // Menu
        0x6F => &[88, 87],      // F12
        0x71 => &[93, 88],      // F15
        0x72 => &[3666, 3655],  // Help / Insert
        0x73 => &[3655, 3657],  // Home
        0x74 => &[3657, 3655],  // Page Up
        0x75 => &[3667, 14],    // Forward Delete
        0x76 => &[62],          // F4
        0x77 => &[3663, 3667],  // End
        0x78 => &[60],          // F2
        0x79 => &[3665, 3663],  // Page Down
        0x7A => &[59],          // F1
        0x7B => &[57419, 75],   // Left Arrow
        0x7C => &[57421, 77],   // Right Arrow
        0x7D => &[57424, 80],   // Down Arrow
        0x7E => &[57416, 72],   // Up Arrow
        _ => &[],
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p rvpack && cargo clippy -p rvpack --all-targets -- -D warnings && cargo fmt --check`
Expected: 18 tests pass, clean.

- [ ] **Step 5: Commit**

```bash
git add crates/rvpack
git commit -m "feat(rvpack): macOS key geometry and Mechvibes key mapping

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 3: Build tool — decoding and clip clean-up DSP

**Files:**
- Create: `crates/xtask/Cargo.toml`, `crates/xtask/src/main.rs`, `crates/xtask/src/decode.rs`, `crates/xtask/src/dsp.rs`, `crates/xtask/src/testutil.rs`
- Test: unit tests inside `decode.rs` and `dsp.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces (used by Task 4):
  - `crate::Result<T> = Result<T, String>`
  - `decode::Audio { rate: u32, samples: Vec<f32> }` (mono), `decode::decode_file(&Path) -> Result<Audio>`
  - `dsp::{TARGET_DB = -22.0, CEILING_DB = -1.0, ATTACK_MS = 60.0}`, `dsp::db_to_lin(f32)`, `dsp::lin_to_db(f32)`, `dsp::peak(&[f32])`, `dsp::dc_block(&mut [f32], u32)`, `dsp::TrimParams` (+ `Default`), `dsp::trim_clip(&[f32], u32, &TrimParams) -> Option<Vec<f32>>`, `dsp::attack_rms_db(&[f32], u32, f32) -> f32`, `dsp::median(Vec<f32>) -> f32`, `dsp::normalize(&mut [Vec<f32>], &[bool], u32, f32, f32) -> f32`, `dsp::to_i16(&[f32], &mut u32) -> Vec<i16>`
  - `testutil::{write_wav(&Path, u32, u16, &[i16]), temp_dir(&str) -> PathBuf, click_i16(usize, usize, f32) -> Vec<i16>}` (test-only)

- [ ] **Step 1: Create the crate skeleton and test helpers**

**File:** `crates/xtask/Cargo.toml`
```toml
[package]
name = "xtask"
description = "Build tooling for Rustyvibes"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
publish = false

[dependencies]
rvpack = { path = "../rvpack" }
serde_json = "1"
symphonia = { version = "0.6.1", default-features = false, features = ["ogg", "vorbis", "mp3", "wav", "pcm"] }

[lints]
workspace = true
```

**File:** `crates/xtask/src/main.rs`
```rust
//! Build tooling for Rustyvibes: `cargo xtask <command>`.

mod decode;
mod dsp;
#[cfg(test)]
mod testutil;

use std::path::Path;
use std::process::ExitCode;

/// Errors are human-readable strings; this is a build tool.
pub type Result<T, E = String> = std::result::Result<T, E>;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  probe <file>...     show how the clean-up pipeline sees audio files";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("probe") => probe(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Prints how the clean-up pipeline sees each file.
fn probe(files: &[String]) -> Result<()> {
    for file in files {
        let mut audio = decode::decode_file(Path::new(file))?;
        dsp::dc_block(&mut audio.samples, audio.rate);
        let ms = |n: usize| n as f32 * 1000.0 / audio.rate as f32;
        let Some(trimmed) = dsp::trim_clip(&audio.samples, audio.rate, &dsp::TrimParams::default())
        else {
            println!("{file}: silent");
            continue;
        };
        let mut clips = vec![trimmed];
        let gain = dsp::normalize(&mut clips, &[true], audio.rate, dsp::TARGET_DB, dsp::CEILING_DB);
        let quantised = dsp::to_i16(&clips[0], &mut 1);
        let peak = quantised.iter().map(|s| i32::from(*s).abs()).max().unwrap_or(0);
        println!(
            "{file}: {} Hz, {:.1} ms → {:.1} ms trimmed, gain {gain:+.1} dB, i16 peak {peak}",
            audio.rate,
            ms(audio.samples.len()),
            ms(clips[0].len()),
        );
    }
    Ok(())
}
```

**File:** `crates/xtask/src/testutil.rs`
```rust
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
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/xtask/src/decode.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{temp_dir, write_wav};

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
    fn missing_file_names_the_path() {
        let err = decode_file(Path::new("/nonexistent/x.wav")).unwrap_err();
        assert!(err.contains("/nonexistent/x.wav"), "{err}");
    }
}
```

**File:** `crates/xtask/src/dsp.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 44_100;

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
        normalize(&mut clips, &[true, true, true], RATE, TARGET_DB, CEILING_DB);
        let levels: Vec<f32> = clips.iter().map(|c| attack_rms_db(c, RATE, ATTACK_MS)).collect();
        assert!((median(levels) - TARGET_DB).abs() < 0.05);
    }

    #[test]
    fn normalize_limits_hot_clips_individually() {
        let quiet = click(0.0, 80.0, 0.0, 0.3);
        let mut spike = vec![0.0; samples(80.0)];
        spike[10] = 1.0;
        let mut clips = vec![quiet.clone(), quiet.clone(), spike];
        let gain_db = normalize(&mut clips, &[true, true, true], RATE, TARGET_DB, CEILING_DB);
        let gain = db_to_lin(gain_db);
        assert!(peak(&clips[2]) <= db_to_lin(CEILING_DB) + 1e-6);
        for (a, b) in quiet.iter().zip(&clips[0]) {
            assert!((a * gain - b).abs() < 1e-6, "quiet clips get exactly the pack gain");
        }
    }

    #[test]
    fn release_clips_follow_the_press_gain() {
        let press = click(0.0, 80.0, 0.0, 0.4);
        let release = click(0.0, 40.0, 0.0, 0.05);
        let mut clips = vec![press.clone(), release.clone()];
        let gain = db_to_lin(normalize(&mut clips, &[true, false], RATE, TARGET_DB, CEILING_DB));
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p xtask`
Expected: compile errors — `decode_file`, `trim_clip`, `normalize`, … not found.

- [ ] **Step 4: Implement decode and DSP** (place above each test module)

**File:** `crates/xtask/src/decode.rs` (implementation part)
```rust
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
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
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
            interleaved.chunks_exact(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32),
        );
    }
    if rate == 0 || mono.is_empty() {
        return Err(fail(&"decoded no audio"));
    }
    Ok(Audio { rate, samples: mono })
}
```

**File:** `crates/xtask/src/dsp.rs` (implementation part)
```rust
//! Clip clean-up applied at build time: DC removal, onset/tail trimming with
//! short fades, loudness normalisation and dithered 16-bit quantisation.

use std::f32::consts::PI;

/// Loudness target: median attack-window RMS of a pack's press clips.
pub const TARGET_DB: f32 = -22.0;
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p xtask && cargo clippy -p xtask --all-targets -- -D warnings && cargo fmt --check`
Expected: 12 tests pass, clean. Also run `cargo xtask probe /System/Library/Sounds/Tink.aiff` and expect an error naming the path (AIFF is not enabled), proving errors surface readably.

- [ ] **Step 6: Commit**

```bash
git add crates/xtask
git commit -m "feat(xtask): audio decoding and clip clean-up DSP

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Soundpack assets, catalog, source adapters and the pack builder

**Files:**
- Create: `assets/soundpacks/catalog.json`, `assets/soundpacks/SOURCES.md`, `assets/soundpacks/mechvibes/**`, `assets/soundpacks/kbsim/**` (copied originals), `crates/xtask/src/catalog.rs`, `crates/xtask/src/sources/mod.rs`, `crates/xtask/src/sources/mechvibes.rs`, `crates/xtask/src/sources/kbsim.rs`, `crates/xtask/src/packs.rs`
- Modify: `crates/xtask/src/main.rs`
- Test: unit tests in `catalog.rs`, `sources/*.rs`, `packs.rs`; `cargo xtask packs` end to end

**Interfaces:**
- Consumes: Task 1–2 `rvpack::{PackData, Meta, ClipRef, KEY_SLOTS, keys, mechvibes}`; Task 3 `decode`, `dsp`, `testutil`.
- Produces:
  - `target/packs/<id>.rvpack` for all 21 catalog packs (used by Tasks 6–12)
  - `catalog::{load, parse, Catalog { default, entries }, Entry { id, name, variant, category, color, credit, kind, dir, order }, SourceKind::{Mechvibes, Kbsim}, CATEGORIES}`
  - `sources::RawPack { rate, clips, press: [Option<usize>; 128], release: [Option<usize>; 128] }` with `new(u32)`, `push(u32, Vec<f32>, &str) -> Result<usize>`
  - `packs::{run(&Path), build_all(&Path, &Path) -> Result<Vec<Report>>, finish(&Entry, bool, RawPack) -> Result<(PackData, f32)>, check(&PackData, f32) -> Result<Report>}`
  - `crate::workspace_root() -> PathBuf`

- [ ] **Step 1: Import the original recordings at pinned commits**

```bash
SRC=$(mktemp -d)
git clone --quiet --filter=blob:none --no-checkout https://github.com/hainguyents13/mechvibes.git "$SRC/mechvibes"
git -C "$SRC/mechvibes" checkout 326252a13e7bef4f1c35d08ef0189b5af6f8ba02 -- LICENSE src/audio
git clone --quiet --filter=blob:none --no-checkout https://github.com/tplai/kbsim.git "$SRC/kbsim"
git -C "$SRC/kbsim" checkout ba103f3b0afa9dab80447aa2e7e2ed80b6bd80e4 -- LICENSE.md src/assets/audio
mkdir -p assets/soundpacks/mechvibes assets/soundpacks/kbsim
cp "$SRC/mechvibes/LICENSE" assets/soundpacks/mechvibes/LICENSE
for p in cherrymx-black-abs cherrymx-black-pbt cherrymx-red-abs cherrymx-red-pbt \
         cherrymx-brown-abs cherrymx-brown-pbt cherrymx-blue-abs cherrymx-blue-pbt \
         topre-purple-hybrid-pbt eg-crystal-purple eg-oreo; do
  cp -R "$SRC/mechvibes/src/audio/$p" assets/soundpacks/mechvibes/
done
cp "$SRC/kbsim/LICENSE.md" assets/soundpacks/kbsim/LICENSE.md
for s in blackink redink cream alpaca turquoise holypanda topre boxnavy bluealps buckling; do
  cp -R "$SRC/kbsim/src/assets/audio/$s" assets/soundpacks/kbsim/
done
find assets/soundpacks -name .DS_Store -delete
ls assets/soundpacks/mechvibes | wc -l   # expect 12 (11 packs + LICENSE)
ls assets/soundpacks/kbsim | wc -l       # expect 11 (10 packs + LICENSE.md)
```

**File:** `assets/soundpacks/catalog.json`
```json
{
  "default": "cherry-mx-brown-pbt",
  "packs": [
    { "id": "cherry-mx-black-abs", "name": "Cherry MX Black", "variant": "ABS", "category": "linear", "color": "#2C2C30", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-black-abs" } },
    { "id": "cherry-mx-black-pbt", "name": "Cherry MX Black", "variant": "PBT", "category": "linear", "color": "#2C2C30", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-black-pbt" } },
    { "id": "cherry-mx-red-abs", "name": "Cherry MX Red", "variant": "ABS", "category": "linear", "color": "#D7263D", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-red-abs" } },
    { "id": "cherry-mx-red-pbt", "name": "Cherry MX Red", "variant": "PBT", "category": "linear", "color": "#D7263D", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-red-pbt" } },
    { "id": "gateron-black-ink", "name": "Gateron Black Ink", "category": "linear", "color": "#3A3F4A", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/blackink" } },
    { "id": "gateron-red-ink", "name": "Gateron Red Ink", "category": "linear", "color": "#B3203B", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/redink" } },
    { "id": "novelkeys-cream", "name": "NovelKeys Cream", "category": "linear", "color": "#EBD9B4", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/cream" } },
    { "id": "alpaca", "name": "Alpaca", "category": "linear", "color": "#F2A7C3", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/alpaca" } },
    { "id": "turquoise-tealios", "name": "Turquoise Tealios", "category": "linear", "color": "#2EC4B6", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/turquoise" } },
    { "id": "cherry-mx-brown-abs", "name": "Cherry MX Brown", "variant": "ABS", "category": "tactile", "color": "#8B5A2B", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-brown-abs" } },
    { "id": "cherry-mx-brown-pbt", "name": "Cherry MX Brown", "variant": "PBT", "category": "tactile", "color": "#8B5A2B", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-brown-pbt" } },
    { "id": "holy-panda", "name": "Holy Panda", "category": "tactile", "color": "#E9A23B", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/holypanda" } },
    { "id": "topre", "name": "Topre", "category": "tactile", "color": "#8E8E93", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/topre" } },
    { "id": "topre-purple-hybrid-pbt", "name": "Topre Purple Hybrid", "variant": "PBT", "category": "tactile", "color": "#7E57C2", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/topre-purple-hybrid-pbt" } },
    { "id": "everglide-crystal-purple", "name": "Everglide Crystal Purple", "category": "tactile", "color": "#A77BF3", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/eg-crystal-purple" } },
    { "id": "everglide-oreo", "name": "Everglide Oreo", "category": "tactile", "color": "#3B3B3B", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/eg-oreo" } },
    { "id": "cherry-mx-blue-abs", "name": "Cherry MX Blue", "variant": "ABS", "category": "clicky", "color": "#1F6FEB", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-blue-abs" } },
    { "id": "cherry-mx-blue-pbt", "name": "Cherry MX Blue", "variant": "PBT", "category": "clicky", "color": "#1F6FEB", "credit": "Mechvibes by Hai Nguyen · MIT License", "source": { "kind": "mechvibes", "dir": "mechvibes/cherrymx-blue-pbt" } },
    { "id": "kailh-box-navy", "name": "Kailh Box Navy", "category": "clicky", "color": "#1B2F5E", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/boxnavy" } },
    { "id": "skcm-blue-alps", "name": "SKCM Blue Alps", "category": "clicky", "color": "#5AA9E6", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/bluealps" } },
    { "id": "buckling-spring", "name": "Buckling Spring", "category": "clicky", "color": "#CFC6B4", "credit": "kbsim by Thomas Lai · MIT License", "source": { "kind": "kbsim", "dir": "kbsim/buckling" } }
  ]
}
```

**File:** `assets/soundpacks/SOURCES.md`
```markdown
# Soundpack sources

Every bundled soundpack is converted at build time (`cargo xtask packs`) from
the original recordings in this directory. The files here are unmodified copies.

| Directory | Upstream | Commit | Licence |
|---|---|---|---|
| `mechvibes/` | https://github.com/hainguyents13/mechvibes (`src/audio`) | `326252a13e7bef4f1c35d08ef0189b5af6f8ba02` | MIT © 2021 Hai Nguyen (`mechvibes/LICENSE`) |
| `kbsim/` | https://github.com/tplai/kbsim (`src/assets/audio`) | `ba103f3b0afa9dab80447aa2e7e2ed80b6bd80e4` | MIT © Thomas Lai (`kbsim/LICENSE.md`) |

Mechvibes packs are per-key recordings stored as one sprite file sliced by
`[start_ms, duration_ms]` (`key_define_type: "single"`). kbsim packs record a
press and a release for each keyboard row (`GENERIC_R0`–`R4`) plus Space,
Enter and Backspace.

Left out on purpose: kbsim's Cherry MX Black/Blue/Brown (Mechvibes has
keycap-specific recordings of the same switches), Mechvibes' copies of kbsim
packs (`*-travel`, `holy-pandas`, `turquoise`), and Mechvibes' `nk-cream`
(kbsim's NovelKeys Cream is used instead).
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/xtask/src/catalog.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r##"{ "default": "b", "packs": [
      {"id":"a","name":"A","category":"linear","color":"#112233","credit":"X","source":{"kind":"kbsim","dir":"kbsim/a"}},
      {"id":"b","name":"B","variant":"PBT","category":"clicky","color":"#AABBCC","credit":"Y","source":{"kind":"mechvibes","dir":"mechvibes/b"}}
    ] }"##;

    #[test]
    fn parses_entries_in_order() {
        let catalog = parse(SAMPLE, Path::new("/assets")).unwrap();
        assert_eq!(catalog.default, "b");
        assert_eq!(catalog.entries.len(), 2);
        assert_eq!(catalog.entries[0].dir, Path::new("/assets/kbsim/a"));
        assert_eq!(catalog.entries[0].variant, "");
        assert_eq!(catalog.entries[1].variant, "PBT");
        assert_eq!(catalog.entries[1].order, 1);
        assert_eq!(catalog.entries[1].kind, SourceKind::Mechvibes);
    }

    #[test]
    fn rejects_bad_catalogs() {
        let cases = [
            ("\"default\": \"b\"", "\"default\": \"zz\"", "not listed"),
            ("\"category\":\"linear\"", "\"category\":\"mushy\"", "unknown category"),
            ("\"color\":\"#112233\"", "\"color\":\"red\"", "#RRGGBB"),
            ("\"id\":\"b\"", "\"id\":\"a\"", "duplicate"),
            ("\"id\":\"b\"", "\"id\":\"B!\"", "a-z"),
            ("\"kind\":\"kbsim\"", "\"kind\":\"other\"", "unknown source kind"),
        ];
        for (needle, replacement, expected) in cases {
            let text = SAMPLE.replacen(needle, replacement, 1);
            let err = parse(&text, Path::new("/")).unwrap_err();
            assert!(err.contains(expected), "{needle} → {err}");
        }
    }

    #[test]
    fn shipped_catalog_is_valid() {
        let path = crate::workspace_root().join("assets/soundpacks/catalog.json");
        let catalog = load(&path).unwrap();
        assert_eq!(catalog.entries.len(), 21);
        for entry in &catalog.entries {
            assert!(entry.dir.is_dir(), "{} is missing {}", entry.id, entry.dir.display());
        }
    }
}
```

**File:** `crates/xtask/src/sources/mechvibes.rs` (test module; implementation goes above it in Step 4)
```rust
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
```

**File:** `crates/xtask/src/sources/kbsim.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{click_i16, temp_dir, write_wav};

    #[test]
    fn rows_and_special_keys_map_like_kbsim() {
        let dir = temp_dir("kbsim");
        std::fs::create_dir_all(dir.join("press")).unwrap();
        std::fs::create_dir_all(dir.join("release")).unwrap();
        let stems = [
            "press/GENERIC_R0",
            "press/GENERIC_R1",
            "press/GENERIC_R2",
            "press/GENERIC_R3",
            "press/GENERIC_R4",
            "press/SPACE",
            "press/ENTER",
            "release/GENERIC",
            "release/SPACE",
        ];
        for (i, stem) in stems.iter().enumerate() {
            write_wav(&dir.join(format!("{stem}.wav")), 44_100, 1, &click_i16(5, 1_000 + i * 10, 0.5));
        }
        let pack = load(&dir).unwrap();
        let press = |c: u8| pack.press[usize::from(c)].unwrap();
        let release = |c: u8| pack.release[usize::from(c)];
        assert_eq!(press(code::ESCAPE), 0, "function row → R0");
        assert_eq!(press(0x12), 1, "number row → R1");
        assert_eq!(press(0x0C), 2, "Q → R2");
        assert_eq!(press(code::A), 3, "home row → R3");
        assert_eq!(press(0x06), 4, "Z → R4");
        assert_eq!(press(code::COMMAND), 4, "space-bar row → R4");
        assert_eq!(press(code::SPACE), 5);
        assert_eq!(press(code::RETURN), 6);
        assert_eq!(press(code::KEYPAD_ENTER), 6);
        assert_eq!(press(code::DELETE), 1, "no BACKSPACE recording → its row");
        assert_eq!(release(code::A), Some(7));
        assert_eq!(release(code::SPACE), Some(8));
        assert_eq!(release(code::RETURN), Some(7), "no ENTER release → generic");
    }

    #[test]
    fn missing_row_recordings_is_an_error() {
        let dir = temp_dir("kbsim-empty");
        let err = load(&dir).unwrap_err();
        assert!(err.contains("GENERIC_R"), "{err}");
    }
}
```

**File:** `crates/xtask/src/packs.rs` (test module; implementation goes above it in Step 4)
```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p xtask`
Expected: compile errors — modules `catalog`, `sources`, `packs` are not declared / items not found.

- [ ] **Step 4: Implement catalog, sources and packs** (place above each test module)

**File:** `crates/xtask/src/main.rs`
```rust
//! Build tooling for Rustyvibes: `cargo xtask <command>`.

mod catalog;
mod decode;
mod dsp;
mod packs;
mod sources;
#[cfg(test)]
mod testutil;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Errors are human-readable strings; this is a build tool.
pub type Result<T, E = String> = std::result::Result<T, E>;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  packs               convert assets/soundpacks into target/packs/*.rvpack
  probe <file>...     show how the clean-up pipeline sees audio files";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let result = match args.first().map(String::as_str) {
        Some("packs") => packs::run(&root),
        Some("probe") => probe(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The repository root (this crate lives in `crates/xtask`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels below the workspace root")
        .to_path_buf()
}

/// Prints how the clean-up pipeline sees each file.
fn probe(files: &[String]) -> Result<()> {
    for file in files {
        let mut audio = decode::decode_file(Path::new(file))?;
        dsp::dc_block(&mut audio.samples, audio.rate);
        let ms = |n: usize| n as f32 * 1000.0 / audio.rate as f32;
        let Some(trimmed) = dsp::trim_clip(&audio.samples, audio.rate, &dsp::TrimParams::default())
        else {
            println!("{file}: silent");
            continue;
        };
        let mut clips = vec![trimmed];
        let gain = dsp::normalize(&mut clips, &[true], audio.rate, dsp::TARGET_DB, dsp::CEILING_DB);
        let quantised = dsp::to_i16(&clips[0], &mut 1);
        let peak = quantised.iter().map(|s| i32::from(*s).abs()).max().unwrap_or(0);
        println!(
            "{file}: {} Hz, {:.1} ms → {:.1} ms trimmed, gain {gain:+.1} dB, i16 peak {peak}",
            audio.rate,
            ms(audio.samples.len()),
            ms(clips[0].len()),
        );
    }
    Ok(())
}
```

**File:** `crates/xtask/src/catalog.rs` (implementation part)
```rust
//! `assets/soundpacks/catalog.json`: which packs ship, in which order, and
//! where their original recordings live.

use std::path::{Path, PathBuf};

use serde_json::Value;

/// Menu categories, in display order.
pub const CATEGORIES: [&str; 3] = ["linear", "tactile", "clicky"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Mechvibes,
    Kbsim,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub variant: String,
    pub category: String,
    pub color: String,
    pub credit: String,
    pub kind: SourceKind,
    pub dir: PathBuf,
    pub order: u16,
}

#[derive(Clone, Debug)]
pub struct Catalog {
    pub default: String,
    pub entries: Vec<Entry>,
}

pub fn load(path: &Path) -> crate::Result<Catalog> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse(&text, path.parent().unwrap_or(Path::new(".")))
}

/// Parses catalog JSON; source directories are resolved against `base`.
pub fn parse(text: &str, base: &Path) -> crate::Result<Catalog> {
    let root: Value = serde_json::from_str(text).map_err(|e| format!("catalog: {e}"))?;
    let default = string(&root, "default", "catalog")?;
    let packs = root
        .get("packs")
        .and_then(Value::as_array)
        .ok_or("catalog: missing \"packs\" array")?;
    let mut entries: Vec<Entry> = Vec::new();
    for (index, pack) in packs.iter().enumerate() {
        let id = string(pack, "id", "catalog pack")?;
        let ctx = format!("pack {id:?}");
        if entries.iter().any(|e| e.id == id) {
            return Err(format!("{ctx}: duplicate id"));
        }
        if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            return Err(format!("{ctx}: ids may only contain a-z, 0-9 and '-'"));
        }
        let category = string(pack, "category", &ctx)?;
        if !CATEGORIES.contains(&category.as_str()) {
            return Err(format!("{ctx}: unknown category {category:?}"));
        }
        let color = string(pack, "color", &ctx)?;
        if !is_hex_color(&color) {
            return Err(format!("{ctx}: color must look like #RRGGBB"));
        }
        let source = pack.get("source").ok_or_else(|| format!("{ctx}: missing source"))?;
        let kind = match string(source, "kind", &ctx)?.as_str() {
            "mechvibes" => SourceKind::Mechvibes,
            "kbsim" => SourceKind::Kbsim,
            other => return Err(format!("{ctx}: unknown source kind {other:?}")),
        };
        entries.push(Entry {
            name: string(pack, "name", &ctx)?,
            variant: pack.get("variant").and_then(Value::as_str).unwrap_or_default().to_owned(),
            credit: string(pack, "credit", &ctx)?,
            dir: base.join(string(source, "dir", &ctx)?),
            order: u16::try_from(index).map_err(|_| "catalog: too many packs")?,
            id,
            category,
            color,
            kind,
        });
    }
    if !entries.iter().any(|e| e.id == default) {
        return Err(format!("catalog: default pack {default:?} is not listed"));
    }
    Ok(Catalog { default, entries })
}

fn string(value: &Value, key: &str, ctx: &str) -> crate::Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("{ctx}: missing string field {key:?}"))
}

fn is_hex_color(s: &str) -> bool {
    s.len() == 7 && s.starts_with('#') && s[1..].chars().all(|c| c.is_ascii_hexdigit())
}
```

**File:** `crates/xtask/src/sources/mod.rs`
```rust
//! Adapters that read original soundpack layouts into a common shape.

pub mod kbsim;
pub mod mechvibes;

use rvpack::KEY_SLOTS;

/// Raw clips from a source pack, before clean-up. Each key maps to at most one
/// press clip and one release clip (indices into `clips`).
#[derive(Debug)]
pub struct RawPack {
    pub rate: u32,
    pub clips: Vec<Vec<f32>>,
    pub press: [Option<usize>; KEY_SLOTS],
    pub release: [Option<usize>; KEY_SLOTS],
}

impl RawPack {
    /// An empty pack; `rate` 0 adopts the rate of the first clip pushed.
    pub fn new(rate: u32) -> RawPack {
        RawPack { rate, clips: Vec::new(), press: [None; KEY_SLOTS], release: [None; KEY_SLOTS] }
    }

    /// Adds a clip recorded at `rate` and returns its index.
    pub fn push(&mut self, rate: u32, clip: Vec<f32>, what: &str) -> crate::Result<usize> {
        if self.rate == 0 {
            self.rate = rate;
        }
        if rate != self.rate {
            return Err(format!("{what}: sample rate {rate} Hz differs from {} Hz", self.rate));
        }
        self.clips.push(clip);
        Ok(self.clips.len() - 1)
    }
}
```

**File:** `crates/xtask/src/sources/mechvibes.rs` (implementation part)
```rust
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
```

**File:** `crates/xtask/src/sources/kbsim.rs` (implementation part)
```rust
//! kbsim packs: `press/GENERIC_R0`…`R4` (one recording per keyboard row) plus
//! `SPACE`, `ENTER` and `BACKSPACE`, and `release/GENERIC` plus the same three
//! specials. Files may be `.mp3`, `.wav` or `.ogg`.

use std::path::{Path, PathBuf};

use rvpack::keys::{self, code};

use super::RawPack;
use crate::{decode::decode_file, dsp};

pub fn load(dir: &Path) -> crate::Result<RawPack> {
    let mut pack = RawPack::new(0);
    let rows: Vec<Option<usize>> = (0..5)
        .map(|row| load_clip(dir, &format!("press/GENERIC_R{row}"), &mut pack))
        .collect::<crate::Result<_>>()?;
    let space = load_clip(dir, "press/SPACE", &mut pack)?;
    let enter = load_clip(dir, "press/ENTER", &mut pack)?;
    let backspace = load_clip(dir, "press/BACKSPACE", &mut pack)?;
    let up_generic = load_clip(dir, "release/GENERIC", &mut pack)?;
    let up_space = load_clip(dir, "release/SPACE", &mut pack)?;
    let up_enter = load_clip(dir, "release/ENTER", &mut pack)?;
    let up_backspace = load_clip(dir, "release/BACKSPACE", &mut pack)?;

    let any_row = rows
        .iter()
        .flatten()
        .next()
        .copied()
        .ok_or_else(|| format!("{}: no press/GENERIC_R* recordings", dir.display()))?;
    for key in keys::KEYS {
        let row = rows[usize::from(key.row.min(4))].unwrap_or(any_row);
        let (press, release) = match key.code {
            code::SPACE => (space.unwrap_or(row), up_space.or(up_generic)),
            code::RETURN | code::KEYPAD_ENTER => (enter.unwrap_or(row), up_enter.or(up_generic)),
            code::DELETE => (backspace.unwrap_or(row), up_backspace.or(up_generic)),
            _ => (row, up_generic),
        };
        pack.press[usize::from(key.code)] = Some(press);
        pack.release[usize::from(key.code)] = release;
    }
    Ok(pack)
}

fn load_clip(dir: &Path, stem: &str, pack: &mut RawPack) -> crate::Result<Option<usize>> {
    let Some(path) = find_audio(dir, stem) else { return Ok(None) };
    let mut audio = decode_file(&path)?;
    dsp::dc_block(&mut audio.samples, audio.rate);
    pack.push(audio.rate, audio.samples, &path.display().to_string()).map(Some)
}

fn find_audio(dir: &Path, stem: &str) -> Option<PathBuf> {
    ["mp3", "wav", "ogg"]
        .iter()
        .map(|ext| dir.join(format!("{stem}.{ext}")))
        .find(|path| path.is_file())
}
```

**File:** `crates/xtask/src/packs.rs` (implementation part)
```rust
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
        return Err(format!("{id}: a clip peaks at {max_peak_db:.2} dBFS (ceiling {} dBFS)", dsp::CEILING_DB));
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
```

- [ ] **Step 5: Run the tests, then convert the real catalog**

Run: `cargo test -p xtask && cargo test -p xtask --profile xtask -- --ignored && cargo xtask packs`
Expected: all 22 unit tests pass and the ignored full-catalog test passes; `cargo xtask packs` prints a 21-row table with every `lead` ≤ 1.00 ms, every `peak` ≤ −0.9 dBFS, `keys` = 120 for all packs, `release` = yes for the 10 kbsim packs, and ends with `21 packs, … MB`.

Run: `cargo clippy -p xtask --all-targets -- -D warnings && cargo fmt --check`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add assets/soundpacks crates/xtask
git commit -m "feat(xtask): curated soundpack catalog and .rvpack converter

21 MIT-licensed packs from Mechvibes and kbsim, trimmed to their transients,
loudness-matched and mapped to every macOS key.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 5: App crate and the real-time engine core (ring, RNG, mixer)

**Files:**
- Create: `crates/rustyvibes/Cargo.toml`, `crates/rustyvibes/src/main.rs`, `crates/rustyvibes/src/engine/mod.rs`, `crates/rustyvibes/src/engine/play.rs`, `crates/rustyvibes/src/engine/ring.rs`, `crates/rustyvibes/src/engine/rng.rs`, `crates/rustyvibes/src/engine/mixer.rs`
- Test: unit tests in `ring.rs`, `rng.rs`, `mixer.rs`

**Interfaces:**
- Consumes: `rvpack::{GUARD_BEFORE, GUARD_AFTER}`.
- Produces (used by Tasks 6–8):
  - `engine::Play { samples: &'static [i16], src_rate: u32, pitch: f32, gain_l: f32, gain_r: f32, delay_us: u32 }` (Copy + Send)
  - `engine::ring::{ring::<T: Copy + Send>(usize) -> (Producer<T>, Consumer<T>)}`, `Producer::push(&mut self, T) -> bool`, `Consumer::pop(&mut self) -> Option<T>`, `Consumer::is_empty(&self) -> bool`
  - `engine::rng::Rng::{new(u32), next_u32(), unit(), range(f32, f32)}`
  - `engine::mixer::{MAX_VOICES = 32, Mixer::new(f64), set_out_rate(f64), start(&Play), render(&mut [f32], &mut [f32], f32) -> bool, active_voices()}`

- [ ] **Step 1: Create the crate**

**File:** `crates/rustyvibes/Cargo.toml`
```toml
[package]
name = "rustyvibes"
description = "Mechanical keyboard sounds for every key press — a native macOS menu bar app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
publish = false

[dependencies]
rvpack = { path = "../rvpack" }

[lints]
workspace = true
```

**File:** `crates/rustyvibes/src/main.rs`
```rust
//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// The engine is wired into the app in later tasks.
#![allow(dead_code)]

mod engine;

fn main() {
    println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
}
```

**File:** `crates/rustyvibes/src/engine/mod.rs`
```rust
//! Real-time core: lock-free command ring, voice mixer and helpers.

pub mod mixer;
pub mod play;
pub mod ring;
pub mod rng;

pub use play::Play;
```

**File:** `crates/rustyvibes/src/engine/play.rs`
```rust
/// A request to start a voice, sent from the input or UI thread to the audio thread.
#[derive(Clone, Copy, Debug)]
pub struct Play {
    /// Clip samples including guards: `rvpack::GUARD_BEFORE` zeros, the audio,
    /// then `rvpack::GUARD_AFTER` zeros.
    pub samples: &'static [i16],
    /// Sample rate of the clip in Hz.
    pub src_rate: u32,
    /// Playback-rate multiplier; 1.0 keeps the original pitch.
    pub pitch: f32,
    pub gain_l: f32,
    pub gain_r: f32,
    /// Delay before the voice starts, in microseconds.
    pub delay_us: u32,
}
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/rustyvibes/src/engine/ring.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_order_and_capacity() {
        let (mut tx, mut rx) = ring::<u32>(4);
        for i in 0..4 {
            assert!(tx.push(i));
        }
        assert!(!tx.push(99), "full");
        for i in 0..4 {
            assert_eq!(rx.pop(), Some(i));
        }
        assert_eq!(rx.pop(), None);
        assert!(rx.is_empty());
    }

    #[test]
    fn capacity_rounds_up_to_a_power_of_two() {
        let (mut tx, _rx) = ring::<u8>(5);
        assert_eq!((0..100).filter(|_| tx.push(1)).count(), 8);
    }

    #[test]
    fn wraps_around_many_times() {
        let (mut tx, mut rx) = ring::<u64>(8);
        for i in 0..10_000u64 {
            assert!(tx.push(i));
            assert!(!rx.is_empty());
            assert_eq!(rx.pop(), Some(i));
        }
    }

    #[test]
    fn concurrent_stress_preserves_order() {
        const N: u64 = 1_000_000;
        let (mut tx, mut rx) = ring::<u64>(64);
        let producer = std::thread::spawn(move || {
            for i in 0..N {
                while !tx.push(i) {
                    std::hint::spin_loop();
                }
            }
        });
        let mut expected = 0;
        while expected < N {
            if let Some(v) = rx.pop() {
                assert_eq!(v, expected);
                expected += 1;
            } else {
                std::hint::spin_loop();
            }
        }
        producer.join().unwrap();
    }
}
```

**File:** `crates/rustyvibes/src/engine/rng.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_seed_still_produces_values() {
        assert_ne!(Rng::new(0).next_u32(), 0);
    }

    #[test]
    fn range_stays_in_bounds_and_covers_it() {
        let mut rng = Rng::new(42);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..10_000 {
            let x = rng.range(-35.0, 35.0);
            assert!((-35.0..35.0).contains(&x));
            lo = lo.min(x);
            hi = hi.max(x);
        }
        assert!(lo < -34.0 && hi > 34.0);
    }
}
```

**File:** `crates/rustyvibes/src/engine/mixer.rs` (test module; implementation goes above it in Step 4)
```rust
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
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p rustyvibes`
Expected: compile errors — `ring`, `Rng`, `Mixer`, `soft_clip`, `MAX_VOICES` not found.

- [ ] **Step 4: Implement ring, RNG and mixer** (place above each test module)

**File:** `crates/rustyvibes/src/engine/ring.rs` (implementation part)
```rust
//! Wait-free single-producer / single-consumer ring buffer for `Copy` items.

use std::cell::UnsafeCell;
use std::mem::MaybeUninit;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Keeps the producer and consumer indices on separate cache lines.
#[repr(align(128))]
struct Padded(AtomicUsize);

struct Inner<T> {
    /// Next slot to read; written only by the consumer.
    head: Padded,
    /// Next slot to write; written only by the producer.
    tail: Padded,
    mask: usize,
    slots: Box<[UnsafeCell<MaybeUninit<T>>]>,
}

// SAFETY: each slot is written by the single producer before it publishes `tail`
// (Release) and read by the single consumer after it observes `tail` (Acquire), and
// vice versa for `head`, so no slot is ever accessed concurrently.
unsafe impl<T: Send> Sync for Inner<T> {}

pub struct Producer<T> {
    inner: Arc<Inner<T>>,
    cached_head: usize,
}

pub struct Consumer<T> {
    inner: Arc<Inner<T>>,
    cached_tail: usize,
}

/// Creates a ring holding at least `capacity` items (rounded up to a power of two).
pub fn ring<T: Copy + Send>(capacity: usize) -> (Producer<T>, Consumer<T>) {
    let capacity = capacity.next_power_of_two().max(2);
    let slots = (0..capacity).map(|_| UnsafeCell::new(MaybeUninit::uninit())).collect();
    let inner = Arc::new(Inner {
        head: Padded(AtomicUsize::new(0)),
        tail: Padded(AtomicUsize::new(0)),
        mask: capacity - 1,
        slots,
    });
    (Producer { inner: inner.clone(), cached_head: 0 }, Consumer { inner, cached_tail: 0 })
}

impl<T: Copy + Send> Producer<T> {
    /// Appends `item`; returns `false` (dropping it) when the ring is full.
    pub fn push(&mut self, item: T) -> bool {
        let tail = self.inner.tail.0.load(Ordering::Relaxed);
        if tail.wrapping_sub(self.cached_head) > self.inner.mask {
            self.cached_head = self.inner.head.0.load(Ordering::Acquire);
            if tail.wrapping_sub(self.cached_head) > self.inner.mask {
                return false;
            }
        }
        // SAFETY: the slot is free (checked above) and invisible to the consumer until
        // `tail` is published below.
        unsafe { (*self.inner.slots[tail & self.inner.mask].get()).write(item) };
        self.inner.tail.0.store(tail.wrapping_add(1), Ordering::Release);
        true
    }
}

impl<T: Copy + Send> Consumer<T> {
    /// Removes the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        let head = self.inner.head.0.load(Ordering::Relaxed);
        if head == self.cached_tail {
            self.cached_tail = self.inner.tail.0.load(Ordering::Acquire);
            if head == self.cached_tail {
                return None;
            }
        }
        // SAFETY: the producer initialised this slot before publishing `tail` (Release),
        // which we observed with Acquire.
        let item = unsafe { (*self.inner.slots[head & self.inner.mask].get()).assume_init_read() };
        self.inner.head.0.store(head.wrapping_add(1), Ordering::Release);
        Some(item)
    }

    pub fn is_empty(&self) -> bool {
        self.inner.head.0.load(Ordering::Relaxed) == self.inner.tail.0.load(Ordering::Acquire)
    }
}
```

**File:** `crates/rustyvibes/src/engine/rng.rs` (implementation part)
```rust
//! Tiny xorshift32 generator for per-keystroke variation (not cryptographic).

#[derive(Clone, Debug)]
pub struct Rng(u32);

impl Rng {
    /// Seeds the generator; zero is replaced because xorshift cannot leave it.
    pub fn new(seed: u32) -> Rng {
        Rng(if seed == 0 { 0x9E37_79B9 } else { seed })
    }

    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform in [lo, hi).
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
}
```

**File:** `crates/rustyvibes/src/engine/mixer.rs` (implementation part)
```rust
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p rustyvibes && cargo clippy -p rustyvibes --all-targets -- -D warnings && cargo fmt --check`
Expected: 18 tests pass, clean.

- [ ] **Step 6: Commit**

```bash
git add crates/rustyvibes
git commit -m "feat(engine): lock-free command ring and real-time voice mixer

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Runtime soundpack library, shared state and the Voicer

**Files:**
- Create: `crates/rustyvibes/src/packs.rs`, `crates/rustyvibes/src/settings.rs`, `crates/rustyvibes/src/log.rs`
- Modify: `crates/rustyvibes/Cargo.toml` (add `libc`), `crates/rustyvibes/src/main.rs`, `crates/rustyvibes/src/engine/mod.rs`
- Test: unit tests in `packs.rs` and `engine/mod.rs`

**Interfaces:**
- Consumes: Task 1–2 `rvpack::{PackView, Meta, ClipRef, KEY_SLOTS, keys}`; Task 5 engine pieces.
- Produces (used by Tasks 7–10):
  - `settings::Settings { enabled: bool, pack: Option<String>, volume: f32, release_sounds: bool, variation: bool, spatial: bool }` + `Default`
  - `packs::{LoadedPack, Library, CATEGORIES, default_dir() -> PathBuf}`; `LoadedPack { meta, sample_rate, has_release }` + `press(u8)`, `release(u8)`, `clip(usize) -> &'static [i16]`, `display_name()`; `Library::{new(Vec<LoadedPack>), load_dir(&Path), len, is_empty, get(usize), iter, index_of(&str), default_index, resolve(Option<&str>) -> usize, make_resident(usize)}`
  - `packs::testing::{pack_data, leak_bytes, loaded}` (test-only)
  - `engine::{Shared, Stats, Kick, Voicer, pan_for_x, pan_gains, PITCH_JITTER_CENTS, GAIN_JITTER_DB, PAN_WIDTH}`; `Shared { enabled, release_sounds, variation, spatial: AtomicBool, pack: AtomicUsize, audio_running: AtomicBool, silent_frames: AtomicU64, stats: Stats }` + `new(&Settings, usize)`, `volume()`, `set_volume(f32)`, `master_gain()`; `Stats { key_events, plays, dropped, callbacks, max_render_ns, max_tap_ns, starts, stops: AtomicU64 }`; `Kick::{new(&'static AtomicBool, Thread), none(&'static AtomicBool), kick()}`; `Voicer::{new(&'static Shared, &'static Library, Producer<Play>, Kick, u32), key(u8, bool) -> bool, queue(usize, u8, bool, u32) -> bool, shared()}`
  - `log::{enabled(), log!}`

- [ ] **Step 1: Add the dependency and support modules**

**File:** `crates/rustyvibes/Cargo.toml`
```toml
[package]
name = "rustyvibes"
description = "Mechanical keyboard sounds for every key press — a native macOS menu bar app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
publish = false

[dependencies]
libc = "0.2"
rvpack = { path = "../rvpack" }

[lints]
workspace = true
```

**File:** `crates/rustyvibes/src/main.rs`
```rust
//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// Audio, input and UI are wired in later tasks.
#![allow(dead_code)]

mod engine;
mod log;
mod packs;
mod settings;

fn main() {
    println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
}
```

**File:** `crates/rustyvibes/src/log.rs`
```rust
//! Opt-in diagnostics: set `RUSTYVIBES_LOG=1` to print to stderr.
//! Never used on the audio or event-tap threads' hot paths.

use std::sync::OnceLock;

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUSTYVIBES_LOG").is_some_and(|v| v != "0"))
}

macro_rules! log {
    ($($arg:tt)*) => {
        if $crate::log::enabled() {
            eprintln!("[rustyvibes] {}", format_args!($($arg)*));
        }
    };
}

pub(crate) use log;
```

**File:** `crates/rustyvibes/src/settings.rs`
```rust
//! User settings.

/// User-facing settings (persisted in `NSUserDefaults` by the UI).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// Saved soundpack id; `None` until the user picks one.
    pub pack: Option<String>,
    pub volume: f32,
    pub release_sounds: bool,
    pub variation: bool,
    pub spatial: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            pack: None,
            volume: 0.6,
            release_sounds: true,
            variation: true,
            spatial: true,
        }
    }
}
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/rustyvibes/src/packs.rs` (test helpers and tests; implementation goes above them in Step 4)
```rust
#[cfg(test)]
pub mod testing {
    use super::LoadedPack;
    use rvpack::{ClipRef, KEY_SLOTS, Meta, PackData};

    /// Every key presses clip 0 (64 samples) and, with `with_release`, releases
    /// clip 1 (32 samples). Space has two variants (clips 2 and 3).
    pub fn pack_data(
        id: &str,
        category: &str,
        order: u16,
        is_default: bool,
        with_release: bool,
    ) -> PackData {
        let mut press = [ClipRef::NONE; KEY_SLOTS];
        let mut release = [ClipRef::NONE; KEY_SLOTS];
        for key in rvpack::keys::KEYS {
            press[usize::from(key.code)] = ClipRef::single(0);
            if with_release {
                release[usize::from(key.code)] = ClipRef::single(1);
            }
        }
        press[usize::from(rvpack::keys::code::SPACE)] = ClipRef { first: 2, count: 2 };
        PackData {
            sample_rate: 44_100,
            meta: Meta {
                id: id.into(),
                name: id.into(),
                variant: String::new(),
                category: category.into(),
                color: "#808080".into(),
                credit: "test".into(),
                order,
                is_default,
            },
            press,
            release,
            clips: vec![vec![1_000; 64], vec![-1_000; 32], vec![2_000; 48], vec![3_000; 48]],
        }
    }

    /// Serialises `data` into leaked, 8-byte aligned storage (like an `mmap`).
    pub fn leak_bytes(data: &PackData) -> &'static [u8] {
        let bytes = data.to_bytes().unwrap();
        let words: &'static mut [u64] =
            Box::leak(vec![0u64; bytes.len().div_ceil(8)].into_boxed_slice());
        // SAFETY: `words` provides at least `bytes.len()` writable, leaked bytes.
        let out = unsafe {
            std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), bytes.len())
        };
        out.copy_from_slice(&bytes);
        out
    }

    pub fn loaded(data: &PackData) -> LoadedPack {
        LoadedPack::from_bytes(leak_bytes(data)).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{loaded, pack_data};
    use super::*;

    #[test]
    fn library_orders_by_category_then_catalog_order() {
        let library = Library::new(vec![
            loaded(&pack_data("c", "clicky", 0, false, false)),
            loaded(&pack_data("t", "tactile", 5, false, false)),
            loaded(&pack_data("l2", "linear", 2, false, false)),
            loaded(&pack_data("l1", "linear", 1, true, false)),
        ]);
        let ids: Vec<&str> = library.iter().map(|p| p.meta.id.as_str()).collect();
        assert_eq!(ids, ["l1", "l2", "t", "c"]);
    }

    #[test]
    fn resolve_unknown_id_falls_back_to_default() {
        let library = Library::new(vec![
            loaded(&pack_data("a", "linear", 0, false, false)),
            loaded(&pack_data("b", "linear", 1, true, false)),
        ]);
        assert_eq!(library.resolve(Some("a")), 0);
        assert_eq!(library.resolve(Some("renamed-in-an-update")), 1);
        assert_eq!(library.resolve(None), 1);
        assert_eq!(Library::new(Vec::new()).resolve(Some("x")), 0);
    }

    #[test]
    fn loaded_pack_exposes_guarded_clips() {
        let pack = loaded(&pack_data("a", "linear", 0, false, true));
        assert!(pack.has_release);
        let clips = pack.press(rvpack::keys::code::A);
        let clip = pack.clip(usize::from(clips.first));
        assert_eq!(clip.len(), rvpack::GUARD_BEFORE + 64 + rvpack::GUARD_AFTER);
        assert_eq!(pack.press(200), ClipRef::NONE);
        assert_eq!(pack.display_name(), "a");
    }

    #[test]
    fn load_dir_skips_corrupt_files() {
        let dir = std::env::temp_dir().join(format!("rustyvibes-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let good = pack_data("good", "linear", 0, true, false).to_bytes().unwrap();
        std::fs::write(dir.join("good.rvpack"), good).unwrap();
        std::fs::write(dir.join("bad.rvpack"), b"definitely not a soundpack").unwrap();
        std::fs::write(dir.join("empty.rvpack"), b"").unwrap();
        std::fs::write(dir.join("notes.txt"), b"ignored").unwrap();
        let library = Library::load_dir(&dir);
        assert_eq!(library.len(), 1);
        assert_eq!(library.get(0).unwrap().meta.id, "good");
        library.make_resident(0);
    }

    #[test]
    fn missing_directory_yields_an_empty_library() {
        assert!(Library::load_dir(Path::new("/nonexistent/packs")).is_empty());
    }
}
```

**File:** `crates/rustyvibes/src/engine/mod.rs` (tests appended at the bottom; implementation in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::ring::{Consumer, ring};
    use super::*;
    use crate::packs::testing::{loaded, pack_data};
    use crate::settings::Settings;
    use rvpack::keys::code;

    fn setup(with_release: bool) -> (&'static Shared, Voicer, Consumer<Play>) {
        let library: &'static Library = Box::leak(Box::new(Library::new(vec![loaded(
            &pack_data("p", "linear", 0, true, with_release),
        )])));
        let settings = Settings { variation: false, spatial: false, ..Settings::default() };
        let shared: &'static Shared = Box::leak(Box::new(Shared::new(&settings, 0)));
        let (tx, rx) = ring(4);
        let voicer = Voicer::new(shared, library, tx, Kick::none(&shared.audio_running), 7);
        (shared, voicer, rx)
    }

    #[test]
    fn key_down_queues_the_press_clip_at_unity() {
        let (shared, mut voicer, mut rx) = setup(false);
        assert!(voicer.key(code::A, true));
        let p = rx.pop().unwrap();
        assert_eq!(p.samples.len(), rvpack::GUARD_BEFORE + 64 + rvpack::GUARD_AFTER);
        assert_eq!((p.pitch, p.gain_l, p.gain_r, p.delay_us, p.src_rate), (1.0, 1.0, 1.0, 0, 44_100));
        assert_eq!(shared.stats.plays.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn disabled_means_silent() {
        let (shared, mut voicer, mut rx) = setup(false);
        shared.enabled.store(false, Ordering::Relaxed);
        assert!(!voicer.key(code::A, true));
        assert!(rx.pop().is_none());
        assert_eq!(shared.stats.key_events.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn release_sounds_respect_the_setting_and_the_pack() {
        let (shared, mut voicer, mut rx) = setup(true);
        assert!(voicer.key(code::A, false));
        assert_eq!(rx.pop().unwrap().samples.len(), rvpack::GUARD_BEFORE + 32 + rvpack::GUARD_AFTER);
        shared.release_sounds.store(false, Ordering::Relaxed);
        assert!(!voicer.key(code::A, false));
        let (_, mut voicer, _) = setup(false);
        assert!(!voicer.key(code::A, false), "packs without release sounds stay silent on key up");
    }

    #[test]
    fn spatial_pans_left_keys_left_and_right_keys_right() {
        let (shared, mut voicer, mut rx) = setup(false);
        shared.spatial.store(true, Ordering::Relaxed);
        voicer.key(code::ESCAPE, true);
        let left = rx.pop().unwrap();
        assert!(left.gain_l > left.gain_r);
        voicer.key(code::RETURN, true);
        let right = rx.pop().unwrap();
        assert!(right.gain_r > right.gain_l);
        assert!((left.gain_l.powi(2) + left.gain_r.powi(2) - 2.0).abs() < 1e-4, "equal power");
    }

    #[test]
    fn variation_stays_within_bounds() {
        let (shared, mut voicer, mut rx) = setup(false);
        shared.variation.store(true, Ordering::Relaxed);
        let max_pitch = cents_to_ratio(PITCH_JITTER_CENTS);
        let max_gain = db_to_gain(GAIN_JITTER_DB);
        let mut distinct = std::collections::HashSet::new();
        for _ in 0..200 {
            voicer.key(code::A, true);
            let p = rx.pop().unwrap();
            assert!(p.pitch <= max_pitch && p.pitch >= 1.0 / max_pitch);
            assert!(p.gain_l <= max_gain && p.gain_l >= 1.0 / max_gain);
            distinct.insert(p.pitch.to_bits());
        }
        assert!(distinct.len() > 100);
    }

    #[test]
    fn variants_never_repeat_back_to_back() {
        let (_, mut voicer, mut rx) = setup(false);
        let mut last = None;
        for _ in 0..100 {
            voicer.key(code::SPACE, true);
            let id = rx.pop().unwrap().samples.as_ptr();
            assert_ne!(Some(id), last);
            last = Some(id);
        }
    }

    #[test]
    fn full_ring_drops_and_counts() {
        let (shared, mut voicer, _rx) = setup(false);
        for _ in 0..4 {
            assert!(voicer.key(code::A, true));
        }
        assert!(!voicer.key(code::A, true));
        assert_eq!(shared.stats.dropped.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn previews_ignore_the_enabled_switch_and_carry_a_delay() {
        let (shared, mut voicer, mut rx) = setup(false);
        shared.enabled.store(false, Ordering::Relaxed);
        assert!(voicer.queue(0, code::A, true, 1_500));
        assert_eq!(rx.pop().unwrap().delay_us, 1_500);
        assert!(!voicer.queue(9, code::A, true, 0), "unknown pack");
    }

    #[test]
    fn pan_law_is_unity_at_the_centre() {
        let (l, r) = pan_gains(0.0);
        assert!((l - 1.0).abs() < 1e-6 && (r - 1.0).abs() < 1e-6);
        assert!(pan_for_x(0.0) < 0.0 && pan_for_x(14.0) > 0.0);
        assert_eq!(pan_for_x(22.0), PAN_WIDTH);
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p rustyvibes`
Expected: compile errors — `Library`, `LoadedPack`, `Shared`, `Voicer`, `Kick` not found.

- [ ] **Step 4: Implement packs and the engine state**

**File:** `crates/rustyvibes/src/packs.rs` (implementation part, above the test modules)
```rust
//! Soundpacks at runtime. `.rvpack` files are memory-mapped once and never
//! unmapped, so clip slices are `&'static` and switching packs is a single
//! atomic store; only the active pack's samples are kept resident.

use std::ffi::c_void;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use rvpack::{ClipRef, KEY_SLOTS, Meta, PackView};

use crate::log::log;

/// Menu categories in display order.
pub const CATEGORIES: [&str; 3] = ["linear", "tactile", "clicky"];

/// A parsed soundpack whose samples live for the rest of the process.
pub struct LoadedPack {
    pub meta: Meta,
    pub sample_rate: u32,
    pub has_release: bool,
    press: [ClipRef; KEY_SLOTS],
    release: [ClipRef; KEY_SLOTS],
    /// Clip samples including guards.
    clips: Vec<&'static [i16]>,
    pcm: &'static [i16],
}

impl LoadedPack {
    pub fn from_bytes(bytes: &'static [u8]) -> Result<LoadedPack, rvpack::FormatError> {
        let view = PackView::parse(bytes)?;
        Ok(LoadedPack {
            press: std::array::from_fn(|k| view.press(k as u8)),
            release: std::array::from_fn(|k| view.release(k as u8)),
            clips: (0..view.clip_count()).map(|i| view.clip_with_guards(i)).collect(),
            pcm: view.pcm(),
            sample_rate: view.sample_rate,
            has_release: view.has_release(),
            meta: view.meta,
        })
    }

    pub fn press(&self, keycode: u8) -> ClipRef {
        self.press.get(usize::from(keycode)).copied().unwrap_or(ClipRef::NONE)
    }

    pub fn release(&self, keycode: u8) -> ClipRef {
        self.release.get(usize::from(keycode)).copied().unwrap_or(ClipRef::NONE)
    }

    /// Clip samples with guards. Clip references were validated when the pack loaded.
    pub fn clip(&self, index: usize) -> &'static [i16] {
        self.clips[index]
    }

    /// "Cherry MX Brown · PBT", or just the name when there is no variant.
    pub fn display_name(&self) -> String {
        if self.meta.variant.is_empty() {
            self.meta.name.clone()
        } else {
            format!("{} · {}", self.meta.name, self.meta.variant)
        }
    }
}

/// All available packs, ordered for the menu.
pub struct Library {
    packs: Vec<LoadedPack>,
}

impl Library {
    pub fn new(mut packs: Vec<LoadedPack>) -> Library {
        packs.sort_by_key(|p| (category_rank(&p.meta.category), p.meta.order));
        Library { packs }
    }

    /// Maps every `*.rvpack` in `dir`; unreadable or corrupt files are skipped.
    pub fn load_dir(dir: &Path) -> Library {
        let mut paths: Vec<PathBuf> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "rvpack"))
                .collect(),
            Err(e) => {
                log!("cannot read {}: {e}", dir.display());
                Vec::new()
            }
        };
        paths.sort();
        let mut packs = Vec::new();
        for path in paths {
            let loaded = map_file(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| LoadedPack::from_bytes(bytes).map_err(|e| e.to_string()));
            match loaded {
                Ok(pack) => packs.push(pack),
                Err(e) => log!("skipping {}: {e}", path.display()),
            }
        }
        log!("loaded {} soundpacks from {}", packs.len(), dir.display());
        Library::new(packs)
    }

    pub fn len(&self) -> usize {
        self.packs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packs.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&LoadedPack> {
        self.packs.get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, LoadedPack> {
        self.packs.iter()
    }

    pub fn index_of(&self, id: &str) -> Option<usize> {
        self.packs.iter().position(|p| p.meta.id == id)
    }

    pub fn default_index(&self) -> usize {
        self.packs.iter().position(|p| p.meta.is_default).unwrap_or(0)
    }

    /// The pack for a saved id, or the default pack when the id is unknown.
    pub fn resolve(&self, id: Option<&str>) -> usize {
        id.and_then(|id| self.index_of(id)).unwrap_or_else(|| self.default_index())
    }

    /// Prefaults and locks the samples of `index` so the audio thread never
    /// page-faults, and unlocks every other pack.
    pub fn make_resident(&self, index: usize) {
        for (i, pack) in self.packs.iter().enumerate() {
            if i == index {
                lock(pack.pcm);
            } else {
                unlock(pack.pcm);
            }
        }
    }
}

/// Where the bundled packs live: `$RUSTYVIBES_PACKS`, the app bundle's
/// `Contents/Resources/Packs`, or `target/packs` during development.
pub fn default_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("RUSTYVIBES_PACKS") {
        return dir.into();
    }
    if let Some(exe) = std::env::current_exe().ok().and_then(|p| p.canonicalize().ok()) {
        if let Some(contents) = exe.parent().and_then(Path::parent) {
            let bundled = contents.join("Resources/Packs");
            if bundled.is_dir() {
                return bundled;
            }
        }
        for dir in exe.ancestors() {
            if dir.file_name().is_some_and(|n| n == "target") && dir.join("packs").is_dir() {
                return dir.join("packs");
            }
        }
    }
    PathBuf::from("target/packs")
}

fn category_rank(category: &str) -> usize {
    CATEGORIES.iter().position(|c| *c == category).unwrap_or(CATEGORIES.len())
}

/// Maps a file read-only for the rest of the process.
fn map_file(path: &Path) -> std::io::Result<&'static [u8]> {
    let file = std::fs::File::open(path)?;
    let len = usize::try_from(file.metadata()?.len()).map_err(std::io::Error::other)?;
    if len == 0 {
        return Err(std::io::Error::other("empty file"));
    }
    // SAFETY: a read-only private mapping of a regular file; it is never unmapped, so the
    // returned slice stays valid for 'static. The fd may close once the mapping exists.
    let ptr = unsafe {
        libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, file.as_raw_fd(), 0)
    };
    if ptr == libc::MAP_FAILED {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `ptr` maps `len` readable bytes for the rest of the process.
    Ok(unsafe { std::slice::from_raw_parts(ptr.cast::<u8>(), len) })
}

fn page_range(samples: &[i16]) -> (*mut c_void, usize) {
    // SAFETY: sysconf has no preconditions.
    let page = usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).unwrap_or(16_384);
    let start = samples.as_ptr() as usize & !(page - 1);
    let end = (samples.as_ptr() as usize + std::mem::size_of_val(samples)).next_multiple_of(page);
    (start as *mut c_void, end - start)
}

fn lock(samples: &'static [i16]) {
    let (ptr, len) = page_range(samples);
    // SAFETY: the range covers mapped pages of `samples`; madvise/mlock only change residency.
    unsafe {
        libc::madvise(ptr, len, libc::MADV_WILLNEED);
    }
    // Touch one sample per page so the data is resident before the first keystroke.
    let mut sum = 0i32;
    for s in samples.iter().step_by(2_048) {
        sum = sum.wrapping_add(i32::from(*s));
    }
    std::hint::black_box(sum);
    // SAFETY: as above; failure (e.g. a lock limit) is harmless.
    unsafe {
        libc::mlock(ptr, len);
    }
}

fn unlock(samples: &'static [i16]) {
    let (ptr, len) = page_range(samples);
    // SAFETY: unlocking mapped pages; harmless if they were never locked.
    unsafe {
        libc::munlock(ptr, len);
    }
}
```

**File:** `crates/rustyvibes/src/engine/mod.rs` (implementation part, above the test module)
```rust
//! Real-time core: shared settings and counters, the key→sound `Voicer`, the
//! lock-free command ring and the voice mixer.

pub mod mixer;
pub mod play;
pub mod ring;
pub mod rng;

pub use play::Play;

use std::f32::consts::{FRAC_PI_4, SQRT_2};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering, fence};
use std::thread::Thread;

use rvpack::ClipRef;

use crate::packs::Library;
use crate::settings::Settings;
use ring::Producer;
use rng::Rng;

/// Pitch jitter per keystroke with natural variation on (± cents).
pub const PITCH_JITTER_CENTS: f32 = 35.0;
/// Gain jitter per keystroke with natural variation on (± dB).
pub const GAIN_JITTER_DB: f32 = 1.5;
/// Largest pan with spatial stereo on (1.0 would be hard left/right).
pub const PAN_WIDTH: f32 = 0.4;

/// Settings and counters shared by every thread. Everything is atomic, so
/// real-time threads never block reading it.
pub struct Shared {
    pub enabled: AtomicBool,
    volume: AtomicU32,
    pub release_sounds: AtomicBool,
    pub variation: AtomicBool,
    pub spatial: AtomicBool,
    /// Index of the active pack in the `Library`.
    pub pack: AtomicUsize,
    /// Whether the audio unit is running (written by the audio control thread).
    pub audio_running: AtomicBool,
    /// Frames of continuous output silence (written by the render callback).
    pub silent_frames: AtomicU64,
    pub stats: Stats,
}

/// Diagnostic counters (relaxed atomics).
#[derive(Default)]
pub struct Stats {
    pub key_events: AtomicU64,
    pub plays: AtomicU64,
    pub dropped: AtomicU64,
    pub callbacks: AtomicU64,
    pub max_render_ns: AtomicU64,
    pub max_tap_ns: AtomicU64,
    pub starts: AtomicU64,
    pub stops: AtomicU64,
}

impl Shared {
    pub fn new(settings: &Settings, pack: usize) -> Shared {
        Shared {
            enabled: AtomicBool::new(settings.enabled),
            volume: AtomicU32::new(settings.volume.clamp(0.0, 1.0).to_bits()),
            release_sounds: AtomicBool::new(settings.release_sounds),
            variation: AtomicBool::new(settings.variation),
            spatial: AtomicBool::new(settings.spatial),
            pack: AtomicUsize::new(pack),
            audio_running: AtomicBool::new(false),
            silent_frames: AtomicU64::new(0),
            stats: Stats::default(),
        }
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Linear output gain for the current volume (perceptual curve: volume²).
    pub fn master_gain(&self) -> f32 {
        let v = self.volume();
        v * v
    }
}

/// Wakes the audio control thread when a sound is queued while audio is stopped.
#[derive(Clone)]
pub struct Kick {
    running: &'static AtomicBool,
    thread: Option<Thread>,
}

impl Kick {
    pub fn new(running: &'static AtomicBool, thread: Thread) -> Kick {
        Kick { running, thread: Some(thread) }
    }

    /// A kick that wakes nobody (tests, or when audio is unavailable).
    pub fn none(running: &'static AtomicBool) -> Kick {
        Kick { running, thread: None }
    }

    pub fn kick(&self) {
        // Pairs with the fence in the audio thread's stop path: either we see
        // `running == false` and wake it, or it sees our queued command.
        fence(Ordering::SeqCst);
        if !self.running.load(Ordering::Relaxed) {
            if let Some(thread) = &self.thread {
                thread.unpark();
            }
        }
    }
}

/// Turns key transitions into `Play` commands for one producer thread.
pub struct Voicer {
    shared: &'static Shared,
    library: &'static Library,
    producer: Producer<Play>,
    kick: Kick,
    rng: Rng,
    pan: [f32; 128],
    last_choice: [u16; 128],
}

impl Voicer {
    pub fn new(
        shared: &'static Shared,
        library: &'static Library,
        producer: Producer<Play>,
        kick: Kick,
        seed: u32,
    ) -> Voicer {
        let mut pan = [0.0; 128];
        for key in rvpack::keys::KEYS {
            pan[usize::from(key.code)] = pan_for_x(key.x);
        }
        Voicer { shared, library, producer, kick, rng: Rng::new(seed), pan, last_choice: [u16::MAX; 128] }
    }

    pub fn shared(&self) -> &'static Shared {
        self.shared
    }

    /// A physical key went down or up. Returns whether a sound was queued.
    pub fn key(&mut self, keycode: u8, down: bool) -> bool {
        self.shared.stats.key_events.fetch_add(1, Ordering::Relaxed);
        if !self.shared.enabled.load(Ordering::Relaxed) {
            return false;
        }
        if !down && !self.shared.release_sounds.load(Ordering::Relaxed) {
            return false;
        }
        let pack = self.shared.pack.load(Ordering::Relaxed);
        self.queue(pack, keycode, down, 0)
    }

    /// Queues the sound `pack` makes for a key transition, ignoring the
    /// enabled switch (previews use this directly).
    pub fn queue(&mut self, pack: usize, keycode: u8, down: bool, delay_us: u32) -> bool {
        let Some(pack) = self.library.get(pack) else { return false };
        let clips = if down { pack.press(keycode) } else { pack.release(keycode) };
        if clips.is_none() {
            return false;
        }
        let index = self.choose(keycode, clips);
        let (mut gain, mut pitch) = (1.0, 1.0);
        if self.shared.variation.load(Ordering::Relaxed) {
            gain = db_to_gain(self.rng.range(-GAIN_JITTER_DB, GAIN_JITTER_DB));
            pitch = cents_to_ratio(self.rng.range(-PITCH_JITTER_CENTS, PITCH_JITTER_CENTS));
        }
        let (left, right) = if self.shared.spatial.load(Ordering::Relaxed) {
            pan_gains(self.pan[usize::from(keycode & 0x7F)])
        } else {
            (1.0, 1.0)
        };
        let play = Play {
            samples: pack.clip(index),
            src_rate: pack.sample_rate,
            pitch,
            gain_l: gain * left,
            gain_r: gain * right,
            delay_us,
        };
        if self.producer.push(play) {
            self.shared.stats.plays.fetch_add(1, Ordering::Relaxed);
            self.kick.kick();
            true
        } else {
            self.shared.stats.dropped.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Picks one of a key's variants at random, never the same one twice in a row.
    fn choose(&mut self, keycode: u8, clips: ClipRef) -> usize {
        if clips.count <= 1 {
            return usize::from(clips.first);
        }
        let slot = usize::from(keycode & 0x7F);
        let mut pick = (self.rng.next_u32() % u32::from(clips.count)) as u16;
        if pick == self.last_choice[slot] {
            pick = (pick + 1) % clips.count;
        }
        self.last_choice[slot] = pick;
        usize::from(clips.first + pick)
    }
}

/// Maps a key's horizontal position (key units) to a pan in [−PAN_WIDTH, PAN_WIDTH].
pub fn pan_for_x(x: f32) -> f32 {
    ((x - 7.25) / 7.75).clamp(-1.0, 1.0) * PAN_WIDTH
}

/// Equal-power pan, normalised so a centred key keeps unity gain on both channels.
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (angle.cos() * SQRT_2, angle.sin() * SQRT_2)
}

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

fn cents_to_ratio(cents: f32) -> f32 {
    2f32.powf(cents / 1200.0)
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p rustyvibes && cargo clippy -p rustyvibes --all-targets -- -D warnings && cargo fmt --check`
Expected: 32 tests pass, clean.

- [ ] **Step 6: Commit**

```bash
git add crates/rustyvibes
git commit -m "feat(engine): mmap'd soundpack library, shared state and voicer

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: CoreAudio output, runtime wiring, `--selftest` and `--bench`

**Files:**
- Create: `crates/rustyvibes/src/audio/mod.rs`, `crates/rustyvibes/src/audio/ffi.rs`, `crates/rustyvibes/src/runtime.rs`, `crates/rustyvibes/src/diagnostics.rs`
- Modify: `crates/rustyvibes/src/main.rs`
- Test: `audio/ffi.rs` layout test; live `--selftest` and `--bench`

**Interfaces:**
- Consumes: Task 5–6 `engine::{Shared, Kick, Play, Voicer, mixer::Mixer, ring}`, `packs::{Library, default_dir}`, `settings::Settings`.
- Produces (used by Tasks 8–10):
  - `audio::{spawn(&'static Shared, Vec<Consumer<Play>>) -> Kick, IDLE_STOP: Duration, BUFFER_FRAMES: u32}`
  - `runtime::Runtime { shared: &'static Shared, library: &'static Library, preview: Voicer }` with `start(&Settings)`, `active_pack()`, `select_pack(usize)`, `preview_flourish(usize)`, `preview_click()`, `take_input() -> Option<Voicer>`, `return_input(Voicer)`
  - `diagnostics::{selftest(&[String]) -> Result<(), String>, bench()}`

- [ ] **Step 1: Write the FFI layout test**

**File:** `crates/rustyvibes/src/audio/ffi.rs`
```rust
//! Minimal AudioToolbox FFI for an AUHAL default-output unit.

#![allow(non_snake_case, non_upper_case_globals)]

use std::ffi::c_void;

pub type OSStatus = i32;
pub type AudioComponent = *mut c_void;
pub type AudioUnit = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioComponentDescription {
    pub componentType: u32,
    pub componentSubType: u32,
    pub componentManufacturer: u32,
    pub componentFlags: u32,
    pub componentFlagsMask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AudioStreamBasicDescription {
    pub mSampleRate: f64,
    pub mFormatID: u32,
    pub mFormatFlags: u32,
    pub mBytesPerPacket: u32,
    pub mFramesPerPacket: u32,
    pub mBytesPerFrame: u32,
    pub mChannelsPerFrame: u32,
    pub mBitsPerChannel: u32,
    pub mReserved: u32,
}

#[repr(C)]
pub struct AudioBuffer {
    pub mNumberChannels: u32,
    pub mDataByteSize: u32,
    pub mData: *mut c_void,
}

/// Variable-length in C: `mNumberBuffers` entries follow.
#[repr(C)]
pub struct AudioBufferList {
    pub mNumberBuffers: u32,
    pub mBuffers: [AudioBuffer; 1],
}

/// Opaque here: the render callback never reads the timestamp.
#[repr(C)]
pub struct AudioTimeStamp {
    _private: [u8; 0],
}

pub type AURenderCallback = unsafe extern "C" fn(
    *mut c_void,
    *mut u32,
    *const AudioTimeStamp,
    u32,
    u32,
    *mut AudioBufferList,
) -> OSStatus;

#[repr(C)]
pub struct AURenderCallbackStruct {
    pub inputProc: AURenderCallback,
    pub inputProcRefCon: *mut c_void,
}

const fn fourcc(code: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*code)
}

pub const kAudioUnitType_Output: u32 = fourcc(b"auou");
pub const kAudioUnitSubType_DefaultOutput: u32 = fourcc(b"def ");
pub const kAudioUnitManufacturer_Apple: u32 = fourcc(b"appl");
pub const kAudioFormatLinearPCM: u32 = fourcc(b"lpcm");
pub const kAudioFormatFlagIsFloat: u32 = 1 << 0;
pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;
pub const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;
pub const kAudioUnitProperty_StreamFormat: u32 = 8;
pub const kAudioUnitProperty_MaximumFramesPerSlice: u32 = 14;
pub const kAudioUnitProperty_SetRenderCallback: u32 = 23;
pub const kAudioOutputUnitProperty_IsRunning: u32 = 2001;
pub const kAudioDevicePropertyBufferFrameSize: u32 = fourcc(b"fsiz");
pub const kAudioUnitScope_Global: u32 = 0;
pub const kAudioUnitScope_Input: u32 = 1;
pub const kAudioUnitScope_Output: u32 = 2;
pub const kAudioUnitRenderAction_OutputIsSilence: u32 = 1 << 4;

#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    pub fn AudioComponentFindNext(
        component: AudioComponent,
        desc: *const AudioComponentDescription,
    ) -> AudioComponent;
    pub fn AudioComponentInstanceNew(component: AudioComponent, out: *mut AudioUnit) -> OSStatus;
    pub fn AudioComponentInstanceDispose(unit: AudioUnit) -> OSStatus;
    pub fn AudioUnitInitialize(unit: AudioUnit) -> OSStatus;
    pub fn AudioUnitUninitialize(unit: AudioUnit) -> OSStatus;
    pub fn AudioUnitSetProperty(
        unit: AudioUnit,
        id: u32,
        scope: u32,
        element: u32,
        data: *const c_void,
        size: u32,
    ) -> OSStatus;
    pub fn AudioUnitGetProperty(
        unit: AudioUnit,
        id: u32,
        scope: u32,
        element: u32,
        data: *mut c_void,
        size: *mut u32,
    ) -> OSStatus;
    pub fn AudioOutputUnitStart(unit: AudioUnit) -> OSStatus;
    pub fn AudioOutputUnitStop(unit: AudioUnit) -> OSStatus;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_match_the_c_headers() {
        assert_eq!(std::mem::size_of::<AudioStreamBasicDescription>(), 40);
        assert_eq!(std::mem::size_of::<AudioComponentDescription>(), 20);
        assert_eq!(std::mem::size_of::<AudioBuffer>(), 16);
        assert_eq!(std::mem::size_of::<AudioBufferList>(), 24);
        assert_eq!(kAudioUnitType_Output, 0x6175_6F75);
    }
}
```

- [ ] **Step 2: Implement the audio output**

**File:** `crates/rustyvibes/src/audio/mod.rs`
```rust
//! Audio output: an AUHAL default-output unit (it follows the system output
//! device), started on demand by a control thread and stopped after a few
//! seconds of silence, so Rustyvibes never holds the device or keeps the Mac
//! awake while idle.

mod ffi;

use std::ffi::c_void;
use std::sync::atomic::{Ordering, fence};
use std::time::{Duration, Instant};

use crate::engine::mixer::Mixer;
use crate::engine::ring::Consumer;
use crate::engine::{Kick, Play, Shared};
use crate::log::log;

/// Stop the audio unit after this much continuous silence.
pub const IDLE_STOP: Duration = Duration::from_secs(8);
/// Requested hardware buffer (≈2.7 ms at 48 kHz); the device may round it.
pub const BUFFER_FRAMES: u32 = 128;
const MAX_FRAMES_PER_SLICE: u32 = 4096;

/// State used by the CoreAudio IO thread while the unit runs, and by the
/// control thread only while it is stopped.
struct RenderState {
    shared: &'static Shared,
    mixer: Mixer,
    inputs: Vec<Consumer<Play>>,
}

/// Starts the audio control thread and returns the `Kick` producers use to wake it.
pub fn spawn(shared: &'static Shared, inputs: Vec<Consumer<Play>>) -> Kick {
    let handle = std::thread::Builder::new()
        .name("rustyvibes-audio".into())
        .spawn(move || Control::new(shared, inputs).run())
        .expect("failed to spawn the audio control thread");
    Kick::new(&shared.audio_running, handle.thread().clone())
}

struct Control {
    shared: &'static Shared,
    /// Leaked on purpose: CoreAudio holds this pointer for the life of the process.
    state: *mut RenderState,
    unit: Option<Unit>,
    running: bool,
    rate: f64,
}

impl Control {
    fn new(shared: &'static Shared, inputs: Vec<Consumer<Play>>) -> Control {
        let state = Box::into_raw(Box::new(RenderState { shared, mixer: Mixer::new(48_000.0), inputs }));
        Control { shared, state, unit: None, running: false, rate: 48_000.0 }
    }

    fn run(mut self) -> ! {
        loop {
            if self.running {
                std::thread::park_timeout(Duration::from_millis(500));
                let silent = self.shared.silent_frames.load(Ordering::Relaxed) as f64;
                let idle = silent >= IDLE_STOP.as_secs_f64() * self.rate;
                let alive = self.unit.as_ref().is_some_and(Unit::is_running);
                if idle || !alive {
                    self.stop(if idle { "idle" } else { "device stopped" });
                }
            } else {
                std::thread::park();
                if self.has_pending() {
                    self.start();
                }
            }
        }
    }

    fn has_pending(&self) -> bool {
        // SAFETY: called only while the unit is stopped, so the IO thread is not using it.
        let state = unsafe { &*self.state };
        state.inputs.iter().any(|input| !input.is_empty())
    }

    fn start(&mut self) {
        let started = Instant::now();
        if self.unit.is_none() {
            match Unit::new(self.state.cast()) {
                Ok(unit) => self.unit = Some(unit),
                Err(status) => {
                    log!("audio: cannot create the output unit (OSStatus {status})");
                    return;
                }
            }
        }
        let Some(unit) = self.unit.as_mut() else { return };
        match unit.configure() {
            Ok(rate) => {
                // SAFETY: the unit is stopped, so the IO thread is not using the state.
                unsafe { (*self.state).mixer.set_out_rate(rate) };
                self.rate = rate;
            }
            Err(status) => {
                log!("audio: cannot configure the output (OSStatus {status})");
                return;
            }
        }
        self.shared.silent_frames.store(0, Ordering::Relaxed);
        let status = unit.start();
        if status != 0 {
            log!("audio: start failed (OSStatus {status})");
            return;
        }
        self.running = true;
        self.shared.audio_running.store(true, Ordering::SeqCst);
        self.shared.stats.starts.fetch_add(1, Ordering::Relaxed);
        log!("audio: started at {} Hz in {:.1} ms", self.rate, started.elapsed().as_secs_f64() * 1e3);
    }

    fn stop(&mut self, why: &str) {
        if let Some(unit) = &self.unit {
            unit.stop();
        }
        self.running = false;
        self.shared.audio_running.store(false, Ordering::SeqCst);
        // Pairs with `Kick::kick`: a command queued before this point is seen below.
        fence(Ordering::SeqCst);
        self.shared.stats.stops.fetch_add(1, Ordering::Relaxed);
        log!("audio: stopped ({why})");
        if self.has_pending() {
            self.start();
        }
    }
}

/// An AUHAL default-output unit with our render callback attached.
struct Unit {
    au: ffi::AudioUnit,
    rate: f64,
    initialized: bool,
}

impl Unit {
    fn new(state: *mut c_void) -> Result<Unit, ffi::OSStatus> {
        let desc = ffi::AudioComponentDescription {
            componentType: ffi::kAudioUnitType_Output,
            componentSubType: ffi::kAudioUnitSubType_DefaultOutput,
            componentManufacturer: ffi::kAudioUnitManufacturer_Apple,
            ..Default::default()
        };
        // SAFETY: valid description pointer; a null result is handled.
        let component = unsafe { ffi::AudioComponentFindNext(std::ptr::null_mut(), &desc) };
        if component.is_null() {
            return Err(-1);
        }
        let mut au = std::ptr::null_mut();
        // SAFETY: `au` is a valid out-pointer; the instance is disposed in `Drop`.
        check(unsafe { ffi::AudioComponentInstanceNew(component, &mut au) })?;
        let unit = Unit { au, rate: 0.0, initialized: false };
        let callback = ffi::AURenderCallbackStruct { inputProc: render, inputProcRefCon: state };
        unit.set(ffi::kAudioUnitProperty_SetRenderCallback, ffi::kAudioUnitScope_Input, &callback)?;
        Ok(unit)
    }

    /// Matches our input format to the device's current rate (re-initialising
    /// only when it changed) and requests a small hardware buffer.
    fn configure(&mut self) -> Result<f64, ffi::OSStatus> {
        let mut hw = ffi::AudioStreamBasicDescription::default();
        let mut size = std::mem::size_of_val(&hw) as u32;
        // SAFETY: `hw` is a valid out-pointer of `size` bytes.
        check(unsafe {
            ffi::AudioUnitGetProperty(
                self.au,
                ffi::kAudioUnitProperty_StreamFormat,
                ffi::kAudioUnitScope_Output,
                0,
                (&raw mut hw).cast(),
                &mut size,
            )
        })?;
        let rate = if hw.mSampleRate > 0.0 { hw.mSampleRate } else { 48_000.0 };
        if !self.initialized || rate != self.rate {
            if self.initialized {
                // SAFETY: the unit is stopped and initialised.
                unsafe { ffi::AudioUnitUninitialize(self.au) };
                self.initialized = false;
            }
            let format = ffi::AudioStreamBasicDescription {
                mSampleRate: rate,
                mFormatID: ffi::kAudioFormatLinearPCM,
                mFormatFlags: ffi::kAudioFormatFlagIsFloat
                    | ffi::kAudioFormatFlagIsPacked
                    | ffi::kAudioFormatFlagIsNonInterleaved,
                mBytesPerPacket: 4,
                mFramesPerPacket: 1,
                mBytesPerFrame: 4,
                mChannelsPerFrame: 2,
                mBitsPerChannel: 32,
                mReserved: 0,
            };
            self.set(ffi::kAudioUnitProperty_StreamFormat, ffi::kAudioUnitScope_Input, &format)?;
            self.set(ffi::kAudioUnitProperty_MaximumFramesPerSlice, ffi::kAudioUnitScope_Global, &MAX_FRAMES_PER_SLICE)?;
            // SAFETY: the unit is configured and uninitialised.
            check(unsafe { ffi::AudioUnitInitialize(self.au) })?;
            self.initialized = true;
            self.rate = rate;
        }
        // Best effort: the device may refuse or round the buffer size.
        let _ = self.set(ffi::kAudioDevicePropertyBufferFrameSize, ffi::kAudioUnitScope_Global, &BUFFER_FRAMES);
        Ok(rate)
    }

    fn start(&self) -> ffi::OSStatus {
        // SAFETY: the unit is initialised.
        unsafe { ffi::AudioOutputUnitStart(self.au) }
    }

    fn stop(&self) {
        // SAFETY: stopping is valid in any state; it waits for the current render cycle.
        unsafe { ffi::AudioOutputUnitStop(self.au) };
    }

    fn is_running(&self) -> bool {
        let mut running: u32 = 0;
        let mut size = std::mem::size_of_val(&running) as u32;
        // SAFETY: `running` is a valid out-pointer of `size` bytes.
        let status = unsafe {
            ffi::AudioUnitGetProperty(
                self.au,
                ffi::kAudioOutputUnitProperty_IsRunning,
                ffi::kAudioUnitScope_Global,
                0,
                (&raw mut running).cast(),
                &mut size,
            )
        };
        status == 0 && running != 0
    }

    fn set<T>(&self, id: u32, scope: u32, value: &T) -> Result<(), ffi::OSStatus> {
        // SAFETY: `value` points to a live `T` of exactly the size passed.
        check(unsafe {
            ffi::AudioUnitSetProperty(self.au, id, scope, 0, (value as *const T).cast(), std::mem::size_of::<T>() as u32)
        })
    }
}

impl Drop for Unit {
    fn drop(&mut self) {
        // SAFETY: tearing down the instance we created.
        unsafe {
            ffi::AudioOutputUnitStop(self.au);
            if self.initialized {
                ffi::AudioUnitUninitialize(self.au);
            }
            ffi::AudioComponentInstanceDispose(self.au);
        }
    }
}

fn check(status: ffi::OSStatus) -> Result<(), ffi::OSStatus> {
    if status == 0 { Ok(()) } else { Err(status) }
}

/// The render callback. Real-time: no allocation, locks, logging or Objective-C.
unsafe extern "C" fn render(
    ref_con: *mut c_void,
    flags: *mut u32,
    _time: *const ffi::AudioTimeStamp,
    _bus: u32,
    frames: u32,
    data: *mut ffi::AudioBufferList,
) -> ffi::OSStatus {
    let started = Instant::now();
    // SAFETY: CoreAudio hands back the RenderState pointer we registered, and only this
    // thread touches it while the unit runs.
    let state = unsafe { &mut *ref_con.cast::<RenderState>() };
    let mut fresh = false;
    for input in &mut state.inputs {
        while let Some(play) = input.pop() {
            state.mixer.start(&play);
            fresh = true;
        }
    }
    // SAFETY: CoreAudio provides `mNumberBuffers` valid buffers.
    let buffers = unsafe {
        std::slice::from_raw_parts_mut(
            (&raw mut (*data).mBuffers).cast::<ffi::AudioBuffer>(),
            (*data).mNumberBuffers as usize,
        )
    };
    let n = frames as usize;
    let shared = state.shared;
    let sounding = match buffers {
        [left, right, ..] => {
            // SAFETY: each buffer holds at least `frames` f32 samples for our format.
            let (left, right) = unsafe { (channel(left, n), channel(right, n)) };
            state.mixer.render(left, right, shared.master_gain())
        }
        [mono] => {
            // SAFETY: as above.
            unsafe { channel(mono, n) }.fill(0.0);
            false
        }
        [] => false,
    };
    if sounding || fresh {
        shared.silent_frames.store(0, Ordering::Relaxed);
    } else {
        shared.silent_frames.fetch_add(n as u64, Ordering::Relaxed);
        // SAFETY: `flags` is a valid in/out pointer for the duration of the call.
        unsafe { *flags |= ffi::kAudioUnitRenderAction_OutputIsSilence };
    }
    shared.stats.callbacks.fetch_add(1, Ordering::Relaxed);
    shared.stats.max_render_ns.fetch_max(started.elapsed().as_nanos() as u64, Ordering::Relaxed);
    0
}

/// # Safety
/// `buffer.mData` must point to at least `min(frames, mDataByteSize / 4)` f32 samples
/// that nothing else accesses during the returned borrow.
unsafe fn channel<'a>(buffer: &mut ffi::AudioBuffer, frames: usize) -> &'a mut [f32] {
    let len = (buffer.mDataByteSize as usize / 4).min(frames);
    // SAFETY: guaranteed by the caller.
    unsafe { std::slice::from_raw_parts_mut(buffer.mData.cast::<f32>(), len) }
}
```

- [ ] **Step 3: Add the runtime and diagnostics**

**File:** `crates/rustyvibes/src/runtime.rs`
```rust
//! Wires the soundpack library, the engine and audio output together.

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use rvpack::keys::code;

use crate::audio;
use crate::engine::{Shared, Voicer, ring::ring};
use crate::log::log;
use crate::packs::{self, Library};
use crate::settings::Settings;

/// Capacity of each command ring (one ring per producer thread).
const RING_CAPACITY: usize = 256;

/// Keys and start times (ms) of the preview played when a pack is picked.
const FLOURISH: [(u8, u32); 6] = [
    (code::R, 0),
    (code::U, 105),
    (code::S, 190),
    (code::T, 290),
    (code::SPACE, 400),
    (code::RETURN, 560),
];

pub struct Runtime {
    pub shared: &'static Shared,
    pub library: &'static Library,
    /// Producer for previews (main thread).
    pub preview: Voicer,
    /// Producer for real key presses, owned by the input thread once it starts.
    input: Option<Voicer>,
}

impl Runtime {
    pub fn start(settings: &Settings) -> Runtime {
        let library: &'static Library = Box::leak(Box::new(Library::load_dir(&packs::default_dir())));
        let pack = library.resolve(settings.pack.as_deref());
        library.make_resident(pack);
        let shared: &'static Shared = Box::leak(Box::new(Shared::new(settings, pack)));
        let (input_tx, input_rx) = ring(RING_CAPACITY);
        let (preview_tx, preview_rx) = ring(RING_CAPACITY);
        let kick = audio::spawn(shared, vec![input_rx, preview_rx]);
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() | 1);
        let preview = Voicer::new(shared, library, preview_tx, kick.clone(), seed);
        let input = Voicer::new(shared, library, input_tx, kick, seed.rotate_left(16) ^ 0x9E37_79B9);
        log!(
            "runtime: {} packs, active {:?}",
            library.len(),
            library.get(pack).map(|p| p.meta.id.as_str())
        );
        Runtime { shared, library, preview, input: Some(input) }
    }

    pub fn active_pack(&self) -> usize {
        self.shared.pack.load(Ordering::Relaxed)
    }

    pub fn select_pack(&self, index: usize) {
        if index < self.library.len() {
            self.shared.pack.store(index, Ordering::Relaxed);
            self.library.make_resident(index);
        }
    }

    /// A short typing flourish ("rust ⏎") in `pack`.
    pub fn preview_flourish(&mut self, pack: usize) {
        let release = self.shared.release_sounds.load(Ordering::Relaxed);
        for (key, at_ms) in FLOURISH {
            self.preview.queue(pack, key, true, at_ms * 1_000);
            if release {
                self.preview.queue(pack, key, false, (at_ms + 70) * 1_000);
            }
        }
    }

    /// One keystroke in the active pack (volume slider feedback).
    pub fn preview_click(&mut self) {
        let pack = self.active_pack();
        self.preview.queue(pack, code::J, true, 0);
    }

    /// Takes the input voicer; the input thread owns it once started.
    pub fn take_input(&mut self) -> Option<Voicer> {
        self.input.take()
    }

    /// Gives the input voicer back after a failed start.
    pub fn return_input(&mut self, voicer: Voicer) {
        self.input = Some(voicer);
    }
}
```

**File:** `crates/rustyvibes/src/diagnostics.rs`
```rust
//! Command-line diagnostics for development and support.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::audio;
use crate::engine::{Play, mixer::Mixer};
use crate::runtime::Runtime;
use crate::settings::Settings;

/// `--selftest [--audible]`: plays every pack's preview through the real audio
/// path (silently unless `--audible`), then checks the unit stops when idle.
pub fn selftest(args: &[String]) -> Result<(), String> {
    let audible = args.iter().any(|a| a == "--audible");
    let settings = Settings { volume: if audible { 0.35 } else { 0.0 }, ..Settings::default() };
    let mut rt = Runtime::start(&settings);
    if rt.library.is_empty() {
        return Err("no soundpacks found (run `cargo xtask packs` first)".into());
    }
    let shared = rt.shared;
    println!("{} soundpacks", rt.library.len());
    for index in 0..rt.library.len() {
        rt.select_pack(index);
        let before = shared.stats.plays.load(Ordering::Relaxed);
        rt.preview_flourish(index);
        std::thread::sleep(Duration::from_millis(750));
        let queued = shared.stats.plays.load(Ordering::Relaxed) - before;
        let name = rt.library.get(index).map(|p| p.display_name()).unwrap_or_default();
        println!("  {name:<30} {queued:>2} sounds");
    }
    let callbacks = shared.stats.callbacks.load(Ordering::Relaxed);
    println!(
        "audio: {} start(s), {callbacks} render callbacks, worst callback {:.1} µs",
        shared.stats.starts.load(Ordering::Relaxed),
        shared.stats.max_render_ns.load(Ordering::Relaxed) as f64 / 1e3
    );
    if callbacks == 0 {
        return Err("the audio unit never rendered".into());
    }
    let deadline = Instant::now() + audio::IDLE_STOP + Duration::from_secs(3);
    while shared.audio_running.load(Ordering::SeqCst) {
        if Instant::now() > deadline {
            return Err("audio did not stop when idle".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("audio stopped after {} s of silence", audio::IDLE_STOP.as_secs());
    Ok(())
}

/// `--bench`: mixer cost per 128-frame block on the interpolating path.
pub fn bench() {
    let clip: &'static [i16] = Box::leak(
        (0..13_230).map(|i| ((i as f32 * 0.07).sin() * 12_000.0) as i16).collect::<Vec<_>>().into_boxed_slice(),
    );
    for voices in [1usize, 8, 32] {
        let mut mixer = Mixer::new(48_000.0);
        let (mut left, mut right) = (vec![0.0f32; 128], vec![0.0f32; 128]);
        let blocks = 20_000;
        let started = Instant::now();
        for block in 0..blocks {
            if block % 100 == 0 {
                for v in 0..voices {
                    let pitch = 1.0 + v as f32 * 0.001;
                    mixer.start(&Play { samples: clip, src_rate: 44_100, pitch, gain_l: 0.5, gain_r: 0.5, delay_us: 0 });
                }
            }
            std::hint::black_box(mixer.render(&mut left, &mut right, 0.5));
        }
        let per_block = started.elapsed().as_nanos() as f64 / f64::from(blocks);
        println!(
            "{voices:>2} voices: {per_block:>7.0} ns per 128-frame block ({:.3}% of the 2.67 ms budget)",
            per_block / 2_666_667.0 * 100.0
        );
    }
}
```

**File:** `crates/rustyvibes/src/main.rs`
```rust
//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// Input and UI are wired in later tasks.
#![allow(dead_code)]

mod audio;
mod diagnostics;
mod engine;
mod log;
mod packs;
mod runtime;
mod settings;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
        Some("--bench") => {
            diagnostics::bench();
            Ok(())
        }
        _ => {
            eprintln!("Rustyvibes {}: menu bar UI arrives in a later task", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 4: Run tests, then the live audio checks**

Run: `cargo test -p rustyvibes && cargo clippy -p rustyvibes --all-targets -- -D warnings && cargo fmt --check`
Expected: 33 tests pass, clean.

Run: `RUSTYVIBES_LOG=1 cargo run --release -p rustyvibes -- --selftest`
Expected: `21 soundpacks`, one line per pack with `6 sounds` (Mechvibes packs) or `12 sounds` (kbsim packs, press + release), an `audio: started at 48000 Hz in …ms` log line, `worst callback` well under 100 µs, then `audio stopped after 8 s of silence` and exit code 0.

Run: `cargo run --release -p rustyvibes -- --bench`
Expected: three lines; 32 voices should cost well under 1% of the budget.

- [ ] **Step 5: Commit**

```bash
git add crates/rustyvibes
git commit -m "feat(audio): on-demand AUHAL output with idle stop, runtime and diagnostics

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Global key capture (event tap) and `--tap-test`

**Files:**
- Create: `crates/rustyvibes/src/input/mod.rs`, `crates/rustyvibes/src/input/ffi.rs`, `crates/rustyvibes/src/input/keystate.rs`
- Modify: `crates/rustyvibes/src/main.rs`, `crates/rustyvibes/src/diagnostics.rs`
- Test: unit tests in `keystate.rs`; live `--tap-test`

**Interfaces:**
- Consumes: Task 6–7 `engine::Voicer`, `runtime::Runtime`.
- Produces (used by Tasks 9–10):
  - `input::{has_permission() -> bool, request_permission() -> bool, start(Voicer) -> Result<(), Voicer>, post_test_shift()}`
  - `input::keystate::{KeyState, Transition { keycode: u8, down: bool }}`
  - `diagnostics::tap_test() -> Result<(), String>`

- [ ] **Step 1: Write the failing key-state tests**

**File:** `crates/rustyvibes/src/input/keystate.rs` (test module; implementation goes above it in Step 3)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: u16 = 0x38;
    const RIGHT_SHIFT: u16 = 0x3C;
    const CAPS: u16 = 0x39;
    const FN: u16 = 0x3F;

    fn down(keycode: u8) -> Option<Transition> {
        Some(Transition { keycode, down: true })
    }

    fn up(keycode: u8) -> Option<Transition> {
        Some(Transition { keycode, down: false })
    }

    #[test]
    fn key_press_and_release() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(0x00, false), down(0x00));
        assert_eq!(keys.key_up(0x00), up(0x00));
    }

    #[test]
    fn autorepeat_and_duplicate_downs_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(0x00, false), down(0x00));
        assert_eq!(keys.key_down(0x00, true), None, "auto-repeat");
        assert_eq!(keys.key_down(0x00, false), None, "already down");
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_up(0x00), None);
    }

    #[test]
    fn out_of_range_keycodes_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(128, false), None);
        assert_eq!(keys.key_down(u16::MAX, false), None);
        assert_eq!(keys.flags_changed(300, FLAG_SHIFT), None);
    }

    #[test]
    fn shift_with_device_bits() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_LSHIFT), down(0x38));
        assert_eq!(keys.flags_changed(SHIFT, 0), up(0x38));
    }

    #[test]
    fn releasing_left_shift_while_right_held() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_LSHIFT), down(0x38));
        assert_eq!(keys.flags_changed(RIGHT_SHIFT, FLAG_SHIFT | DEV_LSHIFT | DEV_RSHIFT), down(0x3C));
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_RSHIFT), up(0x38));
        assert_eq!(keys.flags_changed(RIGHT_SHIFT, 0), up(0x3C));
    }

    #[test]
    fn modifiers_without_device_bits_toggle() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT), down(0x38));
        assert_eq!(keys.flags_changed(SHIFT, 0), up(0x38));
        assert_eq!(keys.flags_changed(0x37, FLAG_COMMAND), down(0x37));
        assert_eq!(keys.flags_changed(0x37, 0), up(0x37));
    }

    #[test]
    fn caps_lock_clicks_on_every_change() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(CAPS, FLAG_ALPHA_SHIFT), down(0x39));
        assert_eq!(keys.flags_changed(CAPS, 0), down(0x39));
    }

    #[test]
    fn fn_key_follows_its_flag() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(FN, FLAG_FN), down(0x3F));
        assert_eq!(keys.flags_changed(FN, FLAG_FN), None);
        assert_eq!(keys.flags_changed(FN, 0), up(0x3F));
    }

    #[test]
    fn non_modifier_flag_changes_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(0x00, FLAG_SHIFT), None);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Create `crates/rustyvibes/src/input/mod.rs` containing only `pub mod keystate;` and add `mod input;` to `main.rs` temporarily, then:

Run: `cargo test -p rustyvibes keystate`
Expected: compile errors — `KeyState`, `Transition`, `FLAG_SHIFT`, `DEV_LSHIFT` not found.

- [ ] **Step 3: Implement key state, FFI and the event tap**

**File:** `crates/rustyvibes/src/input/keystate.rs` (implementation part)
```rust
//! Turns raw key events into clean press/release transitions: drops
//! auto-repeat and duplicates, and decodes modifier `flagsChanged` events.

/// A physical key going down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub keycode: u8,
    pub down: bool,
}

// Device-independent modifier flags (CGEventFlags).
pub const FLAG_ALPHA_SHIFT: u64 = 0x0001_0000;
pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;
pub const FLAG_FN: u64 = 0x0080_0000;

// Device-dependent bits (IOKit NX_DEVICE*KEYMASK) tell left from right.
pub const DEV_LCTL: u64 = 0x0000_0001;
pub const DEV_LSHIFT: u64 = 0x0000_0002;
pub const DEV_RSHIFT: u64 = 0x0000_0004;
pub const DEV_LCMD: u64 = 0x0000_0008;
pub const DEV_RCMD: u64 = 0x0000_0010;
pub const DEV_LALT: u64 = 0x0000_0020;
pub const DEV_RALT: u64 = 0x0000_0040;
pub const DEV_RCTL: u64 = 0x0000_2000;
const DEV_ALL: u64 = DEV_LCTL | DEV_LSHIFT | DEV_RSHIFT | DEV_LCMD | DEV_RCMD | DEV_LALT | DEV_RALT | DEV_RCTL;

const CAPS_LOCK: u8 = 0x39;
const FUNCTION: u8 = 0x3F;

/// Which of the 128 keys are currently down.
#[derive(Clone, Debug, Default)]
pub struct KeyState {
    pressed: [u64; 2],
}

impl KeyState {
    pub fn is_pressed(&self, keycode: u8) -> bool {
        self.pressed[usize::from(keycode >> 6) & 1] & (1 << (keycode & 63)) != 0
    }

    fn set(&mut self, keycode: u8, down: bool) {
        let word = &mut self.pressed[usize::from(keycode >> 6) & 1];
        if down {
            *word |= 1 << (keycode & 63);
        } else {
            *word &= !(1 << (keycode & 63));
        }
    }

    pub fn key_down(&mut self, keycode: u16, autorepeat: bool) -> Option<Transition> {
        let keycode = valid(keycode)?;
        if autorepeat || self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, true);
        Some(Transition { keycode, down: true })
    }

    pub fn key_up(&mut self, keycode: u16) -> Option<Transition> {
        let keycode = valid(keycode)?;
        if !self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, false);
        Some(Transition { keycode, down: false })
    }

    /// Decodes a `flagsChanged` event for a modifier key.
    pub fn flags_changed(&mut self, keycode: u16, flags: u64) -> Option<Transition> {
        let keycode = valid(keycode)?;
        let down = match keycode {
            // macOS reports only the toggle, not the physical release.
            CAPS_LOCK => return Some(Transition { keycode, down: true }),
            FUNCTION => flags & FLAG_FN != 0,
            _ => {
                let bit = device_bit(keycode)?;
                if flags & DEV_ALL != 0 { flags & bit != 0 } else { !self.is_pressed(keycode) }
            }
        };
        if down == self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, down);
        Some(Transition { keycode, down })
    }
}

fn valid(keycode: u16) -> Option<u8> {
    u8::try_from(keycode).ok().filter(|k| *k < 128)
}

fn device_bit(keycode: u8) -> Option<u64> {
    Some(match keycode {
        0x38 => DEV_LSHIFT,
        0x3C => DEV_RSHIFT,
        0x3B => DEV_LCTL,
        0x3E => DEV_RCTL,
        0x3A => DEV_LALT,
        0x3D => DEV_RALT,
        0x37 => DEV_LCMD,
        0x36 => DEV_RCMD,
        _ => return None,
    })
}
```

**File:** `crates/rustyvibes/src/input/ffi.rs`
```rust
//! Minimal CoreGraphics event-tap and CoreFoundation run-loop FFI.

#![allow(non_upper_case_globals)]

use std::ffi::c_void;

pub type CFMachPortRef = *mut c_void;
pub type CFRunLoopSourceRef = *mut c_void;
pub type CFRunLoopRef = *mut c_void;
pub type CFStringRef = *const c_void;
pub type CGEventRef = *mut c_void;
pub type CGEventSourceRef = *mut c_void;
pub type CGEventTapProxy = *mut c_void;
pub type CGEventTapCallBack =
    unsafe extern "C" fn(CGEventTapProxy, u32, CGEventRef, *mut c_void) -> CGEventRef;

pub const kCGHIDEventTap: u32 = 0;
pub const kCGSessionEventTap: u32 = 1;
pub const kCGHeadInsertEventTap: u32 = 0;
pub const kCGEventTapOptionListenOnly: u32 = 1;
pub const kCGEventKeyDown: u32 = 10;
pub const kCGEventKeyUp: u32 = 11;
pub const kCGEventFlagsChanged: u32 = 12;
pub const kCGEventTapDisabledByTimeout: u32 = 0xFFFF_FFFE;
pub const kCGEventTapDisabledByUserInput: u32 = 0xFFFF_FFFF;
pub const kCGKeyboardEventAutorepeat: u32 = 8;
pub const kCGKeyboardEventKeycode: u32 = 9;
pub const kCGEventSourceUserData: u32 = 42;
pub const kCGEventSourceStatePrivate: i32 = -1;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    pub fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    pub fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
    pub fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    pub fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    pub fn CGEventGetFlags(event: CGEventRef) -> u64;
    pub fn CGEventSetFlags(event: CGEventRef, flags: u64);
    pub fn CGEventCreateKeyboardEvent(source: CGEventSourceRef, keycode: u16, key_down: bool) -> CGEventRef;
    pub fn CGEventSourceCreate(state: i32) -> CGEventSourceRef;
    pub fn CGEventPost(tap: u32, event: CGEventRef);
    pub fn CGPreflightListenEventAccess() -> bool;
    pub fn CGRequestListenEventAccess() -> bool;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    pub static kCFRunLoopCommonModes: CFStringRef;
    pub fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: CFMachPortRef,
        order: isize,
    ) -> CFRunLoopSourceRef;
    pub fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    pub fn CFRunLoopAddSource(run_loop: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    pub fn CFRunLoopRun();
    pub fn CFRelease(object: *const c_void);
}
```

**File:** `crates/rustyvibes/src/input/mod.rs`
```rust
//! Global key capture: a listen-only `CGEventTap` on its own high-priority
//! thread. It needs only the Input Monitoring permission, sees which key moved
//! (never text), and costs nothing while no keys are pressed.

mod ffi;
pub mod keystate;

use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::engine::Voicer;
use crate::log::log;
use keystate::KeyState;

/// Marks events injected by `post_test_shift`.
const TEST_MARKER: i64 = 0x5256;

/// Whether this process may observe key events (Input Monitoring).
pub fn has_permission() -> bool {
    // SAFETY: no preconditions.
    unsafe { ffi::CGPreflightListenEventAccess() }
}

/// Asks for Input Monitoring access; macOS shows its prompt only the first time.
pub fn request_permission() -> bool {
    // SAFETY: no preconditions.
    unsafe { ffi::CGRequestListenEventAccess() }
}

struct TapState {
    voicer: Voicer,
    keys: KeyState,
    tap: ffi::CFMachPortRef,
}

/// Starts the event tap on its own thread. Without permission the tap cannot
/// be created and the voicer is handed back so a later attempt can reuse it.
pub fn start(voicer: Voicer) -> Result<(), Voicer> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), Voicer>>();
    std::thread::Builder::new()
        .name("rustyvibes-input".into())
        .spawn(move || {
            // SAFETY: adjusts the QoS of the current thread only.
            unsafe {
                libc::pthread_set_qos_class_self_np(libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE, 0);
            }
            let state = Box::into_raw(Box::new(TapState {
                voicer,
                keys: KeyState::default(),
                tap: std::ptr::null_mut(),
            }));
            let mask = (1u64 << ffi::kCGEventKeyDown)
                | (1u64 << ffi::kCGEventKeyUp)
                | (1u64 << ffi::kCGEventFlagsChanged);
            // SAFETY: `state` outlives the tap: it is freed only if creation fails.
            let tap = unsafe {
                ffi::CGEventTapCreate(
                    ffi::kCGSessionEventTap,
                    ffi::kCGHeadInsertEventTap,
                    ffi::kCGEventTapOptionListenOnly,
                    mask,
                    tap_callback,
                    state.cast(),
                )
            };
            if tap.is_null() {
                // SAFETY: no tap exists, so nothing else references `state`.
                let TapState { voicer, .. } = *unsafe { Box::from_raw(state) };
                let _ = tx.send(Err(voicer));
                return;
            }
            // SAFETY: `tap` is a valid mach port; the source and tap live forever.
            unsafe {
                (*state).tap = tap;
                let source = ffi::CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                ffi::CFRunLoopAddSource(ffi::CFRunLoopGetCurrent(), source, ffi::kCFRunLoopCommonModes);
                ffi::CGEventTapEnable(tap, true);
            }
            let _ = tx.send(Ok(()));
            log!("input: event tap running");
            // SAFETY: runs this thread's run loop forever.
            unsafe { ffi::CFRunLoopRun() };
        })
        .expect("failed to spawn the input thread");
    rx.recv().expect("the input thread exited before reporting")
}

/// Event-tap callback. No allocation, no locks, no Objective-C.
unsafe extern "C" fn tap_callback(
    _proxy: ffi::CGEventTapProxy,
    kind: u32,
    event: ffi::CGEventRef,
    user_info: *mut c_void,
) -> ffi::CGEventRef {
    let started = Instant::now();
    // SAFETY: `user_info` is the TapState owned by this thread for the life of the tap.
    let state = unsafe { &mut *user_info.cast::<TapState>() };
    // SAFETY (all field reads below): `event` is a valid CGEvent for this callback.
    let keycode = || u16::try_from(unsafe { ffi::CGEventGetIntegerValueField(event, ffi::kCGKeyboardEventKeycode) }).unwrap_or(u16::MAX);
    let transition = match kind {
        ffi::kCGEventTapDisabledByTimeout | ffi::kCGEventTapDisabledByUserInput => {
            // SAFETY: `state.tap` is the live tap.
            unsafe { ffi::CGEventTapEnable(state.tap, true) };
            None
        }
        ffi::kCGEventKeyDown => {
            let repeat = unsafe { ffi::CGEventGetIntegerValueField(event, ffi::kCGKeyboardEventAutorepeat) };
            state.keys.key_down(keycode(), repeat != 0)
        }
        ffi::kCGEventKeyUp => state.keys.key_up(keycode()),
        ffi::kCGEventFlagsChanged => {
            let flags = unsafe { ffi::CGEventGetFlags(event) };
            state.keys.flags_changed(keycode(), flags)
        }
        _ => None,
    };
    if let Some(t) = transition {
        state.voicer.key(t.keycode, t.down);
        let elapsed = started.elapsed().as_nanos() as u64;
        state.voicer.shared().stats.max_tap_ns.fetch_max(elapsed, Ordering::Relaxed);
    }
    event
}

/// Injects a Shift press and release (harmless in every app), tagged so they are
/// recognisable. Used by `--tap-test`; requires the Accessibility permission.
pub fn post_test_shift() {
    // SAFETY: creates, posts and releases CoreGraphics objects we own.
    unsafe {
        let source = ffi::CGEventSourceCreate(ffi::kCGEventSourceStatePrivate);
        for (down, flags) in [(true, 0x0002_0102u64), (false, 0x0000_0100u64)] {
            let event = ffi::CGEventCreateKeyboardEvent(source, 0x38, down);
            ffi::CGEventSetFlags(event, flags);
            ffi::CGEventSetIntegerValueField(event, ffi::kCGEventSourceUserData, TEST_MARKER);
            ffi::CGEventPost(ffi::kCGHIDEventTap, event);
            ffi::CFRelease(event.cast_const());
            std::thread::sleep(Duration::from_millis(80));
        }
        if !source.is_null() {
            ffi::CFRelease(source.cast_const());
        }
    }
}
```

**File:** `crates/rustyvibes/src/diagnostics.rs` (append this function)
```rust
/// `--tap-test`: starts the event tap, injects harmless Shift events and checks
/// they reach the engine. Needs Input Monitoring and Accessibility for this process.
pub fn tap_test() -> Result<(), String> {
    if !crate::input::has_permission() {
        return Err("this process lacks the Input Monitoring permission".into());
    }
    let settings = Settings { volume: 0.0, ..Settings::default() };
    let mut rt = Runtime::start(&settings);
    if rt.library.is_empty() {
        return Err("no soundpacks found (run `cargo xtask packs` first)".into());
    }
    if let Some(index) = rt.library.iter().position(|p| p.has_release) {
        rt.select_pack(index);
    }
    let voicer = rt.take_input().ok_or("input already started")?;
    crate::input::start(voicer).map_err(|_| "could not create the event tap".to_string())?;
    let shared = rt.shared;
    std::thread::sleep(Duration::from_millis(100));
    crate::input::post_test_shift();
    std::thread::sleep(Duration::from_millis(300));
    let events = shared.stats.key_events.load(Ordering::Relaxed);
    let plays = shared.stats.plays.load(Ordering::Relaxed);
    println!(
        "tap: {events} key transitions, {plays} sounds queued, worst tap callback {:.1} µs, audio starts {}",
        shared.stats.max_tap_ns.load(Ordering::Relaxed) as f64 / 1e3,
        shared.stats.starts.load(Ordering::Relaxed)
    );
    if events < 2 || plays < 2 {
        return Err("the injected Shift press/release did not reach the engine".into());
    }
    Ok(())
}
```

**File:** `crates/rustyvibes/src/main.rs`
```rust
//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// The UI is wired in a later task.
#![allow(dead_code)]

mod audio;
mod diagnostics;
mod engine;
mod input;
mod log;
mod packs;
mod runtime;
mod settings;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
        Some("--tap-test") => diagnostics::tap_test(),
        Some("--bench") => {
            diagnostics::bench();
            Ok(())
        }
        _ => {
            eprintln!("Rustyvibes {}: menu bar UI arrives in a later task", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

- [ ] **Step 4: Run tests, then the live tap check**

Run: `cargo test -p rustyvibes && cargo clippy -p rustyvibes --all-targets -- -D warnings && cargo fmt --check`
Expected: 43 tests pass, clean.

Run (from a terminal that has Input Monitoring + Accessibility): `RUSTYVIBES_LOG=1 cargo run --release -p rustyvibes -- --tap-test`
Expected: `input: event tap running`, then `tap: 2 key transitions, 2 sounds queued, worst tap callback <20 µs, audio starts 1`, exit code 0.

- [ ] **Step 5: Commit**

```bash
git add crates/rustyvibes
git commit -m "feat(input): listen-only event tap with modifier-aware key state

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---
### Task 9: Settings persistence and the menu building blocks

**Files:**
- Modify: `crates/rustyvibes/Cargo.toml` (objc2 crates), `crates/rustyvibes/src/settings.rs` (add `Store`)
- Create: `crates/rustyvibes/src/ui/icons.rs`, `crates/rustyvibes/src/ui/views.rs`, `crates/rustyvibes/src/ui/menu.rs`, `crates/rustyvibes/src/ui/mod.rs`
- Modify: `crates/rustyvibes/src/main.rs` (`mod ui;`)
- Test: `settings.rs` store round trip, `icons.rs` colour parsing (visual checks happen in Task 10)

**Interfaces:**
- Consumes: Task 6 `packs::{Library, LoadedPack}`, `settings::Settings`.
- Produces (used by Task 10):
  - `settings::{Store, KEY_ENABLED, KEY_PACK, KEY_VOLUME, KEY_RELEASE_SOUNDS, KEY_VARIATION, KEY_SPATIAL}`; `Store::{standard(), load() -> Settings, set_bool(&str, bool), set_float(&str, f32), set_string(&str, &str)}`
  - `ui::icons::{StatusIcon::{Normal, Attention}, status(StatusIcon) -> Retained<NSImage>, keycap(&str) -> Retained<NSImage>, symbol(&str, &str) -> Retained<NSImage>, parse_hex(&str) -> (f64, f64, f64), rect(f64, f64, f64, f64) -> NSRect}`
  - `ui::views::{MENU_WIDTH, header(MainThreadMarker, &AnyObject, bool) -> Retained<NSView>, volume(MainThreadMarker, &AnyObject, f32) -> Retained<NSView>}`
  - `ui::menu::{Menu, control_state(bool)}`; `Menu::build(MainThreadMarker, &AnyObject, &Library, &Settings, usize, bool) -> Menu`, `Menu.menu: Retained<NSMenu>`, `show_active_pack(&Library, usize)`, `show_permission_needed(bool)`
  - Actions the delegate must implement: `toggleEnabled:`, `selectPack:`, `volumeChanged:`, `toggleReleaseSounds:`, `toggleVariation:`, `toggleSpatial:`, `toggleLaunchAtLogin:`, `showPermissionHelp:`, `showAbout:`

- [ ] **Step 1: Add the objc2 dependencies**

**File:** `crates/rustyvibes/Cargo.toml`
```toml
[package]
name = "rustyvibes"
description = "Mechanical keyboard sounds for every key press — a native macOS menu bar app"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
publish = false

[dependencies]
block2 = "0.6.2"
libc = "0.2"
objc2 = "0.6.4"
objc2-app-kit = "0.3.2"
objc2-foundation = "0.3.2"
rvpack = { path = "../rvpack" }

[lints]
workspace = true
```

- [ ] **Step 2: Write the failing tests**

**File:** `crates/rustyvibes/src/settings.rs` (append)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_round_trips_and_clamps() {
        let suite = format!("io.github.kb24x7.rustyvibes.tests.{}", std::process::id());
        let store = Store::suite(&suite);
        assert_eq!(store.load(), Settings::default());
        store.set_bool(KEY_ENABLED, false);
        store.set_float(KEY_VOLUME, 0.25);
        store.set_string(KEY_PACK, "holy-panda");
        store.set_bool(KEY_SPATIAL, false);
        assert_eq!(
            store.load(),
            Settings {
                enabled: false,
                pack: Some("holy-panda".into()),
                volume: 0.25,
                spatial: false,
                ..Settings::default()
            }
        );
        store.set_float(KEY_VOLUME, 7.0);
        assert_eq!(store.load().volume, 1.0, "out-of-range values are clamped");
        Store::clear_suite(&suite);
    }
}
```

**File:** `crates/rustyvibes/src/ui/icons.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colours() {
        assert_eq!(parse_hex("#FF0000"), (1.0, 0.0, 0.0));
        let (r, g, b) = parse_hex("#336699");
        assert!((r - 0.2).abs() < 1e-9 && (g - 0.4).abs() < 1e-9 && (b - 0.6).abs() < 1e-9);
        assert_eq!(parse_hex("red"), (0.5, 0.5, 0.5));
        assert_eq!(parse_hex("#12345"), (0.5, 0.5, 0.5));
        assert_eq!(parse_hex("#GG0000"), (0.5, 0.5, 0.5));
    }
}
```

- [ ] **Step 3: Run the tests to verify they fail**

Add `mod ui;` to `main.rs` and create `crates/rustyvibes/src/ui/mod.rs` with `pub mod icons;`, then:

Run: `cargo test -p rustyvibes store_round_trips parses_hex`
Expected: compile errors — `Store`, `KEY_*`, `parse_hex` not found.

- [ ] **Step 4: Implement the store, icons, views and menu**

**File:** `crates/rustyvibes/src/settings.rs` (full file; the Step 2 test module stays at the bottom)
```rust
//! User settings and their persistence in `NSUserDefaults`.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSString, NSUserDefaults};

pub const KEY_ENABLED: &str = "Enabled";
pub const KEY_PACK: &str = "Pack";
pub const KEY_VOLUME: &str = "Volume";
pub const KEY_RELEASE_SOUNDS: &str = "ReleaseSounds";
pub const KEY_VARIATION: &str = "Variation";
pub const KEY_SPATIAL: &str = "Spatial";

/// User-facing settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// Saved soundpack id; `None` until the user picks one.
    pub pack: Option<String>,
    pub volume: f32,
    pub release_sounds: bool,
    pub variation: bool,
    pub spatial: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            pack: None,
            volume: 0.6,
            release_sounds: true,
            variation: true,
            spatial: true,
        }
    }
}

/// Reads and writes settings in `NSUserDefaults` (thread-safe, cached in memory).
pub struct Store {
    defaults: Retained<NSUserDefaults>,
}

impl Store {
    pub fn standard() -> Store {
        Store { defaults: NSUserDefaults::standardUserDefaults() }
    }

    /// A separate defaults domain, so tests never touch the real settings.
    #[cfg(test)]
    pub fn suite(name: &str) -> Store {
        let defaults =
            NSUserDefaults::initWithSuiteName(NSUserDefaults::alloc(), Some(&NSString::from_str(name)))
                .expect("valid suite name");
        Store { defaults }
    }

    #[cfg(test)]
    pub fn clear_suite(name: &str) {
        NSUserDefaults::standardUserDefaults().removePersistentDomainForName(&NSString::from_str(name));
    }

    /// Stored settings, with defaults for anything never saved.
    pub fn load(&self) -> Settings {
        let d = Settings::default();
        Settings {
            enabled: self.bool(KEY_ENABLED).unwrap_or(d.enabled),
            pack: self.defaults.stringForKey(&NSString::from_str(KEY_PACK)).map(|s| s.to_string()),
            volume: self.float(KEY_VOLUME).map_or(d.volume, |v| v.clamp(0.0, 1.0)),
            release_sounds: self.bool(KEY_RELEASE_SOUNDS).unwrap_or(d.release_sounds),
            variation: self.bool(KEY_VARIATION).unwrap_or(d.variation),
            spatial: self.bool(KEY_SPATIAL).unwrap_or(d.spatial),
        }
    }

    pub fn set_bool(&self, key: &str, value: bool) {
        self.defaults.setBool_forKey(value, &NSString::from_str(key));
    }

    pub fn set_float(&self, key: &str, value: f32) {
        self.defaults.setFloat_forKey(value, &NSString::from_str(key));
    }

    pub fn set_string(&self, key: &str, value: &str) {
        let value = NSString::from_str(value);
        let object: &AnyObject = &value;
        // SAFETY: NSString is a property-list type, as NSUserDefaults requires.
        unsafe { self.defaults.setObject_forKey(Some(object), &NSString::from_str(key)) };
    }

    fn has(&self, key: &str) -> bool {
        self.defaults.objectForKey(&NSString::from_str(key)).is_some()
    }

    fn bool(&self, key: &str) -> Option<bool> {
        self.has(key).then(|| self.defaults.boolForKey(&NSString::from_str(key)))
    }

    fn float(&self, key: &str) -> Option<f32> {
        self.has(key).then(|| self.defaults.floatForKey(&NSString::from_str(key)))
    }
}
```

**File:** `crates/rustyvibes/src/ui/icons.rs` (implementation part)
```rust
//! Icons drawn in code: the template status-bar glyph and the tinted keycap
//! glyphs of the soundpack menu. Vector drawing keeps them sharp at any scale.

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{NSBezierPath, NSColor, NSImage};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// States of the status-bar icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusIcon {
    /// A keycap.
    Normal,
    /// A keycap holding an exclamation mark: keyboard access is missing.
    Attention,
}

/// An 18 pt template image for the status bar (tinted by the system).
pub fn status(kind: StatusIcon) -> Retained<NSImage> {
    let block = RcBlock::new(move |_bounds: NSRect| -> Bool {
        let ink = NSColor::blackColor();
        ink.setStroke();
        ink.setFill();
        let body = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(2.25, 2.75, 13.5, 12.0), 3.2, 3.2);
        body.setLineWidth(1.5);
        body.stroke();
        match kind {
            StatusIcon::Normal => {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(5.25, 6.25, 7.5, 6.0), 1.6, 1.6)
                    .fill();
            }
            StatusIcon::Attention => {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(8.2, 7.4, 1.6, 5.0), 0.8, 0.8)
                    .fill();
                NSBezierPath::bezierPathWithOvalInRect(rect(8.15, 4.6, 1.7, 1.7)).fill();
            }
        }
        Bool::YES
    });
    let image = NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(18.0, 18.0), false, &block);
    image.setTemplate(true);
    image.setAccessibilityDescription(Some(&NSString::from_str("Rustyvibes")));
    image
}

/// A 16 pt keycap glyph tinted with a switch's stem colour (`#RRGGBB`).
pub fn keycap(hex: &str) -> Retained<NSImage> {
    let (r, g, b) = parse_hex(hex);
    let block = RcBlock::new(move |_bounds: NSRect| -> Bool {
        NSColor::colorWithSRGBRed_green_blue_alpha(r * 0.72, g * 0.72, b * 0.72, 1.0).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(1.0, 1.5, 14.0, 13.0), 3.5, 3.5).fill();
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(3.0, 4.5, 10.0, 8.5), 2.2, 2.2).fill();
        NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.3).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(4.0, 10.6, 8.0, 1.5), 0.75, 0.75).fill();
        // A hairline keeps pale caps visible on light menus and dark caps on dark ones.
        NSColor::colorWithSRGBRed_green_blue_alpha(0.5, 0.5, 0.5, 0.35).setStroke();
        let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(rect(1.25, 1.75, 13.5, 12.5), 3.3, 3.3);
        outline.setLineWidth(0.5);
        outline.stroke();
        Bool::YES
    });
    NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(16.0, 16.0), false, &block)
}

/// An SF Symbol, or an empty 16 pt image if the symbol is unavailable.
pub fn symbol(name: &str, description: &str) -> Retained<NSImage> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        Some(&NSString::from_str(description)),
    )
    .unwrap_or_else(|| NSImage::initWithSize(NSImage::alloc(), NSSize::new(16.0, 16.0)))
}

/// Parses `#RRGGBB` into sRGB components in 0…1; malformed input yields mid grey.
pub fn parse_hex(hex: &str) -> (f64, f64, f64) {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    let component = |i: usize| {
        digits.get(i..i + 2).and_then(|s| u8::from_str_radix(s, 16).ok()).map(|v| f64::from(v) / 255.0)
    };
    match (digits.len(), component(0), component(2), component(4)) {
        (6, Some(r), Some(g), Some(b)) => (r, g, b),
        _ => (0.5, 0.5, 0.5),
    }
}

pub fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
```

**File:** `crates/rustyvibes/src/ui/views.rs`
```rust
//! Custom views embedded in the menu: the header with the sounds switch and
//! the volume slider row.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{
    NSColor, NSControlSize, NSFont, NSImageView, NSSlider, NSSwitch, NSTextField, NSView,
};
use objc2_foundation::NSString;

use super::icons::{self, rect};
use super::menu::control_state;

/// Width of the custom menu rows, in points.
pub const MENU_WIDTH: f64 = 280.0;
const INSET: f64 = 14.0;

/// "Rustyvibes" in bold, with the sounds on/off switch at the trailing edge.
pub fn header(mtm: MainThreadMarker, target: &AnyObject, on: bool) -> Retained<NSView> {
    let height = 36.0;
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, MENU_WIDTH, height));
    let title = NSTextField::labelWithString(&NSString::from_str("Rustyvibes"), mtm);
    title.setFont(Some(&NSFont::boldSystemFontOfSize(13.0)));
    title.sizeToFit();
    let size = title.frame().size;
    title.setFrame(rect(INSET, ((height - size.height) / 2.0).round(), size.width, size.height));
    view.addSubview(&title);

    let switch = NSSwitch::new(mtm);
    switch.setControlSize(NSControlSize::Small);
    switch.sizeToFit();
    let size = switch.frame().size;
    switch.setFrame(rect(
        MENU_WIDTH - INSET - size.width,
        ((height - size.height) / 2.0).round(),
        size.width,
        size.height,
    ));
    switch.setState(control_state(on));
    switch.setToolTip(Some(&NSString::from_str("Keyboard sounds on or off")));
    // SAFETY: `target` is the app delegate, which implements `toggleEnabled:`.
    unsafe {
        switch.setTarget(Some(target));
        switch.setAction(Some(sel!(toggleEnabled:)));
    }
    view.addSubview(&switch);
    view
}

/// Speaker glyphs around a continuous 0…1 slider.
pub fn volume(mtm: MainThreadMarker, target: &AnyObject, value: f32) -> Retained<NSView> {
    let height = 30.0;
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, MENU_WIDTH, height));
    let quiet = NSImageView::imageViewWithImage(&icons::symbol("speaker.fill", "Quieter"), mtm);
    let loud = NSImageView::imageViewWithImage(&icons::symbol("speaker.wave.3.fill", "Louder"), mtm);
    for (icon, x) in [(&quiet, INSET), (&loud, MENU_WIDTH - INSET - 18.0)] {
        icon.setFrame(rect(x, (height - 16.0) / 2.0, 18.0, 16.0));
        icon.setContentTintColor(Some(&NSColor::secondaryLabelColor()));
        view.addSubview(icon);
    }
    // SAFETY: `target` is the app delegate, which implements `volumeChanged:`.
    let slider = unsafe {
        NSSlider::sliderWithValue_minValue_maxValue_target_action(
            f64::from(value),
            0.0,
            1.0,
            Some(target),
            Some(sel!(volumeChanged:)),
            mtm,
        )
    };
    let left = INSET + 18.0 + 8.0;
    slider.setFrame(rect(left, (height - 20.0) / 2.0, MENU_WIDTH - 2.0 * left, 20.0));
    slider.setToolTip(Some(&NSString::from_str("Volume")));
    view.addSubview(&slider);
    view
}
```

**File:** `crates/rustyvibes/src/ui/menu.rs`
```rust
//! Builds the status-item menu and keeps it in sync with the settings.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{MainThreadMarker, available, sel};
use objc2_app_kit::{
    NSControlStateValue, NSControlStateValueOff, NSControlStateValueOn, NSFont, NSFontAttributeName,
    NSMenu, NSMenuItem,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};

use super::{icons, views};
use crate::packs::Library;
use crate::settings::Settings;

/// The menu plus the items that change after it is built (the menu retains
/// everything else).
pub struct Menu {
    pub menu: Retained<NSMenu>,
    /// Separator and "Allow Keyboard Access…" row.
    permission: [Retained<NSMenuItem>; 2],
    current_pack: Retained<NSMenuItem>,
    pack_items: Vec<Retained<NSMenuItem>>,
    release_item: Retained<NSMenuItem>,
}

impl Menu {
    pub fn build(
        mtm: MainThreadMarker,
        target: &AnyObject,
        library: &Library,
        settings: &Settings,
        active: usize,
        launch_at_login: bool,
    ) -> Menu {
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);

        let header = NSMenuItem::new(mtm);
        header.setView(Some(&views::header(mtm, target, settings.enabled)));
        menu.addItem(&header);

        let permission_separator = NSMenuItem::separatorItem(mtm);
        let permission_item = item(mtm, "Allow Keyboard Access…", target, sel!(showPermissionHelp:));
        permission_item.setImage(Some(&icons::symbol("exclamationmark.triangle.fill", "Needs permission")));
        menu.addItem(&permission_separator);
        menu.addItem(&permission_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&section_header(mtm, "Soundpack"));
        let submenu = NSMenu::new(mtm);
        submenu.setAutoenablesItems(false);
        let mut pack_items = Vec::with_capacity(library.len());
        let mut category: Option<&str> = None;
        for (index, pack) in library.iter().enumerate() {
            if category != Some(pack.meta.category.as_str()) {
                if category.is_some() {
                    submenu.addItem(&NSMenuItem::separatorItem(mtm));
                }
                submenu.addItem(&section_header(mtm, category_title(&pack.meta.category)));
                category = Some(pack.meta.category.as_str());
            }
            let row = item(mtm, &pack.display_name(), target, sel!(selectPack:));
            row.setImage(Some(&icons::keycap(&pack.meta.color)));
            row.setTag(index as isize);
            submenu.addItem(&row);
            pack_items.push(row);
        }
        if library.is_empty() {
            let none = NSMenuItem::new(mtm);
            none.setTitle(&NSString::from_str("No soundpacks found"));
            none.setEnabled(false);
            submenu.addItem(&none);
        }
        let current_pack = NSMenuItem::new(mtm);
        current_pack.setSubmenu(Some(&submenu));
        menu.addItem(&current_pack);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&section_header(mtm, "Volume"));
        let volume = NSMenuItem::new(mtm);
        volume.setView(Some(&views::volume(mtm, target, settings.volume)));
        menu.addItem(&volume);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let release_item = toggle(
            mtm,
            "Key Release Sounds",
            target,
            sel!(toggleReleaseSounds:),
            settings.release_sounds,
            "Play the upstroke when a key is released (soundpacks recorded with release sounds)",
        );
        menu.addItem(&release_item);
        menu.addItem(&toggle(
            mtm,
            "Natural Variation",
            target,
            sel!(toggleVariation:),
            settings.variation,
            "Vary the pitch and loudness of each keystroke slightly, like a real keyboard",
        ));
        menu.addItem(&toggle(
            mtm,
            "Spatial Stereo",
            target,
            sel!(toggleSpatial:),
            settings.spatial,
            "Place each key's sound where the key sits on the keyboard",
        ));
        menu.addItem(&toggle(
            mtm,
            "Launch at Login",
            target,
            sel!(toggleLaunchAtLogin:),
            launch_at_login,
            "Start Rustyvibes automatically when you log in",
        ));

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&item(mtm, "About Rustyvibes", target, sel!(showAbout:)));
        // SAFETY: `terminate:` is implemented by NSApplication; a nil target reaches it
        // through the responder chain.
        let quit = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Quit Rustyvibes"),
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        menu.addItem(&quit);

        let menu = Menu {
            menu,
            permission: [permission_separator, permission_item],
            current_pack,
            pack_items,
            release_item,
        };
        menu.show_active_pack(library, active);
        menu
    }

    /// Checks the active pack, names it in the main menu, and enables the
    /// release-sounds toggle only for packs recorded with release sounds.
    pub fn show_active_pack(&self, library: &Library, active: usize) {
        for (index, row) in self.pack_items.iter().enumerate() {
            row.setState(control_state(index == active));
        }
        match library.get(active) {
            Some(pack) => {
                self.current_pack.setTitle(&NSString::from_str(&pack.display_name()));
                self.current_pack.setImage(Some(&icons::keycap(&pack.meta.color)));
                self.release_item.setEnabled(pack.has_release);
            }
            None => {
                self.current_pack.setTitle(&NSString::from_str("No soundpacks found"));
                self.release_item.setEnabled(false);
            }
        }
    }

    /// Shows or hides the "Allow Keyboard Access…" row.
    pub fn show_permission_needed(&self, needed: bool) {
        for item in &self.permission {
            item.setHidden(!needed);
        }
    }
}

pub fn control_state(on: bool) -> NSControlStateValue {
    if on { NSControlStateValueOn } else { NSControlStateValueOff }
}

fn item(mtm: MainThreadMarker, title: &str, target: &AnyObject, action: Sel) -> Retained<NSMenuItem> {
    // SAFETY: every action passed here is implemented by `target`, the app delegate.
    unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::new(),
        );
        item.setTarget(Some(target));
        item
    }
}

fn toggle(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
    on: bool,
    tooltip: &str,
) -> Retained<NSMenuItem> {
    let item = item(mtm, title, target, action);
    item.setState(control_state(on));
    item.setToolTip(Some(&NSString::from_str(tooltip)));
    item
}

/// A section title: native on macOS 14+, a disabled bold item on macOS 13.
fn section_header(mtm: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    if available!(macos = 14.0) {
        return NSMenuItem::sectionHeaderWithTitle(&NSString::from_str(title), mtm);
    }
    let font = NSFont::boldSystemFontOfSize(11.0);
    // SAFETY: NSFontAttributeName is an immutable framework constant.
    let key: &NSString = unsafe { NSFontAttributeName };
    let value: &AnyObject = &font;
    let attributes = NSDictionary::from_slices(&[key], &[value]);
    // SAFETY: the attributes dictionary maps a valid key to an NSFont.
    let text = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(title),
            Some(&attributes),
        )
    };
    let item = NSMenuItem::new(mtm);
    item.setAttributedTitle(Some(&text));
    item.setEnabled(false);
    item
}

fn category_title(category: &str) -> &'static str {
    match category {
        "linear" => "Linear",
        "tactile" => "Tactile",
        "clicky" => "Clicky",
        _ => "Other",
    }
}
```

**File:** `crates/rustyvibes/src/ui/mod.rs`
```rust
//! The menu bar user interface (AppKit through objc2).

pub mod icons;
pub mod menu;
pub mod views;
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p rustyvibes && cargo clippy -p rustyvibes --all-targets -- -D warnings && cargo fmt --check`
Expected: 45 tests pass, clean (the binary still has `#![allow(dead_code)]` until Task 10 wires the UI).

- [ ] **Step 6: Commit**

```bash
git add crates/rustyvibes Cargo.lock
git commit -m "feat(ui): settings store, code-drawn icons, menu and custom views

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: App delegate, permission onboarding, About, launch at login, snapshots

**Files:**
- Create: `crates/rustyvibes/src/ui/delegate.rs`, `crates/rustyvibes/src/ui/onboarding.rs`, `crates/rustyvibes/src/ui/about.rs`, `crates/rustyvibes/src/ui/login.rs`, `crates/rustyvibes/src/snapshot.rs`
- Modify: `crates/rustyvibes/src/ui/mod.rs`, `crates/rustyvibes/src/main.rs` (final), `crates/rustyvibes/src/engine/mod.rs` (`Kick::none` test-only), `crates/rustyvibes/src/engine/mixer.rs` (`active_voices` test-only), `crates/rustyvibes/src/input/keystate.rs` (test-only flag constants)
- Test: `about.rs` credits test; live run, `--snapshot` visual review

**Interfaces:**
- Consumes: everything above.
- Produces: the runnable menu bar app (`ui::run()`), `--snapshot`, and these delegate actions: `toggleEnabled:`, `selectPack:`, `volumeChanged:`, `toggleReleaseSounds:`, `toggleVariation:`, `toggleSpatial:`, `toggleLaunchAtLogin:`, `showPermissionHelp:`, `openInputMonitoringSettings:`, `dismissOnboarding:`, `relaunch:`, `pollPermission:`, `showAbout:`.

- [ ] **Step 1: Write the failing credits test**

**File:** `crates/rustyvibes/src/ui/about.rs` (test module; implementation goes above it in Step 3)
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::testing::{loaded, pack_data};

    #[test]
    fn credits_group_packs_by_source() {
        let mut a = pack_data("a", "linear", 0, true, false);
        a.meta.credit = "Mechvibes by Hai Nguyen · MIT License".into();
        let mut b = pack_data("b", "clicky", 1, false, false);
        b.meta.credit = "kbsim by Thomas Lai · MIT License".into();
        let mut c = pack_data("c", "clicky", 2, false, false);
        c.meta.credit = "kbsim by Thomas Lai · MIT License".into();
        let library = Library::new(vec![loaded(&a), loaded(&b), loaded(&c)]);
        let text = credits_text(&library);
        assert!(text.contains("Sounds: Mechvibes by Hai Nguyen · MIT License\na\n"), "{text}");
        assert!(text.contains("Sounds: kbsim by Thomas Lai · MIT License\nb, c\n"), "{text}");
        assert!(text.contains("never recorded"));
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Add `mod about;` to `ui/mod.rs`, then run: `cargo test -p rustyvibes credits_group`
Expected: compile error — `credits_text` not found.

- [ ] **Step 3: Implement the remaining UI**

**File:** `crates/rustyvibes/src/ui/about.rs` (implementation part)
```rust
//! The standard About panel, with credits for every soundpack source.

use std::collections::BTreeMap;

use objc2::MainThreadMarker;
use objc2::runtime::AnyObject;
use objc2_app_kit::{
    NSAboutPanelOptionCredits, NSApplication, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSTextAlignment,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};

use super::onboarding::activate;
use crate::packs::Library;

pub fn show(mtm: MainThreadMarker, library: &Library) {
    activate(mtm);
    let font = NSFont::systemFontOfSize(11.0);
    let color = NSColor::secondaryLabelColor();
    let paragraph = NSMutableParagraphStyle::new();
    paragraph.setAlignment(NSTextAlignment::Center);
    // SAFETY: framework constants.
    let keys: [&NSString; 3] =
        unsafe { [NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName] };
    let values: [&AnyObject; 3] = [&font, &color, &paragraph];
    let attributes = NSDictionary::from_slices(&keys, &values);
    // SAFETY: the attribute values have the types AppKit expects for these keys.
    let credits = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(&credits_text(library)),
            Some(&attributes),
        )
    };
    // SAFETY: framework constant.
    let option: &NSString = unsafe { NSAboutPanelOptionCredits };
    let value: &AnyObject = &credits;
    let options = NSDictionary::from_slices(&[option], &[value]);
    // SAFETY: the credits option takes an NSAttributedString.
    unsafe { NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanelWithOptions(&options) };
}

/// A short description, then each soundpack source with its packs.
pub fn credits_text(library: &Library) -> String {
    let mut by_credit: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for pack in library.iter() {
        by_credit.entry(pack.meta.credit.as_str()).or_default().push(pack.display_name());
    }
    let mut text = String::from(
        "Mechanical keyboard sounds for every key press.\n\
         No network access, no analytics; keystrokes are never recorded.\n",
    );
    for (credit, packs) in by_credit {
        text.push_str(&format!("\nSounds: {credit}\n{}\n", packs.join(", ")));
    }
    text
}
```

**File:** `crates/rustyvibes/src/ui/login.rs`
```rust
//! Launch at login through `SMAppService.mainAppService` (macOS 13+).

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject};
use objc2_foundation::NSError;

#[link(name = "ServiceManagement", kind = "framework")]
unsafe extern "C" {}

/// `SMAppServiceStatus` values.
const ENABLED: isize = 1;
const REQUIRES_APPROVAL: isize = 2;

fn service() -> Option<Retained<AnyObject>> {
    let class = AnyClass::get(c"SMAppService")?;
    // SAFETY: `+[SMAppService mainAppService]` returns a non-nil object.
    Some(unsafe { msg_send![class, mainAppService] })
}

fn status() -> isize {
    // SAFETY: `-[SMAppService status]` returns an NSInteger-backed enum.
    service().map_or(0, |s| unsafe { msg_send![&*s, status] })
}

pub fn is_enabled() -> bool {
    status() == ENABLED
}

pub fn requires_approval() -> bool {
    status() == REQUIRES_APPROVAL
}

pub fn set_enabled(on: bool) -> Result<(), String> {
    let service = service().ok_or("launch at login needs macOS 13 or later")?;
    // SAFETY: both methods take a trailing NSError** and return BOOL.
    let result: Result<(), Retained<NSError>> = unsafe {
        if on {
            msg_send![&*service, registerAndReturnError: _]
        } else {
            msg_send![&*service, unregisterAndReturnError: _]
        }
    };
    result.map_err(|e| e.localizedDescription().to_string())
}

/// Opens System Settings → General → Login Items.
pub fn open_login_items_settings() {
    if let Some(class) = AnyClass::get(c"SMAppService") {
        // SAFETY: class method without arguments or return value (macOS 13+).
        let _: () = unsafe { msg_send![class, openSystemSettingsLoginItems] };
    }
}
```

**File:** `crates/rustyvibes/src/ui/onboarding.rs`
```rust
//! The keyboard-access window shown until Input Monitoring is granted.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, available, sel};
use objc2_app_kit::{
    NSApplication, NSBackingStoreType, NSButton, NSColor, NSFont, NSImage, NSImageView,
    NSLayoutAttribute, NSStackView, NSTextAlignment, NSTextField, NSUserInterfaceLayoutOrientation,
    NSView, NSWindow, NSWindowStyleMask, NSWindowTitleVisibility,
};
use objc2_foundation::{NSArray, NSEdgeInsets, NSString};

use super::icons::{self, rect};

const WIDTH: f64 = 440.0;

/// Where the user is in the permission flow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Waiting,
    Granted,
    NeedsRelaunch,
}

pub struct Onboarding {
    pub window: Retained<NSWindow>,
    status: Retained<NSTextField>,
    primary: Retained<NSButton>,
    secondary: Retained<NSButton>,
}

impl Onboarding {
    /// Builds the window; `target` (the app delegate) receives the button actions.
    pub fn new(mtm: MainThreadMarker, target: &AnyObject) -> Onboarding {
        // SAFETY: standard window creation on the main thread.
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect(0.0, 0.0, WIDTH, 320.0),
                NSWindowStyleMask::Titled | NSWindowStyleMask::Closable | NSWindowStyleMask::FullSizeContentView,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // SAFETY: we keep our own strong reference, so AppKit must not release on close.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str("Rustyvibes"));
        window.setTitleVisibility(NSWindowTitleVisibility::Hidden);
        window.setTitlebarAppearsTransparent(true);
        window.setMovableByWindowBackground(true);

        let icon = NSImageView::imageViewWithImage(&app_icon(mtm), mtm);
        icon.widthAnchor().constraintEqualToConstant(88.0).setActive(true);
        icon.heightAnchor().constraintEqualToConstant(88.0).setActive(true);
        let title = label(mtm, "Let Rustyvibes hear your keys", &NSFont::boldSystemFontOfSize(20.0));
        let body = paragraph(
            mtm,
            "Rustyvibes plays a sound every time you press a key. macOS asks for your \
             permission before any app can notice key presses outside its own windows.",
            13.0,
            &NSColor::secondaryLabelColor(),
        );
        let steps = paragraph(
            mtm,
            "Click Open System Settings, then switch on Rustyvibes under \
             Privacy & Security → Input Monitoring.",
            13.0,
            &NSColor::labelColor(),
        );
        let privacy = paragraph(
            mtm,
            "Private by design: Rustyvibes only learns which key moved, never what you type. \
             Nothing is recorded or stored, and it never connects to the internet.",
            11.0,
            &NSColor::tertiaryLabelColor(),
        );
        let status = label(mtm, "", &NSFont::systemFontOfSize(12.0));
        // SAFETY: `target` implements both actions.
        let (secondary, primary) = unsafe {
            (
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Not Now"),
                    Some(target),
                    Some(sel!(dismissOnboarding:)),
                    mtm,
                ),
                NSButton::buttonWithTitle_target_action(
                    &NSString::from_str("Open System Settings"),
                    Some(target),
                    Some(sel!(openInputMonitoringSettings:)),
                    mtm,
                ),
            )
        };
        primary.setKeyEquivalent(&NSString::from_str("\r"));
        let buttons = stack(mtm, &[&secondary, &primary], NSUserInterfaceLayoutOrientation::Horizontal, 12.0);
        let content = stack(
            mtm,
            &[&icon, &title, &body, &steps, &privacy, &status, &buttons],
            NSUserInterfaceLayoutOrientation::Vertical,
            12.0,
        );
        content.setAlignment(NSLayoutAttribute::CenterX);
        content.setEdgeInsets(NSEdgeInsets { top: 36.0, left: 36.0, bottom: 28.0, right: 36.0 });
        content.setCustomSpacing_afterView(16.0, &icon);
        content.setCustomSpacing_afterView(20.0, &status);
        content.widthAnchor().constraintEqualToConstant(WIDTH).setActive(true);
        window.setContentView(Some(&content));
        window.setContentSize(content.fittingSize());
        window.center();

        let onboarding = Onboarding { window, status, primary, secondary };
        onboarding.set_step(Step::Waiting);
        onboarding
    }

    pub fn set_step(&self, step: Step) {
        let (status, color, primary, action, show_secondary) = match step {
            Step::Waiting => (
                "Waiting for permission…",
                NSColor::secondaryLabelColor(),
                "Open System Settings",
                sel!(openInputMonitoringSettings:),
                true,
            ),
            Step::Granted => (
                "All set. Enjoy the sound of typing.",
                NSColor::systemGreenColor(),
                "Done",
                sel!(dismissOnboarding:),
                false,
            ),
            Step::NeedsRelaunch => (
                "Almost there: Rustyvibes needs a restart to start listening.",
                NSColor::systemOrangeColor(),
                "Relaunch Rustyvibes",
                sel!(relaunch:),
                true,
            ),
        };
        self.status.setStringValue(&NSString::from_str(status));
        self.status.setTextColor(Some(&color));
        self.primary.setTitle(&NSString::from_str(primary));
        // SAFETY: the buttons' target (the app delegate) implements every action used here.
        unsafe { self.primary.setAction(Some(action)) };
        self.secondary.setHidden(!show_secondary);
    }

    pub fn show(&self, mtm: MainThreadMarker) {
        activate(mtm);
        self.window.makeKeyAndOrderFront(None);
    }

    pub fn is_visible(&self) -> bool {
        self.window.isVisible()
    }
}

/// Brings this accessory app to the front (needed before showing a window).
pub fn activate(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    if available!(macos = 14.0) {
        app.activate();
    } else {
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
    }
}

fn app_icon(mtm: MainThreadMarker) -> Retained<NSImage> {
    NSApplication::sharedApplication(mtm)
        .applicationIconImage()
        .unwrap_or_else(|| icons::symbol("keyboard", "Rustyvibes"))
}

fn label(mtm: MainThreadMarker, text: &str, font: &NSFont) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(font));
    label.setAlignment(NSTextAlignment::Center);
    label
}

fn paragraph(mtm: MainThreadMarker, text: &str, size: f64, color: &NSColor) -> Retained<NSTextField> {
    let paragraph = NSTextField::wrappingLabelWithString(&NSString::from_str(text), mtm);
    paragraph.setFont(Some(&NSFont::systemFontOfSize(size)));
    paragraph.setTextColor(Some(color));
    paragraph.setAlignment(NSTextAlignment::Center);
    paragraph.setSelectable(false);
    paragraph.setPreferredMaxLayoutWidth(WIDTH - 72.0);
    paragraph
}

fn stack(
    mtm: MainThreadMarker,
    views: &[&NSView],
    orientation: NSUserInterfaceLayoutOrientation,
    spacing: f64,
) -> Retained<NSStackView> {
    let stack = NSStackView::stackViewWithViews(&NSArray::from_slice(views), mtm);
    stack.setOrientation(orientation);
    stack.setSpacing(spacing);
    stack
}
```

**File:** `crates/rustyvibes/src/ui/delegate.rs`
```rust
//! The application delegate: owns the runtime and the UI and handles every action.

use std::cell::{Cell, OnceCell, RefCell};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSControlStateValueOn,
    NSMenuItem, NSSlider, NSStatusBar, NSStatusItem, NSSwitch, NSVariableStatusItemLength,
    NSWorkspace,
};
use objc2_foundation::{NSBundle, NSNotification, NSString, NSTimer, NSURL};

use super::icons::{self, StatusIcon};
use super::menu::{Menu, control_state};
use super::onboarding::{Onboarding, Step};
use super::{about, login};
use crate::engine::Shared;
use crate::input;
use crate::log::log;
use crate::runtime::Runtime;
use crate::settings::{self, Store};

const INPUT_MONITORING_URL: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent";
/// Minimum gap between preview clicks while the volume slider moves.
const PREVIEW_INTERVAL: Duration = Duration::from_millis(120);

/// Starts the menu bar app. Does not return.
pub fn run() {
    let mtm = MainThreadMarker::new().expect("Rustyvibes must start on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    app.run();
}

/// Everything created at launch.
struct State {
    runtime: Runtime,
    store: Store,
    status_item: Retained<NSStatusItem>,
    menu: Menu,
}

#[derive(Default)]
struct Ivars {
    state: OnceCell<RefCell<State>>,
    input_running: Cell<bool>,
    last_preview: Cell<Option<Instant>>,
    onboarding: RefCell<Option<Onboarding>>,
    poll_timer: RefCell<Option<Retained<NSTimer>>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and we implement no `Drop`.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RVAppDelegate"]
    #[ivars = Ivars]
    struct AppDelegate;

    // SAFETY: NSObjectProtocol has no additional requirements.
    unsafe impl NSObjectProtocol for AppDelegate {}

    // SAFETY: the signature matches `-applicationDidFinishLaunching:`.
    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            self.launch();
        }
    }

    // SAFETY (all actions): each takes one object argument and returns void.
    impl AppDelegate {
        #[unsafe(method(toggleEnabled:))]
        fn toggle_enabled(&self, sender: &NSSwitch) {
            self.set_enabled(sender.state() == NSControlStateValueOn);
        }

        #[unsafe(method(selectPack:))]
        fn select_pack(&self, sender: &NSMenuItem) {
            self.choose_pack(usize::try_from(sender.tag()).unwrap_or(usize::MAX));
        }

        #[unsafe(method(volumeChanged:))]
        fn volume_changed(&self, sender: &NSSlider) {
            self.set_volume(sender.doubleValue() as f32);
        }

        #[unsafe(method(toggleReleaseSounds:))]
        fn toggle_release_sounds(&self, sender: &NSMenuItem) {
            self.flip(sender, settings::KEY_RELEASE_SOUNDS, |s| &s.release_sounds);
        }

        #[unsafe(method(toggleVariation:))]
        fn toggle_variation(&self, sender: &NSMenuItem) {
            self.flip(sender, settings::KEY_VARIATION, |s| &s.variation);
        }

        #[unsafe(method(toggleSpatial:))]
        fn toggle_spatial(&self, sender: &NSMenuItem) {
            self.flip(sender, settings::KEY_SPATIAL, |s| &s.spatial);
        }

        #[unsafe(method(toggleLaunchAtLogin:))]
        fn toggle_launch_at_login(&self, sender: &NSMenuItem) {
            self.set_launch_at_login(sender);
        }

        #[unsafe(method(showPermissionHelp:))]
        fn show_permission_help(&self, _sender: &AnyObject) {
            self.show_onboarding();
        }

        #[unsafe(method(openInputMonitoringSettings:))]
        fn open_input_monitoring_settings(&self, _sender: &AnyObject) {
            self.open_settings();
        }

        #[unsafe(method(dismissOnboarding:))]
        fn dismiss_onboarding(&self, _sender: &AnyObject) {
            self.close_onboarding();
        }

        #[unsafe(method(relaunch:))]
        fn relaunch_action(&self, _sender: &AnyObject) {
            relaunch(self.mtm());
        }

        #[unsafe(method(pollPermission:))]
        fn poll_permission(&self, _timer: &NSTimer) {
            self.check_permission();
        }

        #[unsafe(method(showAbout:))]
        fn show_about(&self, _sender: &AnyObject) {
            if let Some(state) = self.ivars().state.get() {
                about::show(self.mtm(), state.borrow().runtime.library);
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars::default());
        // SAFETY: NSObject's designated initialiser.
        unsafe { msg_send![super(this), init] }
    }

    fn launch(&self) {
        let mtm = self.mtm();
        let store = Store::standard();
        let settings = store.load();
        let runtime = Runtime::start(&settings);
        let target: &AnyObject = self;
        let menu = Menu::build(mtm, target, runtime.library, &settings, runtime.active_pack(), login::is_enabled());
        let status_item = NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        status_item.setMenu(Some(&menu.menu));
        let _ = self.ivars().state.set(RefCell::new(State { runtime, store, status_item, menu }));
        let listening = self.start_input();
        self.refresh_status();
        if !listening || std::env::var_os("RUSTYVIBES_SHOW_ONBOARDING").is_some() {
            self.show_onboarding();
        }
        log!("launched; keyboard {}", if listening { "connected" } else { "waiting for permission" });
    }

    /// Starts the event tap when permission allows. Returns whether input is running.
    fn start_input(&self) -> bool {
        if self.ivars().input_running.get() {
            return true;
        }
        if !input::has_permission() {
            return false;
        }
        let Some(state) = self.ivars().state.get() else { return false };
        let mut state = state.borrow_mut();
        let Some(voicer) = state.runtime.take_input() else { return false };
        match input::start(voicer) {
            Ok(()) => {
                self.ivars().input_running.set(true);
                true
            }
            Err(voicer) => {
                state.runtime.return_input(voicer);
                false
            }
        }
    }

    /// Updates the status icon, its tooltip and the permission row.
    fn refresh_status(&self) {
        let Some(state) = self.ivars().state.get() else { return };
        let state = state.borrow();
        let listening = self.ivars().input_running.get();
        let enabled = state.runtime.shared.enabled.load(Ordering::Relaxed);
        state.menu.show_permission_needed(!listening);
        if let Some(button) = state.status_item.button(self.mtm()) {
            let icon = if listening { StatusIcon::Normal } else { StatusIcon::Attention };
            button.setImage(Some(&icons::status(icon)));
            button.setAppearsDisabled(!enabled);
            let tip = match (listening, enabled) {
                (false, _) => "Rustyvibes needs keyboard access",
                (true, true) => "Rustyvibes",
                (true, false) => "Rustyvibes (sounds off)",
            };
            button.setToolTip(Some(&NSString::from_str(tip)));
        }
    }

    fn set_enabled(&self, on: bool) {
        if let Some(state) = self.ivars().state.get() {
            let state = state.borrow();
            state.runtime.shared.enabled.store(on, Ordering::Relaxed);
            state.store.set_bool(settings::KEY_ENABLED, on);
        }
        self.refresh_status();
    }

    fn choose_pack(&self, index: usize) {
        let Some(state) = self.ivars().state.get() else { return };
        let mut state = state.borrow_mut();
        let library = state.runtime.library;
        let Some(pack) = library.get(index) else { return };
        state.runtime.select_pack(index);
        state.store.set_string(settings::KEY_PACK, &pack.meta.id);
        state.menu.show_active_pack(library, index);
        if state.runtime.shared.enabled.load(Ordering::Relaxed) {
            state.runtime.preview_flourish(index);
        }
    }

    fn set_volume(&self, volume: f32) {
        let Some(state) = self.ivars().state.get() else { return };
        let mut state = state.borrow_mut();
        state.runtime.shared.set_volume(volume);
        state.store.set_float(settings::KEY_VOLUME, volume);
        let due = self.ivars().last_preview.get().is_none_or(|t| t.elapsed() >= PREVIEW_INTERVAL);
        if due && state.runtime.shared.enabled.load(Ordering::Relaxed) {
            state.runtime.preview_click();
            self.ivars().last_preview.set(Some(Instant::now()));
        }
    }

    fn flip(&self, sender: &NSMenuItem, key: &str, field: impl Fn(&Shared) -> &AtomicBool) {
        let Some(state) = self.ivars().state.get() else { return };
        let state = state.borrow();
        let flag = field(state.runtime.shared);
        let on = !flag.load(Ordering::Relaxed);
        flag.store(on, Ordering::Relaxed);
        state.store.set_bool(key, on);
        sender.setState(control_state(on));
    }

    fn set_launch_at_login(&self, sender: &NSMenuItem) {
        let want = !login::is_enabled();
        if let Err(e) = login::set_enabled(want) {
            log!("launch at login: {e}");
        }
        sender.setState(control_state(login::is_enabled()));
        if want && login::requires_approval() {
            login::open_login_items_settings();
        }
    }

    fn show_onboarding(&self) {
        let mtm = self.mtm();
        {
            let mut onboarding = self.ivars().onboarding.borrow_mut();
            let window = onboarding.get_or_insert_with(|| Onboarding::new(mtm, self));
            window.set_step(Step::Waiting);
            window.show(mtm);
        }
        self.start_polling();
    }

    fn open_settings(&self) {
        // Adds Rustyvibes to the Input Monitoring list (and prompts the first time).
        input::request_permission();
        if let Some(url) = NSURL::URLWithString(&NSString::from_str(INPUT_MONITORING_URL)) {
            NSWorkspace::sharedWorkspace().openURL(&url);
        }
    }

    fn close_onboarding(&self) {
        if let Some(onboarding) = self.ivars().onboarding.borrow().as_ref() {
            onboarding.window.orderOut(None);
        }
        // While permission is missing, keep polling so a grant made later in
        // System Settings is still noticed.
        if self.ivars().input_running.get() {
            self.stop_polling();
        }
    }

    fn start_polling(&self) {
        let mut timer = self.ivars().poll_timer.borrow_mut();
        if timer.is_none() {
            // SAFETY: `self` implements `pollPermission:`; the timer retains its target.
            *timer = Some(unsafe {
                NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                    1.0,
                    self,
                    sel!(pollPermission:),
                    None,
                    true,
                )
            });
        }
    }

    fn stop_polling(&self) {
        if let Some(timer) = self.ivars().poll_timer.borrow_mut().take() {
            timer.invalidate();
        }
    }

    /// Runs once a second while keyboard access is missing.
    fn check_permission(&self) {
        if self.ivars().input_running.get() {
            self.stop_polling();
            return;
        }
        if !input::has_permission() {
            return;
        }
        let listening = self.start_input();
        self.stop_polling();
        self.refresh_status();
        log!("permission granted; keyboard {}", if listening { "connected" } else { "needs a relaunch" });
        let onboarding = self.ivars().onboarding.borrow();
        let Some(onboarding) = onboarding.as_ref() else { return };
        if listening {
            if onboarding.is_visible() {
                onboarding.set_step(Step::Granted);
                // SAFETY: `self` implements `dismissOnboarding:`.
                let _ = unsafe {
                    NSTimer::scheduledTimerWithTimeInterval_target_selector_userInfo_repeats(
                        1.6,
                        self,
                        sel!(dismissOnboarding:),
                        None,
                        false,
                    )
                };
            }
        } else {
            onboarding.set_step(Step::NeedsRelaunch);
            onboarding.show(self.mtm());
        }
    }
}

/// Starts a fresh copy of the app bundle, then quits this one.
fn relaunch(mtm: MainThreadMarker) {
    let bundle = NSBundle::mainBundle().bundlePath().to_string();
    if bundle.ends_with(".app") {
        let _ = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 0.5; /usr/bin/open \"$0\"", &bundle])
            .spawn();
    }
    NSApplication::sharedApplication(mtm).terminate(None);
}
```

**File:** `crates/rustyvibes/src/ui/mod.rs`
```rust
//! The menu bar user interface (AppKit through objc2).

mod about;
mod delegate;
pub mod icons;
mod login;
pub mod menu;
pub mod onboarding;
pub mod views;

pub use delegate::run;
```

**File:** `crates/rustyvibes/src/snapshot.rs`
```rust
//! `--snapshot [dir]`: renders the custom views, icons and the onboarding
//! window to PNG in light and dark appearance, for visual review without
//! screen-recording permission. Backgrounds are transparent.

use std::path::Path;

use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSApplicationActivationPolicy, NSBitmapImageFileType, NSImageView, NSView,
};
use objc2_foundation::{NSDictionary, NSString};

use crate::packs::{Library, default_dir};
use crate::ui::icons::{self, StatusIcon, rect};
use crate::ui::onboarding::Onboarding;
use crate::ui::views;

pub fn snapshot(dir: &Path) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("must run on the main thread")?;
    NSApplication::sharedApplication(mtm).setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let library = Library::load_dir(&default_dir());
    // Actions are never sent while rendering, so any object can be the target.
    let target = NSObject::new();
    // SAFETY: framework constants.
    let appearances = unsafe { [("light", NSAppearanceNameAqua), ("dark", NSAppearanceNameDarkAqua)] };
    for (suffix, name) in appearances {
        let appearance = NSAppearance::appearanceNamed(name).ok_or("missing appearance")?;
        let file = |what: &str| dir.join(format!("{what}-{suffix}.png"));
        render(&views::header(mtm, &target, true), &appearance, &file("header"))?;
        render(&views::volume(mtm, &target, 0.6), &appearance, &file("volume"))?;
        render(&icon_strip(mtm, &library), &appearance, &file("icons"))?;
        let onboarding = Onboarding::new(mtm, &target);
        let content = onboarding.window.contentView().ok_or("onboarding has no content")?;
        render(&content, &appearance, &file("onboarding"))?;
    }
    println!("snapshots written to {}", dir.display());
    Ok(())
}

/// Both status icons followed by every pack's keycap glyph.
fn icon_strip(mtm: MainThreadMarker, library: &Library) -> Retained<NSView> {
    let mut images = vec![icons::status(StatusIcon::Normal), icons::status(StatusIcon::Attention)];
    images.extend(library.iter().map(|p| icons::keycap(&p.meta.color)));
    let cell = 32.0;
    let view = NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, cell * images.len() as f64, cell));
    for (i, image) in images.iter().enumerate() {
        let image_view = NSImageView::imageViewWithImage(image, mtm);
        image_view.setFrame(rect(i as f64 * cell + 7.0, 7.0, 18.0, 18.0));
        view.addSubview(&image_view);
    }
    view
}

fn render(view: &NSView, appearance: &NSAppearance, path: &Path) -> Result<(), String> {
    view.setAppearance(Some(appearance));
    view.layoutSubtreeIfNeeded();
    let bounds = view.bounds();
    let bitmap = view.bitmapImageRepForCachingDisplayInRect(bounds).ok_or("cannot allocate a bitmap")?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    // SAFETY: an empty properties dictionary is valid for PNG encoding.
    let png = unsafe { bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new()) }
        .ok_or("PNG encoding failed")?;
    if png.writeToFile_atomically(&NSString::from_str(&path.to_string_lossy()), true) {
        Ok(())
    } else {
        Err(format!("cannot write {}", path.display()))
    }
}
```

**File:** `crates/rustyvibes/src/main.rs`
```rust
//! Rustyvibes 2: mechanical keyboard sounds for every key press, as a native
//! macOS menu bar app.

mod audio;
mod diagnostics;
mod engine;
mod input;
mod log;
mod packs;
mod runtime;
mod settings;
mod snapshot;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "Rustyvibes: mechanical keyboard sounds for every key press.

Run without arguments to start the menu bar app. Diagnostics:
  --version              print the version
  --selftest [--audible] play every soundpack through the audio path (silent unless --audible)
  --tap-test             check that key events reach the engine (injects one Shift press)
  --bench                measure the mixer
  --snapshot [dir]       render UI pieces to PNG (default: target/snapshots)

Set RUSTYVIBES_LOG=1 to log to stderr.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
        Some("--tap-test") => diagnostics::tap_test(),
        Some("--bench") => {
            diagnostics::bench();
            Ok(())
        }
        Some("--snapshot") => {
            let dir = args.get(1).map_or_else(|| PathBuf::from("target/snapshots"), PathBuf::from);
            snapshot::snapshot(&dir)
        }
        // Anything else (including LaunchServices' legacy -psn_ argument) starts the app.
        _ => {
            ui::run();
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

Also, now that `#![allow(dead_code)]` is gone, make test-only helpers test-only:
- `crates/rustyvibes/src/engine/mod.rs`: put `#[cfg(test)]` on `Kick::none`.
- `crates/rustyvibes/src/engine/mixer.rs`: put `#[cfg(test)]` on `Mixer::active_voices`.
- `crates/rustyvibes/src/input/keystate.rs`: keep only `FLAG_FN` and the `DEV_*` constants public in non-test builds; move `FLAG_ALPHA_SHIFT`, `FLAG_SHIFT`, `FLAG_COMMAND` (and delete `FLAG_CONTROL`, `FLAG_OPTION`) into the test module as local constants.

- [ ] **Step 4: Run tests and lints**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: all tests pass (rvpack 18, xtask 22 + 1 ignored, rustyvibes 46), no warnings.

- [ ] **Step 5: Visual review with snapshots**

Run: `cargo run --release -p rustyvibes -- --snapshot target/snapshots`
Then composite each transparent PNG onto its appearance's window colour (light `#ECECEC`, dark `#2B2B2B`) and inspect: the header shows "Rustyvibes" with a switch at the right; the volume row shows two speaker glyphs around the slider; the icon strip shows a crisp keycap, a keycap with "!", and 21 tinted keycaps; the onboarding window shows icon, title, three wrapped paragraphs, a status line and two buttons, nothing clipped. Fix any layout problem before continuing.

- [ ] **Step 6: Live run**

Run (from a terminal with Input Monitoring): `RUSTYVIBES_LOG=1 target/release/rustyvibes & sleep 3; kill %1`
Expected log: packs loaded, `input: event tap running`, `launched; keyboard connected`. While it runs, `--tap-test` style checks are covered by Task 8; here confirm the process stays alive and idle.

- [ ] **Step 7: Commit**

```bash
git add crates/rustyvibes
git commit -m "feat(ui): menu bar app, permission onboarding, About and launch at login

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: App icon, `.app` bundle, signing and DMG

**Files:**
- Create: `scripts/make-icon.swift`, `assets/icon/AppIcon.icns`, `assets/icon/AppIcon-1024.png` (generated), `crates/xtask/src/bundle.rs`, `crates/xtask/src/dmg.rs`, `LICENSE`, `THIRD_PARTY_NOTICES.md`
- Modify: `crates/xtask/src/main.rs`
- Test: `bundle.rs` unit test for `info_plist`; end-to-end `cargo xtask bundle` and `cargo xtask dmg`

**Interfaces:**
- Consumes: Task 4 `packs::build_all`; the app crate.
- Produces: `target/bundle/Rustyvibes.app`, `target/Rustyvibes-2.0.0.dmg`; `bundle::{run, bundle, Options, Bundle, info_plist}`, `dmg::run`.

- [ ] **Step 1: Render the icon**

**File:** `scripts/make-icon.swift`
```swift
// Renders the Rustyvibes app icon.
//   swift scripts/make-icon.swift assets/icon
// Writes AppIcon.iconset/*.png, AppIcon.icns (via iconutil) and AppIcon-1024.png.

import AppKit

let outDir = CommandLine.arguments.count > 1 ? CommandLine.arguments[1] : "assets/icon"
let sRGB = CGColorSpace(name: CGColorSpace.sRGB)!

func rgb(_ hex: UInt32, _ alpha: CGFloat = 1) -> CGColor {
    CGColor(
        srgbRed: CGFloat((hex >> 16) & 0xFF) / 255,
        green: CGFloat((hex >> 8) & 0xFF) / 255,
        blue: CGFloat(hex & 0xFF) / 255,
        alpha: alpha)
}

func gradient(_ colors: [CGColor], _ locations: [CGFloat]) -> CGGradient {
    CGGradient(colorsSpace: sRGB, colors: colors as CFArray, locations: locations)!
}

/// Draws the icon into a 1024×1024 coordinate space.
func drawIcon(_ cg: CGContext) {
    // The macOS icon grid: an 824 pt rounded square centred in the canvas.
    let body = CGRect(x: 100, y: 100, width: 824, height: 824)
    let tile = CGPath(roundedRect: body, cornerWidth: 186, cornerHeight: 186, transform: nil)

    cg.saveGState()
    cg.setShadow(offset: CGSize(width: 0, height: -12), blur: 28, color: rgb(0x000000, 0.32))
    cg.addPath(tile)
    cg.setFillColor(rgb(0xC0461B))
    cg.fillPath()
    cg.restoreGState()

    cg.saveGState()
    cg.addPath(tile)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xF68A4B), rgb(0xE0602A), rgb(0xA9330D)], [0, 0.5, 1]),
        start: CGPoint(x: 512, y: 924), end: CGPoint(x: 512, y: 100), options: [])
    cg.drawRadialGradient(
        gradient([rgb(0xFFFFFF, 0.25), rgb(0xFFFFFF, 0)], [0, 1]),
        startCenter: CGPoint(x: 512, y: 960), startRadius: 0,
        endCenter: CGPoint(x: 512, y: 960), endRadius: 640, options: [])

    // Sound waves either side of the key.
    cg.setLineCap(.round)
    for (radius, alpha) in [(CGFloat(262), CGFloat(0.9)), (CGFloat(334), CGFloat(0.5))] {
        cg.setStrokeColor(rgb(0xFFF3E4, alpha))
        cg.setLineWidth(30)
        for side in [CGFloat(0), CGFloat.pi] {
            cg.addArc(center: CGPoint(x: 512, y: 486), radius: radius,
                      startAngle: side - 0.40, endAngle: side + 0.40, clockwise: false)
            cg.strokePath()
        }
    }

    // Keycap: soft shadow, skirt, then the dished top.
    let skirt = CGPath(roundedRect: CGRect(x: 327, y: 300, width: 370, height: 362),
                       cornerWidth: 72, cornerHeight: 72, transform: nil)
    cg.saveGState()
    cg.setShadow(offset: CGSize(width: 0, height: -22), blur: 34, color: rgb(0x3A0E00, 0.45))
    cg.addPath(skirt)
    cg.setFillColor(rgb(0xE6CFA9))
    cg.fillPath()
    cg.restoreGState()

    cg.saveGState()
    cg.addPath(skirt)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xF4E4C8), rgb(0xD9BE92)], [0, 1]),
        start: CGPoint(x: 512, y: 662), end: CGPoint(x: 512, y: 300), options: [])
    cg.restoreGState()

    let top = CGPath(roundedRect: CGRect(x: 377, y: 398, width: 270, height: 238),
                     cornerWidth: 46, cornerHeight: 46, transform: nil)
    cg.saveGState()
    cg.addPath(top)
    cg.clip()
    cg.drawLinearGradient(
        gradient([rgb(0xFFFAF0), rgb(0xF2E2C4)], [0, 1]),
        start: CGPoint(x: 512, y: 636), end: CGPoint(x: 512, y: 398), options: [])
    // The dish: a gentle darker centre.
    cg.drawRadialGradient(
        gradient([rgb(0xC9A979, 0.22), rgb(0xC9A979, 0)], [0, 1]),
        startCenter: CGPoint(x: 512, y: 510), startRadius: 0,
        endCenter: CGPoint(x: 512, y: 510), endRadius: 150, options: [])
    cg.restoreGState()

    cg.restoreGState()
}

func render(_ pixels: Int) -> NSBitmapImageRep {
    let rep = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels, bitsPerSample: 8,
        samplesPerPixel: 4, hasAlpha: true, isPlanar: false, colorSpaceName: .deviceRGB,
        bytesPerRow: 0, bitsPerPixel: 0)!
    let context = NSGraphicsContext(bitmapImageRep: rep)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.cgContext.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
    drawIcon(context.cgContext)
    NSGraphicsContext.restoreGraphicsState()
    return rep
}

func writePNG(_ rep: NSBitmapImageRep, _ path: String) {
    try! rep.representation(using: .png, properties: [:])!.write(to: URL(fileURLWithPath: path))
}

let iconset = "\(outDir)/AppIcon.iconset"
try? FileManager.default.removeItem(atPath: iconset)
try! FileManager.default.createDirectory(atPath: iconset, withIntermediateDirectories: true)
for (name, pixels) in [
    ("icon_16x16", 16), ("icon_16x16@2x", 32), ("icon_32x32", 32), ("icon_32x32@2x", 64),
    ("icon_128x128", 128), ("icon_128x128@2x", 256), ("icon_256x256", 256),
    ("icon_256x256@2x", 512), ("icon_512x512", 512), ("icon_512x512@2x", 1024),
] {
    writePNG(render(pixels), "\(iconset)/\(name).png")
}
writePNG(render(1024), "\(outDir)/AppIcon-1024.png")

let iconutil = Process()
iconutil.executableURL = URL(fileURLWithPath: "/usr/bin/iconutil")
iconutil.arguments = ["-c", "icns", iconset, "-o", "\(outDir)/AppIcon.icns"]
try! iconutil.run()
iconutil.waitUntilExit()
try? FileManager.default.removeItem(atPath: iconset)
print("wrote \(outDir)/AppIcon.icns and \(outDir)/AppIcon-1024.png")
```

Run: `mkdir -p assets/icon && swift scripts/make-icon.swift assets/icon`
Expected: `wrote assets/icon/AppIcon.icns and assets/icon/AppIcon-1024.png`. Open `assets/icon/AppIcon-1024.png` and check it reads as a cream keycap with sound waves on a rust-orange rounded square; adjust proportions in the script until it does.

- [ ] **Step 2: Write the failing Info.plist test**

**File:** `crates/xtask/src/bundle.rs` (test module; implementation goes above it in Step 4)
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_plist_has_the_required_keys() {
        let plist = info_plist("2.0.0");
        for needle in [
            "<key>CFBundleIdentifier</key>\n\t<string>io.github.kb24x7.rustyvibes</string>",
            "<key>CFBundleExecutable</key>\n\t<string>rustyvibes</string>",
            "<key>CFBundleShortVersionString</key>\n\t<string>2.0.0</string>",
            "<key>LSMinimumSystemVersion</key>\n\t<string>13.0</string>",
            "<key>LSUIElement</key>\n\t<true/>",
            "<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>",
        ] {
            assert!(plist.contains(needle), "missing {needle}");
        }
        assert!(plist.starts_with("<?xml"));
    }
}
```

- [ ] **Step 3: Run the test to verify it fails**

Add `mod bundle;` to `crates/xtask/src/main.rs`, then run: `cargo test -p xtask info_plist`
Expected: compile error — `info_plist` not found.

- [ ] **Step 4: Implement bundling and the DMG**

**File:** `crates/xtask/src/bundle.rs` (implementation part)
```rust
//! `cargo xtask bundle`: builds the release binary (universal by default),
//! converts the soundpacks, and assembles and signs `target/bundle/Rustyvibes.app`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const APP_NAME: &str = "Rustyvibes";
pub const BUNDLE_ID: &str = "io.github.kb24x7.rustyvibes";
pub const MIN_MACOS: &str = "13.0";
const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

pub struct Options {
    /// Build for Apple Silicon and Intel (default) instead of the host only.
    pub universal: bool,
    /// Sign ad-hoc even when a Developer ID identity is available.
    pub adhoc: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Options {
        Options {
            universal: !args.iter().any(|a| a == "--native"),
            adhoc: args.iter().any(|a| a == "--adhoc"),
        }
    }
}

pub struct Bundle {
    pub app: PathBuf,
    /// The Developer ID used, or `None` for an ad-hoc signature.
    pub identity: Option<String>,
}

/// `cargo xtask bundle [--native] [--adhoc]`
pub fn run(root: &Path, args: &[String]) -> crate::Result<()> {
    bundle(root, &Options::parse(args)).map(|_| ())
}

pub fn bundle(root: &Path, options: &Options) -> crate::Result<Bundle> {
    let version = env!("CARGO_PKG_VERSION");
    let binaries = compile(root, options.universal)?;
    let packs_dir = root.join("target/packs");
    let reports = crate::packs::build_all(&root.join("assets/soundpacks/catalog.json"), &packs_dir)?;

    let app = root.join("target/bundle").join(format!("{APP_NAME}.app"));
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|e| format!("{}: {e}", app.display()))?;
    }
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    for dir in [contents.join("MacOS"), resources.join("Packs"), resources.join("Licenses")] {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let exe = contents.join("MacOS/rustyvibes");
    if let [only] = binaries.as_slice() {
        copy(only, &exe)?;
    } else {
        run_cmd(Command::new("lipo").arg("-create").arg("-output").arg(&exe).args(&binaries))?;
    }
    write(&contents.join("Info.plist"), &info_plist(version))?;
    write(&contents.join("PkgInfo"), "APPL????")?;
    copy(&root.join("assets/icon/AppIcon.icns"), &resources.join("AppIcon.icns"))?;
    for report in &reports {
        let name = format!("{}.rvpack", report.id);
        copy(&packs_dir.join(&name), &resources.join("Packs").join(&name))?;
    }
    for (source, name) in [
        ("LICENSE", "Rustyvibes.txt"),
        ("THIRD_PARTY_NOTICES.md", "Third-Party Notices.txt"),
        ("assets/soundpacks/mechvibes/LICENSE", "Mechvibes.txt"),
        ("assets/soundpacks/kbsim/LICENSE.md", "kbsim.txt"),
    ] {
        copy(&root.join(source), &resources.join("Licenses").join(name))?;
    }

    let wanted = if options.adhoc { None } else { developer_id() };
    let identity = sign(&app, wanted.as_deref())?;
    run_cmd(Command::new("codesign").args(["--verify", "--strict", "--verbose=2"]).arg(&app))?;

    let binary = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
    println!(
        "{} ({}): binary {:.2} MB, bundle {:.1} MB, signed {}",
        app.display(),
        if binaries.len() > 1 { "universal" } else { "host architecture" },
        binary as f64 / 1e6,
        dir_size(&app) as f64 / 1e6,
        identity.as_deref().map_or_else(|| "ad-hoc".to_owned(), |id| format!("with Developer ID {id}")),
    );
    Ok(Bundle { app, identity })
}

/// Builds `rustyvibes` in release mode for each target and returns the binaries.
fn compile(root: &Path, universal: bool) -> crate::Result<Vec<PathBuf>> {
    let host = if cfg!(target_arch = "aarch64") { TARGETS[0] } else { TARGETS[1] };
    let targets: Vec<&str> = if universal { TARGETS.to_vec() } else { vec![host] };
    if universal {
        let installed = output(Command::new("rustup").args(["target", "list", "--installed"]))?;
        for target in &targets {
            if !installed.lines().any(|line| line.trim() == *target) {
                run_cmd(Command::new("rustup").args(["target", "add", target]))?;
            }
        }
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut binaries = Vec::new();
    for target in targets {
        run_cmd(
            Command::new(&cargo)
                .current_dir(root)
                .env("MACOSX_DEPLOYMENT_TARGET", MIN_MACOS)
                .args(["build", "--release", "--locked", "-p", "rustyvibes", "--target", target]),
        )?;
        binaries.push(root.join("target").join(target).join("release/rustyvibes"));
    }
    Ok(binaries)
}

pub fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>{APP_NAME}</string>
	<key>CFBundleExecutable</key>
	<string>rustyvibes</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{APP_NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.utilities</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MIN_MACOS}</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>© 2021–2026 Kunal Bagaria · MIT License</string>
	<key>NSSupportsAutomaticTermination</key>
	<false/>
	<key>NSSupportsSuddenTermination</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// SHA-1 of the first "Developer ID Application" signing identity, if any.
fn developer_id() -> Option<String> {
    let listing = output(Command::new("security").args(["find-identity", "-v", "-p", "codesigning"])).ok()?;
    listing
        .lines()
        .find(|line| line.contains("\"Developer ID Application:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .map(str::to_owned)
}

/// Signs with `identity` (hardened runtime, secure timestamp), falling back to an
/// ad-hoc signature if that fails or hangs (e.g. a locked keychain).
fn sign(app: &Path, identity: Option<&str>) -> crate::Result<Option<String>> {
    if let Some(identity) = identity {
        let mut cmd = Command::new("codesign");
        cmd.args(["--force", "--options", "runtime", "--timestamp", "--sign", identity]).arg(app);
        match run_with_timeout(&mut cmd, Duration::from_secs(120)) {
            Ok(()) => return Ok(Some(identity.to_owned())),
            Err(e) => eprintln!("warning: Developer ID signing failed ({e}); signing ad-hoc instead"),
        }
    }
    run_cmd(Command::new("codesign").args(["--force", "--sign", "-"]).arg(app))?;
    Ok(None)
}

pub(crate) fn run_cmd(cmd: &mut Command) -> crate::Result<()> {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{cmd:?} failed with {status}")) }
}

fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> crate::Result<()> {
    let mut child = cmd.stdin(Stdio::null()).spawn().map_err(|e| format!("{cmd:?}: {e}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() { Ok(()) } else { Err(format!("exit status {status}")) };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(format!("timed out after {} s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn output(cmd: &mut Command) -> crate::Result<String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd:?} failed with {}", out.status));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

pub(crate) fn copy(from: &Path, to: &Path) -> crate::Result<()> {
    std::fs::copy(from, to).map(|_| ()).map_err(|e| format!("copy {} → {}: {e}", from.display(), to.display()))
}

fn write(path: &Path, text: &str) -> crate::Result<()> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| {
                    let p = e.path();
                    if p.is_dir() { dir_size(&p) } else { e.metadata().map_or(0, |m| m.len()) }
                })
                .sum()
        })
        .unwrap_or(0)
}
```

**File:** `crates/xtask/src/dmg.rs`
```rust
//! `cargo xtask dmg`: bundles the app, then wraps it in a compressed disk image
//! with an Applications shortcut, signed like the app.

use std::path::Path;
use std::process::Command;

use crate::bundle::{self, Options, run_cmd};

pub fn run(root: &Path, args: &[String]) -> crate::Result<()> {
    let built = bundle::bundle(root, &Options::parse(args))?;
    let version = env!("CARGO_PKG_VERSION");
    let staging = root.join("target/dmg");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    run_cmd(Command::new("ditto").arg(&built.app).arg(staging.join(format!("{}.app", bundle::APP_NAME))))?;
    std::os::unix::fs::symlink("/Applications", staging.join("Applications"))
        .map_err(|e| format!("Applications link: {e}"))?;

    let dmg = root.join("target").join(format!("{}-{version}.dmg", bundle::APP_NAME));
    if dmg.exists() {
        std::fs::remove_file(&dmg).map_err(|e| format!("{}: {e}", dmg.display()))?;
    }
    run_cmd(
        Command::new("hdiutil")
            .args(["create", "-quiet", "-volname", bundle::APP_NAME, "-fs", "APFS", "-format", "ULFO", "-srcfolder"])
            .arg(&staging)
            .arg(&dmg),
    )?;
    if let Some(identity) = &built.identity {
        run_cmd(Command::new("codesign").args(["--force", "--timestamp", "--sign", identity]).arg(&dmg))?;
    }
    let size = std::fs::metadata(&dmg).map(|m| m.len()).unwrap_or(0);
    println!("{} ({:.1} MB)", dmg.display(), size as f64 / 1e6);
    Ok(())
}
```

**File:** `crates/xtask/src/main.rs`
```rust
//! Build tooling for Rustyvibes: `cargo xtask <command>`.

mod bundle;
mod catalog;
mod decode;
mod dmg;
mod dsp;
mod packs;
mod sources;
#[cfg(test)]
mod testutil;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Errors are human-readable strings; this is a build tool.
pub type Result<T, E = String> = std::result::Result<T, E>;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  packs               convert assets/soundpacks into target/packs/*.rvpack
  bundle [options]    build and sign target/bundle/Rustyvibes.app
  dmg [options]       bundle, then create target/Rustyvibes-<version>.dmg
  probe <file>...     show how the clean-up pipeline sees audio files

options:
  --native            build for this Mac's architecture only (default: universal)
  --adhoc             sign ad-hoc even if a Developer ID is available";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("packs") => packs::run(&root),
        Some("bundle") => bundle::run(&root, rest),
        Some("dmg") => dmg::run(&root, rest),
        Some("probe") => probe(rest),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The repository root (this crate lives in `crates/xtask`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels below the workspace root")
        .to_path_buf()
}

/// Prints how the clean-up pipeline sees each file.
fn probe(files: &[String]) -> Result<()> {
    for file in files {
        let mut audio = decode::decode_file(Path::new(file))?;
        dsp::dc_block(&mut audio.samples, audio.rate);
        let ms = |n: usize| n as f32 * 1000.0 / audio.rate as f32;
        let Some(trimmed) = dsp::trim_clip(&audio.samples, audio.rate, &dsp::TrimParams::default())
        else {
            println!("{file}: silent");
            continue;
        };
        let mut clips = vec![trimmed];
        let gain = dsp::normalize(&mut clips, &[true], audio.rate, dsp::TARGET_DB, dsp::CEILING_DB);
        let quantised = dsp::to_i16(&clips[0], &mut 1);
        let peak = quantised.iter().map(|s| i32::from(*s).abs()).max().unwrap_or(0);
        println!(
            "{file}: {} Hz, {:.1} ms → {:.1} ms trimmed, gain {gain:+.1} dB, i16 peak {peak}",
            audio.rate,
            ms(audio.samples.len()),
            ms(clips[0].len()),
        );
    }
    Ok(())
}
```

**File:** `LICENSE`
```
MIT License

Copyright (c) 2021–2026 Kunal Bagaria

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

`THIRD_PARTY_NOTICES.md`: generate the crate list with
`cargo tree -p rustyvibes -e normal --prefix none --format "{p} {l}" | sort -u`
and write a file with one section per runtime crate (name, version, licence) followed by the full MIT texts for Mechvibes (`assets/soundpacks/mechvibes/LICENSE`) and kbsim (`assets/soundpacks/kbsim/LICENSE.md`), and the standard MIT / Apache-2.0 / Zlib licence texts referenced by the crates.

- [ ] **Step 5: Build, bundle and verify**

Run: `cargo test -p xtask && cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check`
Expected: pass.

Run: `cargo xtask bundle`
Expected: ends with `…/target/bundle/Rustyvibes.app (universal): binary … MB, bundle … MB, signed with Developer ID …` (or `ad-hoc` on machines without one).

Run:
```bash
plutil -lint target/bundle/Rustyvibes.app/Contents/Info.plist
lipo -archs target/bundle/Rustyvibes.app/Contents/MacOS/rustyvibes      # x86_64 arm64
codesign -dv --verbose=2 target/bundle/Rustyvibes.app 2>&1 | grep -E "Identifier|TeamIdentifier|Runtime"
ls target/bundle/Rustyvibes.app/Contents/Resources/Packs | wc -l        # 21
RUSTYVIBES_LOG=1 target/bundle/Rustyvibes.app/Contents/MacOS/rustyvibes --selftest
```
Expected: plist OK; both architectures; identifier `io.github.kb24x7.rustyvibes`; 21 packs; the selftest loads packs from `Contents/Resources/Packs` and passes.

Run: `open target/bundle/Rustyvibes.app; sleep 3; pgrep -x rustyvibes; osascript -e 'tell application id "io.github.kb24x7.rustyvibes" to quit'`
Expected: a PID is printed (the app launched from Finder/LaunchServices; it shows the onboarding window because this bundle identity has no Input Monitoring grant yet), then it quits.

Run: `cargo xtask dmg`
Expected: `…/target/Rustyvibes-2.0.0.dmg (… MB)`; `hdiutil verify target/Rustyvibes-2.0.0.dmg` reports the checksum is valid.

- [ ] **Step 6: Commit**

```bash
git add scripts assets/icon crates/xtask LICENSE THIRD_PARTY_NOTICES.md
git commit -m "feat(xtask): app icon, signed universal .app bundle and DMG

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: Measure, document, review

**Files:**
- Create: `README.md`, `CHANGELOG.md`
- Modify: anything the review finds

**Interfaces:**
- Consumes: the finished app and bundle.
- Produces: measured performance numbers, user documentation, a reviewed codebase.

- [ ] **Step 1: Measure the success criteria**

Run each and record the numbers:
```bash
# Binary and bundle size
stat -f %z target/bundle/Rustyvibes.app/Contents/MacOS/rustyvibes
du -sh target/bundle/Rustyvibes.app target/Rustyvibes-2.0.0.dmg

# Launch from the terminal (inherits Input Monitoring), let it settle, then sample.
target/bundle/Rustyvibes.app/Contents/MacOS/rustyvibes & APP=$!
sleep 15
ps -o %cpu=,rss= -p $APP                       # idle CPU and RSS
footprint $APP | grep -i "phys_footprint"      # physical footprint
top -l 6 -s 5 -pid $APP -stats pid,cpu,idlew,power | tail -3   # idle wakeups
# Type-equivalent burst: inject Shift events, confirm audio starts, then stops after 8 s.
target/release/rustyvibes --tap-test
kill $APP
```
Also run `cargo run --release -p rustyvibes -- --bench` and `--selftest` for render-callback timings.

Expected (spec targets): idle CPU 0.0%, footprint < 25 MB, idle wakeups ~0/s after the 8 s audio timeout, binary < 3 MB, tap callback < 20 µs, worst render callback ≪ 2.7 ms.

- [ ] **Step 2: Write the README**

`README.md` covers, in this order: one-paragraph pitch; screenshot of the icon (`assets/icon/AppIcon-1024.png`); install (download the DMG, drag to Applications, first launch → grant Input Monitoring; note on unnotarised builds: right-click → Open); features (21 soundpacks grouped by Linear/Tactile/Clicky, key release sounds, natural variation, spatial stereo, launch at login, privacy); a performance table with the Step 1 measurements next to the spec targets; how it works (threads diagram from the spec, `.rvpack`, build-time clean-up); building from source (`cargo xtask packs`, `cargo run --release -p rustyvibes`, `cargo xtask bundle`, `cargo xtask dmg`, notarisation command: `xcrun notarytool submit target/Rustyvibes-2.0.0.dmg --keychain-profile <profile> --wait && xcrun stapler staple target/Rustyvibes-2.0.0.dmg`); diagnostics flags; troubleshooting (no sound: check the switch, volume, Input Monitoring, Secure Keyboard Entry in terminals and password fields blocks key events by design); credits and licences.

`CHANGELOG.md`: a `2.0.0 — 2026-10-02` entry summarising the rewrite (CLI → native menu bar app, bundled packs, new engine) and what changed from 1.x (soundpack path argument and `-v` flag removed; Linux/Windows support dropped).

- [ ] **Step 3: Whole-branch review**

Use superpowers:requesting-code-review: dispatch a reviewer on the full diff from the first commit, with the spec and this plan as context, focusing on: real-time safety of the render and tap callbacks, `unsafe` blocks and FFI signatures, objc2 memory management, the audio start/stop race, permission flow edge cases, and the Review Focus list above. Fix confirmed findings with tests where possible, re-run `cargo test --workspace`, clippy, fmt, `cargo xtask bundle`, and `--selftest`/`--tap-test`.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: README, changelog and measured performance

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
