//! Build tooling for Rustyvibes: `cargo xtask <command>`.

mod bundle;
mod catalog;
mod decode;
mod dmg;
mod dsp;
mod packs;
mod sources;
#[cfg(test)]
mod testutil;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

/// Errors are human-readable strings; this is a build tool.
pub type Result<T, E = String> = std::result::Result<T, E>;

const USAGE: &str = "usage: cargo xtask <command>

commands:
  packs               convert assets/soundpacks into target/packs/*.rvpack
  bundle [options]    build and sign target/bundle/Rustyvibes.app
  dmg [options]       bundle, then create target/Rustyvibes-<version>.dmg
  probe <file>...     show how the clean-up pipeline sees audio files

options:
  --native            build for this Mac's architecture only (default: universal)
  --adhoc             sign ad-hoc even if a Developer ID is available";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = workspace_root();
    let rest = args.get(1..).unwrap_or_default();
    let result = match args.first().map(String::as_str) {
        Some("packs") => packs::run(&root),
        Some("bundle") => bundle::run(&root, rest),
        Some("dmg") => dmg::run(&root, rest),
        Some("probe") => probe(rest),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
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

/// The repository root (this crate lives in `crates/xtask`).
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels below the workspace root")
        .to_path_buf()
}

/// Prints how the clean-up pipeline sees each file.
fn probe(files: &[String]) -> Result<()> {
    for file in files {
        let mut audio = decode::decode_file(Path::new(file))?;
        dsp::dc_block(&mut audio.samples, audio.rate);
        let ms = |n: usize| n as f32 * 1000.0 / audio.rate as f32;
        let Some(trimmed) = dsp::trim_clip(&audio.samples, audio.rate, &dsp::TrimParams::default())
        else {
            println!("{file}: silent");
            continue;
        };
        let mut clips = vec![trimmed];
        let gain = dsp::normalize(&mut clips, &[true], audio.rate, dsp::TARGET_DB, dsp::CEILING_DB);
        let quantised = dsp::to_i16(&clips[0], &mut 1);
        let peak = quantised.iter().map(|s| i32::from(*s).abs()).max().unwrap_or(0);
        println!(
            "{file}: {} Hz, {:.1} ms → {:.1} ms trimmed, gain {gain:+.1} dB, i16 peak {peak}",
            audio.rate,
            ms(audio.samples.len()),
            ms(clips[0].len()),
        );
    }
    Ok(())
}
