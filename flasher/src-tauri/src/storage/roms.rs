use super::installer::{copy_new, SdManifest};
use crate::utils::{
    errors::{invalid, Error, Result},
    paths::{confined, relative},
};
use serde::Serialize;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Serialize)]
pub struct Rom {
    pub path: String,
    pub core: String,
    pub size: u64,
}
pub fn list(root: &Path, manifest: &SdManifest) -> Result<Vec<Rom>> {
    manifest.validate()?;
    let mut out = Vec::new();
    for core in &manifest.cores {
        let mut pending = vec![(core.directory.clone(), 0usize)];
        while let Some((relative_dir, depth)) = pending.pop() {
            if depth > 32 || out.len() > 100_000 {
                return Err(invalid("ROM tree exceeds safe scan limits"));
            }
            let dir = confined(root, &relative_dir)?;
            if !dir.exists() {
                continue;
            }
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                if kind.is_symlink() {
                    continue;
                }
                let name = entry
                    .file_name()
                    .into_string()
                    .map_err(|_| invalid("ROM path is not UTF-8"))?;
                if name.starts_with('.') {
                    continue;
                }
                let relative_path = format!("{relative_dir}/{name}");
                if kind.is_dir() {
                    pending.push((relative_path, depth + 1));
                    continue;
                }
                if !kind.is_file() {
                    continue;
                }
                let path = entry.path();
                let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if extension.eq_ignore_ascii_case("zip")
                    || core
                        .extensions
                        .iter()
                        .any(|e| e.eq_ignore_ascii_case(extension))
                {
                    out.push(Rom {
                        path: relative_path,
                        core: core.name.clone(),
                        size: entry.metadata()?.len(),
                    });
                }
            }
        }
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}
pub fn import(
    root: &Path,
    manifest: &SdManifest,
    files: &[String],
    available: u64,
    cancel: &AtomicBool,
    mut check: impl FnMut() -> Result<()>,
) -> Result<Vec<String>> {
    manifest.validate()?;
    let mut size = 0u64;
    let mut plan = Vec::new();
    let mut names = std::collections::HashSet::new();
    for source in files {
        let path = Path::new(source);
        if !path.is_file() {
            return Err(invalid("ROM source is not a file"));
        }
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        let core = if ext.eq_ignore_ascii_case("zip") {
            archive_core(path, manifest)?
        } else {
            manifest
                .cores
                .iter()
                .find(|c| c.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
                .ok_or_else(|| invalid(format!("Unsupported ROM extension: {ext}")))?
        };
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| invalid("ROM filename must be UTF-8"))?;
        relative(name)?;
        let target = format!("{}/{name}", core.directory);
        let dest = confined(root, &target)?;
        if !names.insert(target.to_lowercase()) || super::installer::collision(&dest)? {
            return Err(invalid(format!(
                "Duplicate ROM: {name}. Nothing will be overwritten."
            )));
        }
        size = size
            .checked_add(fs::metadata(path)?.len())
            .ok_or_else(|| invalid("ROM sizes overflow"))?;
        plan.push((path.to_path_buf(), target, core.directory.clone()));
    }
    if size > available {
        return Err(invalid("Insufficient SD card space"));
    }
    let mut installed = Vec::new();
    for (src, target, dir) in plan {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        check()?;
        fs::create_dir_all(confined(root, &dir)?)?;
        let dest = confined(root, &target)?;
        copy_new(&src, &dest, None)?;
        installed.push(target);
    }
    Ok(installed)
}
fn archive_core<'a>(path: &Path, manifest: &'a SdManifest) -> Result<&'a super::installer::Core> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)
        .map_err(|e| invalid(format!("Invalid ROM ZIP: {e}")))?;
    if archive.len() > 4096 || fs::metadata(path)?.len() > 64 * 1024 * 1024 {
        return Err(invalid("ZIP exceeds supported limits"));
    }
    let mut chosen = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| invalid(format!("Invalid ROM ZIP: {e}")))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| invalid("Unsafe ZIP member path"))?;
        if entry.is_dir()
            || entry.name().starts_with("__MACOSX/")
            || enclosed
                .file_name()
                .is_some_and(|s| s.to_string_lossy().starts_with('.'))
        {
            continue;
        }
        let ext = enclosed.extension().and_then(|s| s.to_str()).unwrap_or("");
        if let Some(core) = manifest
            .cores
            .iter()
            .find(|c| c.extensions.iter().any(|e| e.eq_ignore_ascii_case(ext)))
        {
            if chosen.is_some() {
                return Err(invalid("ZIP must contain exactly one supported ROM"));
            }
            if entry.encrypted()
                || !matches!(
                    entry.compression(),
                    zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
                )
            {
                return Err(invalid(
                    "ZIP must use stored or Deflate compression without encryption",
                ));
            }
            if entry.size() == 0 || entry.size() > 64 * 1024 * 1024 {
                return Err(invalid("ZIP ROM exceeds supported size limits"));
            }
            chosen = Some(core);
        }
    }
    chosen.ok_or_else(|| invalid("ZIP contains no supported ROM"))
}
pub fn remove(root: &Path, manifest: &SdManifest, path: &str) -> Result<()> {
    if !list(root, manifest)?.iter().any(|r| r.path == path) {
        return Err(invalid("Selected file is not an installed ROM"));
    }
    fs::remove_file(confined(root, path)?)?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zip_import_keeps_archive_and_hides_private_cache() {
        use std::io::Write;
        let manifest: SdManifest =
            serde_json::from_str(include_str!("../../resources/nanoblast-v0.2.0/sd.json")).unwrap();
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("Game (Brazil).zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        zip.start_file(
            "Game (Brazil).sms",
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        zip.write_all(b"fixture ROM").unwrap();
        zip.finish().unwrap();
        let imported = import(
            root.path(),
            &manifest,
            &[path.to_string_lossy().into_owned()],
            100000,
            &AtomicBool::new(false),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(imported, vec!["sms/Game (Brazil).zip"]);
        assert_eq!(
            fs::read(root.path().join(&imported[0])).unwrap(),
            fs::read(&path).unwrap()
        );
        fs::create_dir_all(root.path().join("sms/.nbzip/private")).unwrap();
        fs::write(root.path().join("sms/.nbzip/private/game.sms"), b"cache").unwrap();
        assert_eq!(list(root.path(), &manifest).unwrap().len(), 1);
    }
    #[test]
    fn zip_import_rejects_multiple_games_before_copy() {
        use std::io::Write;
        let manifest: SdManifest =
            serde_json::from_str(include_str!("../../resources/nanoblast-v0.2.0/sd.json")).unwrap();
        let source = tempfile::tempdir().unwrap();
        let path = source.path().join("collection.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for name in ["a.sms", "b.sms"] {
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"fixture").unwrap();
        }
        zip.finish().unwrap();
        assert!(archive_core(&path, &manifest).is_err());
    }
    #[test]
    fn duplicates_never_overwrite() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let file = source.path().join("demo.testrom");
        fs::write(&file, b"user data").unwrap();
        let m = SdManifest {
            version: "test".into(),
            firmware_versions: vec!["test".into()],
            directories: vec![],
            files: vec![super::super::installer::AssetFile {
                source: "asset".into(),
                destination: "asset".into(),
                sha256: "0".repeat(64),
                required: true,
            }],
            cores: vec![super::super::installer::Core {
                name: "Test core (fixture only)".into(),
                directory: "test-roms".into(),
                extensions: vec!["testrom".into()],
            }],
        };
        let files = vec![file.to_string_lossy().into_owned()];
        import(
            root.path(),
            &m,
            &files,
            100,
            &AtomicBool::new(false),
            || Ok(()),
        )
        .unwrap();
        assert!(import(
            root.path(),
            &m,
            &files,
            100,
            &AtomicBool::new(false),
            || Ok(())
        )
        .is_err());
        assert_eq!(list(root.path(), &m).unwrap().len(), 1);
        remove(root.path(), &m, "test-roms/demo.testrom").unwrap();
        assert!(list(root.path(), &m).unwrap().is_empty());
    }
}
