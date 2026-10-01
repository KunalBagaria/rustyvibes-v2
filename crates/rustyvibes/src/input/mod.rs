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
pub fn start(voicer: Voicer) -> Result<(), Box<Voicer>> {
    let (tx, rx) = std::sync::mpsc::channel::<Result<(), Voicer>>();
    std::thread::Builder::new()
        .name("rustyvibes-input".into())
        .spawn(move || {
            // SAFETY: adjusts the QoS of the current thread only.
            unsafe {
                libc::pthread_set_qos_class_self_np(
                    libc::qos_class_t::QOS_CLASS_USER_INTERACTIVE,
                    0,
                );
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
                ffi::CFRunLoopAddSource(
                    ffi::CFRunLoopGetCurrent(),
                    source,
                    ffi::kCFRunLoopCommonModes,
                );
                ffi::CGEventTapEnable(tap, true);
            }
            let _ = tx.send(Ok(()));
            log!("input: event tap running");
            // SAFETY: runs this thread's run loop forever.
            unsafe { ffi::CFRunLoopRun() };
        })
        .expect("failed to spawn the input thread");
    rx.recv().expect("the input thread exited before reporting").map_err(Box::new)
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
    let keycode = || {
        u16::try_from(unsafe {
            ffi::CGEventGetIntegerValueField(event, ffi::kCGKeyboardEventKeycode)
        })
        .unwrap_or(u16::MAX)
    };
    let transition = match kind {
        ffi::kCGEventTapDisabledByTimeout | ffi::kCGEventTapDisabledByUserInput => {
            // SAFETY: `state.tap` is the live tap.
            unsafe { ffi::CGEventTapEnable(state.tap, true) };
            None
        }
        ffi::kCGEventKeyDown => {
            let repeat =
                unsafe { ffi::CGEventGetIntegerValueField(event, ffi::kCGKeyboardEventAutorepeat) };
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
