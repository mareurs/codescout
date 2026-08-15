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

/// Escape SQL `LIKE` wildcard characters (`%`, `_`) and the escape character
/// itself (`\`) in a *needle* (user-supplied pattern) before interpolating it
/// into a `LIKE` clause. Callers MUST pair this with `ESCAPE '\\'` on the SQL
/// side — this function only prepares the bound parameter, it does not touch
/// the query text.
pub(crate) fn escape_like_pattern(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_like_pattern_escapes_percent() {
        assert_eq!(escape_like_pattern("100%"), "100\\%");
    }

    #[test]
    fn escape_like_pattern_escapes_underscore() {
        assert_eq!(escape_like_pattern("foo_bar"), "foo\\_bar");
    }

    #[test]
    fn escape_like_pattern_escapes_backslash() {
        assert_eq!(escape_like_pattern("a\\b"), "a\\\\b");
    }

    #[test]
    fn escape_like_pattern_passes_through_ordinary_chars() {
        assert_eq!(escape_like_pattern("docs/trackers.md"), "docs/trackers.md");
    }

    #[test]
    fn escape_like_pattern_combined_input() {
        assert_eq!(escape_like_pattern("50%_off\\sale"), "50\\%\\_off\\\\sale");
    }

    #[test]
    fn normalize_is_noop_on_unix_style_input() {
        assert_eq!(normalize_rel_path("docs/specs/a.md"), "docs/specs/a.md");
    }

    #[cfg(windows)]
    #[test]
    fn normalize_replaces_backslash_on_windows() {
        assert_eq!(normalize_rel_path("docs\\specs\\a.md"), "docs/specs/a.md");
    }

    /// DRY gate: the Rust-side LIKE-escape idiom (the `.replace(...)` call
    /// that escapes a literal percent sign for a `LIKE` pattern — the
    /// distinctive middle step of the backslash / percent / underscore
    /// triple-replace) must appear exactly once in the tree — inside
    /// `escape_like_pattern` itself. A second occurrence means someone
    /// copy-pasted the idiom instead of calling the helper, which is exactly
    /// the footgun that produced the unescaped-LIKE bug in `resolve_cite_ref`
    /// (see docs/issues/archive/2026-07-17-like-escape-idiom-duplicated-no-shared-helper.md).
    /// SQL-side `REPLACE(REPLACE(REPLACE(col, ...)))` haystack-escaping
    /// (worktree.rs / merge_worktree.rs) uses SQL string literals, not this
    /// Rust `.replace` signature, so it never false-matches here.
    #[test]
    fn like_escape_idiom_is_not_inlined_outside_helper() {
        let needle: String = [
            '.', 'r', 'e', 'p', 'l', 'a', 'c', 'e', '(', '\'', '%', '\'', ',', ' ', '"', '\\',
            '\\', '%', '"', ')',
        ]
        .into_iter()
        .collect();
        // Forward-slash normalised on both sides of the comparison below.
        // `CARGO_MANIFEST_DIR` is OS-shaped (`D:\a\codescout\codescout` on
        // Windows), and walkdir appends OS separators to it, so the actual hit
        // came out mixed (`...codescout/src\librarian\util.rs`) while the
        // expected value used forward slashes throughout. See
        // docs/issues/archive/2026-08-06-windows-doctor-rehome-and-index-lock-tests-fail.md
        let root = normalize_rel_path(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut hits: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let count = content.matches(needle.as_str()).count();
            if count > 0 {
                hits.push(format!(
                    "{} ({count})",
                    normalize_rel_path(&path.display().to_string())
                ));
            }
        }
        assert_eq!(
            hits,
            vec![format!("{root}/librarian/util.rs (1)")],
            "LIKE-escape idiom must appear only inside escape_like_pattern; \
             route new call sites through crate::librarian::util::escape_like_pattern \
             instead of re-inlining the idiom by hand — found: {hits:?}"
        );
    }
}
