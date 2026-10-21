//! Pure file-content hashing for memory anchor staleness detection.
//!
//! Lives here (rather than in `embed::index`) so it survives the eventual
//! removal of the legacy retrieval index. Only consumer post-cleanup is
//! `memory::anchors`.

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;

/// The most bytes of one file [`hash_file`] / [`hash_file_or_empty`] will read. A memory anchor
/// names a source file of the project, so this is generous (no source file approaches it); it is
/// there so that a path which is not what it was when anchored (a link to `/dev/zero`, a data
/// dump committed under a source name) cannot fill memory. A file over it is refused like a
/// special file, not hashed.
// cap-class: NOT_A_CAP — bounds how many bytes of an INPUT file are read for hashing; a file over it is refused whole with an error, no result is shortened and nothing is presented as complete
pub const MAX_HASHED_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// Hash the content of a file for change detection.
///
/// The file must be a regular file once links are followed, of at most
/// [`MAX_HASHED_FILE_BYTES`]; otherwise the `Err` holds a
/// [`crate::util::fs::BoundedReadError`] ([`is_refusal`] tells those from an I/O failure). The
/// path comes from a memory anchor, i.e. from the repository, and a plain read of a FIFO held the
/// caller for good.
pub fn hash_file(path: &Path) -> Result<String> {
    hash_file_capped(path, MAX_HASHED_FILE_BYTES)
}

/// [`hash_file`] at a caller-chosen limit, so a test can put the limit under the size of a file
/// that exists (`/proc` files report length 0 but return bytes).
fn hash_file_capped(path: &Path, max_bytes: u64) -> Result<String> {
    let mut hasher = Sha256::new();
    crate::util::fs::stream_regular(path, max_bytes, |chunk| hasher.update(chunk))?;
    Ok(hex::encode(hasher.finalize()))
}

/// True when `err` (from [`hash_file`]) says the path is not a regular file or is over
/// [`MAX_HASHED_FILE_BYTES`]: a fact about what is at the path now, as opposed to a read that
/// failed (permission, a race with a delete).
pub fn is_refusal(err: &anyhow::Error) -> bool {
    use crate::util::fs::BoundedReadError;
    matches!(
        err.downcast_ref::<BoundedReadError>(),
        Some(BoundedReadError::NotRegular | BoundedReadError::TooLarge { .. })
    )
}
/// Hash a file's content for change detection, treating an unreadable file as
/// empty content.
///
/// Change detection needs a **total** function. A caller recording "what the
/// content was when I asked for this to be rewritten" has to be able to record a
/// baseline before the file exists, and `hash_file`'s `Err` gives it nothing to
/// store — so the absent case would have to be encoded as a sentinel by every
/// caller separately. Absent and empty are the same state for this purpose.
///
/// This deliberately absorbs a permission error into the same value as absence.
/// A path that is not a regular file (a FIFO, a device, a directory) or is over
/// [`MAX_HASHED_FILE_BYTES`] is absorbed the same way, and never read to its end.
/// That is safe only for change detection, where the consequence is a hash that
/// fails to move and therefore reports "not yet regenerated" — the conservative
/// direction. Do not use it where the distinction carries meaning.
pub fn hash_file_or_empty(path: &Path) -> String {
    hash_file(path).unwrap_or_else(|_| hex::encode(Sha256::digest(b"")))
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
/// The hashed paths come from memory anchors and from `.codescout/system-prompt.md`: repository
/// content, so a FIFO or a link to a device can stand where a file was.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_hash_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    fn sha_hex(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    #[test]
    fn a_fifo_or_a_link_to_one_is_refused_by_hash_file_and_hashes_empty_in_the_total_form() {
        let empty = sha_hex(b"");
        for linked in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let file = dir.path().join("system-prompt.md");
            if linked {
                let fifo = dir.path().join("elsewhere-pipe");
                make_fifo(&fifo);
                std::os::unix::fs::symlink(&fifo, &file).unwrap();
            } else {
                make_fifo(&file);
            }
            let f = file.clone();
            let err = within_deadline("hash_file on a FIFO", move || hash_file(&f)).unwrap_err();
            assert!(is_refusal(&err), "linked={linked}: {err:#}");
            let f = file.clone();
            let total = within_deadline("hash_file_or_empty on a FIFO", move || {
                hash_file_or_empty(&f)
            });
            assert_eq!(total, empty, "linked={linked}");
        }
    }

    /// A character device is opened and refused, never read. (`/dev/null`, not `/dev/zero`.)
    #[test]
    fn a_link_to_a_device_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("dev");
        std::os::unix::fs::symlink("/dev/null", &file).unwrap();
        assert!(is_refusal(&hash_file(&file).unwrap_err()));
        assert_eq!(hash_file_or_empty(&file), sha_hex(b""));
    }

    /// A directory is refused too; before, the read failed and the callers saw an I/O error.
    #[test]
    fn a_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        assert!(is_refusal(&hash_file(dir.path()).unwrap_err()));
    }

    /// Control: a regular file, a link to one, an empty one and a missing one hash as before; a
    /// missing file is an I/O error, not a refusal.
    #[test]
    fn regular_missing_and_empty_files_hash_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.rs");
        std::fs::write(&file, b"fn a() {}").unwrap();
        assert_eq!(hash_file(&file).unwrap(), sha_hex(b"fn a() {}"));
        assert_eq!(hash_file_or_empty(&file), sha_hex(b"fn a() {}"));

        let link = dir.path().join("link.rs");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(hash_file(&link).unwrap(), sha_hex(b"fn a() {}"));

        let empty = dir.path().join("empty.rs");
        std::fs::write(&empty, b"").unwrap();
        assert_eq!(hash_file(&empty).unwrap(), sha_hex(b""));

        let missing = dir.path().join("missing.rs");
        let err = hash_file(&missing).unwrap_err();
        assert!(!is_refusal(&err), "{err:#}");
        assert_eq!(hash_file_or_empty(&missing), sha_hex(b""));
    }

    /// The streamed digest equals the digest of the raw bytes at every size around the 64 KiB
    /// read chunk (one byte, one under, exact, one over, two chunks, two chunks and a byte), for
    /// the fallible and the total form alike.
    #[test]
    fn a_multi_chunk_file_hashes_to_the_digest_of_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("sized.rs");
        for size in [1usize, 65_535, 65_536, 65_537, 131_072, 131_073, 300_000] {
            let body: Vec<u8> = (0..size).map(|i| (i % 253) as u8).collect();
            std::fs::write(&file, &body).unwrap();
            assert_eq!(hash_file(&file).unwrap(), sha_hex(&body), "size {size}");
            assert_eq!(hash_file_or_empty(&file), sha_hex(&body), "size {size}");
        }
    }

    /// A `/proc` file reports length 0 but returns bytes: under a limit smaller than what it
    /// returns it is a refusal, never `Ok` with the digest of a prefix. (The limit is lowered
    /// through the private seam; at 64 MiB this small file would simply be hashed.)
    #[cfg(target_os = "linux")]
    #[test]
    fn a_file_that_lies_about_its_length_is_refused_not_hashed_from_a_prefix() {
        let err = hash_file_capped(Path::new("/proc/self/status"), 16).unwrap_err();
        assert!(is_refusal(&err), "{err:#}");
    }

    /// Over the limit is refused. The file starts with real content and is then extended
    /// sparse, so only the length check can refuse it.
    #[test]
    fn a_file_over_the_hashing_limit_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big.bin");
        std::fs::write(&path, b"head").unwrap();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(MAX_HASHED_FILE_BYTES + 1).unwrap();
        drop(file);
        assert!(is_refusal(&hash_file(&path).unwrap_err()));
        assert_eq!(hash_file_or_empty(&path), sha_hex(b""));
    }
}
