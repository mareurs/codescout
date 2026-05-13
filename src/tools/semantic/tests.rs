use super::index::format_index_status;
use super::semantic_search::{
    apply_file_diversity_cap, format_search_result_item, format_semantic_search,
};
use super::*;
use crate::agent::Agent;
use crate::embed::index;
use crate::lsp::LspManager;
use crate::tools::{Tool, ToolContext};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn rrf_fuse_integration_empty_bm25_returns_vector_order() {
    use crate::embed::fusion;
    use crate::embed::schema::SearchResult;

    let vector = vec![
        SearchResult {
            id: 1,
            file_path: "a.rs".into(),
            language: "rust".into(),
            content: "a".into(),
            start_line: 0,
            end_line: 1,
            score: 0.9,
            source: "project".into(),
            project_id: "root".into(),
        },
        SearchResult {
            id: 2,
            file_path: "b.rs".into(),
            language: "rust".into(),
            content: "b".into(),
            start_line: 0,
            end_line: 1,
            score: 0.8,
            source: "project".into(),
            project_id: "root".into(),
        },
    ];
    let fused_ids = fusion::rrf_fuse(&vector, &[], 60.0);
    assert_eq!(fused_ids, vec![1, 2]);
}

fn sr(file: &str, score: f32) -> crate::embed::schema::SearchResult {
    crate::embed::schema::SearchResult {
        id: 0,
        file_path: file.to_string(),
        language: "rust".to_string(),
        content: String::new(),
        start_line: 0,
        end_line: 0,
        score,
        source: "project".to_string(),
        project_id: String::new(),
    }
}

#[test]
fn file_diversity_cap_drops_excess_same_file_entries() {
    let input = vec![
        sr("a.rs", 0.9),
        sr("a.rs", 0.8),
        sr("a.rs", 0.7),
        sr("a.rs", 0.6),
        sr("b.rs", 0.5),
        sr("a.rs", 0.4),
    ];
    let out = apply_file_diversity_cap(input, 3);
    let files: Vec<&str> = out.iter().map(|r| r.file_path.as_str()).collect();
    // a.rs capped at 3; later a.rs entry dropped; b.rs survives with original ordering
    assert_eq!(files, vec!["a.rs", "a.rs", "a.rs", "b.rs"]);
}

#[test]
fn file_diversity_cap_zero_disables() {
    let input = vec![sr("a.rs", 0.9), sr("a.rs", 0.8), sr("a.rs", 0.7)];
    let out = apply_file_diversity_cap(input.clone(), 0);
    assert_eq!(out.len(), 3);
}

#[test]
fn file_diversity_cap_preserves_score_order() {
    // Order is preserved — cap does not re-rank; it just filters.
    let input = vec![
        sr("a.rs", 0.9),
        sr("b.rs", 0.8),
        sr("a.rs", 0.7),
        sr("c.rs", 0.6),
    ];
    let out = apply_file_diversity_cap(input, 1);
    let files: Vec<&str> = out.iter().map(|r| r.file_path.as_str()).collect();
    assert_eq!(files, vec!["a.rs", "b.rs", "c.rs"]);
}

#[tokio::test]
async fn index_project_sets_initial_running_state() {
    use crate::agent::IndexingState;
    // Call index_project on a project with no embedder configured.
    // It will start and quickly fail in the background, but the
    // initial Running state is set synchronously before the spawn.
    let (_dir, ctx) = project_ctx().await;
    let _ = IndexProject.call(json!({}), &ctx).await;

    // State should not be left Idle after a call — it was either
    // set to Running (still in progress) or transitioned to Done/Failed.
    let state = ctx.agent.indexing.lock().unwrap().clone();
    assert!(!matches!(state, IndexingState::Idle));
}

async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    (
        dir,
        ToolContext {
            agent,
            lsp: LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        },
    )
}

#[tokio::test]
async fn index_status_no_index() {
    let (_dir, ctx) = project_ctx().await;
    let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result["indexed"], false);
}

#[tokio::test]
async fn index_status_shows_running_progress() {
    use crate::agent::IndexingState;
    let (_dir, ctx) = project_ctx().await;

    // Simulate mid-run state — IndexStatus appends the indexing block
    // regardless of whether Qdrant is reachable.
    {
        let mut state = ctx.agent.indexing.lock().unwrap();
        *state = IndexingState::Running {
            done: 10,
            total: 50,
            eta_secs: Some(20),
        };
    }

    let result = IndexStatus.call(json!({}), &ctx).await.unwrap();
    let indexing = &result["indexing"];
    assert_eq!(indexing["status"], "running");
    assert_eq!(indexing["done"], 10);
    assert_eq!(indexing["total"], 50);
    assert_eq!(indexing["eta_secs"], 20);

    // eta_secs: None renders as JSON null
    {
        let mut state = ctx.agent.indexing.lock().unwrap();
        *state = IndexingState::Running {
            done: 50,
            total: 50,
            eta_secs: None,
        };
    }
    let result2 = IndexStatus.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result2["indexing"]["eta_secs"], serde_json::Value::Null);
}

#[tokio::test]
async fn tools_error_without_project() {
    let ctx = ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    assert!(SemanticSearch
        .call(json!({ "query": "test" }), &ctx)
        .await
        .is_err());
    assert!(IndexProject.call(json!({}), &ctx).await.is_err());
    assert!(IndexStatus.call(json!({}), &ctx).await.is_err());
}

#[tokio::test]
async fn index_stats_function() {
    let dir = tempdir().unwrap();
    let conn = index::open_db(dir.path()).unwrap();
    let stats = index::index_stats(&conn).unwrap();
    assert_eq!(stats.file_count, 0);
    assert_eq!(stats.chunk_count, 0);
    assert_eq!(stats.embedding_count, 0);
}

#[tokio::test]
async fn semantic_search_schema_has_detail_level() {
    let schema = SemanticSearch.input_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(
        props.contains_key("detail_level"),
        "should accept detail_level parameter"
    );
    assert!(
        props.contains_key("offset"),
        "should accept offset parameter"
    );
}

#[test]
fn preview_truncation_works() {
    // Helper mirrors the inline logic in SemanticSearch::call
    let truncate = |content: &str| -> String {
        let first_line = content.lines().next().unwrap_or("").trim();
        let char_count = first_line.chars().count();
        if char_count > 50 {
            let truncated: String = first_line.chars().take(47).collect();
            format!("{}...", truncated)
        } else {
            first_line.to_string()
        }
    };

    // ASCII: long content truncated to 47 chars + "..."
    let long_ascii = "x".repeat(100);
    let preview = truncate(&long_ascii);
    assert_eq!(preview.chars().count(), 50); // 47 + "..."
    assert!(preview.ends_with("..."));

    // Unicode: multi-byte chars must not panic
    // Each '日' is 3 bytes; 51 chars = 153 bytes — old [..47] would panic mid-codepoint
    let long_unicode = "日".repeat(51);
    let preview_unicode = truncate(&long_unicode);
    assert_eq!(preview_unicode.chars().count(), 50); // 47 '日' + "..."
    assert!(preview_unicode.ends_with("..."));

    // Emoji: also multi-byte
    let emoji_line = "🦀".repeat(51);
    let preview_emoji = truncate(&emoji_line);
    assert_eq!(preview_emoji.chars().count(), 50);

    // Multi-line: only first line is used
    let multiline = "first line\nsecond line\nthird line";
    assert_eq!(truncate(multiline), "first line");

    // Short content: no truncation
    assert_eq!(truncate("short"), "short");
}

#[tokio::test]
async fn semantic_search_schema_has_scope() {
    let schema = SemanticSearch.input_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("scope"), "should accept scope parameter");
}

#[test]
fn semantic_search_schema_has_include_memories() {
    let schema = SemanticSearch.input_schema();
    assert!(schema["properties"]["include_memories"].is_object());
    assert_eq!(schema["properties"]["include_memories"]["type"], "boolean");
}

#[test]
fn semantic_search_schema_has_project() {
    let schema = SemanticSearch.input_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(
        props.contains_key("project_id"),
        "schema should have project_id param"
    );
}

#[tokio::test]
async fn concurrent_semantic_search_does_not_deadlock() {
    let (_dir, ctx) = project_ctx().await;
    let ctx = std::sync::Arc::new(ctx);
    let input = json!({"query": "test"});

    // Run two searches concurrently — neither should hang.
    // They'll likely error (no embedder available in test), but that's fine.
    // The point is they complete without deadlocking.
    let ctx1 = ctx.clone();
    let input1 = input.clone();
    let ctx2 = ctx.clone();
    let input2 = input.clone();

    let (r1, r2) = tokio::join!(
        async move { SemanticSearch.call(input1, &ctx1).await },
        async move { SemanticSearch.call(input2, &ctx2).await },
    );

    // Both should complete (either Ok or Err, but not hang)
    // We expect errors since there's no embedder in test environment
    let _ = r1;
    let _ = r2;
}

// --- format_semantic_search tests ---

#[test]
fn semantic_search_basic() {
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "src/tools/output.rs",
                "language": "rust",
                "content": "pub struct OutputGuard {\n    mode: OutputMode,\n}",
                "start_line": 35,
                "end_line": 50,
                "score": 0.923,
                "source": "project"
            },
            {
                "file_path": "src/tools/mod.rs",
                "language": "rust",
                "content": "pub trait Tool {\n    fn name(&self) -> &str;\n}",
                "start_line": 120,
                "end_line": 140,
                "score": 0.81,
                "source": "project"
            }
        ],
        "total": 2
    });
    let result = format_semantic_search(&val);
    assert!(result.starts_with("2 results\n"));
    assert!(result.contains("src/tools/output.rs:35-50"));
    assert!(result.contains("src/tools/mod.rs:120-140"));
    assert!(result.contains("pub struct OutputGuard {"));
    assert!(result.contains("pub trait Tool {"));
}

#[test]
fn semantic_search_single_result() {
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "src/main.rs",
                "language": "rust",
                "content": "fn main() {}",
                "start_line": 1,
                "end_line": 1,
                "score": 0.95,
                "source": "project"
            }
        ],
        "total": 1
    });
    let result = format_semantic_search(&val);
    assert!(result.starts_with("1 result\n"));
    assert!(!result.starts_with("1 results"));
    assert!(result.contains("src/main.rs:1"));
}

#[test]
fn semantic_search_empty() {
    let val = serde_json::json!({
        "results": [],
        "total": 0
    });
    assert_eq!(format_semantic_search(&val), "0 results");
}

#[test]
fn semantic_search_missing_results() {
    let val = serde_json::json!({});
    assert_eq!(format_semantic_search(&val), "");
}

#[test]
fn semantic_search_with_staleness() {
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "src/a.rs",
                "content": "fn foo() {}",
                "start_line": 1,
                "end_line": 5,
                "score": 0.9,
                "source": "project"
            }
        ],
        "total": 1,
        "git_sync": { "status": "behind", "behind_commits": 5 }
    });
    let result = format_semantic_search(&val);
    assert!(result.contains("5 commits not yet indexed"));
    assert!(result.contains("index(action='build')"));
}

#[test]
fn semantic_search_with_overflow() {
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "src/a.rs",
                "content": "fn foo() {}",
                "start_line": 1,
                "end_line": 5,
                "score": 0.9,
                "source": "project"
            }
        ],
        "total": 50,
        "overflow": {
            "shown": 10,
            "total": 50,
            "hint": "Use detail_level='full' with offset for pagination"
        }
    });
    let result = format_semantic_search(&val);
    assert!(result.contains("10 of 50"));
}

#[test]
fn semantic_search_long_content_truncated() {
    let long_content = "a".repeat(80);
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "src/a.rs",
                "content": long_content,
                "start_line": 1,
                "end_line": 10,
                "score": 0.85,
                "source": "project"
            }
        ],
        "total": 1
    });
    let result = format_semantic_search(&val);
    assert!(result.contains("..."));
    assert!(!result.contains(&"a".repeat(80)));
}

#[test]
fn semantic_search_score_alignment() {
    let val = serde_json::json!({
        "results": [
            {
                "file_path": "a.rs",
                "content": "short",
                "start_line": 1, "end_line": 1,
                "score": 0.9, "source": "project"
            },
            {
                "file_path": "very/long/path/to/file.rs",
                "content": "long path",
                "start_line": 100, "end_line": 200,
                "score": 0.85, "source": "project"
            }
        ],
        "total": 2
    });
    let result = format_semantic_search(&val);
    assert!(result.contains("a.rs:1"));
    assert!(result.contains("very/long/path/to/file.rs:100-200"));
}

// --- format_index_status tests ---

#[test]
fn format_index_status_shows_model_and_timestamp() {
    let result = serde_json::json!({
        "indexed": true,
        "file_count": 42,
        "chunk_count": 1234,
        "git_sync": { "status": "up_to_date" },
        "indexed_with_model": "text-embedding-3-small",
        "indexed_at": "2026-03-01 14:22"
    });
    let out = format_index_status(&result);
    assert!(
        out.contains("42 files"),
        "should show file count, got: {out}"
    );
    assert!(
        out.contains("1234 chunks"),
        "should show chunk count, got: {out}"
    );
    assert!(
        out.contains("text-embedding-3-small"),
        "should show model, got: {out}"
    );
    assert!(
        out.contains("2026-03-01"),
        "should show timestamp, got: {out}"
    );
}

#[test]
fn format_index_status_stale_shows_commit_count() {
    let result = serde_json::json!({
        "indexed": true,
        "file_count": 10,
        "chunk_count": 100,
        "git_sync": { "status": "behind", "behind_commits": 5 }
    });
    let out = format_index_status(&result);
    assert!(
        out.contains("5 commits not yet indexed"),
        "should note git sync lag, got: {out}"
    );
}

#[test]
fn search_result_item_content_is_last_field() {
    // Regression: content must be last — it is the bulk payload.
    // Score is excluded from output; ranking is communicated by result order.
    let item =
        format_search_result_item("src/foo.rs", 10, 20, "project", "fn hello() {}".to_string());
    let keys: Vec<&str> = item
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();

    assert!(
        keys.iter().all(|k| *k != "score"),
        "score must not appear in output, got key order: {keys:?}"
    );
    let content_pos = keys.iter().position(|k| *k == "content").unwrap();
    assert_eq!(
        content_pos,
        keys.len() - 1,
        "content must be the last field, got key order: {keys:?}"
    );
}

#[tokio::test]
async fn semantic_search_uses_scope_for_library_search() {
    // Scope "lib:nonexistent" should return a RecoverableError (not a panic or fatal error).
    // Since issue #5, search_multi_db validates the scope against the registry, so an
    // unregistered library name returns a RecoverableError before any embedder call.
    let (_dir, ctx) = project_ctx().await;
    let tool = SemanticSearch;
    let result = tool
        .call(
            json!({"query": "runtime", "scope": "lib:nonexistent"}),
            &ctx,
        )
        .await;
    // Acceptable outcomes:
    // 1. RecoverableError about unregistered library name
    // 2. RecoverableError about missing embedder / model config
    // A plain anyhow error from source_filter/search_scoped code paths would be a regression.
    match &result {
        Ok(val) => {
            assert!(val["total"].as_u64().unwrap_or(0) == 0);
        }
        Err(e) => {
            let msg = e.to_string();
            assert!(
                msg.contains("model")
                    || msg.contains("embed")
                    || msg.contains("project")
                    || msg.contains("not registered")
                    || msg.contains("registered")
                    || msg.contains("hybrid_query")
                    || msg.contains("retrieval stack"),
                "unexpected error (not embedder or registry-related): {msg}"
            );
        }
    }
}

// --- Progress notification tests (T11) ---

use crate::tools::progress::test_support::CountingSink;
use std::sync::atomic::Ordering;

fn make_progress_pair() -> (
    std::sync::Arc<crate::tools::progress::ProgressReporter>,
    std::sync::Arc<CountingSink>,
) {
    let sink = std::sync::Arc::new(CountingSink::default());
    let reporter = crate::tools::progress::ProgressReporter::with_sink(
        sink.clone(),
        rmcp::model::NumberOrString::Number(1),
    );
    (reporter, sink)
}

async fn project_ctx_with_progress(
) -> (tempfile::TempDir, ToolContext, std::sync::Arc<CountingSink>) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    let (reporter, sink) = make_progress_pair();
    let ctx = ToolContext {
        agent,
        lsp: LspManager::new_arc(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: Some(reporter),
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    };
    (dir, ctx, sink)
}

#[tokio::test]
async fn semantic_search_emits_progress_text() {
    // SemanticSearch calls report_text("loading embedding model") and report_text("searching")
    // even when the embedder itself fails. Verify the text sink fires at least once.
    let (_dir, ctx, sink) = project_ctx_with_progress().await;
    let _ = SemanticSearch
        .call(json!({"query": "test function"}), &ctx)
        .await;
    assert!(
        sink.text_calls.load(Ordering::Relaxed) >= 1,
        "expected at least 1 report_text() call from semantic_search"
    );
}

#[tokio::test]
async fn index_project_emits_progress_on_start() {
    // Progress notifications are disabled (BUG-038: crashes Claude Code 2.x).
    // Verify that index_project still returns "started" without crashing.
    let (_dir, ctx, sink) = project_ctx_with_progress().await;
    let result = IndexProject.call(json!({}), &ctx).await;
    assert!(result.is_ok());
    assert_eq!(
        sink.progress_calls.load(Ordering::Relaxed),
        0,
        "progress should be disabled (BUG-038)"
    );
}

// --- Preflight elicitation tests (T7) ---

#[tokio::test]
async fn index_project_no_elicit_for_normal_project() {
    // A tiny project well under an explicit 10 000-byte threshold.
    // Writing project.toml makes the coupling explicit: preflight must
    // read the threshold, compare it to the ~14-byte file, decide Clear,
    // and never trigger elicitation.  If elicitation WERE triggered,
    // ctx.peer = None would cause the tool to abort with
    // "does not support elicitation" — caught by the Err arm below.
    let (dir, mut ctx) = project_ctx().await;

    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    std::fs::write(
        cs_dir.join("project.toml"),
        "[project]\nname = \"test\"\n\n[security]\nmax_index_bytes = 10000\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    // Rebuild agent so it picks up the new project.toml.
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    ctx.agent = agent;

    let result = IndexProject.call(json!({}), &ctx).await;
    match &result {
        Ok(_) => { /* indexing ran through preflight — test passes */ }
        Err(e) => {
            let msg = format!("{e:?}");
            assert!(
                !msg.contains("client does not support elicitation"),
                "preflight should not have elicited for a tiny project: {msg}"
            );
        }
    }
}

#[tokio::test]
async fn index_project_aborts_when_elicit_unavailable_on_oversized_root() {
    // Force max_index_bytes = 0 so any non-empty directory triggers
    // RequiresConfirmation. With peer = None, elicit returns Ok(None)
    // and the tool must refuse with a clear error rather than proceeding.
    let (dir, mut ctx) = project_ctx().await;

    let cs_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&cs_dir).unwrap();
    std::fs::write(
        cs_dir.join("project.toml"),
        "[project]\nname = \"test\"\n\n[security]\nmax_index_bytes = 0\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}\n").unwrap();

    // Rebuild agent so it picks up the new project.toml.
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
    ctx.agent = agent;

    let err = IndexProject.call(json!({}), &ctx).await.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("does not support elicitation") || msg.contains("user did not confirm"),
        "expected elicit-unavailable abort, got: {msg}"
    );
    // TODO: add confirm=true and confirm=false tests once a MockPeer exists.
}

#[tokio::test]
async fn index_action_unknown_errors() {
    let (_dir, ctx) = project_ctx().await;
    let err = Index
        .call(json!({ "action": "wat" }), &ctx)
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("unknown index action"),
        "expected unknown action error, got: {err}"
    );
}

#[tokio::test]
async fn index_action_missing_errors() {
    let (_dir, ctx) = project_ctx().await;
    let err = Index.call(json!({}), &ctx).await.unwrap_err();
    assert!(
        err.to_string().contains("index requires 'action'"),
        "expected missing action error, got: {err}"
    );
}

#[test]
fn index_is_write_depends_on_action() {
    assert!(Index.is_write(&json!({ "action": "build" })));
    assert!(!Index.is_write(&json!({ "action": "status" })));
    assert!(!Index.is_write(&json!({})));
}
