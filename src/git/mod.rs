//! Git integration using `git2`.

use anyhow::Result;
use std::path::Path;

/// Open the git repository at or above `path`.
pub fn open_repo(path: &Path) -> Result<git2::Repository> {
    git2::Repository::discover(path)
        .map_err(|e| anyhow::anyhow!("No git repository found at {}: {}", path.display(), e))
}

#[derive(Debug, Clone)]
pub enum DiffStatus {
    Added,
    Modified,
    Deleted,
    Renamed { old_path: String },
}

#[derive(Debug, Clone)]
pub struct DiffEntry {
    pub path: String,
    pub status: DiffStatus,
}

/// Minimum similarity (percent) for two files to count as a rename: git's own default.
// cap-class: NOT_A_CAP — a similarity cutoff that decides whether two files count as a rename; it bounds no result and shortens none
const RENAME_THRESHOLD: u16 = 50;

/// How many deleted files rename detection compares with each added one. 10,000 is well above
/// the number of files this repository tracks (about 6,500 when this was set), so no update of it
/// reaches the limit; the cost of a larger limit falls only on a diff that deletes and adds that
/// many files.
// cap-class: NOT_A_CAP — bounds the work of pairing deleted with added files; past it a rename is counted as one addition and one deletion, so every changed file is still counted once, and the counts are reported only (nothing stored or returned as a result list is shortened)
const RENAME_LIMIT: usize = 10_000;

/// Diff two commits by SHA, returning a list of changed files.
/// Returns `Err` if either SHA is not found (e.g. after a rebase).
///
/// Note: `revparse_single` accepts full revspec grammar (`HEAD~3`, `:/regex`,
/// `branch@{1}`). Callers passing attacker-influenced values must validate them
/// (e.g. `git2::Oid::from_str` or `[0-9a-f]{4,40}` allowlist) first.
pub fn diff_tree_to_tree(
    repo: &git2::Repository,
    from_sha: &str,
    to_sha: &str,
) -> Result<Vec<DiffEntry>> {
    let from_obj = repo.revparse_single(from_sha)?;
    let to_obj = repo.revparse_single(to_sha)?;
    let from_tree = from_obj.peel_to_commit()?.tree()?;
    let to_tree = to_obj.peel_to_commit()?.tree()?;
    diff_trees(repo, Some(&from_tree), &to_tree, None)
}

/// Diff two trees, returning a list of changed files. `from = None` is the empty tree, so every
/// file of `to` is `Added`. `pathspec`, when given, narrows both sides to the paths it matches (a
/// git pathspec such as `*.md`, which also matches nested paths); a rename is still seen when
/// both of its paths match.
///
/// Renames are detected with the options set below (renames on, copies off). A Typechange delta
/// is not reported: it is dropped like every status [`DiffStatus`] has no variant for.
pub fn diff_trees(
    repo: &git2::Repository,
    from: Option<&git2::Tree>,
    to: &git2::Tree,
    pathspec: Option<&str>,
) -> Result<Vec<DiffEntry>> {
    let mut opts = git2::DiffOptions::new();
    if let Some(spec) = pathspec {
        opts.pathspec(spec);
    }
    let mut diff = repo.diff_tree_to_tree(from, Some(to), Some(&mut opts))?;

    // Every option that decides what is a rename is set here, so the answer depends on the two
    // trees alone. libgit2 reads `diff.renames` from the repo or global git config when the
    // rename flag is left unset, and `diff.renameLimit` (default 1000) when `rename_limit` is 0;
    // that limit caps how many deleted files it compares with each added one, so a low value in
    // some repo's config turned renames into an add plus a delete. Renames only (no copies, no
    // break-rewrites), at git's own 50% similarity, and a fixed limit (`RENAME_LIMIT`, sized for
    // this repository: see its doc). A diff that deletes more files than the limit is compared
    // against only the first `limit + 1` deleted files per added file, so a rename can then show
    // as an add plus a delete; the work grows with added files x min(deleted files, limit + 1).
    let mut find_opts = git2::DiffFindOptions::new();
    find_opts
        .renames(true)
        .copies(false)
        .rename_threshold(RENAME_THRESHOLD)
        .rename_limit(RENAME_LIMIT);
    diff.find_similar(Some(&mut find_opts))?;

    let mut entries = Vec::new();
    for delta in diff.deltas() {
        let status = match delta.status() {
            git2::Delta::Added => DiffStatus::Added,
            git2::Delta::Modified => DiffStatus::Modified,
            git2::Delta::Deleted => DiffStatus::Deleted,
            git2::Delta::Renamed => {
                let old = match delta.old_file().path() {
                    Some(p) => p.to_string_lossy().replace('\\', "/"),
                    None => continue,
                };
                DiffStatus::Renamed { old_path: old }
            }
            // Silently drops Typechange/Copied/Untracked/Ignored/Conflicted.
            // Fine for indexing cache-invalidation; if a future caller surfaces
            // "what changed" to an LLM, extend this match.
            _ => continue,
        };
        let path = match delta.new_file().path().or_else(|| delta.old_file().path()) {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue,
        };
        entries.push(DiffEntry { path, status });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn init_repo(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test").unwrap();
        config.set_str("user.email", "test@test.com").unwrap();
        repo
    }

    fn commit_file(repo: &git2::Repository, path: &str, content: &str, msg: &str) -> git2::Oid {
        let root = repo.workdir().unwrap();
        let file_path = root.join(path);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &sig, &sig, msg, &tree, &parents)
            .unwrap()
    }

    #[test]
    fn diff_tree_detects_added_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let c2 = commit_file(&repo, "b.rs", "fn b() {}", "add b");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "b.rs");
        assert!(matches!(entries[0].status, DiffStatus::Added));
    }

    #[test]
    fn diff_tree_detects_modified_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let c2 = commit_file(&repo, "a.rs", "fn a() { 1 }", "modify a");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.rs");
        assert!(matches!(entries[0].status, DiffStatus::Modified));
    }

    #[test]
    fn diff_tree_detects_deleted_file() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        // Delete a.rs and commit
        std::fs::remove_file(dir.path().join("a.rs")).unwrap();
        let mut index = repo.index().unwrap();
        index.remove_path(Path::new("a.rs")).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_oid).unwrap();
        let sig = repo.signature().unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        let c2 = repo
            .commit(Some("HEAD"), &sig, &sig, "del a", &tree, &[&parent])
            .unwrap();
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c2.to_string()).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "a.rs");
        assert!(matches!(entries[0].status, DiffStatus::Deleted));
    }

    #[test]
    fn diff_tree_returns_empty_for_same_commit() {
        let dir = tempdir().unwrap();
        let repo = init_repo(dir.path());
        let c1 = commit_file(&repo, "a.rs", "fn a() {}", "init");
        let entries = diff_tree_to_tree(&repo, &c1.to_string(), &c1.to_string()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn diff_trees_from_nothing_lists_every_file_as_added() {
        use crate::librarian::derived::testkit::{repo_with, FILE};
        let dir = tempdir().unwrap();
        let repo = repo_with(
            dir.path(),
            &[
                ("docs/a.md", FILE, b"# A\n"),
                ("src/x.rs", FILE, b"fn x() {}\n"),
            ],
        );
        let tree = repo.head().unwrap().peel_to_tree().unwrap();
        let mut entries = diff_trees(&repo, None, &tree, None).unwrap();
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let listed: Vec<&str> = entries.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(listed, ["docs/a.md", "src/x.rs"]);
        assert!(entries
            .iter()
            .all(|e| matches!(e.status, DiffStatus::Added)));
    }

    #[test]
    fn diff_trees_keeps_to_its_pathspec_and_still_sees_a_rename() {
        use crate::librarian::derived::testkit::{commit_files, repo_with, FILE};
        let dir = tempdir().unwrap();
        let body: &[u8] = b"# Title\n\nA body long enough that the rename is certain.\n";
        let repo = repo_with(
            dir.path(),
            &[
                ("docs/a.md", FILE, body),
                ("src/x.rs", FILE, b"fn x() {}\n"),
            ],
        );
        let first = repo.head().unwrap().peel_to_commit().unwrap();
        let second = commit_files(
            &repo,
            &[("docs/b.md", body), ("src/x.rs", b"fn x() { 1 }\n")],
            &[&first],
            "rename and edit",
        );
        let from = first.tree().unwrap();
        let to = repo.find_commit(second).unwrap().tree().unwrap();

        let narrowed = diff_trees(&repo, Some(&from), &to, Some("*.md")).unwrap();
        assert_eq!(narrowed.len(), 1, "{narrowed:?}");
        assert_eq!(narrowed[0].path, "docs/b.md");
        assert!(
            matches!(&narrowed[0].status, DiffStatus::Renamed { old_path } if old_path == "docs/a.md"),
            "{narrowed:?}"
        );
        assert!(narrowed.iter().all(|e| !e.path.ends_with(".rs")));

        // Without a pathspec the same pair also reports the edited source file.
        let all = diff_trees(&repo, Some(&from), &to, None).unwrap();
        assert!(all.iter().any(|e| e.path == "src/x.rs"), "{all:?}");
    }
}
