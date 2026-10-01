//! Physical key geometry for macOS virtual keycodes (`kVK_*`). Keycodes name
//! key *positions*, independent of the keyboard layout. Positions are in key
//! units on a full-size ANSI keyboard; `row` 0 is the function row and 5 the
//! space-bar row.

/// One physical key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KeyInfo {
    pub code: u8,
    pub name: &'static str,
    pub row: u8,
    /// Horizontal centre of the key, in key units from the left edge.
    pub x: f32,
}

/// macOS virtual keycodes referred to by name.
pub mod code {
    pub const A: u8 = 0x00;
    pub const S: u8 = 0x01;
    pub const E: u8 = 0x0E;
    pub const R: u8 = 0x0F;
    pub const T: u8 = 0x11;
    pub const O: u8 = 0x1F;
    pub const U: u8 = 0x20;
    pub const J: u8 = 0x26;
    pub const RETURN: u8 = 0x24;
    pub const TAB: u8 = 0x30;
    pub const SPACE: u8 = 0x31;
    pub const DELETE: u8 = 0x33;
    pub const ESCAPE: u8 = 0x35;
    pub const RIGHT_COMMAND: u8 = 0x36;
    pub const COMMAND: u8 = 0x37;
    pub const SHIFT: u8 = 0x38;
    pub const CAPS_LOCK: u8 = 0x39;
    pub const OPTION: u8 = 0x3A;
    pub const CONTROL: u8 = 0x3B;
    pub const RIGHT_SHIFT: u8 = 0x3C;
    pub const RIGHT_OPTION: u8 = 0x3D;
    pub const RIGHT_CONTROL: u8 = 0x3E;
    pub const FUNCTION: u8 = 0x3F;
    pub const KEYPAD_ENTER: u8 = 0x4C;
}

const fn k(code: u8, name: &'static str, row: u8, x: f32) -> KeyInfo {
    KeyInfo { code, name, row, x }
}

/// Every physical key Rustyvibes knows about.
pub const KEYS: &[KeyInfo] = &[
    // Row 0: function row.
    k(0x35, "Escape", 0, 0.5),
    k(0x7A, "F1", 0, 2.5),
    k(0x78, "F2", 0, 3.5),
    k(0x63, "F3", 0, 4.5),
    k(0x76, "F4", 0, 5.5),
    k(0x60, "F5", 0, 7.0),
    k(0x61, "F6", 0, 8.0),
    k(0x62, "F7", 0, 9.0),
    k(0x64, "F8", 0, 10.0),
    k(0x65, "F9", 0, 11.5),
    k(0x6D, "F10", 0, 12.5),
    k(0x67, "F11", 0, 13.5),
    k(0x6F, "F12", 0, 14.5),
    k(0x69, "F13", 0, 15.75),
    k(0x6B, "F14", 0, 16.75),
    k(0x71, "F15", 0, 17.75),
    k(0x6A, "F16", 0, 19.0),
    k(0x40, "F17", 0, 20.0),
    k(0x4F, "F18", 0, 21.0),
    k(0x50, "F19", 0, 22.0),
    k(0x5A, "F20", 0, 22.5),
    k(0x4A, "Mute", 0, 12.5),
    k(0x49, "Volume Down", 0, 13.5),
    k(0x48, "Volume Up", 0, 14.5),
    // Row 1: number row.
    k(0x32, "`", 1, 0.5),
    k(0x0A, "§", 1, 0.5),
    k(0x12, "1", 1, 1.5),
    k(0x13, "2", 1, 2.5),
    k(0x14, "3", 1, 3.5),
    k(0x15, "4", 1, 4.5),
    k(0x17, "5", 1, 5.5),
    k(0x16, "6", 1, 6.5),
    k(0x1A, "7", 1, 7.5),
    k(0x1C, "8", 1, 8.5),
    k(0x19, "9", 1, 9.5),
    k(0x1D, "0", 1, 10.5),
    k(0x1B, "-", 1, 11.5),
    k(0x18, "=", 1, 12.5),
    k(0x5D, "¥", 1, 13.5),
    k(0x33, "Delete", 1, 14.0),
    k(0x72, "Help", 1, 15.75),
    k(0x73, "Home", 1, 16.75),
    k(0x74, "Page Up", 1, 17.75),
    k(0x47, "Keypad Clear", 1, 19.0),
    k(0x51, "Keypad =", 1, 20.0),
    k(0x4B, "Keypad /", 1, 21.0),
    k(0x43, "Keypad *", 1, 22.0),
    // Row 2: top letter row.
    k(0x30, "Tab", 2, 0.75),
    k(0x0C, "Q", 2, 2.0),
    k(0x0D, "W", 2, 3.0),
    k(0x0E, "E", 2, 4.0),
    k(0x0F, "R", 2, 5.0),
    k(0x11, "T", 2, 6.0),
    k(0x10, "Y", 2, 7.0),
    k(0x20, "U", 2, 8.0),
    k(0x22, "I", 2, 9.0),
    k(0x1F, "O", 2, 10.0),
    k(0x23, "P", 2, 11.0),
    k(0x21, "[", 2, 12.0),
    k(0x1E, "]", 2, 13.0),
    k(0x2A, "\\", 2, 14.25),
    k(0x75, "Forward Delete", 2, 15.75),
    k(0x77, "End", 2, 16.75),
    k(0x79, "Page Down", 2, 17.75),
    k(0x59, "Keypad 7", 2, 19.0),
    k(0x5B, "Keypad 8", 2, 20.0),
    k(0x5C, "Keypad 9", 2, 21.0),
    k(0x4E, "Keypad -", 2, 22.0),
    // Row 3: home row.
    k(0x39, "Caps Lock", 3, 0.875),
    k(0x00, "A", 3, 2.25),
    k(0x01, "S", 3, 3.25),
    k(0x02, "D", 3, 4.25),
    k(0x03, "F", 3, 5.25),
    k(0x05, "G", 3, 6.25),
    k(0x04, "H", 3, 7.25),
    k(0x26, "J", 3, 8.25),
    k(0x28, "K", 3, 9.25),
    k(0x25, "L", 3, 10.25),
    k(0x29, ";", 3, 11.25),
    k(0x27, "'", 3, 12.25),
    k(0x24, "Return", 3, 13.875),
    k(0x56, "Keypad 4", 3, 19.0),
    k(0x57, "Keypad 5", 3, 20.0),
    k(0x58, "Keypad 6", 3, 21.0),
    k(0x45, "Keypad +", 3, 22.0),
    // Row 4: bottom letter row.
    k(0x38, "Shift", 4, 1.125),
    k(0x06, "Z", 4, 2.75),
    k(0x07, "X", 4, 3.75),
    k(0x08, "C", 4, 4.75),
    k(0x09, "V", 4, 5.75),
    k(0x0B, "B", 4, 6.75),
    k(0x2D, "N", 4, 7.75),
    k(0x2E, "M", 4, 8.75),
    k(0x2B, ",", 4, 9.75),
    k(0x2F, ".", 4, 10.75),
    k(0x2C, "/", 4, 11.75),
    k(0x5E, "_", 4, 12.75),
    k(0x3C, "Right Shift", 4, 13.625),
    k(0x7E, "Up Arrow", 4, 16.75),
    k(0x53, "Keypad 1", 4, 19.0),
    k(0x54, "Keypad 2", 4, 20.0),
    k(0x55, "Keypad 3", 4, 21.0),
    k(0x4C, "Keypad Enter", 4, 22.0),
    // Row 5: space-bar row (MacBook / Magic Keyboard positions).
    k(0x3F, "Fn", 5, 0.5),
    k(0x3B, "Control", 5, 1.5),
    k(0x3A, "Option", 5, 2.5),
    k(0x37, "Command", 5, 3.75),
    k(0x66, "Eisu", 5, 4.75),
    k(0x31, "Space", 5, 7.0),
    k(0x68, "Kana", 5, 9.25),
    k(0x36, "Right Command", 5, 10.25),
    k(0x3D, "Right Option", 5, 11.25),
    k(0x3E, "Right Control", 5, 12.5),
    k(0x6E, "Menu", 5, 13.5),
    k(0x7B, "Left Arrow", 5, 15.75),
    k(0x7D, "Down Arrow", 5, 16.75),
    k(0x7C, "Right Arrow", 5, 17.75),
    k(0x52, "Keypad 0", 5, 19.5),
    k(0x41, "Keypad .", 5, 21.0),
    k(0x5F, "Keypad ,", 5, 22.0),
];

/// Geometry for a keycode, if it is a known physical key.
pub fn info(code: u8) -> Option<&'static KeyInfo> {
    KEYS.iter().find(|key| key.code == code)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_has_every_physical_key_once() {
        assert_eq!(KEYS.len(), 120);
        let mut seen = [false; 128];
        for key in KEYS {
            assert!(key.code < 128, "{} out of range", key.name);
            assert!(!seen[usize::from(key.code)], "duplicate code {:#04x}", key.code);
            seen[usize::from(key.code)] = true;
            assert!(key.row <= 5, "{} has row {}", key.name, key.row);
            assert!((0.0..=23.0).contains(&key.x), "{} has x {}", key.name, key.x);
        }
    }

    #[test]
    fn rows_match_the_physical_keyboard() {
        assert_eq!(info(code::ESCAPE).unwrap().row, 0);
        assert_eq!(info(code::DELETE).unwrap().row, 1);
        assert_eq!(info(code::TAB).unwrap().row, 2);
        assert_eq!(info(code::A).unwrap().row, 3);
        assert_eq!(info(code::RETURN).unwrap().row, 3);
        assert_eq!(info(code::SHIFT).unwrap().row, 4);
        assert_eq!(info(code::SPACE).unwrap().row, 5);
        assert!(info(0x34).is_none());
    }

    #[test]
    fn letters_run_left_to_right() {
        // Q W E R T Y U I O P
        let qwerty = [0x0C, 0x0D, 0x0E, 0x0F, 0x11, 0x10, 0x20, 0x22, 0x1F, 0x23];
        let xs: Vec<f32> = qwerty.iter().map(|&c| info(c).unwrap().x).collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]), "{xs:?}");
        assert!(info(code::A).unwrap().x < info(code::J).unwrap().x);
    }
}
