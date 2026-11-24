use crate::utils::errors::{invalid, Result};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};
pub fn hash(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 65536];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
pub fn verify(path: &Path, expected: &str) -> Result<()> {
    if !hash(path)?.eq_ignore_ascii_case(expected) {
        return Err(invalid(format!("SHA-256 mismatch: {}", path.display())));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checksum_validation() {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), b"abc").unwrap();
        verify(
            f.path(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        )
        .unwrap();
        assert!(verify(f.path(), &"0".repeat(64)).is_err());
    }
}
