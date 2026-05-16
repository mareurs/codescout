use sha2::{Digest, Sha256};

pub fn sha_of_bytes(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    format!("{:x}", h.finalize())
}

/// Normalize a relative path to POSIX separators. No-op on Unix; on Windows,
/// replaces `\` with `/` so IDs, globs, and cross-platform clients all agree
/// on a single canonical form.
pub fn normalize_rel_path(rel: &str) -> String {
    if std::path::MAIN_SEPARATOR == '/' {
        rel.to_string()
    } else {
        rel.replace(std::path::MAIN_SEPARATOR, "/")
    }
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
