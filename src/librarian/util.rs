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

/// Build the SQL `LIKE … ESCAPE` predicate tail that matches a path column
/// **strictly under** `root_expr` — i.e. a proper descendant, never the root
/// itself. Compose it as `format!("… WHERE abs_path = ?1 OR abs_path {}",
/// descendant_path_like("?1"))`.
///
/// This is the SQL-side twin of [`escape_like_pattern`], and exists for the
/// case that one cannot serve: when the value carrying the wildcards is a
/// per-row **column** rather than a value already held in Rust
/// (`catalog::worktree::covering_conn` escapes `worktree_root`), the escaping
/// has to happen inside the query. Bound-parameter callers use it too, so the
/// predicate has exactly one spelling in the tree.
///
/// The escaping is not optional and the order matters — backslash first, then
/// `%`, then `_`. Without it a root like `.worktrees/fix_1` has its `_` read
/// as a single-character wildcard and false-matches an unrelated sibling such
/// as `.worktrees/fixe1`.
///
/// `root_expr` is SQL **text**, not a value: pass a placeholder (`"?1"`) or a
/// column name. Never interpolate a user string here — that is what the
/// placeholder is for.
pub(crate) fn descendant_path_like(root_expr: &str) -> String {
    format!(
        "LIKE REPLACE(REPLACE(REPLACE({root_expr}, '\\', '\\\\'), '%', '\\%'), '_', '\\_') \
         || '/%' ESCAPE '\\'"
    )
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
    /// The SQL-side haystack-escaping form uses SQL string literals, not this
    /// Rust `.replace` signature, so it never false-matches here — it has its
    /// own gate, `sql_descendant_like_is_not_inlined_outside_helper`, which is
    /// where that half of the law is enforced.
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

    /// DRY gate, SQL half: the nested triple-`REPLACE` that escapes a path
    /// value *inside a query* must appear exactly once in the tree — inside
    /// `descendant_path_like`.
    ///
    /// This is the sibling of `like_escape_idiom_is_not_inlined_outside_helper`
    /// and exists because that one cannot see this form: it greps for a Rust
    /// `.replace` call signature, while this idiom is SQL text. One law with
    /// two spellings needs two gates, or the unguarded spelling is the one that
    /// drifts. Before this gate the SQL form sat verbatim at four sites
    /// (`merge_worktree.rs` twice, `catalog/worktree.rs`, `tools/worktree.rs`)
    /// held together only by comments saying "mirrors" — a co-change contract
    /// enforced by prose, which is strictly worse than compiler-visible
    /// duplication because it proves someone knew and supplies no mechanism.
    ///
    /// The needle is assembled character-wise so this test's own source does
    /// not match it.
    #[test]
    fn sql_descendant_like_is_not_inlined_outside_helper() {
        let needle: String = [
            'R', 'E', 'P', 'L', 'A', 'C', 'E', '(', 'R', 'E', 'P', 'L', 'A', 'C', 'E', '(', 'R',
            'E', 'P', 'L', 'A', 'C', 'E', '(',
        ]
        .into_iter()
        .collect();
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
            "the SQL descendant-LIKE idiom must appear only inside \
             descendant_path_like; route new call sites through \
             crate::librarian::util::descendant_path_like instead of \
             re-inlining the REPLACE nest by hand — found: {hits:?}"
        );
    }

    #[test]
    fn descendant_path_like_escapes_all_three_characters() {
        let sql = descendant_path_like("?1");
        // Backslash first, then percent, then underscore — order matters, or
        // the escapes introduced by an earlier step get re-escaped by a later
        // one.
        let backslash = sql.find(r"'\', '\\'").expect("backslash step present");
        let percent = sql.find(r"'%', '\%'").expect("percent step present");
        let underscore = sql.find(r"'_', '\_'").expect("underscore step present");
        assert!(
            backslash < percent && percent < underscore,
            "escape steps must run backslash -> percent -> underscore: {sql}"
        );
    }

    #[test]
    fn descendant_path_like_matches_strict_descendants_only() {
        let sql = descendant_path_like("?1");
        assert!(
            sql.contains("|| '/%'"),
            "must anchor on a trailing separator so the root itself is NOT \
             matched and a sibling prefix cannot match: {sql}"
        );
        assert!(
            sql.trim_end().ends_with(r"ESCAPE '\'"),
            "the ESCAPE clause is what makes the REPLACE escaping mean \
             anything; without it the backslashes are literal: {sql}"
        );
    }

    #[test]
    fn descendant_path_like_interpolates_the_operand_it_is_given() {
        // The operand is SQL text — a placeholder at three call sites, a bare
        // COLUMN NAME at catalog::worktree::covering_conn. That second case is
        // the whole reason this helper exists alongside escape_like_pattern.
        assert!(descendant_path_like("?2").contains("REPLACE(?2,"));
        assert!(descendant_path_like("worktree_root").contains("REPLACE(worktree_root,"));
    }

    /// Characterization test for the SD-2 extraction: pins the EXACT SQL text
    /// the four former inline sites carried, so the refactor is provably
    /// behaviour-preserving rather than merely plausible. Whitespace included —
    /// a stray line-continuation in the helper's `format!` would change the
    /// query string even though SQL tolerates it, and this is the assertion
    /// that would catch it.
    ///
    /// If a future change to the predicate is INTENDED, update this literal in
    /// the same commit and say why. It is a snapshot, not a law.
    #[test]
    fn descendant_path_like_reproduces_the_pre_extraction_sql_exactly() {
        // `r` is interpolated rather than written inline so this expected
        // string does not itself contain the contiguous needle that
        // `sql_descendant_like_is_not_inlined_outside_helper` greps for. It
        // did on the first draft, and that gate caught it — which is the
        // behaviour we want from it.
        let r = "REPLACE";
        assert_eq!(
            descendant_path_like("?1"),
            format!(r"LIKE {r}({r}({r}(?1, '\', '\\'), '%', '\%'), '_', '\_') || '/%' ESCAPE '\'")
        );
    }
}
