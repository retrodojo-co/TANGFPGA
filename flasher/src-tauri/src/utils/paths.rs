use crate::utils::errors::{invalid, Error, Result};
use std::path::{Component, Path, PathBuf};
// FAT32 paths must also be safe when evaluated by Windows, regardless of host OS.
pub fn relative(value: &str) -> Result<&Path> {
    let p = Path::new(value);
    if value.is_empty()
        || value.contains(['\\', ':', '\0'])
        || p.is_absolute()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || value.split('/').any(|s| {
            s.is_empty()
                || s == "."
                || s == ".."
                || s.ends_with(['.', ' '])
                || s.chars().any(|c| c.is_control() || "<>\"|?*".contains(c))
                || {
                    let stem = s.split('.').next().unwrap_or("").to_ascii_uppercase();
                    [
                        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
                        "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6",
                        "LPT7", "LPT8", "LPT9",
                    ]
                    .contains(&stem.as_str())
                }
        })
    {
        return Err(invalid("Invalid relative FAT32 path"));
    }
    Ok(p)
}
pub fn confined(root: &Path, name: &str) -> Result<PathBuf> {
    let rel = relative(name)?;
    let canonical = root.canonicalize()?;
    if std::fs::symlink_metadata(root)?.file_type().is_symlink() {
        return Err(Error::Unsafe("Symlink root refused".into()));
    }
    let mut path = canonical.clone();
    for part in rel.components() {
        path.push(part);
        match std::fs::symlink_metadata(&path) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::Unsafe("Symlink in destination refused".into()))
            }
            Ok(_) => {
                if !path.canonicalize()?.starts_with(&canonical) {
                    return Err(Error::Unsafe("Path escaped selected root".into()));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn traversal_and_windows_paths_rejected() {
        for path in [
            "../x", "/x", "a/../x", "C:\\x", "a\\b", "a//b", "NUL.fs", "x.", "a:stream", "./x",
        ] {
            assert!(relative(path).is_err(), "{path}");
        }
        assert!(relative("menu/config.bin").is_ok());
    }
    #[cfg(unix)]
    #[test]
    fn symlink_escape_rejected() {
        let root = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink("/tmp", root.path().join("escape")).unwrap();
        assert!(confined(root.path(), "escape/file").is_err());
    }
}
