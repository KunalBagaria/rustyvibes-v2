//! Builds the status-item menu and keeps it in sync with the settings.

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{AnyThread, MainThreadMarker, MainThreadOnly, available, sel};
use objc2_app_kit::{
    NSControlStateValue, NSControlStateValueOff, NSControlStateValueOn, NSFont,
    NSFontAttributeName, NSMenu, NSMenuItem,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};

use super::{icons, views};
use crate::packs::Library;
use crate::settings::Settings;

/// The menu plus the items that change after it is built (the menu retains
/// everything else).
pub struct Menu {
    pub menu: Retained<NSMenu>,
    /// Separator and "Allow Keyboard Access…" row.
    permission: [Retained<NSMenuItem>; 2],
    current_pack: Retained<NSMenuItem>,
    pack_items: Vec<Retained<NSMenuItem>>,
    release_item: Retained<NSMenuItem>,
}

impl Menu {
    pub fn build(
        mtm: MainThreadMarker,
        target: &AnyObject,
        library: &Library,
        settings: &Settings,
        active: usize,
        launch_at_login: bool,
    ) -> Menu {
        let menu = NSMenu::new(mtm);
        menu.setAutoenablesItems(false);

        let header = NSMenuItem::new(mtm);
        header.setView(Some(&views::header(mtm, target, settings.enabled)));
        menu.addItem(&header);

        let permission_separator = NSMenuItem::separatorItem(mtm);
        let permission_item =
            item(mtm, "Allow Keyboard Access…", target, sel!(showPermissionHelp:));
        permission_item
            .setImage(Some(&icons::symbol("exclamationmark.triangle.fill", "Needs permission")));
        menu.addItem(&permission_separator);
        menu.addItem(&permission_item);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&section_header(mtm, "Soundpack"));
        let submenu = NSMenu::new(mtm);
        submenu.setAutoenablesItems(false);
        let mut pack_items = Vec::with_capacity(library.len());
        let mut category: Option<&str> = None;
        for (index, pack) in library.iter().enumerate() {
            if category != Some(pack.meta.category.as_str()) {
                if category.is_some() {
                    submenu.addItem(&NSMenuItem::separatorItem(mtm));
                }
                submenu.addItem(&section_header(mtm, category_title(&pack.meta.category)));
                category = Some(pack.meta.category.as_str());
            }
            let row = item(mtm, &pack.display_name(), target, sel!(selectPack:));
            row.setImage(Some(&icons::keycap(&pack.meta.color)));
            row.setTag(index as isize);
            submenu.addItem(&row);
            pack_items.push(row);
        }
        if library.is_empty() {
            let none = NSMenuItem::new(mtm);
            none.setTitle(&NSString::from_str("No soundpacks found"));
            none.setEnabled(false);
            submenu.addItem(&none);
        }
        let current_pack = NSMenuItem::new(mtm);
        current_pack.setSubmenu(Some(&submenu));
        menu.addItem(&current_pack);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&section_header(mtm, "Volume"));
        let volume = NSMenuItem::new(mtm);
        volume.setView(Some(&views::volume(mtm, target, settings.volume)));
        menu.addItem(&volume);

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        let release_item = toggle(
            mtm,
            "Key Release Sounds",
            target,
            sel!(toggleReleaseSounds:),
            settings.release_sounds,
            "Play the upstroke when a key is released (soundpacks recorded with release sounds)",
        );
        menu.addItem(&release_item);
        menu.addItem(&toggle(
            mtm,
            "Natural Variation",
            target,
            sel!(toggleVariation:),
            settings.variation,
            "Vary the pitch and loudness of each keystroke slightly, like a real keyboard",
        ));
        menu.addItem(&toggle(
            mtm,
            "Spatial Stereo",
            target,
            sel!(toggleSpatial:),
            settings.spatial,
            "Place each key's sound where the key sits on the keyboard",
        ));
        menu.addItem(&toggle(
            mtm,
            "Launch at Login",
            target,
            sel!(toggleLaunchAtLogin:),
            launch_at_login,
            "Start Rustyvibes automatically when you log in",
        ));

        menu.addItem(&NSMenuItem::separatorItem(mtm));
        menu.addItem(&item(mtm, "About Rustyvibes", target, sel!(showAbout:)));
        // SAFETY: `terminate:` is implemented by NSApplication; a nil target reaches it
        // through the responder chain.
        let quit = unsafe {
            NSMenuItem::initWithTitle_action_keyEquivalent(
                NSMenuItem::alloc(mtm),
                &NSString::from_str("Quit Rustyvibes"),
                Some(sel!(terminate:)),
                &NSString::from_str("q"),
            )
        };
        menu.addItem(&quit);

        let menu = Menu {
            menu,
            permission: [permission_separator, permission_item],
            current_pack,
            pack_items,
            release_item,
        };
        menu.show_active_pack(library, active);
        menu
    }

    /// Checks the active pack, names it in the main menu, and enables the
    /// release-sounds toggle only for packs recorded with release sounds.
    pub fn show_active_pack(&self, library: &Library, active: usize) {
        for (index, row) in self.pack_items.iter().enumerate() {
            row.setState(control_state(index == active));
        }
        match library.get(active) {
            Some(pack) => {
                self.current_pack.setTitle(&NSString::from_str(&pack.display_name()));
                self.current_pack.setImage(Some(&icons::keycap(&pack.meta.color)));
                self.release_item.setEnabled(pack.has_release);
            }
            None => {
                self.current_pack.setTitle(&NSString::from_str("No soundpacks found"));
                self.release_item.setEnabled(false);
            }
        }
    }

    /// Shows or hides the "Allow Keyboard Access…" row.
    pub fn show_permission_needed(&self, needed: bool) {
        for item in &self.permission {
            item.setHidden(!needed);
        }
    }
}

pub fn control_state(on: bool) -> NSControlStateValue {
    if on { NSControlStateValueOn } else { NSControlStateValueOff }
}

fn item(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
) -> Retained<NSMenuItem> {
    // SAFETY: every action passed here is implemented by `target`, the app delegate.
    unsafe {
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(action),
            &NSString::new(),
        );
        item.setTarget(Some(target));
        item
    }
}

fn toggle(
    mtm: MainThreadMarker,
    title: &str,
    target: &AnyObject,
    action: Sel,
    on: bool,
    tooltip: &str,
) -> Retained<NSMenuItem> {
    let item = item(mtm, title, target, action);
    item.setState(control_state(on));
    item.setToolTip(Some(&NSString::from_str(tooltip)));
    item
}

/// A section title: native on macOS 14+, a disabled bold item on macOS 13.
fn section_header(mtm: MainThreadMarker, title: &str) -> Retained<NSMenuItem> {
    if available!(macos = 14.0) {
        return NSMenuItem::sectionHeaderWithTitle(&NSString::from_str(title), mtm);
    }
    let font = NSFont::boldSystemFontOfSize(11.0);
    // SAFETY: NSFontAttributeName is an immutable framework constant.
    let key: &NSString = unsafe { NSFontAttributeName };
    let value: &AnyObject = &font;
    let attributes = NSDictionary::from_slices(&[key], &[value]);
    // SAFETY: the attributes dictionary maps a valid key to an NSFont.
    let text = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(title),
            Some(&attributes),
        )
    };
    let item = NSMenuItem::new(mtm);
    item.setAttributedTitle(Some(&text));
    item.setEnabled(false);
    item
}

fn category_title(category: &str) -> &'static str {
    match category {
        "linear" => "Linear",
        "tactile" => "Tactile",
        "clicky" => "Clicky",
        _ => "Other",
    }
}
