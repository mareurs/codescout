use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

use crate::librarian::workspace::WorkspaceConfig;

pub mod artifact;
pub mod augmentation;
pub mod commits;
pub mod event_edges;
pub mod events;
pub mod find;
pub mod links;
mod migrate_v6;
pub mod observations;
pub mod sources;

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
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
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
        conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
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
}
