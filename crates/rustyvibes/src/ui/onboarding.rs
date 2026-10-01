//! The keyboard-access window shown until Input Monitoring is granted.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, available, sel};
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
                NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::FullSizeContentView,
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
        let title =
            label(mtm, "Let Rustyvibes hear your keys", &NSFont::boldSystemFontOfSize(20.0));
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
        let buttons =
            stack(mtm, &[&secondary, &primary], NSUserInterfaceLayoutOrientation::Horizontal, 12.0);
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

fn paragraph(
    mtm: MainThreadMarker,
    text: &str,
    size: f64,
    color: &NSColor,
) -> Retained<NSTextField> {
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
