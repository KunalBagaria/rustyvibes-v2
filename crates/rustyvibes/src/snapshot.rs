//! `--snapshot [dir]`: renders the custom views, icons and the onboarding
//! window to PNG in light and dark appearance, for visual review without
//! screen-recording permission. Backgrounds are transparent.

use std::path::Path;

use objc2::rc::Retained;
use objc2::runtime::NSObject;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSAppearance, NSAppearanceCustomization, NSAppearanceNameAqua, NSAppearanceNameDarkAqua,
    NSApplication, NSApplicationActivationPolicy, NSBitmapImageFileType, NSImageView, NSView,
};
use objc2_foundation::{NSDictionary, NSString};

use crate::packs::{Library, default_dir};
use crate::ui::icons::{self, StatusIcon, rect};
use crate::ui::onboarding::Onboarding;
use crate::ui::views;

pub fn snapshot(dir: &Path) -> Result<(), String> {
    let mtm = MainThreadMarker::new().ok_or("must run on the main thread")?;
    NSApplication::sharedApplication(mtm)
        .setActivationPolicy(NSApplicationActivationPolicy::Accessory);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let library = Library::load_dir(&default_dir());
    // Actions are never sent while rendering, so any object can be the target.
    let target = NSObject::new();
    // SAFETY: framework constants.
    let appearances =
        unsafe { [("light", NSAppearanceNameAqua), ("dark", NSAppearanceNameDarkAqua)] };
    for (suffix, name) in appearances {
        let appearance = NSAppearance::appearanceNamed(name).ok_or("missing appearance")?;
        let file = |what: &str| dir.join(format!("{what}-{suffix}.png"));
        render(&views::header(mtm, &target, true), &appearance, &file("header"))?;
        render(&views::volume(mtm, &target, 0.6), &appearance, &file("volume"))?;
        render(&icon_strip(mtm, &library), &appearance, &file("icons"))?;
        let onboarding = Onboarding::new(mtm, &target);
        let content = onboarding.window.contentView().ok_or("onboarding has no content")?;
        render(&content, &appearance, &file("onboarding"))?;
    }
    println!("snapshots written to {}", dir.display());
    Ok(())
}

/// Both status icons followed by every pack's keycap glyph.
fn icon_strip(mtm: MainThreadMarker, library: &Library) -> Retained<NSView> {
    let mut images = vec![icons::status(StatusIcon::Normal), icons::status(StatusIcon::Attention)];
    images.extend(library.iter().map(|p| icons::keycap(&p.meta.color)));
    let cell = 32.0;
    let view =
        NSView::initWithFrame(NSView::alloc(mtm), rect(0.0, 0.0, cell * images.len() as f64, cell));
    for (i, image) in images.iter().enumerate() {
        let image_view = NSImageView::imageViewWithImage(image, mtm);
        image_view.setFrame(rect(i as f64 * cell + 7.0, 7.0, 18.0, 18.0));
        view.addSubview(&image_view);
    }
    view
}

fn render(view: &NSView, appearance: &NSAppearance, path: &Path) -> Result<(), String> {
    view.setAppearance(Some(appearance));
    view.layoutSubtreeIfNeeded();
    let bounds = view.bounds();
    let bitmap =
        view.bitmapImageRepForCachingDisplayInRect(bounds).ok_or("cannot allocate a bitmap")?;
    view.cacheDisplayInRect_toBitmapImageRep(bounds, &bitmap);
    // SAFETY: an empty properties dictionary is valid for PNG encoding.
    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("PNG encoding failed")?;
    if png.writeToFile_atomically(&NSString::from_str(&path.to_string_lossy()), true) {
        Ok(())
    } else {
        Err(format!("cannot write {}", path.display()))
    }
}
