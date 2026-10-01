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
        let meta_end =
            meta_offset.checked_add(meta_len).ok_or(FormatError::OutOfBounds("metadata"))?;
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
        assert_eq!(
            PackView::parse(Aligned::new(&bytes).bytes()).unwrap_err(),
            FormatError::BadMagic
        );

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
