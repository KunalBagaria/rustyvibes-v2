//! The standard About panel, with credits for every soundpack source.

use std::collections::BTreeMap;

use objc2::runtime::AnyObject;
use objc2::{AnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSAboutPanelOptionCredits, NSApplication, NSColor, NSFont, NSFontAttributeName,
    NSForegroundColorAttributeName, NSMutableParagraphStyle, NSParagraphStyleAttributeName,
    NSTextAlignment,
};
use objc2_foundation::{NSAttributedString, NSDictionary, NSString};

use super::onboarding::activate;
use crate::packs::Library;

pub fn show(mtm: MainThreadMarker, library: &Library) {
    activate(mtm);
    let font = NSFont::systemFontOfSize(11.0);
    let color = NSColor::secondaryLabelColor();
    let paragraph = NSMutableParagraphStyle::new();
    paragraph.setAlignment(NSTextAlignment::Center);
    // SAFETY: framework constants.
    let keys: [&NSString; 3] = unsafe {
        [NSFontAttributeName, NSForegroundColorAttributeName, NSParagraphStyleAttributeName]
    };
    let values: [&AnyObject; 3] = [&font, &color, &paragraph];
    let attributes = NSDictionary::from_slices(&keys, &values);
    // SAFETY: the attribute values have the types AppKit expects for these keys.
    let credits = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(&credits_text(library)),
            Some(&attributes),
        )
    };
    // SAFETY: framework constant.
    let option: &NSString = unsafe { NSAboutPanelOptionCredits };
    let value: &AnyObject = &credits;
    let options = NSDictionary::from_slices(&[option], &[value]);
    // SAFETY: the credits option takes an NSAttributedString.
    unsafe {
        NSApplication::sharedApplication(mtm).orderFrontStandardAboutPanelWithOptions(&options)
    };
}

/// A short description, then each soundpack source with its packs.
pub fn credits_text(library: &Library) -> String {
    let mut by_credit: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for pack in library.iter() {
        by_credit.entry(pack.meta.credit.as_str()).or_default().push(pack.display_name());
    }
    let mut text = String::from(
        "Mechanical keyboard sounds for every key press.\n\
         No network access, no analytics; keystrokes are never recorded.\n",
    );
    for (credit, packs) in by_credit {
        text.push_str(&format!("\nSounds: {credit}\n{}\n", packs.join(", ")));
    }
    text
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::packs::testing::{loaded, pack_data};

    #[test]
    fn credits_group_packs_by_source() {
        let mut a = pack_data("a", "linear", 0, true, false);
        a.meta.credit = "Mechvibes by Hai Nguyen · MIT License".into();
        let mut b = pack_data("b", "clicky", 1, false, false);
        b.meta.credit = "kbsim by Thomas Lai · MIT License".into();
        let mut c = pack_data("c", "clicky", 2, false, false);
        c.meta.credit = "kbsim by Thomas Lai · MIT License".into();
        let library = Library::new(vec![loaded(&a), loaded(&b), loaded(&c)]);
        let text = credits_text(&library);
        assert!(text.contains("Sounds: Mechvibes by Hai Nguyen · MIT License\na\n"), "{text}");
        assert!(text.contains("Sounds: kbsim by Thomas Lai · MIT License\nb, c\n"), "{text}");
        assert!(text.contains("never recorded"));
    }
}
