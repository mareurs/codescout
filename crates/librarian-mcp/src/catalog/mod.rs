use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub mod artifact;
pub mod augmentation;
pub mod commits;
pub mod event_edges;
pub mod events;
pub mod find;
pub mod links;
pub mod observations;
pub mod sources;

pub struct Catalog {
    pub conn: Connection,
}

const SCHEMA_SQL: &str = include_str!("schema.sql");

/// Register sqlite-vec as a global auto-extension (idempotent, Once-guarded).
fn init_sqlite_vec() {
    use std::sync::Once;

    // Compile-time pin on the upstream signature — see the matching check in
    // `src/embed/index.rs::init_sqlite_vec` for the rationale.
    const _UPSTREAM_SQLITE_VEC_INIT_SIG: unsafe extern "C" fn() = sqlite_vec::sqlite3_vec_init;

    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // SAFETY: sqlite3_vec_init is a valid SQLite extension entry point.
        unsafe {
            rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute::<
                *const (),
                unsafe extern "C" fn(
                    *mut rusqlite::ffi::sqlite3,
                    *mut *mut i8,
                    *const rusqlite::ffi::sqlite3_api_routines,
                ) -> i32,
            >(
                sqlite_vec::sqlite3_vec_init as *const (),
            )));
        }
    });
}

/// Idempotent post-baseline migrations. SCHEMA_SQL covers v1-v3 (CREATE TABLE
/// IF NOT EXISTS is naturally idempotent); v4+ uses ALTER TABLE which isn't,
/// so each migration checks for its own preconditions before running.
fn run_migrations(conn: &Connection) -> Result<()> {
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
        run_migrations(&conn).context("running migrations")?;
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
        run_migrations(&conn).context("running migrations")?;
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
        assert_eq!(v, 5);
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
        assert_eq!(v, 5);
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
