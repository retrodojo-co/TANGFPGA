use crate::{
    storage::detector::StorageDevice,
    utils::errors::{Error, Result},
};
use std::process::Command;
pub mod linux;
pub mod macos;
pub mod windows;
#[cfg(target_os = "linux")]
use linux as active;
#[cfg(target_os = "macos")]
use macos as active;
#[cfg(target_os = "windows")]
use windows as active;
pub fn scan() -> Result<Vec<StorageDevice>> {
    let mut devices = active::scan()?;
    crate::storage::detector::add_free_space(&mut devices);
    Ok(devices)
}
pub fn format(d: &StorageDevice) -> Result<()> {
    d.safe()?;
    crate::storage::privilege::request(d)
}
pub fn format_direct(d: &StorageDevice) -> Result<()> {
    d.safe()?;
    active::format(d)
}
pub fn eject(d: &StorageDevice) -> Result<()> {
    d.safe()?;
    active::eject(d)
}
pub fn output(exe: &str, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new(exe).args(args).output()?;
    if !out.status.success() {
        return Err(Error::Io(format!("{exe}: {}. If access was denied, authorize only this operation using your OS disk utility.",String::from_utf8_lossy(&out.stderr))));
    }
    Ok(out.stdout)
}
