//! Repository-verified BL616 release metadata. These files are NEVER FPGA .fs inputs.
use super::validator;
use crate::utils::{
    errors::{invalid, Result},
    paths::confined,
};
use serde::{Deserialize, Serialize};
use std::path::Path;
pub const REPOSITORY: &str = "https://github.com/NANO-BLAST/NANOBLAST-Release";
pub const VERSION: &str = "v0.2.0";
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct MenuFirmware {
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CoreAsset {
    pub core_id: u32,
    pub name: String,
    pub file: String,
    pub offset: u32,
    pub bytes: u64,
    pub span: u32,
    pub sha256: String,
    pub raw_sha256: String,
    pub raw_crc32: u32,
    pub raw_bytes: u64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct NanoRelease {
    pub version: String,
    pub status: String,
    pub firmware: MenuFirmware,
    pub assets: Vec<CoreAsset>,
    pub core_format: String,
    pub source: String,
    pub experimental_cores: Vec<String>,
}
impl NanoRelease {
    pub fn validate(&self, root: &Path) -> Result<()> {
        if self.version != VERSION || self.core_format != "NBCRXZ01" || self.assets.len() != 4 {
            return Err(invalid("This utility currently understands only the verified NanoBlast v0.2.0 BL616 layout"));
        }
        if self.firmware.sha256
            != "89154be0b98d13d501eab3406c479fca544ef6ddebe9b84f982e410f21a00acb"
            || self.firmware.bytes != 668640
        {
            return Err(invalid("Unexpected menu firmware for the pinned release"));
        }
        let firmware = confined(root, "nanoblast-firmware.bin")?;
        validator::verify(&firmware, &self.firmware.sha256)?;
        if std::fs::metadata(&firmware)?.len() != self.firmware.bytes {
            return Err(invalid("Menu firmware size mismatch"));
        }
        let expected = [
            (
                1,
                0x201000,
                333364,
                "36feb7ed131880c6535f8bf77acd98bccc4688c8376bceae914359dc09683bfd",
            ),
            (
                5,
                0x253000,
                321392,
                "8b10f626550116d175e1cc1db2af496040e3a28cabf89228790c4ce4252431a3",
            ),
            (
                7,
                0x2a2000,
                351916,
                "81348b6247eb6abe43a1b2c89fa069223257401055468907a3e464937019388f",
            ),
            (
                2,
                0x2f8000,
                449296,
                "f87e8f92d0a4f432228fa427b0d56858840c8bfe401400b5aa942c617ae3574c",
            ),
        ];
        for (asset, (id, offset, size, hash)) in self.assets.iter().zip(expected) {
            if asset.core_id != id
                || asset.offset != offset
                || asset.bytes != size
                || asset.sha256 != hash
            {
                return Err(invalid("Core metadata does not match the verified release"));
            }
            let path = confined(root, &asset.file)?;
            validator::verify(&path, &asset.sha256)?;
            if std::fs::metadata(path)?.len() != asset.bytes {
                return Err(invalid("Compressed core size mismatch"));
            }
        }
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let value: Self = serde_json::from_slice(&std::fs::read(path)?)?;
        value.validate(
            path.parent()
                .ok_or_else(|| invalid("Release has no directory"))?,
        )?;
        Ok(value)
    }
}
pub const ARCHIVE_SHA256: &str = "dd4b392d814e1e68560f31a6c74910ffcdb20c9e6b72cf2cc078223e8f45422b";
pub fn download(url: &str, cache: &Path) -> Result<(NanoRelease, std::path::PathBuf)> {
    if url!="https://github.com/NANO-BLAST/NANOBLAST-Release/releases/download/v0.2.0/NANOBLAST-v0.2.0-firmware.zip" {return Err(invalid("Only the audited v0.2.0 release archive is currently supported. Other versions require a reviewed layout profile."));}
    unpack(&super::downloader::get(url, 16 * 1024 * 1024)?, cache)
}
pub fn unpack(bytes: &[u8], cache: &Path) -> Result<(NanoRelease, std::path::PathBuf)> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    if format!("{:x}", Sha256::digest(bytes)) != ARCHIVE_SHA256 {
        return Err(invalid(
            "Archive does not match the audited NanoBlast v0.2.0 release checksum",
        ));
    }
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| invalid(e.to_string()))?;
    let allowed = [
        "manifest.json",
        "SHA256SUMS",
        "nanoblast-firmware.bin",
        "cores/nes.core",
        "cores/sms-gamegear.core",
        "cores/gb-colour.core",
        "cores/snes-experimental.core",
    ];
    std::fs::create_dir_all(cache)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| invalid(e.to_string()))?;
        let name = file
            .name()
            .strip_prefix("NANOBLAST-v0.2.0/")
            .ok_or_else(|| invalid("Unexpected release archive prefix"))?
            .to_owned();
        if !allowed.contains(&name.as_str()) || file.is_dir() || file.size() > 4 * 1024 * 1024 {
            return Err(invalid("Unexpected release archive entry"));
        }
        let path = confined(cache, &name)?;
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut data = Vec::new();
        file.by_ref()
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut data)?;
        if data.len() > 4 * 1024 * 1024 {
            return Err(invalid("Release entry too large"));
        }
        std::fs::write(path, data)?;
    }
    let release = NanoRelease::load(&cache.join("manifest.json"))?;
    Ok((release, cache.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_archive_extracts_and_altered_archive_is_refused() {
        let bytes = std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/nanoblast-v0.2.0-firmware.zip"),
        )
        .unwrap();
        let dest = tempfile::tempdir().unwrap();
        let (release, _) = unpack(&bytes, dest.path()).unwrap();
        assert_eq!(release.version, "v0.2.0");
        let mut altered = bytes;
        altered[10] ^= 1;
        assert!(unpack(&altered, dest.path()).is_err());
    }
    #[test]
    fn actual_release_and_core_hashes_match() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/nanoblast-v0.2.0");
        NanoRelease::load(&root.join("manifest.json")).unwrap();
    }
    #[test]
    fn menu_binary_rejected_as_fpga_manifest() {
        let path =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/nanoblast-v0.2.0/manifest.json");
        let bytes = std::fs::read(path).unwrap();
        assert!(
            serde_json::from_slice::<super::super::manifest::FirmwareManifest>(&bytes).is_err()
        );
    }
}
