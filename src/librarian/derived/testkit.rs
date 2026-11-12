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
fn tree_of(repo: &git2::Repository, entries: &[(&str, u32, &[u8])]) -> git2::Oid {
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
    let entries: Vec<(&str, u32, &[u8])> = files
        .iter()
        .map(|&(path, bytes)| (path, FILE, bytes))
        .collect();
    let tree = repo.find_tree(tree_of(repo, &entries)).unwrap();
    let sig = fixed_sig();
    repo.commit(None, &sig, &sig, message, &tree, parents)
        .unwrap()
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
