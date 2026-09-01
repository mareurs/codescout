//! Append-only catalog audit trail (T-1, spec: docs/superpowers/specs/
//! 2026-09-01-catalog-audit-trail-design.md + this plan's Design Correction).
//!
//! Main-schema triggers capture every writer's mutations with actor DEFAULT
//! 'unknown'; a per-connection TEMP trigger (install_session, Task 2) enriches
//! this connection's rows. NEVER reference temp objects from the main triggers:
//! probed 2026-09-01 — it creates silently, then REFUSES foreign writers'
//! mutations at fire time ("no such table: main.audit_ctx").

use anyhow::Result;
use rusqlite::Connection;

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

/// json_object('c1', OLD.c1, 'c2', OLD.c2, ...) — full row image.
fn old_image_expr(cols: &[String]) -> String {
    let pairs = cols
        .iter()
        .map(|c| format!("'{c}', OLD.\"{c}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("json_object({pairs})")
}

/// Changed-columns diff: {"col": [old, new], ...} via a json_patch fold.
fn update_diff_expr(cols: &[String]) -> String {
    let mut expr = String::from("'{}'");
    for c in cols {
        expr = format!(
            "json_patch({expr}, CASE WHEN OLD.\"{c}\" IS NOT NEW.\"{c}\" \
             THEN json_object('{c}', json_array(OLD.\"{c}\", NEW.\"{c}\")) ELSE '{{}}' END)"
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
        conn.execute_batch(&format!(
            "DROP TRIGGER IF EXISTS audit_{name}_insert;
             CREATE TRIGGER audit_{name}_insert AFTER INSERT ON \"{name}\" BEGIN
               INSERT INTO catalog_audit(at_ms, tbl, op, row_id)
               VALUES({NOW_MS}, '{name}', 'insert', {rid_new});
             END;
             DROP TRIGGER IF EXISTS audit_{name}_update;
             CREATE TRIGGER audit_{name}_update AFTER UPDATE ON \"{name}\" BEGIN
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
pub(crate) fn health(conn: &Connection) -> Result<serde_json::Value> {
    let (rows, min_ms, max_ms, unknown): (i64, Option<i64>, Option<i64>, i64) = conn.query_row(
        "SELECT count(*), min(at_ms), max(at_ms),
                count(*) FILTER (WHERE actor = 'unknown')
         FROM catalog_audit",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    Ok(serde_json::json!({
        "rows": rows,
        "span_ms": min_ms.zip(max_ms).map(|(a, b)| serde_json::json!([a, b])),
        "unknown_actor_rows": unknown,
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
}
