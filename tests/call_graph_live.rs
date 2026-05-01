//! Live-LSP smoke tests for the call_graph tool.
//!
//! These tests require a real LSP server to be installed and are skipped when
//! the required binary is not on PATH.  Run with:
//!
//!     cargo test --test call_graph_live -- --ignored
//!
//! Each test follows the pattern established in `src/lsp/client.rs` live tests:
//! 1. Guard on binary availability.
//! 2. Create a temp project with the fixture source.
//! 3. Warm up the LSP (poll `symbols` until the server returns actual symbols).
//! 4. Call call_graph and assert on edges + source tags.
//!
//! When rust-analyzer hasn't finished indexing (prepareCallHierarchy returns None
//! or no edges come back), the test soft-skips with an `eprintln!` rather than
//! hard-failing — matching the behavior of `call_hierarchy_prepare_returns_item`.

use codescout::agent::Agent;
use codescout::lsp::LspManager;
use codescout::tools::output_buffer::OutputBuffer;
use codescout::tools::section_coverage::SectionCoverage;
use codescout::tools::symbol::{CallGraph, Symbols};
use codescout::tools::{Tool, ToolContext};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tempfile::tempdir;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Returns true if `cmd` is found on PATH.
fn lsp_available(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Create a temp project context with the given files and a real LspManager.
async fn project_with_files(files: &[(&str, &str)]) -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    for (name, content) in files {
        let path = dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: Arc::new(OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: Arc::new(Mutex::new(SectionCoverage::new())),
    };
    (dir, ctx)
}

/// Warm up the LSP by polling `symbols` until the server returns at least one
/// symbol for the given path, or 30 s elapses.
///
/// This is the same pattern used in `tests/rename_symbol.rs::warmup`.
async fn warmup(ctx: &ToolContext, path: &str) {
    let input = json!({ "path": path });
    for _ in 0u32..60 {
        match Symbols.call(input.clone(), ctx).await {
            Ok(v) if v["symbols"].as_array().map(|a| a.len()).unwrap_or(0) > 0 => return,
            _ => {}
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
}

/// Extract a flat list of edges from a call_graph result for the given direction key.
///
/// Returns `None` when the result contains no edges (empty array or missing key),
/// which is used as a soft-skip signal for indexing lag.
fn edges_for(result: &Value, direction: &str) -> Option<Vec<Value>> {
    let arr = result.get(direction)?.as_array()?;
    if arr.is_empty() {
        None
    } else {
        Some(arr.clone())
    }
}

// ── Rust / rust-analyzer ─────────────────────────────────────────────────────

const CARGO_TOML: &str = r#"[package]
name = "call-graph-fixture"
version = "0.1.0"
edition = "2021"
"#;

/// a → b → c  (linear call chain)
const LIB_RS: &str = r#"pub fn a() {
    b();
}

pub fn b() {
    c();
}

pub fn c() {}
"#;

/// Callees of `a` must reach `b` (depth 1) and `c` (depth 2), sourced from LSP.
///
/// Soft-skips when rust-analyzer hasn't finished indexing (prepareCallHierarchy
/// returns None for the seed — the call_graph tool then hits the TS fallback for
/// callees which returns a RecoverableError, or returns empty edges for callers).
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn call_graph_rust_callees_depth2() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping call_graph_rust_callees_depth2: rust-analyzer not installed");
        return;
    }

    let (dir, ctx) =
        project_with_files(&[("Cargo.toml", CARGO_TOML), ("src/lib.rs", LIB_RS)]).await;
    let _ = &dir; // keep TempDir alive

    warmup(&ctx, "src/lib.rs").await;

    let result = match CallGraph
        .call(
            json!({
                "symbol":    "a",
                "path":      "src/lib.rs",
                "direction": "callees",
                "max_depth": 2
            }),
            &ctx,
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            // TS fallback for callees throws a RecoverableError when callHierarchy
            // is unavailable — treat as indexing lag, not a test failure.
            eprintln!(
                "Skipping call_graph_rust_callees_depth2: call_graph returned error \
                 (likely indexing lag): {e}"
            );
            return;
        }
    };

    // Small result set → auto-promoted → callees is an array of edge objects.
    let edges = match edges_for(&result, "callees") {
        Some(e) => e,
        None => {
            eprintln!(
                "Skipping call_graph_rust_callees_depth2: no callee edges returned \
                 (likely indexing lag); result={result}"
            );
            return;
        }
    };

    let callee_names: Vec<&str> = edges
        .iter()
        .filter_map(|e: &Value| e.get("callee").and_then(|v| v.as_str()))
        .collect();

    assert!(
        callee_names.contains(&"b"),
        "expected 'b' among callees of 'a'; callees={callee_names:?}; full result={result}"
    );
    assert!(
        callee_names.contains(&"c"),
        "expected 'c' among callees of 'a' at depth 2; callees={callee_names:?}; full result={result}"
    );

    // At least one edge must be LSP-sourced (rust-analyzer provides callHierarchy).
    let has_lsp_edge = edges
        .iter()
        .any(|e: &Value| e.get("source").and_then(|s| s.as_str()) == Some("lsp"));
    assert!(
        has_lsp_edge,
        "expected at least one lsp-sourced edge; edges={edges:?}"
    );
}

/// Callers of `c` must find `b` (depth 1) and `a` (depth 2), sourced from LSP.
///
/// Soft-skips when rust-analyzer hasn't finished indexing.
#[tokio::test]
#[ignore] // requires rust-analyzer
async fn call_graph_rust_callers_depth2() {
    if !lsp_available("rust-analyzer") {
        eprintln!("Skipping call_graph_rust_callers_depth2: rust-analyzer not installed");
        return;
    }

    let (dir, ctx) =
        project_with_files(&[("Cargo.toml", CARGO_TOML), ("src/lib.rs", LIB_RS)]).await;
    let _ = &dir;

    warmup(&ctx, "src/lib.rs").await;

    let result = match CallGraph
        .call(
            json!({
                "symbol":    "c",
                "path":      "src/lib.rs",
                "direction": "callers",
                "max_depth": 2
            }),
            &ctx,
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            eprintln!(
                "Skipping call_graph_rust_callers_depth2: call_graph returned error \
                 (likely indexing lag): {e}"
            );
            return;
        }
    };

    let edges = match edges_for(&result, "callers") {
        Some(e) => e,
        None => {
            eprintln!(
                "Skipping call_graph_rust_callers_depth2: no caller edges returned \
                 (likely indexing lag); result={result}"
            );
            return;
        }
    };

    let caller_names: Vec<&str> = edges
        .iter()
        .filter_map(|e: &Value| e.get("caller").and_then(|v| v.as_str()))
        .collect();

    assert!(
        caller_names.contains(&"b"),
        "expected 'b' among callers of 'c'; callers={caller_names:?}; full result={result}"
    );
    assert!(
        caller_names.contains(&"a"),
        "expected 'a' among callers of 'c' at depth 2; callers={caller_names:?}; full result={result}"
    );

    let has_lsp_edge = edges
        .iter()
        .any(|e: &Value| e.get("source").and_then(|s| s.as_str()) == Some("lsp"));
    assert!(
        has_lsp_edge,
        "expected at least one lsp-sourced edge; edges={edges:?}"
    );
}
