use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::Path;

pub mod artifact;
pub mod find;
pub mod links;
pub mod observations;

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
}
