//! A separate invocation of the same signed executable runs only the requested format.
//! It exits before constructing Tauri, and always re-enumerates hardware after elevation.
use crate::{
    storage::detector::{revalidate, NativeStorage, StorageDevice},
    utils::errors::{invalid, Result},
};
use std::path::Path;
fn encode(device: &StorageDevice) -> Result<String> {
    Ok(serde_json::to_vec(device)?
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}
fn decode(value: &str) -> Result<StorageDevice> {
    if value.len() > 32768 || !value.len().is_multiple_of(2) || !value.is_ascii() {
        return Err(invalid("Invalid format request"));
    }
    let bytes: std::result::Result<Vec<u8>, _> = (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16))
        .collect();
    Ok(serde_json::from_slice(
        &bytes.map_err(|_| invalid("Invalid request encoding"))?,
    )?)
}
pub fn entry() -> Option<i32> {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--format-volume") {
        return None;
    }
    let result = (|| {
        if std::env::var("NANOBLAST_SIMULATION").as_deref() == Ok("1") {
            return Err(invalid("Formatting is disabled in simulation"));
        }
        if args.len() != 3 {
            return Err(invalid("Invalid helper arguments"));
        }
        let expected = decode(&args[2])?;
        let fresh = revalidate(&NativeStorage, &expected)?;
        crate::platform::format_direct(&fresh)
    })();
    match result {
        Ok(()) => Some(0),
        Err(e) => {
            eprintln!("{e}");
            Some(1)
        }
    }
}
pub fn request(device: &StorageDevice) -> Result<()> {
    let exe = std::env::current_exe()?.canonicalize()?;
    let payload = encode(device)?;
    launch(&exe, &payload)
}
#[cfg(target_os = "linux")]
fn launch(exe: &Path, payload: &str) -> Result<()> {
    super::super::platform::output(
        "/usr/bin/pkexec",
        &[
            exe.to_str()
                .ok_or_else(|| invalid("Executable path is not UTF-8"))?,
            "--format-volume",
            payload,
        ],
    )?;
    Ok(())
}
#[cfg(target_os = "macos")]
fn launch(exe: &Path, payload: &str) -> Result<()> {
    // Shell and AppleScript escaping are applied to a locally obtained executable path only.
    let exe = exe
        .to_str()
        .ok_or_else(|| invalid("Executable path is not UTF-8"))?;
    let command = format!(
        "'{}' --format-volume {}",
        exe.replace('\'', "'\\''"),
        payload
    );
    let script = format!(
        "do shell script \"{}\" with administrator privileges",
        command.replace('\\', "\\\\").replace('"', "\\\"")
    );
    crate::platform::output("/usr/bin/osascript", &["-e", &script])?;
    Ok(())
}
#[cfg(target_os = "windows")]
fn launch(exe: &Path, payload: &str) -> Result<()> {
    let exe = exe
        .to_str()
        .ok_or_else(|| invalid("Executable path is not UTF-8"))?
        .replace('\'', "''");
    let script=format!("$ErrorActionPreference='Stop'; $p=Start-Process -FilePath '{exe}' -ArgumentList @('--format-volume','{payload}') -Verb RunAs -Wait -PassThru; if($p.ExitCode -ne 0) {{ throw 'Format helper failed or device identity changed. Refresh and retry.' }}");
    crate::platform::output(
        "powershell.exe",
        &["-NoProfile", "-NonInteractive", "-Command", &script],
    )?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn malformed_requests_rejected() {
        assert!(decode("not-json").is_err());
        assert!(decode("🙂").is_err());
        assert!(decode(&"a".repeat(32770)).is_err());
    }
}
