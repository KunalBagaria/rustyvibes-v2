//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// Audio, input and UI are wired in later tasks.
#![allow(dead_code)]

mod engine;
mod log;
mod packs;
mod settings;

fn main() {
    println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
}
