//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// The UI is wired in a later task.
#![allow(dead_code)]

mod audio;
mod diagnostics;
mod engine;
mod input;
mod log;
mod packs;
mod runtime;
mod settings;
mod ui;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
        Some("--tap-test") => diagnostics::tap_test(),
        Some("--bench") => {
            diagnostics::bench();
            Ok(())
        }
        _ => {
            eprintln!(
                "Rustyvibes {}: menu bar UI arrives in a later task",
                env!("CARGO_PKG_VERSION")
            );
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
