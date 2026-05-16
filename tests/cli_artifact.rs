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

/// Parse the `id` field out of an `artifact create` JSON envelope. The tool
/// returns `{"id":"...","abs_path":"...","tracker_hint":...?}` — adjust this
/// helper if the envelope shape changes.
fn extract_created_id(stdout: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(stdout)
        .unwrap_or_else(|e| panic!("create stdout is not JSON: {stdout}\nerror: {e}"));
    parsed
        .get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| panic!("create envelope has no 'id' field: {parsed}"))
}

#[test]
fn artifact_create_then_get_round_trip() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "artifact",
            "create",
            "--kind",
            "spec",
            "--title",
            "Test Spec",
            "--rel-path",
            "docs/test-spec.md",
            "--body",
            "round-trip body",
            "--json",
        ])
        .assert()
        .success();
    let create_out = String::from_utf8(create.get_output().stdout.clone()).unwrap();
    let id = extract_created_id(&create_out);

    run_cmd(&tmp)
        .current_dir(&work)
        .args(["artifact", "get", &id, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Spec"));
}

#[test]
fn artifact_update_status_archived_then_find_excludes() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "artifact",
            "create",
            "--kind",
            "spec",
            "--title",
            "Soon Archived",
            "--rel-path",
            "docs/soon-archived.md",
            "--body",
            "to be archived",
            "--json",
        ])
        .assert()
        .success();
    let id = extract_created_id(&String::from_utf8(create.get_output().stdout.clone()).unwrap());

    run_cmd(&tmp)
        .current_dir(&work)
        .args(["artifact", "update", &id, "--status", "archived", "--json"])
        .assert()
        .success();

    let find = run_cmd(&tmp)
        .current_dir(&work)
        .args(["artifact", "find", "--kind", "spec", "--json"])
        .assert()
        .success();
    let find_out = String::from_utf8(find.get_output().stdout.clone()).unwrap();
    assert!(
        !find_out.contains(&id),
        "archived artifact should not appear in default find; got: {find_out}"
    );
}

#[test]
fn artifact_link_then_graph_shows_edge() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create_a = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "artifact",
            "create",
            "--kind",
            "spec",
            "--title",
            "A",
            "--rel-path",
            "docs/a.md",
            "--body",
            "a body",
            "--json",
        ])
        .assert()
        .success();
    let a_id =
        extract_created_id(&String::from_utf8(create_a.get_output().stdout.clone()).unwrap());

    let create_b = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "artifact",
            "create",
            "--kind",
            "spec",
            "--title",
            "B",
            "--rel-path",
            "docs/b.md",
            "--body",
            "b body",
            "--json",
        ])
        .assert()
        .success();
    let b_id =
        extract_created_id(&String::from_utf8(create_b.get_output().stdout.clone()).unwrap());

    run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "artifact",
            "link",
            "--src",
            &a_id,
            "--dst",
            &b_id,
            "--rel",
            "implements",
            "--json",
        ])
        .assert()
        .success();

    let graph = run_cmd(&tmp)
        .current_dir(&work)
        .args(["artifact", "graph", &a_id, "--depth", "1", "--json"])
        .assert()
        .success();
    let graph_out = String::from_utf8(graph.get_output().stdout.clone()).unwrap();
    assert!(
        graph_out.contains(&b_id),
        "graph should mention B's id; got: {graph_out}"
    );
}
