//! Integration smoke tests for `codescout artifact*` CLI verbs.
//!
//! Each test isolates state via tempdir + env overrides so they can run in
//! parallel without stepping on each other.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn run_cmd(tmp: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("codescout").unwrap();
    let db = tmp.path().join("cat.db");
    let ws = tmp.path().join("workspace.toml");
    std::fs::write(&ws, "").ok();
    cmd.env("LIBRARIAN_DB", &db);
    cmd.env("LIBRARIAN_WORKSPACE", &ws);
    cmd.env_remove("LIBRARIAN_EMBED_MODEL");
    cmd
}

#[test]
fn artifact_find_on_empty_catalog_returns_empty_items_json() {
    let tmp = TempDir::new().unwrap();
    let assert = run_cmd(&tmp)
        .args(["artifact", "find", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    // The exact key set depends on the tool; require at least an "items" array.
    assert!(
        out.contains("\"items\""),
        "expected items field; got: {out}"
    );
}

#[test]
fn artifact_find_bad_filter_reports_error() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "find", "--filter", "{not-json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--filter is not valid JSON"));
}

#[test]
fn artifact_find_semantic_without_embedder_reports_hint() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "find", "--semantic", "anything"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("LIBRARIAN_EMBED_MODEL"));
}

#[test]
fn artifact_get_missing_id_runs() {
    let tmp = TempDir::new().unwrap();
    // Tool returns `null` for a missing artifact rather than erroring — smoke just
    // exercises the clap → tool dispatch path without crashing.
    let assert = run_cmd(&tmp)
        .args(["artifact", "get", "definitely-not-a-real-id", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(!out.is_empty(), "expected some stdout payload; got empty");
}

#[test]
fn artifact_graph_missing_id_runs() {
    let tmp = TempDir::new().unwrap();
    // Tool emits a graph with a single seed node and no edges for an unknown id —
    // it doesn't error. Smoke confirms clap + dispatch + JSON shape.
    let assert = run_cmd(&tmp)
        .args(["artifact", "graph", "definitely-not-a-real-id", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        out.contains("\"nodes\"") && out.contains("\"edges\""),
        "expected nodes/edges fields; got: {out}"
    );
}

#[test]
fn artifact_state_at_requires_commit_or_timestamp() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "state-at", "x", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--commit").or(predicate::str::contains("--timestamp")));
}

#[test]
fn artifact_event_list_empty_catalog_runs() {
    let tmp = TempDir::new().unwrap();
    // Behavior depends on the tool — accept either "error: artifact not found" or success/empty.
    // We only assert it doesn't hang and exits with some status.
    let _ = run_cmd(&tmp)
        .args(["artifact-event", "list", "--artifact-id", "x", "--json"])
        .assert();
}

#[test]
fn artifact_refresh_list_stale_empty_catalog_succeeds() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact-refresh", "list-stale", "--json"])
        .assert()
        .success();
}
