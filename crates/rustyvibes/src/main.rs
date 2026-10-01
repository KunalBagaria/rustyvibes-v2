//! Rustyvibes 2: mechanical keyboard sounds for every key press.

// The engine is wired into the app in later tasks.
#![allow(dead_code)]

mod engine;

fn main() {
    println!("Rustyvibes {}", env!("CARGO_PKG_VERSION"));
}
