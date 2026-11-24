use crate::{
    firmware::validator,
    utils::{
        errors::{invalid, Error, Result},
        paths::{confined, relative},
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssetFile {
    pub source: String,
    pub destination: String,
    pub sha256: String,
    pub required: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Core {
    pub name: String,
    pub directory: String,
    pub extensions: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SdManifest {
    pub version: String,
    pub firmware_versions: Vec<String>,
    pub directories: Vec<String>,
    pub files: Vec<AssetFile>,
    pub cores: Vec<Core>,
}
impl SdManifest {
    pub fn validate(&self) -> Result<()> {
        if self.version.trim().is_empty()
            || self.firmware_versions.is_empty()
            || (self.files.is_empty() && self.directories.is_empty())
        {
            return Err(invalid(
                "SD manifest requires a version, compatible firmware versions and files or directories",
            ));
        }
        let mut files = HashSet::new();
        let mut extensions = HashSet::new();
        let mut dirs = HashSet::new();
        for d in &self.directories {
            relative(d)?;
            if !dirs.insert(d.to_lowercase()) {
                return Err(invalid("Duplicate directory"));
            }
        }
        for f in &self.files {
            relative(&f.source)?;
            relative(&f.destination)?;
            if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(invalid("Asset SHA-256 required"));
            }
            if !files.insert(f.destination.to_lowercase()) {
                return Err(invalid("Duplicate asset destination"));
            }
        }
        for core in &self.cores {
            relative(&core.directory)?;
            if core.name.trim().is_empty() || core.extensions.is_empty() {
                return Err(invalid("Core requires a name and ROM extensions"));
            }
            for e in &core.extensions {
                if e.is_empty()
                    || !e.bytes().all(|b| b.is_ascii_alphanumeric())
                    || !extensions.insert(e.to_lowercase())
                {
                    return Err(invalid(
                        "ROM extensions must be unique alphanumeric values without dots",
                    ));
                }
            }
        }
        for f in &files {
            if dirs.contains(f)
                || files
                    .iter()
                    .any(|other| other.starts_with(&format!("{f}/")))
                || self.cores.iter().any(|c| {
                    f == &c.directory.to_lowercase()
                        || f.starts_with(&format!("{}/", c.directory.to_lowercase()))
                })
            {
                return Err(invalid(
                    "System assets must not overlap ROM directories or directories",
                ));
            }
        }
        Ok(())
    }
    pub fn compatible(&self, firmware: &str, assets: &str) -> Result<()> {
        self.validate()?;
        if self.version != assets || !self.firmware_versions.iter().any(|v| v == firmware) {
            return Err(invalid("SD assets do not match selected firmware version"));
        }
        Ok(())
    }
}
pub fn collision(path: &Path) -> Result<bool> {
    if path.exists() {
        return Ok(true);
    }
    if let Some(parent) = path.parent() {
        if parent.exists() {
            let name = path.file_name().unwrap().to_string_lossy().to_lowercase();
            for e in fs::read_dir(parent)? {
                if e?.file_name().to_string_lossy().to_lowercase() == name {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
pub fn copy_new(source: &Path, destination: &Path, hash: Option<&str>) -> Result<()> {
    if collision(destination)? {
        return Err(invalid(format!(
            "File already exists; not overwritten: {}",
            destination.display()
        )));
    }
    if fs::metadata(source)?.len() > u32::MAX as u64 {
        return Err(invalid("File exceeds FAT32 file size limit"));
    }
    let mut input = fs::File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let result = (|| {
        io::copy(&mut input, &mut output)?;
        output.sync_all()?;
        drop(output);
        let expected = match hash {
            Some(s) => s.to_owned(),
            None => validator::hash(source)?,
        };
        validator::verify(destination, &expected)
    })();
    if result.is_err() {
        let _ = fs::remove_file(destination);
    }
    result
}
pub fn install(
    root: &Path,
    assets: &Path,
    manifest: &SdManifest,
    available: u64,
    cancel: &AtomicBool,
    mut check: impl FnMut() -> Result<()>,
    mut report: impl FnMut(String),
) -> Result<()> {
    manifest.validate()?;
    let mut planned: Vec<(PathBuf, PathBuf, &AssetFile)> = Vec::new();
    let mut required = 0u64;
    for f in &manifest.files {
        let src = confined(assets, &f.source)?;
        if !src.is_file() {
            if f.required {
                return Err(invalid(format!("Missing required asset: {}", f.source)));
            }
            continue;
        }
        validator::verify(&src, &f.sha256)?;
        let dst = confined(root, &f.destination)?;
        if collision(&dst)? {
            if !dst.is_file() || validator::verify(&dst, &f.sha256).is_err() {
                return Err(invalid(format!(
                    "Existing asset differs: {}. Back it up and remove it before retrying.",
                    f.destination
                )));
            }
            continue;
        }
        required = required
            .checked_add(fs::metadata(&src)?.len())
            .ok_or_else(|| invalid("Asset sizes overflow"))?;
        planned.push((src, dst, f));
    }
    if required > available {
        return Err(invalid("Not enough free space for system assets"));
    }
    check()?;
    for d in manifest
        .directories
        .iter()
        .chain(manifest.cores.iter().map(|c| &c.directory))
    {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        check()?;
        fs::create_dir_all(confined(root, d)?)?;
    }
    for (source, dest, asset) in planned {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        check()?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // Recheck ancestors after directory creation, before opening the file.
        let dest = confined(root, &asset.destination)?;
        copy_new(&source, &dest, Some(&asset.sha256))?;
        report(format!("Verified {}", asset.destination));
    }
    check()?;
    verify(root, manifest)?;
    Ok(())
}
pub fn verify(root: &Path, manifest: &SdManifest) -> Result<()> {
    for file in &manifest.files {
        let path = confined(root, &file.destination)?;
        if file.required || path.exists() {
            validator::verify(&path, &file.sha256)?;
        }
    }
    for d in &manifest.directories {
        if !confined(root, d)?.is_dir() {
            return Err(invalid(format!("Missing directory {d}")));
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest(hash: String) -> SdManifest {
        SdManifest {
            version: "test".into(),
            firmware_versions: vec!["test".into()],
            directories: vec!["system".into()],
            files: vec![AssetFile {
                source: "fixture".into(),
                destination: "system/fixture".into(),
                sha256: hash,
                required: true,
            }],
            cores: vec![],
        }
    }
    #[test]
    fn creates_copies_verifies_and_preserves() {
        let root = tempfile::tempdir().unwrap();
        let assets = tempfile::tempdir().unwrap();
        let src = assets.path().join("fixture");
        fs::write(&src, b"test asset").unwrap();
        let m = manifest(validator::hash(&src).unwrap());
        install(
            root.path(),
            assets.path(),
            &m,
            100,
            &AtomicBool::new(false),
            || Ok(()),
            |_| {},
        )
        .unwrap();
        verify(root.path(), &m).unwrap();
        install(
            root.path(),
            assets.path(),
            &m,
            100,
            &AtomicBool::new(false),
            || Ok(()),
            |_| {},
        )
        .unwrap();
        fs::write(root.path().join("system/fixture"), b"keep").unwrap();
        assert!(install(
            root.path(),
            assets.path(),
            &m,
            100,
            &AtomicBool::new(false),
            || Ok(()),
            |_| {}
        )
        .is_err());
        assert_eq!(
            fs::read(root.path().join("system/fixture")).unwrap(),
            b"keep"
        );
    }
    #[test]
    fn missing_and_cancelled() {
        let root = tempfile::tempdir().unwrap();
        let assets = tempfile::tempdir().unwrap();
        let m = manifest("0".repeat(64));
        assert!(install(
            root.path(),
            assets.path(),
            &m,
            100,
            &AtomicBool::new(false),
            || Ok(()),
            |_| {}
        )
        .is_err());
    }
}
#[cfg(test)]
mod actual_profile_tests {
    use super::*;
    #[test]
    fn nanoblast_profile_requires_no_sd_system_files_and_preserves_saves() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/nanoblast-v0.2.0/sd.json");
        let manifest: SdManifest = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        manifest
            .compatible("v0.2.0", "nanoblast-v0.2.0-sd1")
            .unwrap();
        assert!(manifest.files.is_empty());
        let card = tempfile::tempdir().unwrap();
        fs::create_dir_all(card.path().join("gb/saves")).unwrap();
        fs::write(card.path().join("gb/saves/keep.0"), b"existing save").unwrap();
        install(
            card.path(),
            card.path(),
            &manifest,
            0,
            &AtomicBool::new(false),
            || Ok(()),
            |_| {},
        )
        .unwrap();
        for name in ["nes", "sms", "gb", "snes", "gb/saves", "nanoblast"] {
            assert!(card.path().join(name).is_dir());
        }
        assert_eq!(
            fs::read(card.path().join("gb/saves/keep.0")).unwrap(),
            b"existing save"
        );
        fs::create_dir_all(card.path().join("nes/series")).unwrap();
        fs::write(
            card.path().join("nes/series/demo.nes"),
            b"user supplied test fixture",
        )
        .unwrap();
        let roms = super::super::roms::list(card.path(), &manifest).unwrap();
        assert_eq!(roms[0].path, "nes/series/demo.nes");
    }
}
