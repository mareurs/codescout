use sha2::{Digest, Sha256};

pub fn sha_of_bytes(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    format!("{:x}", h.finalize())
}

/// Normalize a relative path to POSIX separators.
///
/// Unconditional backslash-to-forward-slash replacement on every platform —
/// matches `crate::util::fs::to_forward_slash`. The earlier platform-conditional
/// shape (no-op when `MAIN_SEPARATOR == '/'`) was a latent Linux bug: a `rel`
/// string containing a literal `\` byte (e.g. produced by upstream code that
/// already touched a Windows path, or in cross-platform test fixtures) would
/// pass through unchanged on Linux, breaking catalog LIKE matches.
pub fn normalize_rel_path(rel: &str) -> String {
    rel.replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_is_noop_on_unix_style_input() {
        assert_eq!(normalize_rel_path("docs/specs/a.md"), "docs/specs/a.md");
    }

    #[cfg(windows)]
    #[test]
    fn normalize_replaces_backslash_on_windows() {
        assert_eq!(normalize_rel_path("docs\\specs\\a.md"), "docs/specs/a.md");
    }
}
