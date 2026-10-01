//! Turns raw key events into clean press/release transitions: drops
//! auto-repeat and duplicates, and decodes modifier `flagsChanged` events.

/// A physical key going down or up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Transition {
    pub keycode: u8,
    pub down: bool,
}

// Device-independent modifier flags (CGEventFlags).
pub const FLAG_ALPHA_SHIFT: u64 = 0x0001_0000;
pub const FLAG_SHIFT: u64 = 0x0002_0000;
pub const FLAG_CONTROL: u64 = 0x0004_0000;
pub const FLAG_OPTION: u64 = 0x0008_0000;
pub const FLAG_COMMAND: u64 = 0x0010_0000;
pub const FLAG_FN: u64 = 0x0080_0000;

// Device-dependent bits (IOKit NX_DEVICE*KEYMASK) tell left from right.
pub const DEV_LCTL: u64 = 0x0000_0001;
pub const DEV_LSHIFT: u64 = 0x0000_0002;
pub const DEV_RSHIFT: u64 = 0x0000_0004;
pub const DEV_LCMD: u64 = 0x0000_0008;
pub const DEV_RCMD: u64 = 0x0000_0010;
pub const DEV_LALT: u64 = 0x0000_0020;
pub const DEV_RALT: u64 = 0x0000_0040;
pub const DEV_RCTL: u64 = 0x0000_2000;
const DEV_ALL: u64 =
    DEV_LCTL | DEV_LSHIFT | DEV_RSHIFT | DEV_LCMD | DEV_RCMD | DEV_LALT | DEV_RALT | DEV_RCTL;

const CAPS_LOCK: u8 = 0x39;
const FUNCTION: u8 = 0x3F;

/// Which of the 128 keys are currently down.
#[derive(Clone, Debug, Default)]
pub struct KeyState {
    pressed: [u64; 2],
}

impl KeyState {
    pub fn is_pressed(&self, keycode: u8) -> bool {
        self.pressed[usize::from(keycode >> 6) & 1] & (1 << (keycode & 63)) != 0
    }

    fn set(&mut self, keycode: u8, down: bool) {
        let word = &mut self.pressed[usize::from(keycode >> 6) & 1];
        if down {
            *word |= 1 << (keycode & 63);
        } else {
            *word &= !(1 << (keycode & 63));
        }
    }

    pub fn key_down(&mut self, keycode: u16, autorepeat: bool) -> Option<Transition> {
        let keycode = valid(keycode)?;
        if autorepeat || self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, true);
        Some(Transition { keycode, down: true })
    }

    pub fn key_up(&mut self, keycode: u16) -> Option<Transition> {
        let keycode = valid(keycode)?;
        if !self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, false);
        Some(Transition { keycode, down: false })
    }

    /// Decodes a `flagsChanged` event for a modifier key.
    pub fn flags_changed(&mut self, keycode: u16, flags: u64) -> Option<Transition> {
        let keycode = valid(keycode)?;
        let down = match keycode {
            // macOS reports only the toggle, not the physical release.
            CAPS_LOCK => return Some(Transition { keycode, down: true }),
            FUNCTION => flags & FLAG_FN != 0,
            _ => {
                let bit = device_bit(keycode)?;
                if flags & DEV_ALL != 0 { flags & bit != 0 } else { !self.is_pressed(keycode) }
            }
        };
        if down == self.is_pressed(keycode) {
            return None;
        }
        self.set(keycode, down);
        Some(Transition { keycode, down })
    }
}

fn valid(keycode: u16) -> Option<u8> {
    u8::try_from(keycode).ok().filter(|k| *k < 128)
}

fn device_bit(keycode: u8) -> Option<u64> {
    Some(match keycode {
        0x38 => DEV_LSHIFT,
        0x3C => DEV_RSHIFT,
        0x3B => DEV_LCTL,
        0x3E => DEV_RCTL,
        0x3A => DEV_LALT,
        0x3D => DEV_RALT,
        0x37 => DEV_LCMD,
        0x36 => DEV_RCMD,
        _ => return None,
    })
}
#[cfg(test)]
mod tests {
    use super::*;

    const SHIFT: u16 = 0x38;
    const RIGHT_SHIFT: u16 = 0x3C;
    const CAPS: u16 = 0x39;
    const FN: u16 = 0x3F;

    fn down(keycode: u8) -> Option<Transition> {
        Some(Transition { keycode, down: true })
    }

    fn up(keycode: u8) -> Option<Transition> {
        Some(Transition { keycode, down: false })
    }

    #[test]
    fn key_press_and_release() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(0x00, false), down(0x00));
        assert_eq!(keys.key_up(0x00), up(0x00));
    }

    #[test]
    fn autorepeat_and_duplicate_downs_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(0x00, false), down(0x00));
        assert_eq!(keys.key_down(0x00, true), None, "auto-repeat");
        assert_eq!(keys.key_down(0x00, false), None, "already down");
    }

    #[test]
    fn release_without_press_is_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_up(0x00), None);
    }

    #[test]
    fn out_of_range_keycodes_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.key_down(128, false), None);
        assert_eq!(keys.key_down(u16::MAX, false), None);
        assert_eq!(keys.flags_changed(300, FLAG_SHIFT), None);
    }

    #[test]
    fn shift_with_device_bits() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_LSHIFT), down(0x38));
        assert_eq!(keys.flags_changed(SHIFT, 0), up(0x38));
    }

    #[test]
    fn releasing_left_shift_while_right_held() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_LSHIFT), down(0x38));
        assert_eq!(
            keys.flags_changed(RIGHT_SHIFT, FLAG_SHIFT | DEV_LSHIFT | DEV_RSHIFT),
            down(0x3C)
        );
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT | DEV_RSHIFT), up(0x38));
        assert_eq!(keys.flags_changed(RIGHT_SHIFT, 0), up(0x3C));
    }

    #[test]
    fn modifiers_without_device_bits_toggle() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(SHIFT, FLAG_SHIFT), down(0x38));
        assert_eq!(keys.flags_changed(SHIFT, 0), up(0x38));
        assert_eq!(keys.flags_changed(0x37, FLAG_COMMAND), down(0x37));
        assert_eq!(keys.flags_changed(0x37, 0), up(0x37));
    }

    #[test]
    fn caps_lock_clicks_on_every_change() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(CAPS, FLAG_ALPHA_SHIFT), down(0x39));
        assert_eq!(keys.flags_changed(CAPS, 0), down(0x39));
    }

    #[test]
    fn fn_key_follows_its_flag() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(FN, FLAG_FN), down(0x3F));
        assert_eq!(keys.flags_changed(FN, FLAG_FN), None);
        assert_eq!(keys.flags_changed(FN, 0), up(0x3F));
    }

    #[test]
    fn non_modifier_flag_changes_are_ignored() {
        let mut keys = KeyState::default();
        assert_eq!(keys.flags_changed(0x00, FLAG_SHIFT), None);
    }
}
