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
/// asks where the write lands. `std::fs::write` follows a leaf link, and a catalogued row can be
/// one: the indexer no longer catalogs a link out of the repository (the read door refuses it,
/// `read_door`), but a pulled commit can turn a catalogued file into a link, and a catalog an
/// older indexer wrote still holds such rows, so a row can name a file the repository does not
/// own.
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
/// empty where the caller has none, in which case only the repositories the spelling names are
/// available to judge by).
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

/// [`refuse_path_outside_root`] for a catalog-supplied FILE path, against the root [`door_root`]
/// names; a path for which no root can be named is not checked (only the leaf rule applies).
///
/// This is the question the LEAF rule cannot ask: a row whose parent directory is a link out of
/// the repository. Such a row arises when a directory is swapped for a link after indexing, and
/// older catalogs hold rows the walk minted through a symlinked `force_include` anchor (the walk
/// and reclamation now judge both through the read door, `read_door`), and every writer that
/// takes `row.abs_path` would then write, move or delete a file the repository does not own.
/// A link whose target stays INSIDE the repository is untouched: a symlinked tracker directory
/// is a legitimate setup.
///
/// **One function for every verb.** `doc(update)` and every other writer reach it through
/// [`write_catalogued_file`]; `doc(create)` and `doc(move)` call it for the path they are about to
/// create (before the rename or the `create_dir_all`, so the id repair that follows cannot be
/// refused after the file has moved) and [`refuse_catalogued_dir_outside_repo`], the same
/// decision for a directory, serves `doc(move)`'s source and `doc(delete)`. `doc(move)` and
/// `doc(delete)` also name their root with [`managed_base`] rather than the lexical
/// `containing_root`. The four verbs therefore ask one question; the test
/// `every_verb_admits_a_row_spelled_through_a_link_to_the_managed_project` (and its F2 twin)
/// pins one verdict each.
pub(crate) fn refuse_catalogued_path_outside_repo(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
    verb: &str,
) -> anyhow::Result<()> {
    match door_root(roots, path) {
        Some(root) => refuse_path_outside_root(path, &root, verb),
        None => Ok(()),
    }
}

/// [`refuse_catalogued_path_outside_repo`] for a DIRECTORY (the parent of a leaf that may itself
/// be a link, which `doc(move)` and `doc(delete)` act on without following): the same root, the
/// same containment test, applied to `dir` itself. A probe child names the directory's own
/// ancestors, `dir` included, as the candidates; it is never created or opened.
pub(crate) fn refuse_catalogued_dir_outside_repo(
    roots: &[std::path::PathBuf],
    dir: &std::path::Path,
    verb: &str,
) -> anyhow::Result<()> {
    let probe = dir.join(".codescout-door-probe");
    match door_root(roots, &probe) {
        Some(root) => refuse_path_outside_root(dir, &root, verb),
        None => Ok(()),
    }
}

/// The root a catalogued path is judged against: the one place that decision is made.
///
/// **Invariant: the judging root is reachable from the confinement root without crossing a
/// symbolic link; when no such inner root exists, the confinement root itself is used.**
///
/// * The *confinement root* (`managed_base`) is where the spelling says the path lives inside
///   what the caller manages.
/// * The *refinement* is the nearest ancestor of the spelling that is a repository (`.git`, not
///   itself a link) and that is reached from the confinement root by plain directories only. It
///   narrows the root where a managed root holds several repositories (a legacy `[[roots]]`
///   umbrella, an unrelated active project listed first, a nested repository or submodule): a
///   link from one repository into another stays inside the umbrella and must still be refused.
/// * The "without crossing a link" clause is what keeps the refinement honest. A candidate found
///   BELOW a link (`P/up -> ..`, row `P/up/S/docs/x.md`) is whatever the link points at, here the
///   sibling repository `S`; judged against `S` the write looks inside and lands in a repository
///   the project does not own. Such a candidate is skipped and the walk goes on upward, so the
///   answer is `P`, and the resolution (`S/...`) is then outside it.
///
/// With no confinement root (a row no managed root contains, spelled through no link to one) the
/// outermost repository on the spelling is the anchor and the same rule applies from there; with
/// no repository either there is nothing to compare against and `None` is returned.
///
/// There is deliberately no "the resolution lies under some managed root, so accept" shortcut: a
/// foreign repository's row that resolves into a managed project is the very case that must be
/// refused. Known and accepted: a foreign repository that links to a managed root ITSELF
/// (`Q/linkP -> P`) is indistinguishable from a project registered under its canonical path and
/// reached through a link (`home/proj -> data/proj`), and is admitted; a foreign link to a
/// SUBDIRECTORY of a managed root names no root and is refused. The table of layouts that
/// pins every verdict is `door_layout_table` in `link_dir_tests.rs`.
pub(crate) fn door_root(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let base = managed_base(roots, path);
    // Nearest first. Candidates above the confinement root are harmless: `descends_without_link`
    // never accepts one (it is not below the anchor), so they only matter for the no-root case.
    let repos: Vec<&std::path::Path> = path.ancestors().skip(1).filter(|a| holds_git(a)).collect();
    let anchor: std::path::PathBuf = match &base {
        Some(b) => b.clone(),
        None => (*repos.last()?).to_path_buf(),
    };
    let refined = repos.iter().find(|c| descends_without_link(&anchor, c));
    Some(refined.map_or(anchor, |c| (*c).to_path_buf()))
}

/// Do the components from `anchor` down to `candidate` (candidate included) contain no symbolic
/// link, and no climb out of the anchor? `candidate` must lie under `anchor` lexically; one that
/// does not is never reachable.
///
/// The walk keeps a lexical cursor and a depth. A name is pushed and must not be a link. A `..`
/// directly after such a name is an exact pop (the name was a plain directory, so its parent is
/// the cursor's previous value), which keeps `W/Z/../P` the same place as `W/P`; a `..` at depth
/// zero would climb out of the anchor (`P/../Q` merely STARTS with `P`) and is refused. A `..`
/// after a LINK never gets that far: the link is refused when it is pushed, because the link's
/// target, not its spelled parent, is what `..` would climb out of. Refusing every `..` (the
/// first version) let a path with one, below a SHALLOWER anchor, fall back to that anchor and
/// admit a write into a sibling nested repository. (`.` and a doubled separator never appear:
/// `Path::components` drops them, so `P/./docs` and `P//docs` are the plain paths they spell.)
/// When `candidate` IS the anchor there is nothing to descend and it is accepted, even if the
/// anchor was itself reached through a link: that happens when `managed_base` names a link as the
/// confinement root (a project reached through `home/proj -> data/proj`, row C1) and when, with no
/// confinement root, the anchor is the repository a foreign spelling claims (row Y1a).
fn descends_without_link(anchor: &std::path::Path, candidate: &std::path::Path) -> bool {
    let Ok(below) = candidate.strip_prefix(anchor) else {
        return false;
    };
    let mut cur = anchor.to_path_buf();
    let mut depth = 0usize;
    for c in below.components() {
        match c {
            std::path::Component::Normal(_) => {
                cur.push(c);
                depth += 1;
                let is_link = std::fs::symlink_metadata(&cur)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(false);
                if is_link {
                    return false;
                }
            }
            std::path::Component::ParentDir if depth > 0 => {
                cur.pop();
                depth -= 1;
            }
            _ => return false,
        }
    }
    true
}

/// The confinement root of `path`: the FIRST managed root, in the caller's order, that either
/// lexically contains the spelling (`tools::containing_root`, which this function wraps per root
/// and which no verb now calls directly) or has an ancestor of the spelling that resolves to it. In the second
/// case the confinement root is the SHALLOWEST such ancestor (a project registered under its
/// canonical path while rows keep the spelling of a link to it: `home/proj/x.md` against a
/// root of `data/proj`).
///
/// Both clauses live in one loop, in root order, so that two spellings of one file give one
/// answer FOR A FIXED ROOT ORDER: with an unrelated root listed first the lexical test of a LATER
/// root must not win over a resolving ancestor of an EARLIER one. The order itself is part of
/// the input and the verdict can depend on it (rows X6, X6b and C5 against C5c: known, not
/// changed); the callers fix it (`tools::managed_roots`: the active project's git root and project
/// root first, then the workspace roots in file order), so one running server asks one question. The shallowest ancestor keeps a link back into
/// the same project (`home/proj/self -> data/proj`) from shifting the anchor inward and so
/// admitting candidates the plain spelling would not. Only an ancestor that resolves to a root
/// EXACTLY counts.
///
/// `skip(1)` leaves out the path itself: a leaf that resolves exactly to a managed root is a
/// link, which the leaf rule refuses first for writes, and for every other verb it is the root
/// directory judged against itself either way, so `skip(0)` is equivalent (a mutant of it
/// survives, on purpose, and is recorded in the issue).
pub(crate) fn managed_base(
    roots: &[std::path::PathBuf],
    path: &std::path::Path,
) -> Option<std::path::PathBuf> {
    for root in roots {
        if crate::librarian::tools::containing_root(std::slice::from_ref(root), path).is_some() {
            return Some(root.clone());
        }
        let resolved_root = resolve_through_existing_ancestor(root);
        let shallowest = path
            .ancestors()
            .skip(1)
            .filter(|a| std::fs::canonicalize(a).is_ok_and(|c| c == resolved_root))
            .last();
        if let Some(a) = shallowest {
            return Some(a.to_path_buf());
        }
    }
    None
}

/// Is `dir` a repository, by the marker alone: any `.git` entry (directory, file for a submodule
/// or linked worktree, or a link, dangling or not) seen WITHOUT following it. One rule for every
/// kind of marker: `exists()` followed links, so a `.git` link to a directory made a repository
/// and a dangling one made none (rows X3 and X3b disagreed). A candidate that is itself a link needs
/// no test here: [`descends_without_link`] rejects it unless it is the anchor.
fn holds_git(dir: &std::path::Path) -> bool {
    std::fs::symlink_metadata(dir.join(".git")).is_ok()
}

/// SQL predicate (text, with bound parameters) matching a path column that IS `root` or lies
/// under it, by exact comparison: `(col = ?root OR substr(col, 1, length(?prefix)) = ?prefix)`
/// where the caller binds `root` to the root's path and `prefix` to `root + "/"`.
///
/// Not `LIKE`. SQLite's `LIKE` ignores ASCII case, treats `_` and `%` in the pattern as
/// wildcards, and the escaping that cures the second does nothing for the first: `b/my_app`
/// matched the rows of `b/MY_APP`, and a path containing `'` needs the parameter left UNquoted.
/// Equality and `substr` are byte-exact for the characters the stored path holds and take every
/// metacharacter literally, so a sibling can never be "under" the root. `col` and the two
/// parameter placeholders are SQL text the CALLER supplies (a column name and `?N`); never
/// interpolate a user string into them.
pub(crate) fn path_at_or_under_sql(col: &str, root_param: &str, prefix_param: &str) -> String {
    format!("({col} = {root_param} OR substr({col}, 1, length({prefix_param})) = {prefix_param})")
}

/// The two values [`path_at_or_under_sql`] binds for `root` (a forward-slash path as the catalog
/// stores it): the root without a trailing separator, and that plus exactly one `/`. A caller that
/// passed `/gone/x/` used to bind the prefix `/gone/x//`, which no row matches, so the prune and its
/// dry run reported zero rows without saying why. `/` alone yields `("", "/")`: every absolute row
/// is under it, and the empty root matches none by equality.
pub(crate) fn root_and_prefix(root: &str) -> (String, String) {
    let trimmed = root.trim_end_matches('/');
    (trimmed.to_string(), format!("{trimmed}/"))
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
pub(crate) fn resolve_through_existing_ancestor(path: &std::path::Path) -> std::path::PathBuf {
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

/// Where the relative symlink target `target` lands when the link sits in directory `dir`:
/// `dir` resolved as far as it exists ([`resolve_through_existing_ancestor`]), then the target's
/// components applied lexically (`..` pops, a name pushes). The result is a comparison key, not a
/// path to open: two directories that give the SAME key reach the same file through the same
/// relative target, which is what a move that cannot retarget a link needs to know. `dir` may not
/// exist yet (the destination of a move, before its directories are created); the components below
/// the deepest existing ancestor are plain names, so popping them is exact.
pub(crate) fn link_target_landing(
    dir: &std::path::Path,
    target: &std::path::Path,
) -> std::path::PathBuf {
    use std::path::Component;
    let mut out = resolve_through_existing_ancestor(dir);
    for c in target.components() {
        match c {
            Component::Normal(n) => out.push(n),
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            // A rooted or prefixed target is absolute and never reaches here for a relative link.
            Component::RootDir | Component::Prefix(_) => return target.to_path_buf(),
        }
    }
    out
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

    /// Rows as the catalog stores a Windows path (`//?/C:/...`), a root with every LIKE hazard in
    /// it, and siblings that differ in case, in a wildcard position and by a longer name: only the
    /// root itself and its descendants match.
    #[test]
    fn path_at_or_under_sql_is_exact_for_windows_shaped_paths() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (p TEXT);").unwrap();
        let root = "//?/C:/Users/Dev/My_App's%x";
        for p in [
            root.to_string(),
            format!("{root}/docs/a.md"),
            "//?/C:/Users/Dev/MY_APP's%x/docs/a.md".to_string(),
            "//?/c:/Users/Dev/My_App's%x/docs/a.md".to_string(),
            "//?/C:/Users/Dev/MyXApp's%x/docs/a.md".to_string(),
            format!("{root}-other/docs/a.md"),
            format!("{root}x/docs/a.md"),
        ] {
            conn.execute("INSERT INTO t VALUES (?1)", [p]).unwrap();
        }
        let prefix = format!("{root}/");
        let sql = format!(
            "SELECT p FROM t WHERE {} ORDER BY p",
            path_at_or_under_sql("p", "?1", "?2")
        );

        let got: Vec<String> = conn
            .prepare(&sql)
            .unwrap()
            .query_map([root, prefix.as_str()], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(got, vec![root.to_string(), format!("{root}/docs/a.md")]);
    }

    /// `length()` counts CHARACTERS and so does `substr`; a byte count (`length(CAST(.. AS BLOB))`)
    /// would cut a multi-byte root's descendants short and match none of them. Case folding must
    /// not reach a non-ASCII sibling either (`É_APP` against `é_app`).
    #[test]
    fn path_at_or_under_sql_counts_characters_for_a_non_ascii_root() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE t (p TEXT);").unwrap();
        for p in [
            "/tmp/é_app",
            "/tmp/é_app/docs/a.md",
            "/tmp/é_appX/docs/a.md",
            "/tmp/É_APP/docs/a.md",
            "/tmp/éXapp/docs/a.md",
        ] {
            conn.execute("INSERT INTO t VALUES (?1)", [p]).unwrap();
        }
        let (root, prefix) = root_and_prefix("/tmp/é_app");
        let sql = format!(
            "SELECT p FROM t WHERE {} ORDER BY p",
            path_at_or_under_sql("p", "?1", "?2")
        );

        let got: Vec<String> = conn
            .prepare(&sql)
            .unwrap()
            .query_map([&root, &prefix], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(got, vec!["/tmp/é_app", "/tmp/é_app/docs/a.md"]);
    }

    /// `/gone/x/` bound the prefix `/gone/x//` and matched nothing; a bare `/` must still match
    /// every absolute row by prefix.
    #[test]
    fn root_and_prefix_normalizes_one_trailing_separator() {
        assert_eq!(
            root_and_prefix("/gone/x"),
            ("/gone/x".into(), "/gone/x/".into())
        );
        assert_eq!(
            root_and_prefix("/gone/x/"),
            ("/gone/x".into(), "/gone/x/".into())
        );
        assert_eq!(
            root_and_prefix("/gone/x//"),
            ("/gone/x".into(), "/gone/x/".into())
        );
        assert_eq!(root_and_prefix("/"), (String::new(), "/".into()));
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
