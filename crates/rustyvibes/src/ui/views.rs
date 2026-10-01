//! Custom views embedded in the menu: the header with the sounds switch and
//! the volume slider row.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{MainThreadMarker, MainThreadOnly, sel};
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
    let loud =
        NSImageView::imageViewWithImage(&icons::symbol("speaker.wave.3.fill", "Louder"), mtm);
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
