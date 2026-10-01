//! `cargo xtask dmg`: bundles the app, then wraps it in a compressed disk image
//! with an Applications shortcut, signed like the app.

use std::path::Path;
use std::process::Command;

use crate::bundle::{self, Options, run_cmd};

pub fn run(root: &Path, args: &[String]) -> crate::Result<()> {
    let built = bundle::bundle(root, &Options::parse(args))?;
    let version = env!("CARGO_PKG_VERSION");
    let staging = root.join("target/dmg");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    }
    std::fs::create_dir_all(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    run_cmd(
        Command::new("ditto")
            .arg(&built.app)
            .arg(staging.join(format!("{}.app", bundle::APP_NAME))),
    )?;
    std::os::unix::fs::symlink("/Applications", staging.join("Applications"))
        .map_err(|e| format!("Applications link: {e}"))?;

    let dmg = root.join("target").join(format!("{}-{version}.dmg", bundle::APP_NAME));
    if dmg.exists() {
        std::fs::remove_file(&dmg).map_err(|e| format!("{}: {e}", dmg.display()))?;
    }
    run_cmd(
        Command::new("hdiutil")
            .args([
                "create",
                "-quiet",
                "-volname",
                bundle::APP_NAME,
                "-fs",
                "APFS",
                "-format",
                "ULFO",
                "-srcfolder",
            ])
            .arg(&staging)
            .arg(&dmg),
    )?;
    if let Some(identity) = &built.identity {
        run_cmd(
            Command::new("codesign").args(["--force", "--timestamp", "--sign", identity]).arg(&dmg),
        )?;
    }
    let size = std::fs::metadata(&dmg).map(|m| m.len()).unwrap_or(0);
    println!("{} ({:.1} MB)", dmg.display(), size as f64 / 1e6);
    Ok(())
}
