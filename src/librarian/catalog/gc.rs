//! Catalog GC lifecycle: missing_since reconcile, hide-from-find cutoff,
//! move detection, and rename/move rehome. All existence/identity-based;
//! no scope-based deletion, no automatic deletion.

use anyhow::Result;
use rusqlite::Connection;

pub const DEFAULT_GRACE_DAYS: i64 = 14;
const MS_PER_DAY: i64 = 86_400_000;

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    let v = conn
        .query_row(
            "SELECT value FROM catalog_meta WHERE key = ?1",
            [key],
            |r| r.get::<_, String>(0),
        )
        .ok();
    Ok(v)
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO catalog_meta(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn grace_days(conn: &Connection) -> Result<i64> {
    Ok(get_meta(conn, "gc_grace_days")?
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|d| *d >= 0)
        .unwrap_or(DEFAULT_GRACE_DAYS))
}

pub fn visibility_cutoff_ms(conn: &Connection, now_ms: i64) -> Result<i64> {
    Ok(now_ms - grace_days(conn)? * MS_PER_DAY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::Catalog;

    #[test]
    fn meta_roundtrip_and_grace_default_and_cutoff() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        assert_eq!(get_meta(&cat.conn, "x").unwrap(), None);
        set_meta(&cat.conn, "x", "1").unwrap();
        assert_eq!(get_meta(&cat.conn, "x").unwrap(), Some("1".to_string()));
        // grace default
        assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
        set_meta(&cat.conn, "gc_grace_days", "7").unwrap();
        assert_eq!(grace_days(&cat.conn).unwrap(), 7);
        // cutoff = now - grace*day
        let now = 1_000_000_000_000i64;
        assert_eq!(
            visibility_cutoff_ms(&cat.conn, now).unwrap(),
            now - 7 * 86_400_000
        );
    }

    #[test]
    fn set_meta_overwrites_existing_key() {
        let cat = Catalog::open_in_memory().unwrap();
        set_meta(&cat.conn, "k", "first").unwrap();
        set_meta(&cat.conn, "k", "second").unwrap();
        assert_eq!(
            get_meta(&cat.conn, "k").unwrap(),
            Some("second".to_string())
        );
    }

    #[test]
    fn grace_days_falls_back_on_invalid_override() {
        let cat = Catalog::open_in_memory().unwrap();
        set_meta(&cat.conn, "gc_grace_days", "-1").unwrap();
        assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
        set_meta(&cat.conn, "gc_grace_days", "not-a-number").unwrap();
        assert_eq!(grace_days(&cat.conn).unwrap(), DEFAULT_GRACE_DAYS);
    }
}
