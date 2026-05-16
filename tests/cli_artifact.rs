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
