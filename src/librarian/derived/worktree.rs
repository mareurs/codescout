//! HEAD plus the working tree's changed markdown (E2-R12, E2-R13).
//!
//! [`update_with_working_tree`] is the one place the derived index looks at a working tree, and it
//! looks at one thing: the markdown files `git status` reports as different from `HEAD`. Every
//! other input is still the commit's: the repo name, the classification rules, the key space and
//! the augmentation sidecars (a `.toml` or `.yaml` edit in the working tree changes nothing).
//!
//! The steps:
//!
//! 1. [`changed_markdown`] asks libgit2 for the status (untracked files included, ignored ones
//!    not, renames not detected: a rename is a delete and an add) and names the state of each path.
//! 2. [`read_layer`] reads each changed file through the read door and turns it into the same
//!    [`ScannedFile`] the commit scan makes, or into a removal.
//! 3. The commit's scan, with the removals taken out and the files read put in, goes to
//!    [`rebuild_from`] with the layer's rows, which it stores in `worktree_layer`.
//!
//! What a row of `worktree_layer` says: `state` is the state git gave the path (`added`,
//! `modified`, `deleted`, `conflicted`), and `sha256` is the hash of the bytes read, NULL when
//! nothing was read (a deleted file, a conflicted one, a link, a file outside the key space).
//! History is a function of the commit (E2-R13): `doc_history` rows are kept for a document the
//! layer deletes, and a document the layer adds has none.
//!
//! A path that is not UTF-8 cannot be a `rel_path`; the status step leaves such a path out, so a
//! working-tree change to it is not laid over the commit.
//!
//! Nothing here puts an absolute path into an error or a stored value: a failure names the
//! repository-relative path.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Context};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

use super::build::{rebuild_from, BuildInput, BuildReport, Parser};
use super::scan::{scan_commit, ScannedFile, Skip};
use crate::librarian::identity::key_space::KeySpace;
use crate::librarian::read_door::{self, ReadDoor, ReadError};

/// What `git status` says about a markdown path that differs from the commit.
///
/// [`as_str`](LayerState::as_str) is what `worktree_layer.state` stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LayerState {
    /// Not in the commit's tree, and on disk.
    Added,
    /// In the commit's tree, and on disk.
    Modified,
    /// Not on disk.
    Deleted,
    /// An unmerged path: the file holds conflict markers, so its text is not read.
    Conflicted,
}

impl LayerState {
    /// The value stored in `worktree_layer.state`.
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            LayerState::Added => "added",
            LayerState::Modified => "modified",
            LayerState::Deleted => "deleted",
            LayerState::Conflicted => "conflicted",
        }
    }
}

/// One changed markdown path and its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayerPath {
    /// Git-root-relative, `/`-separated.
    pub rel_path: String,
    pub state: LayerState,
}

/// One row of `worktree_layer`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayerRow {
    pub rel_path: String,
    pub state: &'static str,
    /// Hash of the bytes read; `None` when nothing was read.
    pub sha256: Option<String>,
}

/// What reading the changed files gave: the files to put over the commit's scan, the paths to take
/// out of it, and the rows to store.
#[derive(Debug, Default)]
pub(crate) struct Layer {
    pub upserts: Vec<ScannedFile>,
    pub removed: BTreeSet<String>,
    pub rows: Vec<LayerRow>,
}

impl Layer {
    /// The file is not on disk: no document, whatever the commit holds.
    fn gone(&mut self, rel_path: &str) {
        self.removed.insert(rel_path.to_owned());
        self.rows.push(LayerRow {
            rel_path: rel_path.to_owned(),
            state: LayerState::Deleted.as_str(),
            sha256: None,
        });
    }

    /// The file is on disk and was not read (a link, a path outside the key space): it replaces
    /// the commit's file of that path with a skip, and its row carries no hash.
    fn skip(&mut self, rel_path: &str, state: LayerState, reason: Skip) {
        self.upserts.push(ScannedFile {
            rel_path: rel_path.to_owned(),
            sha256: String::new(),
            text: Err(reason),
        });
        self.rows.push(LayerRow {
            rel_path: rel_path.to_owned(),
            state: state.as_str(),
            sha256: None,
        });
    }
}

/// The working directory of `repo`, or the refusal of a bare repository, which has none. The one
/// place that says it, for the status step and for the update.
fn workdir_of(repo: &git2::Repository) -> anyhow::Result<&Path> {
    repo.workdir()
        .context("a bare repository has no working tree")
}

/// The markdown files whose working-tree state differs from `commit` (which must be `HEAD`'s
/// commit for the states to mean anything), sorted by path.
///
/// The status is libgit2's, with a `*.md` pathspec, untracked files (directories walked) and no
/// ignored ones, submodules excluded, and no rename detection: a renamed file is its old path
/// deleted and its new path added. A status entry that names a conflict is `Conflicted`; one whose
/// path is not on disk is `Deleted`; one whose path is not in the commit's tree is `Added`; any
/// other is `Modified`. libgit2 reads the machine's git configuration (`core.excludesFile`,
/// `core.autocrlf`, `core.ignorecase`), so what a status calls changed follows it.
pub(crate) fn changed_markdown(
    repo: &git2::Repository,
    commit: &git2::Commit,
) -> anyhow::Result<Vec<LayerPath>> {
    let workdir = workdir_of(repo)?;
    let mut options = git2::StatusOptions::new();
    options
        .pathspec("*.md")
        .include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_ignored(false)
        .exclude_submodules(true);
    let statuses = repo
        .statuses(Some(&mut options))
        .context("reading the working-tree status")?;
    let tree = commit.tree().context("reading HEAD's tree")?;
    let mut paths = Vec::new();
    for entry in statuses.iter() {
        // A path that is not UTF-8 cannot be a `rel_path`.
        let Some(rel_path) = entry.path() else {
            continue;
        };
        // The pathspec is a glob, and a case-insensitive file system matches `.MD` with it too.
        if !rel_path.ends_with(".md") {
            continue;
        }
        let state = if entry.status().contains(git2::Status::CONFLICTED) {
            LayerState::Conflicted
        } else {
            match std::fs::symlink_metadata(workdir.join(rel_path)) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => LayerState::Deleted,
                Err(e) => bail!("checking {rel_path} in the working tree: {e}"),
                Ok(_) => match tree.get_path(Path::new(rel_path)) {
                    Ok(_) => LayerState::Modified,
                    Err(e) if e.code() == git2::ErrorCode::NotFound => LayerState::Added,
                    Err(e) => {
                        return Err(e).with_context(|| format!("looking up {rel_path} in HEAD"))
                    }
                },
            }
        };
        paths.push(LayerPath {
            rel_path: rel_path.to_owned(),
            state,
        });
    }
    paths.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(paths)
}

/// Read the files `paths` names, through `door`, into a [`Layer`].
///
/// - `Deleted`: removed.
/// - `Conflicted`: a row and nothing else; the commit's text stays.
/// - A path the key space excludes: a skip (`outside_key_space`), unread.
/// - A link: a skip (`symlink`), unread, whatever it points at.
/// - Otherwise the bytes come through the read door: their SHA-256, and their text, or a
///   `not_utf8` skip when they are not UTF-8. A file that is gone by now (`NotFound`) is a
///   deletion; any other failure, a refusal by the door included, fails the whole read.
///
/// An error names the path relative to the repository and never the location the door judged.
pub(crate) fn read_layer(
    door: &ReadDoor,
    workdir: &Path,
    space: &KeySpace,
    paths: &[LayerPath],
) -> anyhow::Result<Layer> {
    let mut layer = Layer::default();
    for LayerPath { rel_path, state } in paths {
        match state {
            LayerState::Deleted => layer.gone(rel_path),
            LayerState::Conflicted => layer.rows.push(LayerRow {
                rel_path: rel_path.clone(),
                state: state.as_str(),
                sha256: None,
            }),
            LayerState::Added | LayerState::Modified => {
                read_present(door, workdir, space, rel_path, *state, &mut layer)?
            }
        }
    }
    Ok(layer)
}

/// One `Added` or `Modified` path: skip it, drop it (it vanished), or read it.
fn read_present(
    door: &ReadDoor,
    workdir: &Path,
    space: &KeySpace,
    rel_path: &str,
    state: LayerState,
    layer: &mut Layer,
) -> anyhow::Result<()> {
    // Checked before anything is touched, as the commit scan does: an excluded path is never read.
    if space.is_outside(rel_path) {
        layer.skip(rel_path, state, Skip::OutsideKeySpace);
        return Ok(());
    }
    let path = workdir.join(rel_path);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            layer.skip(rel_path, state, Skip::Symlink);
            return Ok(());
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            layer.gone(rel_path);
            return Ok(());
        }
        Err(e) => bail!("checking {rel_path} in the working tree: {e}"),
    }
    let bytes = match door.bytes(&path) {
        Ok(bytes) => bytes,
        Err(ReadError::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            layer.gone(rel_path);
            return Ok(());
        }
        Err(ReadError::Io(e)) => bail!("reading {rel_path} in the working tree: {e}"),
        // The door's own message names the location it judged: not repeated here.
        Err(ReadError::Refused(_)) => bail!(
            "the read door refused {rel_path} in the working tree (it is not a regular file \
             inside the repository, or it is on the read deny-list)"
        ),
    };
    let sha256 = format!("{:x}", Sha256::digest(&bytes));
    layer.upserts.push(ScannedFile {
        rel_path: rel_path.to_owned(),
        sha256: sha256.clone(),
        text: String::from_utf8(bytes).map_err(|_| Skip::NotUtf8),
    });
    layer.rows.push(LayerRow {
        rel_path: rel_path.to_owned(),
        state: state.as_str(),
        sha256: Some(sha256),
    });
    Ok(())
}

/// The commit's files with `layer` laid over them: its removals taken out, its files put in
/// (replacing the commit's file of the same path), sorted by path.
fn laid_over(mut files: Vec<ScannedFile>, layer: &Layer) -> Vec<ScannedFile> {
    let replaced: BTreeSet<&str> = layer.upserts.iter().map(|f| f.rel_path.as_str()).collect();
    files.retain(|f| {
        !layer.removed.contains(&f.rel_path) && !replaced.contains(f.rel_path.as_str())
    });
    files.extend(layer.upserts.iter().cloned());
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    files
}

/// Rebuild the derived index from `input.commit` with the working tree's changed markdown laid
/// over it, and record what was laid over in `worktree_layer`.
///
/// `input.commit` must be `HEAD`'s commit: the status says how the working tree differs from
/// `HEAD`, so it says nothing about another commit. The repo name, the rules, the key space and
/// the augmentation sidecars stay the commit's. The text is read through
/// [`read_door::for_project`] on the working directory; a refusal or an I/O error fails the update
/// and leaves the index as it was. The parse cache and the git-history cache are used as in
/// [`update`](super::incremental::update).
///
/// Nothing in production calls this yet (E2-R16: the wiring is a later epic's).
pub fn update_with_working_tree(
    conn: &mut Connection,
    input: &BuildInput,
) -> anyhow::Result<BuildReport> {
    let workdir = workdir_of(input.repo)?;
    let head = input
        .repo
        .head()
        .context("reading HEAD")?
        .peel_to_commit()
        .context("resolving HEAD to a commit")?;
    if head.id() != input.commit.id() {
        bail!(
            "the working-tree layer is laid over HEAD only: {} is not HEAD ({})",
            input.commit.id(),
            head.id()
        );
    }
    let paths = changed_markdown(input.repo, input.commit)?;
    let door = read_door::for_project(workdir);
    let layer = read_layer(&door, workdir, &input.space, &paths)?;
    rebuild_from(
        conn,
        input,
        &Parser::PRODUCTION,
        true,
        None,
        &|| {
            Ok(laid_over(
                scan_commit(input.repo, input.commit, &input.space)?,
                &layer,
            ))
        },
        &layer.rows,
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sha2::{Digest, Sha256};

    use super::*;
    use crate::librarian::derived::build::{build_full, rebuild_from, Parser};
    use crate::librarian::derived::scan::{open_commit, scan_commit};
    use crate::librarian::derived::schema::{dump_all, open_in_memory};
    use crate::librarian::derived::testkit::{
        clone_repo, edges_from, entry_tokens, git, git_exit, repo_with, FILE,
    };
    use crate::librarian::read_door;

    /// What a clone's `docs/a.md` holds in the commit: frontmatter (so title, owners and tags are
    /// all read from it), an entry, and two citations.
    const A_COMMITTED: &str = "---\ntitle: A title\nowners: [amy]\ntags: [x, y]\n---\n# A\n\n\
                               ## F-1 — def\n\nsee [[b]] and [[home/b]]\n";

    /// The committed corpus: a repo name, a key space that excludes `contrib`, and a rule that
    /// makes the trackers `tracker`.
    fn corpus() -> Vec<(&'static str, &'static str)> {
        vec![
            (".codescout/repo.toml", "name = \"home\"\n"),
            (".codescout/key-space.toml", "exclude = [\"contrib\"]\n"),
            (
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"docs/trackers/**/*.md\"\nkind = \"tracker\"\nstatus = \"active\"\n",
            ),
            ("docs/a.md", A_COMMITTED),
            ("docs/b.md", "# B\n"),
            ("docs/trackers/bug-log.md", "# Bug log\n\n## B-1 — a bug\n"),
            ("contrib/c.md", "# C\n"),
        ]
    }

    /// A clone of a one-commit origin, kept with the temp directory both live in.
    struct Clone {
        parent: tempfile::TempDir,
        path: PathBuf,
    }

    /// libgit2 reads the machine's git configuration, and `GIT_CONFIG_GLOBAL` (which the CLI helpers
    /// in `testkit` set) does not reach it. The two settings that would change what a status
    /// reports are therefore pinned in the clone's own config, which wins over the global one: no
    /// line-ending conversion, and no global ignore file. The system config and `core.ignorecase`
    /// are not pinned; every path these tests use is one no ignore rule of a normal machine matches.
    fn clone_of(files: &[(&str, &str)]) -> Clone {
        let parent = tempfile::tempdir().unwrap();
        let origin = parent.path().join("origin-5d2e");
        let entries: Vec<(&str, u32, &[u8])> = files
            .iter()
            .map(|&(path, text)| (path, FILE, text.as_bytes()))
            .collect();
        repo_with(&origin, &entries);
        let path = parent.path().join("clone-alpha-7f3a");
        clone_repo(&origin, &path);
        pin_config(&path);
        Clone { parent, path }
    }

    fn pin_config(path: &Path) {
        git(path, &["config", "core.autocrlf", "false"]);
        git(path, &["config", "core.excludesFile", "/dev/null"]);
    }

    fn write(clone: &Path, rel: &str, body: &[u8]) {
        let path = clone.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    }

    fn sha_hex(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn commit_all(clone: &Path, message: &str) {
        git(
            clone,
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.invalid",
                "commit",
                "-q",
                "-am",
                message,
            ],
        );
    }

    /// The layered update at `HEAD` of the repository at `clone`.
    fn update(conn: &mut Connection, clone: &Path) -> anyhow::Result<BuildReport> {
        let repo = git2::Repository::open(clone).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, clone).unwrap();
        update_with_working_tree(conn, &input)
    }

    /// The commit oracle: `build_full` at `HEAD`, reading no working-tree file.
    fn full(conn: &mut Connection, clone: &Path) {
        let repo = git2::Repository::open(clone).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, clone).unwrap();
        build_full(conn, &input).unwrap();
    }

    type LayerRowOwned = (String, String, Option<String>);

    fn layer_rows(conn: &Connection) -> Vec<LayerRowOwned> {
        conn.prepare("SELECT rel_path, state, sha256 FROM worktree_layer ORDER BY rel_path")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn row(rel: &str, state: &str, sha: Option<&str>) -> LayerRowOwned {
        (rel.to_owned(), state.to_owned(), sha.map(str::to_owned))
    }

    fn documents(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT rel_path FROM document ORDER BY rel_path")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn document_column(conn: &Connection, rel: &str, column: &str) -> String {
        conn.query_row(
            &format!("SELECT {column} FROM document WHERE rel_path = ?1"),
            [rel],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn skipped(conn: &Connection) -> Vec<(String, String)> {
        conn.prepare("SELECT rel_path, reason FROM skipped ORDER BY rel_path, reason")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    // mutation: skip `LayerState::Deleted` (drop the removal): `docs/b.md` stays a document and
    // `[[b]]` resolves.
    #[test]
    fn the_layer_shows_markdown_edits_and_never_the_config() {
        const A_EDITED: &str = "# A, edited\n\nnothing of the commit's text is left [[b]]\n";
        const UNTRACKED: &str = "# Not in any commit\n\n## U-1 — untracked\n";
        const STAGED_EDIT: &str = "# Staged, not committed\n\n## S-1 — staged\n";
        const STAGED_NEW: &str = "# New and staged\n";
        let c = clone_of(&corpus());
        let edits = [
            ("docs/a.md", A_EDITED),
            ("docs/untracked-note.md", UNTRACKED),
            ("docs/trackers/bug-log.md", STAGED_EDIT),
            ("docs/staged-new.md", STAGED_NEW),
            // The three config files: none of them is read from the working tree.
            (".codescout/repo.toml", "name = \"working-tree-name\"\n"),
            (".codescout/key-space.toml", "exclude = [\"docs\"]\n"),
            (
                ".codescout/librarian.toml",
                "[[rule]]\nglob = \"**/*.md\"\nkind = \"worktree\"\n",
            ),
        ];
        for (rel, body) in edits {
            write(&c.path, rel, body.as_bytes());
        }
        std::fs::remove_file(c.path.join("docs/b.md")).unwrap();
        git(
            &c.path,
            &["add", "docs/trackers/bug-log.md", "docs/staged-new.md"],
        );

        // The status step alone: sorted, markdown only, the state of each path.
        let repo = git2::Repository::open(&c.path).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let paths: Vec<(String, &str)> = changed_markdown(&repo, &commit)
            .unwrap()
            .into_iter()
            .map(|p| (p.rel_path, p.state.as_str()))
            .collect();
        let expected_paths = [
            ("docs/a.md", "modified"),
            ("docs/b.md", "deleted"),
            ("docs/staged-new.md", "added"),
            ("docs/trackers/bug-log.md", "modified"),
            ("docs/untracked-note.md", "added"),
        ];
        assert_eq!(
            paths,
            expected_paths
                .iter()
                .map(|&(p, s)| (p.to_owned(), s))
                .collect::<Vec<_>>()
        );

        let mut conn = open_in_memory().unwrap();
        update(&mut conn, &c.path).unwrap();

        // The files of the working tree are what is stored.
        assert_eq!(document_column(&conn, "docs/a.md", "title"), "A, edited");
        assert_eq!(document_column(&conn, "docs/a.md", "owners"), "[]");
        let docs = documents(&conn);
        assert!(
            docs.contains(&"docs/untracked-note.md".to_owned()),
            "{docs:?}"
        );
        assert!(docs.contains(&"docs/staged-new.md".to_owned()), "{docs:?}");
        assert!(!docs.contains(&"docs/b.md".to_owned()), "{docs:?}");
        assert_eq!(entry_tokens(&conn, "docs/untracked-note.md"), ["U-1"]);
        assert_eq!(entry_tokens(&conn, "docs/trackers/bug-log.md"), ["S-1"]);
        // `docs/b.md` is gone, so the citation of its key names nothing.
        assert_eq!(
            edges_from(&conn, "docs/a.md")
                .iter()
                .map(|e| e.0.as_str())
                .collect::<Vec<_>>(),
            ["unknown_key"]
        );

        // The config is the commit's: its name, its rule, its key space.
        let (name, rules_source, commit_sha): (String, String, String) = conn
            .query_row(
                "SELECT repo_name, rules_source, commit_sha FROM checkout",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "home");
        assert_eq!(rules_source, "committed+builtin");
        assert_eq!(commit_sha, commit.id().to_string());
        assert_eq!(
            document_column(&conn, "docs/trackers/bug-log.md", "kind"),
            "tracker"
        );
        assert_ne!(document_column(&conn, "docs/a.md", "kind"), "worktree");
        assert_eq!(
            skipped(&conn),
            [("contrib/c.md".to_owned(), "outside_key_space".to_owned())]
        );

        // Exactly those paths, with the state git gave each, and the hash of what was read.
        assert_eq!(
            layer_rows(&conn),
            vec![
                row("docs/a.md", "modified", Some(&sha_hex(A_EDITED.as_bytes()))),
                row("docs/b.md", "deleted", None),
                row(
                    "docs/staged-new.md",
                    "added",
                    Some(&sha_hex(STAGED_NEW.as_bytes()))
                ),
                row(
                    "docs/trackers/bug-log.md",
                    "modified",
                    Some(&sha_hex(STAGED_EDIT.as_bytes()))
                ),
                row(
                    "docs/untracked-note.md",
                    "added",
                    Some(&sha_hex(UNTRACKED.as_bytes()))
                ),
            ]
        );

        // History is a function of the commit (E2-R13): the document the layer deleted keeps its
        // `doc_history` rows with no `document` row, and a document the layer added has none.
        let history_rows = |rel: &str| -> i64 {
            conn.query_row(
                "SELECT count(*) FROM doc_history WHERE rel_path = ?1",
                [rel],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert!(
            history_rows("docs/b.md") > 0,
            "the deleted document's history"
        );
        assert_eq!(history_rows("docs/untracked-note.md"), 0);
        assert_eq!(history_rows("docs/staged-new.md"), 0);

        // A build of the commit alone drops the layer and brings the committed text back.
        full(&mut conn, &c.path);
        assert!(layer_rows(&conn).is_empty());
        assert_eq!(document_column(&conn, "docs/a.md", "title"), "A title");
        assert!(documents(&conn).contains(&"docs/b.md".to_owned()));
    }

    // mutation: a layer row for every scanned file (not only the changed ones).
    #[test]
    fn a_clean_checkout_dumps_equal_to_build_full() {
        let c = clone_of(&corpus());
        let mut oracle = open_in_memory().unwrap();
        full(&mut oracle, &c.path);
        let mut layered = open_in_memory().unwrap();
        update(&mut layered, &c.path).unwrap();
        assert!(layer_rows(&layered).is_empty());
        let dump = dump_all(&layered);
        assert!(
            dump["document"].len() >= 3,
            "not vacuous: {:?}",
            dump["document"]
        );
        assert!(!dump["doc_history"].is_empty(), "not vacuous: no history");
        assert_eq!(dump, dump_all(&oracle));

        // The same on an index that already holds this commit's history (the cache is reused).
        update(&mut oracle, &c.path).unwrap();
        assert_eq!(dump_all(&oracle), dump);
    }

    // mutation: switch rename detection on in the status options (`renames_head_to_index`): the
    // staged rename is one entry that names the new path only, so the old path stays a document.
    #[test]
    fn a_rename_in_the_working_tree_is_a_delete_and_an_add() {
        let c = clone_of(&corpus());
        // One rename git records in the index, one it does not (a plain `mv`).
        git(&c.path, &["mv", "docs/a.md", "docs/renamed.md"]);
        std::fs::rename(c.path.join("docs/b.md"), c.path.join("docs/b-moved.md")).unwrap();
        let mut conn = open_in_memory().unwrap();
        update(&mut conn, &c.path).unwrap();
        let docs = documents(&conn);
        for gone in ["docs/a.md", "docs/b.md"] {
            assert!(!docs.contains(&gone.to_owned()), "{gone} in {docs:?}");
        }
        for new in ["docs/renamed.md", "docs/b-moved.md"] {
            assert!(docs.contains(&new.to_owned()), "{new} not in {docs:?}");
        }
        assert_eq!(
            layer_rows(&conn),
            vec![
                row("docs/a.md", "deleted", None),
                row("docs/b-moved.md", "added", Some(&sha_hex(b"# B\n"))),
                row("docs/b.md", "deleted", None),
                row(
                    "docs/renamed.md",
                    "added",
                    Some(&sha_hex(A_COMMITTED.as_bytes()))
                ),
            ]
        );
    }

    // mutation: read a conflicted file like any other (the conflict markers become the text).
    #[test]
    fn a_conflicted_file_keeps_its_committed_text() {
        let c = clone_of(&corpus());
        let base = git(&c.path, &["rev-parse", "--abbrev-ref", "HEAD"])
            .trim()
            .to_owned();
        let theirs = A_COMMITTED.replace("see [[b]]", "see THEIRS");
        let ours = A_COMMITTED.replace("see [[b]]", "see OURS");
        git(&c.path, &["checkout", "-q", "-b", "other"]);
        write(&c.path, "docs/a.md", theirs.as_bytes());
        commit_all(&c.path, "theirs");
        git(&c.path, &["checkout", "-q", &base]);
        write(&c.path, "docs/a.md", ours.as_bytes());
        commit_all(&c.path, "ours");
        let (code, output) = git_exit(
            &c.path,
            &[
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@example.invalid",
                "merge",
                "other",
            ],
        );
        assert_eq!(code, 1, "the merge must stop on the conflict: {output}");
        assert!(output.contains("CONFLICT"), "{output}");
        let on_disk = std::fs::read_to_string(c.path.join("docs/a.md")).unwrap();
        assert!(on_disk.contains("<<<<<<<"), "markers on disk: {on_disk}");

        let mut conn = open_in_memory().unwrap();
        update(&mut conn, &c.path).unwrap();
        assert_eq!(
            layer_rows(&conn),
            vec![row("docs/a.md", "conflicted", None)]
        );
        assert_eq!(
            document_column(&conn, "docs/a.md", "sha256"),
            sha_hex(ours.as_bytes()),
            "the commit blob's hash, not the file with markers"
        );
        let mut oracle = open_in_memory().unwrap();
        full(&mut oracle, &c.path);
        let mut layered_dump = dump_all(&conn);
        let mut oracle_dump = dump_all(&oracle);
        assert_eq!(layered_dump.remove("worktree_layer").unwrap().len(), 1);
        oracle_dump.remove("worktree_layer");
        assert_eq!(
            layered_dump, oracle_dump,
            "everything but the layer row is the commit's"
        );
    }

    // mutations: drop the link check (the door then refuses the link and the update fails); drop
    // the outside-key-space upsert (the `skipped` row for `contrib/new.md` is missing).
    #[test]
    fn links_binary_and_outside_space_files_are_skipped_and_not_read() {
        const MARKER: &str = "SECRET-7d1";
        let c = clone_of(&corpus());
        write(&c.path, "docs/bin.md", &[0xff, 0xfe]);
        write(&c.path, "contrib/new.md", b"# New, outside the key space\n");
        // A link to a file outside the clone: the read door would refuse to read it.
        let secret = c.parent.path().join("outside-secret.txt");
        std::fs::write(&secret, MARKER).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(&secret, c.path.join("docs/link.md")).unwrap();

        let mut conn = open_in_memory().unwrap();
        update(&mut conn, &c.path).unwrap();

        #[allow(unused_mut)]
        let mut expected_skips = vec![
            ("contrib/c.md", "outside_key_space"),
            ("contrib/new.md", "outside_key_space"),
            ("docs/bin.md", "not_utf8"),
        ];
        #[allow(unused_mut)]
        let mut expected_rows = vec![
            row("contrib/new.md", "added", None),
            row("docs/bin.md", "added", Some(&sha_hex(&[0xff, 0xfe]))),
        ];
        // A link has a row with its state from status and no hash: it was not read (F12).
        #[cfg(unix)]
        {
            expected_skips.push(("docs/link.md", "symlink"));
            expected_rows.push(row("docs/link.md", "added", None));
        }
        assert_eq!(
            skipped(&conn),
            expected_skips
                .iter()
                .map(|&(p, r)| (p.to_owned(), r.to_owned()))
                .collect::<Vec<_>>()
        );
        assert_eq!(layer_rows(&conn), expected_rows);
        let docs = documents(&conn);
        for skipped_path in ["contrib/new.md", "docs/bin.md", "docs/link.md"] {
            assert!(!docs.contains(&skipped_path.to_owned()), "{docs:?}");
        }
        // The marker is in no stored value, the parse cache (which `dump_all` leaves out) included.
        let dump = dump_all(&conn);
        assert!(
            !dump
                .values()
                .flatten()
                .flatten()
                .any(|v| v.contains(MARKER)),
            "the marker reached a stored value"
        );
        let cached: i64 = conn
            .query_row(
                "SELECT count(*) FROM parse_cache WHERE metadata_json || entries_json || \
                 citations_json || chunks_json || facts_json LIKE '%' || ?1 || '%'",
                [MARKER],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cached, 0);
    }

    // mutation: strip `\r` before hashing, the two hashes then match.
    #[test]
    fn crlf_text_parses_like_its_lf_twin_and_keeps_its_own_hash() {
        // Where `sha256` sits in a `document` row of `dump_all`: after `confidence`.
        const SHA_COLUMN: usize = 11;
        let c = clone_of(&corpus());
        let crlf = A_COMMITTED.replace('\n', "\r\n");
        write(&c.path, "docs/a.md", crlf.as_bytes());
        let mut layered = open_in_memory().unwrap();
        update(&mut layered, &c.path).unwrap();
        let mut lf = open_in_memory().unwrap();
        full(&mut lf, &c.path);
        assert_eq!(
            layer_rows(&layered),
            vec![row(
                "docs/a.md",
                "modified",
                Some(&sha_hex(crlf.as_bytes()))
            )]
        );

        let (layered, lf) = (dump_all(&layered), dump_all(&lf));
        let a_row = |dump: &std::collections::BTreeMap<String, Vec<Vec<String>>>| {
            dump["document"]
                .iter()
                .find(|r| r[0] == "T:docs/a.md")
                .unwrap()
                .clone()
        };
        let (crlf_row, lf_row) = (a_row(&layered), a_row(&lf));
        assert_eq!(
            crlf_row[SHA_COLUMN],
            format!("T:{}", sha_hex(crlf.as_bytes()))
        );
        assert_eq!(
            lf_row[SHA_COLUMN],
            format!("T:{}", sha_hex(A_COMMITTED.as_bytes()))
        );
        assert_ne!(crlf_row[SHA_COLUMN], lf_row[SHA_COLUMN], "its own hash");
        let without_hash = |row: &[String]| -> Vec<String> {
            row.iter()
                .enumerate()
                .filter(|(i, _)| *i != SHA_COLUMN)
                .map(|(_, v)| v.clone())
                .collect()
        };
        // kind, status, title, owners, tags and the rest of the row.
        assert_eq!(without_hash(&crlf_row), without_hash(&lf_row));
        assert!(lf_row.contains(&"T:A title".to_owned()), "{lf_row:?}");
        assert!(lf_row.contains(&r#"T:["amy"]"#.to_owned()), "{lf_row:?}");
        for table in ["entry", "edge"] {
            assert!(!lf[table].is_empty(), "not vacuous: no {table} row");
            assert_eq!(layered[table], lf[table], "{table} rows");
        }
    }

    // mutation: propagate the `NotFound` of the status-time check instead of reading it as a
    // deletion.
    #[test]
    fn a_file_gone_between_status_and_read_is_deleted() {
        let c = clone_of(&corpus());
        write(&c.path, "docs/b.md", b"# B, edited\n");
        let repo = git2::Repository::open(&c.path).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, &c.path).unwrap();
        let paths = changed_markdown(&repo, &commit).unwrap();
        assert_eq!(
            paths,
            vec![LayerPath {
                rel_path: "docs/b.md".to_owned(),
                state: LayerState::Modified
            }]
        );
        std::fs::remove_file(c.path.join("docs/b.md")).unwrap();
        let door = read_door::for_project(&c.path);
        let layer = read_layer(&door, &c.path, &input.space, &paths).unwrap();
        assert_eq!(
            layer.removed.iter().map(String::as_str).collect::<Vec<_>>(),
            ["docs/b.md"]
        );
        assert!(layer.upserts.is_empty());
        assert_eq!(
            layer.rows,
            vec![LayerRow {
                rel_path: "docs/b.md".to_owned(),
                state: "deleted",
                sha256: None
            }]
        );
    }

    // mutation: an ignored file listed (`include_ignored(true)`, which libgit2's own default is).
    #[test]
    fn an_ignored_markdown_file_is_not_in_the_layer() {
        let c = clone_of(&corpus());
        write(&c.path, ".gitignore", b"scratch-ignored.md\n");
        write(&c.path, "docs/scratch-ignored.md", b"# Ignored\n");
        write(&c.path, "docs/kept-note.md", b"# Kept\n");
        let mut conn = open_in_memory().unwrap();
        update(&mut conn, &c.path).unwrap();
        assert_eq!(
            layer_rows(&conn),
            vec![row(
                "docs/kept-note.md",
                "added",
                Some(&sha_hex(b"# Kept\n"))
            )]
        );
        assert!(!documents(&conn).contains(&"docs/scratch-ignored.md".to_owned()));
    }

    // mutation: drop the workdir check, the update then fails with another text.
    #[test]
    fn a_bare_repository_is_refused_by_name() {
        let c = clone_of(&corpus());
        let bare = c.parent.path().join("bare.git");
        git(
            c.parent.path(),
            &[
                "clone",
                "--quiet",
                "--bare",
                c.path.to_str().unwrap(),
                bare.to_str().unwrap(),
            ],
        );
        let repo = git2::Repository::open(&bare).unwrap();
        assert!(repo.is_bare());
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, &bare).unwrap();
        let mut conn = open_in_memory().unwrap();
        let err = update_with_working_tree(&mut conn, &input).unwrap_err();
        assert!(
            format!("{err:#}").contains("a bare repository has no working tree"),
            "{err:#}"
        );
    }

    // mutation: drop the HEAD check, the layer is then laid over an older commit.
    #[test]
    fn a_layer_over_a_commit_other_than_head_is_refused() {
        let c = clone_of(&corpus());
        write(&c.path, "docs/b.md", b"# B, committed again\n");
        commit_all(&c.path, "second");
        let repo = git2::Repository::open(&c.path).unwrap();
        let older = open_commit(&repo, "HEAD~1").unwrap();
        let input = BuildInput::from_commit(&repo, &older, &c.path).unwrap();
        let mut conn = open_in_memory().unwrap();
        let err = update_with_working_tree(&mut conn, &input).unwrap_err();
        let text = format!("{err:#}");
        assert!(text.contains("laid over HEAD only"), "{text}");
        assert!(
            !text.contains(c.path.to_str().unwrap()),
            "no path in the error: {text}"
        );
        // Nothing was written.
        assert!(documents(&conn).is_empty());
    }

    /// The median of five runs of `f`, in milliseconds.
    fn median_ms(mut f: impl FnMut()) -> f64 {
        let mut runs: Vec<f64> = (0..5)
            .map(|_| {
                let started = std::time::Instant::now();
                f();
                started.elapsed().as_secs_f64() * 1000.0
            })
            .collect();
        runs.sort_by(f64::total_cmp);
        runs[2]
    }

    /// A measurement of the layer on a clone of THIS repository: the status, read and rebuild
    /// phases, each a median of five runs, on the clean tree and on a tree with 500 untracked and
    /// 20 modified markdown files. Run it with
    /// `cargo test --lib working_tree_layer_cost_on_a_clone_of_this_repo -- --ignored --nocapture`.
    /// A debug build's numbers are an upper bound on a release build's.
    #[test]
    #[ignore = "a measurement on a clone of this repository: run with --ignored --nocapture"]
    fn working_tree_layer_cost_on_a_clone_of_this_repo() {
        const UNTRACKED: usize = 500;
        const MODIFIED: usize = 20;
        let parent = tempfile::tempdir().unwrap();
        let clone = parent.path().join("bench-clone");
        git(
            parent.path(),
            &[
                "clone",
                "--local",
                // A temp dir on another device cannot hard-link the objects.
                "--no-hardlinks",
                "--quiet",
                env!("CARGO_MANIFEST_DIR"),
                clone.to_str().unwrap(),
            ],
        );
        pin_config(&clone);
        // The index of a fresh clone is racily clean for a second: wait it out, and let the CLI
        // refresh the index once, so that the clean-tree baseline is the steady state.
        std::thread::sleep(std::time::Duration::from_secs(1));
        git(&clone, &["status", "--porcelain"]);

        let repo = git2::Repository::open(&clone).unwrap();
        let commit = open_commit(&repo, "HEAD").unwrap();
        let input = BuildInput::from_commit(&repo, &commit, &clone).unwrap();
        let mut conn = open_in_memory().unwrap();
        let report = build_full(&mut conn, &input).unwrap();
        let door = read_door::for_project(&clone);

        // One round of the three phases; returns the layer row count of the last rebuild.
        let mut phases = |label: &str| -> usize {
            let mut paths = Vec::new();
            let status = median_ms(|| paths = changed_markdown(&repo, &commit).unwrap());
            let mut layer = Layer::default();
            let read = median_ms(|| {
                layer = read_layer(&door, &clone, &input.space, &paths).unwrap();
            });
            let rebuild = median_ms(|| {
                rebuild_from(
                    &mut conn,
                    &input,
                    &Parser::PRODUCTION,
                    true,
                    None,
                    &|| {
                        Ok(laid_over(
                            scan_commit(&repo, &commit, &input.space)?,
                            &layer,
                        ))
                    },
                    &layer.rows,
                )
                .unwrap();
            });
            let rows = conn
                .query_row("SELECT count(*) FROM worktree_layer", [], |r| {
                    r.get::<_, i64>(0)
                })
                .unwrap() as usize;
            println!(
                "{label}: status {status:.1} ms, read {read:.1} ms, rebuild {rebuild:.1} ms, \
                 {rows} layer rows"
            );
            rows
        };
        println!("documents at HEAD: {}", report.documents);
        let clean_rows = phases("clean tree");
        assert_eq!(clean_rows, 0, "a fresh clone has no layer");

        // Twenty tracked markdown files modified, five hundred new ones.
        let tracked: Vec<String> = scan_commit(&repo, &commit, &input.space)
            .unwrap()
            .into_iter()
            .filter(|f| f.text.is_ok())
            .map(|f| f.rel_path)
            .collect();
        assert!(tracked.len() >= MODIFIED, "{} tracked files", tracked.len());
        for rel in &tracked[..MODIFIED] {
            let path = clone.join(rel);
            let mut body = std::fs::read(&path).unwrap();
            body.extend_from_slice(b"\n<!-- layer measurement -->\n");
            std::fs::write(path, body).unwrap();
        }
        for i in 0..UNTRACKED {
            write(
                &clone,
                &format!("layer-bench/note-{i:04}.md"),
                format!("# Note {i}\n\n## N-{i} - an entry\n\nbody of note {i}\n").as_bytes(),
            );
        }
        let dirty_rows = phases("dirty tree (500 untracked, 20 modified)");
        assert_eq!(dirty_rows, UNTRACKED + MODIFIED, "one row per changed file");
    }
}
