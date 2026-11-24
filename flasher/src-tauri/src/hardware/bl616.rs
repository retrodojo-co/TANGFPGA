//! Narrow, bundled BL616 helper. It performs backup/layout validation before any write.
use super::programmer::{NativeRunner, ProcessRunner, Reporter};
use crate::firmware::{nanoblast::NanoRelease, validator};
use crate::utils::errors::{invalid, Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};
#[derive(Clone, Serialize, Deserialize)]
pub struct Port {
    pub port: String,
    pub identity: String,
    pub description: String,
}
fn helper(resources: &Path) -> Result<PathBuf> {
    let root = resources.join("bl616/nanoblast-bl616");
    let exe = root.join(if cfg!(windows) {
        "nanoblast-bl616.exe"
    } else {
        "nanoblast-bl616"
    });
    let digest = std::fs::read_to_string(resources.join("bl616/sha256.txt")).map_err(|_| {
        invalid("The bundled BL616 updater is missing. Use a complete desktop build.")
    })?;
    validator::verify(&exe, digest.trim())?;
    Ok(exe)
}
pub fn ports(resources: &Path) -> Result<Vec<Port>> {
    let output = Command::new(helper(resources)?).arg("--list").output()?;
    if !output.status.success() {
        return Err(invalid(String::from_utf8_lossy(&output.stderr)));
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}
pub fn install(
    resources: &Path,
    root: &Path,
    backup: &Path,
    selected: &Port,
    confirmation: &str,
    report: Reporter,
) -> Result<()> {
    install_selected(
        resources,
        root,
        backup,
        selected,
        confirmation,
        report,
        None,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
pub fn install_selected(
    resources: &Path,
    root: &Path,
    backup: &Path,
    selected: &Port,
    confirmation: &str,
    report: Reporter,
    development: Option<(&Path, bool)>,
    current: Option<&Path>,
) -> Result<()> {
    if confirmation != format!("UPDATE {}", selected.port) {
        return Err(invalid(
            "Type UPDATE followed by the selected port to confirm menu installation",
        ));
    }
    NanoRelease::load(&root.join("manifest.json"))?;
    if !ports(resources)?
        .iter()
        .any(|p| p.port == selected.port && p.identity == selected.identity)
    {
        return Err(invalid("Selected UPDATE port disconnected or changed"));
    }
    let mut args = vec![
        "--release".into(),
        root.to_string_lossy().into_owned(),
        "--output".into(),
        backup.to_string_lossy().into_owned(),
        "--port".into(),
        selected.port.clone(),
        "--identity".into(),
        selected.identity.clone(),
        "--confirmation".into(),
        confirmation.into(),
    ];
    if let Some((path, restore)) = development {
        args.extend([
            "--development-menu".into(),
            path.to_string_lossy().into_owned(),
        ]);
        if restore {
            args.push("--restore-menu".into());
        }
    }
    if let Some(bundle) = current {
        args.extend([
            "--current-bundle".into(),
            bundle.to_string_lossy().into_owned(),
            "--trusted-receipts".into(),
            backup
                .parent()
                .and_then(Path::parent)
                .ok_or_else(|| invalid("Backup has no app data root"))?
                .join("verified-release-layouts.json")
                .to_string_lossy()
                .into_owned(),
        ]);
    }
    // A fresh success record is mandatory; stdout alone is not proof of verification.
    NativeRunner.run(&helper(resources)?, &args, report)?;
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(backup.join("result.json"))?)?;
    if record["status"] != "FULL_FLASH_READBACK_PASS"
        || record["expected_sha256"] != record["after_sha256"]
    {
        return Err(invalid("BL616 verification record missing or invalid"));
    }
    Ok(())
}

pub fn select_release(resources: &Path, source: &Path, cache: &Path) -> Result<serde_json::Value> {
    let output = Command::new(helper(resources)?)
        .args([
            "--select-release",
            &source.to_string_lossy(),
            "--output",
            &cache.to_string_lossy(),
            "--release",
            &resources.join("nanoblast-v0.2.0").to_string_lossy(),
        ])
        .output()?;
    if !output.status.success() {
        return Err(invalid(String::from_utf8_lossy(&output.stderr)));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let result = text
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix("RESULT "))
        .ok_or_else(|| invalid("Release validation returned no result"))?;
    Ok(serde_json::from_str(result)?)
}

pub fn files(
    resources: &Path,
    operation: &str,
    port: &str,
    identity: &str,
    remote: &str,
    local: Option<&Path>,
    report: Reporter,
) -> Result<serde_json::Value> {
    if !["ports", "list", "upload"].contains(&operation) {
        return Err(invalid("Unknown USB file operation"));
    }
    let mut args = vec![
        "--files".into(),
        operation.into(),
        "--port".into(),
        port.into(),
        "--identity".into(),
        identity.into(),
        "--remote".into(),
        remote.into(),
    ];
    if let Some(path) = local {
        args.extend(["--local".into(), path.to_string_lossy().into_owned()]);
    }
    let output = if operation == "upload" {
        NativeRunner
            .run(&helper(resources)?, &args, report)
            .map_err(|error| match error {
                Error::Hardware(message) if message.starts_with("Programming process failed") => {
                    let detail = message
                        .split_once('\n')
                        .map(|(_, s)| s.trim())
                        .unwrap_or(&message);
                    invalid(format!(
                        "USB ROM upload failed: {}",
                        detail.strip_prefix("ERROR: ").unwrap_or(detail)
                    ))
                }
                other => other,
            })?
    } else {
        let result = Command::new(helper(resources)?).args(&args).output()?;
        if !result.status.success() {
            return Err(invalid(String::from_utf8_lossy(&result.stderr)));
        }
        String::from_utf8_lossy(&result.stdout).into_owned()
    };
    let line = output
        .lines()
        .rev()
        .find_map(|s| s.strip_prefix("RESULT "))
        .ok_or_else(|| invalid("USB file helper returned no result"))?;
    Ok(serde_json::from_str(line)?)
}
