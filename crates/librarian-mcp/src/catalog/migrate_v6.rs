use anyhow::Result;
use rusqlite::Connection;

use crate::catalog::column_exists;

/// Step 1 of the migration: add new columns alongside legacy ones.
/// Idempotent — checks column presence first.
pub(super) fn add_columns(conn: &Connection) -> Result<()> {
    if !column_exists(conn, "artifact", "abs_path")? {
        conn.execute("ALTER TABLE artifact ADD COLUMN abs_path TEXT", [])?;
    }
    if !column_exists(conn, "commits", "git_root")? {
        conn.execute("ALTER TABLE commits ADD COLUMN git_root TEXT", [])?;
    }
    Ok(())
}

use crate::workspace::WorkspaceConfig;
use std::collections::HashMap;
use std::path::PathBuf;

/// Step 2 of the migration: backfill `abs_path` and `git_root` for every
/// legacy row, using the workspace.toml `[[roots]]` lookup. Idempotent —
/// rows that already have a non-NULL `abs_path` are skipped.
/// No-op if legacy columns are already gone (post-v6).
pub(super) fn backfill(conn: &Connection, ws: &WorkspaceConfig, drop_orphans: bool) -> Result<()> {
    let has_artifact_repo = column_exists(conn, "artifact", "repo")?;
    let has_artifact_rel_path = column_exists(conn, "artifact", "rel_path")?;
    let has_commits_repo = column_exists(conn, "commits", "repo")?;

    if !has_artifact_repo && !has_artifact_rel_path && !has_commits_repo {
        return Ok(());
    }

    let lookup: HashMap<&str, &PathBuf> = ws
        .roots
        .iter()
        .map(|r| (r.name.as_str(), &r.path))
        .collect();

    if has_artifact_repo && has_artifact_rel_path {
        // Detect orphans BEFORE writing.
        let orphan_ids: Vec<String> = {
            let mut stmt = conn.prepare("SELECT id, repo FROM artifact WHERE abs_path IS NULL")?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            rows.filter_map(|row| {
                let (id, repo) = row.ok()?;
                (!lookup.contains_key(repo.as_str())).then_some(id)
            })
            .collect()
        };

        if !orphan_ids.is_empty() {
            if drop_orphans {
                for id in &orphan_ids {
                    conn.execute("DELETE FROM artifact WHERE id = ?1", [id])?;
                }
            } else {
                let sample: Vec<&str> = orphan_ids.iter().take(5).map(String::as_str).collect();
                anyhow::bail!(
                    "{} artifact(s) reference unknown root: {}{}. Either restore the \
                     root in workspace.toml or set LIBRARIAN_MIGRATE_DROP_ORPHANS=1 \
                     to discard them.",
                    orphan_ids.len(),
                    sample.join(", "),
                    if orphan_ids.len() > 5 { ", …" } else { "" },
                );
            }
        }

        // Backfill artifact.abs_path.
        let mut stmt =
            conn.prepare("SELECT id, repo, rel_path FROM artifact WHERE abs_path IS NULL")?;
        let rows: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        for (id, repo, rel_path) in rows {
            let root = lookup.get(repo.as_str()).expect("orphans rejected above");
            let abs = root.join(&rel_path);
            conn.execute(
                "UPDATE artifact SET abs_path = ?1 WHERE id = ?2",
                rusqlite::params![abs.to_string_lossy(), id],
            )?;
        }
    }

    if has_commits_repo {
        // Backfill commits.git_root.
        let mut stmt = conn.prepare("SELECT hash, repo FROM commits WHERE git_root IS NULL")?;
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        for (hash, repo) in rows {
            if let Some(root) = lookup.get(repo.as_str()) {
                conn.execute(
                    "UPDATE commits SET git_root = ?1 WHERE hash = ?2",
                    rusqlite::params![root.to_string_lossy(), hash],
                )?;
            }
        }
    }

    Ok(())
}

/// Step 3 of the migration: drop legacy columns and stamp v6.
/// Caller MUST have already run `add_columns` and `backfill`.
/// Backup is the caller's responsibility (in `Catalog::open_with_workspace`).
pub(super) fn drop_legacy_and_stamp(conn: &Connection) -> Result<()> {
    let v: String = conn.query_row("SELECT sqlite_version()", [], |r| r.get(0))?;
    if !sqlite_version_supports_drop_column(&v) {
        anyhow::bail!(
            "SQLite {v} does not support ALTER DROP COLUMN (need >= 3.35). \
             Upgrade SQLite or restore the .pre-v6-bak file and downgrade librarian-mcp."
        );
    }

    let has_repo = column_exists(conn, "artifact", "repo")?;
    let has_rel_path = column_exists(conn, "artifact", "rel_path")?;
    let has_commits_repo = column_exists(conn, "commits", "repo")?;

    // Idempotency: nothing to do if all legacy columns are already gone.
    if !has_repo && !has_rel_path && !has_commits_repo {
        conn.execute(
            "INSERT OR IGNORE INTO schema_version (version) VALUES (6)",
            [],
        )?;
        return Ok(());
    }

    if has_repo {
        conn.execute("ALTER TABLE artifact DROP COLUMN repo", [])?;
    }
    if has_rel_path {
        conn.execute("ALTER TABLE artifact DROP COLUMN rel_path", [])?;
    }
    if has_commits_repo {
        conn.execute("ALTER TABLE commits DROP COLUMN repo", [])?;
    }
    conn.execute_batch(
        r#"
        DROP INDEX IF EXISTS idx_artifact_repo;
        CREATE UNIQUE INDEX IF NOT EXISTS idx_artifact_abs_path ON artifact(abs_path);
        DROP INDEX IF EXISTS idx_commits_repo_topo;
        CREATE INDEX IF NOT EXISTS idx_commits_git_root ON commits(git_root, topo_order);
    "#,
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (6)",
        [],
    )?;
    Ok(())
}

fn sqlite_version_supports_drop_column(v: &str) -> bool {
    let parts: Vec<u32> = v.split('.').filter_map(|s| s.parse().ok()).collect();
    matches!(parts.as_slice(), [maj, min, ..] if (*maj, *min) >= (3, 35))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{Root, WorkspaceConfig};
    use rusqlite::Connection;
    use std::path::PathBuf;

    fn new_db_with_legacy_row(repo: &str, rel_path: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE artifact (
                id TEXT PRIMARY KEY, repo TEXT NOT NULL, rel_path TEXT NOT NULL,
                kind TEXT NOT NULL, status TEXT NOT NULL, title TEXT,
                owners TEXT NOT NULL DEFAULT '[]', tags TEXT NOT NULL DEFAULT '[]',
                topic TEXT, time_scope TEXT, source TEXT,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                file_mtime INTEGER NOT NULL, file_sha256 TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE commits (
                hash TEXT PRIMARY KEY, repo TEXT NOT NULL,
                authored_at INTEGER, subject TEXT, topo_order INTEGER
            );
        "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artifact(id, repo, rel_path, kind, status, title,
                                  created_at, updated_at, file_mtime, file_sha256)
             VALUES ('a1', ?1, ?2, 'tracker', 'active', 't', 0, 0, 0, 'sha')",
            rusqlite::params![repo, rel_path],
        )
        .unwrap();
        // Apply v6 step 1 (add columns).
        add_columns(&conn).unwrap();
        conn
    }

    fn ws_with(root_name: &str, root_path: &str) -> WorkspaceConfig {
        WorkspaceConfig {
            roots: vec![Root {
                name: root_name.into(),
                path: PathBuf::from(root_path),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }
    }

    fn seed_v3_db(db_path: &std::path::Path) {
        let conn = rusqlite::Connection::open(db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE artifact (
                id TEXT PRIMARY KEY, repo TEXT NOT NULL, rel_path TEXT NOT NULL,
                kind TEXT NOT NULL, status TEXT NOT NULL, title TEXT,
                owners TEXT NOT NULL DEFAULT '[]', tags TEXT NOT NULL DEFAULT '[]',
                topic TEXT, time_scope TEXT, source TEXT,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                file_mtime INTEGER NOT NULL, file_sha256 TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 1.0
            );
            CREATE TABLE commits (
                hash TEXT PRIMARY KEY, repo TEXT NOT NULL,
                authored_at INTEGER, subject TEXT, topo_order INTEGER
            );
            CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
            INSERT OR IGNORE INTO schema_version (version) VALUES (3);
        "#,
        )
        .unwrap();
        conn.execute(
            "INSERT INTO artifact(id, repo, rel_path, kind, status, title,
                                  created_at, updated_at, file_mtime, file_sha256)
             VALUES ('a1', 'r', 'docs/x.md', 'tracker', 'active', 't', 0, 0, 0, 'sha')",
            [],
        )
        .unwrap();
    }

    #[test]
    fn migration_v6_translates_repo_to_abs_path() {
        let conn = new_db_with_legacy_row("code-explorer", "docs/trackers/foo.md");
        let ws = ws_with("code-explorer", "/home/u/work/code-explorer");
        backfill(&conn, &ws, false).unwrap();
        let abs: String = conn
            .query_row("SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(abs, "/home/u/work/code-explorer/docs/trackers/foo.md");
    }

    #[test]
    fn migration_v6_fails_loudly_on_orphans() {
        let conn = new_db_with_legacy_row("ghost", "x.md");
        let ws = ws_with("alive", "/abs/alive");
        let err = backfill(&conn, &ws, false).unwrap_err();
        assert!(err.to_string().contains("ghost") || err.to_string().contains("a1"));
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn migration_v6_drops_orphans_when_opt_in() {
        let conn = new_db_with_legacy_row("ghost", "x.md");
        let ws = ws_with("alive", "/abs/alive");
        backfill(&conn, &ws, true).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn migration_v6_backfill_is_idempotent() {
        let conn = new_db_with_legacy_row("code-explorer", "docs/x.md");
        let ws = ws_with("code-explorer", "/abs/c");
        backfill(&conn, &ws, false).unwrap();
        let first: String = conn
            .query_row("SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        backfill(&conn, &ws, false).unwrap();
        let second: String = conn
            .query_row("SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn migration_v6_handles_commits_table() {
        let conn = new_db_with_legacy_row("code-explorer", "x.md");
        conn.execute(
            "INSERT INTO commits(hash, repo, topo_order) VALUES ('abc', 'code-explorer', 1)",
            [],
        )
        .unwrap();
        let ws = ws_with("code-explorer", "/abs/c");
        backfill(&conn, &ws, false).unwrap();
        let git_root: String = conn
            .query_row("SELECT git_root FROM commits WHERE hash = 'abc'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(git_root, "/abs/c");
    }

    #[test]
    fn migration_v6_creates_backup_file() {
        use std::fs;
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());
        let _ = crate::catalog::Catalog::open_with_workspace(&db_path, &ws);
        let entries: Vec<_> = fs::read_dir(tmp.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(
            entries
                .iter()
                .any(|n| n.to_string_lossy().starts_with("catalog.db.pre-v6-bak.")),
            "backup file not created; entries: {:?}",
            entries
        );
    }

    #[test]
    fn migration_v6_full_path_translates_and_drops() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());

        let cat = crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE abs_path IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let has_repo = crate::catalog::column_exists(&cat.conn, "artifact", "repo").unwrap();
        assert!(!has_repo);
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 6);
    }

    #[test]
    fn migration_v6_full_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());
        drop(crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap());
        let cat = crate::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 6);
    }
}
