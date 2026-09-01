//! Append-only catalog audit trail (T-1, spec: docs/superpowers/specs/
//! 2026-09-01-catalog-audit-trail-design.md + this plan's Design Correction).
//!
//! Main-schema triggers capture every writer's mutations with actor DEFAULT
//! 'unknown'; a per-connection TEMP trigger (install_session, Task 2) enriches
//! this connection's rows. NEVER reference temp objects from the main triggers:
//! probed 2026-09-01 — it creates silently, then REFUSES foreign writers'
//! mutations at fire time ("no such table: main.audit_ctx").
//!
//! **Failure direction — stated narrowly, because the broad form was false.**
//! Capture never blocks or mis-attributes a writer *for any value a column can
//! hold*, BLOBs included (see `value_expr`'s first arm — `json_object()` raises
//! on a BLOB, and a raising trigger aborts the caller's transaction, not just
//! its audit row). It does NOT cover a NULL row-id expression: `catalog_audit`
//! declares `row_id TEXT NOT NULL`, so a writer inserting a row whose key
//! columns are all NULL would have its own write refused by the audit trigger.
//! No current schema permits that — every audited table's key is `NOT NULL` or
//! a PRIMARY KEY — so there is no caller to reach a guard, and per CLAUDE.md's
//! loudness law a guard nothing reaches is decoration. It is documented here
//! instead: a schema change that makes a key column nullable owes this file a
//! `COALESCE` and a test that can fail.
//! See docs/issues/archive/2026-09-01-audit-trigger-can-abort-writer-on-null-key-or-blob.md.

use anyhow::Result;
use rusqlite::Connection;

pub(crate) mod host;
pub(crate) mod shard;

/// Tables under audit. Columns are read from live PRAGMA table_info at
/// install time, so migration-added columns are always covered and a
/// column-list drift can never break an open. row_id exprs are the one
/// per-table static: a composite key flattened to TEXT.
pub(crate) struct AuditedTable {
    pub name: &'static str,
    pub row_id_new: &'static str,
    pub row_id_old: &'static str,
}

pub(crate) const AUDITED_TABLES: &[AuditedTable] = &[
    AuditedTable {
        name: "artifact",
        row_id_new: "NEW.id",
        row_id_old: "OLD.id",
    },
    AuditedTable {
        name: "artifact_augmentation",
        row_id_new: "NEW.artifact_id",
        row_id_old: "OLD.artifact_id",
    },
    AuditedTable {
        name: "events",
        row_id_new: "NEW.id",
        row_id_old: "OLD.id",
    },
    AuditedTable {
        name: "artifact_link",
        row_id_new: "NEW.src_id || '→' || NEW.dst_id || ':' || NEW.rel",
        row_id_old: "OLD.src_id || '→' || OLD.dst_id || ':' || OLD.rel",
    },
    AuditedTable {
        name: "entry_cite",
        row_id_new: "NEW.src_slug || ':' || NEW.src_local || '→' || NEW.dst_ref",
        row_id_old: "OLD.src_slug || ':' || OLD.src_local || '→' || OLD.dst_ref",
    },
    AuditedTable {
        name: "commits",
        row_id_new: "NEW.hash",
        row_id_old: "OLD.hash",
    },
    AuditedTable {
        name: "worktree_registration",
        row_id_new: "NEW.worktree_root",
        row_id_old: "OLD.worktree_root",
    },
];

/// Epoch-ms UTC, computed SQL-side so foreign-writer rows get real times too.
const NOW_MS: &str = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)";

fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt
        .query_map([], |r| r.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    anyhow::ensure!(
        !cols.is_empty(),
        "audited table {table} not found — install() must run after migrations"
    );
    Ok(cols)
}

/// Chars above which an UPDATE diff stands a value in rather than copying it.
const CLAMP_CHARS: usize = 512;
/// Leading chars kept in a clamped stand-in — enough to say WHICH value it was.
const HEAD_CHARS: usize = 120;

/// A column value as it enters a payload.
///
/// The blob arm is not an optimisation and applies to BOTH payload kinds:
/// `json_object('c', X'DEADBEEF')` raises "JSON cannot hold BLOB values", and a
/// raising trigger aborts the WRITER's transaction — the caller's UPDATE fails,
/// not merely its audit row. SQLite is dynamically typed, so any writer can put
/// a BLOB in a TEXT column and nothing in the schema prevents it. It must be the
/// FIRST arm: `length()` is blob-safe, `json_object()` is not.
///
/// `clamp` is true only for UPDATE diffs. DELETE images stay verbatim because
/// the spec's payload-depth rule is "full OLD row on delete", and the bytes are
/// not there anyway — measured 2026-09-01 on a 27,914-row trail: 19 artifact
/// deletes averaged 740 chars, while 23 `artifact_augmentation` UPDATES held
/// 88% of the whole trail's payload bytes (avg 34KB, max 104KB), because an
/// update stores old AND new of a blob that is rewritten whole on every append.
///
/// No hash in the stand-in, deliberately: SQLite's bundled build has no hash
/// function, and registering one with `create_scalar_function` would make these
/// triggers raise for any FOREIGN connection that never registered it — the
/// same writer-abort failure the blob arm exists to prevent. Built-ins only.
/// A head is more useful than a hash regardless: a diff carries a column only
/// when it changed, so "did it change" is already answered, and what a reader
/// needs is which value it was.
fn value_expr(side: &str, col: &str, clamp: bool) -> String {
    let v = format!("{side}.\"{col}\"");
    let blob =
        format!("WHEN typeof({v}) = 'blob' THEN json_object('elided', 'blob', 'len', length({v}))");
    if !clamp {
        return format!("CASE {blob} ELSE {v} END");
    }
    format!(
        "CASE {blob} WHEN length({v}) > {CLAMP_CHARS} \
         THEN json_object('elided', 'oversize', 'len', length({v}), \
         'head', substr({v}, 1, {HEAD_CHARS})) ELSE {v} END"
    )
}

/// `OLD."c1" IS NOT NEW."c1" OR ...` — the UPDATE trigger's WHEN clause.
///
/// `IS NOT`, never `<>`: `<>` propagates NULL, so a NULL⇄value transition would
/// evaluate NULL (falsy) and the change would be silently dropped in BOTH
/// directions. Same operator the diff expression already uses, for the same
/// reason.
fn changed_predicate(cols: &[String]) -> String {
    cols.iter()
        .map(|c| format!("OLD.\"{c}\" IS NOT NEW.\"{c}\""))
        .collect::<Vec<_>>()
        .join(" OR ")
}

/// json_object('c1', OLD.c1, 'c2', OLD.c2, ...) — full row image, blob-safe.
fn old_image_expr(cols: &[String]) -> String {
    let pairs = cols
        .iter()
        .map(|c| format!("'{c}', {}", value_expr("OLD", c, false)))
        .collect::<Vec<_>>()
        .join(", ");
    format!("json_object({pairs})")
}

/// Changed-columns diff: {"col": [old, new], ...} via a json_patch fold.
fn update_diff_expr(cols: &[String]) -> String {
    let mut expr = String::from("'{}'");
    for c in cols {
        let (old, new) = (value_expr("OLD", c, true), value_expr("NEW", c, true));
        expr = format!(
            "json_patch({expr}, CASE WHEN OLD.\"{c}\" IS NOT NEW.\"{c}\" \
             THEN json_object('{c}', json_array({old}, {new})) ELSE '{{}}' END)"
        );
    }
    expr
}

/// Create the audit table (idempotent) and drop+recreate all capture triggers
/// from the LIVE schema. Called by every Catalog constructor AFTER
/// run_migrations: a table-copy migration silently drops the table's triggers
/// (memory catalog-sql-hazards), and the same open that ran the migration
/// reinstalls them here — no cross-open self-heal window. Drop+create (not
/// IF NOT EXISTS) so a definition change or column add always converges.
///
/// The whole body runs in one `BEGIN IMMEDIATE` / `COMMIT` transaction, same
/// pattern as `run_migrations` (bug 33e4ae68). Without it, each `DROP TRIGGER`
/// / `CREATE TRIGGER` pair is its own autocommit statement, and on a WAL
/// catalog shared by two codescout instances a foreign writer mutating an
/// audited table in the gap between the DROP and the CREATE goes unaudited —
/// silently: no row is written, so there is no seq gap to notice. One
/// `install()` call opens up to `AUDITED_TABLES.len() * 3` such windows.
/// Wrapping the whole thing in a transaction closes all of them: the DROPped
/// triggers and their CREATEd replacements become atomic from every other
/// connection's point of view (busy_timeout makes a concurrent writer block
/// on BEGIN IMMEDIATE rather than observe a half-updated trigger set).
pub(crate) fn install(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    match install_in_txn(conn) {
        Ok(()) => {
            conn.execute_batch("COMMIT")?;
            Ok(())
        }
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}

fn install_in_txn(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS catalog_audit (
           seq     INTEGER PRIMARY KEY AUTOINCREMENT,
           at_ms   INTEGER NOT NULL,
           tbl     TEXT NOT NULL,
           op      TEXT NOT NULL CHECK (op IN ('insert','update','delete')),
           row_id  TEXT NOT NULL,
           actor   TEXT NOT NULL DEFAULT 'unknown',
           verb    TEXT,
           payload TEXT
         );
         CREATE INDEX IF NOT EXISTS idx_audit_tbl_row ON catalog_audit(tbl, row_id, seq);
         CREATE INDEX IF NOT EXISTS idx_audit_at ON catalog_audit(at_ms);",
    )?;
    for t in AUDITED_TABLES {
        let cols = table_columns(conn, t.name)?;
        let (name, image, diff) = (t.name, old_image_expr(&cols), update_diff_expr(&cols));
        // An UPDATE that writes the values a row already holds still fires
        // AFTER UPDATE, and the diff then folds to a literal '{}'. Measured
        // 2026-09-01 before this guard existed: 27,505 of 27,914 rows (98.5%)
        // in a 1.74-hour window were exactly that, from reindex rewriting the
        // commits table. They carry nothing, and they dilute `seq`, whose gaps
        // this design names as its tamper signal.
        // Empty is unreachable (`table_columns` bails on a column-less table),
        // but an empty predicate would emit `WHEN BEGIN` — so fall back to no
        // WHEN at all, failing toward recording MORE rather than a broken open.
        let changed = changed_predicate(&cols);
        let when = if changed.is_empty() {
            String::new()
        } else {
            format!("WHEN {changed} ")
        };
        conn.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS audit_{name}_insert;
             CREATE TRIGGER audit_{name}_insert AFTER INSERT ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id)
               VALUES({NOW_MS}, '{name}', 'insert', {rid_new});
             END;
             DROP TRIGGER IF EXISTS audit_{name}_update;
             CREATE TRIGGER audit_{name}_update AFTER UPDATE ON \"{name}\" {when}BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
               VALUES({NOW_MS}, '{name}', 'update', {rid_new}, {diff});
             END;
             DROP TRIGGER IF EXISTS audit_{name}_delete;
             CREATE TRIGGER audit_{name}_delete AFTER DELETE ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
               VALUES({NOW_MS}, '{name}', 'delete', {rid_old}, {image});
             END;",
            rid_new = t.row_id_new,
            rid_old = t.row_id_old,
        ))?;
    }
    Ok(())
}

/// "codescout:<session-id>" for an identified connection, "codescout:anonymous"
/// for a codescout connection with no harness id. NEVER returns "unknown" —
/// 'unknown' is reserved for writers that did not identify themselves at all
/// (foreign processes), and the distinction is the forensic value.
pub(crate) fn resolve_actor() -> String {
    use crate::tools::session_key::{resolve, HARNESS_SESSION_VARS};
    let key = resolve(
        std::env::var("CODESCOUT_SESSION_ID").ok(),
        HARNESS_SESSION_VARS
            .iter()
            .filter_map(|v| std::env::var(v).ok().map(|val| (*v, val))),
    );
    match key.id() {
        Some(id) => format!("codescout:{id}"),
        None => "codescout:anonymous".to_string(),
    }
}

/// Per-connection identity: audit_ctx temp table + TEMP stamping trigger.
/// Temp objects are invisible to other connections; the trigger fires only for
/// this connection's inserts into catalog_audit (probe-validated, including
/// nested firing from inside the main capture triggers). Idempotent: replaces
/// both the row and the trigger.
pub(crate) fn install_session(conn: &Connection, actor: &str) -> Result<()> {
    conn.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS audit_ctx(actor TEXT NOT NULL, verb TEXT);
         DELETE FROM audit_ctx;",
    )?;
    conn.execute(
        "INSERT INTO audit_ctx(actor, verb) VALUES(?1, NULL)",
        [actor],
    )?;
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS audit_stamp;
         CREATE TEMP TRIGGER audit_stamp AFTER INSERT ON catalog_audit BEGIN
           UPDATE catalog_audit
              SET actor = COALESCE((SELECT actor FROM audit_ctx), 'unknown'),
                  verb  = (SELECT verb FROM audit_ctx)
            WHERE seq = NEW.seq;
         END;",
    )?;
    Ok(())
}

/// Query filters for `query()`. `Default::default()` matches everything.
#[derive(Default)]
pub(crate) struct AuditFilter {
    pub tbl: Option<String>,
    pub row_id: Option<String>,
    pub actor: Option<String>,
    pub op: Option<String>,
    pub since: Option<i64>,
    pub until: Option<i64>,
}

pub(crate) struct AuditRow {
    pub seq: i64,
    pub at_ms: i64,
    pub tbl: String,
    pub op: String,
    pub row_id: String,
    pub actor: String,
    pub verb: Option<String>,
    pub payload: Option<String>,
}

/// Builds the shared `WHERE 1=1 AND ...` clause + bound params for
/// `AuditFilter`, factored out so `query()` (which appends `ORDER BY ... LIMIT`)
/// and `count_matching()` (which wraps in `SELECT count(*)`) can never drift
/// apart on which rows a filter matches — Task review Finding B (2026-09-01)
/// depends on `filtered_total` reflecting exactly the same WHERE as `query()`.
fn filter_where(f: &AuditFilter) -> (String, Vec<Box<dyn rusqlite::ToSql>>) {
    let mut sql = String::from(" WHERE 1=1");
    let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(v) = &f.tbl {
        sql.push_str(" AND tbl = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &f.row_id {
        sql.push_str(" AND row_id = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &f.actor {
        sql.push_str(" AND actor = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = &f.op {
        sql.push_str(" AND op = ?");
        params.push(Box::new(v.clone()));
    }
    if let Some(v) = f.since {
        sql.push_str(" AND at_ms >= ?");
        params.push(Box::new(v));
    }
    if let Some(v) = f.until {
        sql.push_str(" AND at_ms <= ?");
        params.push(Box::new(v));
    }
    (sql, params)
}

/// Dynamic filter SQL: same pattern as `events::timeline_for_artifact`.
/// Newest-first (`ORDER BY seq DESC`) so a capped `limit` always returns the
/// most recent activity rather than the oldest.
pub(crate) fn query(conn: &Connection, f: &AuditFilter, limit: usize) -> Result<Vec<AuditRow>> {
    let (where_sql, mut params) = filter_where(f);
    let mut sql =
        String::from("SELECT seq, at_ms, tbl, op, row_id, actor, verb, payload FROM catalog_audit");
    sql.push_str(&where_sql);
    sql.push_str(" ORDER BY seq DESC LIMIT ?");
    params.push(Box::new(limit as i64));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |r| {
            Ok(AuditRow {
                seq: r.get(0)?,
                at_ms: r.get(1)?,
                tbl: r.get(2)?,
                op: r.get(3)?,
                row_id: r.get(4)?,
                actor: r.get(5)?,
                verb: r.get(6)?,
                payload: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Count of rows matching `f`, with no `LIMIT` — the denominator behind
/// `audit_log`'s `filtered_total`/`truncated` (Task review Finding B,
/// 2026-09-01), built from the exact same WHERE as `query()` via
/// `filter_where` so the two can never disagree on which rows match.
pub(crate) fn count_matching(conn: &Connection, f: &AuditFilter) -> Result<i64> {
    let (where_sql, params) = filter_where(f);
    let sql = format!("SELECT count(*) FROM catalog_audit{where_sql}");
    conn.query_row(&sql, rusqlite::params_from_iter(params.iter()), |r| {
        r.get(0)
    })
    .map_err(Into::into)
}

/// Deletes audit rows older than the cutoff, then writes ONE marker row
/// describing the prune — so the resulting seq gap explains itself instead of
/// reading as tampering.
pub(crate) fn prune_before(conn: &Connection, before_ms: i64) -> Result<usize> {
    let n = conn.execute("DELETE FROM catalog_audit WHERE at_ms < ?1", [before_ms])?;
    if n > 0 {
        conn.execute(
            &format!(
                "INSERT INTO catalog_audit(at_ms, tbl, op, row_id, payload)
                 VALUES({NOW_MS}, 'catalog_audit', 'delete', 'prune',
                        json_object('pruned', ?1, 'before_ms', ?2))"
            ),
            rusqlite::params![n as i64, before_ms],
        )?;
    }
    Ok(n)
}

/// Rolls up audit-trail size, time span and unknown-actor count for
/// `doctor`'s `catalog_health` block. `unknown_actor_rows` surfaces writers
/// (foreign processes, direct SQL) that never identified themselves — see
/// `resolve_actor` for how an identified writer sets `actor` instead.
///
/// Byte fields exist because a row COUNT under-communicates the cost: measured
/// 2026-09-01, 23 rows out of 27,914 (0.08%) carried 88% of the payload bytes.
/// `largest_payload_bytes` is what makes that concentration visible at all — a
/// total alone reads as uniform growth. `CAST(payload AS BLOB)` because
/// `length()` on TEXT counts CHARACTERS, not bytes (memory catalog-sql-hazards).
pub(crate) fn health(conn: &Connection) -> Result<serde_json::Value> {
    let (rows, min_ms, max_ms, unknown, bytes, largest): (
        i64,
        Option<i64>,
        Option<i64>,
        i64,
        Option<i64>,
        Option<i64>,
    ) = conn.query_row(
        "SELECT count(*), min(at_ms), max(at_ms),
                count(*) FILTER (WHERE actor = 'unknown'),
                sum(length(CAST(payload AS BLOB))),
                max(length(CAST(payload AS BLOB)))
         FROM catalog_audit",
        [],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        },
    )?;
    Ok(serde_json::json!({
        "rows": rows,
        "span_ms": min_ms.zip(max_ms).map(|(a, b)| serde_json::json!([a, b])),
        "unknown_actor_rows": unknown,
        "payload_bytes": bytes.unwrap_or(0),
        "largest_payload_bytes": largest.unwrap_or(0),
        "hint": "unknown_actor_rows counts writers that did not identify themselves (foreign processes). Query with librarian(action=\"audit_log\", actor=\"unknown\")."
    }))
}

#[cfg(test)]
mod tests {
    use crate::librarian::catalog::{artifact, Catalog};

    fn seed(cat: &Catalog, id: &str) {
        let row = artifact::TestArtifactRowBuilder::new(id)
            .with_status("draft")
            .build();
        artifact::upsert(cat, &row).unwrap();
    }

    fn audit_rows(cat: &Catalog) -> Vec<(String, String, String, String, Option<String>)> {
        let mut stmt = cat
            .conn
            .prepare("SELECT tbl, op, row_id, actor, payload FROM catalog_audit ORDER BY seq")
            .unwrap();
        stmt.query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap()
    }

    #[test]
    fn insert_update_delete_on_artifact_each_leave_an_audit_row() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn
            .execute("UPDATE artifact SET status='archived' WHERE id='a1'", [])
            .unwrap();
        cat.conn
            .execute("DELETE FROM artifact WHERE id='a1'", [])
            .unwrap();
        let rows = audit_rows(&cat);
        assert_eq!(rows.len(), 3, "one audit row per mutation, got {rows:?}");
        assert_eq!(
            (rows[0].0.as_str(), rows[0].1.as_str()),
            ("artifact", "insert")
        );
        assert_eq!(rows[0].2, "a1");
        assert!(rows[0].4.is_none(), "insert carries no payload");
        // update payload: changed columns only, as {"col": [old, new]}
        let diff: serde_json::Value = serde_json::from_str(rows[1].4.as_deref().unwrap()).unwrap();
        assert_eq!(diff["status"], serde_json::json!(["draft", "archived"]));
        assert!(
            diff.get("title").is_none(),
            "unchanged column must be absent: {diff}"
        );
        // delete payload: full OLD image
        let img: serde_json::Value = serde_json::from_str(rows[2].4.as_deref().unwrap()).unwrap();
        assert_eq!(img["id"], "a1");
        assert!(
            img.get("abs_path").is_some(),
            "full image carries every column"
        );
    }

    // Per guarded SITE, not per feature (CLAUDE.md Testing Discipline): every audited
    // table proves its own delete trigger fires with a payload.
    #[test]
    fn every_audited_table_captures_a_delete_with_old_image() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn
            .execute("UPDATE artifact SET slug='a-one' WHERE id='a1'", [])
            .unwrap();
        // one row per table, minimal columns
        cat.conn.execute("INSERT INTO artifact_link(src_id,dst_id,rel,created_at) VALUES('a1','a1','cites',1)", []).unwrap();
        cat.conn.execute("INSERT INTO events(id,artifact_id,kind,payload,created_at) VALUES('e1','a1','note','{}',1)", []).unwrap();
        cat.conn
            .execute(
                "INSERT INTO artifact_augmentation(artifact_id,prompt) VALUES('a1','p')",
                [],
            )
            .unwrap();
        cat.conn.execute("INSERT INTO entry_cite(src_slug,src_local,dst_ref,rel,created_at) VALUES('a-one','F-1','x','cites',1)", []).unwrap();
        cat.conn
            .execute("INSERT INTO commits(hash,git_root) VALUES('h1','/r')", [])
            .unwrap();
        cat.conn.execute("INSERT INTO worktree_registration(worktree_root,main_root,created_at) VALUES('/w','/m',1)", []).unwrap();
        cat.conn.execute("PRAGMA foreign_keys=OFF", []).unwrap(); // isolate per-table deletes from cascades
        for t in super::AUDITED_TABLES {
            cat.conn
                .execute(&format!("DELETE FROM {}", t.name), [])
                .unwrap();
            let n: i64 = cat
                .conn
                .query_row(
                    "SELECT count(*) FROM catalog_audit WHERE tbl=?1 AND op='delete' AND payload IS NOT NULL",
                    [t.name],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(
                n >= 1,
                "no delete audit row with payload for table {}",
                t.name
            );
        }
    }

    // Deliberate break (CLAUDE.md Testing Discipline): the rows come from the
    // triggers and nowhere else — drop them and the silence proves it.
    #[test]
    fn dropping_the_triggers_stops_capture() {
        let cat = Catalog::open_in_memory().unwrap();
        let names: Vec<String> = {
            let mut s = cat
                .conn
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type='trigger' AND name LIKE 'audit_%'",
                )
                .unwrap();
            let v = s
                .query_map([], |r| r.get(0))
                .unwrap()
                .collect::<rusqlite::Result<Vec<String>>>()
                .unwrap();
            v
        };
        assert!(
            !names.is_empty(),
            "install() must have created audit_ triggers"
        );
        for n in &names {
            cat.conn.execute(&format!("DROP TRIGGER {n}"), []).unwrap();
        }
        seed(&cat, "a2");
        let n: i64 = cat
            .conn
            .query_row("SELECT count(*) FROM catalog_audit", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            n, 0,
            "with triggers dropped, no capture — proves triggers are the mechanism"
        );
    }

    // Table-copy migrations drop a table's triggers with the table
    // (memory catalog-sql-hazards); every open reconverges the set.
    #[test]
    fn reopen_reinstalls_triggers_after_a_manual_drop() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.db");
        {
            let cat = Catalog::open(&db).unwrap();
            cat.conn
                .execute("DROP TRIGGER audit_artifact_delete", [])
                .unwrap();
        }
        let cat = Catalog::open(&db).unwrap();
        seed(&cat, "a3");
        cat.conn
            .execute("DELETE FROM artifact WHERE id='a3'", [])
            .unwrap();
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM catalog_audit WHERE tbl='artifact' AND op='delete'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "reopen must have reinstalled the dropped trigger");
    }

    #[test]
    fn audit_rows_survive_artifact_deletion_no_fk() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "gone");
        cat.conn
            .execute("DELETE FROM artifact WHERE id='gone'", [])
            .unwrap();
        let n: i64 = cat
            .conn
            .query_row(
                "SELECT count(*) FROM catalog_audit WHERE row_id='gone'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 2, "insert+delete rows outlive the artifact row");
    }

    #[test]
    fn codescout_connection_rows_are_stamped_with_the_session_actor() {
        let cat = Catalog::open_in_memory().unwrap();
        // Deterministic: re-seed the session with a known actor (idempotent —
        // install_session replaces the audit_ctx row and the temp trigger).
        super::install_session(&cat.conn, "codescout:test-session").unwrap();
        seed(&cat, "s1");
        let actor: String = cat
            .conn
            .query_row(
                "SELECT actor FROM catalog_audit ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actor, "codescout:test-session");
    }

    // THE vanished-rows reproduction (docs/issues/2026-08-25-sdd-ledger-and-
    // catalog-rows-vanished.md): a raw second connection deletes a row; the trail
    // answers with op, full OLD image, and actor 'unknown'.
    #[test]
    fn foreign_connection_mutations_record_as_unknown_with_full_image() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.db");
        let cat = Catalog::open(&db).unwrap();
        seed(&cat, "victim");
        let foreign = rusqlite::Connection::open(&db).unwrap(); // no install_session
        foreign.pragma_update(None, "busy_timeout", 5000).unwrap();
        foreign
            .execute("DELETE FROM artifact WHERE id='victim'", [])
            .unwrap();
        let (actor, payload): (String, String) = cat
            .conn
            .query_row(
                "SELECT actor, payload FROM catalog_audit WHERE op='delete' AND row_id='victim'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            actor, "unknown",
            "a writer that did not identify itself is 'unknown' — that IS the answer"
        );
        let img: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(
            img["id"], "victim",
            "the deleted row is reconstructible from the trail alone"
        );
    }

    // Deliberate break for the stamping path: without the temp trigger the system
    // degrades to 'unknown' — never blocks, never mis-attributes (probe-pinned
    // failure direction; see module doc).
    #[test]
    fn dropping_the_stamp_trigger_degrades_to_unknown_not_to_an_error() {
        let cat = Catalog::open_in_memory().unwrap();
        cat.conn.execute("DROP TRIGGER audit_stamp", []).unwrap();
        seed(&cat, "u1");
        let actor: String = cat
            .conn
            .query_row(
                "SELECT actor FROM catalog_audit ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(actor, "unknown");
    }

    #[test]
    fn set_audit_verb_reaches_subsequent_rows() {
        let cat = Catalog::open_in_memory().unwrap();
        cat.set_audit_verb("artifact.update").unwrap();
        seed(&cat, "v1");
        let verb: Option<String> = cat
            .conn
            .query_row(
                "SELECT verb FROM catalog_audit ORDER BY seq DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(verb.as_deref(), Some("artifact.update"));
    }

    #[test]
    fn query_filters_compose_and_order_newest_first() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "q1");
        cat.conn
            .execute("DELETE FROM artifact WHERE id='q1'", [])
            .unwrap();
        let all = super::query(&cat.conn, &super::AuditFilter::default(), 50).unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].seq > all[1].seq, "newest first");
        let f = super::AuditFilter {
            op: Some("delete".into()),
            ..Default::default()
        };
        let dels = super::query(&cat.conn, &f, 50).unwrap();
        assert_eq!(dels.len(), 1);
        assert_eq!(dels[0].row_id, "q1");
    }

    #[test]
    fn prune_deletes_old_rows_and_leaves_a_self_describing_marker() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "p1");
        let removed = super::prune_before(&cat.conn, i64::MAX).unwrap();
        assert_eq!(removed, 1);
        let rows = super::query(&cat.conn, &super::AuditFilter::default(), 50).unwrap();
        // the marker row explains the seq gap: tbl catalog_audit, op delete
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tbl, "catalog_audit");
        assert_eq!(rows[0].op, "delete");
        assert_eq!(rows[0].row_id, "prune");
        let p: serde_json::Value =
            serde_json::from_str(rows[0].payload.as_deref().unwrap()).unwrap();
        assert_eq!(p["pruned"], 1);
        assert_eq!(p["before_ms"], i64::MAX);
    }

    #[test]
    fn health_counts_rows_and_unknown_actors() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "h1"); // stamped by this connection's session actor
        cat.conn
            .execute(
                "INSERT INTO catalog_audit(at_ms,tbl,op,row_id) VALUES(5,'artifact','insert','h2')",
                [],
            )
            .unwrap(); // direct insert: stamped too (same conn) — so force one unknown:
        cat.conn
            .execute(
                "UPDATE catalog_audit SET actor='unknown' WHERE row_id='h2'",
                [],
            )
            .unwrap();
        let h = super::health(&cat.conn).unwrap();
        assert!(h["rows"].as_i64().unwrap() >= 2);
        assert_eq!(h["unknown_actor_rows"], 1);
        assert!(h["span_ms"].is_array());
    }

    // ---- The WHEN guard: an UPDATE that moves nothing must record nothing ----
    //
    // These two are a PAIR and must stay adjacent. The suite's other update
    // assertions are monotone under OVER-recording — a trigger that fires on
    // every statement satisfies every one of them, which is how 27,505 empty
    // rows (98.5% of the live trail) passed a green suite for a day. The first
    // test below can only fail if the guard is missing; the second can only
    // fail if the guard is too aggressive. Neither direction is covered by the
    // other.

    #[test]
    fn an_update_that_changes_nothing_writes_no_audit_row() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let before = audit_rows(&cat).len();
        // Writes the value the row already holds. SQLite reports 1 row
        // "changed" and fires AFTER UPDATE; nothing actually moved.
        let n = cat
            .conn
            .execute("UPDATE artifact SET status='draft' WHERE id='a1'", [])
            .unwrap();
        assert_eq!(n, 1, "the UPDATE must really run, or this proves nothing");
        assert_eq!(
            audit_rows(&cat).len(),
            before,
            "an UPDATE that changes no column must leave no audit row"
        );
    }

    #[test]
    fn an_update_that_changes_one_column_still_writes_a_row() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let before = audit_rows(&cat).len();
        cat.conn
            .execute("UPDATE artifact SET status='active' WHERE id='a1'", [])
            .unwrap();
        let rows = audit_rows(&cat);
        assert_eq!(rows.len(), before + 1, "a real change must still record");
        let diff: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        assert_eq!(diff["status"], serde_json::json!(["draft", "active"]));
    }

    #[test]
    fn a_null_to_value_transition_counts_as_a_change() {
        // Discriminates `IS NOT` from `<>` in the WHEN predicate: `<>` is
        // NULL-propagating, so `OLD.missing_since <> NEW.missing_since`
        // evaluates to NULL — falsy — and the row would be silently skipped in
        // BOTH directions. missing_since is NULL on a fresh artifact row.
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let before = audit_rows(&cat).len();
        cat.conn
            .execute("UPDATE artifact SET missing_since=123 WHERE id='a1'", [])
            .unwrap();
        cat.conn
            .execute("UPDATE artifact SET missing_since=NULL WHERE id='a1'", [])
            .unwrap();
        let rows = audit_rows(&cat);
        assert_eq!(
            rows.len(),
            before + 2,
            "NULL→value and value→NULL are both changes, got {rows:?}"
        );
        let out: serde_json::Value =
            serde_json::from_str(rows[rows.len() - 2].4.as_deref().unwrap()).unwrap();
        assert_eq!(out["missing_since"], serde_json::json!([null, 123]));
        let back: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        assert_eq!(back["missing_since"], serde_json::json!([123, null]));
    }

    // ---- Payload clamping: the other measured pair ----

    #[test]
    fn an_oversize_update_value_is_elided_with_its_length_and_head() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let big = "x".repeat(2000);
        cat.conn
            .execute("UPDATE artifact SET title=?1 WHERE id='a1'", [&big])
            .unwrap();
        let rows = audit_rows(&cat);
        let diff: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        let new = &diff["title"][1];
        assert_eq!(new["elided"], "oversize", "got {diff}");
        assert_eq!(new["len"], 2000);
        assert_eq!(
            new["head"].as_str().unwrap().len(),
            120,
            "the head is what identifies WHICH value it was"
        );
        assert!(
            rows.last().unwrap().4.as_deref().unwrap().len() < 600,
            "the whole payload must be bounded, not just the field"
        );
    }

    #[test]
    fn a_small_update_value_is_recorded_verbatim() {
        // Pair of the above: proves the clamp is not swallowing ordinary values.
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn
            .execute("UPDATE artifact SET title='short title' WHERE id='a1'", [])
            .unwrap();
        let rows = audit_rows(&cat);
        let diff: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        assert_eq!(diff["title"][1], serde_json::json!("short title"));
    }

    #[test]
    fn a_delete_image_keeps_oversize_values_verbatim() {
        // DELETE images are NOT clamped — the spec's payload-depth rule is
        // "full OLD row on delete", deletes are rare, and each stores ONE copy.
        // Measured 2026-09-01: 19 artifact deletes averaged 740 chars, while 23
        // augmentation UPDATES held 88% of the trail's bytes. Clamping the
        // wrong one would cost evidence and save nothing.
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        let big = "y".repeat(2000);
        cat.conn
            .execute("UPDATE artifact SET title=?1 WHERE id='a1'", [&big])
            .unwrap();
        cat.conn
            .execute("DELETE FROM artifact WHERE id='a1'", [])
            .unwrap();
        let rows = audit_rows(&cat);
        let img: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        assert_eq!(img["title"], serde_json::json!(big));
    }

    #[test]
    fn a_blob_value_does_not_abort_the_writer() {
        // Deliberate break for the blob half of
        // docs/issues/archive/2026-09-01-audit-trigger-can-abort-writer-on-null-key-or-blob.md.
        // Probed 2026-09-01: `json_object('c', X'DEADBEEF')` raises "JSON cannot
        // hold BLOB values", and a raising trigger aborts the WRITER's
        // transaction — the artifact write fails, not merely its audit row.
        // SQLite is dynamically typed, so any writer can put a BLOB in a TEXT
        // column; nothing in the schema prevents it.
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn
            .execute("UPDATE artifact SET title=X'DEADBEEF' WHERE id='a1'", [])
            .expect("a BLOB in an audited column must not fail the writer's UPDATE");
        let rows = audit_rows(&cat);
        let diff: serde_json::Value =
            serde_json::from_str(rows.last().unwrap().4.as_deref().unwrap()).unwrap();
        assert_eq!(diff["title"][1]["elided"], "blob", "got {diff}");
        assert_eq!(diff["title"][1]["len"], 4);
        cat.conn
            .execute("DELETE FROM artifact WHERE id='a1'", [])
            .expect("and the delete image must not fail either");
    }

    #[test]
    fn health_reports_payload_bytes() {
        let cat = Catalog::open_in_memory().unwrap();
        seed(&cat, "a1");
        cat.conn
            .execute("UPDATE artifact SET status='active' WHERE id='a1'", [])
            .unwrap();
        let h = super::health(&cat.conn).unwrap();
        let total = h["payload_bytes"].as_i64().expect("payload_bytes present");
        let largest = h["largest_payload_bytes"]
            .as_i64()
            .expect("largest_payload_bytes present");
        assert!(total > 0, "the update payload has bytes: {h}");
        assert!(
            largest > 0 && largest <= total,
            "largest must be a real row's size, bounded by the total: {h}"
        );
    }
}
