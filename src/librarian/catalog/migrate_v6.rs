//! Schema v6 migration: replace legacy `(repo, rel_path)` columns with
//! `abs_path`; rename `commits.repo` → `commits.git_root`. See
//! `docs/superpowers/specs/2026-05-08-librarian-project-model-redesign.md`.
//!
//! ## Implicit migration of `artifact_id_from_abs` hash input form
//!
//! Round 5 of the Windows CI rehab changed `ids::artifact_id_from_abs` to
//! hash the forward-slash-normalized path form (previously hashed the OS-native
//! form). This is a breaking ID change for any Windows catalog DB built
//! before that commit — same `abs_path` produces a new `id`.
//!
//! **There is no explicit migration here.** The change is absorbed by
//! `artifact::upsert`'s pre-existing pre-DELETE clause
//! (`DELETE FROM artifact WHERE abs_path = ?1 AND id != ?2`), which was
//! added for a different reason (F-6a, see commit history) but happens to
//! cover this case too: on the first post-upgrade walk, the new-id row
//! displaces the old-id row at the same `abs_path`, and the `link` table's
//! ON DELETE CASCADE keeps referential integrity. External citations to
//! the old IDs go stale — that's the documented user-visible cost.
//!
//! Reviewer note (Ibex M-1, rounds 3-8): if `artifact_id_from_abs`'s hash
//! input ever changes again, do NOT rely on this implicit safety net —
//! add an explicit migration step here.

use anyhow::Result;
use rusqlite::Connection;

use crate::librarian::catalog::column_exists;

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

use crate::librarian::workspace::WorkspaceConfig;
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
                rusqlite::params![crate::util::fs::RepoPath::from(&abs), id],
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
                    rusqlite::params![crate::util::fs::RepoPath::from_path(root), hash],
                )?;
            }
        }
    }

    Ok(())
}

/// Step 3 of the migration: drop legacy columns and stamp v6.
/// Caller MUST have already run `add_columns` and `backfill`.
/// Backup is the caller's responsibility (in `Catalog::open_with_workspace`).
///
/// Uses table-copy migration rather than `ALTER TABLE DROP COLUMN` because:
/// - UNIQUE(repo, rel_path) prevents dropping `repo` with plain ALTER TABLE.
/// - SQLite validates trigger bodies during DDL, which requires vec0 as a
///   loadable extension (.so); vec0 is statically linked here, so the CLI
///   sqlite3 binary (and any non-codescout process) cannot validate the
///   trigger, causing the DROP COLUMN to fail.
pub(super) fn drop_legacy_and_stamp(conn: &Connection) -> Result<()> {
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

    // Foreign keys MUST be disabled for the table-copy. `DROP TABLE artifact`
    // under `PRAGMA foreign_keys = ON` performs an implicit row-DELETE that
    // INVOKES foreign-key actions — firing the `ON DELETE CASCADE` on every
    // child table (artifact_augmentation, events, artifact_link,
    // artifact_observation, event_edges) and deleting their rows, even though
    // the copied artifact rows keep their ids. This copy carries only
    // `artifact` + `commits` forward, so those children would be lost.
    // (Before this guard, the migration silently wiped all augmentations +
    // event history for every artifact present when it ran — see
    // docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md.)
    // `PRAGMA foreign_keys` is a no-op INSIDE a transaction, so it must be
    // toggled OUTSIDE the BEGIN/COMMIT below. On error we ROLLBACK first so
    // the re-enable below is not swallowed by a still-open transaction.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let copy = conn.execute_batch(
        r#"
        BEGIN;

        -- Clean up any leftover temp tables from a previously aborted attempt.
        DROP TABLE IF EXISTS artifact_new;
        DROP TABLE IF EXISTS commits_new;

        CREATE TABLE artifact_new (
          id            TEXT PRIMARY KEY,
          abs_path      TEXT NOT NULL,
          kind          TEXT NOT NULL,
          status        TEXT NOT NULL,
          title         TEXT,
          owners        TEXT NOT NULL DEFAULT '[]',
          tags          TEXT NOT NULL DEFAULT '[]',
          topic         TEXT,
          time_scope    TEXT,
          source        TEXT,
          created_at    INTEGER NOT NULL,
          updated_at    INTEGER NOT NULL,
          file_mtime    INTEGER NOT NULL,
          file_sha256   TEXT NOT NULL,
          confidence    REAL NOT NULL DEFAULT 1.0,
          slug          TEXT,
          missing_since INTEGER,
          -- v11. Carried here for the reason `slug` is: the ALTER blocks in
          -- apply_migrations_in_txn run BEFORE this rebuild, so the source table
          -- already has the column and omitting it from either the DDL or the
          -- SELECT would silently drop it on the legacy path only.
          -- `every_schema_sql_artifact_column_survives_every_migration_path`
          -- is the guard that catches exactly that.
          embedded_sha256 TEXT
          );
          INSERT INTO artifact_new
          SELECT id, abs_path, kind, status, title, owners, tags, topic,
                 time_scope, source, created_at, updated_at, file_mtime,
                 file_sha256, confidence, slug, missing_since, embedded_sha256
          FROM artifact;

          -- DROP TABLE implicitly drops the artifact_vec_cascade_delete trigger.
          DROP TABLE artifact;
          ALTER TABLE artifact_new RENAME TO artifact;
          CREATE UNIQUE INDEX idx_artifact_abs_path  ON artifact(abs_path);
          CREATE        INDEX idx_artifact_kind_status ON artifact(kind, status);
          -- Plain (non-partial) unique index — required as FK parent key for
          -- entry_cite.src_slug REFERENCES artifact(slug); NULLs stay distinct
          -- under SQLite's UNIQUE index semantics, so this doesn't restrict
          -- artifacts that have no slug yet.
          CREATE UNIQUE INDEX ux_artifact_slug ON artifact(slug);
        CREATE TRIGGER artifact_vec_cascade_delete
          AFTER DELETE ON artifact BEGIN
            DELETE FROM artifact_vec WHERE id = OLD.id;
          END;

        CREATE TABLE commits_new (
          hash         TEXT PRIMARY KEY,
          git_root     TEXT,
          authored_at  INTEGER,
          subject      TEXT,
          topo_order   INTEGER
        );
        INSERT INTO commits_new
          SELECT hash, git_root, authored_at, subject, topo_order
          FROM commits;
        DROP TABLE commits;
        ALTER TABLE commits_new RENAME TO commits;
        CREATE INDEX idx_commits_git_root ON commits(git_root, topo_order);

        INSERT OR IGNORE INTO schema_version (version) VALUES (6);
        COMMIT;
        "#,
    );
    if copy.is_err() {
        // A failed batch leaves the transaction open; close it so the pragma
        // re-enable below is honored (it is ignored inside a transaction).
        let _ = conn.execute_batch("ROLLBACK;");
    }
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    copy?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
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
        let conn = new_db_with_legacy_row("codescout", "docs/trackers/foo.md");
        let ws = ws_with("codescout", "/home/u/work/codescout");
        backfill(&conn, &ws, false).unwrap();
        let abs: String = conn
            .query_row("SELECT abs_path FROM artifact WHERE id = 'a1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(abs, "/home/u/work/codescout/docs/trackers/foo.md");
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
        let conn = new_db_with_legacy_row("codescout", "docs/x.md");
        let ws = ws_with("codescout", "/abs/c");
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
        let conn = new_db_with_legacy_row("codescout", "x.md");
        conn.execute(
            "INSERT INTO commits(hash, repo, topo_order) VALUES ('abc', 'codescout', 1)",
            [],
        )
        .unwrap();
        let ws = ws_with("codescout", "/abs/c");
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
        let _ = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws);
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

        let cat = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
        let count: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact WHERE abs_path IS NOT NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let has_repo =
            crate::librarian::catalog::column_exists(&cat.conn, "artifact", "repo").unwrap();
        assert!(!has_repo);
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 12);
    }

    #[test]
    fn migration_v6_full_is_idempotent() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());
        drop(crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap());
        let cat = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 12);
    }
    #[test]
    fn migration_v6_single_open_preserves_v9_entry_graph_shape() {
        // Regression: drop_legacy_and_stamp rebuilds `artifact` via table-copy
        // (CREATE artifact_new / INSERT SELECT / DROP / RENAME). Before this fix
        // the copy's column list and index recreation stopped at `confidence`,
        // silently dropping `slug` and `ux_artifact_slug` on the legacy (v3->v6)
        // upgrade path — a single `open_with_workspace` call left the live
        // `artifact` table without `slug`, without `ux_artifact_slug`, and with
        // `entry_cite`'s FK dangling. It "self-healed" on a SECOND open (the v9
        // `IF NOT EXISTS` guards re-add slug), which is why a twice-open test
        // like `migration_v6_full_is_idempotent` couldn't catch it. This test
        // exercises exactly ONE open of a legacy DB and asserts the v9 shape is
        // already correct immediately after.
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());
        let cat = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();

        assert!(
            crate::librarian::catalog::column_exists(&cat.conn, "artifact", "slug").unwrap(),
            "artifact.slug must survive the single-open legacy upgrade"
        );

        let has_slug_index: bool = cat
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='index' AND name='ux_artifact_slug'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(
            has_slug_index,
            "ux_artifact_slug index must survive the table-copy"
        );

        // entry_cite's FK must be intact, not dangling: set a slug and insert a
        // citing row referencing it — this fails with "foreign key mismatch" (or
        // a constraint violation) if the FK's parent key isn't a real non-partial
        // unique index on artifact(slug).
        cat.conn
            .execute("UPDATE artifact SET slug = 'a1-slug' WHERE id = 'a1'", [])
            .unwrap();
        cat.conn
            .execute(
                "INSERT INTO entry_cite (src_slug, src_local, dst_ref, rel, created_at)
                 VALUES ('a1-slug', 'e1', 'other-artifact', 'cites', 0)",
                [],
            )
            .unwrap();

        let fk_violations: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(fk_violations, 0, "no foreign key violations expected");
    }

    #[test]
    fn migration_v6_preserves_augmentation_and_events() {
        // Regression for docs/issues/archive/2026-07-05-v6-migration-cascade-deletes-child-rows.md:
        // drop_legacy_and_stamp's table-copy ran under foreign_keys=ON, so
        // `DROP TABLE artifact` cascade-deleted every artifact_augmentation /
        // events row (the copy carries only artifact + commits forward). Seed a
        // pre-v6 DB with an augmented artifact + an event, run the full v6
        // migration, and assert both survive. FAILS on the pre-fix code.
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        {
            let conn = rusqlite::Connection::open(&db_path).unwrap();
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
                CREATE TABLE artifact_augmentation (
                    artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
                    prompt TEXT NOT NULL, params TEXT NOT NULL DEFAULT '{}',
                    last_refreshed_at TEXT, refresh_count INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
                    PRIMARY KEY (artifact_id)
                );
                CREATE TABLE events (
                    id TEXT PRIMARY KEY,
                    artifact_id TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
                    kind TEXT NOT NULL, payload TEXT NOT NULL,
                    anchor_commit TEXT, head_commit TEXT, author TEXT,
                    created_at INTEGER NOT NULL
                );
                CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
                INSERT OR IGNORE INTO schema_version (version) VALUES (3);
                INSERT INTO artifact(id, repo, rel_path, kind, status, title,
                                     created_at, updated_at, file_mtime, file_sha256)
                  VALUES ('a1', 'r', 'docs/x.md', 'tracker', 'active', 't', 0, 0, 0, 'sha');
                INSERT INTO artifact_augmentation(artifact_id, prompt, params)
                  VALUES ('a1', 'maintain the T-N table', '{"rows":[1,2,3]}');
                INSERT INTO events(id, artifact_id, kind, payload, created_at)
                  VALUES ('e1', 'a1', 'field_patch', '{}', 0);
                "#,
            )
            .unwrap();
        }

        let ws = ws_with("r", tmp.path().to_str().unwrap());
        let cat = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();

        let aug: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM artifact_augmentation WHERE artifact_id='a1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            aug, 1,
            "augmentation must survive the v6 table-copy migration"
        );
        let ev: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE artifact_id='a1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            ev, 1,
            "event history must survive the v6 table-copy migration"
        );

        // The surviving augmentation still carries its payload, and the legacy
        // artifact column is gone (migration actually ran, not skipped).
        let prompt: String = cat
            .conn
            .query_row(
                "SELECT prompt FROM artifact_augmentation WHERE artifact_id='a1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(prompt, "maintain the T-N table");
        let has_repo =
            crate::librarian::catalog::column_exists(&cat.conn, "artifact", "repo").unwrap();
        assert!(
            !has_repo,
            "v6 migration should have dropped legacy repo column"
        );
    }

    #[test]
    fn every_schema_sql_artifact_column_survives_every_migration_path() {
        // Generalizes migration_v6_single_open_preserves_v9_entry_graph_shape:
        // instead of hand-checking `slug` (the one column that regressed),
        // parse the canonical column list straight out of SCHEMA_SQL and check
        // ALL of them, for every migration path we seed a fixture for. Any
        // future column added to `artifact` is covered automatically, with no
        // test update required.
        let expected_columns = crate::librarian::catalog::parse_create_table_columns(
            crate::librarian::catalog::SCHEMA_SQL,
            "artifact",
        );
        assert!(
            expected_columns.contains(&"slug".to_string()),
            "sanity check: slug must be part of the parsed column set"
        );

        fn assert_index_exists(conn: &Connection, name: &str, path_label: &str) {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='index' AND name=?1",
                    [name],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            assert!(exists, "[{path_label}] index {name} must exist");
        }

        fn assert_no_fk_violations(conn: &Connection, path_label: &str) {
            let fk_violations: i64 = conn
                .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })
                .unwrap();
            assert_eq!(
                fk_violations, 0,
                "[{path_label}] expected no foreign key violations"
            );
        }

        // Path 1: fresh DB (Catalog::open_in_memory). Never runs
        // drop_legacy_and_stamp's table-copy (that only fires for on-disk
        // catalogs below schema_version 6), so idx_artifact_abs_path — which
        // is created solely by that table-copy — is NOT expected here; a
        // fresh DB instead gets an implicit unique index off schema.sql's
        // `abs_path TEXT NOT NULL UNIQUE`, not one named idx_artifact_abs_path.
        {
            let path_label = "fresh open_in_memory";
            let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
            for col in &expected_columns {
                assert!(
                    crate::librarian::catalog::column_exists(&cat.conn, "artifact", col).unwrap(),
                    "[{path_label}] artifact.{col} missing"
                );
            }
            for idx in ["idx_artifact_kind_status", "ux_artifact_slug"] {
                assert_index_exists(&cat.conn, idx, path_label);
            }
            assert_no_fk_violations(&cat.conn, path_label);
        }

        // Path 2: legacy v3 DB, single open_with_workspace — the table-copy
        // path (drop_legacy_and_stamp) that previously dropped `slug` and
        // `ux_artifact_slug` silently. All three indexes are expected here.
        {
            let path_label = "legacy v3 -> v6 single open_with_workspace";
            let tmp = tempfile::TempDir::new().unwrap();
            let db_path = tmp.path().join("catalog.db");
            seed_v3_db(&db_path);
            let ws = ws_with("r", tmp.path().to_str().unwrap());
            let cat =
                crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();

            for col in &expected_columns {
                assert!(
                    crate::librarian::catalog::column_exists(&cat.conn, "artifact", col).unwrap(),
                    "[{path_label}] artifact.{col} missing"
                );
            }
            for idx in [
                "idx_artifact_abs_path",
                "idx_artifact_kind_status",
                "ux_artifact_slug",
            ] {
                assert_index_exists(&cat.conn, idx, path_label);
            }
            assert_no_fk_violations(&cat.conn, path_label);
        }
    }

    // Task review Finding 2 (2026-09-01): open_with_workspace's ordering
    // constraint — audit::install must run AFTER drop_legacy_and_stamp — had
    // no discriminating test. Reusing this module's own v3->v6 fixture
    // (seed_v3_db + ws_with) exercises the table-copy migration path that
    // silently drops a table's triggers with the table, then proves the
    // audit triggers both exist AND fire post-open. Verified by hand: moving
    // `audit::install(&conn)` above the `if needs_v6 { ... }` block in
    // `Catalog::open_with_workspace` turns this test red (0 rows instead of
    // 2) — see task-1-report.md fix log for the captured failure output.
    #[test]
    fn open_with_workspace_installs_audit_triggers_after_v6_table_copy() {
        let tmp = tempfile::TempDir::new().unwrap();
        let db_path = tmp.path().join("catalog.db");
        seed_v3_db(&db_path);
        let ws = ws_with("r", tmp.path().to_str().unwrap());

        let cat = crate::librarian::catalog::Catalog::open_with_workspace(&db_path, &ws).unwrap();

        let trigger_count: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name LIKE 'audit_%'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // Task review Finding C (2026-09-01): `> 0` is widening-monotone — it
        // cannot tell "all triggers installed" from "one survived". Each
        // audited table gets exactly 3 triggers (insert/update/delete), so
        // the true count is derived from the live constant rather than a
        // hardcoded literal that could go stale as AUDITED_TABLES grows.
        let expected = crate::librarian::catalog::audit::AUDITED_TABLES.len() as i64 * 3;
        assert_eq!(
            trigger_count, expected,
            "audit triggers must exist after open_with_workspace's v6 table-copy path \
             (expected AUDITED_TABLES.len() * 3 = {expected})"
        );

        cat.conn
            .execute(
                "INSERT INTO artifact(id, abs_path, kind, status, title,
                                      owners, tags, created_at, updated_at,
                                      file_mtime, file_sha256)
                 VALUES ('v6a', '/r/x.md', 'tracker', 'active', 't',
                         '[]', '[]', 0, 0, 0, 'sha')",
                [],
            )
            .unwrap();
        cat.conn
            .execute("DELETE FROM artifact WHERE id='v6a'", [])
            .unwrap();
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM catalog_audit WHERE tbl='artifact' AND row_id='v6a'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            n, 2,
            "audit triggers installed after the v6 table-copy must still fire (insert+delete)"
        );
    }
}
