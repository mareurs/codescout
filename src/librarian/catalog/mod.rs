use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

use crate::librarian::workspace::WorkspaceConfig;

pub mod artifact;
pub mod augmentation;
pub mod commits;
pub mod event_edges;
pub mod events;
pub mod find;
pub mod graft;
pub mod links;
mod migrate_v6;
pub mod observations;
pub mod sources;
pub mod worktree;

/// `RepoPath` stores its inner string in forward-slash normalized form
/// (see `src/util/fs.rs`). Implementing `ToSql` here keeps `fs.rs`
/// rusqlite-free while still letting every `params![repo_path]` call site
/// pass a `RepoPath` directly — no `.as_str()` boilerplate.
impl rusqlite::ToSql for crate::util::fs::RepoPath {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        self.as_str().to_sql()
    }
}

pub struct Catalog {
    pub conn: Connection,
}

const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Register sqlite-vec as a global auto-extension. Delegates to the shared,
/// non-feature-gated registration in `crate::sqlite_vec_ext` so there is exactly
/// one `Once` across the librarian catalog and the retrieval stores (registering
/// the same auto-extension twice would run the `vec0` init twice per connection).
fn init_sqlite_vec() {
    crate::sqlite_vec_ext::register();
}

/// Idempotent post-baseline migrations. SCHEMA_SQL covers v1-v3 (CREATE TABLE
/// IF NOT EXISTS is naturally idempotent); v4+ uses ALTER TABLE which isn't,
/// so each migration checks for its own preconditions before running.
fn run_migrations(conn: &Connection, ws: Option<&WorkspaceConfig>) -> Result<()> {
    // Atomicity across connections. Each block in `apply_migrations_in_txn` is
    // guarded by `column_exists`, but that check and the following
    // `ALTER TABLE ADD COLUMN` are separate statements. In autocommit mode two
    // connections sharing this database file can both observe a column as
    // missing and both issue the ALTER; the loser fails with "duplicate column
    // name" (bug 33e4ae68 — a guide_hint CI test flake, and a real hazard when
    // two codescout instances open a shared catalog that still needs migration).
    // Running the whole sequence in one write transaction makes the
    // check-then-ALTER atomic; combined with the connection's `busy_timeout`,
    // the second writer blocks on BEGIN IMMEDIATE, then re-checks and no-ops.
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match apply_migrations_in_txn(conn, ws) {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            return Err(e);
        }
    }
    // v9/v10: widen events.kind CHECK to allow 'worktree_fork' and
    // 'worktree_merge'. Table-copy, so it cannot run inside the BEGIN
    // IMMEDIATE above (needs its own foreign_keys=OFF window — see
    // widen_events_kind_check doc comment).
    widen_events_kind_check(conn)
}

/// The v4+ ALTER/backfill sequence run inside the write transaction opened by
/// `run_migrations`. Split out only so the transaction wrapper stays readable;
/// it assumes an open transaction and is not meant to be called directly.
fn apply_migrations_in_txn(conn: &Connection, ws: Option<&WorkspaceConfig>) -> Result<()> {
    // v4: render_template + params_schema columns on artifact_augmentation
    if !column_exists(conn, "artifact_augmentation", "render_template")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN render_template TEXT",
            [],
        )?;
    }
    if !column_exists(conn, "artifact_augmentation", "params_schema")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN params_schema TEXT",
            [],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (4)",
        [],
    )?;
    // v5: append_mode + history_cap columns on artifact_augmentation
    if !column_exists(conn, "artifact_augmentation", "append_mode")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN append_mode INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !column_exists(conn, "artifact_augmentation", "history_cap")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN history_cap INTEGER",
            [],
        )?;
    }
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (5)",
        [],
    )?;
    // v7: entry_collection column on artifact_augmentation (filterable trackers)
    if !column_exists(conn, "artifact_augmentation", "entry_collection")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN entry_collection TEXT",
            [],
        )?;
    }
    // v8: refreshed_at_commit column on artifact_augmentation (server-computed provenance;
    // written by commit_refresh, surfaced by artifact(get) as provenance.refreshed_at_commit).
    if !column_exists(conn, "artifact_augmentation", "refreshed_at_commit")? {
        conn.execute(
            "ALTER TABLE artifact_augmentation ADD COLUMN refreshed_at_commit TEXT",
            [],
        )?;
    }
    // NOTE: the entry_collection block above is ordered before the v6 add/backfill for locality
    // with the other artifact_augmentation column adds. Order is irrelevant — each
    // block is independently guarded (column_exists / catalog_needs_v6_migration),
    // so run_migrations is correct top-to-bottom regardless of version sequence.
    // v6: add abs_path/git_root alongside legacy columns, then backfill.
    // drop_legacy_and_stamp is called separately by open_with_workspace after
    // backfill — NOT here, because backfill requires a workspace config and
    // Catalog::open calls this function without one.
    migrate_v6::add_columns(conn)?;
    if let Some(ws) = ws {
        let drop_orphans = std::env::var("LIBRARIAN_MIGRATE_DROP_ORPHANS").as_deref() == Ok("1");
        migrate_v6::backfill(conn, ws, drop_orphans)?;
    }
    Ok(())
}

/// True if the `events` table's `kind` CHECK constraint already allows
/// `'worktree_merge'` (implies `'worktree_fork'` too — see the note below).
/// SQLite records a CHECK constraint's text only in the table's own
/// `CREATE TABLE` statement (`sqlite_master.sql`) — there is no `PRAGMA` to
/// introspect it column-wise the way `column_exists` does for plain
/// columns via `table_info`.
fn events_check_allows_worktree_merge(conn: &Connection) -> Result<bool> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='events'",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(match sql {
        // `worktree_merge` is always widened in together with `worktree_fork`
        // (see `widen_events_kind_check`'s table-copy CHECK text and
        // SCHEMA_SQL), so checking for `worktree_merge` alone is a sufficient
        // gate: any catalog with it present also has `worktree_fork`.
        Some(s) => s.contains("worktree_merge"),
        // No events table yet: SCHEMA_SQL (already run by the time this is
        // called) creates it fresh with the current (widened) CHECK text, so
        // there is nothing to migrate.
        None => true,
    })
}

/// v9/v10: widen the `events.kind` CHECK constraint to allow `'worktree_fork'`
/// and `'worktree_merge'` (the fork-on-first-write and merge-audit event
/// kinds — see docs/superpowers/specs/2026-07-17-worktree-overlay-design.md
/// §3/§"Merge"). A pre-existing catalog created before this constraint was
/// widened has the old CHECK baked into its `events` table and would reject
/// every `worktree_fork`/`worktree_merge` insert with `CHECK constraint
/// failed`; editing `schema.sql` alone only affects catalogs created from
/// scratch, since `CREATE TABLE IF NOT EXISTS` is a no-op once the table
/// exists.
///
/// SQLite has no `ALTER TABLE ... ALTER COLUMN` for CHECK constraints, so
/// this is a table-copy migration, following the same shape (and FK-pragma
/// caution) as `migrate_v6::drop_legacy_and_stamp`: `DROP TABLE events`
/// under `foreign_keys=ON` would cascade-delete every `event_edges` row
/// referencing it (both `src_event_id` and `dst_event_id` reference
/// `events(id) ON DELETE CASCADE`), so foreign keys are held OFF for the
/// swap and restored after — and, per that same function's note, this must
/// run OUTSIDE any already-open transaction, because `PRAGMA foreign_keys`
/// is a silent no-op inside one.
fn widen_events_kind_check(conn: &Connection) -> Result<()> {
    if events_check_allows_worktree_merge(conn)? {
        return Ok(());
    }
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let copy = conn.execute_batch(
        r#"
        BEGIN;
        DROP TABLE IF EXISTS events_new;
        CREATE TABLE events_new (
          id            TEXT PRIMARY KEY,
          artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
          kind          TEXT NOT NULL CHECK (kind IN (
                          'note', 'reviewed', 'status_change', 'field_patch',
                          'superseded_by', 'external_signal',
                          'intent', 'verdict', 'worktree_fork', 'worktree_merge'
                        )),
          payload       TEXT NOT NULL,
          anchor_commit TEXT,
          head_commit   TEXT,
          author        TEXT,
          created_at    INTEGER NOT NULL
        );
        INSERT INTO events_new
          SELECT id, artifact_id, kind, payload, anchor_commit, head_commit, author, created_at
          FROM events;
        DROP TABLE events;
        ALTER TABLE events_new RENAME TO events;
        CREATE INDEX idx_events_artifact ON events(artifact_id, created_at DESC);
        CREATE INDEX idx_events_head_commit ON events(head_commit);
        CREATE INDEX idx_events_anchor_commit ON events(anchor_commit);
        CREATE INDEX idx_events_kind ON events(kind);
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

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == column {
            return Ok(true);
        }
    }
    Ok(false)
}

fn catalog_needs_v6_migration(db_path: &Path) -> Result<bool> {
    if !db_path.exists() {
        return Ok(false);
    }
    let conn = Connection::open(db_path)
        .with_context(|| format!("inspecting {} for v6 migration", db_path.display()))?;
    // schema_version may not exist on a truly fresh DB; default to 0.
    let version: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(version < 6)
}

fn backup_db(db_path: &Path) -> Result<()> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let bak = db_path.with_extension(format!("db.pre-v6-bak.{ts}"));
    std::fs::copy(db_path, &bak).with_context(|| {
        format!(
            "backing up catalog before v6 migration: {} -> {}",
            db_path.display(),
            bak.display()
        )
    })?;
    tracing::info!("v6 migration backup created at {}", bak.display());
    Ok(())
}

impl Catalog {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating catalog dir {}", parent.display()))?;
        }
        init_sqlite_vec();
        let conn =
            Connection::open(db_path).with_context(|| format!("opening {}", db_path.display()))?;
        // Cross-process writers (separate codescout server instances sharing one
        // catalog file) block and retry instead of failing immediately.
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
        )?;
        conn.execute_batch(SCHEMA_SQL).context("applying schema")?;
        run_migrations(&conn, None).context("running migrations")?;
        // Clean up any artifact_vec rows that lost their parent artifact row
        // (e.g. orphans from before the cascade-delete trigger was added).
        conn.execute_batch("DELETE FROM artifact_vec WHERE id NOT IN (SELECT id FROM artifact);")?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self> {
        init_sqlite_vec();
        let conn = Connection::open_in_memory()?;
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        conn.execute_batch(SCHEMA_SQL).context("applying schema")?;
        run_migrations(&conn, None).context("running migrations")?;
        // Clean up any artifact_vec rows that lost their parent artifact row
        // (e.g. orphans from before the cascade-delete trigger was added).
        conn.execute_batch("DELETE FROM artifact_vec WHERE id NOT IN (SELECT id FROM artifact);")?;
        Ok(Self { conn })
    }

    pub fn open_with_workspace(db_path: &Path, ws: &WorkspaceConfig) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating catalog dir {}", parent.display()))?;
        }
        let needs_v6 = catalog_needs_v6_migration(db_path)?;
        if needs_v6 {
            backup_db(db_path)?;
        }
        init_sqlite_vec();
        let conn =
            Connection::open(db_path).with_context(|| format!("opening {}", db_path.display()))?;
        // Cross-process writers (separate codescout server instances sharing one
        // catalog file) block and retry instead of failing immediately.
        conn.execute_batch(
            "PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;",
        )?;
        conn.execute_batch(SCHEMA_SQL).context("applying schema")?;
        run_migrations(&conn, Some(ws)).context("running migrations")?;
        if needs_v6 {
            migrate_v6::drop_legacy_and_stamp(&conn).context("dropping legacy columns")?;
        }
        // Clean up any artifact_vec rows that lost their parent artifact row
        // (e.g. orphans from before the cascade-delete trigger was added).
        conn.execute_batch("DELETE FROM artifact_vec WHERE id NOT IN (SELECT id FROM artifact);")?;
        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_in_memory_and_applies_schema() {
        let cat = Catalog::open_in_memory().unwrap();
        let tables: Vec<String> = cat
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(tables.iter().any(|t| t == "artifact"));
        assert!(tables.iter().any(|t| t == "artifact_link"));
        assert!(tables.iter().any(|t| t == "artifact_observation"));
    }

    #[test]
    fn schema_has_timemachine_tables() {
        let cat = Catalog::open_in_memory().unwrap();
        let names: Vec<String> = cat
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        for t in ["events", "commits", "sources", "event_edges"] {
            assert!(
                names.iter().any(|n| n == t),
                "missing table {t}: {:?}",
                names
            );
        }
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 6);
    }

    #[test]
    fn migration_v4_adds_render_template_and_params_schema_columns() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(column_exists(&cat.conn, "artifact_augmentation", "render_template").unwrap());
        assert!(column_exists(&cat.conn, "artifact_augmentation", "params_schema").unwrap());
    }

    #[test]
    fn migration_v5_adds_append_mode_and_history_cap_columns() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(column_exists(&cat.conn, "artifact_augmentation", "append_mode").unwrap());
        assert!(column_exists(&cat.conn, "artifact_augmentation", "history_cap").unwrap());
    }
    #[test]
    fn migration_adds_abs_path_and_git_root_columns() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(column_exists(&cat.conn, "artifact", "abs_path").unwrap());
        assert!(column_exists(&cat.conn, "commits", "git_root").unwrap());
    }

    #[test]
    fn migrations_are_idempotent() {
        // Open twice on the same on-disk DB; second open must not error.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cat.sqlite");
        let _ = Catalog::open(&path).unwrap();
        let _ = Catalog::open(&path).unwrap();
        let cat = Catalog::open(&path).unwrap();
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 6);
    }
    #[test]
    fn widen_events_kind_check_migrates_pre_existing_catalog_and_preserves_data() {
        // Regression: a catalog created before the events.kind CHECK constraint
        // was widened to allow 'worktree_fork' and 'worktree_merge' has the OLD
        // constraint baked into its on-disk `events` table forever — editing
        // schema.sql alone only affects catalogs created from scratch
        // (`CREATE TABLE IF NOT EXISTS` is a no-op once the table exists). Seed
        // exactly that pre-existing shape, with a pre-existing event + an
        // event_edges row referencing it, then confirm: (a) the migration
        // widens the CHECK so both a worktree_fork AND a worktree_merge event
        // can be inserted, (b) the pre-existing event and edge survive
        // (foreign_keys=OFF during the table-copy must not cascade-delete
        // them), (c) a second open is idempotent.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cat.sqlite");
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(
                    r#"
                    CREATE TABLE artifact (
                        id TEXT PRIMARY KEY, abs_path TEXT NOT NULL UNIQUE, kind TEXT NOT NULL,
                        status TEXT NOT NULL, title TEXT, owners TEXT NOT NULL DEFAULT '[]',
                        tags TEXT NOT NULL DEFAULT '[]', topic TEXT, time_scope TEXT, source TEXT,
                        created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                        file_mtime INTEGER NOT NULL, file_sha256 TEXT NOT NULL,
                        confidence REAL NOT NULL DEFAULT 1.0
                    );
                    CREATE TABLE events (
                        id            TEXT PRIMARY KEY,
                        artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
                        kind          TEXT NOT NULL CHECK (kind IN (
                                        'note', 'reviewed', 'status_change', 'field_patch',
                                        'superseded_by', 'external_signal',
                                        'intent', 'verdict'
                                      )),
                        payload       TEXT NOT NULL,
                        anchor_commit TEXT,
                        head_commit   TEXT,
                        author        TEXT,
                        created_at    INTEGER NOT NULL
                    );
                    CREATE TABLE event_edges (
                        id              INTEGER PRIMARY KEY AUTOINCREMENT,
                        src_event_id    TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
                        dst_event_id    TEXT REFERENCES events(id) ON DELETE CASCADE,
                        dst_artifact_id TEXT REFERENCES artifact(id) ON DELETE CASCADE,
                        dst_source_id   TEXT,
                        rel             TEXT NOT NULL
                    );
                    CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
                    INSERT OR IGNORE INTO schema_version (version) VALUES (6);

                    INSERT INTO artifact(id, abs_path, kind, status, created_at, updated_at, file_mtime, file_sha256)
                      VALUES ('a1', '/test/a1.md', 'tracker', 'active', 0, 0, 0, 'sha');
                    INSERT INTO events(id, artifact_id, kind, payload, created_at)
                      VALUES ('e1', 'a1', 'note', '{}', 0);
                    INSERT INTO event_edges(src_event_id, rel) VALUES ('e1', 'parent');
                    "#,
                )
                .unwrap();
        }

        // Open via the real entry point, which runs widen_events_kind_check as
        // part of run_migrations.
        let cat = Catalog::open(&path).unwrap();

        // (a) the CHECK is now widened for both worktree event kinds.
        cat.conn
                .execute(
                    "INSERT INTO events(id, artifact_id, kind, payload, created_at) VALUES ('e2', 'a1', 'worktree_fork', '{}', 1)",
                    [],
                )
                .expect("events.kind CHECK must now allow 'worktree_fork'");
        cat.conn
                .execute(
                    "INSERT INTO events(id, artifact_id, kind, payload, created_at) VALUES ('e3', 'a1', 'worktree_merge', '{}', 2)",
                    [],
                )
                .expect("events.kind CHECK must now allow 'worktree_merge'");

        // (b) pre-existing event + edge survived the table-copy.
        let ev_count: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM events WHERE id='e1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(
            ev_count, 1,
            "pre-existing event must survive the CHECK-widening migration"
        );
        let edge_count: i64 = cat
            .conn
            .query_row(
                "SELECT COUNT(*) FROM event_edges WHERE src_event_id='e1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            edge_count, 1,
            "event_edges referencing the pre-existing event must survive the swap"
        );

        // (c) idempotent: re-opening (which re-runs widen_events_kind_check) must not error.
        drop(cat);
        let cat2 = Catalog::open(&path).unwrap();
        let ev_count2: i64 = cat2
            .conn
            .query_row("SELECT COUNT(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ev_count2, 3, "second open must not duplicate or lose rows");
    }

    #[test]
    fn run_migrations_is_safe_under_concurrent_connections() {
        // Reproduces bug 33e4ae68 (guide_hint CI flake) at its true root: the
        // check-then-ALTER blocks in run_migrations are not atomic, so two
        // connections opening the SAME catalog file can both observe a v4+
        // column as missing and both issue `ALTER TABLE ... ADD COLUMN`; the
        // loser fails with "duplicate column name". Also a real production
        // hazard when two codescout instances open a shared catalog that still
        // needs migration.
        use std::sync::{Arc, Barrier};

        // schema.sql references the `vec0` virtual table, so the sqlite-vec
        // auto-extension must be registered before any connection is opened
        // (Catalog::open does this internally; we call run_migrations directly).
        init_sqlite_vec();

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cat.sqlite");

        // Seed ONLY the v3 baseline (SCHEMA_SQL) — no migrations yet — so every
        // connection below must attempt the v4+ ALTERs and can collide on them.
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA busy_timeout = 5000;")
                .unwrap();
            conn.execute_batch(SCHEMA_SQL).unwrap();
        }

        let n = 16;
        let barrier = Arc::new(Barrier::new(n));
        let handles: Vec<_> = (0..n)
            .map(|_| {
                let path = path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let conn = Connection::open(&path).unwrap();
                    conn.execute_batch("PRAGMA busy_timeout = 5000;").unwrap();
                    // Release all threads into run_migrations simultaneously to
                    // maximise overlap on the check-then-ALTER window.
                    barrier.wait();
                    run_migrations(&conn, None)
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap().expect(
                "run_migrations must be safe under concurrent connections sharing a catalog",
            );
        }
    }

    #[test]
    fn schema_has_augmentation_table() {
        let cat = Catalog::open_in_memory().unwrap();
        let tables: Vec<String> = cat
            .conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            tables.iter().any(|t| t == "artifact_augmentation"),
            "expected artifact_augmentation table, got: {tables:?}"
        );
    }

    #[test]
    fn open_sets_busy_timeout_for_cross_process_writers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cat.sqlite");
        let cat = Catalog::open(&path).unwrap();
        let ms: i64 = cat
            .conn
            .query_row("PRAGMA busy_timeout", [], |r| r.get(0))
            .unwrap();
        assert_eq!(ms, 5000);
    }
}
