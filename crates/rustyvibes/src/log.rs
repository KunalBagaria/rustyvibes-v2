//! Opt-in diagnostics: set `RUSTYVIBES_LOG=1` to print to stderr.
//! Never used on the audio or event-tap threads' hot paths.

use std::sync::OnceLock;
use std::time::Instant;

static START: OnceLock<Instant> = OnceLock::new();

/// Marks process start so log lines can show elapsed time; call first thing in `main`.
pub fn init() {
    START.get_or_init(Instant::now);
}

pub fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("RUSTYVIBES_LOG").is_some_and(|v| v != "0"))
}

/// Writes one log line to stderr. Write errors are ignored: a closed or broken
/// stderr must never take the app down (`eprintln!` would panic).
pub fn emit(args: std::fmt::Arguments<'_>) {
    emit_to(&mut std::io::stderr().lock(), args);
}

fn emit_to(out: &mut impl std::io::Write, args: std::fmt::Arguments<'_>) {
    let ms = START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1e3;
    let _ = writeln!(out, "[rustyvibes {ms:8.1} ms] {args}");
}

macro_rules! log {
    ($($arg:tt)*) => {
        if $crate::log::enabled() {
            $crate::log::emit(format_args!($($arg)*));
        }
    };
}

pub(crate) use log;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Write};

    /// A writer whose reader went away, like stderr piped into a closed process.
    struct BrokenPipe;

    impl Write for BrokenPipe {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }

    #[test]
    fn logging_to_a_closed_pipe_does_not_panic() {
        emit_to(&mut BrokenPipe, format_args!("audio: started at {} Hz", 48_000));
    }

    #[test]
    fn log_lines_carry_the_prefix() {
        let mut out = Vec::new();
        emit_to(&mut out, format_args!("hello {}", 1));
        let line = String::from_utf8(out).unwrap();
        assert!(line.starts_with("[rustyvibes "), "{line}");
        assert!(line.ends_with(" ms] hello 1\n"), "{line}");
    }
}
