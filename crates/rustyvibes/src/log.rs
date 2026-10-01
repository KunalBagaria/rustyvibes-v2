//! Opt-in diagnostics: set `RUSTYVIBES_LOG=1` to print to stderr.
//! Never used on the audio or event-tap threads' hot paths.

use std::sync::OnceLock;

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUSTYVIBES_LOG").is_some_and(|v| v != "0"))
}

macro_rules! log {
    ($($arg:tt)*) => {
        if $crate::log::enabled() {
            eprintln!("[rustyvibes] {}", format_args!($($arg)*));
        }
    };
}

pub(crate) use log;
