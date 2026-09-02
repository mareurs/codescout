//! Integration smoke tests for `codescout doc <verb>` CLI subcommands.
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
    // Hermeticity: block the startup dotenv (main.rs `load_startup_env`) from
    // re-supplying LIBRARIAN_EMBED_MODEL out of the machine's global
    // ~/.config/codescout/.env. dotenvy does not override already-set vars but
    // WILL set a var we just removed, which would defeat the env_remove above
    // and make no-embedder assertions env-dependent. Point CODESCOUT_ENV_FILE
    // at a path that does not exist so load_startup_env is a no-op.
    cmd.env("CODESCOUT_ENV_FILE", tmp.path().join("no-startup.env"));
    // Regression guard: pin an unreachable Qdrant so the Qdrant-down path runs
    // on EVERY machine. qdrant-client's compatibility probe `println!`s onto
    // stdout when it cannot reach a server, prepending prose to the `--json`
    // envelopes these tests parse; with a live Qdrant on the default port the
    // probe succeeds silently, so the bug is invisible anywhere the stack is
    // running and only surfaces in CI. Do NOT make this hermetic by forcing
    // CODESCOUT_ARTIFACT_BACKEND=sqlite-vec instead — that skips the Qdrant path
    // entirely and could never catch the bug again.
    // docs/issues/archive/2026-08-08-qdrant-compat-check-printlns-to-stdout.md
    cmd.env("CODESCOUT_QDRANT_URL", "http://127.0.0.1:1");
    cmd
}

#[test]
fn doc_find_on_empty_catalog_returns_empty_items_json() {
    let tmp = TempDir::new().unwrap();
    let assert = run_cmd(&tmp)
        .args(["doc", "find", "--json"])
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
fn doc_find_bad_filter_reports_error() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["doc", "find", "--filter", "{not-json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--filter is not valid JSON"));
}

#[test]
fn doc_find_semantic_without_embedder_reports_hint() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["doc", "find", "--semantic", "anything"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("LIBRARIAN_EMBED_MODEL"));
}

#[test]
fn doc_get_missing_id_errors_and_names_both_recovery_paths() {
    let tmp = TempDir::new().unwrap();
    // An unknown id is an ERROR, not an Ok(null). A null could not be told from an
    // artifact that exists with an empty body, so the CLI printed "no such thing"
    // and "found it, it's empty" identically — and as a SUCCESS.
    //
    // This test previously asserted `.success()` and carried a comment stating the
    // null return as the contract, so it went red when 9a71357e fixed the tool.
    // That is the CLI-side half of the same fix, not a separate regression.
    let assert = run_cmd(&tmp)
        .args(["doc", "get", "definitely-not-a-real-id", "--json"])
        .assert()
        .failure();
    let err = String::from_utf8(assert.get_output().stderr.clone()).unwrap();
    assert!(
        err.contains("definitely-not-a-real-id"),
        "the error must name the id it could not resolve; got: {err}"
    );
    // Both branches, deliberately. A re-keyed id and a never-indexed one need
    // opposite repairs, and the caller cannot choose without being told both
    // exist — a single-branch hint sends every reader to `reindex`, which does
    // not help the far more common case of an id changed by an archive move.
    assert!(
        err.contains("reindex"),
        "must name the never-indexed repair; got: {err}"
    );
    assert!(
        err.contains("include_archived"),
        "must name the re-keyed repair; got: {err}"
    );
}

#[test]
fn doc_graph_missing_id_runs() {
    let tmp = TempDir::new().unwrap();
    // Tool emits a graph with a single seed node and no edges for an unknown id —
    // it doesn't error. Smoke confirms clap + dispatch + JSON shape.
    let assert = run_cmd(&tmp)
        .args(["doc", "graph", "definitely-not-a-real-id", "--json"])
        .assert()
        .success();
    let out = String::from_utf8(assert.get_output().stdout.clone()).unwrap();
    assert!(
        out.contains("\"nodes\"") && out.contains("\"edges\""),
        "expected nodes/edges fields; got: {out}"
    );
}

#[test]
fn doc_state_at_requires_commit_or_timestamp() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["doc", "state-at", "x", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--commit").or(predicate::str::contains("--timestamp")));
}

/// Clap's own error text for a subcommand this collapse removed. Verified
/// live 2026-09-02 by running the built binary rather than trusting the
/// snippet in the plan/brief — `codescout artifact find --json` prints:
///   error: unrecognized subcommand 'artifact'
/// on this clap version. Paired with a positive test (below and throughout
/// this file) that the replacement wiring works — an absence assertion alone
/// is monotone under removal of the WHOLE binary, not just the old verb.
#[test]
fn the_old_artifact_subcommand_is_gone() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["artifact", "find", "--json"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

/// Parse the `id` field out of a `doc create` JSON envelope. The tool
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
fn doc_create_then_get_round_trip() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
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
        .args(["doc", "get", &id, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Test Spec"));
}

#[test]
fn doc_update_status_archived_then_find_excludes() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
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
        .args(["doc", "update", &id, "--status", "archived", "--json"])
        .assert()
        .success();

    let find = run_cmd(&tmp)
        .current_dir(&work)
        .args(["doc", "find", "--kind", "spec", "--json"])
        .assert()
        .success();
    let find_out = String::from_utf8(find.get_output().stdout.clone()).unwrap();
    assert!(
        !find_out.contains(&id),
        "archived artifact should not appear in default find; got: {find_out}"
    );
}

#[test]
fn doc_link_then_graph_shows_edge() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create_a = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
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
            "doc",
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
            "doc",
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
        .args(["doc", "graph", &a_id, "--depth", "1", "--json"])
        .assert()
        .success();
    let graph_out = String::from_utf8(graph.get_output().stdout.clone()).unwrap();
    assert!(
        graph_out.contains(&b_id),
        "graph should mention B's id; got: {graph_out}"
    );
}

// --- `doc event <verb>` --------------------------------------------------

#[test]
fn doc_event_list_empty_catalog_runs() {
    let tmp = TempDir::new().unwrap();
    // Behavior depends on the tool — accept either "error: artifact not found" or success/empty.
    // We only assert it doesn't hang and exits with some status.
    let _ = run_cmd(&tmp)
        .args(["doc", "event", "list", "--id", "x", "--json"])
        .assert();
}

/// Observable-effect test for the `doc event create` / `doc event list`
/// pair — IC-15 means a wrong internal JSON key (e.g. sending `id` where
/// `event_create::Args`/`timeline::Args` require `artifact_id`, which these
/// two functions require because the CLI calls them directly and bypasses
/// `Artifact::call`'s key-translation layer) would be silently swallowed
/// rather than rejected, so exit-status alone proves nothing about wiring.
/// This asserts the created event's payload marker round-trips through list.
#[test]
fn doc_event_create_then_list_shows_the_event() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
            "create",
            "--kind",
            "spec",
            "--title",
            "Event Target",
            "--rel-path",
            "docs/evt.md",
            "--body",
            "body",
            "--json",
        ])
        .assert()
        .success();
    let id = extract_created_id(&String::from_utf8(create.get_output().stdout.clone()).unwrap());

    run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
            "event",
            "create",
            "--id",
            &id,
            "--kind",
            "note",
            "--payload",
            r#"{"text":"cli-doc-marker"}"#,
            "--json",
        ])
        .assert()
        .success();

    let list = run_cmd(&tmp)
        .current_dir(&work)
        .args(["doc", "event", "list", "--id", &id, "--json"])
        .assert()
        .success();
    let list_out = String::from_utf8(list.get_output().stdout.clone()).unwrap();
    assert!(
        list_out.contains("cli-doc-marker"),
        "listed events must include the note just created; got: {list_out}"
    );
}

// --- `doc refresh <verb>` -------------------------------------------------

#[test]
fn doc_refresh_list_stale_empty_catalog_succeeds() {
    let tmp = TempDir::new().unwrap();
    run_cmd(&tmp)
        .args(["doc", "refresh", "list-stale", "--json"])
        .assert()
        .success();
}

/// Observable-effect test for `doc refresh gather` — confirms the CLI's
/// `{"id": ...}` shape (built directly, `refresh::call` has no `action`
/// discriminant) actually resolves the artifact rather than silently
/// gathering nothing for an unrecognized key.
#[test]
fn doc_refresh_gather_on_augmented_artifact_returns_its_prompt() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
            "create",
            "--kind",
            "spec",
            "--title",
            "Gather Target",
            "--rel-path",
            "docs/gather.md",
            "--body",
            "body",
            "--augment-prompt",
            "gather-marker-prompt",
            "--json",
        ])
        .assert()
        .success();
    let id = extract_created_id(&String::from_utf8(create.get_output().stdout.clone()).unwrap());

    let gather = run_cmd(&tmp)
        .current_dir(&work)
        .args(["doc", "refresh", "gather", &id, "--json"])
        .assert()
        .success();
    let gather_out = String::from_utf8(gather.get_output().stdout.clone()).unwrap();
    assert!(
        gather_out.contains("gather-marker-prompt") || gather_out.contains(&id),
        "gather output should reflect the target artifact's augmentation; got: {gather_out}"
    );
}

// --- `doc augment <id>` ---------------------------------------------------

/// Observable-effect test for `doc augment` — confirms the prompt reaches
/// the catalog by round-tripping through `doc get`, not just checking exit
/// status (which a dropped/mis-keyed field would not disturb; see IC-15).
#[test]
fn doc_augment_then_get_shows_the_prompt() {
    let tmp = TempDir::new().unwrap();
    let work = tmp.path().join("project");
    std::fs::create_dir_all(work.join("docs")).unwrap();

    let create = run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
            "create",
            "--kind",
            "spec",
            "--title",
            "Augment Target",
            "--rel-path",
            "docs/augment.md",
            "--body",
            "body",
            "--json",
        ])
        .assert()
        .success();
    let id = extract_created_id(&String::from_utf8(create.get_output().stdout.clone()).unwrap());

    run_cmd(&tmp)
        .current_dir(&work)
        .args([
            "doc",
            "augment",
            &id,
            "--prompt",
            "cli-doc-augment-marker",
            "--json",
        ])
        .assert()
        .success();

    let get = run_cmd(&tmp)
        .current_dir(&work)
        .args(["doc", "get", &id, "--json"])
        .assert()
        .success();
    let get_out = String::from_utf8(get.get_output().stdout.clone()).unwrap();
    assert!(
        get_out.contains("cli-doc-augment-marker"),
        "the augmentation prompt must be readable back via `doc get`; got: {get_out}"
    );
}
