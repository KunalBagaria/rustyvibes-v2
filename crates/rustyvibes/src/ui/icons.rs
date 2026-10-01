//! Icons drawn in code: the template status-bar glyph and the tinted keycap
//! glyphs of the soundpack menu. Vector drawing keeps them sharp at any scale.

use block2::RcBlock;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_app_kit::{NSBezierPath, NSColor, NSImage};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

/// States of the status-bar icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusIcon {
    /// A keycap.
    Normal,
    /// A keycap holding an exclamation mark: keyboard access is missing.
    Attention,
}

/// An 18 pt template image for the status bar (tinted by the system).
pub fn status(kind: StatusIcon) -> Retained<NSImage> {
    let block = RcBlock::new(move |_bounds: NSRect| -> Bool {
        let ink = NSColor::blackColor();
        ink.setStroke();
        ink.setFill();
        let body = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(2.25, 2.75, 13.5, 12.0),
            3.2,
            3.2,
        );
        body.setLineWidth(1.5);
        body.stroke();
        match kind {
            StatusIcon::Normal => {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    rect(5.25, 6.25, 7.5, 6.0),
                    1.6,
                    1.6,
                )
                .fill();
            }
            StatusIcon::Attention => {
                NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
                    rect(8.2, 7.4, 1.6, 5.0),
                    0.8,
                    0.8,
                )
                .fill();
                NSBezierPath::bezierPathWithOvalInRect(rect(8.15, 4.6, 1.7, 1.7)).fill();
            }
        }
        Bool::YES
    });
    let image =
        NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(18.0, 18.0), false, &block);
    image.setTemplate(true);
    image.setAccessibilityDescription(Some(&NSString::from_str("Rustyvibes")));
    image
}

/// A 16 pt keycap glyph tinted with a switch's stem colour (`#RRGGBB`).
pub fn keycap(hex: &str) -> Retained<NSImage> {
    let (r, g, b) = parse_hex(hex);
    let block = RcBlock::new(move |_bounds: NSRect| -> Bool {
        NSColor::colorWithSRGBRed_green_blue_alpha(r * 0.72, g * 0.72, b * 0.72, 1.0).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(1.0, 1.5, 14.0, 13.0),
            3.5,
            3.5,
        )
        .fill();
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(3.0, 4.5, 10.0, 8.5),
            2.2,
            2.2,
        )
        .fill();
        NSColor::colorWithSRGBRed_green_blue_alpha(1.0, 1.0, 1.0, 0.3).setFill();
        NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(4.0, 10.6, 8.0, 1.5),
            0.75,
            0.75,
        )
        .fill();
        // A hairline in the label colour (resolved for the current appearance at draw
        // time) keeps pale caps visible on light menus and dark caps on dark ones.
        NSColor::labelColor().colorWithAlphaComponent(0.3).setStroke();
        let outline = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
            rect(1.25, 1.75, 13.5, 12.5),
            3.3,
            3.3,
        );
        outline.setLineWidth(0.5);
        outline.stroke();
        Bool::YES
    });
    NSImage::imageWithSize_flipped_drawingHandler(NSSize::new(16.0, 16.0), false, &block)
}

/// An SF Symbol, or an empty 16 pt image if the symbol is unavailable.
pub fn symbol(name: &str, description: &str) -> Retained<NSImage> {
    NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        Some(&NSString::from_str(description)),
    )
    .unwrap_or_else(|| NSImage::initWithSize(NSImage::alloc(), NSSize::new(16.0, 16.0)))
}

/// Parses `#RRGGBB` into sRGB components in 0…1; malformed input yields mid grey.
pub fn parse_hex(hex: &str) -> (f64, f64, f64) {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    let component = |i: usize| {
        digits
            .get(i..i + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .map(|v| f64::from(v) / 255.0)
    };
    match (digits.len(), component(0), component(2), component(4)) {
        (6, Some(r), Some(g), Some(b)) => (r, g, b),
        _ => (0.5, 0.5, 0.5),
    }
}

pub fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
    NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colours() {
        assert_eq!(parse_hex("#FF0000"), (1.0, 0.0, 0.0));
        let (r, g, b) = parse_hex("#336699");
        assert!((r - 0.2).abs() < 1e-9 && (g - 0.4).abs() < 1e-9 && (b - 0.6).abs() < 1e-9);
        assert_eq!(parse_hex("red"), (0.5, 0.5, 0.5));
        assert_eq!(parse_hex("#12345"), (0.5, 0.5, 0.5));
        assert_eq!(parse_hex("#GG0000"), (0.5, 0.5, 0.5));
    }
}
