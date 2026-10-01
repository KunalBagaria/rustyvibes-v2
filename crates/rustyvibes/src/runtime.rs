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
        let library: &'static Library =
            Box::leak(Box::new(Library::load_dir(&packs::default_dir())));
        let pack = library.resolve(settings.pack.as_deref());
        library.make_resident(pack);
        let shared: &'static Shared = Box::leak(Box::new(Shared::new(settings, pack)));
        let (input_tx, input_rx) = ring(RING_CAPACITY);
        let (preview_tx, preview_rx) = ring(RING_CAPACITY);
        let kick = audio::spawn(shared, vec![input_rx, preview_rx]);
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.subsec_nanos() | 1);
        let preview = Voicer::new(shared, library, preview_tx, kick.clone(), seed);
        let input =
            Voicer::new(shared, library, input_tx, kick, seed.rotate_left(16) ^ 0x9E37_79B9);
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
