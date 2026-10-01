//! `cargo xtask bundle`: builds the release binary (universal by default),
//! converts the soundpacks, and assembles and signs `target/bundle/Rustyvibes.app`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const APP_NAME: &str = "Rustyvibes";
pub const BUNDLE_ID: &str = "io.github.kb24x7.rustyvibes";
pub const MIN_MACOS: &str = "13.0";
const TARGETS: [&str; 2] = ["aarch64-apple-darwin", "x86_64-apple-darwin"];

pub struct Options {
    /// Build for Apple Silicon and Intel (default) instead of the host only.
    pub universal: bool,
    /// Sign ad-hoc even when a Developer ID identity is available.
    pub adhoc: bool,
}

impl Options {
    pub fn parse(args: &[String]) -> Options {
        Options {
            universal: !args.iter().any(|a| a == "--native"),
            adhoc: args.iter().any(|a| a == "--adhoc"),
        }
    }
}

pub struct Bundle {
    pub app: PathBuf,
    /// The Developer ID used, or `None` for an ad-hoc signature.
    pub identity: Option<String>,
}

/// `cargo xtask bundle [--native] [--adhoc]`
pub fn run(root: &Path, args: &[String]) -> crate::Result<()> {
    bundle(root, &Options::parse(args)).map(|_| ())
}

pub fn bundle(root: &Path, options: &Options) -> crate::Result<Bundle> {
    let version = env!("CARGO_PKG_VERSION");
    let binaries = compile(root, options.universal)?;
    let packs_dir = root.join("target/packs");
    let reports =
        crate::packs::build_all(&root.join("assets/soundpacks/catalog.json"), &packs_dir)?;

    let app = root.join("target/bundle").join(format!("{APP_NAME}.app"));
    if app.exists() {
        std::fs::remove_dir_all(&app).map_err(|e| format!("{}: {e}", app.display()))?;
    }
    let contents = app.join("Contents");
    let resources = contents.join("Resources");
    for dir in [contents.join("MacOS"), resources.join("Packs"), resources.join("Licenses")] {
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }

    let exe = contents.join("MacOS/rustyvibes");
    if let [only] = binaries.as_slice() {
        copy(only, &exe)?;
    } else {
        run_cmd(Command::new("lipo").arg("-create").arg("-output").arg(&exe).args(&binaries))?;
    }
    write(&contents.join("Info.plist"), &info_plist(version))?;
    write(&contents.join("PkgInfo"), "APPL????")?;
    copy(&root.join("assets/icon/AppIcon.icns"), &resources.join("AppIcon.icns"))?;
    for report in &reports {
        let name = format!("{}.rvpack", report.id);
        copy(&packs_dir.join(&name), &resources.join("Packs").join(&name))?;
    }
    for (source, name) in [
        ("LICENSE", "Rustyvibes.txt"),
        ("THIRD_PARTY_NOTICES.md", "Third-Party Notices.txt"),
        ("assets/soundpacks/mechvibes/LICENSE", "Mechvibes.txt"),
        ("assets/soundpacks/kbsim/LICENSE.md", "kbsim.txt"),
    ] {
        copy(&root.join(source), &resources.join("Licenses").join(name))?;
    }

    let wanted = if options.adhoc { None } else { developer_id() };
    let identity = sign(&app, wanted.as_deref())?;
    run_cmd(Command::new("codesign").args(["--verify", "--strict", "--verbose=2"]).arg(&app))?;

    let binary = std::fs::metadata(&exe).map(|m| m.len()).unwrap_or(0);
    println!(
        "{} ({}): binary {:.2} MB, bundle {:.1} MB, signed {}",
        app.display(),
        if binaries.len() > 1 { "universal" } else { "host architecture" },
        binary as f64 / 1e6,
        dir_size(&app) as f64 / 1e6,
        identity
            .as_deref()
            .map_or_else(|| "ad-hoc".to_owned(), |id| format!("with Developer ID {id}")),
    );
    Ok(Bundle { app, identity })
}

/// Builds `rustyvibes` in release mode for each target and returns the binaries.
fn compile(root: &Path, universal: bool) -> crate::Result<Vec<PathBuf>> {
    let host = if cfg!(target_arch = "aarch64") { TARGETS[0] } else { TARGETS[1] };
    let targets: Vec<&str> = if universal { TARGETS.to_vec() } else { vec![host] };
    if universal {
        let installed = output(Command::new("rustup").args(["target", "list", "--installed"]))?;
        for target in &targets {
            if !installed.lines().any(|line| line.trim() == *target) {
                run_cmd(Command::new("rustup").args(["target", "add", target]))?;
            }
        }
    }
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut binaries = Vec::new();
    for target in targets {
        run_cmd(
            Command::new(&cargo)
                .current_dir(root)
                .env("MACOSX_DEPLOYMENT_TARGET", MIN_MACOS)
                .args(["build", "--release", "--locked", "-p", "rustyvibes", "--target", target]),
        )?;
        binaries.push(root.join("target").join(target).join("release/rustyvibes"));
    }
    Ok(binaries)
}

pub fn info_plist(version: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key>
	<string>en</string>
	<key>CFBundleDisplayName</key>
	<string>{APP_NAME}</string>
	<key>CFBundleExecutable</key>
	<string>rustyvibes</string>
	<key>CFBundleIconFile</key>
	<string>AppIcon</string>
	<key>CFBundleIdentifier</key>
	<string>{BUNDLE_ID}</string>
	<key>CFBundleInfoDictionaryVersion</key>
	<string>6.0</string>
	<key>CFBundleName</key>
	<string>{APP_NAME}</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>{version}</string>
	<key>CFBundleVersion</key>
	<string>{version}</string>
	<key>LSApplicationCategoryType</key>
	<string>public.app-category.utilities</string>
	<key>LSMinimumSystemVersion</key>
	<string>{MIN_MACOS}</string>
	<key>LSUIElement</key>
	<true/>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSHumanReadableCopyright</key>
	<string>© 2021–2026 Kunal Bagaria · MIT License</string>
	<key>NSSupportsAutomaticTermination</key>
	<false/>
	<key>NSSupportsSuddenTermination</key>
	<true/>
</dict>
</plist>
"#
    )
}

/// SHA-1 of the first "Developer ID Application" signing identity, if any.
fn developer_id() -> Option<String> {
    let listing =
        output(Command::new("security").args(["find-identity", "-v", "-p", "codesigning"])).ok()?;
    listing
        .lines()
        .find(|line| line.contains("\"Developer ID Application:"))
        .and_then(|line| line.split_whitespace().nth(1))
        .map(str::to_owned)
}

/// Signs with `identity` (hardened runtime, secure timestamp), falling back to an
/// ad-hoc signature if that fails or hangs (e.g. a locked keychain).
fn sign(app: &Path, identity: Option<&str>) -> crate::Result<Option<String>> {
    if let Some(identity) = identity {
        let mut cmd = Command::new("codesign");
        cmd.args(["--force", "--options", "runtime", "--timestamp", "--sign", identity]).arg(app);
        match run_with_timeout(&mut cmd, Duration::from_secs(120)) {
            Ok(()) => return Ok(Some(identity.to_owned())),
            Err(e) => {
                eprintln!("warning: Developer ID signing failed ({e}); signing ad-hoc instead")
            }
        }
    }
    run_cmd(Command::new("codesign").args(["--force", "--sign", "-"]).arg(app))?;
    Ok(None)
}

pub(crate) fn run_cmd(cmd: &mut Command) -> crate::Result<()> {
    let status = cmd.status().map_err(|e| format!("{cmd:?}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("{cmd:?} failed with {status}")) }
}

fn run_with_timeout(cmd: &mut Command, timeout: Duration) -> crate::Result<()> {
    let mut child = cmd.stdin(Stdio::null()).spawn().map_err(|e| format!("{cmd:?}: {e}"))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return if status.success() { Ok(()) } else { Err(format!("exit status {status}")) };
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(format!("timed out after {} s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn output(cmd: &mut Command) -> crate::Result<String> {
    let out = cmd.output().map_err(|e| format!("{cmd:?}: {e}"))?;
    if !out.status.success() {
        return Err(format!("{cmd:?} failed with {}", out.status));
    }
    String::from_utf8(out.stdout).map_err(|e| e.to_string())
}

pub(crate) fn copy(from: &Path, to: &Path) -> crate::Result<()> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} → {}: {e}", from.display(), to.display()))
}

fn write(path: &Path, text: &str) -> crate::Result<()> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

pub(crate) fn dir_size(path: &Path) -> u64 {
    std::fs::read_dir(path)
        .map(|entries| {
            entries
                .flatten()
                .map(|e| {
                    let p = e.path();
                    if p.is_dir() { dir_size(&p) } else { e.metadata().map_or(0, |m| m.len()) }
                })
                .sum()
        })
        .unwrap_or(0)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_plist_has_the_required_keys() {
        let plist = info_plist("2.0.0");
        for needle in [
            "<key>CFBundleIdentifier</key>\n\t<string>io.github.kb24x7.rustyvibes</string>",
            "<key>CFBundleExecutable</key>\n\t<string>rustyvibes</string>",
            "<key>CFBundleShortVersionString</key>\n\t<string>2.0.0</string>",
            "<key>LSMinimumSystemVersion</key>\n\t<string>13.0</string>",
            "<key>LSUIElement</key>\n\t<true/>",
            "<key>CFBundleIconFile</key>\n\t<string>AppIcon</string>",
        ] {
            assert!(plist.contains(needle), "missing {needle}");
        }
        assert!(plist.starts_with("<?xml"));
    }
}
