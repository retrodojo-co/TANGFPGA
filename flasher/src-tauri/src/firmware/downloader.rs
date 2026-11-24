use super::{manifest::FirmwareManifest, validator};
use crate::utils::errors::{invalid, Result};
use reqwest::{blocking::Client, Url};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
const MAX_DOWNLOAD: u64 = 64 * 1024 * 1024;
pub fn secure_url(value: &str) -> Result<Url> {
    let url = Url::parse(value).map_err(|_| invalid("Invalid URL"))?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid(
            "Downloads require HTTPS without embedded credentials",
        ));
    }
    Ok(url)
}
fn client() -> Result<Client> {
    Ok(Client::builder()
        .user_agent("NanoBlast-Setup/0.1")
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120))
        .build()?)
}
pub(crate) fn get(url: &str, limit: u64) -> Result<Vec<u8>> {
    secure_url(url)?;
    let response = client()?.get(url).send()?.error_for_status()?;
    if response.content_length().unwrap_or(0) > limit {
        return Err(invalid("Download exceeds size limit"));
    }
    let mut bytes = Vec::new();
    response.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(invalid("Download exceeds size limit"));
    }
    Ok(bytes)
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub name: Option<String>,
    pub body: Option<String>,
    pub published_at: Option<String>,
    pub assets: Vec<Asset>,
    pub draft: bool,
    pub prerelease: bool,
}
pub fn releases(repository: &str) -> Result<Vec<Release>> {
    let url = secure_url(repository)?;
    let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
    if url.host_str() != Some("github.com")
        || parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || !s
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_ .".replace(' ', "").contains(c))
        })
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid(
            "Enter a GitHub repository URL: https://github.com/owner/repository",
        ));
    }
    let list: Vec<Release> = serde_json::from_slice(&get(
        &format!(
            "https://api.github.com/repos/{}/{}/releases?per_page=100",
            parts[0], parts[1]
        ),
        4 * 1024 * 1024,
    )?)?;
    Ok(list.into_iter().filter(|r| !r.draft).collect())
}
pub fn download_manifest(url: &str, cache: &Path) -> Result<(FirmwareManifest, PathBuf)> {
    let manifest: FirmwareManifest = serde_json::from_slice(&get(url, 256 * 1024)?)?;
    manifest.validate()?;
    let bytes = get(
        manifest
            .download_url
            .as_deref()
            .ok_or_else(|| invalid("Release manifest has no download URL"))?,
        MAX_DOWNLOAD,
    )?;
    std::fs::create_dir_all(cache)?;
    let temp = cache.join("download.part");
    std::fs::write(&temp, bytes)?;
    if let Err(e) = validator::verify(&temp, &manifest.sha256) {
        let _ = std::fs::remove_file(temp);
        return Err(e);
    }
    let target = cache.join(format!("{}.fs", manifest.sha256.to_ascii_lowercase()));
    std::fs::rename(temp, &target)?;
    Ok((manifest, target))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_https() {
        assert!(secure_url("http://example.com/a").is_err());
        assert!(secure_url("https://user:pass@example.com").is_err());
    }
}
