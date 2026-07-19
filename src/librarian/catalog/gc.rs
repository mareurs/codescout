//! Catalog GC lifecycle: missing_since reconcile, hide-from-find cutoff,
//! move detection, and rename/move rehome. All existence/identity-based;
//! no scope-based deletion, no automatic deletion.

use anyhow::Result;
use rusqlite::Connection;

pub const DEFAULT_GRACE_DAYS: i64 = 14;
const MS_PER_DAY: i64 = 86_400_000;

pub fn get_meta(conn: &Connection, key: &str) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT value FROM catalog_meta WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    ) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
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

/// The hide-from-find predicate. `cutoff_ms` is inlined (a server-computed
/// i64 — no user input, no injection). A row is visible if it was never
/// missing, or went missing within the grace window (missing_since > cutoff).
pub fn visibility_sql(cutoff_ms: i64) -> String {
    format!("(missing_since IS NULL OR missing_since > {cutoff_ms})")
}

/// Count of rows currently hidden (missing_since <= cutoff).
pub fn hidden_count(conn: &Connection, cutoff_ms: i64) -> Result<usize> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifact WHERE missing_since IS NOT NULL AND missing_since <= ?1",
        [cutoff_ms],
        |r| r.get(0),
    )?;
    Ok(n.max(0) as usize)
}

#[derive(Debug, Default, PartialEq)]
pub struct ReconcileStats {
    pub newly_missing: usize,
    pub cleared: usize,
    pub still_missing: usize,
}

/// Stat every artifact row's abs_path; stamp newly-missing, clear returned.
/// NEVER deletes. NEVER scope-based. Idempotent on an unchanged filesystem.
pub fn reconcile_missing_since(conn: &Connection, now_ms: i64) -> Result<ReconcileStats> {
    let mut stmt = conn.prepare("SELECT id, abs_path, missing_since FROM artifact")?;
    let rows: Vec<(String, Option<String>, Option<i64>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<std::result::Result<_, _>>()?;
    let mut stats = ReconcileStats::default();
    for (id, abs_path, missing_since) in rows {
        let exists = abs_path
            .as_deref()
            .map(|p| std::path::Path::new(p).exists())
            .unwrap_or(false);
        match (exists, missing_since) {
            (false, None) => {
                conn.execute(
                    "UPDATE artifact SET missing_since = ?1 WHERE id = ?2",
                    rusqlite::params![now_ms, id],
                )?;
                stats.newly_missing += 1;
            }
            (false, Some(_)) => stats.still_missing += 1,
            (true, Some(_)) => {
                conn.execute(
                    "UPDATE artifact SET missing_since = NULL WHERE id = ?1",
                    [&id],
                )?;
                stats.cleared += 1;
            }
            (true, None) => {}
        }
    }
    Ok(stats)
}

/// 24h in ms — the default throttle interval for `reconcile_if_due`.
pub const RECONCILE_INTERVAL_MS: i64 = 24 * 3_600_000;

/// Run reconcile only if the last run was more than `min_interval_ms` ago.
/// Records `last_reconcile_at` on run. Returns `None` when throttled.
pub fn reconcile_if_due(
    conn: &Connection,
    now_ms: i64,
    min_interval_ms: i64,
) -> Result<Option<ReconcileStats>> {
    let last = get_meta(conn, "last_reconcile_at")?
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(i64::MIN);
    if now_ms.saturating_sub(last) < min_interval_ms {
        return Ok(None);
    }
    let stats = reconcile_missing_since(conn, now_ms)?;
    set_meta(conn, "last_reconcile_at", &now_ms.to_string())?;
    Ok(Some(stats))
}

/// Best-effort, non-blocking, throttled reconcile for the librarian
/// tool-call path (`LibrarianAdapter::call`). Uses a non-blocking
/// `try_lock` — a busy catalog is skipped rather than waited on, so this
/// can never stall a call. Any error from the reconcile itself (e.g. a
/// broken catalog) is logged and swallowed: this must never break the
/// librarian call it's piggybacking on.
pub fn maybe_reconcile(
    catalog: &std::sync::Arc<parking_lot::Mutex<crate::librarian::catalog::Catalog>>,
    now_ms: i64,
) {
    let Some(cat) = catalog.try_lock() else {
        return;
    };
    if let Err(e) = reconcile_if_due(&cat.conn, now_ms, RECONCILE_INTERVAL_MS) {
        tracing::debug!("gc reconcile skipped: {e}");
    }
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

    fn seed(cat: &Catalog, id: &str, abs_path: &str) {
        cat.conn
                .execute(
                    "INSERT INTO artifact \
                     (id, abs_path, kind, status, title, created_at, updated_at, file_mtime, file_sha256) \
                     VALUES (?1, ?2, 'tracker', 'active', 't', 0, 0, 0, '')",
                    rusqlite::params![id, abs_path],
                )
                .unwrap();
    }

    #[test]
    fn reconcile_stamps_missing_clears_returned_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        let present = dir.path().join("here.md");
        std::fs::write(&present, "x").unwrap();
        seed(&cat, "gone", "/nonexistent/gone.md");
        seed(&cat, "here", present.to_str().unwrap());

        let s = reconcile_missing_since(&cat.conn, 1000).unwrap();
        assert_eq!(s.newly_missing, 1);
        assert_eq!(s.cleared, 0);
        assert_eq!(s.still_missing, 0);
        let ms: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms, Some(1000));
        let ms2: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='here'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms2, None);

        // idempotent: second run with no fs change stamps nothing new
        let s2 = reconcile_missing_since(&cat.conn, 2000).unwrap();
        assert_eq!(s2.newly_missing, 0);
        assert_eq!(s2.still_missing, 1);
        let ms3: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms3, Some(1000), "existing stamp is not overwritten");

        // file returns → cleared
        let returned = dir.path().join("returned.md");
        cat.conn
            .execute(
                "UPDATE artifact SET abs_path=?1 WHERE id='gone'",
                rusqlite::params![returned.to_str().unwrap()],
            )
            .unwrap();
        std::fs::write(&returned, "x").unwrap();
        let s3 = reconcile_missing_since(&cat.conn, 3000).unwrap();
        assert_eq!(s3.cleared, 1);
        assert_eq!(s3.still_missing, 0);
        let ms4: Option<i64> = cat
            .conn
            .query_row(
                "SELECT missing_since FROM artifact WHERE id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ms4, None);
    }
    #[test]
    fn reconcile_if_due_respects_throttle() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        seed(&cat, "gone", "/nonexistent/z.md");
        // first call runs
        let a = reconcile_if_due(&cat.conn, 100_000, 1000).unwrap();
        assert!(a.is_some());
        // second call within interval is throttled
        let b = reconcile_if_due(&cat.conn, 100_500, 1000).unwrap();
        assert!(b.is_none());
        // after interval, runs again
        let c = reconcile_if_due(&cat.conn, 101_200, 1000).unwrap();
        assert!(c.is_some());
    }

    #[test]
    fn maybe_reconcile_stamps_and_throttles() {
        let dir = tempfile::tempdir().unwrap();
        let cat = Catalog::open(&dir.path().join("c.db")).unwrap();
        seed(&cat, "gone", "/nonexistent/mr.md");
        let catalog = std::sync::Arc::new(parking_lot::Mutex::new(cat));

        // first call: due (no last_reconcile_at yet) -> stamps missing_since + records last_reconcile_at
        maybe_reconcile(&catalog, 100_000);
        {
            let cat = catalog.lock();
            let ms: Option<i64> = cat
                .conn
                .query_row(
                    "SELECT missing_since FROM artifact WHERE id='gone'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ms, Some(100_000));
            assert_eq!(
                get_meta(&cat.conn, "last_reconcile_at").unwrap(),
                Some("100000".to_string())
            );
        }

        // seed a second missing row, then call again well within the 24h
        // interval -> throttled: neither the new row is touched nor
        // last_reconcile_at is bumped.
        {
            let cat = catalog.lock();
            seed(&cat, "gone2", "/nonexistent/mr2.md");
        }
        maybe_reconcile(&catalog, 100_500);
        {
            let cat = catalog.lock();
            let ms2: Option<i64> = cat
                .conn
                .query_row(
                    "SELECT missing_since FROM artifact WHERE id='gone2'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(ms2, None, "throttled call must not run reconcile");
            assert_eq!(
                get_meta(&cat.conn, "last_reconcile_at").unwrap(),
                Some("100000".to_string()),
                "last_reconcile_at must not move while throttled"
            );
        }
    }
}
