use crate::utils::errors::{Error, Result};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StorageDevice {
    pub id: String,
    pub name: String,
    pub mount: Option<String>,
    pub capacity: u64,
    pub available: u64,
    pub filesystem: String,
    pub removable: bool,
    pub system: bool,
    pub identity: String,
}
impl StorageDevice {
    pub fn safe(&self) -> Result<()> {
        if !self.removable
            || self.system
            || self.identity.is_empty()
            || self.capacity == 0
            || self.mount.as_deref() == Some("/")
        {
            return Err(Error::Unsafe(
                "Device is internal, ambiguous, or contains the operating system".into(),
            ));
        }
        Ok(())
    }
    pub fn fat32(&self) -> bool {
        matches!(
            self.filesystem.to_ascii_lowercase().as_str(),
            "fat32" | "vfat" | "ms-dos fat32"
        )
    }
    pub fn writable(&self) -> Result<&std::path::Path> {
        self.safe()?;
        if !self.fat32() {
            return Err(Error::Unsafe(
                "FAT32 required. Formatting is optional and erases data.".into(),
            ));
        }
        let root = self
            .mount
            .as_deref()
            .ok_or_else(|| Error::Unsafe("Mount the selected SD card first".into()))?;
        if root.is_empty() {
            return Err(Error::Unsafe("Empty mount point".into()));
        }
        Ok(std::path::Path::new(root))
    }
}
pub trait StorageAdapter {
    fn scan(&self) -> Result<Vec<StorageDevice>>;
    fn format(&self, device: &StorageDevice) -> Result<()>;
    fn eject(&self, device: &StorageDevice) -> Result<()>;
}
pub struct NativeStorage;
impl StorageAdapter for NativeStorage {
    fn scan(&self) -> Result<Vec<StorageDevice>> {
        crate::platform::scan()
    }
    fn format(&self, d: &StorageDevice) -> Result<()> {
        crate::platform::format(d)
    }
    fn eject(&self, d: &StorageDevice) -> Result<()> {
        crate::platform::eject(d)
    }
}
pub fn revalidate(adapter: &dyn StorageAdapter, expected: &StorageDevice) -> Result<StorageDevice> {
    let fresh = adapter
        .scan()?
        .into_iter()
        .find(|d| d.id == expected.id)
        .ok_or_else(|| Error::Unsafe("Selected SD card disconnected".into()))?;
    fresh.safe()?;
    if fresh.identity != expected.identity
        || fresh.capacity != expected.capacity
        || fresh.mount != expected.mount
        || fresh.filesystem != expected.filesystem
    {
        return Err(Error::Unsafe(
            "Device identity or mount changed. Select the SD card again.".into(),
        ));
    }
    Ok(fresh)
}
pub fn add_free_space(devices: &mut [StorageDevice]) {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    for device in devices {
        if let Some(mount) = &device.mount {
            if let Some(d) = disks
                .iter()
                .find(|d| d.mount_point() == std::path::Path::new(mount))
            {
                device.available = d.available_space();
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    pub fn card() -> StorageDevice {
        StorageDevice {
            id: "test".into(),
            name: "Test".into(),
            mount: Some("/media/test".into()),
            capacity: 100,
            available: 80,
            filesystem: "vfat".into(),
            removable: true,
            system: false,
            identity: "unique".into(),
        }
    }
    #[test]
    fn filesystem_and_unsafe_disk() {
        let mut d = card();
        assert!(d.writable().is_ok());
        d.removable = false;
        assert!(d.safe().is_err());
        d.removable = true;
        d.system = true;
        assert!(d.safe().is_err());
        d.system = false;
        d.filesystem = "exfat".into();
        assert!(d.writable().is_err());
    }
}
