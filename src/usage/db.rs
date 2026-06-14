use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

pub fn open_db(project_root: &Path) -> Result<Connection> {
    let path = project_root.join(".codescout").join("usage.db");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(&path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS tool_calls (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            tool_name  TEXT NOT NULL,
            called_at  TEXT NOT NULL DEFAULT (datetime('now')),
            latency_ms INTEGER NOT NULL,
            outcome    TEXT NOT NULL,
            overflowed INTEGER NOT NULL DEFAULT 0,
            error_msg  TEXT
        );

        CREATE TABLE IF NOT EXISTS lsp_events (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            language          TEXT NOT NULL,
            started_at        TEXT NOT NULL DEFAULT (datetime('now')),
            reason            TEXT NOT NULL,
            handshake_ms      INTEGER NOT NULL,
            first_response_ms INTEGER
        );

        CREATE TABLE IF NOT EXISTS call_edges (
            project_id   TEXT NOT NULL,
            caller_sym   TEXT NOT NULL,
            callee_sym   TEXT NOT NULL,
            file         TEXT NOT NULL,
            line         INTEGER NOT NULL,
            col          INTEGER NOT NULL,
            source       TEXT NOT NULL,
            computed_at  INTEGER NOT NULL,
            PRIMARY KEY (project_id, caller_sym, callee_sym, file, line, col)
        );
        CREATE INDEX IF NOT EXISTS call_edges_caller ON call_edges(project_id, caller_sym);
        CREATE INDEX IF NOT EXISTS call_edges_callee ON call_edges(project_id, callee_sym);
        CREATE INDEX IF NOT EXISTS call_edges_file   ON call_edges(project_id, file);",
    )?;

    // Migration: add traceability columns (v0.9)
    let has_session_id: bool = conn
        .prepare("SELECT session_id FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_session_id {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN codescout_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN project_sha TEXT;
             ALTER TABLE tool_calls ADD COLUMN session_id TEXT;
             ALTER TABLE tool_calls ADD COLUMN input_json TEXT;
             ALTER TABLE tool_calls ADD COLUMN output_json TEXT;",
        )?;
    }

    // Migration: add CC session link (v0.10)
    let has_cc_session_id: bool = conn
        .prepare("SELECT cc_session_id FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_cc_session_id {
        conn.execute_batch("ALTER TABLE tool_calls ADD COLUMN cc_session_id TEXT;")?;
    }

    // Migration: record failed LSP starts, not just completed handshakes.
    // Without this, a server that dies during `initialize` (e.g. an expired LSP
    // build) leaves zero lsp_events rows — a chronically-failing LSP is invisible
    // to usage analytics. `outcome` defaults to 'success' so the unchanged
    // `write_lsp_event` INSERT and every pre-existing row stay correct.
    let has_lsp_outcome: bool = conn
        .prepare("SELECT outcome FROM lsp_events LIMIT 0")
        .is_ok();
    if !has_lsp_outcome {
        conn.execute_batch(
            "ALTER TABLE lsp_events ADD COLUMN outcome TEXT NOT NULL DEFAULT 'success';
             ALTER TABLE lsp_events ADD COLUMN error TEXT;",
        )?;
    }

    // Migration: legibility friction fields (v0.11). Additive + nullable so every
    // pre-existing row and the unchanged INSERTs stay correct.
    let has_friction_target: bool = conn
        .prepare("SELECT friction_target FROM tool_calls LIMIT 0")
        .is_ok();
    if !has_friction_target {
        conn.execute_batch(
            "ALTER TABLE tool_calls ADD COLUMN friction_target TEXT;
             ALTER TABLE tool_calls ADD COLUMN overflow_tokens INTEGER;
             ALTER TABLE tool_calls ADD COLUMN err_family TEXT;
             ALTER TABLE tool_calls ADD COLUMN project_root TEXT;",
        )?;
    }

    backfill_legacy_rows(&conn, &project_root.to_string_lossy())?;

    Ok(conn)
}

#[allow(clippy::too_many_arguments)]
pub fn write_record(
    conn: &Connection,
    tool_name: &str,
    latency_ms: i64,
    outcome: &str,
    overflowed: bool,
    error_msg: Option<&str>,
    codescout_sha: &str,
    project_sha: Option<&str>,
    session_id: &str,
    input_json: Option<&str>,
    output_json: Option<&str>,
    cc_session_id: Option<&str>,
    friction_target: Option<&str>,
    overflow_tokens: Option<i64>,
    err_family: Option<&str>,
    project_root: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed, error_msg, codescout_sha, project_sha, session_id, input_json, output_json, cc_session_id, friction_target, overflow_tokens, err_family, project_root)
         VALUES (?1, datetime('now'), ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            tool_name,
            latency_ms,
            outcome,
            overflowed as i64,
            error_msg,
            codescout_sha,
            project_sha,
            session_id,
            input_json,
            output_json,
            cc_session_id,
            friction_target,
            overflow_tokens,
            err_family,
            project_root,
        ],
    )?;
    conn.execute(
        "DELETE FROM tool_calls WHERE called_at < datetime('now', '-30 days')",
        [],
    )?;
    Ok(())
}

/// Map an error message to a stable, low-cardinality family tag for the probe.
/// Order matters: more specific patterns first. `None` for unrecognized messages.
///
/// Lives here (not in the parent module) so the one-time backfill in `open_db`
/// can re-classify historical `error_msg` values with the same logic
/// `write_content` applies to new rows.
pub(crate) fn normalize_err_family(msg: &str) -> Option<&'static str> {
    // infra / tool-class (excluded from the probe's code-class score)
    if msg.contains("index is locked") {
        return Some("lsp_index_locked");
    }
    if msg.contains("Failed to spawn mux") || msg.contains("mux startup failed") {
        return Some("mux_startup_fail");
    }
    if msg.contains("LSP server is not running") {
        return Some("lsp_not_running");
    }
    if msg.contains("LSP server disconnected") {
        return Some("lsp_disconnect");
    }
    // iron-law routing / wrong-tool class — the agent reached for the wrong tool
    // and the server gate rejected + re-routed it. These dominate the real error
    // population; the original taxonomy missed them all, leaving err_family NULL
    // on ~90% of errors even on fresh rows.
    if msg.contains("overlaps named symbol") {
        return Some("il1_read_overlaps_symbol");
    }
    if msg.contains("Use read_markdown") {
        return Some("il4_read_markdown_routing");
    }
    if msg.contains("Use edit_markdown") {
        return Some("il5_edit_markdown_routing");
    }
    if msg.contains("contains a symbol definition")
        || msg.contains("is blocked for structural edits")
    {
        return Some("il2_structural_edit");
    }
    if msg.contains("shell access to source files is blocked") {
        return Some("il3_shell_on_source");
    }
    if msg.contains("IL3 violation") {
        return Some("il3_pipe_to_trimmer");
    }
    // security / scope class
    if msg.contains("write denied") {
        return Some("write_scope_denied");
    }
    // input-shape / extractor class
    if msg.contains("unsupported json_path") {
        return Some("json_path_unsupported");
    }
    if msg.contains("old_string not found") {
        return Some("edit_stale_match");
    }
    // code / extractor-shape class
    if msg.contains("AST parse failed") || msg.contains("cannot determine end of") {
        return Some("ast_extent_fail");
    }
    if msg.contains("ambiguous name_path") {
        return Some("ambiguous_name_path");
    }
    if msg.contains("dropped sibling") || msg.contains("dropped the symbol") {
        return Some("replace_dropped_sibling");
    }
    if msg.contains("symbol not found") {
        return Some("symbol_not_found");
    }
    None
}

/// Bump to force a one-time re-run of [`backfill_legacy_rows`] on the next open
/// (e.g. after the [`normalize_err_family`] taxonomy is extended). Tracked via
/// SQLite's `PRAGMA user_version`.
const BACKFILL_VERSION: i64 = 1;

/// One-time, idempotent repair of rows written before the friction columns were
/// populated. Gated on `PRAGMA user_version` so it runs once per DB and is a
/// cheap no-op (one pragma read) on every subsequent open.
///
/// Two columns are reconstructable from data still on the row:
/// - `project_root`: every row in a given `usage.db` belongs to that file's
///   project (the DB lives at `<root>/.codescout/usage.db`), so a blanket fill
///   of the NULLs is correct.
/// - `err_family`: `error_msg` is retained on every error row, so re-running the
///   classifier recovers the family. Only NULL families are touched — to re-map
///   an already-classified family after a taxonomy change, clear it first.
///
/// `friction_target` and `overflow_tokens` are NOT backfillable: their source
/// (the call's input / buffered output) is only persisted in debug mode, so old
/// rows can't be reconstructed. They self-heal as pre-migration rows age out
/// under the 30-day retention sweep in `write_record`.
fn backfill_legacy_rows(conn: &Connection, project_root: &str) -> Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current >= BACKFILL_VERSION {
        return Ok(());
    }

    conn.execute(
        "UPDATE tool_calls SET project_root = ?1 WHERE project_root IS NULL",
        params![project_root],
    )?;

    let unclassified: Vec<(i64, String)> = {
        let mut stmt = conn.prepare(
            "SELECT id, error_msg FROM tool_calls \
             WHERE err_family IS NULL AND error_msg IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<std::result::Result<_, _>>()?
    };
    for (id, msg) in unclassified {
        if let Some(family) = normalize_err_family(&msg) {
            conn.execute(
                "UPDATE tool_calls SET err_family = ?1 WHERE id = ?2",
                params![family, id],
            )?;
        }
    }

    conn.execute_batch(&format!("PRAGMA user_version = {BACKFILL_VERSION};"))?;
    Ok(())
}

/// Record an LSP cold-start event. Returns the inserted row id for the
/// two-phase write (first_response_ms is filled in later by `update_lsp_first_response`).
pub fn write_lsp_event(
    conn: &Connection,
    language: &str,
    reason: &str,
    handshake_ms: i64,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO lsp_events (language, reason, handshake_ms) VALUES (?1, ?2, ?3)",
        params![language, reason, handshake_ms],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Record a *failed* LSP start: the server disconnected or errored during the
/// `initialize` handshake, so no session was established. `handshake_ms` is the
/// time elapsed until the failure. Recorded as a separate `outcome='failed'`
/// row so a chronically-failing server (e.g. an expired LSP build) is visible
/// in lsp_events rather than as a silent absence of `success` rows.
pub fn write_lsp_failure(
    conn: &Connection,
    language: &str,
    reason: &str,
    handshake_ms: i64,
    error: &str,
) -> Result<i64> {
    conn.execute(
        "INSERT INTO lsp_events (language, reason, handshake_ms, outcome, error)
         VALUES (?1, ?2, ?3, 'failed', ?4)",
        params![language, reason, handshake_ms, error],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Fill in the first_response_ms for a previously inserted lsp_events row.
/// Best-effort — if the row was already updated or is missing, this is a no-op.
pub fn update_lsp_first_response(
    conn: &Connection,
    rowid: i64,
    first_response_ms: i64,
) -> Result<()> {
    conn.execute(
        "UPDATE lsp_events SET first_response_ms = ?1 WHERE id = ?2 AND first_response_ms IS NULL",
        params![first_response_ms, rowid],
    )?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct ToolStats {
    pub tool: String,
    pub calls: i64,
    pub errors: i64,
    pub error_rate_pct: f64,
    pub overflows: i64,
    pub overflow_rate_pct: f64,
    pub p50_ms: i64,
    pub p99_ms: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct UsageStats {
    pub window: String,
    pub total_calls: i64,
    pub by_tool: Vec<ToolStats>,
}

#[derive(Debug, Default, serde::Serialize)]
pub struct LspReasonCounts {
    pub new_session: i64,
    pub idle_evicted: i64,
    pub lru_evicted: i64,
    pub crashed: i64,
}

#[derive(Debug, serde::Serialize)]
pub struct LspLanguageStats {
    pub language: String,
    pub starts: i64,
    pub failures: i64,
    pub reasons: LspReasonCounts,
    pub avg_handshake_ms: i64,
    pub p95_handshake_ms: i64,
    pub avg_first_response_ms: Option<i64>,
    pub p95_first_response_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspEvent {
    pub language: String,
    pub started_at: String,
    pub reason: String,
    pub handshake_ms: i64,
    pub first_response_ms: Option<i64>,
}

/// A failed LSP start (server died during `initialize`). `error` is the
/// caller-facing message (e.g. "LSP server disconnected").
#[derive(Debug, serde::Serialize)]
pub struct LspFailure {
    pub language: String,
    pub started_at: String,
    pub reason: String,
    pub error: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct LspStats {
    pub window: String,
    pub by_language: Vec<LspLanguageStats>,
    pub recent: Vec<LspEvent>,
    pub recent_failures: Vec<LspFailure>,
}

pub fn query_lsp_stats(conn: &Connection, window: &str) -> Result<LspStats> {
    let modifier = window_to_modifier(window);

    // Aggregate per language. `starts` and the handshake metrics count only
    // successful starts; `failures` counts starts that died during `initialize`
    // (e.g. an expired LSP build). A language that fails *every* start still
    // appears here with starts=0, failures>0 — the case we most want visible.
    let mut agg_stmt = conn.prepare(
        "SELECT language,
                SUM(CASE WHEN outcome = 'success' THEN 1 ELSE 0 END) as starts,
                SUM(CASE WHEN outcome = 'failed'  THEN 1 ELSE 0 END) as failures,
                SUM(CASE WHEN outcome = 'success' AND reason = 'new_session'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'idle_evicted' THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'lru_evicted'  THEN 1 ELSE 0 END),
                SUM(CASE WHEN outcome = 'success' AND reason = 'crashed'      THEN 1 ELSE 0 END),
                AVG(CASE WHEN outcome = 'success' THEN handshake_ms END),
                AVG(CASE WHEN outcome = 'success' THEN first_response_ms END)
         FROM lsp_events
         WHERE started_at >= datetime('now', ?)
         GROUP BY language
         ORDER BY starts DESC, failures DESC",
    )?;

    #[allow(clippy::type_complexity)]
    let rows: Vec<(
        String,
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
        Option<f64>,
        Option<f64>,
    )> = agg_stmt
        .query_map([modifier], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut by_language = Vec::new();
    for (
        language,
        starts,
        failures,
        new_session,
        idle_evicted,
        lru_evicted,
        crashed,
        avg_handshake,
        avg_first,
    ) in rows
    {
        let p95_handshake = lsp_percentile(conn, &language, modifier, 95, "handshake_ms")?;
        // `.ok()` is intentional: `p95_first_response_ms` is an Optional field in the response.
        // `lsp_percentile` returns `Ok(0)` when count=0 (all NULL values), so the only case
        // `.ok()` silently discards is a genuine DB error — acceptable for a best-effort
        // observability field.
        let p95_first = lsp_percentile(conn, &language, modifier, 95, "first_response_ms").ok();

        by_language.push(LspLanguageStats {
            language,
            starts,
            failures,
            reasons: LspReasonCounts {
                new_session,
                idle_evicted,
                lru_evicted,
                crashed,
            },
            // None = no successful start in the window (e.g. a fail-only language) → 0.
            avg_handshake_ms: avg_handshake.map(|v| v.round() as i64).unwrap_or(0),
            p95_handshake_ms: p95_handshake,
            avg_first_response_ms: avg_first.map(|v| v.round() as i64),
            p95_first_response_ms: p95_first,
        });
    }

    // Recent successful events (last 20, not window-filtered — always shows the most
    // recent cold starts regardless of the selected window, so the list is never empty
    // while data exists).
    let mut recent_stmt = conn.prepare(
        "SELECT language, started_at, reason, handshake_ms, first_response_ms
         FROM lsp_events
         WHERE outcome = 'success'
         ORDER BY started_at DESC
         LIMIT 20",
    )?;
    let recent: Vec<LspEvent> = recent_stmt
        .query_map([], |r| {
            Ok(LspEvent {
                language: r.get(0)?,
                started_at: r.get(1)?,
                reason: r.get(2)?,
                handshake_ms: r.get(3)?,
                first_response_ms: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    // Recent failed starts (last 20, not window-filtered) — the actionable signal:
    // which server keeps dying during `initialize`, and the error it reported.
    let mut fail_stmt = conn.prepare(
        "SELECT language, started_at, reason, error
         FROM lsp_events
         WHERE outcome = 'failed'
         ORDER BY started_at DESC
         LIMIT 20",
    )?;
    let recent_failures: Vec<LspFailure> = fail_stmt
        .query_map([], |r| {
            Ok(LspFailure {
                language: r.get(0)?,
                started_at: r.get(1)?,
                reason: r.get(2)?,
                error: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(LspStats {
        window: window.to_string(),
        by_language,
        recent,
        recent_failures,
    })
}

fn lsp_percentile(
    conn: &Connection,
    language: &str,
    modifier: &str,
    pct: i64,
    column: &str,
) -> Result<i64> {
    let column = match column {
        "handshake_ms" => "handshake_ms",
        "first_response_ms" => "first_response_ms",
        _ => anyhow::bail!("lsp_percentile: unexpected column '{column}' — only hardcoded column literals are safe"),
    };
    // Only count non-NULL values for the given column
    let count: i64 = conn.query_row(
        &format!(
            "SELECT COUNT({}) FROM lsp_events\n             WHERE language = ? AND outcome = 'success' AND started_at >= datetime('now', ?) AND {} IS NOT NULL",
            column, column
        ),
        params![language, modifier],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    let offset = ((count * pct + 99) / 100 - 1).max(0);
    let val: i64 = conn.query_row(
        &format!(
            "SELECT {} FROM lsp_events\n             WHERE language = ? AND outcome = 'success' AND started_at >= datetime('now', ?) AND {} IS NOT NULL\n             ORDER BY {} LIMIT 1 OFFSET ?",
            column, column, column
        ),
        params![language, modifier, offset],
        |r| r.get(0),
    )?;
    Ok(val)
}
pub fn query_stats(conn: &Connection, window: &str) -> Result<UsageStats> {
    let modifier = window_to_modifier(window);
    let mut stmt = conn.prepare(
        "SELECT tool_name,
                COUNT(*) as calls,
                SUM(CASE WHEN outcome IN ('error', 'recoverable_error') THEN 1 ELSE 0 END) as errors,
                SUM(overflowed) as overflows
         FROM tool_calls
         WHERE called_at >= datetime('now', ?)
         GROUP BY tool_name
         ORDER BY calls DESC",
    )?;

    let rows: Vec<(String, i64, i64, i64)> = stmt
        .query_map([modifier], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let total_calls: i64 = rows.iter().map(|r| r.1).sum();

    let mut by_tool = Vec::new();
    for (tool_name, calls, errors, overflows) in rows {
        let p50_ms = percentile(conn, &tool_name, modifier, 50)?;
        let p99_ms = percentile(conn, &tool_name, modifier, 99)?;
        by_tool.push(ToolStats {
            error_rate_pct: if calls > 0 {
                errors as f64 / calls as f64 * 100.0
            } else {
                0.0
            },
            overflow_rate_pct: if calls > 0 {
                overflows as f64 / calls as f64 * 100.0
            } else {
                0.0
            },
            tool: tool_name,
            calls,
            errors,
            overflows,
            p50_ms,
            p99_ms,
        });
    }

    Ok(UsageStats {
        window: window.to_string(),
        total_calls,
        by_tool,
    })
}

fn percentile(conn: &Connection, tool_name: &str, modifier: &str, pct: i64) -> Result<i64> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM tool_calls WHERE tool_name = ? AND called_at >= datetime('now', ?)",
        params![tool_name, modifier],
        |r| r.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    // Nearest-rank method: ceil(count * pct / 100) - 1 (0-indexed)
    let offset = ((count * pct + 99) / 100 - 1).max(0);
    let val: i64 = conn.query_row(
        "SELECT latency_ms FROM tool_calls
         WHERE tool_name = ? AND called_at >= datetime('now', ?)
         ORDER BY latency_ms
         LIMIT 1 OFFSET ?",
        params![tool_name, modifier, offset],
        |r| r.get(0),
    )?;
    Ok(val)
}

fn window_to_modifier(window: &str) -> &'static str {
    match window {
        "1h" => "-1 hours",
        "24h" => "-24 hours",
        "7d" => "-7 days",
        _ => "-30 days",
    }
}

#[derive(Debug, serde::Serialize)]
pub struct ErrorRecord {
    pub tool: String,
    pub timestamp: String,
    pub outcome: String,
    pub message: Option<String>,
}

pub fn recent_errors(conn: &Connection, limit: i64) -> Result<Vec<ErrorRecord>> {
    let mut stmt = conn.prepare(
        "SELECT tool_name, called_at, outcome, error_msg
         FROM tool_calls
         WHERE outcome IN ('error', 'recoverable_error')
         ORDER BY called_at DESC, rowid DESC
         LIMIT ?",
    )?;
    let rows = stmt
        .query_map([limit], |r| {
            Ok(ErrorRecord {
                tool: r.get(0)?,
                timestamp: r.get(1)?,
                outcome: r.get(2)?,
                message: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp() -> (TempDir, Connection) {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        (dir, conn)
    }

    #[test]
    fn open_db_creates_table() {
        let (_dir, conn) = tmp();
        // table exists if this doesn't error
        conn.execute("SELECT 1 FROM tool_calls LIMIT 0", [])
            .unwrap();
    }

    #[test]
    fn write_record_roundtrip() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn write_record_stores_all_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "semantic_search",
            150,
            "recoverable_error",
            false,
            Some("path not found"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let (name, latency, outcome, overflowed, msg): (String, i64, String, i64, Option<String>) =
            conn.query_row(
                "SELECT tool_name, latency_ms, outcome, overflowed, error_msg FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(name, "semantic_search");
        assert_eq!(latency, 150);
        assert_eq!(outcome, "recoverable_error");
        assert_eq!(overflowed, 0);
        assert_eq!(msg.as_deref(), Some("path not found"));
    }

    #[test]
    fn write_record_overflow_flag() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "references",
            80,
            "success",
            true,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let overflowed: i64 = conn
            .query_row("SELECT overflowed FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(overflowed, 1);
    }

    #[test]
    fn retention_prunes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert a row with a timestamp 31 days ago
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('old_tool', datetime('now', '-31 days'), 10, 'success', 0)",
            [],
        )
        .unwrap();
        let before: i64 = conn
            .query_row("SELECT COUNT(*) FROM tool_calls", [], |r| r.get(0))
            .unwrap();
        assert_eq!(before, 1);

        // Next write triggers pruning
        write_record(
            &conn,
            "new_tool",
            5,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let after: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE tool_name = 'old_tool'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(after, 0);
    }

    fn insert_call(conn: &Connection, tool: &str, latency: i64, outcome: &str, overflowed: bool) {
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES (?1, datetime('now'), ?2, ?3, ?4)",
            params![tool, latency, outcome, overflowed as i64],
        )
        .unwrap();
    }

    #[test]
    fn query_stats_empty_db() {
        let (_dir, conn) = tmp();
        let stats = query_stats(&conn, "30d").unwrap();
        assert_eq!(stats.total_calls, 0);
        assert!(stats.by_tool.is_empty());
    }

    #[test]
    fn query_stats_counts_correctly() {
        let (_dir, conn) = tmp();
        insert_call(&conn, "symbols", 100, "success", false);
        insert_call(&conn, "symbols", 200, "success", false);
        insert_call(&conn, "symbols", 300, "error", false);
        insert_call(&conn, "semantic_search", 500, "success", true);

        let stats = query_stats(&conn, "30d").unwrap();
        assert_eq!(stats.total_calls, 4);
        assert_eq!(stats.by_tool.len(), 2);

        // symbols should be first (3 calls > 1)
        let fs = &stats.by_tool[0];
        assert_eq!(fs.tool, "symbols");
        assert_eq!(fs.calls, 3);
        assert_eq!(fs.errors, 1);
        assert_eq!(fs.overflows, 0);

        let ss = &stats.by_tool[1];
        assert_eq!(ss.tool, "semantic_search");
        assert_eq!(ss.overflows, 1);
    }

    #[test]
    fn query_stats_percentiles() {
        let (_dir, conn) = tmp();
        // Insert 10 calls with known latencies 10..100ms
        for i in 1..=10 {
            insert_call(&conn, "symbols", i * 10, "success", false);
        }
        let stats = query_stats(&conn, "30d").unwrap();
        let fs = &stats.by_tool[0];
        // p50 = 50ms (5th of 10, 0-indexed offset 5)
        assert_eq!(fs.p50_ms, 50);
        // p99 = ~100ms (last item)
        assert_eq!(fs.p99_ms, 100);
    }

    #[test]
    fn query_stats_window_excludes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert a row 2 days ago
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, overflowed)
             VALUES ('old_tool', datetime('now', '-2 days'), 50, 'success', 0)",
            [],
        )
        .unwrap();
        insert_call(&conn, "new_tool", 10, "success", false);

        let stats_1h = query_stats(&conn, "1h").unwrap();
        // Only new_tool (inserted now) should appear in 1h window
        assert_eq!(stats_1h.total_calls, 1);
        assert_eq!(stats_1h.by_tool[0].tool, "new_tool");
    }

    #[test]
    fn recent_errors_returns_latest_errors() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            50,
            "success",
            false,
            None,
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        write_record(
            &conn,
            "semantic_search",
            100,
            "error",
            false,
            Some("index missing"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        write_record(
            &conn,
            "references",
            30,
            "recoverable_error",
            false,
            Some("path not found"),
            "unknown",
            None,
            "test-session",
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let errors = recent_errors(&conn, 10).unwrap();
        assert_eq!(errors.len(), 2);
        // Most recent first
        assert_eq!(errors[0].tool, "references");
        assert_eq!(errors[1].tool, "semantic_search");
    }

    #[test]
    fn recent_errors_respects_limit() {
        let (_dir, conn) = tmp();
        for i in 0..5 {
            write_record(
                &conn,
                &format!("tool_{}", i),
                10,
                "error",
                false,
                Some("fail"),
                "unknown",
                None,
                "test-session",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        }
        let errors = recent_errors(&conn, 3).unwrap();
        assert_eq!(errors.len(), 3);
    }

    #[test]
    fn write_lsp_event_returns_rowid() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        assert!(rowid > 0);
    }

    #[test]
    fn write_lsp_failure_records_failed_outcome() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            813,
            "LSP server disconnected",
        )
        .unwrap();
        assert!(rowid > 0);
        let (outcome, error): (String, Option<String>) = conn
            .query_row(
                "SELECT outcome, error FROM lsp_events WHERE id = ?",
                [rowid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(outcome, "failed");
        assert_eq!(error.as_deref(), Some("LSP server disconnected"));
    }

    #[test]
    fn write_lsp_event_defaults_outcome_to_success() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        let outcome: String = conn
            .query_row(
                "SELECT outcome FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(outcome, "success");
    }

    #[test]
    fn query_lsp_stats_excludes_failed_starts() {
        let (_dir, conn) = tmp();
        write_lsp_event(&conn, "kotlin", "new_session", 3000).unwrap();
        write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            800,
            "LSP server disconnected",
        )
        .unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .unwrap();
        // A failed start must not inflate the success count or skew the handshake avg.
        assert_eq!(kotlin.starts, 1);
        assert_eq!(kotlin.avg_handshake_ms, 3000);
        // ...but it IS counted as a failure and surfaced in recent_failures.
        assert_eq!(kotlin.failures, 1);
        assert_eq!(stats.recent_failures.len(), 1);
        assert_eq!(stats.recent_failures[0].language, "kotlin");
        assert_eq!(
            stats.recent_failures[0].error.as_deref(),
            Some("LSP server disconnected")
        );
    }

    #[test]
    fn query_lsp_stats_surfaces_fail_only_language() {
        let (_dir, conn) = tmp();
        // kotlin has ONLY a failed start — it must still appear in by_language
        // (starts=0, failures=1), not vanish the way a success-only aggregate would.
        write_lsp_failure(
            &conn,
            "kotlin",
            "new_session",
            800,
            "LSP server disconnected",
        )
        .unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .expect("a fail-only language must still appear in by_language");
        assert_eq!(kotlin.starts, 0);
        assert_eq!(kotlin.failures, 1);
        assert_eq!(kotlin.avg_handshake_ms, 0);
        assert_eq!(stats.recent_failures.len(), 1);
    }

    #[test]
    fn update_lsp_first_response_fills_null() {
        let (_dir, conn) = tmp();
        let rowid = write_lsp_event(&conn, "rust", "new_session", 820).unwrap();
        // Before update: first_response_ms should be NULL
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert!(val.is_none());
        // After update: should be set
        update_lsp_first_response(&conn, rowid, 9100).unwrap();
        let val: Option<i64> = conn
            .query_row(
                "SELECT first_response_ms FROM lsp_events WHERE id = ?",
                [rowid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(val, Some(9100));
    }

    #[test]
    fn query_lsp_stats_aggregates_correctly() {
        let (_dir, conn) = tmp();
        write_lsp_event(&conn, "rust", "new_session", 800).unwrap();
        write_lsp_event(&conn, "rust", "idle_evicted", 1200).unwrap();
        write_lsp_event(&conn, "kotlin", "new_session", 5000).unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        assert_eq!(stats.by_language.len(), 2);

        let rust = stats
            .by_language
            .iter()
            .find(|l| l.language == "rust")
            .unwrap();
        assert_eq!(rust.starts, 2);
        assert_eq!(rust.reasons.new_session, 1);
        assert_eq!(rust.reasons.idle_evicted, 1);
        assert_eq!(rust.avg_handshake_ms, 1000); // (800 + 1200) / 2
        assert!(rust.p95_handshake_ms >= 800);

        let kotlin = stats
            .by_language
            .iter()
            .find(|l| l.language == "kotlin")
            .unwrap();
        assert_eq!(kotlin.starts, 1);
        assert_eq!(kotlin.avg_handshake_ms, 5000);
    }

    #[test]
    fn query_lsp_stats_window_excludes_old_rows() {
        let (_dir, conn) = tmp();
        // Insert an old row manually with an ancient timestamp
        conn.execute(
            "INSERT INTO lsp_events (language, started_at, reason, handshake_ms)
             VALUES ('rust', datetime('now', '-60 days'), 'new_session', 999)",
            [],
        )
        .unwrap();
        // Insert a recent row
        write_lsp_event(&conn, "rust", "new_session", 800).unwrap();

        let stats = query_lsp_stats(&conn, "30d").unwrap();
        let rust = stats
            .by_language
            .iter()
            .find(|l| l.language == "rust")
            .unwrap();
        // Only the recent row should be counted
        assert_eq!(rust.starts, 1);
        assert_eq!(rust.avg_handshake_ms, 800);
    }

    #[test]
    fn query_lsp_stats_recent_returns_last_20() {
        let (_dir, conn) = tmp();
        for i in 0..25i64 {
            write_lsp_event(&conn, "rust", "new_session", i * 10).unwrap();
        }
        let stats = query_lsp_stats(&conn, "30d").unwrap();
        assert_eq!(stats.recent.len(), 20);
    }

    #[test]
    fn open_db_migrates_traceability_columns() {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, called_at, latency_ms, outcome, codescout_sha, project_sha, session_id, input_json, output_json)
             VALUES ('test', datetime('now'), 10, 'success', 'abc1234', 'def5678', 'sess-1', '{\"q\":\"x\"}', NULL)",
            [],
        )
        .unwrap();
        type Row = (
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        );
        let (cs, ps, sid, inp, out): Row = conn
            .query_row(
                "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(cs.as_deref(), Some("abc1234"));
        assert_eq!(ps.as_deref(), Some("def5678"));
        assert_eq!(sid.as_deref(), Some("sess-1"));
        assert_eq!(inp.as_deref(), Some("{\"q\":\"x\"}"));
        assert!(out.is_none());
    }

    #[test]
    fn open_db_migrates_friction_columns() {
        let dir = TempDir::new().unwrap();
        let conn = open_db(dir.path()).unwrap();
        conn.execute(
            "INSERT INTO tool_calls (tool_name, latency_ms, outcome, friction_target, overflow_tokens, err_family, project_root)
             VALUES ('symbols', 10, 'success', 'LspManager/get_or_start', 1045, NULL, '/repo')",
            [],
        )
        .unwrap();
        let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(1045));
        assert_eq!(ef, None);
        assert_eq!(pr.as_deref(), Some("/repo"));
    }

    #[test]
    fn write_record_stores_traceability_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "error",
            false,
            Some("not found"),
            "abc1234",
            Some("def5678"),
            "sess-1",
            Some("{\"query\":\"foo\"}"),
            Some("{\"error\":\"not found\"}"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let (cs, ps, sid, inp, out): (String, String, String, String, String) = conn
            .query_row(
                "SELECT codescout_sha, project_sha, session_id, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .unwrap();
        assert_eq!(cs, "abc1234");
        assert_eq!(ps, "def5678");
        assert_eq!(sid, "sess-1");
        assert_eq!(inp, "{\"query\":\"foo\"}");
        assert_eq!(out, "{\"error\":\"not found\"}");
    }

    #[test]
    fn write_record_traceability_fields_nullable() {
        let (_dir, conn) = tmp();
        write_record(
            &conn, "symbols", 42, "success", false, None, "abc1234", None, "sess-1", None, None,
            None, None, None, None, None,
        )
        .unwrap();
        let (ps, inp, out): (Option<String>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT project_sha, input_json, output_json FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(ps.is_none());
        assert!(inp.is_none());
        assert!(out.is_none());
    }

    #[test]
    fn write_record_stores_friction_fields() {
        let (_dir, conn) = tmp();
        write_record(
            &conn,
            "symbols",
            42,
            "success",
            true,
            None,
            "cs-sha",
            Some("proj-sha"),
            "sess-1",
            None,
            None,
            None,
            Some("LspManager/get_or_start"),
            Some(1045),
            None,
            Some("/repo"),
        )
        .unwrap();
        let (ft, tok, ef, pr): (Option<String>, Option<i64>, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT friction_target, overflow_tokens, err_family, project_root FROM tool_calls",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(ft.as_deref(), Some("LspManager/get_or_start"));
        assert_eq!(tok, Some(1045));
        assert_eq!(ef, None);
        assert_eq!(pr.as_deref(), Some("/repo"));
    }

    #[test]
    fn normalize_err_family_maps_iron_law_routing_errors() {
        // The families that dominate the real error population — previously all NULL.
        let cases = [
            (
                "source range overlaps named symbol(s): 'open_db'",
                Some("il1_read_overlaps_symbol"),
            ),
            (
                "Use read_markdown for markdown files",
                Some("il4_read_markdown_routing"),
            ),
            (
                "Use edit_markdown for markdown files",
                Some("il5_edit_markdown_routing"),
            ),
            (
                "edit contains a symbol definition (\"def \") — use symbol tools",
                Some("il2_structural_edit"),
            ),
            (
                "edit_file is blocked for structural edits on source code files",
                Some("il2_structural_edit"),
            ),
            (
                "shell access to source files is blocked",
                Some("il3_shell_on_source"),
            ),
            (
                "IL3 violation — piped `cargo test` to a log-trimmer. BLOCKED.",
                Some("il3_pipe_to_trimmer"),
            ),
            (
                "write denied: '/x/INDEX.md' is outside the project root",
                Some("write_scope_denied"),
            ),
            (
                "unsupported json_path segment '[*]'",
                Some("json_path_unsupported"),
            ),
            ("old_string not found in src/x.rs", Some("edit_stale_match")),
            // Pre-existing families still resolve.
            ("LSP server disconnected", Some("lsp_disconnect")),
            ("symbol not found: Foo/bar", Some("symbol_not_found")),
            ("some unrecognized failure", None),
        ];
        for (msg, want) in cases {
            assert_eq!(normalize_err_family(msg), want, "msg: {msg}");
        }
    }

    #[test]
    fn backfill_fills_project_root_and_err_family_once() {
        let dir = TempDir::new().unwrap();
        // First open runs the backfill on an empty DB and stamps user_version.
        let conn = open_db(dir.path()).unwrap();

        // Simulate legacy rows: friction columns NULL, but error_msg retained.
        conn.execute(
            "INSERT INTO tool_calls (tool_name, latency_ms, outcome, error_msg, project_root, err_family) VALUES \
             ('read_file', 5, 'recoverable_error', 'source range overlaps named symbol(s): foo', NULL, NULL), \
             ('edit_file', 5, 'recoverable_error', 'Use edit_markdown for markdown files', NULL, NULL), \
             ('symbols',   5, 'success', NULL, NULL, NULL)",
            [],
        )
        .unwrap();
        // Roll the marker back to simulate a pre-backfill DB, then re-open.
        conn.execute_batch("PRAGMA user_version = 0;").unwrap();
        drop(conn);

        let conn = open_db(dir.path()).unwrap();

        let pr_nulls: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE project_root IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pr_nulls, 0, "project_root backfilled for every row");

        let fam = |tool: &str| -> Option<String> {
            conn.query_row(
                "SELECT err_family FROM tool_calls WHERE tool_name = ?1",
                [tool],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(
            fam("read_file").as_deref(),
            Some("il1_read_overlaps_symbol")
        );
        assert_eq!(
            fam("edit_file").as_deref(),
            Some("il5_edit_markdown_routing")
        );
        assert_eq!(fam("symbols"), None, "no error_msg → family stays NULL");

        // Idempotent: a third open is a no-op and does not error.
        drop(conn);
        let conn = open_db(dir.path()).unwrap();
        let still_null: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM tool_calls WHERE project_root IS NULL",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still_null, 0);
    }
}
