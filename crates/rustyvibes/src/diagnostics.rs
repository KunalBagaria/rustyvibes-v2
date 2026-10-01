//! Command-line diagnostics for development and support.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use crate::audio;
use crate::engine::{Play, Shared, mixer::Mixer};
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
    // Give the audio thread a moment to prepare the output unit, as a real launch would.
    std::thread::sleep(Duration::from_millis(300));
    let first = time_to_audio(shared, || rt.preview_click())?;
    println!("first sound after launch: {:.1} ms to the first audio callback", ms(first));
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
    let again = time_to_audio(shared, || rt.preview_click())?;
    println!("first sound after the idle stop: {:.1} ms to the first audio callback", ms(again));
    Ok(())
}

/// Runs `queue` and measures how long until the next render callback.
fn time_to_audio(shared: &Shared, queue: impl FnOnce()) -> Result<Duration, String> {
    let callbacks = shared.stats.callbacks.load(Ordering::Relaxed);
    let started = Instant::now();
    queue();
    while shared.stats.callbacks.load(Ordering::Relaxed) == callbacks {
        if started.elapsed() > Duration::from_secs(2) {
            return Err("no audio callback within 2 s".into());
        }
        std::thread::yield_now();
    }
    Ok(started.elapsed())
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1e3
}

/// `--bench`: mixer cost per 128-frame block on the interpolating path.
pub fn bench() {
    let clip: &'static [i16] = Box::leak(
        (0..13_230)
            .map(|i| ((i as f32 * 0.07).sin() * 12_000.0) as i16)
            .collect::<Vec<_>>()
            .into_boxed_slice(),
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
                    mixer.start(&Play {
                        samples: clip,
                        src_rate: 44_100,
                        pitch,
                        gain_l: 0.5,
                        gain_r: 0.5,
                        delay_us: 0,
                    });
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
