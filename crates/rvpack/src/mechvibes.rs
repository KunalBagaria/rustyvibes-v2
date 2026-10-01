//! Mechvibes soundpack configs name keys by libuiohook scan code. This maps
//! macOS virtual keycodes to candidate codes, most specific first, so a pack
//! that lacks a key can fall back to a similar one.

/// Mechvibes code for the letter A, the last-resort fallback.
pub const CODE_A: u16 = 30;

/// Candidate Mechvibes codes for a macOS keycode; empty for unknown keys.
pub fn candidates(mac: u8) -> &'static [u16] {
    match mac {
        0x00 => &[30],          // A
        0x01 => &[31],          // S
        0x02 => &[32],          // D
        0x03 => &[33],          // F
        0x04 => &[35],          // H
        0x05 => &[34],          // G
        0x06 => &[44],          // Z
        0x07 => &[45],          // X
        0x08 => &[46],          // C
        0x09 => &[47],          // V
        0x0A => &[41, 2],       // §
        0x0B => &[48],          // B
        0x0C => &[16],          // Q
        0x0D => &[17],          // W
        0x0E => &[18],          // E
        0x0F => &[19],          // R
        0x10 => &[21],          // Y
        0x11 => &[20],          // T
        0x12 => &[2],           // 1
        0x13 => &[3],           // 2
        0x14 => &[4],           // 3
        0x15 => &[5],           // 4
        0x16 => &[7],           // 6
        0x17 => &[6],           // 5
        0x18 => &[13, 12],      // =
        0x19 => &[10],          // 9
        0x1A => &[8],           // 7
        0x1B => &[12, 13],      // -
        0x1C => &[9],           // 8
        0x1D => &[11],          // 0
        0x1E => &[27, 26],      // ]
        0x1F => &[24],          // O
        0x20 => &[22],          // U
        0x21 => &[26, 27],      // [
        0x22 => &[23],          // I
        0x23 => &[25],          // P
        0x24 => &[28],          // Return
        0x25 => &[38],          // L
        0x26 => &[36],          // J
        0x27 => &[40, 39],      // '
        0x28 => &[37],          // K
        0x29 => &[39, 40],      // ;
        0x2A => &[43, 28],      // \
        0x2B => &[51, 52],      // ,
        0x2C => &[53, 52],      // /
        0x2D => &[49],          // N
        0x2E => &[50],          // M
        0x2F => &[52, 51],      // .
        0x30 => &[15],          // Tab
        0x31 => &[57],          // Space
        0x32 => &[41, 2],       // `
        0x33 => &[14],          // Delete (backspace)
        0x35 => &[1],           // Escape
        0x36 => &[3676, 3675],  // Right Command
        0x37 => &[3675, 3676],  // Command
        0x38 => &[42, 54],      // Shift
        0x39 => &[58, 15],      // Caps Lock
        0x3A => &[56, 3640],    // Option
        0x3B => &[29, 3613],    // Control
        0x3C => &[54, 42],      // Right Shift
        0x3D => &[3640, 56],    // Right Option
        0x3E => &[3613, 29],    // Right Control
        0x3F => &[3666, 29],    // Fn (sits where Insert is on PC keyboards)
        0x40 => &[101, 88],     // F17
        0x41 => &[83, 52],      // Keypad .
        0x43 => &[55, 9],       // Keypad *
        0x45 => &[78, 13],      // Keypad +
        0x47 => &[69, 1],       // Keypad Clear
        0x48 => &[57392, 88],   // Volume Up
        0x49 => &[57390, 87],   // Volume Down
        0x4A => &[57376, 68],   // Mute
        0x4B => &[3637, 53],    // Keypad /
        0x4C => &[3612, 28],    // Keypad Enter
        0x4E => &[74, 12],      // Keypad -
        0x4F => &[102, 88],     // F18
        0x50 => &[103, 88],     // F19
        0x51 => &[3597, 13],    // Keypad =
        0x52 => &[82, 11],      // Keypad 0
        0x53 => &[79, 2],       // Keypad 1
        0x54 => &[80, 3],       // Keypad 2
        0x55 => &[81, 4],       // Keypad 3
        0x56 => &[75, 5],       // Keypad 4
        0x57 => &[76, 6],       // Keypad 5
        0x58 => &[77, 7],       // Keypad 6
        0x59 => &[71, 8],       // Keypad 7
        0x5A => &[104, 88],     // F20
        0x5B => &[72, 9],       // Keypad 8
        0x5C => &[73, 10],      // Keypad 9
        0x5D => &[125, 43],     // JIS Yen
        0x5E => &[115, 53],     // JIS Underscore
        0x5F => &[126, 83, 51], // JIS Keypad ,
        0x60 => &[63],          // F5
        0x61 => &[64],          // F6
        0x62 => &[65],          // F7
        0x63 => &[61],          // F3
        0x64 => &[66],          // F8
        0x65 => &[67],          // F9
        0x66 => &[3675],        // JIS Eisu (left of space)
        0x67 => &[87, 88],      // F11
        0x68 => &[3676],        // JIS Kana (right of space)
        0x69 => &[91, 88],      // F13
        0x6A => &[99, 88],      // F16
        0x6B => &[92, 88],      // F14
        0x6D => &[68],          // F10
        0x6E => &[3677, 3676],  // Menu
        0x6F => &[88, 87],      // F12
        0x71 => &[93, 88],      // F15
        0x72 => &[3666, 3655],  // Help / Insert
        0x73 => &[3655, 3657],  // Home
        0x74 => &[3657, 3655],  // Page Up
        0x75 => &[3667, 14],    // Forward Delete
        0x76 => &[62],          // F4
        0x77 => &[3663, 3667],  // End
        0x78 => &[60],          // F2
        0x79 => &[3665, 3663],  // Page Down
        0x7A => &[59],          // F1
        0x7B => &[57419, 75],   // Left Arrow
        0x7C => &[57421, 77],   // Right Arrow
        0x7D => &[57424, 80],   // Down Arrow
        0x7E => &[57416, 72],   // Up Arrow
        _ => &[],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{KEYS, code};

    #[test]
    fn every_known_key_has_candidates() {
        for key in KEYS {
            assert!(!candidates(key.code).is_empty(), "{} has no Mechvibes candidates", key.name);
        }
        assert!(candidates(0x34).is_empty());
    }

    #[test]
    fn primary_codes_match_libuiohook() {
        assert_eq!(candidates(code::A)[0], 30);
        assert_eq!(candidates(code::SPACE)[0], 57);
        assert_eq!(candidates(code::RETURN)[0], 28);
        assert_eq!(candidates(code::ESCAPE)[0], 1);
        assert_eq!(candidates(code::DELETE)[0], 14);
        assert_eq!(candidates(0x7E)[0], 57416); // up arrow
        assert_eq!(candidates(code::RIGHT_COMMAND)[0], 3676);
    }

    #[test]
    fn right_modifiers_fall_back_to_left() {
        assert_eq!(candidates(code::RIGHT_COMMAND), &[3676, 3675]);
        assert_eq!(candidates(code::RIGHT_SHIFT), &[54, 42]);
        assert_eq!(candidates(code::RIGHT_OPTION), &[3640, 56]);
        assert_eq!(candidates(code::RIGHT_CONTROL), &[3613, 29]);
    }
}
