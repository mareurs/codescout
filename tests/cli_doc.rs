//! Integration smoke tests for `codescout doc <verb>` CLI subcommands.
//!
//! Each test isolates state via tempdir + env overrides so they can run in
//! parallel without stepping on each other.

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// The path every test in this file executes: a **hardlink** to the built binary, taken
/// once, which pins the INODE so no peer can swap it mid-suite.
///
/// [`Command::cargo_bin`] resolves `target/debug/codescout` at RUN time and `target/` is
/// shared by every session in this checkout, so the file at that path can be replaced
/// between this lane's build and this lane's execution. Two measurements, 2026-09-14:
///
/// * Cargo holds `target/debug/.cargo-lock` through the BUILD phase (holder pid observed on
///   four consecutive samples) and **releases it before running tests** (no holder, sampled
///   with three `cli_doc` processes alive). So a peer's `--no-default-features` build lands
///   inside this window **however either session orders its lanes** — which is why the
///   gate's ordering rule cannot close it and no amount of compliance by anyone helps.
/// * Cargo replaces the binary by **rename**, not by writing in place: inode `196951675` →
///   `197002269` across one rebuild. **That is what makes a hardlink a fix rather than a
///   gesture** — the rename swaps the directory entry, and this link keeps the original
///   inode alive and unmodified for the life of the suite.
///
/// Without this, the outage is 13 of 15 tests failing on `unrecognized subcommand 'doc'`,
/// reading as a feature-gating regression in whatever the reader just committed. The one
/// test that looks like it would catch it — [`the_old_artifact_subcommand_is_gone`] — keeps
/// PASSING, because its absence assertion is monotone under losing the whole verb set
/// rather than just the old verb.
///
/// The `doc`-advertised check runs **once**, here, and that is only correct BECAUSE of the
/// pin: an unpinned path would need re-checking before every invocation and would still
/// leave a window between the check and the exec. Pinning is what collapses a recurring race
/// into a single precondition.
///
/// Stale pins are swept by AGE rather than by pid, which needs no `/proc` and so behaves the
/// same on the Windows lane. A pin is one directory entry, but it holds a ~200 MB inode
/// alive, so leaking them is not free.
///
/// `cluster/transient-shared-state-lies-to-readers` —
/// `docs/issues/2026-09-14-the-gate-ordering-guarantee-is-false-under-concurrency.md`
fn pinned_binary() -> &'static std::path::Path {
    static PIN: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PIN.get_or_init(|| {
        let src = assert_cmd::cargo::cargo_bin("codescout");
        let dir = src
            .parent()
            .expect("built binary has a parent dir")
            .to_path_buf();

        // Sweep pins older than 6h. Best-effort throughout: a failure to tidy must never
        // fail a test run.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let cutoff = std::time::SystemTime::now() - std::time::Duration::from_secs(6 * 3600);
            for e in entries.flatten() {
                let name = e.file_name();
                let Some(name) = name.to_str() else { continue };
                if !name.starts_with(".cli_doc-pin-") {
                    continue;
                }
                if e.metadata()
                    .and_then(|m| m.modified())
                    .is_ok_and(|m| m < cutoff)
                {
                    let _ = std::fs::remove_file(e.path());
                }
            }
        }

        // Link into the binary's OWN directory, so the link is guaranteed to be on the same
        // filesystem. A tempdir is not: `/tmp` is tmpfs here and `link()` would fail EXDEV.
        let pin = dir.join(format!(".cli_doc-pin-{}", std::process::id()));
        let _ = std::fs::remove_file(&pin);
        if std::fs::hard_link(&src, &pin).is_err() {
            // Cross-device, or a filesystem without links: copy instead. Costs ~200 MB once
            // and pins the bytes just as well; only the mechanism differs.
            std::fs::copy(&src, &pin).expect("cannot pin the codescout binary for this run");
        }

        let out = std::process::Command::new(&pin)
            .args(["doc", "--help"])
            .output()
            .expect("the pinned codescout binary must be runnable");
        assert!(
            out.status.success(),
            "the binary at {src:?} does not advertise `doc`, so it was built WITHOUT the \
             librarian feature. THIS IS ALMOST CERTAINLY NOT YOUR DIFF.\n\
             `target/` is shared across every session in this checkout, and cargo releases its \
             build lock before running tests, so a peer's `cargo test --no-default-features` \
             replaced the binary between this lane's build and this pin.\n\
             REPAIR, which you can perform alone and which is also the cheapest discriminator: \
             re-run `cargo test --test cli_doc`. The window is minutes. If it reds a second \
             time with this message, the binary on disk really is librarian-less — rebuild it \
             with `cargo test --workspace`.\n\
             binary stderr: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
        pin
    })
}

fn run_cmd(tmp: &TempDir) -> Command {
    let mut cmd = Command::new(pinned_binary());
    let db = tmp.path().join("cat.db");
    let ws = tmp.path().join("workspace.toml");
    std::fs::write(&ws, "").ok();
    cmd.env("LIBRARIAN_DB", &db);
    cmd.env("LIBRARIAN_WORKSPACE", &ws);
    cmd.env_remove("LIBRARIAN_EMBED_MODEL");
    // Since LibrarianEnv::from_env falls back to the shared CODESCOUT_EMBEDDING_*
    // family (2026-09-18, Task 6 of the embedding-config-consolidation plan), a
    // no-embedder assertion needs that whole family cleared too — not just
    // LIBRARIAN_EMBED_MODEL — or a machine whose shell has CODESCOUT_EMBEDDING_MODEL
    // (or a deprecated alias) exported ambiently would silently satisfy the
    // fallback and this test would pass or fail depending on who runs it.
    // `env_remove` only strips the name from THIS child's inherited environment;
    // it says nothing about the parent shell, which is exactly the part that
    // varies machine to machine.
    for name in codescout::config::embedding_env::all_names() {
        cmd.env_remove(name);
    }
    cmd.env_remove("LIBRARIAN_EMBED_URL");
    cmd.env_remove("LIBRARIAN_EMBED_API_KEY");
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
fn doc_find_semantic_falls_back_to_codescout_embedding_model() {
    // LIBRARIAN_EMBED_MODEL absent (run_cmd's default), but CODESCOUT_EMBEDDING_MODEL
    // set — the pre-check must not report "requires the embedding service", since
    // LibrarianEnv::from_env now falls back to that family (Task 6, 2026-09-18).
    // The command may still fail LATER for an unrelated reason (no reachable server
    // for this bogus model name, an empty catalog, ...) — this asserts only that the
    // PRE-CHECK'S OWN message is absent, isolating "did the fallback satisfy the
    // gate" from "did semantic search actually run end to end", which is a separate
    // concern already covered elsewhere.
    let tmp = TempDir::new().unwrap();
    let assert = run_cmd(&tmp)
        .env("CODESCOUT_EMBEDDING_MODEL", "local:AllMiniLML6V2Q")
        .args(["doc", "find", "--semantic", "anything"])
        .assert();
    let out = assert.get_output();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("requires the embedding service"),
        "the pre-check's own refusal fired even though CODESCOUT_EMBEDDING_MODEL was set: {stderr}"
    );
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
