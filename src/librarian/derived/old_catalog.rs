//! The old catalog: how it is opened and how its rows are scoped to one repository.
//!
//! The old catalog is the machine-global `catalog.db` of the librarian before the repo-relative
//! key. Nothing here writes to it. [`open_old_immutable`] and [`open_old_read_only`] are the two
//! opens, [`RootMap`] judges where an old `abs_path` lies ([`scope_of`]), and
//! [`resolve_old_catalog`] names the file. `derived::diff` re-exports the public items, so its
//! callers do not change.
//!
//! # The read-only open
//!
//! `SQLITE_OPEN_READ_ONLY` creates nothing for a rollback-journal database. For a WAL database
//! it depends on the files beside it. Measured with the bundled SQLite, and pinned by
//! `a_read_only_open_of_a_clean_wal_catalog_makes_sqlite_create_its_side_files` and
//! `a_read_only_open_of_a_clean_wal_catalog_in_a_read_only_directory_is_refused`: a read-only
//! open of a cleanly closed WAL database (no `-wal`, no `-shm`) in a writable directory
//! succeeds and makes SQLite itself create both side files, and in a read-only directory it is
//! refused with `attempt to write a readonly database`. The old catalog is a WAL database, so
//! [`open_old_read_only`] alone can leave two new files beside it. `cli::derive_shadow` opens
//! such a catalog with [`open_old_immutable`] instead (below). A WAL database that another
//! process holds open already
//! has both files; reading it writes nothing to the catalog or its `-wal`, but SQLite updates
//! read marks in the `-shm` file (measured: a `mode=ro` reader ran BEGIN, a count and ROLLBACK;
//! the main file and the `-wal` were byte-identical afterwards and the `-shm` was not).
//!
//! # The immutable open
//!
//! [`open_old_immutable`] opens `file:<url>?mode=ro&immutable=1`. SQLite then takes no lock and
//! creates no `-wal` or `-shm`, so a cleanly closed WAL catalog opens and nothing appears beside
//! it, and the main file's bytes and mtime stay as they are. The price is that SQLite ignores a
//! `-wal` beside the file and reads the main file alone: rows that lie only in the `-wal` are
//! missing from the answer, and no error says so (pinned by
//! `an_immutable_open_would_read_stale_rows_so_a_wal_with_rows_is_refused`). So the open refuses
//! when `<path>-wal`, `<path>-shm` or a non-empty `<path>-journal` exists
//! ([`side_files_present`]): without a `-wal` the main file holds every row. SQLite keeps the side
//! files beside a symlink's target, so the open resolves the path first (`canonicalize`, once) and
//! checks and opens the resolved path. A process that starts after the check, and writes, can
//! still make the answer out of date; a caller that needs a complete answer compares
//! [`file_stamp`] before and after it reads.

use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use anyhow::{anyhow, bail, Context};
use rusqlite::{Connection, OpenFlags};

/// The directory under the git root that holds shadow copies of the repository.
const WORKTREES_DIR: &str = ".worktrees";

/// How long a read waits on a lock another process holds.
const BUSY_TIMEOUT: Duration = Duration::from_millis(5000);

/// The four queries the diff runs against the old catalog. [`OldCatalog::from_connection`]
/// prepares every one of them, so a missing table or column is named before any comparison.
pub(super) const OLD_ARTIFACTS_SQL: &str =
    "SELECT id, abs_path, kind, status, title, owners, tags, \
     file_sha256, slug, missing_since IS NOT NULL FROM artifact ORDER BY id";
pub(super) const OLD_LINKS_SQL: &str =
    "SELECT src_id, dst_id FROM artifact_link WHERE rel = 'cites'";
pub(super) const OLD_CITES_SQL: &str =
    "SELECT src_slug, dst_ref FROM entry_cite WHERE rel IN ('cites', 'rests-on')";
pub(super) const OLD_CHUNKS_SQL: &str =
    "SELECT artifact_id, count(*) FROM artifact_chunk GROUP BY artifact_id";

/// The old catalog, opened read-only.
pub struct OldCatalog {
    pub(super) conn: Connection,
}

impl OldCatalog {
    /// Wrap a connection the caller opened read-only (or, in a test, an in-memory one) after
    /// checking that every table and column the diff reads is there. A file that is not a
    /// database, a missing table and a missing column are errors that name SQLite's reason.
    ///
    /// Nothing here checks how the connection was opened, and a writable one is accepted: a
    /// caller must pass a connection opened read-only ([`open_old_immutable`],
    /// [`open_old_read_only`]).
    pub fn from_connection(conn: Connection) -> anyhow::Result<OldCatalog> {
        conn.busy_timeout(BUSY_TIMEOUT)
            .context("setting the old catalog's busy timeout")?;
        for sql in [
            OLD_ARTIFACTS_SQL,
            OLD_LINKS_SQL,
            OLD_CITES_SQL,
            OLD_CHUNKS_SQL,
        ] {
            conn.prepare(sql)
                .map_err(|e| anyhow!("the old catalog cannot be read: {e}"))?;
        }
        Ok(OldCatalog { conn })
    }
    /// The connection the diff reads through. A caller reads rows with it and never writes.
    pub(crate) fn connection(&self) -> &Connection {
        &self.conn
    }
}

/// The spellings of one repository's root that old `abs_path` values can carry: the git root the
/// derived index is built for, then each old root (`--old-root`), in order. The old indexer
/// stores `abs_path` as its caller held it, so a root reached through a symlink, or a checkout
/// that has since moved, has another spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootMap {
    pub git_root: PathBuf,
    pub old_roots: Vec<PathBuf>,
}

impl RootMap {
    /// A map with the git root only.
    pub fn new(git_root: PathBuf) -> Self {
        RootMap {
            git_root,
            old_roots: Vec::new(),
        }
    }

    /// A map with the git root and the other spellings.
    ///
    /// An old root that lies above the git root is refused, by path components: every old row of
    /// another repository under it would then count as a file of this one. An old root equal to
    /// the git root, inside it, or beside it is accepted (the git root is tried first). A root that
    /// exists is also judged by where it points, so a link to a directory above the git root is
    /// refused as well. The text of the refusal names no path. Every caller that builds a map
    /// passes through here, so `derive-shadow` and `cutover` share the rule.
    pub fn with_old_roots(git_root: PathBuf, old_roots: Vec<PathBuf>) -> anyhow::Result<Self> {
        let above = |root: &Path| git_root.starts_with(root) && git_root != root;
        for root in &old_roots {
            if !root.is_absolute() {
                bail!("an --old-root must be an absolute path");
            }
            let resolved = std::fs::canonicalize(root).ok();
            if above(root) || resolved.as_deref().is_some_and(above) {
                bail!(
                    "an --old-root lies above the git root, so it would take in the rows of \
                     other repositories; give the other spelling of the git root itself"
                );
            }
        }
        Ok(RootMap {
            git_root,
            old_roots,
        })
    }

    /// `scope_of` against the git root, then each old root in order; the first answer that is
    /// not `OtherRoot` wins.
    pub(crate) fn scope(&self, abs_path: &str) -> Scope {
        for root in self.spellings() {
            match scope_of(root, abs_path) {
                Scope::OtherRoot => {}
                found => return found,
            }
        }
        Scope::OtherRoot
    }

    /// The git root first, then the old roots.
    pub fn spellings(&self) -> Vec<&Path> {
        std::iter::once(self.git_root.as_path())
            .chain(self.old_roots.iter().map(PathBuf::as_path))
            .collect()
    }
}

/// The suffixes of the files SQLite keeps beside a database while a process has it open or after
/// one stopped without closing it.
pub(crate) const SIDE_FILE_SUFFIXES: [&str; 3] = ["-wal", "-shm", "-journal"];

/// What an operator reads when [`open_old_immutable`] refuses. It names no path: the caller
/// knows which file it asked for.
const SIDE_FILE_REFUSAL: &str = "a -wal or -shm file, or a -journal file that is not empty, lies \
     beside the old catalog. Another process holds it open, or one stopped without closing it. \
     Stop every codescout process, run sqlite3 <old catalog file> 'PRAGMA \
     wal_checkpoint(TRUNCATE)', and run this again.";

/// `<path>-wal`, `<path>-shm` or `<path>-journal` exists (`symlink_metadata`). A side file that
/// cannot be inspected counts as present.
///
/// SQLite keeps the side files beside a symlink's target, so the check resolves `path` first
/// (`canonicalize`; a path that does not resolve is used as given). A caller that holds an
/// operator-typed link path therefore looks where the files would be.
pub fn side_files_present(path: &Path) -> bool {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    SIDE_FILE_SUFFIXES
        .iter()
        .any(|suffix| side_file_present(&resolved, suffix))
}

/// Does `<path><suffix>` count as a side file? A link, or a file that cannot be inspected, does.
/// A `-journal` of zero length does not: journal mode `TRUNCATE` cuts the journal to zero length
/// after each transaction, and an empty one holds nothing to roll back. Journal mode `PERSIST`
/// keeps the journal's length and only zeroes its header, so a `PERSIST` leftover has bytes and
/// stays refused, although it holds nothing to roll back. A `-journal` with bytes may be a hot
/// journal, and `PRAGMA wal_checkpoint(TRUNCATE)` does not clear that. This function does not
/// resolve links: the caller passes the resolved path (see [`side_files_present`]).
pub(crate) fn side_file_present(path: &Path, suffix: &str) -> bool {
    let mut side = path.as_os_str().to_owned();
    side.push(suffix);
    match std::fs::symlink_metadata(PathBuf::from(side)) {
        Ok(meta) => !(suffix == "-journal" && meta.is_file() && meta.len() == 0),
        Err(e) => e.kind() != std::io::ErrorKind::NotFound,
    }
}

/// The length and modification time of the file at `path`, for the re-check after a load.
pub fn file_stamp(path: &Path) -> std::io::Result<(u64, std::time::SystemTime)> {
    let meta = std::fs::metadata(path)?;
    Ok((meta.len(), meta.modified()?))
}

/// Refuse when [`side_files_present`]; else open `file:<url>?mode=ro&immutable=1`.
///
/// `immutable=1` tells SQLite that nothing changes the file while it is open, so SQLite takes no
/// lock, reads no `-wal`, and creates no `-wal` or `-shm`: a cleanly closed WAL catalog opens and
/// nothing appears beside it. Reading it ignores a `-wal` and so can return rows that are out of
/// date without an error. The refusal of every side file is what makes the answer complete: a
/// catalog with no `-wal` holds every row in the main file. Only a catalog that no process is
/// writing may be opened this way.
///
/// The error text names no path.
pub fn open_old_immutable(path: &Path) -> anyhow::Result<OldCatalog> {
    if !path.is_absolute() {
        bail!("the path of the old catalog must be absolute");
    }
    // SQLite resolves a symlink and keeps `-wal` and `-shm` beside the target, so the side-file
    // check and the URL both use the resolved path, from one `canonicalize`. A path that cannot
    // be resolved (a missing file) is used as given, and the open then reports it.
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if side_files_present(&resolved) {
        bail!(SIDE_FILE_REFUSAL);
    }
    let url = url::Url::from_file_path(&resolved)
        .map_err(|()| anyhow!("the path of the old catalog cannot be written as a file URL"))?;
    let conn = Connection::open_with_flags(
        format!("{url}?mode=ro&immutable=1"),
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        // SQLite's own message for a failed open repeats the URL, an absolute path.
        let why = match &e {
            rusqlite::Error::SqliteFailure(code, _) => code.to_string(),
            other => other.to_string(),
        };
        anyhow!(
            "the old catalog cannot be opened read-only{}: {why}",
            if path.exists() {
                ""
            } else {
                " (the file does not exist)"
            }
        )
    })?;
    OldCatalog::from_connection(conn)
}

/// Open the old catalog at `path` read-only, and check that it holds what the diff reads.
///
/// A missing file is an error and is never created: the open is `SQLITE_OPEN_READ_ONLY` without
/// the create flag. Nothing here applies a schema, runs a migration or installs the audit.
/// See the module doc for what a WAL database does to the files beside it.
pub fn open_old_read_only(path: &Path) -> anyhow::Result<OldCatalog> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .with_context(|| {
        format!(
            "opening the old catalog {} read-only{}",
            path.display(),
            if path.exists() {
                ""
            } else {
                " (the file does not exist)"
            }
        )
    })?;
    OldCatalog::from_connection(conn).with_context(|| format!("old catalog {}", path.display()))
}

/// Where an old `abs_path` lies relative to the git root.
pub(crate) enum Scope {
    /// Under the root, outside `.worktrees/`: the git-root-relative path with `/`.
    Inside(String),
    /// Outside the root, or not spelled as a plain path below it.
    OtherRoot,
    /// Under `<root>/.worktrees/`.
    Worktree,
}

/// Judge `abs_path` by path components, so `/work/proj2/x.md` is not under `/work/proj`. A path
/// with a `..` or non-UTF-8 component after the root, or the root itself, is `OtherRoot`: it
/// cannot be named as a git-root-relative file. (`Path` already reads `a//b` and `a/./b` as
/// `a/b`.)
pub(crate) fn scope_of(git_root: &Path, abs_path: &str) -> Scope {
    let Ok(rel) = Path::new(abs_path).strip_prefix(git_root) else {
        return Scope::OtherRoot;
    };
    let mut parts: Vec<&str> = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(name) => match name.to_str() {
                Some(name) => parts.push(name),
                None => return Scope::OtherRoot,
            },
            _ => return Scope::OtherRoot,
        }
    }
    match parts.first() {
        None => Scope::OtherRoot,
        Some(&WORKTREES_DIR) => Scope::Worktree,
        Some(_) => Scope::Inside(parts.join("/")),
    }
}

/// The old catalog both `derive-shadow` and `cutover` read: the flag, else `LIBRARIAN_DB`, else
/// `librarian/catalog.db` in the local data directory, so two dry runs read one catalog.
///
/// This is not the librarian server's resolution. The server reads `LIBRARIAN_DB` from its own
/// environment (the MCP configuration), which can differ from the operator's shell, so a run
/// with the variable unset here can read another file than the server uses; pass `--old-db` to
/// name the file. With no local data directory the server falls back to
/// `/tmp/librarian/catalog.db` and this function returns `None`.
pub(crate) fn resolve_old_catalog(
    arg: Option<&Path>,
    librarian_db: Option<std::ffi::OsString>,
    data_dir: Option<PathBuf>,
) -> Option<PathBuf> {
    arg.map(Path::to_path_buf)
        .or_else(|| librarian_db.map(PathBuf::from))
        .or_else(|| data_dir.map(|dir| dir.join("librarian").join("catalog.db")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_old_catalog_prefers_the_flag_then_librarian_db_then_the_data_dir() {
        use std::ffi::OsString;
        let flag = PathBuf::from("/flag/catalog.db");
        let env = || Some(OsString::from("/env/catalog.db"));
        let data = || Some(PathBuf::from("/data"));
        assert_eq!(
            resolve_old_catalog(Some(&flag), env(), data()),
            Some(flag.clone())
        );
        assert_eq!(
            resolve_old_catalog(None, env(), data()),
            Some(PathBuf::from("/env/catalog.db"))
        );
        assert_eq!(
            resolve_old_catalog(None, None, data()),
            Some(PathBuf::from("/data/librarian/catalog.db"))
        );
        assert_eq!(resolve_old_catalog(None, None, None), None);
    }

    /// SQLite keeps `-wal` and `-shm` beside the TARGET of a symlink. A caller that holds an
    /// operator-typed link path must still see them.
    #[cfg(unix)]
    #[test]
    fn side_files_present_looks_beside_the_target_of_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let real_dir = dir.path().join("real");
        let link_dir = dir.path().join("link");
        std::fs::create_dir(&real_dir).unwrap();
        std::fs::create_dir(&link_dir).unwrap();
        let real = real_dir.join("catalog.db");
        std::fs::write(&real, b"x").unwrap();
        let link = link_dir.join("catalog.db");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        assert!(!side_files_present(&link), "no side file yet");
        std::fs::write(real_dir.join("catalog.db-wal"), b"x").unwrap();
        assert!(
            side_files_present(&link),
            "a -wal beside the target must be seen through the link"
        );
        assert!(
            side_files_present(&real),
            "the target itself still reports it"
        );
    }

    /// A path that cannot be resolved is judged as given.
    #[test]
    fn side_files_present_falls_back_to_the_given_path_when_it_does_not_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.db");
        assert!(!side_files_present(&missing));
        std::fs::write(dir.path().join("gone.db-shm"), b"x").unwrap();
        assert!(side_files_present(&missing));
    }
}
