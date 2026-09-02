use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension};
use std::path::Path;

use crate::librarian::workspace::WorkspaceConfig;

pub mod artifact;
pub(crate) mod audit;
pub mod augmentation;
pub mod commits;
pub mod entry_cite;
pub mod event_edges;
pub mod events;
pub mod find;
pub mod gc;
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

/// Parses the column names declared by `CREATE TABLE [IF NOT EXISTS] <table> ( ... );`
/// in `schema_sql`. Used by the schema-invariant migration test so that every
/// column `SCHEMA_SQL` declares for `table` is checked automatically against
/// every migration path — no hand-maintained column list to fall out of sync.
/// Test-only: gated behind `#[cfg(test)]` so it doesn't count as dead code in
/// the non-test build (its only caller is the test module).
///
/// Grammar assumed (matches this file's schema.sql formatting): one column
/// per line between the opening `(` and a line that is exactly `);`. The
/// first whitespace-delimited token on each line is the column name; lines
/// that are blank, `--` comments, or table-level constraints (PRIMARY,
/// UNIQUE, FOREIGN, CHECK, CONSTRAINT) are skipped.
#[cfg(test)]
fn parse_create_table_columns(schema_sql: &str, table: &str) -> Vec<String> {
    let marker_if_not_exists = format!("CREATE TABLE IF NOT EXISTS {table} (");
    let marker_plain = format!("CREATE TABLE {table} (");
    let start = schema_sql
        .find(&marker_if_not_exists)
        .map(|i| i + marker_if_not_exists.len())
        .or_else(|| {
            schema_sql
                .find(&marker_plain)
                .map(|i| i + marker_plain.len())
        })
        .unwrap_or_else(|| panic!("no CREATE TABLE found for `{table}` in schema_sql"));

    let rest = &schema_sql[start..];
    let end = rest
        .find("\n);")
        .unwrap_or_else(|| panic!("no closing `);` found for CREATE TABLE `{table}`"));
    let body = &rest[..end];

    const TABLE_CONSTRAINT_KEYWORDS: &[&str] =
        &["PRIMARY", "UNIQUE", "FOREIGN", "CHECK", "CONSTRAINT"];

    body.lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.is_empty() || trimmed.starts_with("--") {
                return None;
            }
            let first = trimmed.split_whitespace().next()?;
            if TABLE_CONSTRAINT_KEYWORDS.contains(&first.to_uppercase().as_str()) {
                return None;
            }
            Some(first.to_string())
        })
        .collect()
}

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
    // v9: entry-graph — artifact.slug + entry_cite table (Stage 2, TMR-1/TMR-7).
    if !column_exists(conn, "artifact", "slug")? {
        conn.execute("ALTER TABLE artifact ADD COLUMN slug TEXT", [])?;
    }
    // Note: no WHERE clause — SQLite requires a FK parent key to be covered by a
    // non-partial UNIQUE index (entry_cite.src_slug references artifact(slug) below).
    // NULLs are still treated as distinct by SQLite's UNIQUE index semantics, so this
    // still permits any number of NULL slugs; only non-null duplicates are rejected.
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS ux_artifact_slug ON artifact(slug)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entry_cite (
           src_slug   TEXT NOT NULL REFERENCES artifact(slug) ON DELETE CASCADE,
           src_local  TEXT NOT NULL,
           dst_ref    TEXT NOT NULL,
           rel        TEXT NOT NULL,
           origin     TEXT NOT NULL DEFAULT 'write',
           created_at INTEGER NOT NULL,
           PRIMARY KEY (src_slug, src_local, dst_ref, rel)
         )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_entry_cite_dst ON entry_cite(dst_ref)",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (9)",
        [],
    )?;
    // v10: catalog GC lifecycle — missing_since on artifact + catalog_meta kv.
    if !column_exists(conn, "artifact", "missing_since")? {
        conn.execute("ALTER TABLE artifact ADD COLUMN missing_since INTEGER", [])?;
    }
    // Entry-id reservations for ledgers. DELIBERATELY separate from
    // `artifact_augmentation`: a ledger's identity (`entry_prefix` in frontmatter)
    // is committed and portable, while a reservation is transient, local, and
    // re-derivable from the committed body — so binding the two would make a
    // portable declaration depend on a machine-local row. See HY-10.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS entry_reservation (
           artifact_id   TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
           prefix        TEXT NOT NULL,
           max_allocated INTEGER NOT NULL,
           updated_at    TEXT NOT NULL,
           PRIMARY KEY (artifact_id, prefix)
         )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS catalog_meta (
           key   TEXT PRIMARY KEY,
           value TEXT
         )",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (10)",
        [],
    )?;
    // v11: chunk-grain artifact embeddings.
    //
    // `artifact_vec` needs no schema change — its `id` is already TEXT PRIMARY KEY
    // and nothing requires it to DENOTE an artifact. v11 adds the side table and a
    // second vec table; a later task backfills v2 and swaps. Keeping both alive is
    // what avoids a dark window over ~90,500 embeds.
    //
    // `chunk_id` is an OPAQUE uuid, deliberately not derived from artifact_id:
    // `id = sha256(abs_path)`, so archiving re-keys an artifact, and a derived
    // chunk id would make every archive move an O(chunks) loop through
    // `gc::migrate_vec_id` (which exists only because vec0 rejects UPDATE ... SET id).
    conn.execute(
        "CREATE TABLE IF NOT EXISTS artifact_chunk (
           chunk_id     TEXT PRIMARY KEY,
           artifact_id  TEXT NOT NULL REFERENCES artifact(id) ON DELETE CASCADE,
           chunk_ix     INTEGER NOT NULL,
           start_line   INTEGER NOT NULL,
           end_line     INTEGER NOT NULL,
           entry_token  TEXT,
           content      TEXT NOT NULL,
           content_hash TEXT NOT NULL,
           UNIQUE (artifact_id, chunk_ix)
         )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_artifact_chunk_artifact
           ON artifact_chunk(artifact_id)",
        [],
    )?;
    conn.execute(
        "CREATE VIRTUAL TABLE IF NOT EXISTS artifact_vec_v2 USING vec0(
           id        TEXT PRIMARY KEY,
           embedding FLOAT[768]
         )",
        [],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO schema_version (version) VALUES (11)",
        [],
    )?;
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

/// The catalog's backing DB file, or `None` for an in-memory connection.
/// `PRAGMA database_list` yields rows `(seq, name, file)`; the `main` database's
/// `file` column is `""` for an in-memory/temp connection and an absolute path
/// for a file-backed one.
pub(crate) fn catalog_db_path(conn: &rusqlite::Connection) -> Option<std::path::PathBuf> {
    let file: String = conn
        .query_row("PRAGMA database_list", [], |row| row.get::<_, String>(2))
        .ok()?;
    if file.is_empty() {
        None
    } else {
        Some(std::path::PathBuf::from(file))
    }
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
        audit::install(&conn).context("installing audit triggers")?;
        audit::install_session(&conn, &audit::resolve_actor())
            .context("installing audit session")?;
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
        audit::install(&conn).context("installing audit triggers")?;
        audit::install_session(&conn, &audit::resolve_actor())
            .context("installing audit session")?;
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
        audit::install(&conn).context("installing audit triggers")?;
        audit::install_session(&conn, &audit::resolve_actor())
            .context("installing audit session")?;
        // Clean up any artifact_vec rows that lost their parent artifact row
        // (e.g. orphans from before the cascade-delete trigger was added).
        conn.execute_batch("DELETE FROM artifact_vec WHERE id NOT IN (SELECT id FROM artifact);")?;
        Ok(Self { conn })
    }

    /// Best-effort verb tag for subsequent audit rows on this connection.
    /// The verb persists until the next stamp — it means "last dispatched verb",
    /// not "verb of this exact statement"; audit_log documents this.
    pub fn set_audit_verb(&self, verb: &str) -> rusqlite::Result<()> {
        self.conn
            .execute("UPDATE audit_ctx SET verb = ?1", [verb])
            .map(|_| ())
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
        assert_eq!(v, 11);
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
        assert_eq!(v, 11);
    }

    #[test]
    fn v10_adds_missing_since_and_catalog_meta() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        // missing_since column exists and defaults NULL
        assert!(column_exists(&cat.conn, "artifact", "missing_since").unwrap());
        // catalog_meta table exists and is writable
        cat.conn
            .execute("INSERT INTO catalog_meta(key, value) VALUES ('k','v')", [])
            .unwrap();
        let v: String = cat
            .conn
            .query_row("SELECT value FROM catalog_meta WHERE key='k'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(v, "v");
        let ver: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert!(ver >= 10);
    }
    #[test]
    fn v11_creates_the_chunk_table_and_stamps_the_version() {
        let cat = Catalog::open_in_memory().unwrap();
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 11);
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_chunk", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn v11_is_idempotent() {
        let cat = Catalog::open_in_memory().unwrap();
        apply_migrations_in_txn(&cat.conn, None).unwrap();
        let v: i64 = cat
            .conn
            .query_row("SELECT MAX(version) FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 11, "re-running must not advance or duplicate");
    }

    #[test]
    fn deleting_an_artifact_cascades_its_chunk_rows() {
        use crate::librarian::catalog::artifact::{self, TestArtifactRowBuilder};

        fn art(
            id: &str,
            kind: &str,
            status: &str,
        ) -> crate::librarian::catalog::artifact::ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/{id}.md"))
                .with_kind(kind)
                .with_status(status)
                .with_file_sha256("x")
                .build()
        }

        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &art("a", "spec", "active")).unwrap();
        cat.conn
            .execute(
                "INSERT INTO artifact_chunk
                   (chunk_id, artifact_id, chunk_ix, start_line, end_line, entry_token, content, content_hash)
                 VALUES ('c1','a',0,1,9,NULL,'body','h')",
                [],
            )
            .unwrap();
        cat.conn
            .execute("DELETE FROM artifact WHERE id='a'", [])
            .unwrap();
        let n: i64 = cat
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_chunk", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0, "FK cascade must remove the chunk rows");
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
    fn widen_events_kind_check_migrates_catalog_that_already_allows_worktree_fork_but_not_worktree_merge(
    ) {
        // Discriminating regression for `events_check_allows_worktree_merge`.
        // The guard MUST test for `'worktree_merge'` specifically. The sibling
        // test above (`..._migrates_pre_existing_catalog_and_preserves_data`)
        // only seeds a catalog whose CHECK has NEITHER worktree kind, so a
        // mutation reverting the guard to `contains("worktree_fork")` would
        // still pass it (both guards agree "needs migration" when fork is
        // absent too) — it does not discriminate the correct guard from the
        // buggy one.
        //
        // This test seeds the intermediate Task-4-era shape instead: CHECK
        // already lists 'worktree_fork' (added when forking landed) but not
        // yet 'worktree_merge' (added later, for this merge feature) — exactly
        // where the two guards diverge. The correct guard (checks for
        // 'worktree_merge') reports "still needs migration" and widens the
        // CHECK; the buggy guard (checks for 'worktree_fork') would report
        // "already migrated", skip the widen, and leave 'worktree_merge'
        // inserts rejected by the stale CHECK constraint.
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
                                        'intent', 'verdict', 'worktree_fork'
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
                      VALUES ('e1', 'a1', 'worktree_fork', '{}', 0);
                    "#,
                )
                .unwrap();
        }

        // Open via the real entry point, which runs widen_events_kind_check as
        // part of run_migrations.
        let cat = Catalog::open(&path).unwrap();

        cat.conn
            .execute(
                "INSERT INTO events(id, artifact_id, kind, payload, created_at) VALUES ('e2', 'a1', 'worktree_merge', '{}', 1)",
                [],
            )
            .expect(
                "events.kind CHECK must allow 'worktree_merge' even when the catalog already \
                 allowed 'worktree_fork' before open — the guard must key off \
                 'worktree_merge' specifically, not 'worktree_fork'",
            );
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
    fn migration_v9_adds_slug_column_and_entry_cite_table() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(column_exists(&cat.conn, "artifact", "slug").unwrap());
        let has_entry_cite: bool = cat
            .conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type='table' AND name='entry_cite'",
                [],
                |_| Ok(true),
            )
            .unwrap_or(false);
        assert!(has_entry_cite, "entry_cite table must exist");
    }

    #[test]
    fn parse_create_table_columns_extracts_artifact_columns() {
        let cols = parse_create_table_columns(SCHEMA_SQL, "artifact");
        assert_eq!(
                cols,
                vec![
                    "id",
                    "abs_path",
                    "kind",
                    "status",
                    "title",
                    "owners",
                    "tags",
                    "topic",
                    "time_scope",
                    "source",
                    "created_at",
                    "updated_at",
                    "file_mtime",
                    "file_sha256",
                    "confidence",
                    "slug",
                    "missing_since",
                ],
                "parse_create_table_columns must return every artifact column schema.sql declares, in order"
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

    #[test]
    fn catalog_db_path_none_for_in_memory() {
        let cat = Catalog::open_in_memory().unwrap();
        assert!(catalog_db_path(&cat.conn).is_none());
    }

    #[test]
    fn catalog_db_path_some_for_file_backed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("catalog.db");
        let cat = Catalog::open(&path).unwrap();
        let got = catalog_db_path(&cat.conn).expect("file-backed catalog must report a path");
        // SQLite may hand back a canonicalized form; compare on the file name.
        assert_eq!(got.file_name(), path.file_name());
    }
}
