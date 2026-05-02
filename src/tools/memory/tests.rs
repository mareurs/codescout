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

async fn test_ctx_with_project() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    // Create .codescout dir so MemoryStore::open works
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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
        },
    )
}

async fn test_ctx_no_project() -> ToolContext {
    ToolContext {
        agent: Agent::new(None).await.unwrap(),
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
    }
}

#[tokio::test]
async fn write_and_read_roundtrip() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let result = WriteMemory
        .call(
            json!({
                "topic": "test-topic",
                "content": "hello memory"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result, "ok");

    let result = ReadMemory
        .call(json!({ "topic": "test-topic" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["content"], "hello memory");
}

#[tokio::test]
async fn read_missing_returns_null() {
    let (_dir, ctx) = test_ctx_with_project().await;
    let err = ReadMemory
        .call(json!({ "topic": "nonexistent" }), &ctx)
        .await;
    assert!(err.is_err());
    let msg = err.unwrap_err().to_string();
    assert!(msg.contains("nonexistent"), "got: {msg}");
}

#[tokio::test]
async fn list_after_writes() {
    let (_dir, ctx) = test_ctx_with_project().await;
    WriteMemory
        .call(json!({ "topic": "b-topic", "content": "b" }), &ctx)
        .await
        .unwrap();
    WriteMemory
        .call(json!({ "topic": "a-topic", "content": "a" }), &ctx)
        .await
        .unwrap();

    let result = ListMemories.call(json!({}), &ctx).await.unwrap();
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
    WriteMemory
        .call(json!({ "topic": "to-delete", "content": "bye" }), &ctx)
        .await
        .unwrap();
    DeleteMemory
        .call(json!({ "topic": "to-delete" }), &ctx)
        .await
        .unwrap();

    let err = ReadMemory.call(json!({ "topic": "to-delete" }), &ctx).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn tools_error_without_active_project() {
    let ctx = test_ctx_no_project().await;
    assert!(WriteMemory
        .call(json!({ "topic": "x", "content": "y" }), &ctx)
        .await
        .is_err());
    assert!(ReadMemory
        .call(json!({ "topic": "x" }), &ctx)
        .await
        .is_err());
    assert!(ListMemories.call(json!({}), &ctx).await.is_err());
    assert!(DeleteMemory
        .call(json!({ "topic": "x" }), &ctx)
        .await
        .is_err());
}

#[tokio::test]
async fn nested_topic_works() {
    let (_dir, ctx) = test_ctx_with_project().await;
    WriteMemory
        .call(
            json!({
                "topic": "debugging/async-patterns",
                "content": "avoid blocking the runtime"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let result = ReadMemory
        .call(json!({ "topic": "debugging/async-patterns" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["content"], "avoid blocking the runtime");
}

#[test]
fn list_memories_format_compact() {
    use serde_json::json;
    let tool = ListMemories;
    let r = json!({ "topics": ["a", "b", "c"] });
    let t = tool.format_compact(&r).unwrap();
    assert!(t.contains("3"), "got: {t}");
}

#[test]
fn write_memory_schema_has_private_field() {
    let schema = WriteMemory.input_schema();
    assert!(schema["properties"]["private"].is_object());
    assert_eq!(schema["properties"]["private"]["type"], "boolean");
}

#[test]
fn read_memory_schema_has_private_field() {
    let schema = ReadMemory.input_schema();
    assert!(schema["properties"]["private"].is_object());
    assert_eq!(schema["properties"]["private"]["type"], "boolean");
}

#[test]
fn delete_memory_schema_has_private_field() {
    let schema = DeleteMemory.input_schema();
    assert!(schema["properties"]["private"].is_object());
    assert_eq!(schema["properties"]["private"]["type"], "boolean");
}

#[test]
fn list_memories_schema_has_include_private_field() {
    let schema = ListMemories.input_schema();
    assert!(schema["properties"]["include_private"].is_object());
    assert_eq!(schema["properties"]["include_private"]["type"], "boolean");
}

#[tokio::test]
async fn write_private_goes_to_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    WriteMemory
        .call(
            json!({"topic": "prefs", "content": "verbose", "private": true}),
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

#[tokio::test]
async fn read_private_reads_from_private_store() {
    let (_dir, ctx) = test_ctx_with_project().await;
    ctx.agent
        .with_project(|p| p.private_memory.write("wip", "issue-42"))
        .await
        .unwrap();
    let result = ReadMemory
        .call(json!({"topic": "wip", "private": true}), &ctx)
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
    let err = ReadMemory
        .call(json!({"topic": "shared-topic", "private": true}), &ctx)
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
    DeleteMemory
        .call(json!({"topic": "tmp", "private": true}), &ctx)
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
    DeleteMemory
        .call(json!({"topic": "tmp", "private": true}), &ctx)
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
    let result = ListMemories.call(json!({}), &ctx).await.unwrap();
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
    let result = ListMemories
        .call(json!({"include_private": true}), &ctx)
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
    let result = ListMemories
        .call(json!({"include_private": true}), &ctx)
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
    assert!(body.len() % 3 == 0, "should truncate at char boundary");
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
    };

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
