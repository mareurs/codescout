use super::*;
use crate::agent::Agent;
use std::sync::Arc;
use tempfile::tempdir;

fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
    crate::lsp::LspManager::new_arc()
}

/// Memory writes may return either `"ok"` (no best-effort side-effect
/// failures) or `{"status":"ok", "warnings":[…]}` (one or more non-fatal
/// side effects failed — e.g. no semantic index built in the test fixture
/// so `cross_embed_memory` fails). Both count as a successful write; this
/// helper keeps tests indifferent to which shape they got.
fn assert_memory_write_ok(result: &Value) {
    if result == &json!("ok") {
        return;
    }
    assert_eq!(result["status"], json!("ok"), "unexpected result: {result}");
}

/// Constant-vector test double for `DenseEmbedder`, shared by every test that
/// needs a network-free embedder. Both sides return the same vector, which is
/// enough to exercise store/recall plumbing without asserting on similarity.
struct FixedEmbedder;
#[async_trait::async_trait]
impl crate::retrieval::embedder::DenseEmbedder for FixedEmbedder {
    async fn embed(&self, _text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0])
    }

    async fn embed_document(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        self.embed(text).await
    }

    #[cfg(test)]
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Builds a real `Agent` with no store/embedder override — `semantic_memory_store()`
/// and `memory_embedder()` resolve however the environment resolves them. Only
/// for tests that install their OWN isolation immediately afterward (before any
/// tool call can trigger resolution); everything else must use
/// `test_ctx_with_project()` below.
///
/// One override IS installed here, and deliberately not left to callers: the
/// code-chunk search behind anchor creation. It is a THIRD resolution path —
/// `create_semantic_anchors` embeds via the embedder seam and then searches code
/// through a client neither the store nor the embedder seam covers — so no caller
/// stubbing "its own isolation" was ever closing it. No memory test wants a real
/// one, and unlike the other two seams this override is read per call instead of
/// initialising a cache, so installing it up front constrains nothing.
async fn test_ctx_with_project_raw() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    // Create .codescout dir so MemoryStore::open works
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    agent.set_code_search_for_test(std::sync::Arc::new(NoCodeSearch)
        as std::sync::Arc<dyn crate::retrieval::search::CodeChunkSearch>);
    (
        dir,
        ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        },
    )
}

/// Network-free `CodeChunkSearch` for anchor creation. Returns no hits, which is
/// the same state the suite already tolerated with the retrieval stack offline —
/// so installing it changes no assertion, only where the emptiness comes from.
struct NoCodeSearch;
#[async_trait::async_trait]
impl crate::retrieval::search::CodeChunkSearch for NoCodeSearch {
    async fn search_code(
        &self,
        _project_id: &str,
        _query: &str,
        _opts: crate::retrieval::search::SearchOpts,
    ) -> anyhow::Result<Vec<crate::retrieval::search::Hit>> {
        Ok(Vec::new())
    }
}

/// Regression for docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md:
/// `test_ctx_with_project_raw()` resolves its store/embedder from ambient
/// config, and on a machine with a real local Qdrant + embedder configured in
/// the shell environment, that silently cross-embeds fixture memories into the
/// real `memories` collection. Every test that doesn't need to install its own
/// stub (i.e. almost all of them) should use THIS function instead, which
/// pre-installs a network-free store and embedder before any tool call can
/// trigger the ambient resolution path.
async fn test_ctx_with_project() -> (tempfile::TempDir, ToolContext) {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let (dir, ctx) = test_ctx_with_project_raw().await;
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    (dir, ctx)
}

/// Regression for docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md:
/// the default `test_ctx_with_project()` must resolve a store/embedder that is
/// pre-installed and deterministic, never one resolved from ambient config —
/// on a machine with a real local Qdrant + embedder configured in the shell
/// environment, the ambient path silently cross-embeds fixture memories into
/// the real `memories` collection.
#[tokio::test]
async fn test_ctx_with_project_writes_land_in_an_isolated_store() {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;

    // The default context's store resolves via `Agent::semantic_memory_store()`,
    // which lazily caches whatever it resolves. On a machine with no ambient
    // Qdrant/embedder config, that path can ALREADY succeed today via the
    // always-compiled SqliteVec fallback — round-tripping a write through it
    // would pass whether or not the isolation seam is installed, and so
    // wouldn't distinguish "resolved from ambient config" from "the test
    // double". Assert on the concrete TYPE instead: it must be the specific
    // `InMemorySemanticMemoryStore` this test never constructed itself, proving
    // `test_ctx_with_project()` pre-installed it rather than deferring to
    // whatever the environment happens to resolve.
    let (_dir, ctx) = test_ctx_with_project().await;

    let store = ctx.agent.semantic_memory_store().await.unwrap();
    assert!(
        store
            .as_any()
            .downcast_ref::<InMemorySemanticMemoryStore>()
            .is_some(),
        "the default test context must resolve the in-memory test double, not a store \
         resolved from ambient config"
    );

    let embedder = ctx.agent.memory_embedder().await.unwrap();
    assert!(
        embedder.as_any().downcast_ref::<FixedEmbedder>().is_some(),
        "the default test context must resolve the fixed test embedder, not one resolved \
         from ambient config"
    );
}

/// Regression for the second half of
/// docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md:
/// anchor creation must reach code search through `Agent::code_search`, not build
/// its own `RetrievalClient::from_env`.
///
/// Until 2026-08-30 it built its own, so a memory `write` in tests queried whatever
/// retrieval stack the developer had running even with the embedder and store seams
/// installed. Measured that day: `tools::memory::tests` took 1.16s against a live
/// local embedder and 20.65s against one that accepted connections and never
/// answered — 76 passing either way, so the coupling was invisible to the gate and
/// showed up only as an unexplained slowdown, in the counter-intuitive direction
/// (slower when the stack is DOWN).
///
/// **The mutation this must die on:** restore
/// `RetrievalClient::from_env(Some(&root))` at the `create_semantic_anchors` call
/// site. `calls` then stays 0, because the real client is used instead of the seam.
/// Asserting `> 0` rather than on any search result is deliberate — the assertion
/// is about which code path ran, and a hit-count assertion would pass for a real
/// client that happened to return nothing.
#[tokio::test]
async fn a_memory_write_reaches_code_search_through_the_seam() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct CountingCodeSearch {
        calls: Arc<AtomicUsize>,
    }
    #[async_trait::async_trait]
    impl crate::retrieval::search::CodeChunkSearch for CountingCodeSearch {
        async fn search_code(
            &self,
            _project_id: &str,
            _query: &str,
            _opts: crate::retrieval::search::SearchOpts,
        ) -> anyhow::Result<Vec<crate::retrieval::search::Hit>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    let (_dir, ctx) = test_ctx_with_project().await;
    let calls = Arc::new(AtomicUsize::new(0));
    // Overwrites the default `NoCodeSearch` the raw helper installed — the reason
    // this seam replaces rather than set-onces.
    ctx.agent
        .set_code_search_for_test(Arc::new(CountingCodeSearch {
            calls: calls.clone(),
        })
            as Arc<dyn crate::retrieval::search::CodeChunkSearch>);

    Memory
        .call(
            json!({ "action": "write", "topic": "seam-probe",
                    "content": "# Seam probe\n\nContent long enough to anchor." }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        calls.load(Ordering::SeqCst) > 0,
        "a memory write must reach code search through Agent::code_search; a count \
         of 0 means create_semantic_anchors built its own RetrievalClient again and \
         the suite is back to talking to the developer's live retrieval stack"
    );
}

async fn test_ctx_no_project() -> ToolContext {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Network-free stubs, installed before any tool call can trigger ambient
    // resolution — see test_ctx_with_project's doc comment for why every
    // real-Agent fixture needs this, not just the two named helpers. The
    // code-search override is a THIRD, separate resolution path (see
    // test_ctx_with_project_raw's doc comment) that neither the embedder nor
    // the store seam covers.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    ctx.agent.set_code_search_for_test(
        Arc::new(NoCodeSearch) as Arc<dyn crate::retrieval::search::CodeChunkSearch>
    );
    ctx
}

#[tokio::test]
async fn write_and_read_roundtrip() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let result = Memory
        .call(
            json!({
                "action": "write",
                "topic": "test-topic",
                "content": "hello memory"
            }),
            &ctx,
        )
        .await
        .unwrap();
    // `write` answers with a bare `"ok"` (the no-echo write convention), or with
    // `{"status": "ok", "warnings": [...]}` when an OPTIONAL step attached one —
    // semantic anchor creation needs a reachable embedder and warns rather than
    // failing when it is not.
    //
    // Assert on the status alone. This used to be `assert_eq!(result, "ok")`,
    // which failed on the object form, so an unreachable embedder was
    // indistinguishable from a code regression: two stale env vars pointing at a
    // dead port turned this into a red gate that read as a broken build.
    // A warning from an optional service is not a write failure; the functional
    // check is the read assertion below.
    // docs/issues/archive/2026-08-27-cargo-test-fails-from-bash-passes-via-run-command.md
    let status = result
        .as_str()
        .or_else(|| result["status"].as_str())
        .unwrap_or_else(|| {
            panic!("write returned neither a bare status string nor a `status` field: {result}")
        });
    assert_eq!(
        status, "ok",
        "write should report ok; full result: {result}"
    );

    let result = Memory
        .call(json!({ "action": "read", "topic": "test-topic" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["content"], "hello memory");
}

/// The one operator rule that can fire end-to-end, driven through the real call path.
///
/// This closes a gap that the fix for
/// `docs/issues/archive/2026-08-28-triggered-operator-rules-route-nothing-in-production.md`
/// named in its own `unverified:` field and deliberately left open: *"no test drives a
/// real `memory(action="write")` call through `call_content` and asserts the OP-3 block
/// appears."* Until this existed the chain was covered by two tests meeting at a verified
/// point — the real tool returns `Some(key)`, and `route()` delivers given a `Some`
/// selector — which is not the same claim as the composition working. Every link can pass
/// while the joint does not.
///
/// **It must be `call_content`, not `call`.** `call` returns the tool's own JSON and never
/// consults the router at all: the selector projection, the `if selector.is_some()` guard,
/// the once-per-session ledger stamp and the block rendering all live in `call_content`. A
/// test against `call` would be green in a world where routing is entirely dead, which is
/// the world that shipped for three days.
///
/// The write has to genuinely succeed. `call_content` propagates the tool's error with
/// `?` before it reaches the routing block, so a failing write produces no operator
/// content and this test would fail for a reason unrelated to routing — hence
/// `test_ctx_with_project`, not a bare context.
///
/// **Mutation that must kill this:** make `Tool::selector_key`'s default return `None`
/// again (`src/tools/core/types.rs`). `Memory` no longer overrides it — as of 2026-09-01
/// the inverted default *is* the mechanism — so that one edit removes the key, and
/// `Shape::matches` reads `None` as "cannot match". `every_registered_tool_supplies_a_selector_key`
/// (`src/server.rs`) fires on the same mutation across all 21 registered tools; this test
/// is the one that proves the delivery those keys exist for.
#[tokio::test]
async fn a_real_memory_write_call_delivers_op_3() {
    let (_dir, ctx) = test_ctx_with_project().await;

    let out = Memory
        .call_content(
            serde_json::json!({
                "action": "write",
                "topic": "op3-routing-probe",
                "content": "A real write, so `call_content` reaches the router instead of \
                            short-circuiting on the error path.",
            }),
            &ctx,
        )
        .await
        .expect("the write must succeed — call_content's `?` would skip the router");

    let text = out
        .iter()
        .filter_map(|c| c.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        text.contains("operator-rule OP-3"),
        "a real memory(action=\"write\") must deliver OP-3 through call_content — this is \
         the composition the two unit tests could not establish. Got: {text}"
    );
    // The marker alone is not the rule: a mutation that emitted the comment with an
    // empty or wrong body would otherwise ship green. Same reasoning as
    // `a_triggered_operator_rule_is_delivered_once_per_session`, which asserts against
    // the stub rather than the real tool.
    assert!(
        text.contains("codescout memory or a tracker"),
        "the OP-3 block arrived without its body — marker present, rule text absent: {text}"
    );
}

#[tokio::test]
async fn read_missing_returns_null() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let err = Memory
        .call(json!({ "action": "read", "topic": "nonexistent" }), &ctx)
        .await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("nonexistent"), "got: {msg}");
}

#[tokio::test]
async fn list_after_writes() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({ "action": "write", "topic": "b-topic", "content": "b" }),
            &ctx,
        )
        .await
        .unwrap();
    Memory
        .call(
            json!({ "action": "write", "topic": "a-topic", "content": "a" }),
            &ctx,
        )
        .await
        .unwrap();

    let result = Memory
        .call(json!({ "action": "list" }), &ctx)
        .await
        .unwrap();
    let topics: Vec<&str> = result["topics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(topics, vec!["a-topic", "b-topic"]);
}

#[tokio::test]
async fn delete_removes_entry() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({ "action": "write", "topic": "to-delete", "content": "bye" }),
            &ctx,
        )
        .await
        .unwrap();
    Memory
        .call(json!({ "action": "delete", "topic": "to-delete" }), &ctx)
        .await
        .unwrap();

    let err = Memory
        .call(json!({ "action": "read", "topic": "to-delete" }), &ctx)
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn memory_delete_removes_anchor_sidecar() {
    use crate::memory::anchors::anchor_path_for_topic;

    let (dir, ctx) = test_ctx_with_project().await;

    // Write a memory; this also creates the path-anchor sidecar
    // (see write_creates_anchor_sidecar).
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "anchor-leak-fixture",
                "content": "anchors should not leak on delete"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let memories_dir = dir.path().join(".codescout/memories");
    let sidecar = anchor_path_for_topic(&memories_dir, "anchor-leak-fixture");

    // Sidecar may or may not exist depending on whether path-anchor
    // computation found matching files in the empty fixture project.
    // Pre-create one so the test is deterministic.
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(&sidecar, "anchors = []\n").unwrap();
    assert!(sidecar.exists(), "fixture sidecar must exist pre-delete");

    Memory
        .call(
            json!({
                "action": "delete",
                "topic": "anchor-leak-fixture"
            }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        !sidecar.exists(),
        "anchor sidecar should be removed when its memory is deleted"
    );
}

#[tokio::test]
async fn memory_forget_delegates_to_store() {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::{MemoryFilter, SemanticMemoryStore};
    use crate::retrieval::memory_payload::{point_id_for, SemanticMemory};
    use std::sync::Arc;

    let (_dir, ctx) = test_ctx_with_project_raw().await;

    // Swap in the in-memory stub before any tool call — once Agent caches a
    // store via semantic_memory_store(), set_semantic_memory_store_for_test
    // would fail with SetError.
    let stub: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
    ctx.agent
        .set_semantic_memory_store_for_test(stub.clone() as Arc<dyn SemanticMemoryStore>)
        .map_err(|_| ())
        .expect("set stub");

    // Resolve the active project_id (matches what the tool will look up).
    let project_id = ctx
        .agent
        .with_project(|p| Ok(p.config.project.name.clone()))
        .await
        .unwrap();

    // Pre-seed a memory with a known id.
    let id = point_id_for(&project_id, "unstructured", "to-forget");
    let mem = SemanticMemory {
        project_id: project_id.clone(),
        bucket: "unstructured".into(),
        title: "to-forget".into(),
        content: "doomed".into(),
        anchors: vec![],
        created_at: "2026-05-13T00:00:00Z".into(),
        updated_at: "2026-05-13T00:00:00Z".into(),
    };
    stub.upsert(&mem, &[0.0_f32; 8]).await.unwrap();
    assert_eq!(
        stub.list(&project_id, MemoryFilter::default())
            .await
            .unwrap()
            .len(),
        1,
        "fixture must seed exactly one memory"
    );

    // Forget via the unified tool — exercises the
    // set_semantic_memory_store_for_test seam end-to-end.
    Memory
        .call(json!({ "action": "forget", "id": id.to_string() }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        stub.list(&project_id, MemoryFilter::default())
            .await
            .unwrap()
            .len(),
        0,
        "forget must remove the memory from the stub"
    );

    // Idempotency: a second forget on the same id must not error.
    Memory
        .call(json!({ "action": "forget", "id": id.to_string() }), &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn memory_remember_then_recall_e2e_via_test_seams() {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let (_dir, ctx) = test_ctx_with_project_raw().await;

    // Stub the dense embedder so memory ops don't need a live retrieval
    // stack. A constant vector makes every recall match every memory at
    // cosine 1.0, which is enough to exercise the round-trip without
    // requiring real semantic similarity.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");

    // Stub the semantic memory store too — same seam pattern as 4g.
    let stub: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
    ctx.agent
        .set_semantic_memory_store_for_test(stub.clone() as Arc<dyn SemanticMemoryStore>)
        .map_err(|_| ())
        .expect("set store");

    // remember — exercises the full tool path: project_id resolution,
    // dense embedding via the seam, point_id derivation, upsert.
    Memory
        .call(
            json!({
                "action": "remember",
                "content": "shipped fix for migration default path",
                "title": "step-7-cli-fix",
                "bucket": "unstructured",
            }),
            &ctx,
        )
        .await
        .unwrap();

    // recall — same path in reverse: embed query, dense KNN, payload decode.
    let result = Memory
        .call(
            json!({
                "action": "recall",
                "query": "migration default path",
                "limit": 3,
            }),
            &ctx,
        )
        .await
        .unwrap();

    let hits = result["results"]
        .as_array()
        .unwrap_or_else(|| panic!("expected `results` array, got: {result}"));
    assert_eq!(hits.len(), 1, "expected exactly one hit; got: {result}");
    assert_eq!(hits[0]["title"], "step-7-cli-fix");
    assert_eq!(hits[0]["bucket"], "unstructured");
    assert!(hits[0]["content"]
        .as_str()
        .unwrap()
        .contains("migration default path"));
}
/// Regression for the workspace-pin gap: cross-embedding a memory under a
/// `workspace=` pin must store it under the PINNED project_id, not the
/// session-default project. Previously `cross_embed_memory` resolved the
/// project via `active_project()` (the session default), silently landing
/// the semantic memory in the wrong project.
#[tokio::test]
async fn cross_embed_memory_stores_under_pinned_project_not_session_default() {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::{MemoryFilter, SemanticMemoryStore};
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    // Session-default project (from Agent::new in the helper).
    let (_default_dir, mut ctx) = test_ctx_with_project_raw().await;

    // A second, DISTINCT project we will pin via workspace_override.
    let pinned_dir = tempdir().unwrap();
    std::fs::create_dir_all(pinned_dir.path().join(".codescout")).unwrap();
    std::fs::write(
        pinned_dir.path().join(".codescout").join("project.toml"),
        "[project]\nname = \"pinned-proj\"\n",
    )
    .unwrap();
    // Make the pinned workspace resident so the pin can resolve it.
    ctx.agent
        .ensure_resident(pinned_dir.path().to_path_buf(), None)
        .await
        .unwrap();

    // Network-free stubs: constant embedder + in-memory store.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    let stub: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
    ctx.agent
        .set_semantic_memory_store_for_test(stub.clone() as Arc<dyn SemanticMemoryStore>)
        .map_err(|_| ())
        .expect("set store");

    let default_id = ctx
        .agent
        .with_project(|p| Ok(p.config.project.name.clone()))
        .await
        .unwrap();
    assert_ne!(default_id, "pinned-proj", "test setup: names must differ");

    // Cross-embed a memory UNDER THE PIN.
    ctx.workspace_override = Some(pinned_dir.path().to_path_buf());
    super::cross_embed_memory(&ctx, "pinned-note", "belongs to the pinned project")
        .await
        .unwrap();

    // It must land under the pinned project_id, not the session default.
    assert_eq!(
        stub.list("pinned-proj", MemoryFilter::default())
            .await
            .unwrap()
            .len(),
        1,
        "cross-embed must store under the pinned project_id"
    );
    assert_eq!(
        stub.list(&default_id, MemoryFilter::default())
            .await
            .unwrap()
            .len(),
        0,
        "cross-embed must NOT store under the session-default project_id"
    );
}

/// CONSERVATION: segmenting must not drop or duplicate a single character.
///
/// This is the invariant that matters, because the defect being fixed IS silent
/// content loss — a segmenter that quietly dropped a tail would remove the error
/// while preserving the data loss, which is strictly worse than the bug. Checked
/// across budgets that divide the input evenly and unevenly, and against content
/// whose lines fall both under and over the budget.
#[test]
fn segmenting_never_drops_or_duplicates_content() {
    let cases: Vec<String> = vec![
        "short".into(),
        "a\nb\nc\n".into(),
        "line of moderate length\n".repeat(50),
        // One line far longer than any budget below: no boundary can help, so this
        // exercises the hard-split path.
        format!("prefix\n{}\nsuffix\n", "x".repeat(5000)),
        "no trailing newline at all".into(),
        "\n\n\n".into(),
    ];
    for content in &cases {
        for budget in [1usize, 7, 64, 1000, 100_000] {
            let segs = crate::embed::document::segment_for_budget(content, budget);
            assert_eq!(
                segs.concat(),
                *content,
                "budget={budget} must preserve content exactly (len {})",
                content.len()
            );
            for s in &segs {
                assert!(
                    s.chars().count() <= budget,
                    "segment of {} chars exceeds budget {budget}",
                    s.chars().count()
                );
            }
        }
    }
}

/// Content at or under the budget is ONE segment — the pre-fix path, byte-identical.
///
/// Without this, a conservative budget would quietly start pooling ordinary memories,
/// trading a data-loss bug for a retrieval-quality one.
#[test]
fn content_within_budget_is_never_segmented() {
    for content in ["", "one line", "two\nlines\n"] {
        let segs = crate::embed::document::segment_for_budget(content, 1000);
        assert!(
            segs.len() <= 1,
            "{content:?} fits the budget and must not be split: {segs:?}"
        );
    }
}

/// Re-normalising after pooling is load-bearing, not cosmetic.
///
/// Two orthogonal unit vectors mean-pool to norm 1/sqrt(2) ≈ 0.707. The sqlite-vec
/// store queries with `embedding MATCH vec_f32(?)`, whose metric is L2 — so an
/// unnormalised pooled vector would sit measurably closer to the origin than any
/// unpooled embedding, putting every segmented memory further from every query in
/// proportion to how varied its content is. A test that only asserted "a vector came
/// back" would pass against that.
#[test]
fn pooling_returns_a_unit_vector_and_rejects_ragged_input() {
    let pooled =
        crate::embed::document::mean_pool_normalized(&[vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
    let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm - 1.0).abs() < 1e-5,
        "pooled vector must be unit-norm, got {norm}"
    );
    assert!(
        (pooled[0] - pooled[1]).abs() < 1e-5,
        "orthogonal inputs must pool symmetrically, got {pooled:?}"
    );

    assert!(
        crate::embed::document::mean_pool_normalized(&[]).is_err(),
        "nothing to pool must error, not return a bogus vector"
    );
    assert!(
        crate::embed::document::mean_pool_normalized(&[vec![1.0, 0.0], vec![1.0]]).is_err(),
        "ragged dimensions must error rather than silently produce a truncated vector"
    );

    // Degenerate all-zero input must not divide by zero into NaNs — a NaN vector
    // poisons the index silently, where a zero vector reads as "no signal".
    let zero = crate::embed::document::mean_pool_normalized(&[vec![0.0, 0.0]]).unwrap();
    assert!(
        zero.iter().all(|x| x.is_finite()),
        "all-zero input must not produce NaNs: {zero:?}"
    );
}

/// The budget is derived from the CONFIGURED MODEL, not a constant — and the two
/// backends this bug spans must land far apart.
#[test]
fn the_embedding_budget_tracks_the_model() {
    // The reporter's backend. fastembed truncates at 512 and the table clamps to
    // min(256, 512) = 256 tokens.
    let mini = crate::embed::chunk_size_for_model("local:AllMiniLML6V2Q");
    // This stack's backend, measured at 2048 tokens on 2026-08-26.
    let coderank = crate::embed::chunk_size_for_model("CodeRankEmbed");
    assert!(mini > 0 && coderank > 0);
    assert!(
        coderank > mini * 3,
        "CodeRankEmbed's measured 2048-token window must yield a far larger budget \
         than AllMiniLM's 256 — a shared constant would make these equal: \
         {coderank} vs {mini}"
    );
}

/// End to end through the embedder seam: under budget is one call, over budget pools
/// into a still-unit-norm vector.
#[tokio::test]
async fn under_budget_makes_one_call_and_over_budget_pools_to_unit_norm() {
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct CountingEmbedder {
        calls: AtomicUsize,
    }
    #[async_trait::async_trait]
    impl DenseEmbedder for CountingEmbedder {
        async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
            self.embed_document(text).await
        }
        async fn embed_document(&self, text: &str) -> anyhow::Result<Vec<f32>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            // Real backends return unit-norm vectors (measured 2026-08-26:
            // CodeRankEmbed L2 = 1.000000). Vary the direction by input length so
            // pooling has something non-trivial to average.
            let n = (text.len() % 7) as f32 + 1.0;
            let norm = (n * n + 1.0).sqrt();
            Ok(vec![n / norm, 1.0 / norm])
        }
        #[cfg(test)]
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    let short = CountingEmbedder {
        calls: AtomicUsize::new(0),
    };
    let v = crate::embed::document::embed_document_pooled(&short, "well under budget", 1000)
        .await
        .unwrap();
    assert_eq!(
        short.calls.load(Ordering::SeqCst),
        1,
        "under budget must be exactly ONE call — the pre-fix path, unchanged"
    );
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!((norm - 1.0).abs() < 1e-5, "got {norm}");

    let long_embedder = CountingEmbedder {
        calls: AtomicUsize::new(0),
    };
    let long = "some line of text\n".repeat(300); // ~5400 chars
    let v2 = crate::embed::document::embed_document_pooled(&long_embedder, &long, 500)
        .await
        .unwrap();
    let calls = long_embedder.calls.load(Ordering::SeqCst);
    assert!(
        calls >= 10,
        "a ~5400-char memory at budget 500 must segment; got {calls} call(s)"
    );
    let norm2: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(
        (norm2 - 1.0).abs() < 1e-5,
        "a pooled vector must still be unit-norm, got {norm2}"
    );
}

#[tokio::test]
async fn memory_recall_signals_has_more_when_capped() {
    // Silent-cap regression: a limit-capped recall must flag that more memories
    // match. docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let (_dir, ctx) = test_ctx_with_project_raw().await;
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    let stub: Arc<InMemorySemanticMemoryStore> = Arc::new(InMemorySemanticMemoryStore::new());
    ctx.agent
        .set_semantic_memory_store_for_test(stub.clone() as Arc<dyn SemanticMemoryStore>)
        .map_err(|_| ())
        .expect("set store");

    for i in 0..4 {
        Memory
            .call(
                json!({
                    "action": "remember",
                    "content": format!("memory number {i}"),
                    "title": format!("t{i}"),
                    "bucket": "unstructured",
                }),
                &ctx,
            )
            .await
            .unwrap();
    }

    let result = Memory
        .call(
            json!({"action": "recall", "query": "memory", "limit": 2}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        result["results"].as_array().unwrap().len(),
        2,
        "capped to limit"
    );
    assert_eq!(
        result["has_more"],
        json!(true),
        "4 memories, limit 2 -> has_more"
    );
}

#[tokio::test]
async fn tools_error_without_active_project() {
    let ctx = test_ctx_no_project().await;
    assert!(Memory
        .call(
            json!({ "action": "write", "topic": "x", "content": "y" }),
            &ctx
        )
        .await
        .is_err());
    assert!(Memory
        .call(json!({ "action": "read", "topic": "x" }), &ctx)
        .await
        .is_err());
    assert!(Memory
        .call(json!({ "action": "list" }), &ctx)
        .await
        .is_err());
    assert!(Memory
        .call(json!({ "action": "delete", "topic": "x" }), &ctx)
        .await
        .is_err());
}

#[tokio::test]
async fn nested_topic_works() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "debugging/async-patterns",
                "content": "avoid blocking the runtime"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = Memory
        .call(
            json!({ "action": "read", "topic": "debugging/async-patterns" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["content"], "avoid blocking the runtime");
}

#[test]
fn list_memories_format_compact() {
    use serde_json::json;
    let tool = Memory;
    let r = json!({ "topics": ["a", "b", "c"] });
    let t = tool.format_compact(&r).unwrap();
    assert!(t.contains("3"), "got: {t}");
}

/// The live `memory` tool's schema carries the private-store fields.
///
/// This replaces four near-identical tests that asserted the same two properties
/// on `WriteMemory` / `ReadMemory` / `DeleteMemory` / `ListMemories` — four
/// `impl Tool` blocks `src/server.rs` never registered, so no client was ever
/// served those schemas. Four green assertions about a schema nobody receives
/// carried no information about the one that ships. There is one schema, so
/// there is one test.
///
/// docs/issues/archive/2026-08-27-unregistered-memory-tool-structs-read-as-the-live-tool.md
#[test]
fn memory_schema_carries_the_private_store_fields() {
    let schema = Memory.input_schema();
    assert!(schema["properties"]["private"].is_object());
    assert_eq!(schema["properties"]["private"]["type"], "boolean");
    assert!(schema["properties"]["include_private"].is_object());
    assert_eq!(schema["properties"]["include_private"]["type"], "boolean");
}

#[tokio::test]
async fn write_private_goes_to_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({"action": "write", "topic": "prefs", "content": "verbose", "private": true}),
            &ctx,
        )
        .await
        .unwrap();
    // not in shared store
    let shared = ctx
        .agent
        .with_project(|p| p.memory.read("prefs"))
        .await
        .unwrap();
    assert_eq!(shared, None);
    // is in private store
    let private = ctx
        .agent
        .with_project(|p| p.private_memory.read("prefs"))
        .await
        .unwrap();
    assert_eq!(private, Some("verbose".to_string()));
}

// ── shrink guard (CM-6) ─────────────────────────────────────────────────
//
// `write` replaces a topic wholesale. Writing two new sections to a
// 17-section memory deleted the other fifteen and returned `{"status":"ok"}`.
// docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md

/// 751 bytes — the size measured in the reproduction, and comfortably over
/// `memory::SHRINK_GUARD_MIN_BYTES`.
fn ten_section_memory() -> String {
    let mut s = String::from("# Big memory\n");
    for i in 1..=10 {
        s.push_str(&format!(
            "\n## Section {i}\nLoad-bearing content that must not vanish silently.\n"
        ));
    }
    assert!(s.len() > crate::memory::SHRINK_GUARD_MIN_BYTES);
    s
}

#[tokio::test]
async fn write_refuses_a_destructive_overwrite() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let original = ten_section_memory();
    Memory
        .call(
            json!({"action": "write", "topic": "big", "content": original}),
            &ctx,
        )
        .await
        .unwrap();

    let err = Memory
        .call(
            json!({"action": "write", "topic": "big",
                   "content": "# Big memory\n\n## Section 11\nJust the new one.\n"}),
            &ctx,
        )
        .await
        .expect_err("replacing ten sections with one must be refused");

    let msg = err.to_string();
    assert!(
        msg.contains("memory-shrink guard"),
        "the error must name the guard so the caller can find it; got: {msg}"
    );

    // The assertion that actually matters. An error that still writes is worse
    // than no guard at all — the caller believes it failed AND has lost the
    // data. Mutating the guard to warn-and-proceed must fail HERE, not above.
    let on_disk = ctx
        .agent
        .with_project(|p| p.memory.read("big"))
        .await
        .unwrap();
    assert_eq!(
        on_disk,
        Some(original),
        "a refused write must leave the topic byte-identical"
    );
}

#[tokio::test]
async fn write_with_force_permits_the_overwrite() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({"action": "write", "topic": "big", "content": ten_section_memory()}),
            &ctx,
        )
        .await
        .unwrap();

    Memory
        .call(
            json!({"action": "write", "topic": "big", "content": "pruned", "force": true}),
            &ctx,
        )
        .await
        .expect("force=true is the documented escape and must work");

    let on_disk = ctx
        .agent
        .with_project(|p| p.memory.read("big"))
        .await
        .unwrap();
    assert_eq!(on_disk, Some("pruned".to_string()));
}

/// The private store is a different directory, so a guard hoisted above the
/// private/project branch would check the wrong file. Pinned here because
/// that mistake is invisible: it silently guards nothing on this path.
#[tokio::test]
async fn write_guard_also_covers_the_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let original = ten_section_memory();
    Memory
        .call(
            json!({"action": "write", "topic": "big", "content": original, "private": true}),
            &ctx,
        )
        .await
        .unwrap();

    let err = Memory
        .call(
            json!({"action": "write", "topic": "big", "content": "x", "private": true}),
            &ctx,
        )
        .await
        .expect_err("the private store must be guarded too");
    assert!(err.to_string().contains("memory-shrink guard"));

    let on_disk = ctx
        .agent
        .with_project(|p| p.private_memory.read("big"))
        .await
        .unwrap();
    assert_eq!(on_disk, Some(original));
}

/// A guard that blocks first writes would make the tool unusable. The
/// shrink check has nothing to compare against, so it must stay silent.
#[tokio::test]
async fn write_guard_is_silent_on_a_first_write() {
    let (_dir, ctx) = test_ctx_with_project().await;
    Memory
        .call(
            json!({"action": "write", "topic": "brand-new", "content": "x"}),
            &ctx,
        )
        .await
        .expect("a first write destroys nothing and must not be blocked");
}

#[tokio::test]
async fn schema_advertises_force() {
    let schema = Memory.input_schema();
    assert_eq!(schema["properties"]["force"]["type"], "boolean");
    // The description must say REPLACES, not "overwrites" — the wrong mental
    // model ("write appends") is what the guard exists to catch, so the schema
    // has to correct it at the point of use.
    let desc = schema["properties"]["force"]["description"]
        .as_str()
        .unwrap();
    assert!(desc.contains("REPLACES"), "got: {desc}");
}

#[tokio::test]
async fn read_private_reads_from_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.private_memory.write("wip", "issue-42"))
        .await
        .unwrap();
    let result = Memory
        .call(
            json!({"action": "read", "topic": "wip", "private": true}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["content"], "issue-42");
}

#[tokio::test]
async fn read_private_does_not_see_shared() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.memory.write("shared-topic", "data"))
        .await
        .unwrap();
    // private store doesn't have the topic → should error, not return shared data
    let err = Memory
        .call(
            json!({"action": "read", "topic": "shared-topic", "private": true}),
            &ctx,
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
async fn delete_private_removes_from_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.private_memory.write("tmp", "gone"))
        .await
        .unwrap();
    Memory
        .call(
            json!({"action": "delete", "topic": "tmp", "private": true}),
            &ctx,
        )
        .await
        .unwrap();
    let result = ctx
        .agent
        .with_project(|p| p.private_memory.read("tmp"))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn delete_private_does_not_affect_shared_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.memory.write("tmp", "keep"))
        .await
        .unwrap();
    Memory
        .call(
            json!({"action": "delete", "topic": "tmp", "private": true}),
            &ctx,
        )
        .await
        .unwrap();
    let result = ctx
        .agent
        .with_project(|p| p.memory.read("tmp"))
        .await
        .unwrap();
    assert_eq!(result, Some("keep".to_string()));
}

#[tokio::test]
async fn list_memories_default_returns_topics_key() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.memory.write("arch", "..."))
        .await
        .unwrap();
    let result = Memory
        .call(json!({ "action": "list" }), &ctx)
        .await
        .unwrap();
    assert!(result["topics"].is_array());
    assert!(result["shared"].is_null()); // old shape preserved by default
}

#[tokio::test]
async fn list_memories_include_private_returns_shared_and_private_keys() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| {
            p.memory.write("arch", "...")?;
            p.private_memory.write("prefs", "...")?;
            Ok(())
        })
        .await
        .unwrap();
    let result = Memory
        .call(json!({"action": "list", "include_private": true}), &ctx)
        .await
        .unwrap();
    assert!(result["shared"].is_array());
    assert!(result["private"].is_array());
    assert!(result["topics"].is_null()); // new shape, no "topics" key
    let shared: Vec<_> = result["shared"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(shared.contains(&"arch"));
    let private: Vec<_> = result["private"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    assert!(private.contains(&"prefs"));
}

#[tokio::test]
async fn list_memories_include_private_empty_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.memory.write("arch", "..."))
        .await
        .unwrap();
    let result = Memory
        .call(json!({"action": "list", "include_private": true}), &ctx)
        .await
        .unwrap();
    let private = result["private"].as_array().unwrap();
    assert!(private.is_empty());
}

// --- format_list_memories / format_read_memory tests ---

#[test]
fn format_list_memories_shows_topic_names() {
    let result = serde_json::json!({
        "topics": ["architecture", "conventions", "gotchas"]
    });
    let out = format_list_memories(&result);
    assert!(out.contains("architecture"), "should list topic names");
    assert!(out.contains("conventions"), "should list topic names");
    assert!(out.contains("gotchas"), "should list topic names");
    assert!(out.contains('3'), "should include count");
}

#[test]
fn format_list_memories_empty() {
    let result = serde_json::json!({ "topics": [] });
    let out = format_list_memories(&result);
    assert!(out.contains('0'), "should say 0 topics");
}

#[test]
fn format_list_memories_include_private_shows_both() {
    let result = serde_json::json!({ "shared": ["arch", "conventions"], "private": ["prefs"] });
    let out = format_list_memories(&result);
    assert!(out.contains("2 shared"));
    assert!(out.contains("1 private"));
    assert!(out.contains("arch"));
    assert!(out.contains("prefs"));
}

#[test]
fn format_list_memories_include_private_empty_private() {
    let result = serde_json::json!({ "shared": ["arch"], "private": [] });
    let out = format_list_memories(&result);
    assert!(out.contains("1 shared"));
    assert!(out.contains("0 private"));
}

#[test]
fn format_read_memory_shows_content() {
    let result = serde_json::json!({
        "content": "## Layers\n\nAgent → Server → Tools"
    });
    let out = format_read_memory(&result);
    assert!(out.contains("Layers"), "should show content");
    assert!(
        out.contains("Agent → Server → Tools"),
        "should show full content"
    );
}

#[test]
fn memory_declares_output_form_text() {
    // Pinned wire contract: small `memory` results (topic lists / read content)
    // render via the compact text form, not pretty JSON. Both helpers are
    // lossless (all topic names, full content verbatim), so the small path is
    // safe to flip.
    use crate::tools::{OutputForm, Tool};
    assert_eq!(Memory.output_form(), OutputForm::Text);
}

#[tokio::test]
async fn memory_write_and_read_via_dispatch() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;

    // write
    let w = tool
        .call(
            json!({ "action": "write", "topic": "test/key", "content": "hello" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_memory_write_ok(&w);

    // read
    let r = tool
        .call(json!({ "action": "read", "topic": "test/key" }), &ctx)
        .await
        .unwrap();
    assert_eq!(r["content"], json!("hello"));

    drop(dir);
}

#[tokio::test]
async fn memory_read_accepts_name_alias_for_topic() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;

    let w = tool
        .call(
            json!({ "action": "write", "topic": "alias-test", "content": "hi" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_memory_write_ok(&w);

    let r = tool
        .call(json!({ "action": "read", "name": "alias-test" }), &ctx)
        .await
        .unwrap();
    assert_eq!(r["content"], json!("hi"));

    let r2 = tool
        .call(json!({ "action": "read", "key": "alias-test" }), &ctx)
        .await
        .unwrap();
    assert_eq!(r2["content"], json!("hi"));

    drop(dir);
}

#[tokio::test]
async fn memory_read_missing_topic_and_aliases_returns_recoverable() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;

    let err = tool
        .call(json!({ "action": "read" }), &ctx)
        .await
        .unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("topic"), "error should mention topic: {msg}");

    drop(dir);
}

#[tokio::test]
async fn memory_read_missing_topic_embeds_available_and_suggestions() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;

    for topic in ["iel-solver-debug", "iel-solver-config", "prompt-hamsa"] {
        let w = tool
            .call(
                json!({ "action": "write", "topic": topic, "content": "x" }),
                &ctx,
            )
            .await
            .unwrap();
        assert_memory_write_ok(&w);
    }

    let err = tool
        .call(
            json!({ "action": "read", "topic": "iel-solver-toolkit" }),
            &ctx,
        )
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<RecoverableError>()
        .expect("missing-topic read should be a RecoverableError");
    assert!(
        rec.message.contains("iel-solver-toolkit"),
        "message should name the missing topic: {}",
        rec.message
    );

    // The full store is previewed inline so the caller needn't run `list`.
    let available = rec.extra["available_topics"]
        .as_array()
        .expect("available_topics should be an array");
    assert_eq!(available.len(), 3, "all topics previewed: {available:?}");
    assert!(available.iter().any(|t| t == "prompt-hamsa"));

    // Token overlap surfaces the siblings, ranked alphabetically on the tie,
    // and excludes the unrelated topic.
    let suggestions = rec.extra["did_you_mean"]
        .as_array()
        .expect("did_you_mean should be present when siblings share tokens");
    assert_eq!(
        suggestions,
        &vec![json!("iel-solver-config"), json!("iel-solver-debug")],
        "siblings ranked, unrelated topic excluded"
    );

    drop(dir);
}

#[test]
fn closest_topics_ranks_by_token_overlap() {
    let available = vec![
        "iel-solver-debug-toolkit".to_string(),
        "iel-solver".to_string(),
        "research/agent-memory".to_string(),
        "prompt-hamsa".to_string(),
    ];

    // Two shared tokens (iel, solver) beats one; unrelated topics dropped.
    let hits = closest_topics("iel-solver-config", &available);
    assert_eq!(hits, vec!["iel-solver", "iel-solver-debug-toolkit"]);

    // Nothing shares a token -> no suggestions (full list still shown upstream).
    assert!(closest_topics("zzz-unrelated", &available).is_empty());
}

#[tokio::test]
async fn memory_large_read_buffers_as_file_ref() {
    // Regression: memory(action="read") for large topics must return a @file_* ref
    // rather than {"content":"..."} inline. Without this, call_content wraps the
    // result in a 3-line @tool_* JSON envelope, making start_line/end_line useless.
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;

    // Write a topic whose content exceeds TOOL_OUTPUT_BUFFER_THRESHOLD (10 KB)
    let big: String = (1..=300)
        .map(|i| format!("# line {:04} padding_padding_padding_pad\n", i))
        .collect();
    assert!(
        big.len() > crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
        "test data must exceed threshold ({} bytes), got {}",
        crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
        big.len()
    );
    tool.call(
        json!({ "action": "write", "topic": "large-topic", "content": big }),
        &ctx,
    )
    .await
    .unwrap();

    let result = tool
        .call(json!({ "action": "read", "topic": "large-topic" }), &ctx)
        .await
        .unwrap();

    // Large content: must return @file_* ref, not inline {"content": "..."}
    assert!(
        result.get("file_id").is_some(),
        "large memory read should return @file_* ref; got: {}",
        result
    );
    assert_eq!(result["total_lines"].as_u64().unwrap(), 300);

    // Verify the @file_* ref is line-navigable
    let file_id = result["file_id"].as_str().unwrap().to_string();
    let sub = crate::tools::read_file::ReadFile
        .call(
            json!({"path": file_id, "start_line": 10, "end_line": 10}),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        sub["content"].as_str().unwrap_or("").contains("line 0010"),
        "sub-range on @file_* ref should return line 10; got: {}",
        sub
    );

    drop(dir);
}

#[tokio::test]
async fn memory_list_via_dispatch() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    tool.call(
        json!({ "action": "write", "topic": "a", "content": "x" }),
        &ctx,
    )
    .await
    .unwrap();
    let result = tool.call(json!({ "action": "list" }), &ctx).await.unwrap();
    let topics = result["topics"].as_array().expect("expected topics array");
    assert!(topics.iter().any(|t| t.as_str() == Some("a")));
    drop(dir);
}

#[tokio::test]
async fn memory_delete_via_dispatch() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    tool.call(
        json!({ "action": "write", "topic": "to_delete", "content": "x" }),
        &ctx,
    )
    .await
    .unwrap();
    tool.call(json!({ "action": "delete", "topic": "to_delete" }), &ctx)
        .await
        .unwrap();
    let result = tool
        .call(json!({ "action": "read", "topic": "to_delete" }), &ctx)
        .await;
    assert!(result.is_err(), "expected error reading deleted topic");
    drop(dir);
}

#[tokio::test]
async fn memory_unknown_action_returns_recoverable_error() {
    let (dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool.call(json!({ "action": "explode" }), &ctx).await;
    assert!(result.is_err());
    drop(dir);
}

#[tokio::test]
async fn memory_remember_requires_content() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool.call(json!({ "action": "remember" }), &ctx).await;
    assert!(result.is_err(), "should error without content");
}

#[tokio::test]
async fn memory_recall_requires_query() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool.call(json!({ "action": "recall" }), &ctx).await;
    assert!(result.is_err(), "should error without query");
}

#[tokio::test]
async fn memory_forget_requires_id() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool.call(json!({ "action": "forget" }), &ctx).await;
    assert!(result.is_err(), "should error without id");
}

#[test]
fn memory_schema_has_new_actions() {
    let schema = Memory.input_schema();
    let actions = schema["properties"]["action"]["enum"].as_array().unwrap();
    assert!(actions.contains(&json!("remember")));
    assert!(actions.contains(&json!("recall")));
    assert!(actions.contains(&json!("forget")));
}

#[test]
fn memory_schema_has_new_properties() {
    let schema = Memory.input_schema();
    assert!(schema["properties"]["query"].is_object());
    assert!(schema["properties"]["bucket"].is_object());
    assert!(schema["properties"]["title"].is_object());
    assert!(schema["properties"]["id"].is_object());
    assert!(schema["properties"]["limit"].is_object());
}

#[test]
fn extract_title_first_sentence() {
    assert_eq!(
        extract_title("Hello world. More text here."),
        "Hello world."
    );
}

#[test]
fn extract_title_truncates_long_content() {
    let long = "a".repeat(200);
    let title = extract_title(&long);
    assert!(title.len() <= 83); // 80 + "..."
}

#[test]
fn extract_title_short_content() {
    assert_eq!(extract_title("Short"), "Short");
}

#[test]
fn extract_title_used_in_cross_embed_context() {
    // Verify extract_title works for typical memory topics
    assert_eq!(
        extract_title("Three layer architecture design."),
        "Three layer architecture design."
    );
}

#[test]
fn extract_title_multibyte_at_boundary() {
    // \u{2500} (box drawing char) is 3 bytes each. 27 chars = 81 bytes.
    // Byte 80 falls inside the 27th char (bytes 78..81), so naive
    // content[..80] would panic. safe_truncate rounds down to byte 78.
    let content: String = "\u{2500}".repeat(27);
    let title = extract_title(&content);
    // Should not panic and should end with "..."
    assert!(
        title.ends_with("..."),
        "expected trailing '...', got: {title}"
    );
    // Title body (minus the "...") should be valid UTF-8 and <= 80 bytes
    let body = &title[..title.len() - 3];
    assert!(body.len() <= 80);
    assert!(
        body.len().is_multiple_of(3),
        "should truncate at char boundary"
    );
}

#[tokio::test]
async fn memory_write_still_works_without_embedder() {
    // Write should succeed even if cross-embedding fails
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool
        .call(
            json!({ "action": "write", "topic": "test-topic", "content": "hello" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_memory_write_ok(&result);

    // Verify markdown file was written
    let read_result = tool
        .call(json!({ "action": "read", "topic": "test-topic" }), &ctx)
        .await
        .unwrap();
    assert_eq!(read_result["content"], "hello");
}

#[tokio::test]
async fn memory_delete_still_works_without_embedder() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    tool.call(
        json!({ "action": "write", "topic": "del-me", "content": "x" }),
        &ctx,
    )
    .await
    .unwrap();
    let result = tool
        .call(json!({ "action": "delete", "topic": "del-me" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result, json!("ok"));
}

#[tokio::test]
async fn memory_write_private_not_cross_embedded() {
    // Private memories should not attempt cross-embedding
    let (_dir, ctx) = test_ctx_with_project().await;
    let tool = Memory;
    let result = tool
            .call(
                json!({ "action": "write", "topic": "secret", "content": "private data", "private": true }),
                &ctx,
            )
            .await
            .unwrap();
    assert_eq!(result, json!("ok"));
}

#[tokio::test]
async fn write_creates_anchor_sidecar() {
    let (dir, ctx) = test_ctx_with_project().await;

    // Create a source file in the temp project
    std::fs::create_dir_all(dir.path().join("src/tools")).unwrap();
    std::fs::write(dir.path().join("src/tools/mod.rs"), "pub fn tool() {}").unwrap();

    let input = json!({
        "action": "write",
        "topic": "architecture",
        "content": "## Tools\nThe tool trait lives in `src/tools/mod.rs`."
    });
    let result = Memory.call(input, &ctx).await.unwrap();
    assert_memory_write_ok(&result);

    // Check sidecar was created
    let sidecar = dir
        .path()
        .join(".codescout/memories/architecture.anchors.toml");
    assert!(sidecar.exists(), "anchor sidecar should be created");
    let af = crate::memory::anchors::read_anchor_file(&sidecar).unwrap();
    assert_eq!(af.anchors.len(), 1);
    assert_eq!(af.anchors[0].path, "src/tools/mod.rs");
}

#[tokio::test]
async fn refresh_anchors_clears_staleness() {
    let (dir, ctx) = test_ctx_with_project().await;
    let memories_dir = dir.path().join(".codescout/memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/a.rs"), "v1").unwrap();

    // Write memory to create sidecar
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "test-topic",
                "content": "References `src/a.rs`."
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Modify file to make it stale
    std::fs::write(dir.path().join("src/a.rs"), "v2").unwrap();

    // Verify stale
    let af =
        crate::memory::anchors::read_anchor_file(&memories_dir.join("test-topic.anchors.toml"))
            .unwrap();
    let report = crate::memory::anchors::check_path_staleness(dir.path(), &af).unwrap();
    assert!(!report.is_fresh());

    // Refresh anchors
    let result = Memory
        .call(
            json!({
                "action": "refresh_anchors",
                "topic": "test-topic"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result, json!("ok"));

    // Verify fresh
    let af =
        crate::memory::anchors::read_anchor_file(&memories_dir.join("test-topic.anchors.toml"))
            .unwrap();
    let report = crate::memory::anchors::check_path_staleness(dir.path(), &af).unwrap();
    assert!(report.is_fresh());
}

#[tokio::test]
async fn memory_write_routes_to_project_dir() {
    use crate::agent::Agent;
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let root = dir.path();

    // Multi-project structure: root gradle project + mcp-server sub-project
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
    // .codescout dir needed for Agent::new
    std::fs::create_dir_all(root.join(".codescout")).unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = crate::lsp::LspManager::new_arc();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Network-free stubs, installed before any tool call can trigger ambient
    // resolution — this test's "write" actions succeed, which reaches the
    // best-effort semantic-anchor side effect in Memory::call; without this,
    // it silently cross-embeds into whatever real embedder/store the
    // ambient environment resolves. See test_ctx_with_project's doc comment.
    // The code-search override is a THIRD, separate resolution path (see
    // test_ctx_with_project_raw's doc comment) that neither the embedder nor
    // the store seam covers.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    ctx.agent.set_code_search_for_test(
        Arc::new(NoCodeSearch) as Arc<dyn crate::retrieval::search::CodeChunkSearch>
    );

    // Write memory to mcp-server project
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "conventions",
                "content": "Use camelCase",
                "project_id": "mcp-server"
            }),
            &ctx,
        )
        .await
        .unwrap();

    // File should be in per-project dir
    let project_mem_path = root.join(".codescout/projects/mcp-server/memories/conventions.md");
    assert!(
        project_mem_path.exists(),
        "memory should be in per-project dir: {project_mem_path:?}"
    );

    // Write memory with no project param — resolves to workspace root dir
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "root-conventions",
                "content": "Use Kotlin idioms"
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Root memory in workspace-level dir
    let root_mem_path = root.join(".codescout/memories/root-conventions.md");
    assert!(
        root_mem_path.exists(),
        "root memory should be in workspace-level dir: {root_mem_path:?}"
    );

    // list scoped to mcp-server should only show conventions
    let list_result = Memory
        .call(
            json!({ "action": "list", "project_id": "mcp-server" }),
            &ctx,
        )
        .await
        .unwrap();
    let topics: Vec<&str> = list_result["topics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(topics, vec!["conventions"]);

    // read scoped to mcp-server
    let read_result = Memory
        .call(
            json!({ "action": "read", "topic": "conventions", "project_id": "mcp-server" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(read_result["content"], "Use camelCase");

    // delete scoped to mcp-server
    Memory
        .call(
            json!({ "action": "delete", "topic": "conventions", "project_id": "mcp-server" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!project_mem_path.exists(), "memory should be deleted");
}

/// Root gradle project plus a real `mcp-server` sub-project, so `project_ids()` is
/// non-empty and an unknown id is genuinely unknown rather than merely unlisted.
///
/// Worth noting why this fixture is needed: under the pre-fix code a BOGUS id also
/// resolved to `.codescout/projects/<id>/memories`, so
/// `memory_write_routes_to_project_dir`'s path assertion passes whether or not
/// `mcp-server` is actually discovered. The hint assertions below are the first
/// thing in the suite that can tell the difference.
async fn multi_project_ctx() -> (tempfile::TempDir, ToolContext) {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Network-free stubs, installed before any tool call can trigger ambient
    // resolution — see test_ctx_with_project's doc comment. The code-search
    // override is a THIRD, separate resolution path (see
    // test_ctx_with_project_raw's doc comment) that neither the embedder nor
    // the store seam covers.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    ctx.agent.set_code_search_for_test(
        Arc::new(NoCodeSearch) as Arc<dyn crate::retrieval::search::CodeChunkSearch>
    );
    (dir, ctx)
}

#[tokio::test]
async fn memory_write_with_unknown_project_id_errors_and_leaves_no_directory() {
    let (dir, ctx) = multi_project_ctx().await;
    let phantom = dir.path().join(".codescout/projects/zz-not-a-project");

    let err = Memory
        .call(
            json!({
                "action": "write",
                "topic": "conventions",
                "content": "should never land",
                "project_id": "zz-not-a-project"
            }),
            &ctx,
        )
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("an unknown project_id is a correctable input mistake, not a bail");
    assert!(
        rec.message.contains("zz-not-a-project"),
        "message must name the offending id; got: {}",
        rec.message
    );
    let hint = rec
        .hint()
        .expect("expected Hint guidance listing the real ids");
    assert!(
        hint.contains("mcp-server"),
        "hint must list the workspace's real project ids; got: {hint}"
    );

    // The half a response-only test would miss. The error and the litter are
    // separate failures, and a fix could close one without the other.
    assert!(
        !phantom.exists(),
        "a rejected write must not create {phantom:?}"
    );
}

#[tokio::test]
async fn memory_read_with_unknown_project_id_says_no_such_project_not_no_topics() {
    let (dir, ctx) = multi_project_ctx().await;
    let phantom = dir.path().join(".codescout/projects/zz-not-a-project");

    let err = Memory
        .call(
            json!({ "action": "read", "topic": "conventions", "project_id": "zz-not-a-project" }),
            &ctx,
        )
        .await
        .unwrap_err();

    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("unknown project_id on read is also a correctable input mistake");
    assert!(
        rec.message.contains("zz-not-a-project"),
        "message must name the absent project; got: {}",
        rec.message
    );

    // The exact regression: a read against a non-existent project used to report
    // it as merely EMPTY — "no memory topics exist yet", available_topics: [] —
    // which a caller acts on. Absent and empty must not read the same.
    let combined = format!("{} {}", rec.message, rec.hint().unwrap_or_default());
    assert!(
        !combined.contains("no memory topics exist yet"),
        "a non-existent project must not be reported as empty; got: {combined}"
    );

    assert!(
        !phantom.exists(),
        "a rejected read must not create {phantom:?}"
    );
}

#[tokio::test]
async fn memory_write_accepts_project_alias_for_project_id() {
    use crate::agent::Agent;
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let root = dir.path();

    // Multi-project: root gradle project + mcp-server sub-project.
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    // Substantive manifest: an empty `{}` is NOT enough for discovery to register
    // a project, so this fixture used to declare an `mcp-server` that did not
    // exist — and the assertions below passed anyway, because an unknown
    // project_id silently got its own `projects/<id>/memories` tree. Matching the
    // sibling fixture's content makes the sub-project real, so the alias is now
    // tested against a project that is actually there.
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::create_dir_all(root.join(".codescout")).unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let lsp: Arc<dyn crate::lsp::LspProvider> = crate::lsp::LspManager::new_arc();
    let ctx = ToolContext {
        agent,
        lsp,
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    // Network-free stubs, installed before any tool call can trigger ambient
    // resolution — this test's "write" action succeeds, which reaches the
    // best-effort semantic-anchor side effect in Memory::call. See
    // test_ctx_with_project's doc comment. The code-search override is a
    // THIRD, separate resolution path (see test_ctx_with_project_raw's doc
    // comment) that neither the embedder nor the store seam covers.
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    ctx.agent.set_code_search_for_test(
        Arc::new(NoCodeSearch) as Arc<dyn crate::retrieval::search::CodeChunkSearch>
    );

    // Write using the `project` ALIAS (not project_id). Regression for the 2026-06-09
    // onboarding bug: the unknown `project` key was silently dropped and the write
    // misrouted to the focused/root project.
    Memory
        .call(
            json!({
                "action": "write",
                "topic": "conventions",
                "content": "Use camelCase",
                "project": "mcp-server"
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Alias must land in the per-project dir that project_id= would use.
    let project_mem_path = root.join(".codescout/projects/mcp-server/memories/conventions.md");
    assert!(
        project_mem_path.exists(),
        "project= alias must route to per-project dir (not silently dropped): {project_mem_path:?}"
    );

    // And must NOT misroute to the workspace-level/root dir.
    let root_mem_path = root.join(".codescout/memories/conventions.md");
    assert!(
        !root_mem_path.exists(),
        "project= alias must NOT misroute to the root memory dir: {root_mem_path:?}"
    );

    // Read back via the canonical project_id= key returns the same content.
    let read_result = Memory
        .call(
            json!({ "action": "read", "topic": "conventions", "project_id": "mcp-server" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(read_result["content"], "Use camelCase");
}

#[tokio::test]
async fn memory_read_sections_filter_integration() {
    let (_dir, ctx) = test_ctx_with_project().await;

    // Write a multi-section memory
    let content =
        "# Lang Patterns\n\nIntro.\n\n### Rust\n\nRust stuff.\n\n### TypeScript\n\nTS stuff.\n";
    Memory
        .call(
            json!({ "action": "write", "topic": "language-patterns", "content": content }),
            &ctx,
        )
        .await
        .unwrap();

    // Filter to Rust only
    let result = Memory
        .call(
            json!({ "action": "read", "topic": "language-patterns", "sections": ["Rust"] }),
            &ctx,
        )
        .await
        .unwrap();
    let text = result["content"].as_str().unwrap();
    assert!(text.contains("### Rust"), "should contain Rust section");
    assert!(text.contains("Rust stuff."));
    assert!(
        !text.contains("### TypeScript"),
        "should not contain TypeScript"
    );
    assert!(text.contains("# Lang Patterns"), "should contain preamble");

    // Empty sections array → full content (same as omitting the param)
    let result = Memory
        .call(
            json!({ "action": "read", "topic": "language-patterns", "sections": [] }),
            &ctx,
        )
        .await
        .unwrap();
    let text = result["content"].as_str().unwrap();
    assert!(
        text.contains("### Rust") && text.contains("### TypeScript"),
        "empty sections = full content"
    );

    // Unknown section → RecoverableError; hint lists available sections.
    // Tool::call returns Err(RecoverableError) directly — route_tool_error is
    // only invoked by the MCP server, not in unit tests.
    let err = Memory
        .call(
            json!({ "action": "read", "topic": "language-patterns", "sections": ["Go"] }),
            &ctx,
        )
        .await
        .unwrap_err();
    let rec = err
        .downcast_ref::<RecoverableError>()
        .expect("should be RecoverableError");
    let hint = rec.hint().unwrap_or("");
    assert!(
        hint.contains("Rust") && hint.contains("TypeScript"),
        "hint should list available sections: {hint}"
    );

    // Partial match → content + missing list
    let result = Memory
        .call(
            json!({ "action": "read", "topic": "language-patterns", "sections": ["Rust", "Go"] }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result["content"].as_str().is_some(),
        "matched sections should be in content"
    );
    let missing = result["missing"]
        .as_array()
        .expect("missing field should be present");
    assert_eq!(missing, &[json!("Go")]);
}

#[tokio::test]
async fn memory_read_sections_string_coerced() {
    let (_dir, ctx) = test_ctx_with_project().await;

    let content =
        "# Lang Patterns\n\nIntro.\n\n### Rust\n\nRust stuff.\n\n### TypeScript\n\nTS stuff.\n";
    Memory
        .call(
            json!({ "action": "write", "topic": "lang-coerce-test", "content": content }),
            &ctx,
        )
        .await
        .unwrap();

    // Simulate MCP client that stringifies array params
    let result = Memory
        .call(
            json!({ "action": "read", "topic": "lang-coerce-test", "sections": "[\"Rust\"]" }),
            &ctx,
        )
        .await
        .unwrap();
    let text = result["content"].as_str().unwrap();
    assert!(text.contains("### Rust"), "should contain Rust section");
    assert!(
        !text.contains("### TypeScript"),
        "should not contain TypeScript"
    );
}

#[tokio::test]
async fn memory_read_sections_filter_private_integration() {
    let (_dir, ctx) = test_ctx_with_project().await;

    // Write a private multi-section memory
    let content = "### Rust\n\nRust stuff.\n\n### Python\n\nPython stuff.\n";
    Memory
        .call(
            json!({ "action": "write", "topic": "lang", "content": content, "private": true }),
            &ctx,
        )
        .await
        .unwrap();

    // Filtering applies in the private branch too
    let result = Memory
        .call(
            json!({ "action": "read", "topic": "lang", "sections": ["Rust"], "private": true }),
            &ctx,
        )
        .await
        .unwrap();
    let text = result["content"].as_str().unwrap();
    assert!(text.contains("### Rust"), "should contain Rust");
    assert!(!text.contains("### Python"), "should not contain Python");
}

// ─── read-union across a sub-project's two memory layouts ────────────────────
//
// A sub-project has two memory directories and always has had:
//   `<ws>/.codescout/projects/<id>/memories`  — `Workspace::memory_dir_for_project`
//   `<ws>/<rel>/.codescout/memories`          — `MemoryStore::open(project.root)`
// Reads now union both; writes still target the first alone.
//
// docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md

/// Two-project workspace (`test` at the root, `svc` beneath it), with the
/// semantic store and embedder isolated exactly as `test_ctx_with_project` does —
/// a workspace fixture must not become the ambient-resolution hole that
/// `docs/issues/archive/2026-08-26-test-fixtures-write-into-the-live-memories-collection.md`
/// closed.
async fn workspace_ctx_with_sub_project() -> (tempfile::TempDir, std::path::PathBuf, ToolContext) {
    use crate::memory::semantic_store::test_support::InMemorySemanticMemoryStore;
    use crate::memory::semantic_store::SemanticMemoryStore;
    use crate::retrieval::embedder::DenseEmbedder;
    use std::sync::Arc;

    let dir = tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    let svc = root.join("svc");
    std::fs::create_dir_all(&svc).unwrap();
    std::fs::write(svc.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

    let codescout = root.join(".codescout");
    std::fs::create_dir_all(&codescout).unwrap();
    std::fs::write(
        codescout.join("workspace.toml"),
        r#"
[workspace]
name = "test"

[[project]]
id = "test"
root = "."
languages = ["kotlin"]

[[project]]
id = "svc"
root = "svc"
languages = ["typescript"]
"#,
    )
    .unwrap();
    std::fs::write(
        codescout.join("project.toml"),
        "[project]\nname = \"test\"\nlanguages = [\"kotlin\"]\n",
    )
    .unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    ctx.agent
        .set_memory_embedder_for_test(Arc::new(FixedEmbedder) as Arc<dyn DenseEmbedder>)
        .map_err(|_| ())
        .expect("set embedder");
    ctx.agent
        .set_semantic_memory_store_for_test(
            Arc::new(InMemorySemanticMemoryStore::new()) as Arc<dyn SemanticMemoryStore>
        )
        .map_err(|_| ())
        .expect("set store");
    // The code-search override is a THIRD, separate resolution path (see
    // test_ctx_with_project_raw's doc comment) that neither the embedder nor
    // the store seam above covers.
    ctx.agent.set_code_search_for_test(
        Arc::new(NoCodeSearch) as Arc<dyn crate::retrieval::search::CodeChunkSearch>
    );
    // CANONICALISED, and handing it back is what makes the callers' path assertions
    // portable. `Agent::new` canonicalises the root it is given, so every path
    // `resolve_memory_dirs` returns is already normalised while `dir.path()` is not.
    //
    // On Linux the two spellings coincide, so the difference is invisible to the local
    // gate. On macOS the temp dir resolves /var -> /private/var; on Windows
    // canonicalisation adds the `\\?\` verbatim prefix AND expands the `RUNNER~1` 8.3
    // short name. Comparing a resolved path against a raw `dir.path()` therefore passes
    // here and fails on both other platforms — it did, on six lanes, for five days
    // (CI run 33404896131).
    //
    // Returned rather than left to each test to remember: two of the six callers compare
    // paths today, and a seventh written from this fixture would reintroduce the bug with
    // nothing local able to catch it.
    let canonical_root = std::fs::canonicalize(dir.path()).expect("canonicalise temp root");
    (dir, canonical_root, ctx)
}

/// The write target for sub-project `svc`.
fn ws_layout_dir(root: &std::path::Path) -> std::path::PathBuf {
    root.join(".codescout")
        .join("projects")
        .join("svc")
        .join("memories")
}

/// The project-local layout for sub-project `svc`.
fn local_layout_dir(root: &std::path::Path) -> std::path::PathBuf {
    root.join("svc").join(".codescout").join("memories")
}

fn seed(dir: &std::path::Path, topic: &str, body: &str) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{topic}.md")), body).unwrap();
}

/// `memory(action="list")` reports BOTH of a sub-project's memory layouts.
///
/// Before the union each surface saw one directory, so a project holding topics
/// in the other one was reported as empty — measured on a real 12-sub-project
/// workspace where 53 memories sat project-local and the workspace tree held 0.
#[tokio::test]
async fn memory_list_unions_both_layouts_for_a_sub_project() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;
    seed(&ws_layout_dir(&root), "from-workspace-tree", "# W");
    seed(&local_layout_dir(&root), "from-project-local", "# L");

    let listed = Memory
        .call(json!({ "action": "list", "project_id": "svc" }), &ctx)
        .await
        .unwrap();

    assert_eq!(
        listed["topics"],
        json!(["from-project-local", "from-workspace-tree"]),
        "list must union both layouts, sorted and deduped: {listed:?}"
    );
}

/// A topic living ONLY in the project-local layout is now readable — and the
/// response says where it came from, because a later `memory(write)` on that
/// topic targets the workspace tree and would leave this file shadowed.
#[tokio::test]
async fn memory_read_falls_back_to_the_other_layout_and_names_the_write_target() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;
    seed(&local_layout_dir(&root), "only-local", "# Local only");

    let read = Memory
        .call(
            json!({ "action": "read", "topic": "only-local", "project_id": "svc" }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(read["content"], json!("# Local only"), "got: {read:?}");
    let from = read["resolved_from"].as_str().unwrap_or_default();
    let target = read["write_target"].as_str().unwrap_or_default();
    assert!(
        from.ends_with("svc/.codescout/memories"),
        "resolved_from must name the layout it actually read: {read:?}"
    );
    assert!(
        target.ends_with("projects/svc/memories"),
        "write_target must name where a write would land instead: {read:?}"
    );
}

/// Negative control for the test above: a topic served FROM the write target
/// carries no provenance fields at all. Without this, a formatter that always
/// stamped them would pass the fallback test unchanged.
#[tokio::test]
async fn memory_read_from_the_write_target_carries_no_provenance_fields() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;
    seed(&ws_layout_dir(&root), "in-write-target", "# W");

    let read = Memory
        .call(
            json!({ "action": "read", "topic": "in-write-target", "project_id": "svc" }),
            &ctx,
        )
        .await
        .unwrap();

    assert_eq!(read["content"], json!("# W"), "got: {read:?}");
    assert!(
        read.get("resolved_from").is_none() && read.get("write_target").is_none(),
        "a read served by the write target is the normal case and must stay quiet: {read:?}"
    );
}

/// The union must not CREATE the directory it looks into.
///
/// `MemoryStore::from_dir` calls `create_dir_all`, so routing a read through it
/// would have made every list materialise the other layout — the same litter
/// class as
/// `docs/issues/archive/2026-08-08-memory-dir-for-project-materializes-any-id.md`,
/// re-introduced by the fix for a different bug. `from_dir_readonly` exists for
/// exactly this, and this test is what holds it in place.
#[tokio::test]
async fn a_union_read_does_not_materialise_the_other_layouts_directory() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;
    seed(&ws_layout_dir(&root), "only-in-tree", "# W");
    let local = local_layout_dir(&root);
    assert!(!local.exists(), "precondition: the local layout is absent");

    Memory
        .call(json!({ "action": "list", "project_id": "svc" }), &ctx)
        .await
        .unwrap();
    Memory
        .call(
            json!({ "action": "read", "topic": "only-in-tree", "project_id": "svc" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        !local.exists(),
        "a read created {} — an empty directory that reads as an empty memory set",
        local.display()
    );
}

/// The ROOT project has one directory, not two, so every read path here reduces
/// to its previous single-directory behaviour. This is why a single-project repo
/// never reproduced the bug and sees no change from the fix.
#[tokio::test]
async fn the_root_project_resolves_a_single_directory() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;

    // No `project_id`: focus defaults to the root project (`Workspace::new`), and
    // this is the path that must stay byte-identical to its pre-union behaviour.
    let dirs = resolve_memory_dirs(&json!({}), &ctx).await.unwrap();

    assert_eq!(dirs.primary, root.join(".codescout").join("memories"));
    assert!(
        dirs.secondary.is_none(),
        "the root project's two layouts are the same path, so there is nothing to \
         union — got {:?}",
        dirs.secondary
    );
}

/// A sub-project resolves two distinct directories — the precondition every test
/// above rests on, asserted directly so a change that collapsed them would fail
/// here rather than silently making the union a no-op everywhere.
#[tokio::test]
async fn a_sub_project_resolves_two_distinct_directories() {
    let (_dir, root, ctx) = workspace_ctx_with_sub_project().await;

    let dirs = resolve_memory_dirs(&json!({ "project_id": "svc" }), &ctx)
        .await
        .unwrap();

    assert_eq!(dirs.primary, ws_layout_dir(&root));
    assert_eq!(dirs.secondary, Some(local_layout_dir(&root)));
}

/// The provenance fields must be RENDERED, not merely carried.
///
/// `format_read_memory` returns `$.content` alone, so a field added to the JSON
/// and left out of the formatter reaches nobody — the exact failure shipped and
/// fixed the same day in
/// `docs/issues/archive/2026-08-26-read-file-truncation-flag-never-rendered.md`.
/// The note is head-placed because this function's output IS the memory body: a
/// trailing note is what truncation cuts, and what a caller pasting the result
/// would carry into the memory.
#[test]
fn format_read_memory_renders_the_shadow_warning_at_the_head() {
    let plain = format_read_memory(&json!({ "content": "# Arch" }));
    assert_eq!(plain, "# Arch", "the normal case stays verbatim");

    let shadowed = format_read_memory(&json!({
        "content": "# Arch",
        "resolved_from": "/ws/svc/.codescout/memories",
        "write_target": "/ws/.codescout/projects/svc/memories",
    }));
    assert!(
        shadowed.starts_with('⚠'),
        "the note must lead, not trail: {shadowed}"
    );
    assert!(
        shadowed.contains("/ws/svc/.codescout/memories")
            && shadowed.contains("/ws/.codescout/projects/svc/memories"),
        "both paths must survive into the rendered text: {shadowed}"
    );
    assert!(
        shadowed.ends_with("# Arch"),
        "the memory body must still be there, and last: {shadowed}"
    );
}
