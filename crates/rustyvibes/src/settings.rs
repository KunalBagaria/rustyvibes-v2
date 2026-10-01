//! User settings.

/// User-facing settings (persisted in `NSUserDefaults` by the UI).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub enabled: bool,
    /// Saved soundpack id; `None` until the user picks one.
    pub pack: Option<String>,
    pub volume: f32,
    pub release_sounds: bool,
    pub variation: bool,
    pub spatial: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            enabled: true,
            pack: None,
            volume: 0.75,
            release_sounds: true,
            variation: true,
            spatial: true,
        }
    }
}
