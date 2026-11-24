use super::output;
use crate::{
    storage::detector::StorageDevice,
    utils::errors::{Error, Result},
};
use plist::Value;
fn val(bytes: &[u8]) -> Result<Value> {
    Value::from_reader(std::io::Cursor::new(bytes)).map_err(|e| Error::Io(e.to_string()))
}
fn field<'a>(v: &'a Value, k: &str) -> Option<&'a Value> {
    v.as_dictionary().and_then(|d| d.get(k))
}
fn text(v: &Value, k: &str) -> String {
    field(v, k).and_then(Value::as_string).unwrap_or("").into()
}
fn boolean(v: &Value, k: &str) -> Option<bool> {
    field(v, k).and_then(Value::as_boolean)
}
fn size(v: &Value, k: &str) -> u64 {
    field(v, k)
        .and_then(Value::as_unsigned_integer)
        .unwrap_or(0)
}
fn info(id: &str) -> Result<Value> {
    val(&output("/usr/sbin/diskutil", &["info", "-plist", id])?)
}
pub fn scan() -> Result<Vec<StorageDevice>> {
    let list = val(&output(
        "/usr/sbin/diskutil",
        &["list", "-plist", "external", "physical"],
    )?)?;
    let mut result = Vec::new();
    for disk in field(&list, "AllDisksAndPartitions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let parent_id = text(disk, "DeviceIdentifier");
        let parent = info(&parent_id)?;
        let contains_os = field(disk, "Partitions")
            .and_then(Value::as_array)
            .is_some_and(|parts| {
                parts.iter().any(|p| {
                    ["Apple_APFS", "Apple_HFS", "Apple_Boot"].contains(&text(p, "Content").as_str())
                })
            });
        let removable = !contains_os
            && boolean(&parent, "Internal") == Some(false)
            && (boolean(&parent, "RemovableMedia") == Some(true)
                || boolean(&parent, "Ejectable") == Some(true));
        for partition in field(disk, "Partitions")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let id = text(partition, "DeviceIdentifier");
            let p = info(&id)?;
            let mount = text(&p, "MountPoint");
            let uuid = text(&p, "VolumeUUID");
            result.push(StorageDevice {
                id: format!("/dev/{id}"),
                name: text(&parent, "MediaName"),
                mount: if mount.is_empty() {
                    None
                } else {
                    Some(mount.clone())
                },
                capacity: size(&p, "TotalSize"),
                available: size(&p, "VolumeFreeSpace"),
                filesystem: text(&p, "FilesystemName"),
                removable,
                system: mount == "/" || boolean(&p, "Internal") != Some(false),
                identity: if uuid.is_empty() {
                    String::new()
                } else {
                    format!("{parent_id}:{uuid}")
                },
            });
        }
    }
    Ok(result)
}
fn valid(d: &StorageDevice) -> Result<()> {
    if !d.id.starts_with("/dev/disk")
        || !d.id[9..].chars().all(|c| c.is_ascii_digit() || c == 's')
        || !d.id[9..].contains('s')
    {
        return Err(Error::Unsafe(
            "Expected a macOS partition identifier".into(),
        ));
    }
    Ok(())
}
pub fn format(d: &StorageDevice) -> Result<()> {
    valid(d)?;
    output(
        "/usr/sbin/diskutil",
        &["eraseVolume", "FAT32", "NANOBLAST", &d.id],
    )?;
    Ok(())
}
pub fn eject(d: &StorageDevice) -> Result<()> {
    valid(d)?;
    output("/usr/sbin/diskutil", &["unmount", &d.id])?;
    Ok(())
}
