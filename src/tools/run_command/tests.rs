use super::inner::classify_slow_command;
use super::*;
use crate::agent::Agent;
use crate::prompts::builders::{
    build_buffered_onboarding_instructions, build_buffered_refresh_instructions, build_heading_map,
    build_language_patterns_memory, build_per_project_prompt, build_prompt_refresh_subagent_prompt,
    build_subagent_epilogue, build_subagent_preamble, build_synthesis_prompt,
    build_system_prompt_draft, build_workspace_instructions, language_patterns,
};
use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
use crate::tools::core::types::is_subagent_capable_name;
use crate::tools::onboarding::{
    gather_project_context, onboarding_version_stale, Onboarding, ONBOARDING_VERSION,
};
#[test]
fn system_prompt_draft_includes_per_project_memory_refs() {
    use std::path::PathBuf;
    let projects = vec![
        crate::workspace::DiscoveredProject {
            id: "api".to_string(),
            relative_root: PathBuf::from("api"),
            languages: vec!["rust".to_string()],
            manifest: Some("Cargo.toml".to_string()),
        },
        crate::workspace::DiscoveredProject {
            id: "web".to_string(),
            relative_root: PathBuf::from("web"),
            languages: vec!["typescript".to_string()],
            manifest: Some("package.json".to_string()),
        },
    ];
    let draft = build_system_prompt_draft(
        &["rust".to_string(), "typescript".to_string()],
        &[],
        None,
        Some(&projects),
        &Vec::new(),
    );
    assert!(
        draft.contains("memory(project_id="),
        "should reference per-project memories"
    );
    assert!(draft.contains("api"), "should mention api project");
    assert!(draft.contains("web"), "should mention web project");
}

#[test]
fn subagent_preamble_contains_activate_project() {
    let preamble = build_subagent_preamble();
    assert!(
        preamble.contains("onboarding subagent"),
        "preamble must identify the subagent role"
    );
    assert!(
        preamble.contains("workspace(action=\"activate\""),
        "preamble must instruct subagent to activate project"
    );
    assert!(
        preamble.contains("read_only=false"),
        "preamble must request write access"
    );
}

#[test]
fn subagent_epilogue_contains_return_contract() {
    let epilogue = build_subagent_epilogue();
    assert!(
        epilogue.contains("Exploration Summary"),
        "epilogue must define exploration summary format"
    );
    assert!(
        epilogue.contains("Memories Written"),
        "epilogue must request memory list"
    );
    assert!(
        epilogue.contains("workspace(action=\"activate\""),
        "epilogue must instruct subagent to restore project state"
    );
}

#[test]
fn version_needs_refresh_when_none() {
    assert!(onboarding_version_stale(None));
}

#[test]
fn version_needs_refresh_when_old() {
    assert!(onboarding_version_stale(Some(0)));
}

#[test]
fn version_current_when_equal() {
    assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION)));
}

#[test]
fn version_current_when_newer_than_compiled() {
    assert!(!onboarding_version_stale(Some(ONBOARDING_VERSION + 1)));
}

#[test]
fn prompt_refresh_subagent_prompt_contains_memory_reads() {
    let topics = vec!["architecture".to_string(), "conventions".to_string()];
    let prompt = build_prompt_refresh_subagent_prompt(&topics);
    assert!(prompt.contains("workspace(action=\"activate\""));
    assert!(prompt.contains("architecture"));
    assert!(prompt.contains("conventions"));
    assert!(prompt.contains("system-prompt.md"));
    assert!(prompt.contains("Do NOT re-explore"));
}

#[test]
fn is_subagent_capable_detects_claude() {
    assert!(is_subagent_capable_name(Some("claude-code")));
    assert!(is_subagent_capable_name(Some("Claude Code")));
    assert!(is_subagent_capable_name(Some("claude-code-ide")));
    assert!(!is_subagent_capable_name(Some("cursor")));
    assert!(!is_subagent_capable_name(Some("copilot")));
    assert!(!is_subagent_capable_name(Some("windsurf")));
    assert!(!is_subagent_capable_name(None));
}

#[test]
fn build_heading_map_extracts_level2_headings() {
    let prompt = "# Title\n\nIntro text.\n\n## Phase 1: Explore\nStep 1.\nStep 2.\nMore.\n\n## Phase 2: Write\nA.\nB.\n\n## After\nFinal.\n";
    let sections = build_heading_map(prompt);
    assert_eq!(sections.len(), 3);
    assert!(sections[0].starts_with("1. ## Phase 1: Explore"));
    assert!(sections[0].contains("lines)"));
    assert!(sections[1].starts_with("2. ## Phase 2: Write"));
    assert!(sections[2].starts_with("3. ## After"));
}

#[test]
fn build_buffered_onboarding_instructions_claude() {
    let instructions =
        build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", true);
    assert!(
        instructions.contains(".codescout/tmp/onboarding-prompt.md"),
        "must contain the prompt path"
    );
    assert!(
        instructions.contains("subagent"),
        "Claude instructions must mention subagent"
    );
    assert!(
        instructions.contains("read_markdown"),
        "must tell how to read via read_markdown"
    );
    // Must have numbered checklist
    assert!(
        instructions.contains("1. read_markdown"),
        "must have numbered phase checklist"
    );
    assert!(
        instructions.contains("## THE IRON LAW"),
        "checklist must start with THE IRON LAW"
    );
    assert!(
        instructions.contains("## Return Contract"),
        "checklist must end with Return Contract"
    );
}

#[test]
fn build_buffered_onboarding_instructions_generic() {
    let instructions =
        build_buffered_onboarding_instructions(".codescout/tmp/onboarding-prompt.md", false);
    assert!(
        instructions.contains(".codescout/tmp/onboarding-prompt.md"),
        "must contain the prompt path"
    );
    assert!(
        !instructions.contains("subagent"),
        "generic instructions must NOT mention subagent"
    );
    assert!(
        instructions.contains("read_markdown"),
        "must tell how to read via read_markdown"
    );
    // Must have numbered checklist
    assert!(
        instructions.contains("1. read_markdown"),
        "must have numbered phase checklist"
    );
}

#[test]
fn build_buffered_refresh_instructions_claude() {
    let instructions = build_buffered_refresh_instructions(
        ".codescout/tmp/onboarding-prompt.md",
        Some(1),
        2,
        true,
    );
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
    assert!(instructions.contains("v1"));
    assert!(instructions.contains("v2"));
    assert!(instructions.contains("subagent"));
    assert!(instructions.contains("read_markdown"));
    assert!(!instructions.contains("read_file"));
}

#[test]
fn build_buffered_refresh_instructions_generic() {
    let instructions =
        build_buffered_refresh_instructions(".codescout/tmp/onboarding-prompt.md", None, 2, false);
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
    assert!(instructions.contains("pre-versioning"));
    assert!(!instructions.contains("subagent"));
    assert!(instructions.contains("read_markdown"));
    assert!(!instructions.contains("read_file"));
}

#[test]
fn build_per_project_prompt_contains_project_context() {
    let project = crate::workspace::DiscoveredProject {
        id: "backend".to_string(),
        relative_root: std::path::PathBuf::from("."),
        languages: vec!["kotlin".to_string(), "java".to_string()],
        manifest: Some("build.gradle.kts".to_string()),
    };
    let siblings = vec![
        ("mcp-server".to_string(), vec!["rust".to_string()]),
        ("python-svc".to_string(), vec!["python".to_string()]),
    ];
    let prompt = build_per_project_prompt(&project, &siblings);

    // Must contain project identity
    assert!(prompt.contains("backend"), "must contain project id");
    assert!(prompt.contains("kotlin"), "must contain languages");
    assert!(prompt.contains("build.gradle.kts"), "must contain manifest");

    // Must contain sibling info (for context, not deep-diving)
    assert!(prompt.contains("mcp-server"), "must mention siblings");
    assert!(
        prompt.contains("Do NOT deep-dive"),
        "must warn against sibling deep-dives"
    );

    // Must contain exploration steps
    assert!(
        prompt.contains("## Phase 2: Explore"),
        "must contain exploration phase"
    );
    assert!(
        prompt.contains("symbols"),
        "must contain exploration instructions"
    );

    // Must contain memory writing instructions
    assert!(
        prompt.contains("## Phase 3: Write"),
        "must contain memory phase"
    );
    assert!(
        prompt.contains("project_id=\"backend\""),
        "must scope memories to project"
    );

    assert!(
        !prompt.contains("project=\""),
        "must NOT emit the bare project= param - it is silently ignored (2026-06-09 onboarding bug)"
    );

    // Must contain iron law
    assert!(prompt.contains("IRON LAW"), "must contain iron law");

    // Must contain return contract
    assert!(
        prompt.contains("## Return Contract"),
        "must contain return contract"
    );

    // Must NOT contain workspace synthesis instructions
    assert!(
        !prompt.contains("Workspace Memory Synthesis"),
        "must NOT contain workspace synthesis"
    );
}

#[test]
fn build_synthesis_prompt_contains_readback_and_claude_md() {
    let projects = vec![
        ("backend".to_string(), vec!["kotlin".to_string()]),
        ("mcp-server".to_string(), vec!["rust".to_string()]),
    ];
    let prompt = build_synthesis_prompt(&projects);

    // Must contain memory readback commands for each project
    assert!(prompt.contains("memory(action=\"read\", project_id=\"backend\""));
    assert!(prompt.contains("memory(action=\"read\", project_id=\"mcp-server\""));

    assert!(
        !prompt.contains("project=\""),
        "synthesis prompt must NOT emit the bare project= param (2026-06-09 onboarding bug)"
    );

    // Must contain workspace memory topics
    assert!(prompt.contains("architecture"));
    assert!(prompt.contains("conventions"));
    assert!(prompt.contains("development-commands"));
    assert!(prompt.contains("domain-glossary"));
    assert!(prompt.contains("gotchas"));

    // Must contain CLAUDE.md refresh instructions
    assert!(
        prompt.contains("CLAUDE.md"),
        "must include CLAUDE.md refresh"
    );
    assert!(
        prompt.contains("preserve"),
        "must mention preserving user content"
    );

    // Must contain system prompt generation
    assert!(prompt.contains("system-prompt"));
}

#[test]
fn build_workspace_instructions_claude_contains_parallel_dispatch() {
    let project_prompts = vec![
        (
            "backend".to_string(),
            ".codescout/tmp/onboarding-project-backend.md".to_string(),
        ),
        (
            "mcp".to_string(),
            ".codescout/tmp/onboarding-project-mcp.md".to_string(),
        ),
    ];
    let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
    let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
    let instructions =
        build_workspace_instructions(main_prompt_path, &project_prompts, synthesis_path, true);

    // Must mention parallel dispatch
    assert!(instructions.contains("parallel") || instructions.contains("PARALLEL"));
    // Must reference each project prompt
    assert!(instructions.contains("onboarding-project-backend.md"));
    assert!(instructions.contains("onboarding-project-mcp.md"));
    // Must reference synthesis prompt
    assert!(instructions.contains("onboarding-workspace-synthesis.md"));
    // Must reference Phase 0-1 from main prompt
    assert!(instructions.contains("Phase 0") || instructions.contains("Phase 1"));
    // Must mention subagent
    assert!(instructions.contains("subagent"));
}

#[test]
fn build_workspace_instructions_generic_is_sequential() {
    let project_prompts = vec![(
        "backend".to_string(),
        ".codescout/tmp/onboarding-project-backend.md".to_string(),
    )];
    let synthesis_path = ".codescout/tmp/onboarding-workspace-synthesis.md";
    let main_prompt_path = ".codescout/tmp/onboarding-prompt.md";
    let instructions =
        build_workspace_instructions(main_prompt_path, &project_prompts, synthesis_path, false);

    assert!(!instructions.contains("subagent"));
    assert!(instructions.contains("onboarding-project-backend.md"));
    assert!(instructions.contains("read_markdown"));
}

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
    crate::lsp::LspManager::new_arc()
}

async fn project_ctx() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    // Create some source files for language detection
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("lib.py"), "def hello(): pass").unwrap();
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
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        },
    )
}

/// Like project_ctx() but uses the given directory as the project root.
/// Caller is responsible for keeping the tempdir alive.
async fn project_ctx_at(root: &std::path::Path) -> ToolContext {
    std::fs::create_dir_all(root.join(".codescout")).unwrap();
    std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
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
    }
}

/// Create a two-project workspace layout in the given directory.
/// Returns (api_dir, web_dir).
fn setup_workspace_dirs(root: &std::path::Path) -> (PathBuf, PathBuf) {
    let api_dir = root.join("api");
    std::fs::create_dir_all(api_dir.join("src")).unwrap();
    std::fs::write(api_dir.join("Cargo.toml"), "[package]\nname = \"api\"").unwrap();
    std::fs::write(api_dir.join("src/main.rs"), "fn main() {}").unwrap();
    let web_dir = root.join("web");
    std::fs::create_dir_all(web_dir.join("src")).unwrap();
    std::fs::write(
        web_dir.join("package.json"),
        r#"{"name":"web","scripts":{"build":"tsc"}}"#,
    )
    .unwrap();
    std::fs::write(web_dir.join("src/index.ts"), "console.log('hello')").unwrap();
    (api_dir, web_dir)
}

#[tokio::test]
async fn onboarding_detects_languages() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    let langs: Vec<&str> = result["languages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(langs.contains(&"rust"));
    assert!(langs.contains(&"python"));
}

#[tokio::test]
async fn onboarding_creates_config() {
    let (dir, ctx) = project_ctx().await;
    // Remove the config if it exists
    let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result["config_created"], true);
    assert!(dir.path().join(".codescout/project.toml").exists());
}

#[tokio::test]
async fn onboarding_returns_status_when_already_done() {
    let (dir, ctx) = project_ctx().await;
    let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

    // First call does full onboarding
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert!(result.get("languages").is_some()); // full onboarding result

    // Second call (no force) returns status instead
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result["onboarded"], true);
    assert_eq!(result["has_config"], true);
    assert_eq!(result["has_onboarding_memory"], true);

    // Force re-scan
    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert!(result.get("languages").is_some()); // full onboarding again
}
#[tokio::test]
async fn onboarding_returns_instruction_prompt() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("## Rules"));
    assert!(prompt.contains("### project-scope: project-overview"));
    assert!(prompt.contains("rust")); // detected language
}

#[tokio::test]
async fn onboarding_returns_subagent_prompt_and_instructions() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // New fields must exist
    assert!(
        result.get("subagent_prompt").is_some(),
        "response must include subagent_prompt"
    );
    assert!(
        result["subagent_prompt"].is_string(),
        "subagent_prompt must be a string"
    );
    // Old fields must be gone
    assert!(
        result.get("instructions").is_none(),
        "instructions field must be removed"
    );
    assert!(
        result.get("system_prompt_draft").is_none(),
        "system_prompt_draft must be removed"
    );

    // subagent_prompt must contain preamble, body, and epilogue
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("workspace(action=\"activate\""),
        "subagent_prompt must contain preamble"
    );
    assert!(
        prompt.contains("## Return Contract"),
        "subagent_prompt must contain epilogue"
    );
    assert!(
        prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
        "subagent_prompt must contain onboarding prompt body"
    );
    assert!(
        prompt.contains("## System Prompt Draft"),
        "subagent_prompt must contain system prompt draft section"
    );

    // Lightweight metadata still present
    assert!(result.get("languages").is_some());
    assert!(result.get("config_created").is_some());
}

#[tokio::test]
async fn onboarding_errors_without_project() {
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
    assert!(Onboarding.call(json!({}), &ctx).await.is_err());
}

#[tokio::test]
async fn onboarding_status_includes_memories_and_message() {
    let (_dir, ctx) = project_ctx().await;

    // Run onboarding first
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Status call returns guidance message and memories
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    let msg = result["message"].as_str().unwrap();
    assert!(msg.contains("already performed"));
    assert!(!result["memories"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn onboarding_status_includes_private_memories_when_present() {
    let (_dir, ctx) = project_ctx().await;

    // Run full onboarding first (creates config + onboarding memory)
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Seed a private memory
    ctx.agent
        .with_project(|p| p.private_memory.write("my-prefs", "verbose"))
        .await
        .unwrap();

    // Fast-path status call should include private memories
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert!(result["onboarded"].as_bool().unwrap_or(false));
    let private = result["private_memories"].as_array().unwrap();
    assert!(private.iter().any(|v| v.as_str() == Some("my-prefs")));
    assert!(result["message"].as_str().unwrap().contains("my-prefs"));
}

#[tokio::test]
async fn onboarding_status_omits_private_memories_field_when_empty() {
    let (_dir, ctx) = project_ctx().await;

    // Run full onboarding first (creates config + onboarding memory), no private memory
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Fast-path status call should NOT include private_memories field
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert!(result["onboarded"].as_bool().unwrap_or(false));
    assert!(result["private_memories"].is_null());
    assert!(!result["message"].as_str().unwrap().contains("private"));
}

#[tokio::test]
async fn onboarding_call_content_delivers_message_when_already_done() {
    let (_dir, ctx) = project_ctx().await;

    // First call does full onboarding (creates config + writes memory)
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Second call (no force) — call_content must deliver the message, not "[?]"
    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        text.contains("already performed"),
        "expected already-onboarded message, got: {text:?}"
    );
    assert!(
        text.contains("onboarding"),
        "expected memory list in message, got: {text:?}"
    );
    assert!(
        !text.contains("[?]"),
        "call_content must not emit [?] placeholder, got: {text:?}"
    );
}

#[tokio::test]
async fn onboarding_call_content_writes_prompt_file() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Must return exactly 1 block
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("block must be valid JSON");

    // Must have prompt_path pointing at the markdown file
    let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "response must contain prompt_path with onboarding-prompt.md, got: {}",
        &text[..text.len().min(200)]
    );

    // Must contain read_markdown instructions
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown"),
        "response must contain read_markdown instructions"
    );
    assert!(
        !instructions.contains("read_file"),
        "response must NOT contain read_file instructions"
    );

    // Must NOT contain output_id (@tool_ ref)
    assert!(
        parsed.get("output_id").is_none(),
        "response must NOT have output_id"
    );

    // Must NOT contain raw prompt body content (heading names in sections[] are ok)
    assert!(
        !text.contains("REQUIRED_KEYS") && !text.contains("subagent_prompt"),
        "response must NOT contain raw prompt body content (should be in file)"
    );
}

#[tokio::test]
async fn onboarding_call_content_writes_markdown_file() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");

    let prompt_path = parsed["prompt_path"]
        .as_str()
        .expect("must have prompt_path");
    assert!(prompt_path.contains("onboarding-prompt.md"));
    assert!(parsed.get("output_id").is_none(), "must NOT have output_id");

    let root = ctx.agent.project_root().await.unwrap();
    let full_path = root.join(prompt_path);
    assert!(full_path.exists());

    let sections = parsed["sections"].as_array().expect("must have sections");
    assert!(!sections.is_empty());

    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(instructions.contains("read_markdown"));
}

#[tokio::test]
async fn onboarding_status_includes_per_project_memories_for_workspace() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);
    let ctx = project_ctx_at(root).await;

    // Full workspace onboarding — writes per-project onboarding memories
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Second call hits the already-onboarded fast path
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert!(result["onboarded"].as_bool().unwrap_or(false));

    // project_memories field is present and non-empty
    let pm = &result["project_memories"];
    assert!(
        pm.is_object(),
        "expected project_memories object, got: {pm}"
    );
    assert!(
        !pm.as_object().unwrap().is_empty(),
        "project_memories should be non-empty after workspace onboarding"
    );

    // Message mentions per-project memories and the project_id param hint
    let msg = result["message"].as_str().unwrap();
    assert!(
        msg.contains("Per-project memories"),
        "message should mention per-project memories"
    );
    assert!(
        msg.contains("project_id="),
        "message should include project_id scoping hint"
    );
}

#[tokio::test]
async fn onboarding_call_content_force_delivers_instructions() {
    let (_dir, ctx) = project_ctx().await;

    // force=true must always deliver the full instructions, never "[?]"
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    assert!(
        !text.contains("[?]"),
        "call_content must not emit [?] placeholder, got: {text:?}"
    );

    // Must be valid JSON with prompt_path and instructions
    let parsed: serde_json::Value =
        serde_json::from_str(text).expect("call_content block must be valid JSON");
    assert!(
        parsed["prompt_path"]
            .as_str()
            .is_some_and(|s| s.contains("onboarding-prompt.md")),
        "must have prompt_path pointing to onboarding-prompt.md, got: {:?}",
        parsed["prompt_path"]
    );
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown") || instructions.contains("subagent"),
        "instructions must guide the agent, got: {instructions:?}"
    );
    assert!(
        !instructions.contains("read_file"),
        "instructions must NOT reference read_file, got: {instructions:?}"
    );
}

#[tokio::test]
async fn onboarding_call_content_returns_two_blocks() {
    // Test name kept for history; new contract is 1 structured JSON block.
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Must return exactly 1 content block (file path)
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("block must be valid JSON");

    // prompt_path must point to the markdown file
    let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "prompt_path must contain onboarding-prompt.md, got: {prompt_path:?}"
    );

    // sections must be present and non-empty
    let empty = vec![];
    let sections = parsed["sections"].as_array().unwrap_or(&empty);
    assert!(!sections.is_empty(), "sections must be non-empty");

    // instructions must not contain raw subagent prompt body (long prose),
    // but may reference heading names in the checklist.
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        !instructions.contains("NO MEMORIES WRITTEN WITHOUT COMPLETING"),
        "instructions must NOT contain raw prompt body (should be in file)"
    );

    // instructions must reference read_markdown
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown"
    );
}

// ---- Task 5 tests: refresh_prompt parameter ----

/// Helper: build a fully onboarded project context (config + onboarding memory written).
/// `project_ctx()` creates an empty project — we need to run full onboarding first so
/// the fast-path checks (has_config && has_onboarding_memory) pass.
async fn onboarded_project_ctx() -> (tempfile::TempDir, ToolContext) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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
    // Run full onboarding to write config + onboarding memory
    Onboarding.call(json!({}), &ctx).await.unwrap();
    (dir, ctx)
}

#[tokio::test]
async fn refresh_prompt_on_onboarded_project_returns_refresh_response() {
    let (_dir, ctx) = onboarded_project_ctx().await;

    // refresh_prompt=true must trigger the refresh path even when version is current
    let result = Onboarding
        .call(json!({ "refresh_prompt": true }), &ctx)
        .await
        .unwrap();

    assert!(
        result["onboarded"].as_bool().unwrap_or(false),
        "onboarded must be true"
    );
    assert!(
        result["explicit_refresh"].as_bool().unwrap_or(false),
        "explicit_refresh flag must be set"
    );
    assert!(
        result.get("subagent_prompt").is_some(),
        "must include subagent_prompt"
    );
    assert!(
        result["subagent_prompt"]
            .as_str()
            .unwrap()
            .contains("workspace(action=\"activate\""),
        "subagent_prompt must contain workspace activate"
    );
}

#[tokio::test]
async fn refresh_prompt_on_unonboarded_project_returns_error() {
    // No config, no memories — project_ctx() gives us a bare project dir
    let (_dir, ctx) = project_ctx().await;

    let err = Onboarding
        .call(json!({ "refresh_prompt": true }), &ctx)
        .await
        .unwrap_err();

    let recoverable = err
        .downcast::<crate::tools::RecoverableError>()
        .expect("expected RecoverableError for refresh_prompt on unonboarded project");
    assert!(
        recoverable.message.contains("fully onboarded"),
        "error message must mention fully onboarded, got: {:?}",
        recoverable.message
    );
}

#[tokio::test]
async fn force_takes_priority_over_refresh_prompt() {
    // force=true + refresh_prompt=true must do a full re-scan, not a lightweight refresh.
    // project_ctx() is fine: force=true bypasses the onboarding check entirely.
    let (_dir, ctx) = project_ctx().await;

    let result = Onboarding
        .call(json!({ "force": true, "refresh_prompt": true }), &ctx)
        .await
        .unwrap();

    // Full onboarding result must NOT have explicit_refresh
    assert!(
        result.get("explicit_refresh").is_none(),
        "explicit_refresh must not be set on force path"
    );
    // Full onboarding result has languages, subagent_prompt with "Explore the Code"
    let prompt = result["subagent_prompt"].as_str().unwrap_or("");
    assert!(
        prompt.contains("Explore the Code") || prompt.contains("Memories to Create"),
        "full onboarding subagent_prompt must contain onboarding body, got: {prompt:?}"
    );
}

// ---- Task 6 test: call_content routing for version refresh ----

#[tokio::test]
async fn onboarding_call_content_returns_two_blocks_for_version_refresh() {
    // Test name kept for history; new contract is 1 structured JSON block.
    let (_dir, ctx) = onboarded_project_ctx().await;

    // Manually write a stale (version=None) config to disk, then reload so the
    // agent's in-memory config reflects the stale state.
    let config_path = ctx
        .agent
        .with_project(|p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
            config.project.onboarding_version = None;
            let toml_str = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, &toml_str)?;
            Ok(config_path)
        })
        .await
        .unwrap();
    ctx.agent.reload_config_if_project_toml(&config_path).await;

    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();

    assert_eq!(
        content.len(),
        1,
        "version refresh must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("block must be valid JSON");

    // Must have a prompt_path
    assert!(
        parsed["prompt_path"]
            .as_str()
            .is_some_and(|s| s.contains("onboarding-prompt.md")),
        "must have prompt_path, got: {:?}",
        parsed["prompt_path"]
    );

    // Must NOT have output_id
    assert!(parsed.get("output_id").is_none(), "must NOT have output_id");

    // instructions must contain version info
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("v2")
            || instructions.contains("outdated")
            || instructions.contains("refresh"),
        "instructions must contain version info, got: {instructions:?}"
    );

    // instructions must reference read_markdown
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown, got: {instructions:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn execute_shell_command_timeout_is_enforced() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "sleep 10", "timeout_secs": 1 }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["timed_out"], true, "command should have timed out");
    assert!(result["stderr"]
        .as_str()
        .unwrap()
        .contains("timed out after 1 seconds"));
    let hint = result["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains("run_in_background"),
        "timeout hint should mention run_in_background, got: {hint}"
    );
}

// --- run_command progress test (T11) ---

use crate::tools::progress::test_support::CountingSink;
use std::sync::atomic::Ordering;

async fn project_ctx_with_progress(
) -> (tempfile::TempDir, ToolContext, std::sync::Arc<CountingSink>) {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    let agent = crate::agent::Agent::new(Some(dir.path().to_path_buf()))
        .await
        .unwrap();
    let sink = std::sync::Arc::new(CountingSink::default());
    let reporter = crate::tools::progress::ProgressReporter::with_sink(
        sink.clone(),
        rmcp::model::NumberOrString::Number(1),
    );
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: Some(reporter),
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };
    (dir, ctx, sink)
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_heartbeat_emits_progress_text() {
    // The heartbeat task fires report_text("Xs elapsed") every 3s.
    // We use a 5s sleep with a 6s timeout so at least one heartbeat fires.
    let (_dir, ctx, sink) = project_ctx_with_progress().await;
    let _ = RunCommand
        .call(json!({"command": "sleep 5", "timeout_secs": 6}), &ctx)
        .await;
    assert!(
        sink.text_calls.load(Ordering::Relaxed) >= 1,
        "expected at least 1 report_text() from run_command heartbeat"
    );
}

#[tokio::test]
async fn execute_shell_command_fast_command_succeeds() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["timed_out"], serde_json::Value::Null);
    assert!(result["stdout"].as_str().unwrap().contains("hello"));
}

#[cfg(unix)]
#[tokio::test]
async fn execute_shell_command_output_truncated() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "seq 1 100000", "timeout_secs": 10 }),
            &ctx,
        )
        .await
        .unwrap();
    // Large output is buffered, not byte-truncated.
    assert!(
        result["output_id"].as_str().is_some(),
        "large output should be buffered with output_id"
    );
    assert!(result["hint"].is_null(), "hint field should be absent");
    assert!(
        result["total_stdout_lines"].is_null(),
        "total_stdout_lines should be absent"
    );
}

#[tokio::test]
async fn execute_shell_command_small_output_not_truncated() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    // Short output: no output_id, direct stdout
    assert_eq!(result["output_id"], serde_json::Value::Null);
    assert!(result["stdout"].as_str().unwrap().contains("hello"));
}

#[tokio::test]
async fn run_command_does_not_include_warning() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "echo test", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    assert!(
        result["warning"].is_null(),
        "run_command should not emit a warning field"
    );
}

#[tokio::test]
async fn execute_shell_command_exit_code_preserved() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "exit 42", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["exit_code"], 42);
}

#[tokio::test]
async fn execute_shell_command_echo_cross_platform() {
    let (_dir, ctx) = project_ctx().await;
    // "echo hello" works on both sh and cmd.exe
    let result = RunCommand
        .call(json!({ "command": "echo hello", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    let stdout = result["stdout"].as_str().unwrap();
    assert!(
        stdout.contains("hello"),
        "stdout should contain 'hello': {}",
        stdout
    );
}

#[test]
fn gather_context_reads_readme_and_build_file() {
    let dir = tempdir().unwrap();
    std::fs::write(
        dir.path().join("README.md"),
        "# My Project\nA test project.",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\nversion = \"0.1.0\"",
    )
    .unwrap();
    let ctx = gather_project_context(dir.path(), vec![]);
    assert_eq!(ctx.readme_path.as_deref(), Some("README.md"));
    assert_eq!(ctx.build_file_name.as_deref(), Some("Cargo.toml"));
    assert!(!ctx.claude_md_exists);
}

#[test]
fn gather_context_finds_ci_files() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".github/workflows")).unwrap();
    std::fs::write(dir.path().join(".github/workflows/ci.yml"), "name: CI").unwrap();
    let ctx = gather_project_context(dir.path(), vec![]);
    assert_eq!(ctx.ci_files, vec![".github/workflows/ci.yml"]);
}

#[test]
fn gather_context_finds_entry_points_and_test_dirs() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    let ctx = gather_project_context(dir.path(), vec![]);
    assert!(ctx.entry_points.contains(&"src/main.rs".to_string()));
    assert!(ctx.test_dirs.contains(&"tests".to_string()));
}

#[test]
fn gather_context_handles_empty_project() {
    let dir = tempdir().unwrap();
    let ctx = gather_project_context(dir.path(), vec![]);
    assert!(ctx.readme_path.is_none());
    assert!(ctx.build_file_name.is_none());
    assert!(!ctx.claude_md_exists);
    assert!(ctx.ci_files.is_empty());
    assert!(ctx.entry_points.is_empty());
    assert!(ctx.test_dirs.is_empty());
}

#[tokio::test]
async fn onboarding_returns_gathered_context_fields() {
    let dir = tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(dir.path().join("README.md"), "# Test Project").unwrap();
    std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::create_dir_all(dir.path().join("tests")).unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert_eq!(result["has_readme"], true);
    assert_eq!(result["build_file"], "Cargo.toml");
    assert!(result["test_dirs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "tests"));
    // Verify the subagent_prompt is present
    assert!(result.get("subagent_prompt").is_some());
    // Verify the subagent_prompt references key files (paths, not embedded content)
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("README.md"));
}

#[tokio::test]
async fn onboarding_includes_system_prompt_draft_in_subagent_prompt() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("README.md"), "# Test Project\nA test.").unwrap();
    std::fs::write(dir.path().join("main.py"), "print('hello')").unwrap();
    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // system_prompt_draft should NOT be a top-level field
    assert!(
        result.get("system_prompt_draft").is_none(),
        "system_prompt_draft must not be a top-level field"
    );
    // It should be embedded in subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("## System Prompt Draft"),
        "subagent_prompt should contain system prompt draft section"
    );
}

#[tokio::test]
async fn onboarding_writes_language_patterns_memory() {
    let (_dir, ctx) = project_ctx().await;
    // project_ctx creates main.rs (rust) and lib.py (python)
    let _result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Verify the language-patterns memory was written
    let memory_content = ctx
        .agent
        .with_project(|p| p.memory.read("language-patterns"))
        .await
        .unwrap()
        .expect("language-patterns memory should exist");
    assert!(
        memory_content.contains("### Rust"),
        "should contain Rust patterns"
    );
    assert!(
        memory_content.contains("### Python"),
        "should contain Python patterns"
    );
    assert!(
        memory_content.contains("Anti-patterns"),
        "should contain anti-patterns section"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_dangerous_blocked_without_acknowledge() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "rm -rf /tmp/codescout_test_nonexistent" }),
            &ctx,
        )
        .await
        .expect("dangerous command should return Ok with pending_ack");
    // Now returns a pending_ack handle instead of an error
    assert!(
        result.get("pending_ack").is_some(),
        "should have pending_ack key: {:?}",
        result
    );
    assert!(
        result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
        "pending_ack should start with @ack_: {:?}",
        result["pending_ack"]
    );
    assert!(result.get("reason").is_some(), "should have reason key");
}

#[tokio::test]
async fn run_command_dangerous_allowed_with_acknowledge() {
    let (_dir, ctx) = project_ctx().await;
    // Use a safe command but with acknowledge_risk: true — should succeed
    let result = RunCommand
        .call(
            json!({ "command": "echo safe", "acknowledge_risk": true }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result["stdout"].as_str().unwrap().contains("safe"));
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_skips_safety() {
    let (_dir, ctx) = project_ctx().await;
    // Store some output in the buffer (must exceed token budget to trigger buffering)
    let result = RunCommand
        .call(json!({ "command": "seq 1 3000", "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    let output_id = result["output_id"].as_str().unwrap();

    // grep on buffer ref only — should skip both dangerous-command check
    // and shell_command_mode check (buffer_only = true).
    let query = format!("grep '^5$' {}", output_id);
    let result2 = RunCommand
        .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    // No warning should be present when buffer_only
    // (the default mode is "warn" which adds warning for non-buffer commands)
    assert_eq!(
        result2["warning"],
        serde_json::Value::Null,
        "buffer-only queries should not get shell warning"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_cwd_works() {
    let (dir, ctx) = project_ctx().await;
    // Create a subdirectory with a file
    let sub = dir.path().join("subdir");
    std::fs::create_dir_all(&sub).unwrap();
    std::fs::write(sub.join("hello.txt"), "world").unwrap();

    let result = RunCommand
        .call(
            json!({ "command": "cat hello.txt", "cwd": "subdir", "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["stdout"].as_str().unwrap().trim(), "world");
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_cwd_rejects_traversal() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "ls", "cwd": "../../etc", "timeout_secs": 5 }),
            &ctx,
        )
        .await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("escapes project root") || err_msg.contains("not a valid directory"),
        "should reject traversal: {}",
        err_msg
    );
}

#[tokio::test]
async fn run_command_dangerous_rejected_without_ack() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({"command": "rm -rf /tmp/ce_nonexistent_test"}), &ctx)
        .await
        .expect("dangerous command should return Ok with pending_ack, not Err");
    // Previously returned Err(RecoverableError); now returns Ok with a pending_ack handle.
    assert!(
        result.get("pending_ack").is_some(),
        "should have pending_ack key: {:?}",
        result
    );
    assert!(
        result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
        "pending_ack should start with @ack_: {:?}",
        result["pending_ack"]
    );
    assert!(
        result.get("reason").is_some(),
        "should have reason key: {:?}",
        result
    );
    assert!(
        result.get("hint").is_some(),
        "should have hint key: {:?}",
        result
    );
}

#[tokio::test]
async fn dangerous_command_returns_ack_handle() {
    let (dir, ctx) = project_ctx().await;
    let root = dir.path().to_path_buf();
    let security = Default::default();
    let result = run_command_inner(
        "rm -rf /dist",
        "rm -rf /dist",
        30,
        false, // acknowledge_risk
        None,  // cwd_param
        false, // buffer_only
        false, // run_in_background
        &root,
        &security,
        &ctx,
    )
    .await
    .expect("should return Ok with pending_ack, not Err");

    assert!(
        result.get("pending_ack").is_some(),
        "should have pending_ack key"
    );
    assert!(
        result["pending_ack"].as_str().unwrap().starts_with("@ack_"),
        "pending_ack should start with @ack_: {:?}",
        result["pending_ack"]
    );
    assert!(result.get("reason").is_some(), "should have reason key");
    assert!(result.get("hint").is_some(), "should have hint key");
}

#[tokio::test]
async fn run_in_background_returns_bg_handle() {
    let (dir, ctx) = project_ctx().await;
    let root = dir.path().to_path_buf();
    let security = Default::default();

    let result = run_command_inner(
        "echo hello-bg-test",
        "echo hello-bg-test",
        30,
        false, // acknowledge_risk
        None,  // cwd_param
        false, // buffer_only
        true,  // run_in_background
        &root,
        &security,
        &ctx,
    )
    .await
    .expect("should succeed");

    let output_id = result["output_id"].as_str().expect("output_id missing");
    assert!(
        output_id.starts_with("@bg_"),
        "expected @bg_ prefix, got {output_id}"
    );
    let stdout = result["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("hello-bg-test"),
        "expected stdout to contain echo output, got: {stdout}"
    );
    let hint = result["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains(output_id),
        "hint should reference the handle, got: {hint}"
    );
}

#[tokio::test]
async fn run_in_background_rejects_buffer_only() {
    let (dir, ctx) = project_ctx().await;
    let root = dir.path().to_path_buf();
    let security = crate::util::path_security::PathSecurityConfig::default();
    let result = run_command_inner(
        "echo x", "echo x", 30, false, // acknowledge_risk
        None,  // cwd_param
        true,  // buffer_only
        true,  // run_in_background
        &root, &security, &ctx,
    )
    .await;
    let err = result.unwrap_err();
    assert!(
        err.downcast_ref::<crate::tools::RecoverableError>()
            .is_some(),
        "expected RecoverableError, got: {err}"
    );
    assert!(
        err.to_string().contains("buffer queries"),
        "error should mention buffer queries, got: {err}"
    );
}

/// A command that backgrounds a subprocess with `&` causes the foreground `output()` call
/// to hang: the background process inherits the stdout pipe FD and keeps it open until it
/// exits, preventing EOF.  With a short timeout this manifests as `timed_out: true`.
/// The hint in the response should point the caller to `run_in_background: true`.
#[cfg(unix)]
#[tokio::test]
async fn pipe_inheritance_from_shell_background_causes_timeout() {
    let (_dir, ctx) = project_ctx().await;
    // `sleep 60 &` — sh forks sleep (background), sleep inherits the stdout pipe,
    // sh exits but sleep keeps the pipe open for 60 s → output() can't get EOF.
    let result = RunCommand
        .call(json!({ "command": "sleep 60 &", "timeout_secs": 1 }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["timed_out"], true,
        "background subprocess holding pipe should cause timeout"
    );
    let hint = result["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains("run_in_background"),
        "hint should mention run_in_background, got: {hint}"
    );
}

/// `run_in_background: true` routes stdout to a log file, not a pipe, so background
/// subprocesses holding the log FD open does not block the caller.  Even a command
/// that would hang indefinitely in foreground mode returns promptly.
#[cfg(unix)]
#[tokio::test]
async fn run_in_background_avoids_pipe_inheritance_hang() {
    let (_dir, ctx) = project_ctx().await;
    // Same pattern as the timeout test, but using run_in_background: true.
    // Should return a @bg_ handle without timing out.
    let result = RunCommand
        .call(
            json!({ "command": "echo launched && sleep 60 &", "run_in_background": true }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result["timed_out"].is_null(),
        "run_in_background should not produce timed_out, got: {:?}",
        result["timed_out"]
    );
    let output_id = result["output_id"].as_str().expect("output_id missing");
    assert!(
        output_id.starts_with("@bg_"),
        "expected @bg_ handle, got: {output_id}"
    );
    // Warm-window stdout should contain the echo output.
    let stdout = result["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("launched"),
        "stdout should capture echo output within warm window, got: {stdout}"
    );
}

#[tokio::test]
async fn run_command_safe_command_not_blocked() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({"command": "echo hello"}), &ctx)
        .await;
    assert!(result.is_ok(), "echo should not be blocked: {:?}", result);
}

#[tokio::test]
async fn run_command_blocks_cat_on_source_file() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({"command": "cat src/main.rs"}), &ctx)
        .await;
    let err = result.unwrap_err();
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be a RecoverableError");
    assert!(
        rec.message.contains("source files is blocked"),
        "expected source-file block message, got: {}",
        rec.message
    );
}

#[tokio::test]
async fn run_command_source_block_bypassed_with_acknowledge_risk() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("tiny.rs"), "fn main() {}\n").unwrap();
    let result = RunCommand
        .call(
            json!({"command": "cat tiny.rs", "acknowledge_risk": true}),
            &ctx,
        )
        .await;
    assert!(
        result.is_ok(),
        "acknowledge_risk should bypass source block"
    );
}

#[tokio::test]
async fn run_command_source_block_not_triggered_for_markdown() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("README.md"), "# hello\n").unwrap();
    let result = RunCommand
        .call(json!({"command": "cat README.md"}), &ctx)
        .await;
    assert!(result.is_ok(), "cat on markdown should not be blocked");
}

#[tokio::test]
async fn run_command_source_block_not_triggered_for_non_source() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("data.txt"), "hello\n").unwrap();
    let result = RunCommand
        .call(json!({"command": "cat data.txt"}), &ctx)
        .await;
    assert!(result.is_ok(), "cat on .txt should not be blocked");
}

#[tokio::test]
async fn run_command_cwd_rejects_nonexistent_path() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({"command": "ls", "cwd": "definitely_nonexistent_subdir_xyz"}),
            &ctx,
        )
        .await;
    assert!(result.is_err(), "nonexistent cwd should be rejected");
    let err = result.unwrap_err();
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be RecoverableError");
    assert!(
        rec.message.contains("not accessible") || rec.message.contains("not a valid"),
        "got: {}",
        rec.message
    );
}

#[cfg_attr(
    target_os = "windows",
    ignore = "test uses /var as 'outside project but exists'; no Windows analog (allowed-roots vary). See docs/issues/2026-05-24-ci-windows-test-portability-rot.md"
)]
#[tokio::test]
async fn run_command_cwd_rejects_path_escaping_root() {
    let (_dir, ctx) = project_ctx().await;
    // Use /var — it always exists, is outside any temp project root, and is
    // not under /tmp (which is now an allowed cwd root).
    let result = RunCommand
        .call(json!({"command": "ls", "cwd": "/var"}), &ctx)
        .await;
    assert!(
        result.is_err(),
        "absolute cwd outside root should be rejected"
    );
    let err = result.unwrap_err();
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("should be RecoverableError");
    assert!(
        rec.message.contains("escapes project root"),
        "got: {}",
        rec.message
    );
}

#[tokio::test]
async fn run_command_buffer_only_skips_speed_bump() {
    let (_dir, ctx) = project_ctx().await;
    // Store directly in buffer — no need to run a command that may or may not buffer
    // depending on the current buffering threshold.
    let id = ctx
        .output_buffer
        .store("test_cmd".into(), "rm -rf data\n".into(), "".into(), 0);
    // "rm" appears in the buffer content, but the query command is buffer-only.
    // It should NOT be rejected as dangerous.
    let result = RunCommand
        .call(json!({"command": format!("grep rm {}", id)}), &ctx)
        .await;
    // Should succeed (or fail with grep exit 1 "not found") — but NOT as a RecoverableError
    // about dangerous commands.
    match result {
        Ok(v) => {
            assert!(
                v.get("error")
                    .map(|e| !e
                        .as_str()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains("dangerous"))
                    .unwrap_or(true),
                "buffer-only grep should not be flagged as dangerous"
            );
        }
        Err(e) => {
            let rec = e.downcast_ref::<crate::tools::RecoverableError>();
            assert!(
                rec.map(|r| !r.message.to_lowercase().contains("dangerous"))
                    .unwrap_or(false),
                "buffer-only should not fail with dangerous error"
            );
        }
    }
}

#[test]
fn run_command_schema_has_cwd_and_acknowledge_risk() {
    let schema = RunCommand.input_schema();

    let cwd = &schema["properties"]["cwd"];
    assert!(cwd.is_object(), "cwd should be a schema object");
    assert_eq!(cwd["type"], "string", "cwd type should be string");

    let ack = &schema["properties"]["acknowledge_risk"];
    assert!(
        ack.is_object(),
        "acknowledge_risk should be a schema object"
    );
    assert_eq!(
        ack["type"], "boolean",
        "acknowledge_risk type should be boolean"
    );

    let required = schema["required"].as_array().unwrap();
    assert!(
        required.iter().any(|v| v == "command"),
        "command must remain required"
    );
}

// Task 4 TDD regression tests — buffer-backed smart summaries + buffer ref execution
// -----------------------------------------------------------------------

#[tokio::test]
async fn run_command_short_output_returned_directly() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({"command": "echo hello"}), &ctx)
        .await
        .unwrap();
    assert!(
        result.get("output_id").is_none(),
        "short output should not buffer: got output_id {:?}",
        result.get("output_id")
    );
    assert!(
        result["stdout"].as_str().unwrap().contains("hello"),
        "stdout should contain 'hello': {:?}",
        result["stdout"]
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_large_output_stored_in_buffer() {
    let (_dir, ctx) = project_ctx().await;
    // seq 3000 produces ~14KB, exceeding MAX_INLINE_TOKENS * 4 (~10KB)
    let result = RunCommand
        .call(json!({"command": "seq 1 3000"}), &ctx)
        .await
        .unwrap();
    let output_id = result["output_id"]
        .as_str()
        .expect("large output should have output_id");
    assert!(
        output_id.starts_with("@cmd_"),
        "output_id should start with @cmd_: {}",
        output_id
    );
    assert!(result["hint"].is_null(), "hint field should be absent");
    assert!(
        result["total_stdout_lines"].is_null(),
        "total_stdout_lines should be absent"
    );
    let entry = ctx.output_buffer.get(output_id).unwrap();
    assert!(
        entry.stdout.contains("50\n"),
        "buffered stdout should contain '50\\n'"
    );
    assert!(
        entry.stdout.contains("3000\n"),
        "buffered stdout should contain '3000\\n'"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_ref_executes_correctly() {
    let (_dir, ctx) = project_ctx().await;
    let r1 = RunCommand
        .call(json!({"command": "seq 1 3000"}), &ctx)
        .await
        .unwrap();
    let output_id = r1["output_id"].as_str().unwrap();
    let r2 = RunCommand
        .call(
            json!({"command": format!("grep '^50$' {}", output_id)}),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(r2["exit_code"], 0, "grep should find '50': {:?}", r2);
    assert_eq!(
        r2["stdout"].as_str().unwrap().trim(),
        "50",
        "stdout should be exactly '50'"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_above_threshold_truncates_inline() {
    // BUFFER_QUERY_INLINE_CAP + 1 lines — strictly above the inline cap.
    // Must return Ok with truncated content, NOT an error or a new buffer ref.
    // Each line is padded to ~120 bytes so total exceeds the token budget.
    let (_dir, ctx) = project_ctx().await;
    let content: String = (1..=BUFFER_QUERY_INLINE_CAP + 1)
        .map(|i| format!("{i:>120}\n"))
        .collect();
    let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected Ok with truncated inline output");
    assert_eq!(
        result["truncated"], true,
        "should be truncated: {:?}",
        result
    );
    let shown = result["stdout_shown"].as_u64().unwrap() as usize;
    assert!(
        shown > 0 && shown <= BUFFER_QUERY_INLINE_CAP,
        "stdout_shown should be >0 and <=inline cap, got {shown}: {:?}",
        result
    );
    assert_eq!(
        result["stdout_total"],
        BUFFER_QUERY_INLINE_CAP + 1,
        "stdout_total should be full count: {:?}",
        result
    );
    assert!(
        result.get("output_id").is_none(),
        "must not create a new buffer ref: {:?}",
        result
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_at_threshold_returns_inline() {
    // Content exactly at MAX_INLINE_TOKENS token budget — the check is `>` not `>=`,
    // so this must return content inline, not error.
    let (_dir, ctx) = project_ctx().await;
    // Build content that is exactly MAX_INLINE_TOKENS * 4 bytes (at the limit, not over)
    let target_bytes = crate::tools::MAX_INLINE_TOKENS * 4;
    let mut content = String::new();
    for i in 1.. {
        let line = format!("{i}\n");
        if content.len() + line.len() > target_bytes {
            break;
        }
        content.push_str(&line);
    }
    let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected inline output at threshold");
    assert!(
        result.get("stdout").is_some(),
        "expected stdout field: {:?}",
        result
    );
    assert!(
        result.get("output_id").is_none(),
        "should not be buffered: {:?}",
        result
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_large_single_line_does_not_rebuffer() {
    // Regression: grep on a @tool_* ref returns the entire compact-JSON blob as
    // one line.  Even when estimated tokens are low, the byte
    // size can exceed the inline token budget.  The result must be truncated
    // inline — never stored as a new @tool_* ref (which would create an infinite
    // query loop: grep @tool_A → @tool_B → grep @tool_B → @tool_C…).
    let (_dir, ctx) = project_ctx().await;

    // Create a @cmd_* buffer whose content is one very long line (>5 KB).
    let long_line = "x".repeat(crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD + 1000);
    let id = ctx
        .output_buffer
        .store("cmd".into(), long_line, "".into(), 0);

    // cat @cmd_* triggers buffer_only; the single-line stdout exceeds the byte budget.
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("should return truncated inline result, not error");

    // Must be inline (no output_id) and must be truncated with a hint.
    assert!(
        result.get("output_id").is_none(),
        "must not create new buffer ref: {:?}",
        result
    );
    // stdout may be absent when the single line exceeded the byte budget entirely
    // (stdout_shown=0, stdout_total=1) — truncated+hint communicate the situation.
    assert_eq!(
        result.get("truncated").and_then(|v| v.as_bool()),
        Some(true),
        "must be marked truncated: {:?}",
        result
    );
    let hint = result["hint"].as_str().unwrap_or("");
    assert!(
        !hint.is_empty(),
        "hint should guide to next page or read_file: {}",
        hint
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_large_output_no_new_ref() {
    // Regression: `sed @cmd_A` that reproduces a large buffer must
    // return truncated inline content, NOT a new @cmd_B reference.
    // Use 150 lines (> BUFFER_QUERY_INLINE_CAP=100) to trigger truncation.
    let (_dir, ctx) = project_ctx().await;

    let large_content: String = (1..=250).map(|i| format!("{i:>60}\n")).collect();
    let id = ctx
        .output_buffer
        .store("original_cmd".into(), large_content, "".into(), 0);

    let result = RunCommand
        .call(
            json!({ "command": format!("sed -n '1,250p' {}", id) }),
            &ctx,
        )
        .await
        .expect("expected Ok with truncated inline output");

    assert!(
        result.get("output_id").is_none(),
        "must not create a new buffer ref: {:?}",
        result
    );
    assert_eq!(
        result["truncated"], true,
        "should be truncated: {:?}",
        result
    );
    assert_eq!(
        result["stdout_total"], 250usize,
        "stdout_total: {:?}",
        result
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_long_lines_fit_under_threshold() {
    // Regression: buffer-only queries with long lines (e.g. Java/Kotlin log output
    // with timestamps and class names, ~200 chars/line) must produce a response JSON
    // that stays under TOOL_OUTPUT_BUFFER_THRESHOLD.  Before the fix, a 100-line cap
    // on 200-char lines produced ~20 KB of stdout, which call_content() re-buffered
    // as @tool_* — creating an infinite query loop:
    //   grep @cmd_A → inline JSON (>10KB) → @tool_B → jq @tool_B → same → @tool_C…
    let (_dir, ctx) = project_ctx().await;

    // 200-char lines: typical Java log output with timestamp + class + message.
    let long_line = "x".repeat(200);
    let content: String = (0..=BUFFER_QUERY_INLINE_CAP)
        .map(|_| format!("{long_line}\n"))
        .collect();
    let id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected Ok");

    // Core assertion: the serialized JSON must fit under the re-buffering threshold.
    let json_size = serde_json::to_string(&result).unwrap().len();
    assert!(
        json_size <= crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
        "buffer_only response ({json_size} bytes) must not exceed TOOL_OUTPUT_BUFFER_THRESHOLD \
             ({} bytes) — would cause infinite @tool_* re-buffering loop",
        crate::tools::TOOL_OUTPUT_BUFFER_THRESHOLD,
    );

    // Must also avoid creating a new buffer ref.
    assert!(
        result.get("output_id").is_none(),
        "must not create a new buffer ref: {:?}",
        result
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_stderr_gets_priority() {
    // stderr = 25 lines (> 20 cap) + stdout = 250 lines (> remaining budget).
    // Expected: stderr_shown = 20, stdout_shown = 80 (BUFFER_QUERY_INLINE_CAP - 20).
    // Lines padded to ~60 bytes so total exceeds the token budget.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=250).map(|i| format!("out{i:>60}\n")).collect();
    let stderr: String = (1..=25).map(|i| format!("err{i:>60}\n")).collect();
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected Ok");
    assert_eq!(
        result["stderr_shown"], 20usize,
        "stderr_shown: {:?}",
        result
    );
    assert_eq!(
        result["stderr_total"], 25usize,
        "stderr_total: {:?}",
        result
    );
    assert_eq!(
        result["stdout_shown"],
        BUFFER_QUERY_INLINE_CAP - 20,
        "stdout_shown: {:?}",
        result
    );
    assert_eq!(
        result["stdout_total"], 250usize,
        "stdout_total: {:?}",
        result
    );
    assert_eq!(result["truncated"], true);
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_short_stderr_gives_budget_to_stdout() {
    // stderr = 10 lines (< 20 cap) + stdout = 250 lines (> remaining budget).
    // Expected: stderr_shown = 10, stdout_shown = 90 (BUFFER_QUERY_INLINE_CAP - 10).
    // Lines padded to ~60 bytes so total exceeds the token budget.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=250).map(|i| format!("out{i:>60}\n")).collect();
    let stderr: String = (1..=10).map(|i| format!("err{i:>60}\n")).collect();
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected Ok");
    assert_eq!(
        result["stdout_shown"],
        BUFFER_QUERY_INLINE_CAP - 10,
        "stdout_shown: {:?}",
        result
    );
    assert_eq!(
        result["stdout_total"], 250usize,
        "stdout_total: {:?}",
        result
    );
    assert_eq!(result["truncated"], true);
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffer_only_within_limit_no_truncation_fields() {
    // combined = 45 lines (< 50 threshold) — must NOT add truncated/shown/total fields.
    // needs_summary returns false, so we fall through to the short-output branch.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=30).map(|i| format!("out{i}\n")).collect();
    let stderr: String = (1..=15).map(|i| format!("err{i}\n")).collect();
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .expect("expected Ok");
    assert!(
        result.get("truncated").is_none(),
        "no truncated field: {:?}",
        result
    );
    assert!(
        result.get("stdout_shown").is_none(),
        "no stdout_shown: {:?}",
        result
    );
    assert!(
        result.get("output_id").is_none(),
        "no buffer ref: {:?}",
        result
    );
}

#[test]
fn system_prompt_draft_omits_hints_for_unsupported_languages() {
    let langs = vec!["markdown".to_string()];
    let draft = build_system_prompt_draft(&langs, &[], None, None, &[]);
    assert!(
        !draft.contains("## Language Navigation"),
        "should not have Language Navigation for markdown-only"
    );
}

#[test]
fn system_prompt_draft_includes_language_patterns_hint() {
    let langs = vec!["rust".to_string(), "python".to_string()];
    let entries = vec!["src/main.rs".to_string()];
    let draft = build_system_prompt_draft(&langs, &entries, None, None, &[]);
    assert!(
        draft.contains("language-patterns"),
        "draft should reference language-patterns memory"
    );
}

#[test]
fn system_prompt_draft_is_concise() {
    let draft = build_system_prompt_draft(&[], &[], None, None, &[]);
    // Private memory rules removed — duplicates server_instructions.md
    assert!(
        !draft.contains("Private Memory Rules"),
        "draft should NOT include Private Memory Rules (covered by server_instructions)"
    );
    assert!(
        !draft.contains("Semantic Memories"),
        "draft should NOT include Semantic Memories section (covered by server_instructions)"
    );
    // Core sections still present
    assert!(draft.contains("## Entry Points"));
    assert!(draft.contains("## Key Abstractions"));
    assert!(draft.contains("## Navigation Strategy"));
    assert!(draft.contains("## Project Rules"));
}

#[test]
fn system_prompt_draft_single_project_nav_strategy_unchanged() {
    // Single project: classic numbered list under ## Navigation Strategy
    let langs = vec!["rust".to_string()];
    let entries = vec!["src/main.rs".to_string()];
    let draft = build_system_prompt_draft(&langs, &entries, None, None, &[]);
    assert!(draft.contains("## Navigation Strategy\n"));
    assert!(
        draft.contains("symbols(\"src/main.rs\")"),
        "single-project nav should use first entry point"
    );
    assert!(
        !draft.contains("### "),
        "single-project draft should not have per-project subsections"
    );
}

#[test]
fn system_prompt_draft_multi_project_nav_strategy_has_subsections() {
    use crate::workspace::DiscoveredProject;
    let projects = vec![
        DiscoveredProject {
            id: "backend".to_string(),
            relative_root: std::path::PathBuf::from("backend"),
            languages: vec!["rust".to_string()],
            manifest: Some("Cargo.toml".to_string()),
        },
        DiscoveredProject {
            id: "frontend".to_string(),
            relative_root: std::path::PathBuf::from("frontend"),
            languages: vec!["typescript".to_string()],
            manifest: Some("package.json".to_string()),
        },
    ];
    let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
    assert!(
        draft.contains("### backend (rust)"),
        "should have backend subsection"
    );
    assert!(
        draft.contains("### frontend (typescript)"),
        "should have frontend subsection"
    );
    assert!(
        draft.contains("project_id=\"backend\""),
        "should have scoped semantic_search for backend"
    );
    assert!(
        draft.contains("project_id=\"frontend\""),
        "should have scoped semantic_search for frontend"
    );
    assert!(
        draft.contains("memory(project_id=\"backend\""),
        "should have per-project memory hint for backend"
    );
    assert!(
        draft.contains("symbols(\"backend\")"),
        "should use project root as placeholder entry point"
    );
}

#[test]
fn system_prompt_draft_multi_project_workspace_level_orient_step() {
    use crate::workspace::DiscoveredProject;
    let projects = vec![
        DiscoveredProject {
            id: "a".to_string(),
            relative_root: std::path::PathBuf::from("a"),
            languages: vec![],
            manifest: None,
        },
        DiscoveredProject {
            id: "b".to_string(),
            relative_root: std::path::PathBuf::from("b"),
            languages: vec![],
            manifest: None,
        },
    ];
    let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
    assert!(
        draft.contains("orient yourself to the workspace"),
        "workspace-level orient step should be present"
    );
}

#[test]
fn system_prompt_draft_multi_project_search_tips_has_scope_warning() {
    use crate::workspace::DiscoveredProject;
    let projects = vec![
        DiscoveredProject {
            id: "backend".to_string(),
            relative_root: std::path::PathBuf::from("backend"),
            languages: vec!["rust".to_string()],
            manifest: Some("Cargo.toml".to_string()),
        },
        DiscoveredProject {
            id: "frontend".to_string(),
            relative_root: std::path::PathBuf::from("frontend"),
            languages: vec!["typescript".to_string()],
            manifest: Some("package.json".to_string()),
        },
    ];
    let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
    assert!(
        draft.contains("Workspace mode"),
        "should warn about workspace scoping in Search Tips"
    );
    assert!(
        draft.contains("project_id=\"backend\""),
        "should include per-project example for backend"
    );
    assert!(
        draft.contains("project_id=\"frontend\""),
        "should include per-project example for frontend"
    );
}

#[test]
fn system_prompt_draft_single_project_search_tips_no_scope_warning() {
    let draft = build_system_prompt_draft(&[], &[], None, None, &[]);
    assert!(
        !draft.contains("Workspace mode"),
        "single-project draft should not have workspace scoping warning"
    );
}

#[test]
fn system_prompt_draft_multi_project_rust_search_tip_uses_type_hint() {
    use crate::workspace::DiscoveredProject;
    let projects = vec![
        DiscoveredProject {
            id: "core".to_string(),
            relative_root: std::path::PathBuf::from("core"),
            languages: vec!["rust".to_string()],
            manifest: None,
        },
        DiscoveredProject {
            id: "ui".to_string(),
            relative_root: std::path::PathBuf::from("ui"),
            languages: vec!["typescript".to_string()],
            manifest: None,
        },
    ];
    let draft = build_system_prompt_draft(&[], &[], None, Some(&projects), &[]);
    assert!(
        draft.contains("key type or trait name"),
        "rust project tip should mention type/trait"
    );
    assert!(
        draft.contains("handler or component name"),
        "typescript project tip should mention handler/component"
    );
}

#[test]
fn system_prompt_points_to_tool_guide_resource() {
    let prompt = build_system_prompt_draft(&[], &[], None, None, &[]);
    assert!(
        prompt.contains("doc://codescout-tool-guide"),
        "system prompt must point agents to the tool-guide resource"
    );
    assert_eq!(ONBOARDING_VERSION, 29);
}

#[test]
fn system_prompt_draft_read_markdown_hint_mentions_file_ref_reuse() {
    let draft = build_system_prompt_draft(
        &["rust".to_string()],
        &["src/main.rs".to_string()],
        None,
        None,
        &[],
    );
    assert!(
        draft.contains("@file_ref") || draft.contains("@file_"),
        "draft must teach @file_* reuse for read_markdown; got:\n{draft}"
    );
    assert!(
        draft.contains("IRON LAW #6"),
        "draft must cite IRON LAW #6 in the read_markdown guidance; got:\n{draft}"
    );
}

#[tokio::test]
async fn onboarding_discovers_sub_projects() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Root: Kotlin
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    std::fs::create_dir_all(root.join("src/main/kotlin")).unwrap();
    std::fs::write(root.join("src/main/kotlin/App.kt"), "fun main() {}").unwrap();

    // Sub: TypeScript
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(mcp.join("src")).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();
    std::fs::write(mcp.join("src/index.ts"), "").unwrap();

    // Sub: Python
    let py = root.join("python-services");
    std::fs::create_dir_all(&py).unwrap();
    std::fs::write(py.join("requirements.txt"), "flask\n").unwrap();
    std::fs::write(py.join("app.py"), "").unwrap();

    let agent = Agent::new(Some(root.to_path_buf())).await.unwrap();
    let ctx = ToolContext {
        agent,
        lsp: lsp(),
        output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
        progress: None,
        peer: None,
        section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
            crate::tools::section_coverage::SectionCoverage::new(),
        )),
        guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
        workspace_override: None,
    };

    let result = Onboarding
        .call(serde_json::json!({"force": true}), &ctx)
        .await
        .unwrap();

    let projects = result
        .get("projects")
        .expect("onboarding should return projects");
    let projects_arr = projects.as_array().unwrap();
    assert_eq!(
        projects_arr.len(),
        3,
        "should discover 3 projects (root + mcp-server + python-services), got {}",
        projects_arr.len()
    );

    // System prompt draft is now inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("mcp-server"),
        "subagent_prompt should mention mcp-server"
    );
}

#[test]
fn run_command_format_compact_test_result() {
    let tool = RunCommand;
    let result = json!({
        "type": "test", "exit_code": 0,
        "passed": 533, "failed": 0, "ignored": 0,
        "output_id": "@cmd_abc123"
    });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("533"), "got: {text}");
    assert!(text.contains("passed"), "got: {text}");
}

#[test]
fn run_command_format_compact_short_output() {
    let tool = RunCommand;
    let result = json!({ "stdout": "hello\nworld", "stderr": "", "exit_code": 0 });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("exit 0"), "got: {text}");
}

// Fix A: buffer-only queries should use BUFFER_QUERY_INLINE_CAP, not
// the summarization threshold. A 100-line result should be returned fully inline.
#[tokio::test]
async fn buffer_query_returns_up_to_200_lines_inline() {
    let (_dir, ctx) = project_ctx().await;
    // Directly store 100 lines in the buffer (bypasses needs_summary)
    let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
    let output_id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

    // Query the buffer — 100 lines is within the BUFFER_QUERY_INLINE_CAP
    #[cfg(windows)]
    let query = format!("type {output_id}");
    #[cfg(not(windows))]
    let query = format!("cat {output_id}");
    let result2 = RunCommand
        .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    let stdout = result2["stdout"].as_str().unwrap_or("");
    let line_count = stdout.lines().count();
    assert_eq!(
        line_count, 100,
        "buffer query of 100 lines should return all 100 inline (got {line_count})"
    );
    assert!(
        result2["truncated"].is_null(),
        "should not be truncated when within inline cap"
    );
}

// Fix B: the truncation hint for buffer queries should show the *next* page range,
// not always start from line 1.
#[tokio::test]
async fn buffer_query_truncation_hint_shows_next_page() {
    let (_dir, ctx) = project_ctx().await;
    // Directly store 300 lines (> BUFFER_QUERY_INLINE_CAP=100) in the buffer.
    // Lines padded to ~40 bytes so total exceeds token budget.
    let content: String = (1..=300).map(|i| format!("{i:>40}\n")).collect();
    let output_id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

    // Query it — output exceeds 100-line cap, so hint should show next-page command
    #[cfg(windows)]
    let query = format!("type {output_id}");
    #[cfg(not(windows))]
    let query = format!("cat {output_id}");
    let result2 = RunCommand
        .call(json!({ "command": query, "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    let hint = result2["hint"].as_str().unwrap_or("");
    // Hint must guide to the NEXT page (line 101 onwards), not back to line 1
    assert!(
        hint.contains("101"),
        "hint should show next-page start (101), got: {hint}"
    );
    assert!(
        !hint.contains("'1,"),
        "hint must not restart from line 1, got: {hint}"
    );
}

// Fix C: when the first run_command looks like a plain file read (cat file),
// the buffer creation hint should suggest read_file as an alternative.
#[tokio::test]
async fn cat_file_no_hint_field() {
    let (dir, ctx) = project_ctx().await;
    let md_path = dir.path().join("big_plan.md");
    let content: String = (1..=60).map(|i| format!("line {i}\n")).collect();
    std::fs::write(&md_path, content).unwrap();

    let result = RunCommand
        .call(
            json!({ "command": "cat big_plan.md", "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result["hint"].is_null(), "hint field should be absent");
}

#[tokio::test]
async fn ack_handle_executes_stored_command() {
    let (_dir, ctx) = project_ctx().await;
    let handle = ctx
        .output_buffer
        .store_dangerous("echo hello_ack".to_string(), None, 30);

    let tool = RunCommand;
    let input = serde_json::json!({ "command": handle });
    let result = tool
        .call(input, &ctx)
        .await
        .expect("ack call should succeed");

    let stdout = result["stdout"].as_str().unwrap_or("");
    assert!(
        stdout.contains("hello_ack"),
        "expected 'hello_ack' in stdout, got: {stdout}"
    );
}

#[tokio::test]
async fn ack_handle_unknown_returns_recoverable_error() {
    let (_dir, ctx) = project_ctx().await;
    let tool = RunCommand;
    let input = serde_json::json!({ "command": "@ack_deadbeef" });
    let err = tool
        .call(input, &ctx)
        .await
        .expect_err("unknown ack handle should return Err");
    assert!(
        err.to_string().contains("expired"),
        "error should mention 'expired', got: {err}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_prepends_refresh_indicator_for_stale_file_handle() {
    use std::fs;
    let (dir, ctx) = project_ctx().await;

    let path = dir.path().join("data.txt");
    fs::write(&path, "original").unwrap();
    let id = ctx
        .output_buffer
        .store_file(path.to_string_lossy().to_string(), "original".to_string());

    // Make the file look newer than the cached entry
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(2);
    filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future)).unwrap();

    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .unwrap();

    let stdout = result["stdout"].as_str().unwrap();
    assert!(
        stdout.starts_with(&format!("↻ {} refreshed from disk", id)),
        "expected refresh indicator, got: {:?}",
        stdout
    );
}

#[cfg(unix)]
#[tokio::test]
async fn run_command_buffered_output_has_output_id_before_stdout() {
    // Regression: output_id (the buffer reference the agent needs to query results)
    // was appended dynamically after the summary object was built, placing it AFTER
    // stdout/content fields. It must appear before content.
    let (_dir, ctx) = project_ctx().await;
    // seq 100 produces 100 lines, exceeding the token budget to trigger buffering.
    let result = RunCommand
        .call(json!({ "command": "seq 3000" }), &ctx)
        .await
        .unwrap();

    assert!(
        result["output_id"].is_string(),
        "expected buffered output (output_id present) for large command, got: {result:?}"
    );

    let keys: Vec<&str> = result
        .as_object()
        .unwrap()
        .keys()
        .map(|s| s.as_str())
        .collect();

    let output_id_pos = keys.iter().position(|k| *k == "output_id").unwrap();
    // stdout is the content field in generic summaries; failures/first_error in others.
    // We assert output_id appears before any content-heavy field.
    let stdout_pos = keys
        .iter()
        .position(|k| *k == "stdout")
        .unwrap_or(keys.len());

    assert!(
        output_id_pos < stdout_pos,
        "output_id must appear before stdout (content payload), got key order: {keys:?}"
    );
}

#[tokio::test]
async fn il3_blocks_cargo_pipe_grep_via_run_command() {
    // Integration check that the IL3 gate fires through the full run_command
    // path (not just the unit fn). cat|grep is now bounded-LHS and allowed —
    // use cargo as the canonical unbounded LHS sentinel.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(json!({ "command": "cargo test | grep FAILED" }), &ctx)
        .await
        .expect_err("IL3 should block live `cargo test | grep`");
    let msg = err.to_string();
    assert!(msg.contains("IL3 violation"), "missing IL3 marker: {msg}");
    assert!(msg.contains("buffer system"), "missing rewrite hint: {msg}");
}

#[tokio::test]
async fn non_filter_pipe_no_unfiltered_ref() {
    let (_dir, ctx) = project_ctx().await;
    // Second stage is not a known filter — no unfiltered_output
    let result = RunCommand
        .call(json!({ "command": "echo hello | cat" }), &ctx)
        .await
        .unwrap();
    assert!(
        result.get("unfiltered_output").is_none(),
        "unexpected unfiltered_output for non-filter pipe: {result}"
    );
}

#[tokio::test]
async fn il3_blocks_chained_unbounded_pipe() {
    // Chained pipes off an unbounded LHS still block (was originally
    // cat|grep|head; cat is now bounded — substitute cargo).
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo test | grep zzz | head -5" }),
            &ctx,
        )
        .await
        .expect_err("IL3 should block live `cargo test | grep | head`");
    assert!(err.to_string().contains("IL3 violation"));
}

#[tokio::test]
async fn il3_blocks_unbounded_pipe_pre_exec() {
    // IL3 fires before exec — verifies the gate does not depend on actual
    // output size. (Original test built a >MAX_INLINE_TOKENS file behind
    // `cat big.txt | grep`; cat is now bounded-LHS, so we use cargo as the
    // unconditionally-unbounded LHS.)
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(json!({ "command": "cargo test | grep line0" }), &ctx)
        .await
        .expect_err("IL3 should block regardless of payload size");
    assert!(err.to_string().contains("IL3 violation"));
}

#[test]
fn language_patterns_covers_all_supported_languages() {
    let supported = [
        "rust",
        "python",
        "typescript",
        "javascript",
        "go",
        "java",
        "kotlin",
    ];
    for lang in &supported {
        assert!(
            language_patterns(lang).is_some(),
            "language_patterns() should return Some for {lang}"
        );
    }
}

#[test]
fn language_patterns_returns_none_for_unsupported() {
    assert!(language_patterns("haskell").is_none());
    assert!(language_patterns("ruby").is_none());
    assert!(language_patterns("c").is_none());
}

#[test]
fn build_language_patterns_memory_assembles_detected_languages() {
    let langs = vec!["rust".to_string(), "python".to_string()];
    let result = build_language_patterns_memory(&langs);
    assert!(result.is_some());
    let content = result.unwrap();
    assert!(content.contains("### Rust"));
    assert!(content.contains("### Python"));
    assert!(!content.contains("### Go"));
    assert!(content.starts_with("# Language Patterns"));
}

#[test]
fn build_language_patterns_memory_returns_none_for_unsupported_only() {
    let langs = vec!["haskell".to_string(), "ruby".to_string()];
    let result = build_language_patterns_memory(&langs);
    assert!(result.is_none());
}

#[test]
fn build_language_patterns_memory_returns_none_for_empty() {
    let result = build_language_patterns_memory(&[]);
    assert!(result.is_none());
}

#[tokio::test]
async fn onboarding_includes_hardware_and_model_options() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // hardware and model_options are now inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("**Hardware:**"),
        "subagent_prompt must contain hardware data"
    );
    assert!(
        prompt.contains("cpu_cores"),
        "subagent_prompt must contain cpu_cores"
    );
    assert!(
        prompt.contains("**Model options:**"),
        "subagent_prompt must contain model options"
    );
    assert!(
        prompt.contains("recommended"),
        "subagent_prompt must contain recommended model info"
    );
}

#[tokio::test]
async fn onboarding_writes_recommended_model_to_config() {
    let (dir, ctx) = project_ctx().await;
    // Remove any pre-existing config so onboarding creates a fresh one
    let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    let toml = std::fs::read_to_string(dir.path().join(".codescout/project.toml")).unwrap();
    // model_options are now inside subagent_prompt; verify the config was written
    // with the recommended model by checking subagent_prompt contains the model
    // and the config contains a model setting
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("**Model options:**"),
        "subagent_prompt must contain model options"
    );
    assert!(
        toml.contains("model = "),
        "project.toml should contain a model setting\ntoml:\n{toml}"
    );
    // Should NOT contain the old hardcoded default
    assert!(
        !toml.contains("mxbai-embed-large"),
        "project.toml should not contain mxbai-embed-large\ntoml:\n{toml}"
    );
}

#[tokio::test]
async fn onboarding_includes_protected_memories_for_existing_topic() {
    let (dir, ctx) = project_ctx().await;

    // Pre-populate a protected memory with content
    let memories_dir = dir.path().join(".codescout").join("memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(
        memories_dir.join("gotchas.md"),
        "# Gotchas\n\n- **Problem:** foo\n  **Fix:** bar\n",
    )
    .unwrap();

    // Create config with protected = ["gotchas"]
    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

    // Force onboarding
    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // protected_memories is no longer top-level — it's inside subagent_prompt
    assert!(result.get("protected_memories").is_none());
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("**Protected memories:**"),
        "subagent_prompt must contain protected memories"
    );
    assert!(
        prompt.contains("gotchas"),
        "subagent_prompt must mention gotchas topic"
    );
    assert!(
        prompt.contains("# Gotchas"),
        "subagent_prompt must contain gotchas content"
    );
}

#[tokio::test]
async fn onboarding_protected_memory_missing_topic() {
    let (dir, ctx) = project_ctx().await;

    // Config protects "gotchas" but no gotchas.md exists
    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // protected_memories now inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("**Protected memories:**"));
    // The missing topic should show exists: false in the serialized JSON
    assert!(prompt.contains("\"exists\": false"));
}

#[tokio::test]
async fn onboarding_excludes_programmatic_from_protected() {
    let (dir, ctx) = project_ctx().await;

    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"onboarding\", \"language-patterns\", \"gotchas\"]\n",
        )
        .unwrap();

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // protected_memories now inside subagent_prompt as serialized JSON
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("**Protected memories:**"));
    // Programmatic topics excluded — should not appear as keys in the serialized JSON
    assert!(
        !prompt.contains("\"onboarding\":"),
        "onboarding should be excluded from protected memories"
    );
    assert!(
        !prompt.contains("\"language-patterns\":"),
        "language-patterns should be excluded from protected memories"
    );
    // Non-programmatic topic still present
    assert!(
        prompt.contains("\"gotchas\":"),
        "gotchas should be present in protected memories"
    );
}

#[tokio::test]
async fn onboarding_protected_memory_untracked_no_anchors() {
    let (dir, ctx) = project_ctx().await;

    let memories_dir = dir.path().join(".codescout").join("memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(
        memories_dir.join("gotchas.md"),
        "# Gotchas\n\n- Some gotcha referencing src/main.rs\n",
    )
    .unwrap();
    // No .anchors.toml file created

    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Staleness info is now serialized inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("\"untracked\": true"));
}

#[tokio::test]
async fn onboarding_protected_memory_stale_anchors() {
    let (dir, ctx) = project_ctx().await;

    // Write a source file and compute its hash
    let src_file = dir.path().join("main.rs");
    std::fs::write(&src_file, "fn main() {}").unwrap();
    let original_hash = crate::memory::hash::hash_file(&src_file).unwrap();

    // Create a protected memory referencing that file
    let memories_dir = dir.path().join(".codescout").join("memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(
        memories_dir.join("gotchas.md"),
        "# Gotchas\n\n- **Problem:** main.rs has issue\n  **Fix:** fix it\n",
    )
    .unwrap();

    // Create anchor sidecar with the original hash
    use crate::memory::anchors::{
        anchor_path_for_topic, write_anchor_file, AnchorFile, PathAnchor,
    };
    let anchor_file = AnchorFile {
        anchors: vec![PathAnchor {
            path: "main.rs".to_string(),
            hash: original_hash,
        }],
    };
    let anchor_path = anchor_path_for_topic(&memories_dir, "gotchas");
    write_anchor_file(&anchor_path, &anchor_file).unwrap();

    // Now modify the source file so the hash changes
    std::fs::write(&src_file, "fn main() { println!(\"changed\"); }").unwrap();

    // Config
    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Staleness info is now serialized inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("\"untracked\": false"));
    assert!(prompt.contains("\"status\": \"changed\""));
    assert!(prompt.contains("\"path\": \"main.rs\""));
}

#[tokio::test]
async fn onboarding_protected_memory_fresh_anchors() {
    let (dir, ctx) = project_ctx().await;

    // Write a source file and compute its hash
    let src_file = dir.path().join("main.rs");
    std::fs::write(&src_file, "fn main() {}").unwrap();
    let current_hash = crate::memory::hash::hash_file(&src_file).unwrap();

    // Create a protected memory referencing that file
    let memories_dir = dir.path().join(".codescout").join("memories");
    std::fs::create_dir_all(&memories_dir).unwrap();
    std::fs::write(
        memories_dir.join("gotchas.md"),
        "# Gotchas\n\n- **Problem:** main.rs has issue\n  **Fix:** fix it\n",
    )
    .unwrap();

    // Create anchor sidecar with the CURRENT hash (file hasn't changed)
    use crate::memory::anchors::{
        anchor_path_for_topic, write_anchor_file, AnchorFile, PathAnchor,
    };
    let anchor_file = AnchorFile {
        anchors: vec![PathAnchor {
            path: "main.rs".to_string(),
            hash: current_hash,
        }],
    };
    let anchor_path = anchor_path_for_topic(&memories_dir, "gotchas");
    write_anchor_file(&anchor_path, &anchor_file).unwrap();

    // Do NOT modify the source file — it stays the same

    // Config
    let config_path = dir.path().join(".codescout").join("project.toml");
    std::fs::write(
            &config_path,
            "[project]\nname = \"test\"\nlanguages = [\"rust\"]\n\n[memory]\nprotected = [\"gotchas\"]\n",
        )
        .unwrap();

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Staleness info is now serialized inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("\"untracked\": false"));
    // Fresh = no stale files, so stale_files should be empty array
    assert!(prompt.contains("\"stale_files\": []"));
}

#[tokio::test]
async fn onboarding_force_with_protected_memory_full_flow() {
    let (dir, ctx) = project_ctx().await;

    // First onboarding — creates everything fresh
    let _ = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Manually write a gotchas memory to simulate user curation
    let memories_dir = dir.path().join(".codescout").join("memories");
    std::fs::write(
        memories_dir.join("gotchas.md"),
        "# Gotchas\n\n- **Problem:** custom user gotcha\n  **Fix:** do the thing\n",
    )
    .unwrap();

    // Force re-onboarding
    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    // Should have standard fields plus subagent_prompt
    assert!(result.get("languages").is_some());
    assert!(result.get("subagent_prompt").is_some());
    // Old fields removed
    assert!(result.get("instructions").is_none());
    assert!(result.get("protected_memories").is_none());

    // Protected memories are now inside subagent_prompt
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(prompt.contains("custom user gotcha"));
    // No anchor sidecar was created, so staleness should be untracked
    assert!(prompt.contains("\"untracked\": true"));
}

#[tokio::test]
async fn onboarding_creates_workspace_toml_for_multi_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    // Root: Kotlin
    std::fs::write(root.join("build.gradle.kts"), "").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/App.kt"), "").unwrap();

    // Sub: TypeScript
    let mcp = root.join("mcp-server");
    std::fs::create_dir_all(&mcp).unwrap();
    std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

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

    Onboarding
        .call(serde_json::json!({"force": true}), &ctx)
        .await
        .unwrap();

    let ws_path = crate::config::workspace::workspace_config_path(root);
    assert!(
        ws_path.exists(),
        "workspace.toml should be created for multi-project repos"
    );

    let content = std::fs::read_to_string(&ws_path).unwrap();
    let config: crate::config::workspace::WorkspaceConfig = toml::from_str(&content).unwrap();
    assert_eq!(
        config.projects.len(),
        2,
        "should have 2 projects (root + mcp-server), got: {:?}",
        config.projects.iter().map(|p| &p.id).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn onboarding_skips_workspace_toml_for_single_project() {
    let dir = tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("src/main.rs"), "fn main() {}").unwrap();

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

    Onboarding
        .call(serde_json::json!({"force": true}), &ctx)
        .await
        .unwrap();

    let ws_path = crate::config::workspace::workspace_config_path(root);
    assert!(
        !ws_path.exists(),
        "workspace.toml should NOT be created for single-project repos"
    );
}

#[tokio::test]
async fn single_project_onboarding_unchanged() {
    let (_dir, ctx) = project_ctx().await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Single project: no workspace_mode field or it's false
    assert!(result.get("workspace_mode").is_none() || result["workspace_mode"] == false);
    // subagent_prompt should contain the standard Phase 1/Phase 2, not workspace phases
    let prompt = result["subagent_prompt"].as_str().unwrap_or("");
    assert!(prompt.contains("Phase 2: Explore the Code"));
    assert!(prompt.contains("Phase 3: Write the Memories"));
    assert!(!prompt.contains("Workspace Survey"));
    assert!(!prompt.contains("Workspace Survey"));
}

#[tokio::test]
async fn single_project_call_content_has_no_project_prompts() {
    let (_dir, ctx) = project_ctx().await;
    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");
    assert!(
        parsed.get("project_prompts").is_none(),
        "single-project must NOT have project_prompts"
    );
    assert!(
        parsed.get("synthesis_prompt_path").is_none(),
        "single-project must NOT have synthesis_prompt_path"
    );
}

#[tokio::test]
async fn onboarding_call_content_includes_workspace_info() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    let content = Onboarding.call_content(json!({}), &ctx).await.unwrap();
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block, got {}",
        content.len()
    );

    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("block must be valid JSON");

    // summary should mention workspace
    let summary = parsed["summary"].as_str().unwrap_or("");
    assert!(
        summary.contains("workspace") || summary.contains("project"),
        "summary should mention workspace mode, got: {summary}"
    );

    // prompt_path must point at the markdown file
    let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "must have prompt_path pointing to onboarding-prompt.md, got: {prompt_path:?}"
    );

    // Must NOT have output_id
    assert!(
        parsed.get("output_id").is_none(),
        "must NOT have output_id (old buffer pattern removed)"
    );

    // The file content itself should contain workspace instructions.
    let full_path = root.join(prompt_path);
    assert!(
        full_path.exists(),
        "onboarding-prompt.md must exist on disk"
    );
    let file_content = std::fs::read_to_string(&full_path).unwrap();
    assert!(
        file_content.contains("Workspace Survey"),
        "file content should include workspace instructions"
    );

    // Must have project_prompts array (workspace parallel dispatch)
    let project_prompts = parsed["project_prompts"]
        .as_array()
        .expect("workspace call_content must have project_prompts");
    assert!(
        project_prompts.len() >= 2,
        "workspace must have at least 2 project prompts, got {}",
        project_prompts.len()
    );

    // Must have synthesis_prompt_path
    assert!(
        parsed["synthesis_prompt_path"].as_str().is_some(),
        "workspace call_content must have synthesis_prompt_path"
    );
}

#[tokio::test]
async fn onboarding_call_content_workspace_writes_per_project_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();

    assert_eq!(content.len(), 1);
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("must be JSON");

    // Must have project_prompts array
    let project_prompts = parsed["project_prompts"]
        .as_array()
        .expect("workspace must have project_prompts");
    assert!(
        project_prompts.len() >= 2,
        "must have at least 2 project prompts"
    );

    // Each entry must have id and path
    for pp in project_prompts {
        let id = pp["id"].as_str().expect("must have id");
        let path = pp["path"].as_str().expect("must have path");
        assert!(
            path.contains("onboarding-project-"),
            "path must contain project prefix"
        );
        // File must exist
        assert!(
            root.join(path).exists(),
            "prompt file must exist for {}",
            id
        );
    }

    // Must have synthesis_prompt_path
    let synthesis_path = parsed["synthesis_prompt_path"]
        .as_str()
        .expect("must have synthesis_prompt_path");
    assert!(
        root.join(synthesis_path).exists(),
        "synthesis file must exist"
    );

    // Instructions must mention read_markdown
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_markdown"),
        "instructions must reference read_markdown"
    );
}

#[tokio::test]
async fn onboarding_includes_workspace_mode_and_per_project_protected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert_eq!(result["workspace_mode"], true);
    // per_project_protected_memories is now inside subagent_prompt
    assert!(result.get("per_project_protected_memories").is_none());
    let prompt = result["subagent_prompt"].as_str().unwrap();
    // Each discovered project should have an entry in the serialized protected memories
    assert!(
        prompt.contains("**Per-project protected memories:**"),
        "subagent_prompt must contain per-project protected memories"
    );
    assert!(prompt.contains("api"), "api project must be mentioned");
    assert!(prompt.contains("web"), "web project must be mentioned");
}

#[tokio::test]
async fn onboarding_writes_per_project_programmatic_memories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;
    Onboarding.call(json!({}), &ctx).await.unwrap();

    // Per-project memory directories should exist with onboarding + language-patterns
    let api_mem = root.join(".codescout/projects/api/memories");
    assert!(
        api_mem.join("onboarding.md").exists(),
        "api onboarding memory missing"
    );
    assert!(
        api_mem.join("language-patterns.md").exists(),
        "api language-patterns missing"
    );
    let web_mem = root.join(".codescout/projects/web/memories");
    assert!(
        web_mem.join("onboarding.md").exists(),
        "web onboarding memory missing"
    );
    assert!(
        web_mem.join("language-patterns.md").exists(),
        "web language-patterns missing"
    );
}

#[tokio::test]
async fn workspace_onboarding_full_flow() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);

    let ctx = project_ctx_at(root).await;

    // First onboarding
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    // Workspace mode active
    assert_eq!(result["workspace_mode"], true);
    assert!(result["projects"].as_array().unwrap().len() >= 2);

    // Per-project programmatic memories written
    assert!(root
        .join(".codescout/projects/api/memories/onboarding.md")
        .exists());
    assert!(root
        .join(".codescout/projects/web/memories/onboarding.md")
        .exists());

    // workspace.toml created
    assert!(crate::config::workspace::workspace_config_path(root).exists());

    // subagent_prompt contains workspace sections and system prompt draft
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("Workspace"),
        "subagent_prompt should contain workspace content"
    );
    assert!(
        prompt.contains("Workspace Survey"),
        "subagent_prompt should contain Phase 1A"
    );

    // System prompt draft is inside subagent_prompt
    assert!(prompt.contains("## System Prompt Draft"));
    assert!(prompt.contains("api"));
    assert!(prompt.contains("web"));
    assert!(prompt.contains("memory(project_id="));

    // call_content delivers 1 structured JSON block with prompt_path
    let content = Onboarding
        .call_content(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        content.len(),
        1,
        "call_content must return 1 structured block"
    );
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(text).expect("block must be valid JSON");

    // prompt_path must point to the markdown file
    let prompt_path = parsed["prompt_path"].as_str().unwrap_or("");
    assert!(
        prompt_path.contains("onboarding-prompt.md"),
        "must have prompt_path pointing to onboarding-prompt.md, got: {prompt_path:?}"
    );

    // Must NOT have output_id
    assert!(
        parsed.get("output_id").is_none(),
        "must NOT have output_id (old buffer pattern removed)"
    );

    // summary should contain workspace info
    let summary = parsed["summary"].as_str().unwrap_or("");
    assert!(
        summary.contains("workspace") || summary.contains("project"),
        "summary should mention workspace, got: {summary}"
    );

    // The file on disk has workspace content
    let full_path = root.join(prompt_path);
    assert!(
        full_path.exists(),
        "onboarding-prompt.md must exist on disk"
    );
    let file_content = std::fs::read_to_string(&full_path).unwrap();
    assert!(
        file_content.contains("Workspace Survey"),
        "file content must contain workspace content"
    );

    // Must have project_prompts (new parallel dispatch fields)
    let project_prompts = parsed["project_prompts"]
        .as_array()
        .expect("workspace full flow must have project_prompts");
    assert!(
        project_prompts.len() >= 2,
        "must have at least 2 project prompts"
    );
    for pp in project_prompts {
        assert!(
            pp["id"].as_str().is_some(),
            "each project prompt must have id"
        );
        assert!(
            pp["path"].as_str().is_some(),
            "each project prompt must have path"
        );
        let pp_path = pp["path"].as_str().unwrap();
        assert!(
            root.join(pp_path).exists(),
            "project prompt file must exist for {}",
            pp["id"]
        );
    }

    // Must have synthesis_prompt_path
    let synthesis_path = parsed["synthesis_prompt_path"]
        .as_str()
        .expect("must have synthesis_prompt_path");
    assert!(
        root.join(synthesis_path).exists(),
        "synthesis file must exist on disk"
    );

    // format_compact shows workspace info
    let compact = Onboarding.format_compact(&result).unwrap_or_default();
    assert!(compact.contains("workspace"));
}

#[test]
fn parse_timeout_input_correct_key_small() {
    let input = serde_json::json!({ "timeout_secs": 120 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 120);
    assert!(hint.is_none());
}

#[test]
fn parse_timeout_input_correct_key_boundary() {
    let input = serde_json::json!({ "timeout_secs": 86400 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 86400);
    assert!(hint.is_none());
}

#[test]
fn parse_timeout_input_correct_key_over_boundary() {
    let input = serde_json::json!({ "timeout_secs": 86401 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 86);
    let h = hint.unwrap();
    assert!(h.contains("86401"), "hint should contain raw value: {h}");
    assert!(
        h.contains("86s"),
        "hint should contain converted value: {h}"
    );
}

#[test]
fn parse_timeout_input_correct_key_large() {
    let input = serde_json::json!({ "timeout_secs": 120_000u64 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 120);
    assert!(hint.is_some());
}

#[test]
fn parse_timeout_input_correct_key_zero() {
    let input = serde_json::json!({ "timeout_secs": 0 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 30);
    assert!(hint.is_some());
}

#[test]
fn parse_timeout_input_wrong_key_small() {
    let input = serde_json::json!({ "timeout": 300 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 300);
    assert!(hint.is_some());
}

#[test]
fn parse_timeout_input_wrong_key_large() {
    let input = serde_json::json!({ "timeout": 120_000u64 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 120);
    assert!(hint.is_some());
}

#[test]
fn parse_timeout_input_wrong_key_zero() {
    let input = serde_json::json!({ "timeout": 0 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 30);
    assert!(hint.is_some());
}

#[test]
fn parse_timeout_input_neither_key() {
    let input = serde_json::json!({});
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 30);
    assert!(hint.is_none());
}

#[test]
fn parse_timeout_input_both_keys_valid() {
    // timeout_secs wins; timeout is silently ignored; no hint (timeout_secs value is valid)
    let input = serde_json::json!({ "timeout_secs": 60, "timeout": 5000 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 60);
    assert!(hint.is_none());
}

/// A dangerous command must return the pending_ack shape (two-round-trip pattern).
#[tokio::test]
async fn dangerous_command_returns_pending_ack() {
    let (_dir, ctx) = project_ctx().await;
    assert!(
        ctx.peer.is_none(),
        "test requires peer: None — dangerous commands bypass peer"
    );

    let result = RunCommand
        .call(
            json!({ "command": "rm -rf /tmp/test_elicitation_placeholder" }),
            &ctx,
        )
        .await
        .unwrap();

    assert!(
        result["pending_ack"].is_string(),
        "dangerous command without peer must return pending_ack handle, got: {result}"
    );
    assert!(
        result["reason"].is_string(),
        "response must include a reason, got: {result}"
    );
}

#[test]
fn parse_timeout_input_both_keys_secs_large() {
    // timeout_secs wins and triggers conversion hint; timeout is ignored
    let input = serde_json::json!({ "timeout_secs": 120_000u64, "timeout": 5000 });
    let (secs, hint) = parse_timeout_input(&input);
    assert_eq!(secs, 120);
    assert!(hint.is_some());
}

#[tokio::test]
async fn onboarding_triggers_refresh_when_version_stale() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: None, // pre-versioning → stale
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
        lsp: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert!(
        result.get("subagent_prompt").is_some(),
        "stale version must trigger refresh"
    );
    assert_eq!(result["version_stale"].as_bool(), Some(true));
    let prompt = result["subagent_prompt"].as_str().unwrap();
    assert!(
        prompt.contains("Do NOT re-explore"),
        "must be lightweight refresh"
    );
}
#[cfg(windows)]
#[tokio::test]
async fn background_command_with_quotes_captures_output() {
    // Regression: the background path used .args() → MSVC-CRT quote mangling →
    // a quoted -c argument dropped Python into its stdin-blocked REPL. Requires
    // `py` on PATH (present on this VDI).
    let (_dir, ctx) = project_ctx().await;
    let res = RunCommand
        .call(
            json!({
                "command": r#"py -c "print('bg-ok', 2+2)""#,
                "run_in_background": true
            }),
            &ctx,
        )
        .await
        .unwrap();
    let ref_id = res["output_id"].as_str().unwrap().to_string();
    // Poll the bg log buffer (same ctx → same OutputBuffer) until the line appears.
    let mut found = false;
    for _ in 0..50 {
        let out = RunCommand
            .call(
                json!({ "command": format!("type {ref_id}"), "timeout_secs": 10 }),
                &ctx,
            )
            .await;
        if let Ok(v) = out {
            if v["stdout"].as_str().unwrap_or("").contains("bg-ok 4") {
                found = true;
                break;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(found, "background command output not captured");
}

#[tokio::test]
async fn onboarding_fast_path_when_version_current() {
    let dir = tempdir().unwrap();
    let config_dir = dir.path().join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: Some(ONBOARDING_VERSION),
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
        lsp: Default::default(),
    };
    let toml_str = toml::to_string_pretty(&config).unwrap();
    std::fs::write(config_dir.join("project.toml"), &toml_str).unwrap();

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();

    let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
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

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();

    assert_eq!(result["onboarded"].as_bool(), Some(true));
    assert!(
        result.get("subagent_prompt").is_none(),
        "current version must not trigger refresh"
    );
}

#[test]
fn classify_slow_command_tags_pytest() {
    assert_eq!(
        classify_slow_command("uv run pytest -m permutation tests/eval"),
        Some("test suite")
    );
    assert_eq!(
        classify_slow_command("cargo test --release"),
        Some("test suite")
    );
}

#[test]
fn classify_slow_command_tags_builds() {
    assert_eq!(
        classify_slow_command("cargo build --release"),
        Some("build")
    );
    assert_eq!(classify_slow_command("./scripts/build.sh"), Some("build"));
    assert_eq!(
        classify_slow_command("docker build -t foo ."),
        Some("build")
    );
}

#[test]
fn classify_slow_command_tags_etl() {
    assert_eq!(
        classify_slow_command("uv run mrv ingest --reset"),
        Some("ETL/eval/training")
    );
    assert_eq!(
        classify_slow_command("python -m tests.eval._rescore"),
        Some("python script")
    );
}

#[test]
fn classify_slow_command_none_for_quick_commands() {
    assert_eq!(classify_slow_command("ls -la"), None);
    assert_eq!(classify_slow_command("git status"), None);
    assert_eq!(classify_slow_command("echo hello"), None);
}
