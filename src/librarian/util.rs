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

/// Refuse a write whose TARGET PATH is itself a symbolic link, dangling or not.
///
/// **Why a refusal, not a replace and not a conditional write-through.** Every librarian writer
/// takes its path from the catalog (`row.abs_path`), from a frontmatter declaration or from
/// `base_dir.join(rel_path)` — none of them passes through `validate_write_path`, so nothing else
/// asks where the write lands. `std::fs::write` follows a leaf link, and the indexer deliberately
/// catalogs an in-root `.md` link whose target is OUTSIDE the root (`indexer.rs`, the
/// `is_symlink` block: skipping it would trade a silent duplicate for a silent omission), so a
/// catalogued row can name a file the repository does not own.
///
/// * Replacing the link with a regular file (what `util::fs::atomic_write` does for the
///   `create_file` / `edit_*` tools) is wrong HERE: the caller asked to update a document and the
///   call would report success while the real document stayed unchanged and the user's link
///   silently became a stale copy.
/// * Writing through "only when the target is inside the root" supports a case the indexer never
///   produces: a link that resolves inside the root is skipped as a duplicate name and gets no
///   row, so such a row exists only from before that skip or after a link was re-pointed. Through
///   it, the write would also leave the TARGET's own catalog row behind its file
///   (`row_behind_file`). The target is addressable under its own id, and that is the route.
///
/// So any path that is a link is refused, naming the link and where it points. A missing path is
/// `Ok` (a create has nothing to refuse until it exists), and so is an `lstat` that fails for any
/// other reason: the write that follows will fail by itself and say why.
///
/// Limit, stated so it is not read as a guarantee: this checks the LEAF only (a link in a parent
/// directory is [`refuse_path_outside_root`]'s question, asked by [`write_catalogued_file`]), and
/// the check and the write are two syscalls, so a link planted between them still wins.
/// `fs::write` has no `O_NOFOLLOW` form in `std`.
pub(crate) fn refuse_write_through_symlink(path: &std::path::Path) -> anyhow::Result<()> {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return Ok(());
    };
    if !meta.file_type().is_symlink() {
        return Ok(());
    }
    let target = std::fs::read_link(path)
        .map(|t| t.display().to_string())
        .unwrap_or_else(|_| "<unreadable>".to_string());
    Err(
        crate::librarian::tools::LibrarianRecoverableError::with_hint(
            format!(
            "refusing to write `{}`: it is a symbolic link (-> `{target}`), so the write would \
             modify a file the link points at rather than this path",
            path.display()
        ),
            "Nothing was written. A symbolic link is never written through, so the librarian \
         refuses to write this path (it can still move or delete a catalogued link itself). If the link points at a file inside this repository, change that file \
         under its own id; if it points outside, edit the target directly with your file tools, \
         or replace the link with a regular file if this path should hold its own content.",
        ),
    )
}

/// Write `contents` to `path` — a path the CATALOG (or a document's frontmatter) supplied — unless
/// the path is a symbolic link ([`refuse_write_through_symlink`]) or resolves outside the root the
/// caller manages ([`refuse_catalogued_path_outside_repo`]; `roots` is `tools::managed_roots`, or
/// empty where the caller has none, in which case the spelled repository is the fallback).
///
/// The one door the librarian's "rewrite a catalogued file in place" writers go through, so a
/// fourth writer added later takes the guard by calling this rather than by remembering it.
/// Returns `anyhow::Error` (the refusal is a recoverable error, an `io::Error` is wrapped) so a
/// caller's `?` and `.with_context` read the same as they did over `std::fs::write`.
pub(crate) fn write_catalogued_file(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
    contents: impl AsRef<[u8]>,
) -> anyhow::Result<()> {
    refuse_write_through_symlink(path)?;
    refuse_catalogued_path_outside_repo(roots, path, "write")?;
    std::fs::write(path, contents)?;
    Ok(())
}

/// The repository root a catalogued path SPELLS: the nearest ancestor that holds a `.git` and is
/// not itself a symbolic link.
///
/// Lexical on purpose, and the link exclusion is load-bearing. A row below a link directory is
/// exactly the case this exists for, and `lookup_git_root` would stop at the LINK when the link's
/// target is itself a repository (`docs -> ~/other-repo`): the "root" would then be the outside
/// repository and every path below it would read as inside. `None` when no ancestor qualifies
/// (a workspace that is not a git checkout, or a repository root reached through a link): the
/// containment checks then have nothing to compare against and the LEAF rule alone applies.
pub(crate) fn spelled_repo_root(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut cur = path.parent()?;
    loop {
        let is_link = std::fs::symlink_metadata(cur)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);
        if !is_link && cur.join(".git").exists() {
            return Some(cur.to_path_buf());
        }
        cur = cur.parent()?;
    }
}

/// [`refuse_path_outside_root`] for a catalog-supplied path, against ONE root chosen by
/// [`confinement_root`]; a path with no root at all is not checked.
///
/// This is the question the LEAF rule cannot ask: a row whose parent directory is a link out of
/// the repository. The indexer can mint one two ways, both shippable by a repository
/// (`force_include` walks a symlinked anchor directory; a directory swapped for a link after
/// indexing keeps its rows because reclamation asks `Path::exists()`), and every writer that
/// takes `row.abs_path` would then write, move or delete a file the repository does not own.
/// A link whose target stays INSIDE the repository is untouched: a symlinked tracker directory
/// is a legitimate setup.
///
/// `roots` are the caller's MANAGED roots (`tools::managed_roots`, git root first), the same list
/// `doc(move)` and `doc(delete)` consult, so the three agree on what "outside" means. The
/// first design used a root found by walking the spelled ancestors for a `.git`, which cannot
/// tell "a link to ANOTHER repository" (outside) from "my own project reached through a link"
/// (inside) — the two are the same input, and only the managed roots say which one this is.
pub(crate) fn refuse_catalogued_path_outside_repo(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
    verb: &str,
) -> anyhow::Result<()> {
    if let Some(root) = confinement_root(roots, path) {
        return refuse_path_outside_root(path, &root, verb);
    }
    // No managed root contains the SPELLING. A project reached through a link is registered under
    // its canonical path while rows keep the spelled one (`home/proj/x.md` against a root of
    // `data/proj`), so before falling back, accept a path whose RESOLUTION lies under a managed
    // root: it is inside something the caller manages.
    let resolved = resolve_through_existing_ancestor(path);
    if roots
        .iter()
        .any(|r| resolved.starts_with(resolve_through_existing_ancestor(r)))
    {
        return Ok(());
    }
    match spelled_repo_root(path) {
        Some(root) => refuse_path_outside_root(path, &root, verb),
        None => Ok(()),
    }
}

/// The root a catalogued path is judged against: the first managed root that contains its
/// spelling (`tools::containing_root`, the guard move and delete use), else `None`. Shared by
/// the write door, the sidecar preflight and `doc(augment)`, so a refusal found late cannot
/// disagree with the check made early.
pub(crate) fn confinement_root(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    crate::librarian::tools::containing_root(roots, path).cloned()
}

/// Refuse a `path` that does not resolve to somewhere under `root`.
///
/// The deepest EXISTING ancestor of `path` is canonicalized and the not-yet-existing components
/// below it are appended verbatim; the result must be under `root` resolved the same way (a root
/// that does not exist yet, under a symlinked ancestor such as `/tmp` -> `/private/tmp`, must
/// still match what is created below it). An unresolved `..` is refused rather than trusted:
/// `starts_with` is component-wise and would match a root the `..` escapes. Call it BEFORE
/// `create_dir_all`, which would otherwise build the escaping directories before anything is
/// refused. `verb` names the operation in the message ("create", "write", "move", "delete").
///
/// Why not `validate_write_path`: it also admits the system temp dir and the server's working
/// directory as write roots, so a link into either would pass, and the librarian holds no
/// `PathSecurityConfig`. The question here is narrower: stay inside this repo root.
///
/// DUPLICATION, noted so the unification is a follow-up: [`resolve_through_existing_ancestor`]
/// is the same walk as `canonicalize_through_existing_ancestor` in `src/util/path_security.rs`,
/// which is private there and was off limits to the change that added this. A shared, public
/// form would let both call one implementation.
///
/// Limit: the check and the later write are separate syscalls, so a link planted between them
/// still wins.
pub(crate) fn refuse_path_outside_root(
    path: &std::path::Path,
    root: &std::path::Path,
    verb: &str,
) -> anyhow::Result<()> {
    use std::path::Component;
    let resolved = resolve_through_existing_ancestor(path);
    let root_resolved = resolve_through_existing_ancestor(root);
    let unresolved_dotdot = resolved
        .components()
        .any(|c| matches!(c, Component::ParentDir));
    if unresolved_dotdot || !resolved.starts_with(&root_resolved) {
        return Err(
            crate::librarian::tools::LibrarianRecoverableError::with_hint(
                format!(
                    "refusing to {verb} `{}`: it resolves to `{}`, outside the repository root `{}`",
                    path.display(),
                    resolved.display(),
                    root_resolved.display()
                ),
                "Nothing was changed. The path passes through a symbolic link, or a `..` that \
                 could not be resolved, that leaves the repository root. Use a path that stays \
                 inside it, or replace the link with a real directory.",
            ),
        );
    }
    Ok(())
}

/// `path` canonicalized as far as it exists: the deepest ancestor `fs::canonicalize` accepts is
/// resolved and the components below it are appended verbatim, `..` included.
///
/// Limit: `canonicalize` also fails for a dangling link or an unreadable component, and the walk
/// then stops ABOVE it and keeps it as written, so such a link is not followed here.
fn resolve_through_existing_ancestor(path: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let mut below: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    loop {
        if let Ok(canon) = std::fs::canonicalize(cur) {
            return below.iter().rev().fold(canon, |acc, c| acc.join(c));
        }
        let Some(up) = cur.parent() else {
            return path.to_path_buf();
        };
        match cur.components().next_back() {
            Some(Component::Normal(n)) => below.push(n.to_os_string()),
            Some(Component::ParentDir) => below.push("..".into()),
            _ => return path.to_path_buf(),
        }
        cur = up;
    }
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
