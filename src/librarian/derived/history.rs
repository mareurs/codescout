//! The first-parent git history of every markdown document of a commit (E2-R10, E2-R11).
//!
//! [`walk`] follows the first-parent line from the indexed commit back to the root (or to the
//! boundary of a shallow clone) and records, for each document the commit holds, the commits that
//! added it, changed it or renamed it. The rows are keyed by the document's path in the indexed
//! commit, however the path was spelled further back. The tables are not part of the derived
//! answer about a tree (two commits of one tree have different histories), so a rebuild rewrites
//! them with [`write`] only when [`reusable`] says the stored ones were walked from the same
//! commit with the same parse version in a repository that is not shallow. [`settle`] makes that
//! decision again inside the write transaction.
//!
//! The rules the walk follows, where the data leaves a choice:
//!
//! - A commit's deltas are a set. No path's fate depends on the order the diff lists them in: the
//!   hits are decided against the tracked paths as they stood before the commit, and only then are
//!   the tracked paths updated.
//! - A document is tracked only while it is a regular file (mode 100644 or 100755) with a `.md`
//!   name, the population the commit scan indexes. A `.md` symlink has no rows. A path that was a
//!   symlink and became a regular file has its `added` row at the commit where it did: git lists
//!   that change as the old object deleted and the new one added.
//! - A path is a lossy `String`: a name that is not UTF-8 is spelled with U+FFFD, in the tracked
//!   paths and in the rows. Two entries of one tree that spell alike are one path, and the first
//!   delta in git's path order decides the row. The `status` of such a path is `None`: the blob
//!   lookup by the lossy spelling finds nothing.
//! - `diff_trees` returns paths with `\` replaced by `/`, so the tracked paths are keyed by that
//!   spelling, and a row keeps the raw name as its `rel_path`. The `status` of a name with a `\`
//!   is `None`, as for a lossy one. Two different raw names with one such spelling (`a\b.md` and
//!   `a/b.md`) cannot be told apart in a delta: neither is tracked, and neither has rows.
//! - A blob that is absent from the object database (a blobless clone) gives `status` `None`. A
//!   similarity rename whose blobs are absent fails the walk: git needs both blobs to score it.
//!   An exact rename needs none.
//! - Only the first parent is followed. At a shallow boundary libgit2 reports no parent, so the
//!   boundary commit is diffed against the empty tree and every path still tracked is `added`
//!   there. The stored history of a shallow repository is never reused: its depth can grow with
//!   no change of commit (`git fetch --deepen`), and a walk of it is bounded. Any other git error
//!   fails the build.
//! - The time is the committer's, in seconds, and the author is the name alone: no clock is read
//!   and no email is stored.
//! - `PARSER_VERSION` is also the version of these rules: see its doc.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use anyhow::Context;
use rusqlite::{params, Connection};

use super::build::PARSER_VERSION;
use crate::git::{diff_trees, DiffStatus};
use crate::librarian::frontmatter;

/// The pathspec of every diff: markdown files, at any depth.
const PATHSPEC: &str = "*.md";

/// The tree entry modes of a regular file: not 120000 (a symlink) and not 160000 (a gitlink).
const REGULAR_FILE_MODES: [i32; 2] = [0o100644, 0o100755];

/// One commit that changed a document.
pub(super) struct HistoryRow {
    /// The document's path in the indexed commit.
    pub rel_path: String,
    /// How many first-parent steps from the indexed commit (0) the commit is.
    pub walk_pos: i64,
    pub commit_sha: String,
    /// The committer's time, in seconds.
    pub committed_at: i64,
    /// The author's name (lossy UTF-8), without the email.
    pub author: String,
    /// `added`, `modified` or `renamed`.
    pub change_kind: &'static str,
    /// The frontmatter `status` of the document as that commit had it, when it had one.
    pub status: Option<String>,
}

/// What [`walk`] found, and what it was walked from.
pub(super) struct History {
    pub commit_sha: String,
    pub parser_version: String,
    pub shallow: bool,
    pub rows: Vec<HistoryRow>,
}

/// A tracked document that a commit changed: where it was in that commit and how it changed.
struct Hit {
    /// The path in the commit being read.
    path: String,
    kind: &'static str,
    /// The path the document had in the parent, for a rename.
    from: Option<String>,
}

/// Walk the first-parent line of `commit`, from the commit itself (position 0) to the root or the
/// shallow boundary, and return the history of every regular-file `.md` document it holds.
pub(super) fn walk(repo: &git2::Repository, commit: &git2::Commit) -> anyhow::Result<History> {
    // Path as `diff_trees` spells it in the commit being read -> path in the indexed commit.
    let mut documents: BTreeSet<String> = BTreeSet::new();
    collect_documents(repo, &commit.tree()?, "", &mut documents)?;
    let mut tracked = track(&documents);

    let mut rows: Vec<HistoryRow> = Vec::new();
    let mut current = commit.clone();
    let mut pos: i64 = 0;
    while !tracked.is_empty() {
        let tree = current.tree()?;
        // libgit2 applies a shallow graft when it parses a commit, so the boundary commit has no
        // parent here, as a root does.
        let parent = if current.parent_count() == 0 {
            None
        } else {
            Some(
                current
                    .parent(0)
                    .with_context(|| format!("reading the first parent of {}", current.id()))?,
            )
        };
        let parent_tree = match &parent {
            Some(p) => Some(p.tree()?),
            None => None,
        };
        let deltas = diff_trees(repo, parent_tree.as_ref(), &tree, Some(PATHSPEC))
            .with_context(|| format!("diffing {} against its first parent", current.id()))?;

        // The hits, against the tracked paths as they stood before this commit. Keyed by the
        // indexed path: one row per document per commit, whatever the delta order.
        let mut hits: BTreeMap<String, Hit> = BTreeMap::new();
        for delta in &deltas {
            let (kind, from) = match &delta.status {
                DiffStatus::Added => ("added", None),
                DiffStatus::Modified => ("modified", None),
                DiffStatus::Renamed { old_path } => ("renamed", Some(old_path.clone())),
                // A delete says nothing about a path that is tracked: a tracked path is a file in
                // this commit. (The old half of a file/symlink swap is a delete too.)
                DiffStatus::Deleted => continue,
            };
            let Some(head) = tracked.get(&delta.path) else {
                continue;
            };
            hits.entry(head.clone()).or_insert(Hit {
                path: delta.path.clone(),
                kind,
                from,
            });
        }

        if !hits.is_empty() {
            let commit_sha = current.id().to_string();
            let committed_at = current.committer().when().seconds();
            let author = String::from_utf8_lossy(current.author().name_bytes()).into_owned();
            for (head, hit) in &hits {
                rows.push(HistoryRow {
                    rel_path: head.clone(),
                    walk_pos: pos,
                    commit_sha: commit_sha.clone(),
                    committed_at,
                    author: author.clone(),
                    change_kind: hit.kind,
                    status: status_at(repo, &tree, &hit.path).with_context(|| {
                        format!("reading the status of {} in {commit_sha}", hit.path)
                    })?,
                });
            }
            // Removals first, then insertions. The two sets are disjoint (a rename's source is
            // absent from this commit's tree and a hit path is present in it), so the order of
            // the two loops is not observable.
            for hit in hits.values() {
                if hit.kind != "modified" {
                    tracked.remove(&hit.path);
                }
            }
            for (head, hit) in &hits {
                if let Some(from) = &hit.from {
                    tracked.insert(from.clone(), head.clone());
                }
            }
        }

        match parent {
            Some(p) => {
                current = p;
                pos += 1;
            }
            None => break,
        }
    }

    Ok(History {
        commit_sha: commit.id().to_string(),
        parser_version: PARSER_VERSION.to_owned(),
        shallow: repo.is_shallow(),
        rows,
    })
}

/// Every regular-file `.md` entry of `tree`, the path spelled lossily (U+FFFD for a byte that is
/// not UTF-8).
fn collect_documents(
    repo: &git2::Repository,
    tree: &git2::Tree,
    prefix: &str,
    out: &mut BTreeSet<String>,
) -> anyhow::Result<()> {
    for entry in tree.iter() {
        let name = String::from_utf8_lossy(entry.name_bytes());
        let path = format!("{prefix}{name}");
        match entry.kind() {
            Some(git2::ObjectType::Tree) => {
                let sub = repo
                    .find_tree(entry.id())
                    .with_context(|| format!("reading tree {path}"))?;
                collect_documents(repo, &sub, &format!("{path}/"), out)?;
            }
            Some(git2::ObjectType::Blob)
                if name.ends_with(".md") && REGULAR_FILE_MODES.contains(&entry.filemode()) =>
            {
                out.insert(path);
            }
            _ => {}
        }
    }
    Ok(())
}

/// The path as `diff_trees` returns it.
fn diff_spelling(path: &str) -> String {
    path.replace('\\', "/")
}

/// The tracked paths of the indexed commit: `diff_trees`'s spelling of each document -> the
/// document's own path. A spelling that two different paths share names neither of them.
fn track(documents: &BTreeSet<String>) -> HashMap<String, String> {
    let mut by_spelling: HashMap<String, Option<&String>> = HashMap::new();
    for path in documents {
        by_spelling
            .entry(diff_spelling(path))
            .and_modify(|holder| *holder = None)
            .or_insert(Some(path));
    }
    by_spelling
        .into_iter()
        .filter_map(|(spelling, holder)| holder.map(|path| (spelling, path.clone())))
        .collect()
}

/// The frontmatter `status` of the regular file at `path` in `tree`. `None` when there is no such
/// entry (a lossy spelling names nothing), when the entry is not a regular file, when the bytes
/// are not UTF-8, or when the frontmatter does not parse or has no status.
fn status_at(
    repo: &git2::Repository,
    tree: &git2::Tree,
    path: &str,
) -> anyhow::Result<Option<String>> {
    let entry = match tree.get_path(Path::new(path)) {
        Ok(entry) => entry,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    if entry.kind() != Some(git2::ObjectType::Blob)
        || !REGULAR_FILE_MODES.contains(&entry.filemode())
    {
        return Ok(None);
    }
    // A blob the object database lacks (a blobless clone) is no status, as an unknown path is.
    // A blob the object database lacks (a blobless clone) is no status, as an unknown path is.
    let blob = match repo.find_blob(entry.id()) {
        Ok(blob) => blob,
        Err(e) if e.code() == git2::ErrorCode::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let Ok(text) = std::str::from_utf8(blob.content()) else {
        return Ok(None);
    };
    Ok(match frontmatter::parse(text) {
        Ok((Some(fm), _)) => fm.status,
        _ => None,
    })
}

/// Whether the stored history was walked from `commit` with this build's parse version, in a
/// repository that is not shallow. A shallow repository never reuses: its depth can change with
/// no change of commit, and a shallow walk is bounded.
pub(super) fn reusable(
    conn: &Connection,
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> rusqlite::Result<bool> {
    if repo.is_shallow() {
        return Ok(false);
    }
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM history_meta \
         WHERE commit_sha = ?1 AND parser_version = ?2 AND shallow = 0)",
        params![commit.id().to_string(), PARSER_VERSION],
        |r| r.get(0),
    )
}

/// Replace both history tables with `history`. The caller holds the write transaction.
pub(super) fn write(tx: &Connection, history: &History) -> rusqlite::Result<()> {
    tx.execute("DELETE FROM doc_history", [])?;
    tx.execute("DELETE FROM history_meta", [])?;
    tx.execute(
        "INSERT INTO history_meta (commit_sha, parser_version, shallow) VALUES (?1, ?2, ?3)",
        params![
            history.commit_sha,
            history.parser_version,
            i64::from(history.shallow)
        ],
    )?;
    let mut insert = tx.prepare_cached(
        "INSERT INTO doc_history \
         (rel_path, walk_pos, commit_sha, committed_at, author, change_kind, status) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for row in &history.rows {
        insert.execute(params![
            row.rel_path,
            row.walk_pos,
            row.commit_sha,
            row.committed_at,
            row.author,
            row.change_kind,
            row.status
        ])?;
    }
    Ok(())
}

/// Finish the history inside the write transaction. With a walk in hand it is written. Without
/// one the caller skipped the walk because [`reusable`] said yes before the transaction opened;
/// another writer may have stored another commit's history since, so the check is made again
/// here, under the write lock, and a history that is no longer reusable is dropped (the next
/// update walks) rather than left beside a `checkout` it does not belong to.
pub(super) fn settle(
    tx: &Connection,
    repo: &git2::Repository,
    commit: &git2::Commit,
    walked: Option<&History>,
) -> rusqlite::Result<()> {
    match walked {
        Some(history) => write(tx, history),
        None if reusable(tx, repo, commit)? => Ok(()),
        None => {
            tx.execute("DELETE FROM doc_history", [])?;
            tx.execute("DELETE FROM history_meta", [])?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::derived::build::{build_full, BuildInput};
    use crate::librarian::derived::schema::open_in_memory;
    use crate::librarian::derived::testkit::{git, repo_with_raw_tree, tree_of, FILE, LINK};
    use std::path::Path;

    /// Executable regular file: a document like any other.
    const EXEC: u32 = 0o100755;

    type Entries<'a> = &'a [(&'a str, u32, &'a [u8])];

    fn sig(name: &str, seconds: i64) -> git2::Signature<'static> {
        let email = format!("{}@example.invalid", name.to_lowercase().replace(' ', "."));
        git2::Signature::new(name, &email, &git2::Time::new(seconds, 0)).unwrap()
    }

    /// A commit of `repo` holding exactly `entries`, by `author` and `committer`; no ref moves.
    fn commit_entries(
        repo: &git2::Repository,
        entries: Entries,
        parents: &[&git2::Commit],
        author: &git2::Signature,
        committer: &git2::Signature,
    ) -> git2::Oid {
        let tree = repo.find_tree(tree_of(repo, entries)).unwrap();
        repo.commit(None, author, committer, "fixture", &tree, parents)
            .unwrap()
    }

    /// A chain of commits, one per `(entries, seconds)`, each the child of the one before, all by
    /// `Test` at that time. The ids, root first.
    fn chain(repo: &git2::Repository, snapshots: &[(Entries, i64)]) -> Vec<git2::Oid> {
        let mut ids: Vec<git2::Oid> = Vec::new();
        for (entries, seconds) in snapshots {
            let who = sig("Test", *seconds);
            let parent = ids.last().map(|id| repo.find_commit(*id).unwrap());
            let parents: Vec<&git2::Commit> = parent.iter().collect();
            ids.push(commit_entries(repo, entries, &parents, &who, &who));
        }
        ids
    }

    fn doc(status: &str, body: &str) -> String {
        format!("---\nstatus: {status}\n---\n# Doc\n\n{body}\n")
    }

    const BODY: &str = "A body long enough that git pairs a rename with its source, \
                        line after line of it, so the similarity is certain.";

    /// The history of `tip`, stored in a fresh index.
    fn stored(repo: &git2::Repository, tip: git2::Oid) -> Connection {
        let conn = open_in_memory().unwrap();
        let history = walk(repo, &repo.find_commit(tip).unwrap()).unwrap();
        write(&conn, &history).unwrap();
        conn
    }

    /// `(walk_pos, commit_sha, change_kind, status, committed_at)`.
    type Row = (i64, String, String, Option<String>, i64);

    fn rows_of(conn: &Connection, rel_path: &str) -> Vec<Row> {
        conn.prepare(
            "SELECT walk_pos, commit_sha, change_kind, status, committed_at \
             FROM doc_history WHERE rel_path = ?1 ORDER BY walk_pos",
        )
        .unwrap()
        .query_map([rel_path], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
    }

    fn row(pos: i64, sha: git2::Oid, kind: &str, status: Option<&str>, at: i64) -> Row {
        (
            pos,
            sha.to_string(),
            kind.to_owned(),
            status.map(str::to_owned),
            at,
        )
    }

    fn total_rows(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM doc_history", [], |r| r.get(0))
            .unwrap()
    }

    // mutation: drop `tracked.insert(from, head)` after a rename: the document's history ends at
    // the rename.
    #[test]
    fn a_documents_history_follows_modify_and_rename() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let (t0, t1, t2, t3) = (1_700_000_000, 1_700_000_100, 1_700_000_200, 1_700_000_300);
        let draft = doc("draft", BODY);
        let active = doc("active", BODY);
        let ids = chain(
            &repo,
            &[
                (&[("docs/a.md", FILE, draft.as_bytes())], t0),
                (&[("docs/a.md", FILE, active.as_bytes())], t1),
                (&[("docs/archive/a.md", FILE, active.as_bytes())], t2),
                (
                    &[
                        ("docs/archive/a.md", FILE, active.as_bytes()),
                        ("docs/b.md", FILE, b"# B\n"),
                    ],
                    t3,
                ),
            ],
        );
        let conn = stored(&repo, ids[3]);

        assert_eq!(
            rows_of(&conn, "docs/archive/a.md"),
            [
                row(1, ids[2], "renamed", Some("active"), t2),
                row(2, ids[1], "modified", Some("active"), t1),
                row(3, ids[0], "added", Some("draft"), t0),
            ]
        );
        assert_eq!(
            rows_of(&conn, "docs/b.md"),
            [row(0, ids[3], "added", None, t3)]
        );
        // The rows are keyed by the indexed path alone: nothing under the old name.
        assert_eq!(rows_of(&conn, "docs/a.md"), []);
        assert_eq!(total_rows(&conn), 4);
    }

    // mutation: the author's time instead of the committer's, or `Utc::now()`; the email stored
    // beside the name.
    #[test]
    fn times_and_authors_are_the_commits_never_the_clock() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let author = sig("Alice Early", 1_262_304_000);
        let committer = sig("Carol Committer", 1_262_390_400);
        let id = commit_entries(&repo, &[("a.md", FILE, b"# A\n")], &[], &author, &committer);
        let conn = stored(&repo, id);

        let (stored_author, at): (String, i64) = conn
            .query_row("SELECT author, committed_at FROM doc_history", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(stored_author, "Alice Early");
        assert_eq!(at, 1_262_390_400);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        assert!(
            (now - at).abs() > 86_400,
            "a row within a day of now: {at} vs {now}"
        );
        let emails: i64 = conn
            .query_row(
                "SELECT count(*) FROM doc_history WHERE author LIKE '%@%' OR author LIKE '%example%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(emails, 0, "no email is stored");
    }

    // mutation: follow the last parent instead of the first, or every parent: the side commit
    // gets a row and the main line's commit does not.
    #[test]
    fn only_the_first_parent_line_is_walked() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let who = sig("Test", 1_700_000_000);
        let (v0, v1) = (doc("draft", BODY), doc("active", BODY));
        let c0 = commit_entries(
            &repo,
            &[("docs/a.md", FILE, v0.as_bytes())],
            &[],
            &who,
            &who,
        );
        let c0c = repo.find_commit(c0).unwrap();
        // The side line changes `docs/a.md`; the main line adds `docs/b.md`; the merge has both.
        let side = commit_entries(
            &repo,
            &[("docs/a.md", FILE, v1.as_bytes())],
            &[&c0c],
            &who,
            &who,
        );
        let main = commit_entries(
            &repo,
            &[
                ("docs/a.md", FILE, v0.as_bytes()),
                ("docs/b.md", FILE, b"# B\n"),
            ],
            &[&c0c],
            &who,
            &who,
        );
        let (mainc, sidec) = (
            repo.find_commit(main).unwrap(),
            repo.find_commit(side).unwrap(),
        );
        let merge = commit_entries(
            &repo,
            &[
                ("docs/a.md", FILE, v1.as_bytes()),
                ("docs/b.md", FILE, b"# B\n"),
            ],
            &[&mainc, &sidec],
            &who,
            &who,
        );
        let conn = stored(&repo, merge);
        let t = 1_700_000_000;

        assert_eq!(
            rows_of(&conn, "docs/a.md"),
            [
                row(0, merge, "modified", Some("active"), t),
                row(2, c0, "added", Some("draft"), t),
            ]
        );
        assert_eq!(
            rows_of(&conn, "docs/b.md"),
            [row(1, main, "added", None, t)]
        );
        let side_rows: i64 = conn
            .query_row(
                "SELECT count(*) FROM doc_history WHERE commit_sha = ?1",
                [side.to_string()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            side_rows, 0,
            "the side commit is not on the first-parent line"
        );
    }

    // mutation: keep tracking a path after its `added` row.
    #[test]
    fn a_path_deleted_and_added_again_has_only_its_new_history() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let keep: (&str, u32, &[u8]) = ("keep.md", FILE, b"# Keep\n");
        let ids = chain(
            &repo,
            &[
                (&[("x.md", FILE, b"# X one\n"), keep], 1_700_000_000),
                (&[keep], 1_700_000_100),
                (&[("x.md", FILE, b"# X two\n"), keep], 1_700_000_200),
            ],
        );
        let conn = stored(&repo, ids[2]);

        assert_eq!(
            rows_of(&conn, "x.md"),
            [row(0, ids[2], "added", None, 1_700_000_200)]
        );
        // The walk went on past the deletion for the other document.
        assert_eq!(
            rows_of(&conn, "keep.md"),
            [row(2, ids[0], "added", None, 1_700_000_000)]
        );
    }

    // No mutation separates the order of an addition from a rename here: the sets are disjoint
    // (an added path and a renamed-to path cannot be one path), so the order of the two steps
    // cannot be observed. The test pins the outcome: a path one rename frees and a later commit
    // fills keeps two histories.
    #[test]
    fn a_rename_onto_a_freed_path_keeps_both_histories() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let first = doc("draft", BODY);
        let second = doc(
            "active",
            "A different document that has nothing in common with it.",
        );
        let ids = chain(
            &repo,
            &[
                (&[("a.md", FILE, first.as_bytes())], 1_700_000_000),
                (&[("b.md", FILE, first.as_bytes())], 1_700_000_100),
                (
                    &[
                        ("a.md", FILE, second.as_bytes()),
                        ("b.md", FILE, first.as_bytes()),
                    ],
                    1_700_000_200,
                ),
            ],
        );
        let conn = stored(&repo, ids[2]);

        assert_eq!(
            rows_of(&conn, "a.md"),
            [row(0, ids[2], "added", Some("active"), 1_700_000_200)]
        );
        assert_eq!(
            rows_of(&conn, "b.md"),
            [
                row(1, ids[1], "renamed", Some("draft"), 1_700_000_100),
                row(2, ids[0], "added", Some("draft"), 1_700_000_000),
            ]
        );
    }

    // mutations: `shallow` = false; a hard error at the boundary (the parent is not there).
    #[test]
    fn a_shallow_clone_says_so_and_its_history_starts_at_the_boundary() {
        let parent = tempfile::tempdir().unwrap();
        let origin_path = parent.path().join("origin");
        let clone_path = parent.path().join("clone");
        let origin = git2::Repository::init(&origin_path).unwrap();
        let (a, b, c): (&[u8], &[u8], &[u8]) = (b"# A\n", b"# B\n", b"# C\n");
        let ids = chain(
            &origin,
            &[
                (&[("a.md", FILE, a)], 1_700_000_000),
                (&[("a.md", FILE, a), ("b.md", FILE, b)], 1_700_000_100),
                (
                    &[("a.md", FILE, a), ("b.md", FILE, b), ("c.md", FILE, c)],
                    1_700_000_200,
                ),
            ],
        );
        origin
            .reference("refs/heads/main", ids[2], true, "tip")
            .unwrap();
        origin.set_head("refs/heads/main").unwrap();

        // The control: the full history has each path at its own depth.
        let full = stored(&origin, ids[2]);
        assert!(!origin.is_shallow());
        assert_eq!(rows_of(&full, "a.md")[0].0, 2);
        assert_eq!(rows_of(&full, "b.md")[0].0, 1);
        assert_eq!(rows_of(&full, "c.md")[0].0, 0);

        let url = format!("file://{}", origin_path.display());
        git(
            parent.path(),
            &[
                "clone",
                "--quiet",
                "--depth",
                "1",
                &url,
                clone_path.to_str().unwrap(),
            ],
        );
        let clone = git2::Repository::open(&clone_path).unwrap();
        assert!(clone.is_shallow(), "the clone is shallow");
        let tip = clone.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.id(), ids[2]);

        // The build succeeds, and the history is the boundary's.
        let mut conn = open_in_memory().unwrap();
        build_full(
            &mut conn,
            &BuildInput::from_commit(&clone, &tip, Path::new("/fixture/root")).unwrap(),
        )
        .unwrap();
        let shallow: i64 = conn
            .query_row("SELECT shallow FROM history_meta", [], |r| r.get(0))
            .unwrap();
        assert_eq!(shallow, 1);
        for path in ["a.md", "b.md", "c.md"] {
            assert_eq!(
                rows_of(&conn, path),
                [row(0, ids[2], "added", None, 1_700_000_200)],
                "{path}"
            );
        }
        assert_eq!(total_rows(&conn), 3);
    }

    // mutation: include symlink entries in `tracked`.
    #[test]
    fn a_symlinked_md_has_no_history_and_a_symlink_that_became_a_file_starts_there() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let who = |s: i64| sig("Test", s);
        let target: &[u8] = b"real.md";
        let real: (&str, u32, &[u8]) = ("real.md", FILE, b"# Real\n");
        let mut ids: Vec<git2::Oid> = Vec::new();
        let steps: [Vec<(&str, u32, &[u8])>; 5] = [
            // `t.md` is a file, then a symlink, then a file again; `l.md` is a symlink, then a
            // file; `x.md` is a symlink at the tip; `e.md` is an executable file.
            vec![
                real,
                ("t.md", FILE, b"# T one\n"),
                ("l.md", LINK, target),
                ("x.md", FILE, b"# X\n"),
                ("e.md", EXEC, b"# E\n"),
            ],
            vec![
                real,
                ("t.md", LINK, target),
                ("l.md", FILE, b"# L\n"),
                ("x.md", FILE, b"# X\n"),
                ("e.md", EXEC, b"# E\n"),
            ],
            vec![
                real,
                ("t.md", FILE, b"# T two\n"),
                ("l.md", FILE, b"# L\n"),
                ("x.md", FILE, b"# X\n"),
                ("e.md", EXEC, b"# E\n"),
            ],
            vec![
                real,
                ("t.md", FILE, b"# T two\n"),
                ("l.md", FILE, b"# L\n"),
                ("x.md", FILE, b"# X\n"),
                ("e.md", EXEC, b"# E\n"),
                ("other.md", FILE, b"# Other\n"),
            ],
            vec![
                real,
                ("t.md", FILE, b"# T two\n"),
                ("l.md", FILE, b"# L\n"),
                ("x.md", LINK, target),
                ("e.md", EXEC, b"# E\n"),
                ("other.md", FILE, b"# Other\n"),
            ],
        ];
        for (i, entries) in steps.iter().enumerate() {
            let w = who(1_700_000_000 + i as i64 * 100);
            let parent = ids.last().map(|id| repo.find_commit(*id).unwrap());
            let parents: Vec<&git2::Commit> = parent.iter().collect();
            ids.push(commit_entries(&repo, entries, &parents, &w, &w));
        }
        let conn = stored(&repo, ids[4]);
        let at = |i: usize| 1_700_000_000 + i as i64 * 100;

        // A symlink at the indexed commit has no rows, whatever it was before.
        assert_eq!(rows_of(&conn, "x.md"), []);
        // A path that was a link and became a file: one `added` row, at the commit where it did.
        assert_eq!(
            rows_of(&conn, "l.md"),
            [row(3, ids[1], "added", None, at(1))]
        );
        // A path that was a file, a link, then a file again: its row is the latest change to a
        // file, and the file it was before is another life.
        assert_eq!(
            rows_of(&conn, "t.md"),
            [row(2, ids[2], "added", None, at(2))]
        );
        // An executable file is a document like any other.
        assert_eq!(
            rows_of(&conn, "e.md"),
            [row(4, ids[0], "added", None, at(0))]
        );
        assert_eq!(
            rows_of(&conn, "other.md"),
            [row(1, ids[3], "added", None, at(3))]
        );
        assert_eq!(
            rows_of(&conn, "real.md"),
            [row(4, ids[0], "added", None, at(0))]
        );
        assert_eq!(
            total_rows(&conn),
            5,
            "five documents at the tip, one row each"
        );
    }

    // The walk takes a rename's old path on trust: this pins that git does not pair a symlink with
    // a regular file, so the old path of a `renamed` hit was a regular file in the parent (else a
    // symlink's life would be tracked as a document's).
    #[test]
    fn git_does_not_pair_a_symlink_with_a_regular_file_as_a_rename() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let same: &[u8] = b"docs/some/long/target/path/that/is/shared.md";
        let before = repo
            .find_tree(tree_of(
                &repo,
                &[("old.md", LINK, same), ("keep.md", FILE, b"# K\n")],
            ))
            .unwrap();
        let after = repo
            .find_tree(tree_of(
                &repo,
                &[("new.md", FILE, same), ("keep.md", FILE, b"# K\n")],
            ))
            .unwrap();
        let deltas = diff_trees(&repo, Some(&before), &after, Some(PATHSPEC)).unwrap();
        let listed: Vec<String> = deltas
            .iter()
            .map(|d| format!("{} {:?}", d.path, d.status))
            .collect();
        assert!(
            deltas
                .iter()
                .all(|d| !matches!(d.status, DiffStatus::Renamed { .. })),
            "paired: {listed:?}"
        );
        // Not vacuous: the new file is there, as an addition.
        assert!(
            deltas
                .iter()
                .any(|d| d.path == "new.md" && matches!(d.status, DiffStatus::Added)),
            "{listed:?}"
        );
    }

    // mutations: dedupe the rows of one document by nothing (a primary-key failure), or unwrap the
    // blob lookup by the lossy spelling.
    #[test]
    fn a_non_utf8_name_is_keyed_by_its_lossy_spelling_with_no_status_and_the_build_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let draft = b"---\nstatus: draft\n---\n# D\n";
        let repo = repo_with_raw_tree(dir.path(), |repo, root| {
            let blob = repo.blob(draft).unwrap();
            // Two entries whose lossy name is one `a\u{fffd}.md`, and a plain one beside them.
            root.insert(&b"a\xff.md"[..], blob, FILE as i32).unwrap();
            root.insert(&b"a\xfe.md"[..], blob, FILE as i32).unwrap();
            root.insert("ok.md", blob, FILE as i32).unwrap();
        });
        let tip = repo.head().unwrap().peel_to_commit().unwrap();
        let conn = stored(&repo, tip.id());
        let at = 1_700_000_000;

        // One row for the one lossy path, with no status: nothing in the tree has that spelling.
        assert_eq!(
            rows_of(&conn, "a\u{fffd}.md"),
            [row(0, tip.id(), "added", None, at)]
        );
        assert_eq!(
            rows_of(&conn, "ok.md"),
            [row(0, tip.id(), "added", Some("draft"), at)]
        );
        assert_eq!(total_rows(&conn), 2);

        // The whole build goes through on such a tree.
        let mut built = open_in_memory().unwrap();
        build_full(
            &mut built,
            &BuildInput::from_commit(&repo, &tip, Path::new("/fixture/root")).unwrap(),
        )
        .unwrap();
        assert_eq!(total_rows(&built), 2);
    }

    /// Point `refs/heads/main` of `origin` at `tip` and make it HEAD, so a clone checks it out.
    fn publish(origin: &git2::Repository, tip: git2::Oid) {
        origin
            .reference("refs/heads/main", tip, true, "tip")
            .unwrap();
        origin.set_head("refs/heads/main").unwrap();
    }

    /// A blobless clone (`--filter=blob:none`): the checkout fetches the tip's blobs, the older
    /// ones stay on the remote, and libgit2 does not fetch them lazily.
    fn blobless_clone(parent: &Path, origin_path: &Path, clone_path: &Path) {
        git(origin_path, &["config", "uploadpack.allowFilter", "true"]);
        let url = format!("file://{}", origin_path.display());
        git(
            parent,
            &[
                "clone",
                "--quiet",
                "--filter=blob:none",
                &url,
                clone_path.to_str().unwrap(),
            ],
        );
    }

    /// The oid of `path` in `commit`'s tree.
    fn blob_id(repo: &git2::Repository, commit: git2::Oid, path: &str) -> git2::Oid {
        let tree = repo.find_commit(commit).unwrap().tree().unwrap();
        tree.get_path(Path::new(path)).unwrap().id()
    }

    // mutation: a missing blob fails the walk again (the `find_blob` NotFound arm is an error).
    #[test]
    fn a_blobless_clone_builds_and_a_missing_blob_gives_no_status() {
        let parent = tempfile::tempdir().unwrap();
        let origin_path = parent.path().join("origin");
        let clone_path = parent.path().join("clone");
        let origin = git2::Repository::init(&origin_path).unwrap();
        let (draft, active) = (doc("draft", BODY), doc("active", BODY));
        let ids = chain(
            &origin,
            &[
                (&[("a.md", FILE, draft.as_bytes())], 1_700_000_000),
                (&[("a.md", FILE, active.as_bytes())], 1_700_000_100),
            ],
        );
        publish(&origin, ids[1]);
        let old_blob = blob_id(&origin, ids[0], "a.md");
        blobless_clone(parent.path(), &origin_path, &clone_path);

        let clone = git2::Repository::open(&clone_path).unwrap();
        assert!(
            clone.find_blob(old_blob).is_err(),
            "the clone lacks the old blob: the test would be vacuous otherwise"
        );
        let tip = clone.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(tip.id(), ids[1]);

        let mut conn = open_in_memory().unwrap();
        build_full(
            &mut conn,
            &BuildInput::from_commit(&clone, &tip, Path::new("/fixture/root")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            rows_of(&conn, "a.md"),
            [
                row(0, ids[1], "modified", Some("active"), 1_700_000_100),
                row(1, ids[0], "added", None, 1_700_000_000),
            ]
        );
    }

    // Measured, and kept as a known limit (module doc): git needs both blobs to score a
    // similarity rename, so the walk of a blobless clone fails where the rename's source blob is
    // absent.
    #[test]
    fn a_similarity_rename_whose_blobs_are_absent_fails_the_walk() {
        let parent = tempfile::tempdir().unwrap();
        let origin_path = parent.path().join("origin");
        let clone_path = parent.path().join("clone");
        let origin = git2::Repository::init(&origin_path).unwrap();
        let before = doc("draft", BODY);
        let after = doc(
            "draft",
            &format!("{BODY}\nOne more line, so it is not an exact copy."),
        );
        let ids = chain(
            &origin,
            &[
                (&[("a.md", FILE, before.as_bytes())], 1_700_000_000),
                (&[("b.md", FILE, after.as_bytes())], 1_700_000_100),
            ],
        );
        publish(&origin, ids[1]);
        let old_blob = blob_id(&origin, ids[0], "a.md");
        blobless_clone(parent.path(), &origin_path, &clone_path);

        let clone = git2::Repository::open(&clone_path).unwrap();
        assert!(
            clone.find_blob(old_blob).is_err(),
            "the source blob is absent"
        );
        let tip = clone.head().unwrap().peel_to_commit().unwrap();
        let outcome = walk(&clone, &tip);
        let message = format!("{:#}", outcome.err().expect("the walk fails"));
        assert!(
            message.contains("diffing") && message.contains("code=NotFound"),
            "the failure is the diff's: {message}"
        );
    }

    // mutation: key `tracked` by the raw name (no `\` -> `/`), or let the first raw name win a
    // collision.
    #[test]
    fn a_backslash_name_is_tracked_by_the_spelling_diff_trees_returns() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let (v0, v1) = (doc("draft", BODY), doc("active", BODY));
        let (t0, t1) = (1_700_000_000, 1_700_000_100);
        // Alone: the document `a\b.md` has its history under its raw name, with no status (the
        // blob lookup by the normalised spelling finds nothing).
        let ids = chain(
            &repo,
            &[
                (&[("a\\b.md", FILE, v0.as_bytes())], t0),
                (&[("a\\b.md", FILE, v1.as_bytes())], t1),
            ],
        );
        let conn = stored(&repo, ids[1]);
        assert_eq!(
            rows_of(&conn, "a\\b.md"),
            [
                row(0, ids[1], "modified", None, t1),
                row(1, ids[0], "added", None, t0),
            ]
        );
        assert_eq!(rows_of(&conn, "a/b.md"), []);

        // Beside a real `a/b.md`: one edit to `a\b.md` alone is not a change of `a/b.md`. The two
        // names have one normalised spelling, so the walk cannot tell whose delta it is and
        // gives neither a row.
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let other: &[u8] = b"# The real a/b\n";
        let ids = chain(
            &repo,
            &[
                (
                    &[("a\\b.md", FILE, v0.as_bytes()), ("a/b.md", FILE, other)],
                    t0,
                ),
                (
                    &[("a\\b.md", FILE, v1.as_bytes()), ("a/b.md", FILE, other)],
                    t1,
                ),
            ],
        );
        let conn = stored(&repo, ids[1]);
        assert_eq!(rows_of(&conn, "a/b.md"), [], "no false modified row");
        assert_eq!(rows_of(&conn, "a\\b.md"), []);
        assert_eq!(total_rows(&conn), 0);
    }

    // mutation: skip the re-check (`settle` keeps whatever is stored when no walk was made).
    #[test]
    fn a_history_that_stopped_being_reusable_before_the_write_is_cleared() {
        let dir = tempfile::tempdir().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        let ids = chain(
            &repo,
            &[
                (&[("a.md", FILE, b"# A\n")], 1_700_000_000),
                (
                    &[("a.md", FILE, b"# A\n"), ("b.md", FILE, b"# B\n")],
                    1_700_000_100,
                ),
            ],
        );
        let at = |i: usize| repo.find_commit(ids[i]).unwrap();
        let conn = stored(&repo, ids[0]);
        let (rows, metas): (i64, i64) = (
            total_rows(&conn),
            conn.query_row("SELECT count(*) FROM history_meta", [], |r| r.get(0))
                .unwrap(),
        );
        assert_eq!((rows, metas), (1, 1));

        // Reusable for the commit it was walked from: kept.
        settle(&conn, &repo, &at(0), None).unwrap();
        assert_eq!(total_rows(&conn), 1);
        // Another writer stored another commit's history between the check and the write: the
        // caller skipped its walk on a stale check, so the stored history is dropped, and the
        // next update walks.
        settle(&conn, &repo, &at(1), None).unwrap();
        assert_eq!(total_rows(&conn), 0);
        let metas: i64 = conn
            .query_row("SELECT count(*) FROM history_meta", [], |r| r.get(0))
            .unwrap();
        assert_eq!(metas, 0);
        // With a walk in hand, it is written whatever was there.
        let walked = walk(&repo, &at(1)).unwrap();
        settle(&conn, &repo, &at(1), Some(&walked)).unwrap();
        assert_eq!(total_rows(&conn), 2);
    }
}
