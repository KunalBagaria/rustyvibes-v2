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
        Voicer {
            shared,
            library,
            producer,
            kick,
            rng: Rng::new(seed),
            pan,
            last_choice: [u16::MAX; 128],
        }
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
        assert_eq!(
            (p.pitch, p.gain_l, p.gain_r, p.delay_us, p.src_rate),
            (1.0, 1.0, 1.0, 0, 44_100)
        );
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
        assert_eq!(
            rx.pop().unwrap().samples.len(),
            rvpack::GUARD_BEFORE + 32 + rvpack::GUARD_AFTER
        );
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
