//! Fixtures the derived index's tests share: a commit built through `git2` alone.
//!
//! Every repo is made with an explicit signature and a fixed time, never from the global git
//! config and never from where the temp directory lies, so a commit id depends on content only.

/// The mode of a regular file tree entry.
pub(crate) const FILE: u32 = 0o100644;
/// The mode of a symlink tree entry.
pub(crate) const LINK: u32 = 0o120000;

/// A fixed author time, so a commit id depends on the content alone.
pub(crate) fn fixed_sig() -> git2::Signature<'static> {
    git2::Signature::new(
        "Test",
        "test@example.invalid",
        &git2::Time::new(1_700_000_000, 0),
    )
    .unwrap()
}

/// A repo in `dir` with one commit holding `entries` (`path`, `mode`, `bytes`). Built through
/// the index and an explicit signature: no working tree, no global git config.
pub(crate) fn repo_with(dir: &std::path::Path, entries: &[(&str, u32, &[u8])]) -> git2::Repository {
    let sig = fixed_sig();
    repo_with_commit(dir, entries, &sig, &sig, "fixture")
}

/// [`repo_with`] with the author, the committer and the message of the one commit chosen by the
/// caller: two repos with one tree and different histories (and so different commit ids).
pub(crate) fn repo_with_commit(
    dir: &std::path::Path,
    entries: &[(&str, u32, &[u8])],
    author: &git2::Signature,
    committer: &git2::Signature,
    message: &str,
) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    let tree_id = tree_of(&repo, entries);
    {
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), author, committer, message, &tree, &[])
            .unwrap();
    }
    repo
}

/// The tree holding `entries` (`path`, `mode`, `bytes`), written to `repo`'s object database.
pub(crate) fn tree_of(repo: &git2::Repository, entries: &[(&str, u32, &[u8])]) -> git2::Oid {
    // The repository's own index, used only to assemble a tree: it is never written to disk.
    let mut index = repo.index().unwrap();
    index.clear().unwrap();
    for &(path, mode, bytes) in entries {
        let entry = git2::IndexEntry {
            ctime: git2::IndexTime::new(0, 0),
            mtime: git2::IndexTime::new(0, 0),
            dev: 0,
            ino: 0,
            mode,
            uid: 0,
            gid: 0,
            file_size: bytes.len() as u32,
            id: git2::Oid::zero(),
            flags: 0,
            flags_extended: 0,
            path: path.as_bytes().to_vec(),
        };
        index.add_frombuffer(&entry, bytes).unwrap();
    }
    index.write_tree().unwrap()
}

/// A commit of `repo` holding exactly `files` (regular files), on top of `parents` (none for a
/// root commit that shares no history with the others). It moves no ref: the caller keeps the
/// id and opens the commit by it.
pub(crate) fn commit_files(
    repo: &git2::Repository,
    files: &[(&str, &[u8])],
    parents: &[&git2::Commit],
    message: &str,
) -> git2::Oid {
    let sig = fixed_sig();
    commit_files_at(repo, files, parents, message, &sig, &sig)
}

/// [`commit_files`] with the author and the committer chosen by the caller: a test of a history
/// walk gives each commit its own time, and so its own place in the walk's order.
pub(crate) fn commit_files_at(
    repo: &git2::Repository,
    files: &[(&str, &[u8])],
    parents: &[&git2::Commit],
    message: &str,
    author: &git2::Signature,
    committer: &git2::Signature,
) -> git2::Oid {
    let entries: Vec<(&str, u32, &[u8])> = files
        .iter()
        .map(|&(path, bytes)| (path, FILE, bytes))
        .collect();
    let tree = repo.find_tree(tree_of(repo, &entries)).unwrap();
    repo.commit(None, author, committer, message, &tree, parents)
        .unwrap()
}

/// `git` as the tests run it: the machine's own configuration out of the way, so that what a test
/// sees does not depend on the developer's git setup (nor on a hook that set `GIT_DIR`). One
/// place sets the environment, so [`git`] and [`git_exit`] cannot drift apart.
fn git_command(dir: &std::path::Path, args: &[&str]) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE");
    cmd
}

/// Run `git` in `dir` with the machine's own configuration out of the way, so that what a test
/// sees does not depend on the developer's git setup (nor on a hook that set `GIT_DIR`).
pub(crate) fn git(dir: &std::path::Path, args: &[&str]) -> String {
    let out = git_command(dir, args).output().expect("run git");
    assert!(
        out.status.success(),
        "git {args:?} in {}: {}",
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

/// [`git`] without the success assertion: the exit code and the output (stdout, then stderr), for
/// a test of a command that is meant to fail, such as a `git merge` that stops on a conflict. A
/// process ended by a signal has no code and gives -1.
pub(crate) fn git_exit(dir: &std::path::Path, args: &[&str]) -> (i32, String) {
    let out = git_command(dir, args).output().expect("run git");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (out.status.code().unwrap_or(-1), text)
}

/// `git clone` of `src` into `dst` over a `file://` URL: a second checkout of one repository,
/// with its own path, its own working tree and its own local config.
pub(crate) fn clone_repo(src: &std::path::Path, dst: &std::path::Path) {
    let url = format!("file://{}", src.display());
    git(
        dst.parent().unwrap(),
        &["clone", "--quiet", &url, dst.to_str().unwrap()],
    );
}

/// A repo whose root tree is built by hand, so an entry name can be raw bytes.
pub(crate) fn repo_with_raw_tree(
    dir: &std::path::Path,
    build: impl FnOnce(&git2::Repository, &mut git2::TreeBuilder),
) -> git2::Repository {
    let repo = git2::Repository::init(dir).unwrap();
    let tree_id = {
        let mut root = repo.treebuilder(None).unwrap();
        build(&repo, &mut root);
        root.write().unwrap()
    };
    {
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = fixed_sig();
        repo.commit(Some("HEAD"), &sig, &sig, "fixture", &tree, &[])
            .unwrap();
    }
    repo
}

/// The tables of a hand-built "old" catalog, with only the columns the shadow diff reads. The
/// diff's tests never go through `Catalog::open`: it migrates, and a test that needed it would
/// not show that the diff reads nothing else.
pub(crate) const OLD_CATALOG_SCHEMA: &str = r#"
CREATE TABLE artifact (
    id            TEXT PRIMARY KEY,
    abs_path      TEXT NOT NULL UNIQUE,
    kind          TEXT NOT NULL,
    status        TEXT NOT NULL,
    title         TEXT,
    owners        TEXT NOT NULL DEFAULT '[]',
    tags          TEXT NOT NULL DEFAULT '[]',
    file_sha256   TEXT NOT NULL,
    slug          TEXT,
    missing_since INTEGER
);
CREATE TABLE artifact_link (
    src_id TEXT NOT NULL,
    dst_id TEXT NOT NULL,
    rel    TEXT NOT NULL,
    PRIMARY KEY (src_id, dst_id, rel)
);
CREATE TABLE entry_cite (
    src_slug  TEXT NOT NULL,
    src_local TEXT NOT NULL,
    dst_ref   TEXT NOT NULL,
    rel       TEXT NOT NULL,
    PRIMARY KEY (src_slug, src_local, dst_ref, rel)
);
CREATE TABLE artifact_chunk (
    chunk_id    TEXT PRIMARY KEY,
    artifact_id TEXT NOT NULL,
    chunk_ix    INTEGER NOT NULL
);
"#;

/// One row of the old catalog's `artifact` table.
#[derive(Clone)]
pub(crate) struct OldRow {
    pub id: String,
    pub abs_path: String,
    pub kind: String,
    pub status: String,
    pub title: Option<String>,
    /// A JSON array, as the old catalog stores it.
    pub owners: String,
    pub tags: String,
    pub sha: String,
    pub slug: Option<String>,
    pub missing: bool,
}

/// An old row with plain defaults; a test sets the fields it is about.
pub(crate) fn old_row(id: &str, abs_path: &str) -> OldRow {
    OldRow {
        id: id.to_owned(),
        abs_path: abs_path.to_owned(),
        kind: "doc".to_owned(),
        status: "draft".to_owned(),
        title: Some("T".to_owned()),
        owners: "[]".to_owned(),
        tags: "[]".to_owned(),
        sha: "00".to_owned(),
        slug: None,
        missing: false,
    }
}

/// Create the old tables in `conn` and fill them: the artifacts, the file-grain links (all
/// `rel = 'cites'`), the entry cites as `(src_slug, dst_ref)` (all `rel = 'cites'`), and the
/// chunk rows as `(artifact_id, how many)`.
pub(crate) fn fill_old_catalog(
    conn: &rusqlite::Connection,
    rows: &[OldRow],
    links: &[(&str, &str)],
    cites: &[(&str, &str)],
    chunks: &[(String, usize)],
) {
    conn.execute_batch(OLD_CATALOG_SCHEMA).unwrap();
    for r in rows {
        conn.execute(
            "INSERT INTO artifact (id, abs_path, kind, status, title, owners, tags, file_sha256, \
             slug, missing_since) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                r.id,
                r.abs_path,
                r.kind,
                r.status,
                r.title,
                r.owners,
                r.tags,
                r.sha,
                r.slug,
                r.missing.then_some(1_700_000_000_000_i64)
            ],
        )
        .unwrap();
    }
    for (src, dst) in links {
        conn.execute(
            "INSERT INTO artifact_link (src_id, dst_id, rel) VALUES (?1, ?2, 'cites')",
            [src, dst],
        )
        .unwrap();
    }
    for (src_slug, dst_ref) in cites {
        conn.execute(
            "INSERT INTO entry_cite (src_slug, src_local, dst_ref, rel) \
             VALUES (?1, 'X-1', ?2, 'cites')",
            [src_slug, dst_ref],
        )
        .unwrap();
    }
    for (artifact_id, count) in chunks {
        for ix in 0..*count {
            conn.execute(
                "INSERT INTO artifact_chunk (chunk_id, artifact_id, chunk_ix) VALUES (?1, ?2, ?3)",
                rusqlite::params![format!("{artifact_id}-{ix}"), artifact_id, ix as i64],
            )
            .unwrap();
        }
    }
}

/// The derived index of a commit, kept with the temp directory the commit lives in.
pub(crate) struct Built {
    pub conn: rusqlite::Connection,
    _dir: tempfile::TempDir,
}

/// Commit `files` (`path`, text) into a temp repo and run the full build over that commit: the
/// commit oracle. `build_full` reads the commit only (its tree and its history), so what an
/// uncommitted working tree holds is invisible to it; only `update_with_working_tree` reads the
/// working tree.
pub(crate) fn build_files(files: &[(&str, &str)]) -> Built {
    use super::build::{build_full, BuildInput};
    use super::scan::open_commit;

    let dir = tempfile::tempdir().unwrap();
    let entries: Vec<(&str, u32, &[u8])> = files
        .iter()
        .map(|&(path, text)| (path, FILE, text.as_bytes()))
        .collect();
    let repo = repo_with(dir.path(), &entries);
    let commit = open_commit(&repo, "HEAD").unwrap();
    let input = BuildInput::from_commit(&repo, &commit, dir.path()).unwrap();
    let mut conn = super::schema::open_in_memory().unwrap();
    build_full(&mut conn, &input).unwrap();
    Built { conn, _dir: dir }
}

/// The tokens of the entries `rel_path` defines in the built index, in document order.
pub(crate) fn entry_tokens(conn: &rusqlite::Connection, rel_path: &str) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT token FROM entry WHERE rel_path = ?1 ORDER BY heading_line")
        .unwrap();
    let rows = stmt
        .query_map([rel_path], |r| r.get::<_, String>(0))
        .unwrap();
    rows.collect::<Result<_, _>>().unwrap()
}

/// One edge as [`edges_from`] returns it: `kind`, `target_repo`, `target_key`, `target_token`.
pub(crate) type EdgeRow = (String, Option<String>, Option<String>, Option<String>);

/// What `rel_path` cites in the built index, by position: `kind`, `target_repo`, `target_key`,
/// `target_token`. `kind` is the resolution (`file`, `entry`, `unknown_key`, `cross_repo`, ...; see
/// `edge_kind` in `build.rs`).
pub(crate) fn edges_from(conn: &rusqlite::Connection, rel_path: &str) -> Vec<EdgeRow> {
    let mut stmt = conn
        .prepare(
            "SELECT kind, target_repo, target_key, target_token \
             FROM edge WHERE src_rel_path = ?1 ORDER BY ord",
        )
        .unwrap();
    let rows = stmt
        .query_map([rel_path], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .unwrap();
    rows.collect::<Result<_, _>>().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_commit_oracle_sees_what_a_commit_holds() {
        let b = build_files(&[
            ("docs/ledger.md", "## F-1 — x\n\n**Cites:** [[target]]\n"),
            ("docs/target.md", "# T\n"),
        ]);
        assert_eq!(entry_tokens(&b.conn, "docs/ledger.md"), vec!["F-1"]);
        assert_eq!(
            edges_from(&b.conn, "docs/ledger.md"),
            vec![("file".into(), None, Some("target".into()), None)]
        );
    }

    #[test]
    fn git_exit_reports_a_failing_command_without_panicking() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "--quiet"]);
        let (ok_code, ok_text) = git_exit(dir.path(), &["rev-parse", "--git-dir"]);
        assert_eq!(ok_code, 0);
        assert_eq!(ok_text.trim(), ".git");
        let (bad_code, bad_text) =
            git_exit(dir.path(), &["rev-parse", "--verify", "refs/heads/absent"]);
        assert_ne!(bad_code, 0, "{bad_text}");
        assert!(!bad_text.is_empty(), "git says why it failed");
    }
}
