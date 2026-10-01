//! `cargo xtask notarize --keychain-profile <name>`: bundles the app, has Apple's
//! notary service check it and staples the ticket, then builds the disk image
//! around the stapled app and notarizes and staples that too, so both pass
//! Gatekeeper offline.
//!
//! The Apple credentials stay in the login keychain under a profile made once
//! with `xcrun notarytool store-credentials <name>`; this tool only passes the
//! profile name on to `notarytool`.

use std::path::Path;
use std::process::Command;

use crate::bundle::{self, Options, run_cmd};
use crate::dmg;

pub fn run(root: &Path, args: &[String]) -> crate::Result<()> {
    let profile = profile_arg(args)?;
    let built = bundle::bundle(root, &Options::parse(args))?;
    let identity = built.identity.as_deref().ok_or(
        "notarization needs a Developer ID Application signature, but the app was signed \
         ad-hoc (no such identity in the keychain, or --adhoc was given)",
    )?;

    // The notary service takes zips, disk images and installer packages, not bare bundles.
    let zip = root.join("target").join(format!("{}.zip", bundle::APP_NAME));
    if zip.exists() {
        std::fs::remove_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    }
    run_cmd(Command::new("ditto").args(["-c", "-k", "--keepParent"]).arg(&built.app).arg(&zip))?;
    submit(&zip, &profile)?;
    std::fs::remove_file(&zip).map_err(|e| format!("{}: {e}", zip.display()))?;
    staple(&built.app)?;

    let image = dmg::create(root, &built.app, Some(identity))?;
    submit(&image, &profile)?;
    staple(&image)?;

    run_cmd(Command::new("spctl").args(["--assess", "--type", "execute", "-v"]).arg(&built.app))?;
    run_cmd(
        Command::new("spctl")
            .args(["--assess", "--type", "open", "--context", "context:primary-signature", "-v"])
            .arg(&image),
    )
}

/// The `--keychain-profile` value. Required: without credentials there is nothing to do.
fn profile_arg(args: &[String]) -> crate::Result<String> {
    args.iter()
        .position(|a| a == "--keychain-profile")
        .and_then(|i| args.get(i + 1))
        .filter(|name| !name.starts_with("--"))
        .cloned()
        .ok_or_else(|| {
            "notarize needs --keychain-profile <name>, naming a notarytool profile created once \
             with `xcrun notarytool store-credentials <name>`"
                .to_owned()
        })
}

/// Uploads `file` and waits for the verdict. A rejection's error carries the
/// service's log, which names each offending file and the reason.
fn submit(file: &Path, profile: &str) -> crate::Result<()> {
    println!("notarizing {} (usually a few minutes)", file.display());
    let output = Command::new("xcrun")
        .args(["notarytool", "submit"])
        .arg(file)
        .args(["--keychain-profile", profile, "--wait", "--output-format", "json"])
        .output()
        .map_err(|e| format!("xcrun notarytool: {e}"))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let Some((id, status)) = parse_submission(&stdout) else {
        return Err(format!(
            "notarytool submit failed with {}: {} {}",
            output.status,
            stdout.trim(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    };
    if status == "Accepted" {
        println!("accepted (submission {id})");
        return Ok(());
    }
    let log = Command::new("xcrun")
        .args(["notarytool", "log", &id, "--keychain-profile", profile])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default();
    Err(format!("notarization of {} ended {status} (submission {id}):\n{log}", file.display()))
}

/// Finds the submission id and status in `notarytool --output-format json` output:
/// compact objects, one per line, the verdict last.
fn parse_submission(output: &str) -> Option<(String, String)> {
    output.lines().rev().find_map(|line| {
        let value: serde_json::Value = serde_json::from_str(line).ok()?;
        Some((value["id"].as_str()?.to_owned(), value["status"].as_str()?.to_owned()))
    })
}

/// Attaches the notarization ticket so Gatekeeper need not look it up online.
fn staple(path: &Path) -> crate::Result<()> {
    run_cmd(Command::new("xcrun").args(["stapler", "staple"]).arg(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn keychain_profile_is_required() {
        assert_eq!(
            profile_arg(&args(&["--keychain-profile", "rustyvibes"])).unwrap(),
            "rustyvibes"
        );
        for bad in [&[][..], &["--keychain-profile"][..], &["--keychain-profile", "--native"][..]] {
            let err = profile_arg(&args(bad)).unwrap_err();
            assert!(err.contains("store-credentials"), "{err}");
        }
    }

    #[test]
    fn reads_the_submission_result() {
        let accepted =
            r#"{"id":"2f1c7b0e-1111","status":"Accepted","message":"Processing complete"}"#;
        assert_eq!(
            parse_submission(accepted),
            Some(("2f1c7b0e-1111".to_owned(), "Accepted".to_owned()))
        );
        let invalid =
            "\n{\"id\":\"x-2\",\"status\":\"Invalid\",\"message\":\"Processing complete\"}\n";
        assert_eq!(parse_submission(invalid), Some(("x-2".to_owned(), "Invalid".to_owned())));
        let upload_then_verdict = concat!(
            "{\"id\":\"x-3\",\"path\":\"/tmp/a.zip\",\"message\":\"Successfully uploaded file\"}\n",
            "{\"id\":\"x-3\",\"status\":\"Accepted\",\"message\":\"Processing complete\"}\n",
        );
        assert_eq!(
            parse_submission(upload_then_verdict),
            Some(("x-3".to_owned(), "Accepted".to_owned()))
        );
        assert_eq!(parse_submission("Error: HTTP status code: 401"), None);
        assert_eq!(parse_submission(r#"{"status":"Accepted"}"#), None);
    }
}
