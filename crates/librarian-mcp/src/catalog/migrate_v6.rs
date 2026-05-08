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
