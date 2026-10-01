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
        let state =
            Box::into_raw(Box::new(RenderState { shared, mixer: Mixer::new(48_000.0), inputs }));
        Control { shared, state, unit: None, running: false, rate: 48_000.0 }
    }

    fn run(mut self) -> ! {
        // Create and initialise the unit up front (the first instantiation takes
        // ~100 ms), so a keystroke only pays for `AudioOutputUnitStart`.
        self.prepare();
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

    /// Creates the unit if needed and matches it to the current output device.
    /// Called only while the unit is stopped.
    fn prepare(&mut self) -> bool {
        if self.unit.is_none() {
            match Unit::new(self.state.cast()) {
                Ok(unit) => self.unit = Some(unit),
                Err(status) => {
                    log!("audio: cannot create the output unit (OSStatus {status})");
                    return false;
                }
            }
        }
        let Some(unit) = self.unit.as_mut() else { return false };
        match unit.configure() {
            Ok(rate) => {
                // SAFETY: the unit is stopped, so the IO thread is not using the state.
                unsafe { (*self.state).mixer.set_out_rate(rate) };
                self.rate = rate;
                true
            }
            Err(status) => {
                log!("audio: cannot configure the output (OSStatus {status})");
                false
            }
        }
    }

    fn start(&mut self) {
        let started = Instant::now();
        if !self.prepare() {
            return;
        }
        let Some(unit) = self.unit.as_ref() else { return };
        self.shared.silent_frames.store(0, Ordering::Relaxed);
        let status = unit.start();
        if status != 0 {
            log!("audio: start failed (OSStatus {status})");
            return;
        }
        self.running = true;
        self.shared.audio_running.store(true, Ordering::SeqCst);
        self.shared.stats.starts.fetch_add(1, Ordering::Relaxed);
        log!(
            "audio: started at {} Hz in {:.1} ms",
            self.rate,
            started.elapsed().as_secs_f64() * 1e3
        );
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
            self.set(
                ffi::kAudioUnitProperty_MaximumFramesPerSlice,
                ffi::kAudioUnitScope_Global,
                &MAX_FRAMES_PER_SLICE,
            )?;
            // SAFETY: the unit is configured and uninitialised.
            check(unsafe { ffi::AudioUnitInitialize(self.au) })?;
            self.initialized = true;
            self.rate = rate;
        }
        // Best effort, and only when needed: changing the buffer size makes the
        // device reconfigure, which would add tens of milliseconds to every start.
        if self.get::<u32>(ffi::kAudioDevicePropertyBufferFrameSize) != Some(BUFFER_FRAMES) {
            let _ = self.set(
                ffi::kAudioDevicePropertyBufferFrameSize,
                ffi::kAudioUnitScope_Global,
                &BUFFER_FRAMES,
            );
        }
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

    /// Reads a global-scope property of plain-old-data type `T`.
    fn get<T: Default>(&self, id: u32) -> Option<T> {
        let mut value = T::default();
        let mut size = std::mem::size_of::<T>() as u32;
        // SAFETY: `value` is a valid out-pointer of `size` bytes.
        let status = unsafe {
            ffi::AudioUnitGetProperty(
                self.au,
                id,
                ffi::kAudioUnitScope_Global,
                0,
                (&raw mut value).cast(),
                &mut size,
            )
        };
        (status == 0).then_some(value)
    }

    fn set<T>(&self, id: u32, scope: u32, value: &T) -> Result<(), ffi::OSStatus> {
        // SAFETY: `value` points to a live `T` of exactly the size passed.
        check(unsafe {
            ffi::AudioUnitSetProperty(
                self.au,
                id,
                scope,
                0,
                (value as *const T).cast(),
                std::mem::size_of::<T>() as u32,
            )
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
