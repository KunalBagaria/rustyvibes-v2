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
        let menu = Menu::build(
            mtm,
            target,
            runtime.library,
            &settings,
            runtime.active_pack(),
            login::is_enabled(),
        );
        let status_item =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSVariableStatusItemLength);
        status_item.setMenu(Some(&menu.menu));
        let _ = self.ivars().state.set(RefCell::new(State { runtime, store, status_item, menu }));
        let listening = self.start_input();
        self.refresh_status();
        if !listening || std::env::var_os("RUSTYVIBES_SHOW_ONBOARDING").is_some() {
            self.show_onboarding();
        }
        log!(
            "launched; keyboard {}",
            if listening { "connected" } else { "waiting for permission" }
        );
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
                state.runtime.return_input(*voicer);
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
        log!(
            "permission granted; keyboard {}",
            if listening { "connected" } else { "needs a relaunch" }
        );
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
