use super::output;
use crate::{
    storage::detector::StorageDevice,
    utils::errors::{Error, Result},
};
use serde_json::Value;
fn text(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or("").into()
}
fn number(v: &Value, key: &str) -> u64 {
    v[key]
        .as_u64()
        .or_else(|| v[key].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(0)
}
fn boolean(v: &Value, key: &str) -> bool {
    v[key].as_bool().unwrap_or_else(|| number(v, key) == 1)
}
fn mounts(v: &Value) -> Vec<String> {
    v["mountpoints"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
fn system(v: &Value) -> bool {
    mounts(v).iter().any(|m| {
        matches!(
            m.as_str(),
            "/" | "/boot" | "/boot/efi" | "/home" | "/usr" | "/var" | "[SWAP]"
        )
    }) || v["children"]
        .as_array()
        .is_some_and(|a| a.iter().any(system))
}
pub fn parse(bytes: &[u8]) -> Result<Vec<StorageDevice>> {
    let json: Value = serde_json::from_slice(bytes)?;
    let mut result = Vec::new();
    for disk in json["blockdevices"].as_array().into_iter().flatten() {
        if text(disk, "type") != "disk" {
            continue;
        }
        let removable = boolean(disk, "rm") || text(disk, "tran") == "usb";
        let is_system = system(disk);
        let children = disk["children"]
            .as_array()
            .cloned()
            .unwrap_or_else(|| vec![disk.clone()]);
        for part in children {
            if !["part", "disk"].contains(&text(&part, "type").as_str()) {
                continue;
            }
            let uuid = text(&part, "uuid");
            let fstype = text(&part, "fstype");
            let fs = if fstype == "vfat" && !text(&part, "fsver").eq_ignore_ascii_case("fat32") {
                format!("vfat ({})", text(&part, "fsver"))
            } else {
                fstype
            };
            result.push(StorageDevice {
                id: text(&part, "path"),
                name: text(disk, "model"),
                mount: mounts(&part).into_iter().next(),
                capacity: number(&part, "size"),
                available: 0,
                filesystem: fs,
                removable,
                system: is_system,
                identity: if uuid.is_empty() {
                    String::new()
                } else {
                    format!("{}:{}:{}", text(disk, "serial"), text(disk, "path"), uuid)
                },
            });
        }
    }
    Ok(result)
}
pub fn scan() -> Result<Vec<StorageDevice>> {
    parse(&output(
        "lsblk",
        &[
            "--json",
            "--bytes",
            "--output",
            "PATH,TYPE,MODEL,SERIAL,SIZE,FSTYPE,FSVER,MOUNTPOINTS,RM,TRAN,UUID",
        ],
    )?)
}
fn valid(d: &StorageDevice) -> Result<()> {
    if !d.id.starts_with("/dev/") || !d.id[5..].chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(Error::Unsafe("Invalid Linux device identifier".into()));
    }
    Ok(())
}
pub fn format(d: &StorageDevice) -> Result<()> {
    valid(d)?;
    // This function is only called by the elevated, non-GUI helper after validation.
    if d.mount.is_some() {
        output("/usr/bin/umount", &[&d.id])?;
    }
    let mut expected = d.clone();
    expected.mount = None;
    let fresh = super::super::storage::detector::revalidate(
        &super::super::storage::detector::NativeStorage,
        &expected,
    )?;
    let mkfs = ["/usr/sbin/mkfs.vfat", "/sbin/mkfs.vfat"]
        .into_iter()
        .find(|p| std::path::Path::new(p).is_file())
        .ok_or_else(|| Error::Io("Install dosfstools to format FAT32".into()))?;
    output(mkfs, &["-F", "32", "-n", "NANOBLAST", &fresh.id])?;
    Ok(())
}
pub fn eject(d: &StorageDevice) -> Result<()> {
    valid(d)?;
    output("udisksctl", &["unmount", "--block-device", &d.id])?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excludes_system_and_detects_fat32() {
        let data=br#"{"blockdevices":[{"path":"/dev/sda","type":"disk","tran":"usb","rm":true,"serial":"one","children":[{"path":"/dev/sda1","type":"part","size":100,"fstype":"vfat","fsver":"FAT32","uuid":"abc","mountpoints":["/media/card"]}]},{"path":"/dev/nvme0n1","type":"disk","rm":false,"children":[{"path":"/dev/nvme0n1p1","type":"part","size":100,"uuid":"root","mountpoints":["/"]}]}]}"#;
        let d = parse(data).unwrap();
        assert!(d[0].writable().is_ok());
        assert!(d[1].safe().is_err());
    }
}
