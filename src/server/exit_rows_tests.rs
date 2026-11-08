//! Every exit from `call_tool_inner` leaves exactly one row in `usage.db`.
//!
//! The recorder wraps only the tool call, so an exit taken BEFORE it (a gate refusal, a
//! refused write lock, an unknown tool) left no row, and every analysis over `tool_calls`
//! was blind to those calls. Measured 2026-11-08 on a rebuilt server: two refusals under a
//! read-only activation, zero rows. `dispatch_tool_call` now returns an `Exit`, so a new
//! early return has to say whether the recorder covers it; these tests pin the exits that
//! exist today, one row each, with the family a query can group on.
//!
//! Every test drives the real dispatch (`call_tool_inner`). A row is read back with a
//! second connection and counted again after a pause, because "one row" must also rule out
//! a second write.
//!
//! docs/issues/archive/2026-11-08-access-gate-refusals-leave-no-usage-row.md

use super::config_reload_tests::call;
use super::test_support::{make_server, make_server_with_project_toml};
use serde_json::json;
use std::path::Path;
use std::time::Duration;

/// `(outcome, err_family, error_msg)` of every row for `tool`, once at least one exists and
/// the count has held still for 300 ms.
async fn rows_for(dir: &Path, tool: &str) -> Vec<(String, Option<String>, Option<String>)> {
    let read = || -> Vec<(String, Option<String>, Option<String>)> {
        let Ok(conn) = rusqlite::Connection::open_with_flags(
            dir.join(".codescout").join("usage.db"),
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        ) else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT outcome, err_family, error_msg FROM tool_calls WHERE tool_name = ?1 ORDER BY id",
        ) else {
            return Vec::new();
        };
        stmt.query_map([tool], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .map(|rows| rows.filter_map(Result::ok).collect())
            .unwrap_or_default()
    };
    for _ in 0..100 {
        if !read().is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    tokio::time::sleep(Duration::from_millis(300)).await;
    read()
}

fn assert_one_row(
    rows: &[(String, Option<String>, Option<String>)],
    outcome: &str,
    family: &str,
    what: &str,
) {
    assert_eq!(rows.len(), 1, "{what}: exactly one row; got {rows:?}");
    let (got_outcome, got_family, msg) = &rows[0];
    assert_eq!(got_outcome, outcome, "{what}: outcome; row {rows:?}");
    assert_eq!(
        got_family.as_deref(),
        Some(family),
        "{what}: err_family; msg {msg:?}"
    );
}

#[tokio::test]
async fn a_read_only_gate_refusal_leaves_one_row() {
    let (_home, server) = make_server().await;
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let (err, text) = call(
        &server,
        "workspace",
        json!({ "action": "activate", "path": root.to_string_lossy(), "read_only": true }),
    )
    .await;
    assert!(!err, "precondition: {text}");

    let (refused, text) = call(
        &server,
        "create_file",
        json!({ "path": root.join("x.txt").to_string_lossy(), "content": "c" }),
    )
    .await;
    assert!(
        refused && text.contains("File writes are disabled"),
        "precondition: the gate refuses: {text}"
    );

    let rows = rows_for(&root, "create_file").await;
    assert_one_row(
        &rows,
        "error",
        "write_blocked_read_only",
        "read-only refusal",
    );
}

#[tokio::test]
async fn a_configured_off_gate_refusal_leaves_one_row_in_its_own_family() {
    let toml = "[project]\nname = \"t\"\n\n[security]\nfile_write_enabled = false\n";
    let (dir, server) = make_server_with_project_toml(Some(toml)).await;
    let (refused, text) = call(
        &server,
        "create_file",
        json!({ "path": dir.path().join("x.txt").to_string_lossy(), "content": "c" }),
    )
    .await;
    assert!(refused, "precondition: writes are off: {text}");

    let rows = rows_for(dir.path(), "create_file").await;
    assert_one_row(
        &rows,
        "error",
        "write_blocked_by_config",
        "configured-off refusal",
    );
}

/// The write lock is taken by the test, so `create_file` waits out `write_lock_timeout_secs`
/// and is refused before the tool body runs. That exit is `acquire_write_guard_if_writing`'s.
#[tokio::test]
async fn a_refused_write_lock_leaves_one_row() {
    let toml = "[project]\nname = \"t\"\n\n[security]\nwrite_lock_timeout_secs = 1\n";
    let (dir, server) = make_server_with_project_toml(Some(toml)).await;
    let (mutex, fd_lock, holder_path) = server
        .agent
        .with_project_at(None, |p| {
            Ok((
                p.write_lock.clone(),
                p.file_lock.clone(),
                crate::agent::holder_record_path(&p.root),
            ))
        })
        .await
        .unwrap();
    let _held = crate::agent::acquire_write_guard(
        mutex,
        fd_lock,
        holder_path,
        Duration::from_secs(5),
        "test-holder",
    )
    .await
    .expect("the test takes the lock first");

    let started = std::time::Instant::now();
    let (_err, text) = call(
        &server,
        "create_file",
        json!({ "path": dir.path().join("x.txt").to_string_lossy(), "content": "c" }),
    )
    .await;
    assert!(
        text.contains("write lock") && started.elapsed() >= Duration::from_millis(900),
        "precondition: the lock refused the call after its timeout: {text}"
    );
    assert!(!dir.path().join("x.txt").exists(), "nothing may be written");

    let rows = rows_for(dir.path(), "create_file").await;
    assert_one_row(
        &rows,
        "recoverable_error",
        "write_lock_unavailable",
        "lock refusal",
    );
}

/// An unknown tool is a protocol error and must stay one; it must also leave a row.
#[tokio::test]
async fn an_unknown_tool_stays_a_protocol_error_and_leaves_one_row() {
    let toml = "[project]\nname = \"t\"\n";
    let (dir, server) = make_server_with_project_toml(Some(toml)).await;
    let req: super::CallToolRequestParams = serde_json::from_value(json!({
        "name": "no_such_tool",
        "arguments": { "x": 1 },
    }))
    .unwrap();
    let result = server
        .call_tool_inner(req, None, None, tokio_util::sync::CancellationToken::new())
        .await;
    let err = result.expect_err("an unknown tool is still a protocol error, not a tool result");
    assert!(
        err.message.contains("unknown tool"),
        "the protocol error is unchanged: {err:?}"
    );

    let rows = rows_for(dir.path(), "no_such_tool").await;
    assert_one_row(&rows, "error", "unknown_tool", "unknown tool");
}

/// The `ack` refusals already went through `refuse_pin` and were recorded; they had no family,
/// so no query could group them.
#[tokio::test]
async fn a_refused_ack_handle_leaves_one_row_in_its_family() {
    let toml = "[project]\nname = \"t\"\n";
    let (dir, server) = make_server_with_project_toml(Some(toml)).await;
    let (_err, text) = call(&server, "create_file", json!({ "ack": "@ack_deadbeef" })).await;
    assert!(text.contains("expired or unknown"), "precondition: {text}");

    let rows = rows_for(dir.path(), "create_file").await;
    assert_one_row(
        &rows,
        "recoverable_error",
        "ack_handle_refused",
        "ack refusal",
    );
}

/// A tool that outlives `tool_timeout_secs`. Injected into the server's own tool list, because
/// no real tool is both slow on demand and subject to the server timeout (`run_command`, `index`
/// and `librarian reindex` are exempt, see `tool_skips_server_timeout`).
struct SlowTool;

#[async_trait::async_trait]
impl crate::tools::Tool for SlowTool {
    fn name(&self) -> &str {
        "slow_test_tool"
    }
    fn description(&self) -> &str {
        "sleeps for five seconds"
    }
    fn input_schema(&self) -> serde_json::Value {
        json!({"type": "object"})
    }
    async fn call(
        &self,
        _input: serde_json::Value,
        _ctx: &crate::tools::ToolContext,
    ) -> anyhow::Result<serde_json::Value> {
        tokio::time::sleep(Duration::from_secs(5)).await;
        Ok(json!({"done": true}))
    }
}

/// The server's own timeout drops the future that holds the recorder, so the recorder's write,
/// which runs after the tool body, never happened: the slowest calls were the ones missing.
/// Measured 2026-11-08 with a probe: the caller got the error, `usage.db` got nothing.
/// docs/issues/archive/2026-11-08-cancelled-and-timed-out-calls-leave-no-usage-row.md
///
/// The caller's answer must not change, and the row must say how long the call was held.
#[tokio::test]
async fn a_server_timeout_leaves_one_row_with_the_time_it_waited() {
    let toml = "[project]\nname = \"t\"\ntool_timeout_secs = 1\n";
    let (dir, mut server) = make_server_with_project_toml(Some(toml)).await;
    server.tools.push(std::sync::Arc::new(SlowTool));

    let (is_error, text) = call(&server, "slow_test_tool", json!({})).await;
    assert!(
        is_error && text.contains("Tool 'slow_test_tool' timed out after 1s."),
        "precondition: the caller still gets the timeout error, unchanged: {text}"
    );

    let rows = rows_for(dir.path(), "slow_test_tool").await;
    assert_one_row(&rows, "error", "tool_timeout", "server timeout");

    let latency_ms: i64 = rusqlite::Connection::open_with_flags(
        dir.path().join(".codescout").join("usage.db"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
    .query_row(
        "SELECT latency_ms FROM tool_calls WHERE tool_name = 'slow_test_tool'",
        [],
        |r| r.get(0),
    )
    .unwrap();
    assert!(
        (900..4_000).contains(&latency_ms),
        "the row records the wait (about 1000 ms), not the instant the write ran; got {latency_ms}"
    );
}
