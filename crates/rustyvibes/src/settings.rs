//! User settings and their persistence in `NSUserDefaults`.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSString, NSUserDefaults};

pub const KEY_ENABLED: &str = "Enabled";
pub const KEY_PACK: &str = "Pack";
pub const KEY_VOLUME: &str = "Volume";
pub const KEY_RELEASE_SOUNDS: &str = "ReleaseSounds";
pub const KEY_VARIATION: &str = "Variation";
pub const KEY_SPATIAL: &str = "Spatial";

/// User-facing settings.
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

/// Reads and writes settings in `NSUserDefaults` (thread-safe, cached in memory).
pub struct Store {
    defaults: Retained<NSUserDefaults>,
}

impl Store {
    pub fn standard() -> Store {
        Store { defaults: NSUserDefaults::standardUserDefaults() }
    }

    /// A separate defaults domain, so tests never touch the real settings.
    #[cfg(test)]
    pub fn suite(name: &str) -> Store {
        use objc2::AnyThread;
        let defaults = NSUserDefaults::initWithSuiteName(
            NSUserDefaults::alloc(),
            Some(&NSString::from_str(name)),
        )
        .expect("valid suite name");
        Store { defaults }
    }

    #[cfg(test)]
    pub fn clear_suite(name: &str) {
        NSUserDefaults::standardUserDefaults()
            .removePersistentDomainForName(&NSString::from_str(name));
    }

    /// Stored settings, with defaults for anything never saved.
    pub fn load(&self) -> Settings {
        let d = Settings::default();
        Settings {
            enabled: self.bool(KEY_ENABLED).unwrap_or(d.enabled),
            pack: self.defaults.stringForKey(&NSString::from_str(KEY_PACK)).map(|s| s.to_string()),
            volume: self.float(KEY_VOLUME).map_or(d.volume, |v| v.clamp(0.0, 1.0)),
            release_sounds: self.bool(KEY_RELEASE_SOUNDS).unwrap_or(d.release_sounds),
            variation: self.bool(KEY_VARIATION).unwrap_or(d.variation),
            spatial: self.bool(KEY_SPATIAL).unwrap_or(d.spatial),
        }
    }

    pub fn set_bool(&self, key: &str, value: bool) {
        self.defaults.setBool_forKey(value, &NSString::from_str(key));
    }

    pub fn set_float(&self, key: &str, value: f32) {
        self.defaults.setFloat_forKey(value, &NSString::from_str(key));
    }

    pub fn set_string(&self, key: &str, value: &str) {
        let value = NSString::from_str(value);
        let object: &AnyObject = &value;
        // SAFETY: NSString is a property-list type, as NSUserDefaults requires.
        unsafe { self.defaults.setObject_forKey(Some(object), &NSString::from_str(key)) };
    }

    fn has(&self, key: &str) -> bool {
        self.defaults.objectForKey(&NSString::from_str(key)).is_some()
    }

    fn bool(&self, key: &str) -> Option<bool> {
        self.has(key).then(|| self.defaults.boolForKey(&NSString::from_str(key)))
    }

    fn float(&self, key: &str) -> Option<f32> {
        self.has(key).then(|| self.defaults.floatForKey(&NSString::from_str(key)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_round_trips_and_clamps() {
        let suite = format!("io.github.kb24x7.rustyvibes.tests.{}", std::process::id());
        let store = Store::suite(&suite);
        assert_eq!(store.load(), Settings::default());
        store.set_bool(KEY_ENABLED, false);
        store.set_float(KEY_VOLUME, 0.25);
        store.set_string(KEY_PACK, "holy-panda");
        store.set_bool(KEY_SPATIAL, false);
        assert_eq!(
            store.load(),
            Settings {
                enabled: false,
                pack: Some("holy-panda".into()),
                volume: 0.25,
                spatial: false,
                ..Settings::default()
            }
        );
        store.set_float(KEY_VOLUME, 7.0);
        assert_eq!(store.load().volume, 1.0, "out-of-range values are clamped");
        Store::clear_suite(&suite);
    }
}
