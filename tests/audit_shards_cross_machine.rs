// The whole corpus exercises `codescout::librarian`, which is gated behind the
// `librarian` feature. Without this the file fails to compile under
// `--no-default-features` / `--features local-embed` (CI's other two configs)
// — same reasoning as tests/link_scan.rs.
#![cfg(feature = "librarian")]

//! Task 5 acceptance test for `2026-09-01-committed-audit-shards`.
//!
//! Consumes ONLY the shipped `librarian(action="audit_log", ...)` surface
//! (`codescout::librarian::tools::audit_log::call`), never the `pub(crate)`
//! `catalog::audit::{host, shard}` internals — those modules are genuinely
//! unreachable from an external integration-test crate (confirmed: `mod
//! audit;` in `src/librarian/catalog/mod.rs` carries no `pub`), so this file
//! cannot "reach into internals" even by accident.
//!
//! The brief's own fixture was single-repo, single-host — replaced here with
//! a genuinely multi-repo, two-host fixture (see task-5-report.md Correction
//! 1/2 for why): a single-repo fixture cannot express the repo-scoping
//! regression Task 6 fixed (there is nothing to leak *from*), and a
//! single-host fixture cannot express "is a row deleted on host A answerable
//! on host B" at all (there is no second host to answer on).
//!
//! "Host" is simulated via `CODESCOUT_AUDIT_HOST` + a dedicated in-memory
//! `Catalog` per host (host identity is minted once per catalog and persisted
//! in `catalog_meta`, so one catalog == one host for the lifetime of this
//! test). A "pull" is simulated by pointing a *fresh, empty* catalog's
//! `ToolContext` at the *same* repo-root tempdir host A already exported
//! into — exactly what a second machine's clone looks like: an empty local
//! audit table plus committed `.codescout/audit/*.jsonl` files on disk.
//! `temp_env::async_with_vars` (already a project dependency, see
//! `tests/retrieval_unit.rs`) serializes `CODESCOUT_AUDIT_HOST` mutation
//! across this binary's tests on its own global lock — no extra
//! `#[serial_test::serial]` is needed.

use std::sync::Arc;

use codescout::librarian::{
    catalog::{artifact, artifact::ArtifactRow, Catalog},
    current_project::CurrentProject,
    tools::{audit_log, ToolContext},
    workspace::{Root, WorkspaceConfig},
};
use serde_json::{json, Value};
use tempfile::TempDir;

fn ctx_for(
    catalog: Arc<parking_lot::Mutex<Catalog>>,
    repo_root: std::path::PathBuf,
) -> ToolContext {
    ToolContext {
        lsp: codescout::lsp::MockLspProvider::with_client(codescout::lsp::MockLspClient::default()),
        catalog,
        workspace: Arc::new(WorkspaceConfig {
            roots: vec![Root {
                name: "r".into(),
                path: repo_root.clone(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }),
        rules: Arc::new(vec![]),
        temp_guard: codescout::librarian::tools::TempGuardEnv::from_env(),
        embedding: None,
        artifact_store: None,
        current_project: Some(Arc::new(CurrentProject {
            abs_path: repo_root.clone(),
            git_root: repo_root,
            main_root: None,
            umbrella: None,
        })),
    }
}

fn row(id: &str, abs_path: std::path::PathBuf) -> ArtifactRow {
    ArtifactRow {
        id: id.into(),
        abs_path,
        kind: "tracker".into(),
        status: "draft".into(),
        title: None,
        owners: vec![],
        tags: vec![],
        topic: None,
        time_scope: None,
        source: None,
        created_at: 0,
        updated_at: 0,
        file_mtime: 0,
        file_sha256: "".into(),
        confidence: 1.0,
    }
}

/// Seeds `id` as an artifact row under `ctx`'s own repo root — placement
/// under the root is load-bearing for `attribute()`'s `starts_with` check
/// (same load-bearing note as `shard.rs::tests::seed`).
fn seed_artifact(ctx: &ToolContext, id: &str) {
    let root = ctx.current_project.as_ref().unwrap().git_root.clone();
    let cat = ctx.catalog.lock();
    artifact::upsert(&cat, &row(id, root.join(format!("{id}.md")))).unwrap();
}

fn delete_artifact(ctx: &ToolContext, id: &str) {
    let cat = ctx.catalog.lock();
    artifact::delete(&cat, id).unwrap();
}

async fn audit_log_call(ctx: &ToolContext, args: Value) -> Value {
    audit_log::call(ctx, args).await.unwrap()
}

/// Mutation this test kills if it regresses:
/// - `exported == 0` (instead of `>= 1`): the delete row's payload-based
///   attribution breaks (shard.rs::attribute regresses to a live-join-only
///   lookup for deletes too), silently dropping the only audit line a
///   hard-deleted artifact can ever produce.
/// - `foreign == 0`: cross-repo rows sharing one catalog stop being counted
///   (Correction 2 — previously asserted nowhere in this suite).
/// - repo A's shard file containing "repo-b-only": the exact repo-scoping
///   regression Task 6's review fixed (Critical finding) — a foreign row
///   silently written into the wrong repo's committed shard.
/// - repo A's shard file NOT containing "vanished-1": the delete never made
///   it into the file export claims to have written it to.
/// - host B's merged query not seeing the delete row (or seeing it under the
///   wrong host prefix): the cross-machine read/merge path (`read_shards`)
///   regresses — a fresh clone could never answer for another host's history.
/// - host B's query for "repo-b-only" returning any rows: `read_shards`
///   stops scoping to the queried repo's own shard directory.
///
/// A single-repo fixture cannot express the foreign/leak assertions (there is
/// no second repo to leak from); a single-host fixture cannot express the
/// "answerable on host B" assertions (there is no second host to answer on).
#[tokio::test]
async fn a_row_deleted_on_repo_a_is_answerable_on_host_b_after_a_pull() {
    let repo_a = TempDir::new().unwrap();
    let repo_b = TempDir::new().unwrap();

    // One physical catalog plays "host A", which happens to have worked in
    // both repo A and repo B — the shared-catalog, two-repo shape that makes
    // `foreign` non-vacuous.
    let cat_a = Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap()));
    let ctx_a_repo_a = ctx_for(cat_a.clone(), repo_a.path().to_path_buf());
    let ctx_a_repo_b = ctx_for(cat_a, repo_b.path().to_path_buf());

    seed_artifact(&ctx_a_repo_a, "vanished-1");
    delete_artifact(&ctx_a_repo_a, "vanished-1");
    seed_artifact(&ctx_a_repo_b, "repo-b-only");

    let export =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hosta-aaa111"))], async {
            audit_log_call(&ctx_a_repo_a, json!({"export": true})).await
        })
        .await;

    assert!(
        export["exported"].as_i64().unwrap() >= 1,
        "delete row must survive export: {export}"
    );
    assert!(
        export["foreign"].as_i64().unwrap() >= 1,
        "repo B's row must be counted foreign, not silently dropped: {export}"
    );
    let files = export["files"].as_array().unwrap();
    assert_eq!(
        files.len(),
        1,
        "one host, one month, one repo scope: {export}"
    );
    let shard_name = files[0].as_str().unwrap().to_string();
    assert!(
        shard_name.starts_with("hosta-aaa111"),
        "shard file must be named for the minted host, prefix-matched since \
         mint_host_id always appends a random suffix: {shard_name}"
    );

    let shard_path = repo_a.path().join(".codescout/audit").join(&shard_name);
    let shard_contents = std::fs::read_to_string(&shard_path).unwrap();
    assert!(
        !shard_contents.contains("repo-b-only"),
        "repo B's row leaked into repo A's committed shard:\n{shard_contents}"
    );
    assert!(
        shard_contents.contains("vanished-1"),
        "repo A's own delete row is missing from its own shard:\n{shard_contents}"
    );

    // Host B "pulls" repo A: same tempdir (its committed shard file is now
    // on disk), but a brand-new, empty local catalog.
    let cat_b = Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap()));
    let ctx_b_repo_a = ctx_for(cat_b, repo_a.path().to_path_buf());

    let query =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hostb-bbb222"))], async {
            audit_log_call(&ctx_b_repo_a, json!({"row_id": "vanished-1", "limit": 50})).await
        })
        .await;

    let entries = query["entries"].as_array().unwrap();
    let delete_row = entries
        .iter()
        .find(|e| e["op"] == "delete")
        .unwrap_or_else(|| panic!("no delete row for vanished-1 in merged query: {query}"));
    assert!(
        delete_row["host"]
            .as_str()
            .unwrap()
            .starts_with("hosta-aaa111"),
        "the delete row must be attributed to host A, not host B or blank: {delete_row}"
    );
    assert!(
        delete_row["actor"]
            .as_str()
            .unwrap()
            .starts_with("codescout:"),
        "actor must never be blank: {delete_row}"
    );
    assert!(
        query["shards"]["self_host"]
            .as_str()
            .unwrap()
            .starts_with("hostb-bbb222"),
        "host B must resolve its own identity, not host A's: {query}"
    );

    let repo_b_leak =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hostb-bbb222"))], async {
            audit_log_call(&ctx_b_repo_a, json!({"row_id": "repo-b-only", "limit": 50})).await
        })
        .await;
    assert_eq!(
        repo_b_leak["count"].as_i64().unwrap(),
        0,
        "repo B's row must not be answerable by scanning repo A's shard dir: {repo_b_leak}"
    );
}

/// Mutation this test kills: `shard::export` (reached only via the
/// `audit_log(export=true)` surface) starts sourcing rows from
/// `read_shards`'s merged view instead of the local `catalog_audit` table
/// alone — which would make host B re-emit host A's already-committed row
/// under host B's own name, duplicating history across two hosts' shard
/// files. A single-host fixture cannot express this: there is no second
/// host's shard to accidentally re-export.
#[tokio::test]
async fn host_b_does_not_re_export_host_as_rows_as_its_own() {
    let repo_a = TempDir::new().unwrap();

    let cat_a = Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap()));
    let ctx_a = ctx_for(cat_a, repo_a.path().to_path_buf());
    seed_artifact(&ctx_a, "vanished-2");
    delete_artifact(&ctx_a, "vanished-2");

    let export_a =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hosta-ccc333"))], async {
            audit_log_call(&ctx_a, json!({"export": true})).await
        })
        .await;
    assert!(
        export_a["exported"].as_i64().unwrap() >= 1,
        "sanity: host A must have exported something to have a shard for host B to pull: {export_a}"
    );

    let cat_b = Arc::new(parking_lot::Mutex::new(Catalog::open_in_memory().unwrap()));
    let ctx_b = ctx_for(cat_b, repo_a.path().to_path_buf());

    let query_b =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hostb-ddd444"))], async {
            audit_log_call(&ctx_b, json!({"row_id": "vanished-2", "limit": 10})).await
        })
        .await;
    assert!(
        query_b["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["op"] == "delete"),
        "sanity: host B must see host A's row via the merged shard read before we \
         probe the export side: {query_b}"
    );

    let export_b =
        temp_env::async_with_vars([("CODESCOUT_AUDIT_HOST", Some("hostb-ddd444"))], async {
            audit_log_call(&ctx_b, json!({"export": true})).await
        })
        .await;
    assert_eq!(
        export_b["exported"].as_i64().unwrap(),
        0,
        "host B has no local audit rows of its own for repo A — it must not re-export \
         host A's rows as its own: {export_b}"
    );
    assert!(
        export_b["files"].as_array().unwrap().is_empty(),
        "a no-op export must not write a shard file at all: {export_b}"
    );

    let dir = repo_a.path().join(".codescout/audit");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names.len(),
        1,
        "host B's no-op export must not add a file next to host A's: {names:?}"
    );
    assert!(
        names[0].starts_with("hosta-ccc333"),
        "the sole shard file must still be host A's, never duplicated under host B's name: {names:?}"
    );
}
