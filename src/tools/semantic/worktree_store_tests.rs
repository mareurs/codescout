//! Failing tests for mareurs/codescout#34: in a linked worktree on the sqlite-vec
//! backend, `index(action="status")` reports 0 chunks after a successful build, and
//! the worktree client cannot see the main checkout's chunks.
//!
//! Root cause (read from the source, see the issue thread):
//!
//! 1. `IndexStatus::call` asks the store about `ActiveProject::project_id()` — the
//!    worktree's own `project.toml` name (`proj-wt`). The build writes under
//!    `delta_project_id` (`proj@wtname`). Two ids, so status reads an empty file.
//! 2. `RetrievalClient::from_env(root)` puts the sqlite store at
//!    `<root>/.codescout/embeddings`, and `root` is the WORKTREE root. The worktree
//!    branch of `semantic_search` then asks that store whether main has chunks. Main's
//!    chunks live under `<main>/.codescout/embeddings`, so the answer is `false`.
//!
//! The tests drive the same calls production makes and read no process env: the
//! embedder section comes from the fixture's `project.toml`, per
//! `docs/conventions/test-env-isolation.md`.
//!
//! | Test | Today | Production change that turns it green |
//! |---|---|---|
//! | `status_in_a_plain_checkout_counts_its_own_chunks` | passes (positive control) | none |
//! | `status_in_a_linked_worktree_reports_the_delta_the_build_wrote` | FAILS | `IndexStatus` resolves the worktree's delta id |
//! | `a_worktree_client_sees_the_main_checkouts_chunks` | FAILS | worktree client reads main's store |

use super::IndexStatus;
use crate::agent::Agent;
use crate::lsp::LspManager;
use crate::retrieval::client::RetrievalClient;
use crate::retrieval::code_store::{CodeVectorStore, VectorBackend};
use crate::retrieval::embedder::{EmbedOutput, SparseVector};
use crate::retrieval::payload::CodePayload;
use crate::retrieval::sqlite_code_store::SqliteVecCodeStore;
use crate::tools::{Tool, ToolContext};
use serde_json::json;
use std::path::{Path, PathBuf};

/// Git's own name for the linked worktree. Deliberately different from the checkout
/// directory's basename (`wt`) so a key read off the wrong end of the path fails.
const WORKTREE_NAME: &str = "wtname";

struct Layout {
    _dir: tempfile::TempDir,
    main: PathBuf,
    wt: PathBuf,
}

/// `project.toml` for a fixture root. The `[embeddings]` url is never contacted:
/// building the client only constructs the HTTP embedder, the probe is lazy. It is
/// here so `RetrievalClient::from_env` succeeds without ambient env vars.
fn project_toml(name: &str) -> String {
    format!(
        "[project]\nname = \"{name}\"\nlanguages = [\"markdown\"]\n\n\
         [embeddings]\nmodel = \"fixture-model\"\nurl = \"http://127.0.0.1:9/v1\"\n"
    )
}

/// `<tmp>/proj` (main, project `proj`) and `<tmp>/wt` (linked worktree, project
/// `proj-wt`, its own `project.toml` — the shape in the issue).
fn linked_worktree_layout() -> Layout {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("proj");
    let wt = dir.path().join("wt");

    let meta = main.join(".git").join("worktrees").join(WORKTREE_NAME);
    std::fs::create_dir_all(&meta).unwrap();
    std::fs::write(meta.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
    std::fs::create_dir_all(main.join(".codescout")).unwrap();
    std::fs::write(main.join(".codescout/project.toml"), project_toml("proj")).unwrap();

    std::fs::create_dir_all(wt.join(".codescout")).unwrap();
    std::fs::write(wt.join(".git"), format!("gitdir: {}\n", meta.display())).unwrap();
    std::fs::write(wt.join(".codescout/project.toml"), project_toml("proj-wt")).unwrap();

    Layout {
        _dir: dir,
        main,
        wt,
    }
}

/// Both bugs are specific to the per-root sqlite-vec stores. Qdrant keeps every
/// project in one collection, where the same code is correct. Fail loudly rather than
/// let a different backend turn a RED test green for the wrong reason.
fn assert_sqlite_backend() {
    assert!(
        matches!(VectorBackend::resolve(), VectorBackend::SqliteVec),
        "these tests need the sqlite-vec backend; unset CODESCOUT_VECTOR_BACKEND \
         or build without the `server-stack` feature"
    );
}

/// Write one chunk for `project_id` into the sqlite store at `dir` — what a finished
/// `sync_project` / `sync_worktree` leaves behind.
async fn seed_chunk(dir: &Path, project_id: &str, file: &str) {
    let store = SqliteVecCodeStore::at(dir.to_path_buf());
    let payload = CodePayload {
        project_id: project_id.into(),
        file_path: file.into(),
        language: "rust".into(),
        start_line: 1,
        end_line: 2,
        ast_header: String::new(),
        content: format!("content of {file}"),
        content_hash: "h1".into(),
        last_indexed_commit: String::new(),
        chunk_id: format!("{project_id}:{file}:h1"),
    };
    let embedding = EmbedOutput {
        dense: vec![1.0, 0.0, 0.0],
        sparse: SparseVector {
            indices: vec![],
            values: vec![],
        },
    };
    store
        .upsert_chunks("code_chunks", &[(payload, embedding)])
        .await
        .unwrap();
}

async fn ctx_for(root: &Path) -> ToolContext {
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    }
}

/// Positive control. A regular checkout has no delta id, so status must count the
/// chunks stored under the project's own id. This passes today. If it ever fails, the
/// fixture or the embedder setup is broken and the two RED tests below prove nothing.
#[tokio::test]
async fn status_in_a_plain_checkout_counts_its_own_chunks() {
    assert_sqlite_backend();
    let layout = linked_worktree_layout();
    seed_chunk(&layout.main.join(".codescout/embeddings"), "proj", "m.rs").await;

    let ctx = ctx_for(&layout.main).await;
    let result = IndexStatus.call(json!({}), &ctx).await.unwrap();

    assert_eq!(
        result["indexed"], true,
        "a plain checkout with chunks under its own project id must report indexed; \
         if this fails the fixture is wrong, not the worktree path: {result}"
    );
    assert_eq!(result["chunk_count"], 1, "{result}");
}

/// Issue #34, symptom 1. After a worktree build the chunks sit under the delta id.
/// `status` must count them. It reports `indexed: false` because it looks up the
/// worktree's own project name (`proj-wt`) instead.
///
/// Would fail if: `IndexStatus::call` keeps using `ActiveProject::project_id()` on a
/// linked worktree. Passes once it resolves the same `worktree_ids` pair that
/// `sync_worktree` writes under.
#[tokio::test]
async fn status_in_a_linked_worktree_reports_the_delta_the_build_wrote() {
    assert_sqlite_backend();
    let layout = linked_worktree_layout();

    // Preconditions: pin the ids this scenario depends on, so a change to the id
    // derivation fails here and not as a confusing status mismatch.
    let (main_id, delta_id) = crate::retrieval::sync::worktree_ids(&layout.main, &layout.wt);
    assert_eq!(main_id, "proj");
    assert_eq!(delta_id, "proj@wtname");
    RetrievalClient::from_env(Some(&layout.wt))
        .await
        .map(|_| ())
        .expect("fixture must build a retrieval client, or status cannot be observed");

    // What a finished `index(action="build")` leaves behind. The worktree client's code
    // store is the MAIN checkout's (`resolve_code_sqlite_dir`), so the delta lands there,
    // not under the worktree root.
    seed_chunk(
        &layout.main.join(".codescout/embeddings"),
        &delta_id,
        "a.rs",
    )
    .await;

    let ctx = ctx_for(&layout.wt).await;
    let result = IndexStatus.call(json!({}), &ctx).await.unwrap();

    assert_eq!(
        result["indexed"], true,
        "worktree status must count the delta chunks the build wrote under \
         '{delta_id}', not look up the worktree's own project name: {result}"
    );
    assert_eq!(result["chunk_count"], 1, "{result}");
}

/// Issue #34, symptom 2. `semantic_search`'s worktree branch asks
/// `client.project_has_chunks(collection, main_project_id)` on a client built from the
/// worktree root. Main is fully indexed in its own checkout, so the answer must be
/// `true`. Today it opens `<wt>/.codescout/embeddings/proj.db`, empty, and answers
/// `false` — which skips the main query and prints `main_never_indexed_note`.
///
/// Would fail if: the worktree client keeps resolving its sqlite store from the
/// worktree root. Passes once the worktree client can read main's store.
#[tokio::test]
async fn a_worktree_client_sees_the_main_checkouts_chunks() {
    assert_sqlite_backend();
    let layout = linked_worktree_layout();
    let (main_id, _delta_id) = crate::retrieval::sync::worktree_ids(&layout.main, &layout.wt);
    assert_eq!(main_id, "proj");

    // Main is fully indexed, in main's own store.
    seed_chunk(&layout.main.join(".codescout/embeddings"), &main_id, "m.rs").await;

    let client = RetrievalClient::from_env(Some(&layout.wt))
        .await
        .expect("fixture must build a retrieval client for the worktree root");
    let collection = client.config.collection("code_chunks");

    // The exact call `semantic_search` makes for `main_has_chunks`.
    let main_has_chunks = client
        .project_has_chunks(&collection, &main_id)
        .await
        .unwrap();

    assert!(
        main_has_chunks,
        "main's chunks exist in {}; the worktree client looked in {} and found none, \
         so semantic_search skips main and reports it as never indexed",
        layout.main.join(".codescout/embeddings").display(),
        client.config.sqlite_dir.display(),
    );
}
