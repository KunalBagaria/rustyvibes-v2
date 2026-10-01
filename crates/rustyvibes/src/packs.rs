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
        libc::mmap(
            std::ptr::null_mut(),
            len,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
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
        let out =
            unsafe { std::slice::from_raw_parts_mut(words.as_mut_ptr().cast::<u8>(), bytes.len()) };
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
