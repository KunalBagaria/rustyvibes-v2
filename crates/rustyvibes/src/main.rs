//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// Input and UI are wired in later tasks.
#![allow(dead_code)]

mod audio;
mod diagnostics;
mod engine;
mod log;
mod packs;
mod runtime;
mod settings;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("--version") => {
            println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Some("--selftest") => diagnostics::selftest(&args[1..]),
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
