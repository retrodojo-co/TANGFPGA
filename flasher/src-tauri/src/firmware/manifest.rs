use crate::utils::{
    errors::{invalid, Result},
    paths::relative,
};
use serde::{Deserialize, Serialize};
pub const BOARD: &str = "tangnano20k";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FirmwareManifest {
    pub version: String,
    pub board: String,
    pub bitstream: String,
    pub download_url: Option<String>,
    pub sha256: String,
    pub release_date: String,
    pub sd_asset_version: String,
}
impl FirmwareManifest {
    pub fn validate(&self) -> Result<()> {
        if self.board != BOARD {
            return Err(invalid("Firmware is not intended for Tang Nano 20K"));
        }
        if self.version.trim().is_empty() || self.sd_asset_version.trim().is_empty() {
            return Err(invalid("Firmware and SD asset versions are required"));
        }
        relative(&self.bitstream)?;
        if self.bitstream.contains('/') || !self.bitstream.ends_with(".fs") {
            return Err(invalid("Bitstream must be a .fs filename"));
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid("SHA-256 must contain 64 hex characters"));
        }
        if let Some(url) = &self.download_url {
            super::downloader::secure_url(url)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn manifest() -> FirmwareManifest {
        FirmwareManifest {
            version: "test".into(),
            board: BOARD.into(),
            bitstream: "test.fs".into(),
            download_url: None,
            sha256: "a".repeat(64),
            release_date: "2026-01-01".into(),
            sd_asset_version: "test".into(),
        }
    }
    #[test]
    fn parsing_and_compatibility() {
        let m = manifest();
        m.validate().unwrap();
        let json = serde_json::to_string(&m).unwrap();
        serde_json::from_str::<FirmwareManifest>(&json)
            .unwrap()
            .validate()
            .unwrap();
        let mut wrong = m;
        wrong.board = "other".into();
        assert!(wrong.validate().is_err());
    }
    #[test]
    fn rejects_hash_and_traversal() {
        let mut m = manifest();
        m.sha256 = "bad".into();
        assert!(m.validate().is_err());
        m.sha256 = "0".repeat(64);
        m.bitstream = "../evil.fs".into();
        assert!(m.validate().is_err());
    }
}
