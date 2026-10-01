//! Rustyvibes 2: mechanical keyboard sounds for every key press, as a native
//! macOS menu bar app.

mod audio;
mod diagnostics;
mod engine;
mod input;
mod log;
mod packs;
mod runtime;
mod settings;
mod snapshot;
mod ui;

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "Rustyvibes: mechanical keyboard sounds for every key press.

Run without arguments to start the menu bar app. Diagnostics:
  --version              print the version
  --selftest [--audible] play every soundpack through the audio path (silent unless --audible)
  --tap-test             check that key events reach the engine (injects one Shift press)
  --bench                measure the mixer
  --snapshot [dir]       render UI pieces to PNG (default: target/snapshots)

Set RUSTYVIBES_LOG=1 to log to stderr.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--help" | "-h") => {
            println!("{HELP}");
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
        Some("--tap-test") => diagnostics::tap_test(),
        Some("--bench") => {
            diagnostics::bench();
            Ok(())
        }
        Some("--snapshot") => {
            let dir = args.get(1).map_or_else(|| PathBuf::from("target/snapshots"), PathBuf::from);
            snapshot::snapshot(&dir)
        }
        // Anything else (including LaunchServices' legacy -psn_ argument) starts the app.
        _ => {
            ui::run();
            Ok(())
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
