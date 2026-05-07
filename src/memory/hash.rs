//! Pure file-content hashing for memory anchor staleness detection.
//!
//! Lives here (rather than in `embed::index`) so it survives the eventual
//! removal of the legacy retrieval index. Only consumer post-cleanup is
//! `memory::anchors`.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// Hash the content of a file for change detection.
pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = Sha256::digest(&bytes);
    Ok(hex::encode(digest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn hash_file_produces_64_char_hex() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("test.rs");
        std::fs::write(&file, b"fn main() {}").unwrap();
        let hash = hash_file(&file).unwrap();
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_file_differs_for_different_content() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        let f2 = dir.path().join("b.rs");
        std::fs::write(&f1, b"fn a() {}").unwrap();
        std::fs::write(&f2, b"fn b() {}").unwrap();
        assert_ne!(hash_file(&f1).unwrap(), hash_file(&f2).unwrap());
    }
}
