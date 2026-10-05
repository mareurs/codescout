use super::inner::classify_slow_command;
use super::*;
use crate::agent::Agent;
use crate::prompts::builders::{
    build_buffered_onboarding_instructions, build_buffered_refresh_instructions, build_heading_map,
    build_language_patterns_memory, build_per_project_prompt, build_prompt_refresh_subagent_prompt,
    build_subagent_epilogue, build_subagent_preamble, build_synthesis_prompt,
    build_system_prompt_draft, build_workspace_instructions, language_patterns,
};
#[cfg(unix)]
use crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP;
use crate::tools::core::types::client_name_can_spawn_subagents;
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
fn client_name_can_spawn_subagents_detects_claude_family_only() {
    assert!(client_name_can_spawn_subagents(Some("claude-code")));
    assert!(client_name_can_spawn_subagents(Some("Claude Code")));
    assert!(client_name_can_spawn_subagents(Some("claude-code-ide")));
    assert!(!client_name_can_spawn_subagents(Some("cursor")));
    assert!(!client_name_can_spawn_subagents(Some("copilot")));
    assert!(!client_name_can_spawn_subagents(Some("windsurf")));
    assert!(!client_name_can_spawn_subagents(None));
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
        instructions.contains("read_file"),
        "must tell how to read via read_file (Task 7: read_markdown was folded into it)"
    );
    // Must have numbered checklist
    assert!(
        instructions.contains("1. read_file"),
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
        instructions.contains("read_file"),
        "must tell how to read via read_file (Task 7: read_markdown was folded into it)"
    );
    // Must have numbered checklist
    assert!(
        instructions.contains("1. read_file"),
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
    // Task 7: read_markdown was folded into read_file (heading-addressed by default).
    assert!(!instructions.contains("read_markdown"));
    assert!(instructions.contains("read_file"));
}

#[test]
fn build_buffered_refresh_instructions_generic() {
    let instructions =
        build_buffered_refresh_instructions(".codescout/tmp/onboarding-prompt.md", None, 2, false);
    assert!(instructions.contains(".codescout/tmp/onboarding-prompt.md"));
    assert!(instructions.contains("pre-versioning"));
    assert!(!instructions.contains("subagent"));
    // Task 7: read_markdown was folded into read_file (heading-addressed by default).
    assert!(!instructions.contains("read_markdown"));
    assert!(instructions.contains("read_file"));
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
    // Task 7: read_markdown was folded into read_file (heading-addressed by default).
    assert!(instructions.contains("read_file"));
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
async fn onboarding_honors_workspace_override_pin() {
    // BUG (docs/issues/archive/2026-07-09-residual-workspace-pin-gaps-post-edit-code-fix.md,
    // finding 6): onboarding.rs was never wired for per-request pinning at all —
    // all 16 call sites used the plain require_project_root / with_project /
    // reload_config_if_project_toml. Onboarding WRITES (.codescout/project.toml,
    // memory files), so a pinned call silently onboarded the SESSION-DEFAULT
    // project instead of the one the caller named.
    let dir_a = tempdir().unwrap();
    let dir_b = tempdir().unwrap();
    // Workspace A is the pin target; give it source files to detect.
    std::fs::create_dir_all(dir_a.path().join(".codescout")).unwrap();
    std::fs::write(dir_a.path().join("main.rs"), "fn main() {}").unwrap();
    let canon_a = std::fs::canonicalize(dir_a.path()).unwrap();

    // Session default is B (project_ctx_at also seeds B with a main.rs).
    let mut ctx = project_ctx_at(dir_b.path()).await;
    let _ = std::fs::remove_file(dir_b.path().join(".codescout/project.toml"));
    let _ = std::fs::remove_file(dir_a.path().join(".codescout/project.toml"));

    // Pin THIS call to A.
    ctx.workspace_override = Some(canon_a.clone());

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result["config_created"], true);

    assert!(
        canon_a.join(".codescout/project.toml").exists(),
        "onboarding must write project.toml into the PINNED workspace A"
    );
    assert!(
        !dir_b.path().join(".codescout/project.toml").exists(),
        "onboarding must NOT write project.toml into the session-default workspace B"
    );
}

#[tokio::test]
async fn onboarding_returns_status_when_already_done() {
    let (dir, ctx) = project_ctx().await;
    let _ = std::fs::remove_file(dir.path().join(".codescout/project.toml"));

    // First call does full onboarding
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert!(result.get("languages").is_some()); // full onboarding result
                                                // …then the step it only instructs: a subagent writing the system prompt. The
                                                // version stamp is witnessed against that file now, so a project with memories and
                                                // no prompt correctly reports a refresh rather than "already onboarded".
    simulate_subagent_prompt_write(dir.path());

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
    let (dir, ctx) = project_ctx().await;

    // Run onboarding first
    Onboarding.call(json!({}), &ctx).await.unwrap();
    // …then the step onboarding only *instructs*: a subagent writing the prompt.
    // Without it the project has memories but no `.codescout/system-prompt.md`, and is
    // legitimately not yet fully onboarded.
    simulate_subagent_prompt_write(dir.path());

    // Status call returns guidance message and memories
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    let msg = result["message"].as_str().unwrap();
    assert!(msg.contains("already performed"));
    assert!(!result["memories"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn onboarding_status_includes_private_memories_when_present() {
    let (dir, ctx) = project_ctx().await;

    // Run full onboarding first (creates config + onboarding memory)
    Onboarding.call(json!({}), &ctx).await.unwrap();
    // …plus the deferred subagent write, which the version stamp is witnessed against.
    simulate_subagent_prompt_write(dir.path());

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
    let (dir, ctx) = project_ctx().await;

    // Run full onboarding first (creates config + onboarding memory), no private memory
    Onboarding.call(json!({}), &ctx).await.unwrap();
    // …plus the deferred subagent write, which the version stamp is witnessed against.
    simulate_subagent_prompt_write(dir.path());

    // Fast-path status call should NOT include private_memories field
    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert!(result["onboarded"].as_bool().unwrap_or(false));
    assert!(result["private_memories"].is_null());
    assert!(!result["message"].as_str().unwrap().contains("private"));
}

#[tokio::test]
async fn onboarding_call_content_delivers_message_when_already_done() {
    let (dir, ctx) = project_ctx().await;

    // First call does full onboarding (creates config + writes memory)
    Onboarding.call(json!({}), &ctx).await.unwrap();
    // …plus the deferred subagent write, which the version stamp is witnessed against.
    simulate_subagent_prompt_write(dir.path());

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

    // Must contain read_file instructions (Task 7: read_markdown was folded into read_file).
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_file"),
        "response must contain read_file instructions"
    );
    assert!(
        !instructions.contains("read_markdown"),
        "response must NOT contain read_markdown instructions — that tool no longer exists"
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

    // Task 7: read_markdown was folded into read_file (heading-addressed by default).
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(instructions.contains("read_file"));
}

#[tokio::test]
async fn onboarding_status_includes_per_project_memories_for_workspace() {
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();
    setup_workspace_dirs(root);
    let ctx = project_ctx_at(root).await;

    // Full workspace onboarding — writes per-project onboarding memories
    Onboarding.call(json!({}), &ctx).await.unwrap();
    // …plus the deferred subagent write, which the version stamp is witnessed against.
    simulate_subagent_prompt_write(root);

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
        instructions.contains("read_file") || instructions.contains("subagent"),
        "instructions must guide the agent, got: {instructions:?}"
    );
    // Task 7: read_markdown was folded into read_file (heading-addressed by default).
    assert!(
        !instructions.contains("read_markdown"),
        "instructions must NOT reference read_markdown — that tool no longer exists, got: {instructions:?}"
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

    // instructions must reference read_file (Task 7: read_markdown was folded into it).
    assert!(
        instructions.contains("read_file"),
        "instructions must reference read_file"
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

    // instructions must reference read_file (Task 7: read_markdown was folded into it).
    assert!(
        instructions.contains("read_file"),
        "instructions must reference read_file, got: {instructions:?}"
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

#[cfg(unix)]
use crate::tools::progress::test_support::CountingSink;
#[cfg(unix)]
use std::sync::atomic::Ordering;

#[cfg(unix)]
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
/// docs/issues/archive/2026-09-24-run-command-refuses-a-filtered-command-when-tmp-is-full.md
///
/// The tee capture is an optional side channel; failing to create its temp file used to
/// refuse the command itself. Three inputs, because each alone is monotone: "degrades on a
/// bad dir" is satisfied by a helper that ALWAYS degrades, and "no filter needs no dir" is
/// satisfied by one that never reaches the tee branch. Only together with the positive case
/// (a good dir DOES capture) do they pin the one branch that changed.
///
/// The bad directory does not exist, which forces the creation-failure branch — the same
/// `tempfile_in` error path a full `/tmp` takes — but not ENOSPC itself, which cannot be
/// produced portably in a unit test.
#[test]
fn a_tee_capture_that_cannot_be_created_degrades_instead_of_refusing() {
    let good = tempdir().unwrap();
    let bad = good.path().join("does-not-exist");

    let degraded = super::inner::inject_tee_in("echo hi | head -1", false, &bad)
        .expect("an unusable capture dir must degrade, not refuse the command");
    assert_eq!(
        degraded.command, "echo hi | head -1",
        "the command must run un-teed and otherwise untouched"
    );
    assert!(degraded.capture.is_none(), "nothing is being captured");
    let note = degraded
        .skipped
        .expect("a capture that was wanted and could not be made must say so");
    assert!(
        note.contains("does-not-exist") && note.contains("unfiltered_output"),
        "the note names the directory and the missing buffer; got: {note}"
    );

    // Control: no terminal filter means no capture is wanted, so nothing is skipped and the
    // unusable directory is never consulted.
    let plain = super::inner::inject_tee_in("echo hi", false, &bad).unwrap();
    assert_eq!(plain.command, "echo hi");
    assert!(plain.skipped.is_none(), "nothing wanted, nothing skipped");

    // Positive: a usable directory still captures — the degrade path is not the only path.
    let captured = super::inner::inject_tee_in("echo hi | head -1", false, good.path()).unwrap();
    assert!(
        captured.command.contains("| tee '"),
        "a usable dir must splice the tee; got: {}",
        captured.command
    );
    assert!(captured.capture.is_some());
    assert!(captured.skipped.is_none());
}

/// The same defect through the tool, so the WIRING is covered and not only the helper: the
/// command runs, its output arrives, and the response says the unfiltered buffer is missing.
///
/// The directory is injected through a per-thread seam, not `TMPDIR`
/// (`docs/conventions/test-env-isolation.md`). `#[tokio::test]` is single-threaded, so the
/// override set here is the one `inject_tee` reads.
#[tokio::test]
async fn a_full_tmp_does_not_refuse_a_filtered_command_and_the_response_says_so() {
    struct ResetTeeDir;
    impl Drop for ResetTeeDir {
        fn drop(&mut self) {
            super::inner::TEE_DIR_OVERRIDE.with(|d| *d.borrow_mut() = None);
        }
    }
    let _reset = ResetTeeDir;
    let (dir, ctx) = project_ctx().await;

    super::inner::TEE_DIR_OVERRIDE
        .with(|d| *d.borrow_mut() = Some(dir.path().join("does-not-exist")));
    let degraded = RunCommand
        .call(
            json!({ "command": "echo hi | head -1", "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .expect("an uncreatable capture file must not refuse the command");
    assert!(
        degraded["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("hi"),
        "the command must have run; got: {degraded}"
    );
    assert!(
        degraded["unfiltered_output_skipped"].is_string(),
        "the response must say the unfiltered buffer is missing; got: {degraded}"
    );
    assert!(
        degraded.get("unfiltered_output").is_none(),
        "no capture existed, so no buffer ref may be offered; got: {degraded}"
    );

    // Control on the same call with a usable directory: no skip note, and the buffer is there.
    super::inner::TEE_DIR_OVERRIDE.with(|d| *d.borrow_mut() = Some(dir.path().to_path_buf()));
    let healthy = RunCommand
        .call(
            json!({ "command": "echo hi | head -1", "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        healthy.get("unfiltered_output_skipped").is_none(),
        "a healthy capture must not carry a skip note; got: {healthy}"
    );
    assert!(
        healthy.get("unfiltered_output").is_some(),
        "a healthy capture must still offer its buffer; got: {healthy}"
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

/// Wiring, not logic — the logic is pinned in `path_security`'s
/// `commit_backtick_gate_*` family. This asserts the gate is reachable through
/// `RunCommand::call`, the boundary the bug it closes was itself burned by: a value
/// computed correctly and never rendered reaches nobody. See
/// `docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`.
#[tokio::test]
async fn run_command_refuses_a_commit_message_the_shell_would_substitute() {
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({"command": r#"git commit -m "per memory `conventions` here""#}),
            &ctx,
        )
        .await
        .expect_err("a commit message with an evaluated backtick must be refused");
    let msg = err.to_string();
    assert!(
        msg.contains("conventions"),
        "the refusal must name the text the shell would run: {msg}"
    );
    assert!(
        msg.contains("commit -F") || msg.contains("heredoc"),
        "the refusal must point at the safe convention, not just say no: {msg}"
    );
}

/// Paired control. The escape hatch stays open — `acknowledge_risk` is how a caller
/// says the substitution is intended — and the refusing half also shows the gate reads
/// the shape rather than requiring `git` to lead the command.
#[tokio::test]
async fn run_command_commit_backtick_gate_honours_acknowledge_risk() {
    let (_dir, ctx) = project_ctx().await;
    // Inert on purpose: `echo` prefixes it, so nothing commits on either call.
    let cmd = "echo git commit -m \"cites `true` here\"";

    RunCommand
        .call(json!({"command": cmd}), &ctx)
        .await
        .expect_err("the gate fires on the shape alone");

    let ok = RunCommand
        .call(json!({"command": cmd, "acknowledge_risk": true}), &ctx)
        .await;
    assert!(ok.is_ok(), "acknowledge_risk must bypass the gate: {ok:?}");
}

/// End-to-end regression for the heredoc pipe-rewrite corruption: write a file through a
/// heredoc whose body contains pipes, then read the bytes back.
///
/// The unit tests pin the masking; this pins that nothing downstream re-introduces the
/// rewrite, which matters because the damage is invisible at the call site — exit 0, file
/// written, corruption only in content the author does not re-read.
/// `docs/issues/archive/2026-08-19-run-command-rewrites-pipes-inside-heredoc-content.md`.
#[tokio::test]
async fn heredoc_body_pipes_are_not_rewritten_into_the_written_file() {
    let (_dir, ctx) = project_ctx().await;

    // Every `|` here is inside the body, destined for the file. Pre-fix,
    // `detect_terminal_filter` found the last one and spliced
    // `| tee '/tmp/codescout-unfiltered-…' |` into the text that got written.
    let write = "cat > note.txt <<'EOF'\n- Resolve: git log --all -p | git patch-id --stable | grep abc123\nEOF";
    let wrote = RunCommand
        .call(json!({"command": write}), &ctx)
        .await
        .expect("writing the heredoc should succeed");

    // A non-zero exit comes back as Ok, so the `expect` above proves the tool RAN, not
    // that the file landed. (A blocked command does surface as Err — measured, not
    // assumed — but a shell that fails on its own does not.) Without this check a failed
    // write leaves no file, `cat` prints nothing, and the *first* assertion below passes
    // vacuously on the empty string: a green that reads identically in a broken world.
    // A timeout reports `exit_code: null`, which also fails this comparison rather than
    // slipping through as zero.
    assert_eq!(
        wrote["exit_code"], 0,
        "the heredoc write must land before the read means anything: {wrote}"
    );

    let read = RunCommand
        .call(json!({"command": "cat note.txt"}), &ctx)
        .await
        .expect("reading it back should succeed");
    let content = read["stdout"].as_str().unwrap_or_default();

    assert!(
        !content.contains("codescout-unfiltered"),
        "tee instrumentation leaked into written content: {content}"
    );
    assert!(
        content.contains("git log --all -p | git patch-id --stable | grep abc123"),
        "the heredoc body must land byte-for-byte.\n  write: {wrote}\n  read: {read}"
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
    // The response returns at SPAWN time, so it carries no output yet and — the
    // load-bearing half — asserts no exit status. `format_run_command` reads that
    // absence as "running", which is true by construction only because we did not
    // wait. Assert the absence: a response that grew an `exit_code` here would be
    // claiming an outcome nothing had observed, which is the defect this slice closed.
    assert!(
        result["exit_code"].is_null(),
        "a just-spawned job must assert no exit status, got: {:?}",
        result["exit_code"]
    );
    let hint = result["hint"].as_str().unwrap_or("");
    assert!(
        hint.contains(output_id),
        "hint should reference the handle, got: {hint}"
    );
}

/// The defect this slice exists to close, end to end.
///
/// A backgrounded failure used to be unobservable: the response asserted
/// `Process running` without checking, and a later `tail @bg_x` returned the
/// READER's exit code, never the job's. This asserts the outcome now ARRIVES —
/// through the envelope, because the `@bg_` substitution channel expands to a
/// filename and cannot carry a status.
///
/// docs/issues/archive/2026-09-13-background-command-loses-terminal-status.md
#[tokio::test]
async fn a_failed_background_job_reports_its_exit_code_through_the_envelope() {
    let (_dir, ctx) = project_ctx().await;
    let res = RunCommand
        .call(
            json!({ "command": "exit 7", "run_in_background": true }),
            &ctx,
        )
        .await
        .unwrap();
    let ref_id = res["output_id"].as_str().unwrap().to_string();

    // Poll rather than sleep a fixed interval: the claim is that the outcome
    // arrives, not how fast, and a fixed wait would be flaky on a loaded machine.
    // Keep the last state so a failure names what it actually saw.
    let mut last = String::from("<never populated>");
    for _ in 0..200 {
        let out = RunCommand
            .call(json!({ "command": format!("cat {ref_id}") }), &ctx)
            .await
            .unwrap();
        if let Some(jobs) = out["jobs"].as_array() {
            if let Some(state) = jobs.first().and_then(|j| j["state"].as_str()) {
                last = state.to_string();
                if last == "exited 7" {
                    return;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("background job never reported its terminal state; last seen: {last:?}");
}

/// The reader's exit code and the job's outcome are DIFFERENT NUMBERS, and the
/// response must carry both without conflating them. `cat` succeeds (0) while
/// the job it reads failed (7) — the exact pair that made the original defect
/// invisible.
#[tokio::test]
async fn the_readers_exit_code_and_the_jobs_outcome_are_reported_separately() {
    let (_dir, ctx) = project_ctx().await;
    let res = RunCommand
        .call(
            json!({ "command": "exit 7", "run_in_background": true }),
            &ctx,
        )
        .await
        .unwrap();
    let ref_id = res["output_id"].as_str().unwrap().to_string();

    for _ in 0..200 {
        let out = RunCommand
            .call(json!({ "command": format!("cat {ref_id}") }), &ctx)
            .await
            .unwrap();
        let job_state = out["jobs"]
            .as_array()
            .and_then(|j| j.first())
            .and_then(|j| j["state"].as_str())
            .unwrap_or("");
        if job_state == "exited 7" {
            assert_eq!(
                out["exit_code"].as_i64(),
                Some(0),
                "the READER (cat) succeeded; its exit code must stay its own"
            );
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("job never reached its terminal state");
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

#[tokio::test]
async fn shell_command_mode_disabled_blocks_run_command() {
    // shell_command_mode = "disabled" is the sole mechanism for turning shell
    // off — the former shell_enabled master switch was removed as redundant.
    //
    // This refusal is NOT made redundant by `RunCommand::availability()` hiding
    // the tool from `list_tools` under the same setting. `current_capabilities()`
    // reads the SESSION-DEFAULT project, while `call` reads
    // `security_config_for(ctx.workspace_override)` — the PINNED one. A
    // `workspace`-pinned call into a shell-disabled project therefore never
    // passes the availability filter at all, and this check is the only thing
    // standing in front of it. (An MCP client may also call a tool it was never
    // advertised.) Do not delete it on the grounds that the tool is hidden now.
    let (dir, ctx) = project_ctx().await;
    let root = dir.path().to_path_buf();
    let security = crate::util::path_security::PathSecurityConfig {
        shell_command_mode: "disabled".into(),
        ..Default::default()
    };
    let result = run_command_inner(
        "echo x", "echo x", 30, false, // acknowledge_risk
        None,  // cwd_param
        false, // buffer_only
        false, // run_in_background
        &root, &security, &ctx,
    )
    .await;
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("disabled"),
        "error should mention shell is disabled, got: {err}"
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

    // The output still has to REACH the log — that is what routing stdout to a
    // file rather than a pipe buys, and it is the half of this test that is
    // about the defect rather than about latency.
    //
    // Read it through the handle rather than from the spawn response: the 5s
    // warm window this used to assert on was removed on 2026-09-15 so the call
    // could return immediately, and asserting on a scraped preview would now be
    // asserting on a race. Polling tests the same property without one.
    let mut last = String::new();
    for _ in 0..100 {
        let out = RunCommand
            .call(json!({ "command": format!("cat {output_id}") }), &ctx)
            .await
            .unwrap();
        last = out["stdout"].as_str().unwrap_or("").to_string();
        if last.contains("launched") {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    panic!("echo output never reached the background log; last read: {last:?}");
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
    ignore = "test uses /var as 'outside project but exists'; no Windows analog (allowed-roots vary). See docs/issues/archive/2026-05-24-ci-windows-test-portability-rot.md"
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
/// REACH test for the byte bound on `summarize_generic`, through the surface a caller sees.
///
/// BUG docs/issues/archive/2026-10-05-run-command-json-stdout-overflow-has-no-working-json-path-recovery.md
///
/// One 95 KB line is "1 line", so the line-only summary returned it verbatim, the response
/// overflowed AGAIN in `call_content`, and the caller got a `@tool_*` envelope whose summary
/// carried no content and whose hint, `json_path="$.field"`, no `read_file` route can honour.
/// This asserts the response is the small `@cmd_*` envelope instead, still carries the
/// document's two ends, and that the handle it names really holds the whole document — the
/// recovery route (`grep`/`jq`/`sed` on `@cmd_*`) is only real if the data is behind it.
///
/// The unit tests in `command_summary.rs` pin the bound; only this one pins that
/// `handle_successful_output` routes the generic arm through it.
#[cfg(unix)]
#[tokio::test]
async fn a_huge_stdout_line_is_summarized_inline_not_rebuffered() {
    let (_dir, ctx) = project_ctx().await;
    let content = RunCommand
        .call_content(
            json!({
                "command": "yes 'abcdefghij' | tr -d '\\n' | head -c 95000",
                "timeout_secs": 10,
            }),
            &ctx,
        )
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let response: Value = serde_json::from_str(text).unwrap_or_else(|e| {
        panic!(
            "response is not JSON ({e}); starts {:?}",
            &text[..text.len().min(200)]
        )
    });

    let output_id = response["output_id"].as_str().expect("an output_id");
    assert!(
        output_id.starts_with("@cmd_"),
        "a second, `@tool_*` buffering means the summary did not bound the line: {output_id}"
    );
    assert!(
        response.get("buffered_bytes").is_none(),
        "`buffered_bytes` is the `@tool_*` re-buffering envelope's field: {response}"
    );
    let stdout = response["stdout"].as_str().expect("an inline stdout");
    assert!(
        stdout.contains("bytes shown"),
        "no elision marker: {:?}",
        &stdout[..stdout.len().min(120)]
    );
    assert!(
        stdout.starts_with("abcdefghij"),
        "the head of the output is missing"
    );

    let held = ctx
        .output_buffer
        .get_stream(output_id)
        .expect("the handle resolves");
    assert_eq!(
        held.len(),
        95_000,
        "the handle must hold the WHOLE stream the summary elided"
    );
}
/// REACH test for the byte bound on the `test` envelope's `failures` field.
///
/// BUG docs/issues/archive/2026-10-05-run-command-test-envelope-failures-field-has-no-byte-bound.md
///
/// The command ends in `echo cargo test` only so `detect_command_type` classifies the run as
/// a test run; the `failures:` block is printed by `printf`. One 60 KB line used to push the
/// envelope past the inline budget, so the response became a `@tool_*` envelope with a
/// one-line summary and no failure text. Asserting `type` first keeps a misclassification from
/// passing this vacuously: a `generic` envelope has no `failures` key to bound.
#[cfg(unix)]
#[tokio::test]
async fn a_huge_failure_line_is_summarized_inline_not_rebuffered() {
    let (_dir, ctx) = project_ctx().await;
    let content = RunCommand
        .call_content(
            json!({
                "command": "printf 'failures:\\n%s\\nfailures:\\n' \"$(head -c 60000 /dev/zero | tr '\\0' x)\"; echo cargo test",
                "timeout_secs": 10,
            }),
            &ctx,
        )
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let response: Value = serde_json::from_str(text).unwrap_or_else(|e| {
        panic!(
            "response is not JSON ({e}); starts {:?}",
            text.chars().take(200).collect::<String>()
        )
    });

    let output_id = response["output_id"].as_str().expect("an output_id");
    assert!(
        output_id.starts_with("@cmd_"),
        "a `@tool_*` handle means `failures` re-buffered the response: {output_id}"
    );
    assert!(
        response.get("buffered_bytes").is_none(),
        "re-buffering envelope: {response}"
    );
    assert_eq!(
        response["type"], "test",
        "the run must classify as a test run: {response}"
    );
    let failures = response["failures"].as_str().expect("an inline `failures`");
    assert!(
        failures.contains("bytes shown"),
        "no elision marker: {:?}",
        failures.chars().take(120).collect::<String>()
    );
    assert!(
        failures.starts_with("failures:"),
        "the head of the section is missing"
    );
}

/// The `build` envelope's `first_error`, through the same surface. Same `echo` classification
/// trick; the error line carries a rustc-style code so `rust_error_code_re` selects it.
#[cfg(unix)]
#[tokio::test]
async fn a_huge_error_block_is_summarized_inline_not_rebuffered() {
    let (_dir, ctx) = project_ctx().await;
    let content = RunCommand
        .call_content(
            json!({
                "command": "printf 'error[E0308]: mismatched types\\n%s\\n' \"$(head -c 60000 /dev/zero | tr '\\0' x)\"; echo cargo build",
                "timeout_secs": 10,
            }),
            &ctx,
        )
        .await
        .unwrap();
    let text = content[0].as_text().map(|t| t.text.as_str()).unwrap_or("");
    let response: Value = serde_json::from_str(text).unwrap_or_else(|e| {
        panic!(
            "response is not JSON ({e}); starts {:?}",
            text.chars().take(200).collect::<String>()
        )
    });

    assert!(
        response["output_id"]
            .as_str()
            .expect("an output_id")
            .starts_with("@cmd_"),
        "a `@tool_*` handle means `first_error` re-buffered the response: {response}"
    );
    assert!(
        response.get("buffered_bytes").is_none(),
        "re-buffering envelope: {response}"
    );
    assert_eq!(
        response["type"], "build",
        "the run must classify as a build: {response}"
    );
    let first_error = response["first_error"]
        .as_str()
        .expect("an inline `first_error`");
    assert!(first_error.contains("bytes shown"), "no elision marker");
    assert!(
        first_error.starts_with("error[E0308]"),
        "the error line itself is missing"
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
    // ADDED 2026-09-14. The three assertions above are all ABSENCE assertions, and each
    // stays true whether or not the entry's stderr is surfaced — so this test built the
    // failing case (15 stored stderr lines), named the gate in its own comment, and
    // still could not see the stream vanish. CLAUDE.md § Testing Discipline, law one:
    // monotone under removal. The positive assertion is the half that discriminates.
    assert!(
        result["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("err15"),
        "the entry's stored stderr must reach the reader on the short-output path \
         too; got: {result:?}"
    );
}

/// THE BUG'S OWN REPRODUCTION, pinned. A `grep -c` that finds nothing returns two
/// bytes, so `needs_summary` is false — and until 2026-09-14 that was exactly the
/// query that received no stderr at all, while one returning >10 KB got it in full.
/// The gate was anti-correlated with need: this `0` is indistinguishable from a
/// stream that was never surfaced, which is why the SMALL query is the one that has
/// to carry it.
///
/// LOAD-BEARING: the pattern must not occur in the stored stdout. One that matched
/// would return a count > 0 and this would still pass while testing nothing about
/// the zero case.
/// BUG docs/issues/archive/2026-09-14-every-reader-of-a-cmd-buffer-takes-stdout-only-so-the-stored-stderr-reaches-nobody.md
#[cfg(unix)]
#[tokio::test]
async fn buffer_query_below_summary_threshold_still_surfaces_stored_stderr() {
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=30).map(|i| format!("out{i}\n")).collect();
    let id = ctx.output_buffer.store(
        "cmd".into(),
        stdout,
        "WRAPPER_VERDICT: the stream a reader came for\n".into(),
        0,
    );
    let result = RunCommand
        .call(
            json!({ "command": format!("grep -c NOSUCHTOKEN {id}") }),
            &ctx,
        )
        .await
        .expect("expected Ok");
    // Control first: the query really did return the empty-looking count, so the
    // assertion below is about a genuinely SMALL response rather than one that
    // accidentally crossed the summary threshold and took the other branch.
    assert!(
        result["stdout"]
            .as_str()
            .unwrap_or_default()
            .starts_with('0'),
        "expected a zero count from a pattern absent from the buffer; got: {result:?}"
    );
    assert!(
        result["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("WRAPPER_VERDICT"),
        "a buffer query below the summary threshold must still carry the entry's \
         stored stderr; got: {result:?}"
    );
}
// ---- a buffer query bounds the STORED stderr by bytes, not only by lines ----
//
// BUG-adjacent, found by the 2026-10-05 sibling sweep: all three buffer-only arms of
// `handle_successful_output` carried the stored stderr through `truncate_lines(.., 20)`. A line has
// no length, so one 50 KB stderr line passed whole, the response crossed the inline limit, and
// `call_content` buffered it under `@tool_*`, hiding the query's own answer (`0`). The comment
// above the cap claimed it prevented exactly that. Three arms, three tests: each arm has its
// own call, and a mutation of one is not caught by a test of another.

/// True when `text` contains an actual `@tool_<8 hex>` handle. A bare `@tool_` substring is NOT
/// the test: hint prose may legitimately mention the kind (`@tool_*`), and one hint literal
/// did for a `@cmd_*` query, which is a different defect than minting a second handle.
fn has_tool_handle(text: &str) -> bool {
    regex::Regex::new(r"@tool_[0-9a-f]{8}")
        .expect("static pattern")
        .is_match(text)
}

/// A stored `@cmd_*` entry whose stderr is ONE wide line with distinguishable ends.
fn wide_stderr() -> String {
    format!("HEAD{}TAIL\n", "e".repeat(50_000))
}

/// Run `command` through `RunCommand::call_content`; return the primary block's text and parse.
async fn buffer_query(ctx: &ToolContext, command: String) -> (String, Value) {
    let content = RunCommand
        .call_content(json!({ "command": command, "timeout_secs": 10 }), ctx)
        .await
        .unwrap();
    let text = content[0]
        .as_text()
        .map(|t| t.text.clone())
        .unwrap_or_default();
    let mut parsed: Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("response is not JSON ({e}): {text:.200}"));
    // The COMPACT form, without the first-call `_guide_hint`: that is what `call_content` measures
    // against the inline limit, and it is shorter than the pretty-printed text the transport
    // carries. Every size assertion in the tests below is therefore in the unit the limit uses.
    if let Some(obj) = parsed.as_object_mut() {
        obj.remove("_guide_hint");
    }
    let text = parsed.to_string();
    (text, parsed)
}
/// Like [`buffer_query`] but takes a plain `&str` command, for runs that are not buffer queries.
async fn buffer_query_free(ctx: &ToolContext, command: &str) -> (String, Value) {
    buffer_query(ctx, command.to_string()).await
}
/// The `sed -n 'A,Bp' @handle` page a truncation hint advises, parsed OUT of the hint so a test
/// follows the route the response actually gave and not one it hard-coded.
fn next_page_command(hint: &str) -> String {
    regex::Regex::new(r"sed -n '\d+,\d+p' @[A-Za-z0-9_]+")
        .expect("static pattern")
        .find(hint)
        .unwrap_or_else(|| panic!("no next-page command in the hint: {hint}"))
        .as_str()
        .to_string()
}

/// Store `stdout` / `stderr` and run `command_for(id)` as a buffer query.
async fn query_stored(
    ctx: &ToolContext,
    stdout: String,
    stderr: String,
    command_for: impl Fn(&str) -> String,
) -> (String, String, Value) {
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let (text, parsed) = buffer_query(ctx, command_for(&id)).await;
    (id, text, parsed)
}

fn assert_stderr_bounded(id: &str, text: &str, parsed: &Value) {
    assert!(
        !has_tool_handle(text),
        "a wide stored stderr line re-buffered the response under a second handle: {text:.300}"
    );
    let stderr = parsed["stderr"]
        .as_str()
        .unwrap_or_else(|| panic!("no inline stderr: {text:.300}"));
    assert!(
        stderr.starts_with("HEAD"),
        "the head of the stderr line survives"
    );
    assert!(
        stderr.ends_with("TAIL\n") || stderr.ends_with("TAIL"),
        "the tail survives"
    );
    assert!(
        stderr.contains("bytes shown"),
        "a cut must say so: {:.200}",
        stderr
    );
    assert!(
        stderr.contains(&format!("of {} bytes shown", wide_stderr().len())),
        "the total is the stored stream as it was, not the cut text"
    );
    assert!(
        stderr.contains(&format!("{id}.err")),
        "the marker names the `.err` handle that holds all of it"
    );
    assert!(
        !crate::tools::exceeds_inline_limit(text),
        "the response is {} bytes and is still over the inline limit",
        text.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_short_buffer_query_bounds_a_wide_stored_stderr_by_bytes() {
    // The `grep -c` zero-count arm: tiny stdout, so only the stderr can overflow.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=30).map(|i| format!("out{i}\n")).collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), stdout, wide_stderr(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep -c NOSUCHTOKEN {id}")).await;

    assert_stderr_bounded(&id, &text, &parsed);
    assert!(
        parsed["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("bytes shown"),
        "the cut must announce itself: {text:.200}"
    );
    assert!(parsed["stdout"]
        .as_str()
        .unwrap_or_default()
        .starts_with('0'));
}

#[cfg(unix)]
#[tokio::test]
async fn a_summarized_buffer_query_bounds_a_wide_stored_stderr_by_bytes() {
    // The `needs_summary` arm: the query's own stdout is over 10 KB.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=4000).map(|i| format!("out{i}\n")).collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), stdout, wide_stderr(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep out {id}")).await;

    assert_stderr_bounded(&id, &text, &parsed);
    assert!(parsed["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("out1"));
}
// ---- a buffer query never returns zero bytes of a non-empty result ----
//
// BUG-adjacent, found by the 2026-10-05 sibling sweep and verified live: `grep status @cmd_X` and
// `sed -n '1,100p' @cmd_X` on a 78 KB SINGLE-LINE buffer returned
// {"truncated":true,"stdout_shown":0,"stdout_total":1} with the hint "Next page: sed -n '1,100p'
// @ref" — and that advised route returned the identical response, a loop. `truncate_lines_and_bytes`
// emitted nothing when the first line alone exceeded the byte budget. `jq` and `grep -o` worked.

/// One wide line with distinguishable ends, as `jq -c` or `curl` prints a JSON document.
fn wide_line(width: usize) -> String {
    format!("HEAD{}TAIL", "status:created,".repeat(width / 15))
}

#[cfg(unix)]
#[tokio::test]
async fn a_wide_first_line_is_shown_clipped_never_dropped() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("cmd".into(), wide_line(78_000), String::new(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep status {id}")).await;

    let stdout = parsed["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("a non-empty result returned zero bytes: {text:.300}"));
    assert!(stdout.starts_with("HEAD"), "the head of the line survives");
    assert!(stdout.ends_with("TAIL"), "the tail of the line survives");
    assert!(
        stdout.contains("bytes shown"),
        "a clipped line must say so: {:.160}",
        stdout
    );
    assert!(
        stdout.contains("grep -o") && stdout.contains(&id),
        "the marker names a working route on the queried handle: {:.400}",
        stdout.chars().skip(1000).collect::<String>()
    );
    assert_eq!(
        parsed["truncated"], true,
        "one line shown whole-by-count but clipped by bytes is still truncated: {text:.300}"
    );
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(
        hint.contains("grep -o"),
        "the hint must name a route that works on a wide line: {hint}"
    );
    assert!(
        !has_tool_handle(&text),
        "the response re-buffered under a second handle: {text:.200}"
    );
    assert!(!crate::tools::exceeds_inline_limit(&text));
}

#[cfg(unix)]
#[tokio::test]
async fn the_advised_next_page_of_a_wide_line_makes_progress() {
    // Line 1 narrow, line 2 wide, line 3 narrow. The first query stops before the wide line and
    // advises `sed -n '2,..p'`; that advised query used to return zero bytes, forever.
    let (_dir, ctx) = project_ctx().await;
    let content = format!("first-line\n{}\nthird-line\n", wide_line(30_000));
    let id = ctx
        .output_buffer
        .store("cmd".into(), content, String::new(), 0);

    let (_t1, p1) = buffer_query(&ctx, format!("cat {id}")).await;
    assert!(p1["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("first-line"));
    let hint1 = p1["hint"].as_str().unwrap_or_default();
    assert!(
        hint1.contains("sed -n '2,"),
        "page 1 must advise starting at line 2: {hint1}"
    );

    // Follow the advice exactly.
    let (t2, p2) = buffer_query(&ctx, format!("sed -n '2,101p' {id}")).await;
    let page2 = p2["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("the advised route returned zero bytes: {t2:.300}"));
    assert!(page2.starts_with("HEAD") && page2.contains("bytes shown"));

    // And FOLLOW page 2's own hint: the page it advises reaches line 3. Progress, not a loop.
    let hint2 = p2["hint"].as_str().unwrap_or_default();
    let next = next_page_command(hint2);
    let (t3, p3) = buffer_query(&ctx, next.clone()).await;
    assert!(
        p3["stdout"]
            .as_str()
            .unwrap_or_else(|| panic!("`{next}` returned zero bytes: {t3:.300}"))
            .contains("third-line"),
        "`{next}`, advised by page 2, did not reach line 3: {t3:.300}"
    );
}
#[cfg(unix)]
#[tokio::test]
async fn a_summarized_query_budgets_stdout_against_the_escaped_stderr() {
    // 3,000 double quotes of stored stderr are 3,000 raw bytes but ~4,000 escaped once bounded.
    // Charging them raw leaves stdout ~2 KB too much room and the response is re-buffered.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=400)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let stderr = format!("HEAD{}TAIL\n", "\"".repeat(3_000));
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep row {id}")).await;

    assert!(
        !has_tool_handle(&text),
        "quote-dense stderr was charged raw and the response re-buffered: {text:.200}"
    );
    assert!(parsed["stderr"]
        .as_str()
        .unwrap_or_default()
        .starts_with("HEAD"));
    assert!(
        !crate::tools::exceeds_inline_limit(&text),
        "{} bytes",
        text.len()
    );
}
#[cfg(unix)]
#[tokio::test]
async fn a_near_limit_query_budgets_stdout_against_the_escaped_stderr() {
    // The near-limit twin of the summary test above: ~9.8 KB of stdout takes the
    // third arm, and 3,000 stored double quotes are ~4,000 escaped once bounded. Charged raw
    // they leave stdout ~2 KB too much room (mutation C3, 2026-10-05).
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=98)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let stderr = format!("HEAD{}TAIL\n", "\"".repeat(3_000));
    let id = ctx.output_buffer.store("cmd".into(), stdout, stderr, 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep row {id}")).await;

    assert!(
        !has_tool_handle(&text),
        "quote-dense stderr was charged raw and the response re-buffered: {text:.200}"
    );
    assert!(parsed["stderr"]
        .as_str()
        .unwrap_or_default()
        .starts_with("HEAD"));
    assert!(
        !crate::tools::exceeds_inline_limit(&text),
        "{} bytes",
        text.len()
    );
}
// ---- a truncation hint names a route that works for the ref the caller queried ----
//
// BUG-adjacent, found by the 2026-10-05 sibling sweep. One buffer-only arm carried a hard-coded
// hint: "a single grep match
// inside a @tool_* ref ... Use read_file(@tool_abc, json_path=\"$.field\")". On a `@cmd_*` or
// `@file_*` query `read_file` REFUSES `json_path`, and `@tool_abc` is a literal placeholder.

#[cfg(unix)]
#[tokio::test]
async fn a_truncated_cmd_query_gets_routes_that_work_on_a_cmd_ref() {
    let (_dir, ctx) = project_ctx().await;
    // 130 lines x 100 B = 13 KB: over the limit, so the query is cut and the hint fires.
    let stdout: String = (1..=130)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), stdout, String::new(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("cat {id}")).await;

    assert_eq!(
        parsed["truncated"], true,
        "the hint only fires on a cut: {text:.300}"
    );
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(
        !hint.contains("json_path") && !hint.contains("@tool_"),
        "`read_file` refuses json_path on a {id} ref, and `@tool_abc` is a placeholder: {hint}"
    );
    assert!(
        hint.contains(&id),
        "the hint names the handle the caller used: {hint}"
    );

    // FOLLOW the route: the page the hint advises returns the very next line after those shown.
    let shown = parsed["stdout_shown"].as_u64().expect("a count") as usize;
    let page = next_page_command(hint);
    let (t2, p2) = buffer_query(&ctx, page.clone()).await;
    let first = p2["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("`{page}` returned zero bytes: {t2:.300}"))
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        first,
        format!("row{:03} {}", shown + 1, "r".repeat(92)),
        "`{page}` did not continue where the first page stopped"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_truncated_tool_query_keeps_json_path_and_the_route_returns_data() {
    // `json_path` IS the right route on a `@tool_*` ref, so it stays, on the real handle, and the
    // route must WORK: following the hint's `read_file` call returns the field it names.
    let (_dir, ctx) = project_ctx().await;
    let blob = format!(
        "{{\"meta\":\"found-me\",\"rows\":[\"{}\"]}}",
        "r".repeat(12_000)
    );
    let id = ctx.output_buffer.store_tool("probe", blob);
    let (text, parsed) = buffer_query(&ctx, format!("cat {id}")).await;

    assert_eq!(parsed["truncated"], true, "{text:.300}");
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(
        hint.contains(&format!("read_file(\"{id}\", json_path=")),
        "a tool ref is read with json_path on ITS OWN handle: {hint}"
    );
    assert!(
        !hint.contains("@tool_abc"),
        "the placeholder is gone: {hint}"
    );

    // Follow it, with the field the hint leaves for the caller to fill in.
    assert!(hint.contains("json_path=\"$.<field>\""), "{hint}");
    let got = crate::tools::read_file::ReadFile
        .call(json!({ "path": id, "json_path": "$.meta" }), &ctx)
        .await
        .unwrap_or_else(|e| panic!("the hinted json_path route failed: {e}"));
    assert!(
        got.to_string().contains("found-me"),
        "the route returned no data: {:.200}",
        got.to_string()
    );
}
// ---- the summary-or-inline gate measures the response, not the raw output ----
//
// BUG-adjacent, found by the 2026-10-05 sibling sweep and verified live: a 175-record pretty-JSON
// run is ~9.3 KB raw but serializes to ~11.4 KB (every quote and newline costs two bytes). The
// gate compared RAW bytes against the limit, so it chose the inline arm; `call_content` measures the
// SERIALIZED response, saw it over the limit, and re-buffered it under `@tool_*` with a
// content-free summary, a `$.field` hint, and no `@cmd_*` handle at all. The same defect hit PLAIN
// text between 9,977 and 10,003 raw bytes, once the response's own keys were counted.

#[cfg(unix)]
#[tokio::test]
async fn a_pretty_json_run_inside_the_old_raw_gate_is_summarized_under_a_cmd_handle() {
    let (_dir, ctx) = project_ctx().await;
    // 175 records, ~53 B raw each = ~9.3 KB raw; ~65 B escaped each = ~11.4 KB.
    let command = r#"awk 'BEGIN{for(i=0;i<175;i++){printf "  {\n    \"name\": \"n%d\",\n    \"status\": \"created\"\n  },\n", i}}'"#;
    let (text, parsed) = buffer_query_free(&ctx, command).await;

    assert!(
        parsed["output_id"].as_str().unwrap_or_default().starts_with("@cmd_"),
        "the run must keep a `@cmd_*` handle, not be re-buffered under a content-free `@tool_*`: {text:.300}"
    );
    assert!(!has_tool_handle(&text), "{text:.200}");
    assert!(parsed.get("buffered_bytes").is_none());
    assert!(
        !crate::tools::exceeds_inline_limit(&text),
        "{} bytes",
        text.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_plain_run_at_the_exact_inline_edge_stays_inline() {
    // `{"exit_code":0,"stdout":""}` is 27 B, so 9,976 B of text is a 10,003 B response: the largest
    // `exceeds_inline_limit` lets through. Output that FITS must not start being summarized.
    let (_dir, ctx) = project_ctx().await;
    let (text, parsed) = buffer_query_free(&ctx, "printf '%09976d' 0").await;

    assert!(
        parsed.get("output_id").is_none(),
        "must not buffer: {text:.200}"
    );
    assert_eq!(parsed["stdout"].as_str().unwrap_or_default().len(), 9_976);
    // The limit applies to the COMPACT response BEFORE `call_content` injects its first-call
    // `_guide_hint`; the inline text is also pretty-printed. Pin the edge itself so a drifted
    // envelope shows here rather than as a silent off-by-N.
    let mut compact = parsed.clone();
    compact.as_object_mut().unwrap().remove("_guide_hint");
    assert_eq!(
        compact.to_string().len(),
        10_003,
        "the response is not at the edge"
    );
    assert!(!crate::tools::exceeds_inline_limit(&compact.to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn a_plain_run_one_byte_past_the_inline_edge_is_summarized_not_rebuffered() {
    // One byte more is a 10,004 B response. The old raw gate called it small, and `call_content`
    // then buffered it under `@tool_*` with no `@cmd_*` handle.
    let (_dir, ctx) = project_ctx().await;
    let (text, parsed) = buffer_query_free(&ctx, "printf '%09977d' 0").await;

    assert!(
        parsed["output_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("@cmd_"),
        "{text:.300}"
    );
    assert!(!has_tool_handle(&text), "{text:.200}");
    assert!(!crate::tools::exceeds_inline_limit(&text));
}
#[cfg(unix)]
#[tokio::test]
async fn a_buffer_query_gate_counts_the_bounded_stored_stderr_it_will_emit() {
    // 85 lines of 100 B = 8,500 B of plain stdout fits on its own (8,527 B response), but the
    // bounded stored stderr this query ALSO emits is ~2.1 KB: together over the limit. A gate that
    // judged the query's own (empty) stderr called it small, and the response was re-buffered.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=85)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), stdout, wide_stderr(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep row {id}")).await;

    assert!(!has_tool_handle(&text), "{text:.300}");
    assert!(parsed["stderr"]
        .as_str()
        .unwrap_or_default()
        .starts_with("HEAD"));
    assert!(
        !crate::tools::exceeds_inline_limit(&text),
        "{} bytes",
        text.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_run_whose_diagnostic_tips_it_over_the_limit_is_summarized() {
    // The streams alone fit (a 9,990 B response). The empty-selection diagnostic attached AFTER the
    // branch adds a few hundred bytes and pushes the whole over the limit; the gate must count it.
    let (_dir, ctx) = project_ctx().await;
    let libtest = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; \
                   5 filtered out; finished in 0.00s\n";
    // Size the filler so the streams alone make a 9,990 B response, measured, not assumed.
    let response_len = |filler: usize| {
        serde_json::json!({"exit_code": 0, "stdout": format!("{libtest}{}", "0".repeat(filler))})
            .to_string()
            .len()
    };
    let filler = 9_990 - (response_len(0));
    assert_eq!(response_len(filler), 9_990);
    let command = format!("printf 'running 0 tests\\n\\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s\\n'; printf '%0{filler}d' 0");
    let (text, parsed) = buffer_query_free(&ctx, &command).await;

    assert!(
        parsed.get("empty_test_selection").is_some(),
        "the fixture must trigger the diagnostic or this proves nothing: {text:.300}"
    );
    assert!(
        parsed["output_id"].as_str().unwrap_or_default().starts_with("@cmd_"),
        "the diagnostic pushed the response over the limit and the gate did not count it: {text:.300}"
    );
    assert!(!has_tool_handle(&text), "{text:.200}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_query_just_under_the_limit_bounds_a_wide_stored_stderr() {
    // ~9.8 KB of stdout fits on its own, and a wide stored stderr beside it does not: the stderr
    // must be bounded and the stdout must give way to it by the MEASURED amount. This size used to
    // land in a third arm that cut at a fixed 9,700 B guard; that arm is gone, because the gate
    // already measures the serialized response and the arm cut responses it had just said fit.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=98)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), stdout, wide_stderr(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep row {id}")).await;

    assert_stderr_bounded(&id, &text, &parsed);
}

#[cfg(unix)]
#[tokio::test]
async fn a_response_exactly_at_the_limit_stays_whole_and_one_byte_over_is_cut() {
    // `cat @cmd` of N bytes with no newline answers `{"exit_code":0,"stdout":"a…"}`: 27 + N bytes
    // COMPACT, the unit `call_content` measures. N = 9,976 is exactly the 10,003 B limit and must
    // come back WHOLE; N = 9,977 is one byte over and must be cut. A fixed guard under the limit
    // (the 9,700 B guard this replaced) clipped the first case for nothing.
    for (n, whole) in [(9_976usize, true), (9_977, false)] {
        let (_dir, ctx) = project_ctx().await;
        let id = ctx
            .output_buffer
            .store("cmd".into(), "a".repeat(n), String::new(), 0);
        let (text, parsed) = buffer_query(&ctx, format!("cat {id}")).await;

        assert!(!has_tool_handle(&text), "n={n}: {text:.200}");
        let stdout = parsed["stdout"].as_str().unwrap_or_default();
        if whole {
            assert_eq!(text.len(), 10_003, "n={n}: not on the edge");
            assert!(parsed.get("truncated").is_none(), "n={n}: cut for nothing");
            assert_eq!(stdout.len(), n, "n={n}: bytes were dropped");
        } else {
            assert_eq!(parsed["truncated"], true, "n={n}");
            assert!(stdout.contains("bytes shown"), "n={n}");
            assert!(
                (9_900..=10_003).contains(&text.len()),
                "n={n}: {} B; a cut must fill the limit, not stop a reserve short of it",
                text.len()
            );
        }
    }
}
// ---- the routes the hints name are FOLLOWED, against the same buffer, and the data comes back ----
//
// A test that compares a hint to a literal proves the sentence is stable, not that the route works;
// the defect these replace was a hint whose advised route returned the identical empty response.
// Each case here runs the command the hint tells the reader to run.

/// The backtick-quoted commands in `text` that start with `prefix`, in order.
fn hinted_commands(text: &str, prefix: &str) -> Vec<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|c| c.starts_with(prefix))
        .map(str::to_string)
        .collect()
}

#[cfg(unix)]
#[tokio::test]
async fn following_the_wide_line_hint_returns_the_data_it_promised() {
    let (_dir, ctx) = project_ctx().await;
    let line = wide_line(78_000);
    let id = ctx
        .output_buffer
        .store("cmd".into(), line.clone(), String::new(), 0);
    let (_t, first) = buffer_query(&ctx, format!("grep status {id}")).await;
    let hint = first["hint"].as_str().unwrap_or_default().to_string();

    // Route 1: a window of the line around a token that occurs ONCE.
    let grep_o = hinted_commands(&hint, "grep -o");
    assert_eq!(grep_o.len(), 1, "the hint names one grep -o route: {hint}");
    let cmd = grep_o[0].replace("TEXT", "HEAD");
    let (t1, p1) = buffer_query(&ctx, cmd.clone()).await;
    let window = p1["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("`{cmd}` returned zero bytes: {t1:.300}"));
    assert!(window.starts_with("HEAD"), "{window:.80}");
    assert_eq!(
        window.trim_end().len(),
        4 + 200,
        "HEAD plus the 200 bytes asked for"
    );

    // Route 2: the first N characters of the line.
    let cut = hinted_commands(&hint, "cut -c");
    assert_eq!(cut.len(), 1, "the hint names one cut route: {hint}");
    let (t2, p2) = buffer_query(&ctx, cut[0].clone()).await;
    let head = p2["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("`{}` returned zero bytes: {t2:.300}", cut[0]));
    assert!(head.starts_with("HEAD") && head.contains("status"));
    assert_eq!(
        head.trim_end().len(),
        4_000,
        "exactly the 4,000 characters asked for"
    );
    assert!(
        line.starts_with(head.trim_end()),
        "and they are the line's own first bytes"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_marker_in_the_clipped_line_names_the_same_working_routes() {
    // The marker travels with the data; a reader who never sees the envelope's hint sees this one.
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("cmd".into(), wide_line(78_000), String::new(), 0);
    let (_t, p) = buffer_query(&ctx, format!("cat {id}")).await;
    let stdout = p["stdout"].as_str().unwrap_or_default().to_string();

    let grep_o = hinted_commands(&stdout, "grep -o");
    assert_eq!(
        grep_o.len(),
        1,
        "the marker names a grep -o route: {stdout:.0}"
    );
    let (t, window) = buffer_query(&ctx, grep_o[0].replace("TEXT", "HEAD")).await;
    assert!(
        window["stdout"]
            .as_str()
            .unwrap_or_default()
            .starts_with("HEAD"),
        "following the marker's own route returned nothing: {t:.300}"
    );
}

// A comma-free wide match is what `grep -o 'PATTERN[^,]*'` returns on JSON or CSV-ish output: ONE
// line, no separator to page on. At 9,850 B the response (27 + the match + its newline, escaped)
// FITS the inline limit, so it comes back WHOLE; at 20,000 B it does not, and a clipped head and
// tail come back instead. Both used to return zero bytes, and a fixed guard under the limit once
// clipped the first for nothing. Real bytes, never none, in both.
#[cfg(unix)]
#[tokio::test]
async fn a_comma_free_wide_match_returns_real_bytes_whole_or_clipped() {
    for (width, whole) in [(9_850usize, true), (20_000, false)] {
        let (_dir, ctx) = project_ctx().await;
        let body = format!("a,b,HEAD{},tail\n", "x".repeat(width));
        let (_id, text, parsed) = query_stored(&ctx, body, String::new(), |id| {
            format!("grep -o 'HEAD[^,]*' {id}")
        })
        .await;

        let stdout = parsed["stdout"].as_str().unwrap_or_else(|| {
            panic!("width {width}: zero bytes of a non-empty match: {text:.300}")
        });
        assert!(stdout.starts_with("HEAD"), "width {width}");
        assert!(!has_tool_handle(&text), "width {width}: {text:.200}");
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "width {width}: {} B compact",
            text.len()
        );
        if whole {
            assert_eq!(
                stdout.trim_end().len(),
                4 + width,
                "width {width}: bytes dropped"
            );
            assert!(
                parsed.get("truncated").is_none(),
                "width {width}: cut for nothing"
            );
        } else {
            assert!(
                stdout.len() > 3_000,
                "width {width}: only {} B came back",
                stdout.len()
            );
            assert!(
                stdout.contains("bytes shown"),
                "width {width}: a clipped match must say so"
            );
            assert_eq!(parsed["truncated"], true, "width {width}: {text:.200}");
        }
    }
}

// ---- the stored-stderr byte bound: exact at the limit, one byte over ----

#[cfg(unix)]
#[tokio::test]
async fn a_stored_stderr_line_of_exactly_the_budget_is_returned_whole() {
    let (_dir, ctx) = project_ctx().await;
    // The literal 2,000, not the constant: a test that reads the constant moves its own edge when
    // the constant is bumped, and a +1 mutation of the budget survived exactly that way.
    let stderr = "e".repeat(2_000);
    let id = ctx
        .output_buffer
        .store("cmd".into(), "a\nb\n".into(), stderr.clone(), 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep -c a {id}")).await;

    assert_eq!(
        parsed["stderr"].as_str().unwrap_or_default(),
        stderr,
        "{text:.200}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_stored_stderr_line_one_byte_over_the_budget_is_cut_and_says_so() {
    let (_dir, ctx) = project_ctx().await;
    let stderr = "e".repeat(2_001);
    let id = ctx
        .output_buffer
        .store("cmd".into(), "a\nb\n".into(), stderr, 0);
    let (text, parsed) = buffer_query(&ctx, format!("grep -c a {id}")).await;

    let got = parsed["stderr"].as_str().unwrap_or_default();
    assert!(got.contains("bytes shown"), "{text:.200}");
    assert!(
        got.contains("of 2001 bytes shown"),
        "the total names the stored stream: {:.0}",
        got
    );
}
// ---- stderr that escapes to more bytes than it holds, and the query's own stderr ----

#[cfg(unix)]
#[tokio::test]
async fn a_control_character_stored_stderr_keeps_one_handle_and_the_answer() {
    // 50,000 x \x01 is 50,000 raw bytes and 300,000 serialized. A raw-byte cut of 2,000 serialized
    // to ~12 KB, so the response was re-buffered under `@tool_*` and the answer (`2`) hidden.
    let (_dir, ctx) = project_ctx().await;
    let (_id, text, parsed) =
        query_stored(&ctx, "a1\na2\nb\n".into(), "\u{1}".repeat(50_000), |id| {
            format!("grep -c a {id}")
        })
        .await;

    assert!(!has_tool_handle(&text), "re-buffered: {text:.200}");
    assert_eq!(
        parsed["stdout"].as_str().unwrap_or_default().trim(),
        "2",
        "the query's own answer must be in the response: {text:.200}"
    );
    assert!(parsed["stderr"]
        .as_str()
        .unwrap_or_default()
        .contains("bytes shown"));
    assert!(
        text.len() <= 10_003,
        "{} B compact is over the limit",
        text.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_wide_stdout_beside_a_control_character_stderr_is_never_zero_bytes() {
    // The bounded stderr used to serialize to ~12 KB, which drove the stdout budget to ZERO and
    // `clip_to_bytes(line, 0)` returned nothing: the "never zero bytes of a non-empty result"
    // rule broken by its own caller.
    let (_dir, ctx) = project_ctx().await;
    let (_id, text, parsed) =
        query_stored(&ctx, "a".repeat(20_000), "\u{1}".repeat(50_000), |id| {
            format!("cat {id}")
        })
        .await;

    let stdout = parsed["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("zero bytes of a non-empty result: {text:.300}"));
    assert!(
        stdout.starts_with("aaaa") && stdout.len() > 3_000,
        "{} B",
        stdout.len()
    );
    assert!(!has_tool_handle(&text), "{text:.200}");
    assert!(text.len() <= 10_003, "{} B compact", text.len());
}

#[cfg(unix)]
#[tokio::test]
async fn a_query_own_stderr_is_bounded_and_its_marker_does_not_name_a_stored_handle() {
    // The reviewer's input: 77 stored lines of 100 B (7,777 B of stdout), queried with an awk that
    // also writes 5,000 B to ITS OWN stderr. That stderr is stored nowhere, so a marker saying
    // `all of it: @cmd_X.err` is false, and unbounded it is 5 KB the response cannot carry.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=77)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let awk = |id: &str, tail: &str| {
        format!(
            "awk '{{print}} END{{ for(i=0;i<5000;i++) s=s \"E\"; print s | \"cat 1>&2\"}}' {id}{tail}"
        )
    };
    let (id, text, parsed) = query_stored(&ctx, stdout, String::new(), |id| awk(id, "")).await;

    assert!(!has_tool_handle(&text), "{text:.200}");
    assert_eq!(
        parsed["stdout"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .count(),
        77,
        "the response fits, so the whole stdout comes back: {text:.200}"
    );
    assert!(
        parsed.get("truncated").is_none(),
        "cut for nothing: {text:.200}"
    );
    let stderr = parsed["stderr"].as_str().expect("the query's own stderr");
    assert!(stderr.contains("bytes shown"), "{stderr:.0}");
    assert!(stderr.contains("stored nowhere"), "{stderr:.0}");
    assert!(
        !stderr.contains(&format!("{id}.err")),
        "the marker names a handle that does not hold this stream"
    );
    assert!(
        crate::util::text::json_escaped_len(stderr) <= 2_000,
        "{} B escaped",
        crate::util::text::json_escaped_len(stderr)
    );

    // FOLLOW the rerun route the marker gives: it returns a window of the stream that was cut.
    let (t2, p2) = buffer_query(&ctx, awk(&id, " 2>&1 >/dev/null | cut -c1-4000")).await;
    let window = p2["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("the rerun route returned nothing: {t2:.300}"));
    assert_eq!(
        window.trim_end(),
        "E".repeat(4_000),
        "a window of the cut stream"
    );
}

// ---- the counters and the late keys are inside the gate, at the exact edge ----

#[cfg(unix)]
#[tokio::test]
async fn the_stderr_counters_are_inside_the_gate_at_the_exact_edge() {
    // Stored stderr of 25 lines is cut to 20, so the response carries `stderr_shown` and
    // `stderr_total`. Sized from a MEASURED response, not a constant: stdout of N 'a' bytes makes
    // the compact response `base + N`, so N* lands it exactly on 10,003. A gate that forgot the
    // counters would call N*+1 small and the response would go over the limit.
    let stderr: String = (0..25).map(|i| format!("e{i:02}\n")).collect();
    let probe = |n: usize| {
        let stderr = stderr.clone();
        async move {
            let (_dir, ctx) = project_ctx().await;
            let (_id, text, parsed) =
                query_stored(&ctx, "a".repeat(n), stderr, |id| format!("cat {id}")).await;
            (text, parsed)
        }
    };
    let (small, _) = probe(100).await;
    let n_edge = 10_003 - (small.len() - 100);

    let (at, at_parsed) = probe(n_edge).await;
    assert_eq!(at.len(), 10_003, "the measured size is not the edge");
    assert!(at_parsed.get("truncated").is_none(), "cut for nothing");
    assert_eq!(at_parsed["stderr_shown"], 20);

    let (over, over_parsed) = probe(n_edge + 1).await;
    assert_eq!(over_parsed["truncated"], true, "one byte over must be cut");
    assert!(!has_tool_handle(&over), "{over:.200}");
    assert!(
        over.len() <= 10_003,
        "{} B: the counters were not counted by the gate",
        over.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn late_keys_are_inside_the_gate_at_the_exact_edge() {
    // `redacted_credentials` and `unfiltered_output_skipped` used to be added by the caller AFTER
    // the gate ran, so a run at exactly the limit that also redacted a credential was re-buffered
    // under `@tool_*` while the same run without one stayed inline.
    use super::output::{handle_successful_output_with, LateKeys};
    let (_dir, ctx) = project_ctx().await;
    let run = |n: usize, late: LateKeys| {
        let ctx = &ctx;
        async move {
            handle_successful_output_with(
                "echo hi",
                "a".repeat(n),
                String::new(),
                0,
                false,
                None,
                std::path::Path::new("."),
                ctx,
                late,
            )
            .await
            .unwrap()
        }
    };
    for (name, make, key_len) in [
        (
            "redacted_credentials",
            Box::new(|| LateKeys {
                redacted: 1,
                tee_skipped: None,
            }) as Box<dyn Fn() -> LateKeys>,
            json!({"exit_code": 0, "stdout": "", "redacted_credentials": 1})
                .to_string()
                .len(),
        ),
        (
            "unfiltered_output_skipped",
            Box::new(|| LateKeys {
                redacted: 0,
                tee_skipped: Some("full".into()),
            }),
            json!({"exit_code": 0, "stdout": "", "unfiltered_output_skipped": "full"})
                .to_string()
                .len(),
        ),
    ] {
        let n = 10_003 - key_len;
        let at = run(n, make()).await;
        assert!(
            at.get("output_id").is_none(),
            "{name}: a response ON the limit was summarized"
        );
        assert_eq!(at.to_string().len(), 10_003, "{name}: not on the edge");

        let over = run(n + 1, make()).await;
        assert!(
            over["output_id"]
                .as_str()
                .unwrap_or_default()
                .starts_with("@cmd_"),
            "{name}: one byte over must take the summary arm, not be re-buffered later"
        );

        // The control: without the key the same stdout fits whole, so the key is what tipped it.
        let control = run(n + 1, LateKeys::default()).await;
        assert!(
            control.get("output_id").is_none(),
            "{name}: the key is not what tipped it"
        );
    }
}
#[cfg(unix)]
#[tokio::test]
async fn a_tee_capture_behind_an_empty_stdout_is_inside_the_gate_at_the_exact_edge() {
    // A capture behind an EMPTY filtered stdout adds the tee keys AND an explicit `"stdout":""`.
    // The size is taken from a MEASURED response: a run with an empty `unfiltered_output_skipped`
    // gives the base length, so a note of `10,003 - base` bytes puts the response exactly on the
    // limit. A gate that forgot either the tee keys or the empty-stdout key would call one byte
    // over small, and the response would go over the limit.
    use super::output::{handle_successful_output_with, LateKeys};
    let (dir, ctx) = project_ctx().await;
    let capture = dir.path().join("capture.txt");
    let run = |note: usize| {
        let (ctx, capture) = (&ctx, capture.clone());
        async move {
            std::fs::write(&capture, "captured\n").unwrap();
            handle_successful_output_with(
                "echo hi",
                String::new(),
                String::new(),
                0,
                false,
                Some(super::inner::TmpfileGuard(
                    capture.to_string_lossy().into_owned(),
                )),
                std::path::Path::new("."),
                ctx,
                LateKeys {
                    redacted: 0,
                    tee_skipped: Some("n".repeat(note)),
                },
            )
            .await
            .unwrap()
        }
    };
    let base = run(0).await;
    assert!(base["unfiltered_output"].is_string(), "{base}");
    assert_eq!(base["stdout"], "", "{base}");
    let note_edge = 10_003 - base.to_string().len();

    let at = run(note_edge).await;
    assert!(
        at.get("output_id").is_none(),
        "a response ON the limit was summarized: {at}"
    );
    assert_eq!(at.to_string().len(), 10_003, "not on the edge");

    let over = run(note_edge + 1).await;
    assert!(
        over["output_id"]
            .as_str()
            .unwrap_or_default()
            .starts_with("@cmd_"),
        "one byte over must take the summary arm: {:.200}",
        over.to_string()
    );
}
#[cfg(unix)]
#[tokio::test]
async fn a_truncated_page_fills_the_limit_to_within_one_line() {
    // 130 lines of 100 B (13 KB) cut by BYTES: the page must end within one line of the limit,
    // not a reserve short of it. A page budgeted for the clipped-line sentence it does not carry
    // (172 B) or for an 800 B reserve ends two lines short, and this fails.
    let (_dir, ctx) = project_ctx().await;
    let stdout: String = (1..=130)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let (_id, text, parsed) =
        query_stored(&ctx, stdout, String::new(), |id| format!("cat {id}")).await;

    assert_eq!(parsed["truncated"], true, "{text:.200}");
    assert!(
        text.len() <= 10_003,
        "{} B compact is over the limit",
        text.len()
    );
    let slack = 10_003 - text.len();
    assert!(
        slack < 100,
        "the page stops {slack} B short of the limit: more than a whole 100 B line would have fit"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_truncated_page_counts_the_late_keys_it_carries() {
    // A buffer query cut by bytes also carries a late key (`unfiltered_output_skipped`, 400 B
    // here). The page room is measured from a skeleton that has every key; one that left the late
    // keys out sends the page 400 B over the limit.
    use super::output::{handle_successful_output_with, LateKeys};
    let (_dir, ctx) = project_ctx().await;
    let wide: String = (1..=130)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(92)))
        .collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), wide.clone(), String::new(), 0);
    let result = handle_successful_output_with(
        &format!("cat {id}"),
        wide,
        String::new(),
        0,
        true,
        None,
        std::path::Path::new("."),
        &ctx,
        LateKeys {
            redacted: 0,
            tee_skipped: Some("n".repeat(400)),
        },
    )
    .await
    .unwrap();

    assert_eq!(result["truncated"], true, "{:.200}", result.to_string());
    assert!(
        result["unfiltered_output_skipped"].is_string(),
        "the late key is attached"
    );
    let len = result.to_string().len();
    assert!(
        len <= 10_003,
        "{len} B compact: the page did not count its late key"
    );
    assert!(10_003 - len < 100, "{} B short of the limit", 10_003 - len);
}

#[cfg(unix)]
#[tokio::test]
async fn following_the_stderr_marker_reads_the_whole_stream_back() {
    // The marker says "all of it: <handle>.err". Follow it: the whole stderr must come back.
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("cmd".into(), "a\n".into(), wide_stderr(), 0);
    let (_t, first) = buffer_query(&ctx, format!("grep -c a {id}")).await;
    let marker = first["stderr"].as_str().unwrap_or_default();
    assert!(marker.contains(&format!("{id}.err")), "{marker:.0}");

    // `grep -o 'TAIL' <id>.err` proves the stream behind the named handle holds what was cut.
    let (t, tail) = buffer_query(&ctx, format!("grep -o TAIL {id}.err")).await;
    assert_eq!(
        tail["stdout"].as_str().unwrap_or_default().trim_end(),
        "TAIL",
        "the `.err` handle named by the marker holds the cut bytes: {t:.300}"
    );
}
// ---- wip_authors is bounded: files x peers is the only thing that sized it ----
//
// Measured 2026-10-05 against scripts/attribute-red.py: a red naming 5 dirty files answers in
// 1,466 B, 150 in 35,976 B. Nothing capped the field, and the backstop in `call_content` says the
// tool's own buffer holds what it cuts, which is false for a diagnostic computed from the streams.

/// A git repo with `n` committed files that are then dirtied; names deep enough that the answer
/// grows by ~240 B a file, like the repo paths that produced the measurement. `None` when git is
/// unavailable, so a skipped run is never read as a passing one (the caller prints why).
fn dirty_repo_with(n: usize) -> Option<(tempfile::TempDir, Vec<String>)> {
    let dir = tempfile::tempdir().ok()?;
    let p = dir.path();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    git(&["init", "-q"])?;
    git(&["config", "user.email", "t@t"])?;
    git(&["config", "user.name", "t"])?;
    let sub = "src/some/deeply/nested/module/directory";
    std::fs::create_dir_all(p.join(sub)).ok()?;
    let names: Vec<String> = (0..n)
        .map(|i| format!("{sub}/file_number_{i:04}.rs"))
        .collect();
    for f in &names {
        std::fs::write(p.join(f), "fn main() {}\n").ok()?;
    }
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "seed"])?;
    for f in &names {
        std::fs::write(p.join(f), "fn main() { broken\n").ok()?;
    }
    Some((dir, names))
}

fn red_naming(files: &[String]) -> String {
    files
        .iter()
        .map(|f| format!("error[E0425]: cannot find value `broken`\n  --> {f}:1:13\n"))
        .collect()
}

#[cfg(unix)]
#[tokio::test]
async fn a_red_naming_many_dirty_files_carries_a_bounded_wip_authors() {
    let (_ctx_dir, ctx) = project_ctx().await;
    let run = |dir: std::path::PathBuf, red: String| {
        let ctx = &ctx;
        async move {
            super::output::handle_successful_output(
                "cargo build",
                red,
                String::new(),
                101,
                false,
                None,
                &dir,
                ctx,
            )
            .await
            .expect("a completed command returns a response")
        }
    };

    // Control: the engine answers at all here. Without it an absent python3 yields the same
    // missing field as a bound that deleted the diagnostic, and this would pass vacuously.
    let Some((small_dir, small_files)) = dirty_repo_with(3) else {
        eprintln!("skipping: git unavailable");
        return;
    };
    let small = run(small_dir.path().to_path_buf(), red_naming(&small_files)).await;
    let Some(small_who) = small["wip_authors"].as_str() else {
        eprintln!("skipping: no diagnostic at 3 files (python3 absent?)");
        return;
    };
    assert!(
        !small_who.contains("bytes shown"),
        "a diagnostic under the budget is returned whole"
    );

    let (dir, files) = dirty_repo_with(150).expect("git worked a moment ago");
    let big = run(dir.path().to_path_buf(), red_naming(&files)).await;
    let who = big["wip_authors"]
        .as_str()
        .expect("the same engine answers for 150 files");

    assert!(
        who.len() <= super::output::WIP_AUTHORS_BYTE_BUDGET + 300,
        "{} B of wip_authors: the field is unbounded",
        who.len()
    );
    assert!(
        who.contains("file_number_0000"),
        "the head names the first file"
    );
    assert!(
        who.contains("silence is not 'nobody'"),
        "the tail keeps the scope footer that stops silence reading as an exoneration"
    );
    assert!(who.contains("bytes shown"), "a cut must say so");
    assert!(
        who.contains("git status --short"),
        "the marker names a route that lists every file, and the diagnostic is stored nowhere else"
    );
    assert!(
        !who.contains("tool's own buffer"),
        "the backstop's remedy would be FALSE for this field"
    );
    // And the route the marker names really lists what was cut.
    let (_t, listed) = buffer_query_free(
        &ctx,
        &format!("git -C {} status --short", dir.path().display()),
    )
    .await;
    assert!(listed["stdout"]
        .as_str()
        .unwrap_or_default()
        .contains("file_number_0149"));
}

#[test]
fn wip_authors_is_returned_whole_at_exactly_the_budget_and_cut_one_byte_over() {
    let at = "w".repeat(super::output::WIP_AUTHORS_BYTE_BUDGET);
    assert_eq!(super::output::bound_wip_authors(at.clone()), at);
    let over = "w".repeat(super::output::WIP_AUTHORS_BYTE_BUDGET + 1);
    let cut = super::output::bound_wip_authors(over);
    assert!(cut.contains("of 3001 bytes shown"), "{cut:.0}");
}
// ---- C: the remaining run_command fields cannot mint a second handle, and the tests say why ----

/// Compacted libtest response (`compacted_test_response`). It is built only when the run is under
/// the inline gate and the compacted text is at most 70% of the raw size, and it ALWAYS carries its
/// own `@cmd_*` `output_id`, so a response that still came out over the limit is clipped by
/// `clip_prebuffered_envelope` and never re-buffered under `@tool_*`. This sweeps raw sizes from the
/// 1,024 B compaction floor to the inline edge with quote-dense kept lines (the worst case for
/// escaping), through the real builder AND the clip, and requires: one handle, of the right kind,
/// within the limit. A size that falls out of the compaction arm lands in the summary or raw arm
/// and is held to the same property.
#[cfg(unix)]
#[tokio::test]
async fn a_libtest_response_never_needs_a_second_handle_at_any_size() {
    let mut compacted = 0usize;
    for payload in (0..=9_500usize).step_by(500) {
        let kept: String = (0..)
            .map(|i| format!("    \"case {i}\" \\\"x\\\" \"y\"\n"))
            .scan(0usize, |n, line| {
                if *n >= payload {
                    return None;
                }
                *n += line.len();
                Some(line)
            })
            .collect();
        let stdout = format!("{INLINE_EMPTY_WORKSPACE_STDOUT}{kept}");
        let (result, _ctx) =
            run_inline("cargo test x", &stdout, INLINE_EMPTY_WORKSPACE_STDERR).await;
        if result["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("codescout compacted this run")
        {
            compacted += 1;
        }
        let before_clip = result.to_string();
        let out = crate::tools::clip_prebuffered_envelope(result, false);
        let text = out.to_string();
        assert!(
            !crate::tools::exceeds_inline_limit(&before_clip),
            "payload {payload}: the response is {} B BEFORE the backstop, so the gate and the 70% \
             rule alone do not bound it",
            before_clip.len()
        );
        let id = out["output_id"].as_str().unwrap_or("");
        assert!(
            id.is_empty() || id.starts_with("@cmd_"),
            "payload {payload}: a handle that is not the tool's own: {id}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "payload {payload}: a {} B response would be re-buffered under a second handle",
            text.len()
        );
    }
    assert!(
        compacted >= 3,
        "only {compacted} sizes reached the compaction arm, so the sweep did not test it"
    );
}

/// Interactive stdout. The accumulated output has no cap, but the response has NO `output_id`:
/// nothing is stored behind a `@cmd_*` handle, so there is exactly one place the full text can
/// live, and it is the single `@tool_*` buffer `call_content` makes. That is the designed
/// single-handle path, not a second handle, and it is line-addressable (a multi-line `stdout` is
/// materialized one line per line). Pinned in three steps: the response carries no handle; the clip
/// leaves it alone; and a range read of the one buffer returns the lines.
#[cfg(unix)]
#[tokio::test]
async fn an_oversized_interactive_stdout_has_one_handle_and_it_reads_back() {
    let (_dir, ctx) = project_ctx().await;
    let output: String = (1..=3_000).map(|i| format!("repl line {i}\n")).collect();
    let response = super::interactive::interactive_response(0, &output, 7, None);

    assert!(response.get("output_id").is_none(), "{response}");
    assert_eq!(
        crate::tools::clip_prebuffered_envelope(response.clone(), false),
        response,
        "no handle of its own, so the clip never touches it"
    );
    assert!(crate::tools::exceeds_inline_limit(&response.to_string()));

    let id = ctx
        .output_buffer
        .store_tool("run_command", response.to_string());
    let page = crate::tools::read_file::ReadFile
        .call(
            json!({ "path": id, "json_path": "$.interactive_rounds" }),
            &ctx,
        )
        .await;
    assert!(page.is_ok(), "the one handle is readable: {page:?}");
    let lines = crate::tools::read_file::ReadFile
        .call(json!({ "path": id, "start_line": 1, "end_line": 12 }), &ctx)
        .await
        .expect("a range read of the single handle");
    assert!(
        lines.to_string().contains("repl line 2"),
        "the interactive text comes back from the one handle: {lines}"
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
    // Effect (not necessarily the original intent, which is unrecorded): this pins
    // ONBOARDING_VERSION so a bump cannot be silent — it must be paired with an edit here.
    // Bumping is required when the `onboarding_prompt` surface changes, because already-
    // onboarded projects cache the rendered prompt; see src/prompts/README.md. Last moved
    // to 31 on 2026-09-05, when the surface's artifact-tracking step stopped naming
    // `librarian_context(topic)` — retired 2026-05-02, and invisible to the prompt-surface
    // gate for four months because that gate could not see a name written in CALL form.
    assert_eq!(ONBOARDING_VERSION, 31);
}

#[test]
fn system_prompt_draft_read_file_hint_mentions_file_ref_reuse() {
    let draft = build_system_prompt_draft(
        &["rust".to_string()],
        &["src/main.rs".to_string()],
        None,
        None,
        &[],
    );
    assert!(
        draft.contains("@file_ref") || draft.contains("@file_"),
        "draft must teach @file_* reuse for read_file (heading-addressed); got:\n{draft}"
    );
    assert!(
        draft.contains("IRON LAW #6"),
        "draft must cite IRON LAW #6 in the read_file guidance; got:\n{draft}"
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

/// REACH, not logic. `summarize_test_output` now emits a `stderr` key; this asserts it
/// survives the one function standing between that key and the caller.
///
/// `rebuild_buffered_summary` reorders an envelope by copying fields into three named
/// groups, so a key it does not enumerate is dropped silently — the exact shape of
/// `CLAUDE.md` § *Testing Discipline*: "a field this function does not read reaches
/// nobody". Every other test for this fix asserts about the summarizer's return value,
/// which is upstream of this and identical whether the key arrives or not.
///
/// The absence is checked BEFORE the value, and that ordering is load-bearing: written
/// as `rebuilt["stderr"].as_str().unwrap()` the mutation panics on `unwrap` at this
/// line and the explanatory message never renders, so the failure says
/// `called Option::unwrap() on a None value` — true, and silent about which field went
/// and why anyone cares. Measured when the mutation below was first run.
#[test]
fn rebuild_buffered_summary_preserves_the_test_envelopes_stderr() {
    let verdict = "mutation-probe: INCONCLUSIVE — the runner selected 0 tests\n";
    let raw = json!({
        "type": "test",
        "exit_code": 0,
        "passed": 0,
        "stderr": verdict,
    });
    let rebuilt = crate::tools::run_command::output::rebuild_buffered_summary(raw, "@cmd_abc123");
    let got = rebuilt["stderr"].as_str();
    assert!(
        got.is_some(),
        "rebuild_buffered_summary dropped the `stderr` key, so a wrapper's verdict cannot \
         reach the caller even though the summarizer emitted it; got {rebuilt}"
    );
    assert_eq!(got.unwrap(), verdict);
    assert_eq!(rebuilt["output_id"], "@cmd_abc123");
}

#[test]
fn run_command_format_compact_short_output() {
    let tool = RunCommand;
    let result = json!({ "stdout": "hello\nworld", "stderr": "", "exit_code": 0 });
    let text = tool.format_compact(&result).unwrap();
    assert!(text.contains("exit 0"), "got: {text}");
}

/// The real stderr from the reported incident, verbatim. Four errors, and the one that
/// reads most like an explanation is the wrong one.
/// See `docs/issues/archive/2026-08-16-run-command-backticks-substituted-in-quoted-message.md`.
const SUBSTITUTION_STDERR: &str = "Usage: grep [OPTION]... PATTERNS [FILE]...\n\
     Try 'grep --help' for more information.\n\
     sh: command substitution: line 1: syntax error near unexpected token `...'\n\
     sh: line 1: `self.call(...).await?'\n\
     sh: line 1: ?: command not found\n\
     sh: line 1: /usr/bin/git: Argument list too long\n";

#[test]
fn substitution_diagnostic_names_the_cause_and_disowns_the_misleading_line() {
    use super::output::substitution_diagnostic;

    let cmd = "git commit -m \"fix: rename `is_unbounded_lhs` and `self.call(...).await?`\"";
    let cause = substitution_diagnostic(cmd, SUBSTITUTION_STDERR)
        .expect("the shell's own marker is present, so the cause must be named");

    assert!(
        cause.contains("command substitution"),
        "must name the mechanism: {cause}"
    );
    assert!(
        cause.contains("backtick"),
        "must name what in the command triggered it: {cause}"
    );
    // The load-bearing assertion: the last stderr line is a self-consistent WRONG
    // explanation, and acting on it loses commit-message content for no benefit.
    assert!(
        cause.contains("CONSEQUENCE"),
        "must disown `Argument list too long` as a consequence, or the caller shortens \
         the message and fixes nothing: {cause}"
    );
    assert!(
        cause.contains("git commit -F"),
        "must give a runnable correction: {cause}"
    );
}

/// Anchored on the shell's marker, not on command shape — so a command that genuinely
/// wanted substitution and got it stays silent. Without this the diagnostic would fire on
/// every backtick-bearing command, including the working ones.
#[test]
fn substitution_diagnostic_is_silent_when_substitution_worked() {
    use super::output::substitution_diagnostic;

    let cmd = "echo \"today is `date +%F`\"";
    assert!(
        substitution_diagnostic(cmd, "").is_none(),
        "no shell marker means no claim"
    );
    assert!(
        substitution_diagnostic(cmd, "some unrelated warning\n").is_none(),
        "unrelated stderr must not be read as a substitution failure"
    );
}

/// The marker without any substitution syntax in the command we were handed: the failure
/// came from somewhere else (a nested script, an alias), so claiming a cause we cannot
/// point at in the caller's own string would be a guess.
#[test]
fn substitution_diagnostic_is_silent_when_the_command_shows_no_substitution() {
    use super::output::substitution_diagnostic;

    assert!(
        substitution_diagnostic("bash script.sh", SUBSTITUTION_STDERR).is_none(),
        "the cause must be visible in the caller's command to be claimed"
    );
}

/// Real `cargo test` output, measured 2026-09-16 on `--test doc_tool_refs` with a filter
/// naming a HELPER function rather than a test. The word `ok` and the exit code are
/// byte-identical to a genuinely passing run; only which integers are non-zero differs.
/// See `docs/issues/archive/2026-09-13-a-test-filter-that-matches-nothing-reports-success.md`.
const EMPTY_SELECTION_STDOUT: &str = "\nrunning 0 tests\n\ntest result: ok. 0 passed; \
     0 failed; 0 ignored; 0 measured; 14 filtered out; finished in 0.00s\n";

/// The same target, same day, no filter — the control that makes the above a measurement.
const FULL_RUN_STDOUT: &str = "\nrunning 14 tests\n\ntest result: ok. 14 passed; \
     0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.60s\n";

#[test]
fn an_empty_test_selection_is_named_rather_than_left_reading_as_a_pass() {
    use super::output::empty_test_selection_diagnostic;

    let d = empty_test_selection_diagnostic(EMPTY_SELECTION_STDOUT)
        .expect("0 passed against 14 filtered out is an empty selection, not a pass");

    assert!(
        d.contains("filtered out"),
        "must name the discriminator that was already in the output: {d}"
    );
    assert!(
        d.contains("14"),
        "must quote the count, so the reader can check the claim against their own \
         screen rather than take it: {d}"
    );
    // The load-bearing assertion, and the exact analogue of the substitution case's
    // `Argument list too long`: the reader's problem is not a missing signal, it is a
    // present and self-consistent WRONG one. Naming `filtered out` while leaving `ok`
    // unchallenged leaves both readings alive.
    assert!(
        d.contains("ok"),
        "must disown the success word itself, not merely add a fact beside it: {d}"
    );
}

/// The commonest shape of all, and the one a careless predicate breaks. A filter that
/// matched 3 of 14 is a CORRECT selective run; `filtered out > 0` alone cannot tell it
/// from an empty selection, which is why the predicate is `nothing ran`, not `something
/// was filtered`.
#[test]
fn a_filter_that_matched_something_is_not_an_empty_selection() {
    use super::output::empty_test_selection_diagnostic;

    let partial = "\nrunning 3 tests\n\ntest result: ok. 3 passed; 0 failed; 0 ignored; \
                   0 measured; 11 filtered out; finished in 0.12s\n";
    assert!(
        empty_test_selection_diagnostic(partial).is_none(),
        "a selective run that ran something is exactly what filters are for"
    );
}

/// The whole target. `filtered out` is 0 by construction, so there is nothing to warn about
/// — this is the shape the bug file recommends preferring.
#[test]
fn a_full_target_run_is_silent() {
    use super::output::empty_test_selection_diagnostic;

    assert!(
        empty_test_selection_diagnostic(FULL_RUN_STDOUT).is_none(),
        "an unfiltered run cannot select empty"
    );
}

/// A target holding NO TESTS AT ALL — an integration file whose tests were deleted, or a
/// workspace member that never had any. `0 passed; 0 filtered out` is an empty POPULATION,
/// not an empty selection: there is no filter to blame and nothing to tell the caller.
///
/// This case exists because the `filtered == 0` guard had nothing else guarding it.
/// Measured 2026-09-16: deleting that guard SURVIVED the suite, because
/// `a_full_target_run_is_silent` carries `14 passed` and the `passed > 0` guard refuses it
/// first. A case written for one bound was being answered by another — the same shape as
/// `bug-fix-session-log:W-143`, in a different language, and found the same way.
#[test]
fn a_target_with_no_tests_at_all_is_silent() {
    use super::output::empty_test_selection_diagnostic;

    let empty_target = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; \
                        0 measured; 0 filtered out; finished in 0.00s\n";
    assert!(
        empty_test_selection_diagnostic(empty_target).is_none(),
        "no tests and no filter is an empty population, not an empty selection"
    );
}

/// THE false-positive guard, and the reason this cannot be decided one `test result:` line
/// at a time. `cargo test <filter>` builds every target in the workspace and each prints its
/// own result line, so a target holding no match prints `0 passed; N filtered out`
/// LEGITIMATELY while a sibling runs the match. A per-line predicate would fire on every
/// successful filtered workspace run — turning the diagnostic into noise, which is the one
/// failure mode that gets a warning ignored rather than read.
#[test]
fn a_workspace_run_where_another_target_matched_is_silent() {
    use super::output::empty_test_selection_diagnostic;

    let workspace = "\nrunning 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; \
                     0 measured; 14 filtered out; finished in 0.00s\n\
                     \nrunning 1 test\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; \
                     0 measured; 7 filtered out; finished in 0.03s\n";
    assert!(
        empty_test_selection_diagnostic(workspace).is_none(),
        "one target matching nothing is normal when a sibling matched; the caller's \
         selection was NOT empty"
    );
}

/// Matched, but every match was `#[ignore]`d. Nothing ran, yet the selection was not empty
/// and the remedy is different (`-- --ignored`), so claiming an empty selection would send
/// the reader somewhere useless.
#[test]
fn a_selection_whose_matches_were_all_ignored_is_silent() {
    use super::output::empty_test_selection_diagnostic;

    let ignored = "\nrunning 2 tests\n\ntest result: ok. 0 passed; 0 failed; 2 ignored; \
                   0 measured; 12 filtered out; finished in 0.00s\n";
    assert!(
        empty_test_selection_diagnostic(ignored).is_none(),
        "an ignored match is a match; the filter selected something"
    );
}

/// A red is already loud and already routed — `wip_authors` covers it. Firing here too would
/// put a second, quieter explanation beside a real failure.
#[test]
fn a_failing_run_is_left_to_the_red_that_already_speaks() {
    use super::output::empty_test_selection_diagnostic;

    let failed = "\nrunning 14 tests\n\ntest result: FAILED. 0 passed; 1 failed; 0 ignored; \
                  0 measured; 13 filtered out; finished in 0.20s\n";
    assert!(
        empty_test_selection_diagnostic(failed).is_none(),
        "a FAILED line is not a vacuous pass"
    );
}

/// The boundary test, and this diagnostic needs one more than most. `format_compact` builds
/// its one-liner from a fixed set of keys, so a field it does not read reaches nobody however
/// correct the JSON is — the defect filed as
/// `docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`.
/// Here the unread field would sit beside an `exit 0` the reader has every reason to believe,
/// so the silent-drop failure is indistinguishable from the bug this whole change exists to
/// fix. Asserting the function EXISTS is not the same as asserting its verdict ARRIVES.
#[test]
fn the_compact_renderer_actually_shows_an_empty_selection() {
    let tool = RunCommand;
    let result = json!({
        "stdout": EMPTY_SELECTION_STDOUT,
        "stderr": "",
        "exit_code": 0,
        "empty_test_selection": "This run selected NO tests: `14 filtered out` against `0 passed`",
    });
    let text = tool.format_compact(&result).unwrap();
    assert!(
        text.contains("selected NO tests"),
        "the verdict must reach the surface call_content renders: {text}"
    );
    assert!(
        text.contains("exit 0"),
        "and it must sit beside the exit code it contradicts, not replace it: {text}"
    );
}

/// The boundary test. `format_compact` is what `call_content` renders, and it builds a
/// one-liner from a fixed set of keys — so a field it does not read reaches nobody, no
/// matter how correct the JSON is. That is the defect filed as
/// `docs/issues/archive/2026-08-17-allocate-outcome-frontmatter-max-dropped-at-the-mcp-boundary.md`,
/// and this test is what stops it recurring here.
#[test]
fn format_compact_surfaces_the_shell_cause_on_every_output_shape() {
    let tool = RunCommand;

    let short = json!({
        "stdout": "", "stderr": SUBSTITUTION_STDERR, "exit_code": 126,
        "shell_cause": "The shell performed command substitution on a backtick …"
    });
    let text = tool.format_compact(&short).unwrap();
    assert!(
        text.contains("cause:"),
        "short-output shape must surface the cause: {text}"
    );

    // Same assertion through the buffered shape, which renders from a different branch.
    let buffered = json!({
        "type": "generic", "exit_code": 126, "output_id": "@cmd_abc123",
        "shell_cause": "The shell performed command substitution on a backtick …"
    });
    let text = tool.format_compact(&buffered).unwrap();
    assert!(
        text.contains("cause:"),
        "buffered shape must surface it too — the attachment is after the branch \
         precisely so both are covered: {text}"
    );
}

// Fix A: buffer-only queries should use BUFFER_QUERY_INLINE_CAP, not
// the summarization threshold. A 100-line result should be returned fully inline.
#[tokio::test]
async fn buffer_query_returns_up_to_200_lines_inline() {
    let (_dir, ctx) = project_ctx().await;
    // Directly store 100 lines in the buffer (bypasses needs_summary)
    let content: String = (1..=100).map(|i| format!("{i}\n")).collect();
    let output_id = ctx.output_buffer.store("cmd".into(), content, "".into(), 0);

    // Query the buffer — 100 lines is within the BUFFER_QUERY_INLINE_CAP.
    // `cat` works on both platforms now that Windows runs through Git Bash.
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
        .store_dangerous("echo hello_ack".to_string(), None, 30, false);

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

/// Regression: `run_in_background` was accepted, dropped into `store_dangerous`,
/// and the ack re-dispatch hardcoded `false` — so a backgrounded dangerous
/// command ran FOREGROUND after its ack with nothing reporting the change
/// (`cluster/accepted-parameter-silently-dropped`, IC-15).
///
/// The discriminator is deliberately the ABSENCE of `exit_code` plus a `@bg_`
/// handle, not stdout: a foreground `sleep` also returns stdout — empty — so a
/// stdout assertion is satisfied by the broken behaviour and would not red under
/// the mutation this test exists to catch. Slice 1 made "no `exit_code` key"
/// true by construction for a spawn response, which is what makes it usable here.
#[tokio::test]
async fn ack_re_dispatch_honours_run_in_background() {
    let (_dir, ctx) = project_ctx().await;
    let handle = ctx
        .output_buffer
        // `true` is the load-bearing argument: with `false` this test asserts
        // nothing, because a foreground response is then the correct one.
        .store_dangerous("sleep 30".to_string(), None, 30, true);

    let tool = RunCommand;
    let result = tool
        .call(serde_json::json!({ "command": handle }), &ctx)
        .await
        .expect("ack call should succeed");

    assert!(
        result.get("exit_code").is_none(),
        "a backgrounded job has not exited yet, so the spawn response must carry \
         no exit_code; got: {result}"
    );
    let output_id = result["output_id"].as_str().unwrap_or("");
    assert!(
        output_id.starts_with("@bg_"),
        "expected a @bg_ background handle, got: {output_id:?} in {result}"
    );
}

/// Companion to the above, pinning the OTHER direction so the pair is not
/// monotone: an ack stored with `false` must still re-dispatch foreground.
/// Without this, honouring the flag unconditionally — backgrounding every acked
/// command — would pass the test above and break every other ack caller.
#[tokio::test]
async fn ack_re_dispatch_stays_foreground_when_not_requested() {
    let (_dir, ctx) = project_ctx().await;
    let handle = ctx
        .output_buffer
        .store_dangerous("echo fg_ack".to_string(), None, 30, false);

    let tool = RunCommand;
    let result = tool
        .call(serde_json::json!({ "command": handle }), &ctx)
        .await
        .expect("ack call should succeed");

    assert_eq!(
        result["exit_code"].as_i64(),
        Some(0),
        "a foreground ack must report the command's own exit code; got: {result}"
    );
    assert!(
        result["stdout"].as_str().unwrap_or("").contains("fg_ack"),
        "expected foreground stdout, got: {result}"
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

/// The wine-lane flake's inferred mechanism, pinned instead of assumed.
///
/// `docs/issues/archive/2026-08-26-wine-lane-flakes-under-load-on-three-tests.md` recorded two
/// `run_command` failures whose responses were missing keys that
/// `src/tools/run_command/output.rs` sets in the **same block** as keys that were
/// present. That impossibility is what identified the run, rather than the code, as
/// wrong. The mechanism it inferred — never exercised — was the tee-capture read
/// returning `None`, collapsing `unfiltered_ref` and dropping the whole key group.
///
/// This drives that path directly with an unreadable tee path. Two things are asserted
/// that the bug file could only reason about: the degradation is **total** (no partial
/// key group, which is what made the flake look impossible) and it does **not** panic.
///
/// It also pins the contract the `.ok()` → traced-`match` change preserves. The change
/// added a `tracing::warn!` and nothing else; this test is what proves "nothing else".
#[tokio::test]
async fn an_unreadable_tee_capture_drops_the_whole_key_group_without_panicking() {
    let (_dir, ctx) = project_ctx().await;
    let missing = super::inner::TmpfileGuard(
        "/nonexistent/codescout-unfiltered-this-path-cannot-be-read".to_string(),
    );

    let result = super::output::handle_successful_output(
        "printf hi",
        "hi\n".to_string(),
        String::new(),
        0,
        false,
        Some(missing),
        std::path::Path::new("."),
        &ctx,
    )
    .await
    .expect("an unreadable capture must degrade, not error");

    for key in [
        "unfiltered_output",
        "unfiltered_output_lines",
        "unfiltered_truncated",
        "unfiltered_buffered_lines",
    ] {
        assert!(
            result.get(key).is_none(),
            "an unreadable capture must drop the ENTIRE unfiltered_* group — a partial \
             group is the shape that made the wine flake read as impossible — but {key} \
             survived: {result}"
        );
    }

    assert_eq!(
        result["stdout"], "hi\n",
        "the rest of the response must be unaffected by the capture failure: {result}"
    );
}

/// A filtered workspace run that matched nothing: eight sibling targets, each `running 0
/// tests` + `0 passed … N filtered out`. Load-bearing: it is over 1 KB and almost entirely
/// noise, so it is COMPACTED — which is what makes it the right input for asserting that
/// compaction leaves the raw output and the empty-selection diagnostic intact.
const INLINE_EMPTY_WORKSPACE_STDOUT: &str = "
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4479 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 9 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 11 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.00s


running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 31 filtered out; finished in 0.00s

";

const INLINE_EMPTY_WORKSPACE_STDERR: &str =
    "    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.31s
     Running unittests src/lib.rs (target/debug/deps/codescout-abe48e7f59a7161f)
     Running unittests src/main.rs (target/debug/deps/codescout-8a214943111ff1fb)
     Running tests/audit_doc_refs.rs (target/debug/deps/audit_doc_refs-eaa5462f2843ab20)
     Running tests/bug_regression.rs (target/debug/deps/bug_regression-403c50d5d427ce9f)
     Running tests/cli_artifact.rs (target/debug/deps/cli_artifact-d7fa82706169fa18)
     Running tests/link_scan.rs (target/debug/deps/link_scan-48c68e17171d75f4)
     Running tests/retrieval_unit.rs (target/debug/deps/retrieval_unit-4078ae56642d5ce4)
     Running tests/symbol_lsp.rs (target/debug/deps/symbol_lsp-4c27c61aaa67bd08)
";

async fn run_inline(command: &str, stdout: &str, stderr: &str) -> (serde_json::Value, ToolContext) {
    let (_dir, ctx) = project_ctx().await;
    let result = super::output::handle_successful_output(
        command,
        stdout.to_string(),
        stderr.to_string(),
        0,
        false,
        None,
        std::path::Path::new("."),
        &ctx,
    )
    .await
    .expect("a completed command returns a response");
    (result, ctx)
}

/// The compaction contract: the reader gets the compacted text, and the RAW streams stay
/// byte-for-byte retrievable behind `output_id`. If the store were skipped, the trailer
/// would point at a handle that resolves to nothing.
#[tokio::test]
async fn an_inline_libtest_run_is_compacted_and_its_raw_streams_stay_in_the_buffer() {
    let (result, ctx) = run_inline(
        "cargo test no_such_filter",
        INLINE_EMPTY_WORKSPACE_STDOUT,
        INLINE_EMPTY_WORKSPACE_STDERR,
    )
    .await;

    let id = result["output_id"]
        .as_str()
        .unwrap_or_else(|| panic!("a compacted run must carry output_id; got {result}"));
    let entry = ctx.output_buffer.get(id).expect("the handle must resolve");
    assert_eq!(entry.stdout, INLINE_EMPTY_WORKSPACE_STDOUT);
    assert_eq!(entry.stderr, INLINE_EMPTY_WORKSPACE_STDERR);

    let stdout = result["stdout"].as_str().unwrap();
    assert!(
        !stdout.contains("running 0 tests"),
        "empty targets must be dropped; got {stdout:?}"
    );
    assert!(
        stdout.contains("omitted 8 target(s) that ran no tests (4554 filtered out)"),
        "the trailer must name what was dropped; got {stdout:?}"
    );
    assert!(
        stdout.contains(&format!("read_file(\"{id}\")")),
        "the trailer must name the handle"
    );
    assert_eq!(result["type"], "test");
    assert_eq!(result["passed"], 0);
    // Every stderr line was cargo progress, so nothing is left to show — and the
    // summarizer's raw stderr excerpt must not be reinstated in its place.
    assert!(
        result.get("stderr").is_none(),
        "compacted stderr was empty; got {result}"
    );
}

/// The guard rtk silenced (docs/research/2026-09-24-rtk-evaluation.pdf § 5) must survive
/// codescout's own compaction, because it is computed from the raw output first.
#[tokio::test]
async fn an_empty_selection_is_still_named_after_compaction() {
    let (result, _ctx) = run_inline(
        "cargo test no_such_filter",
        INLINE_EMPTY_WORKSPACE_STDOUT,
        INLINE_EMPTY_WORKSPACE_STDERR,
    )
    .await;
    assert!(
        result["output_id"].is_string(),
        "precondition: this run was compacted"
    );
    assert!(
        result["empty_test_selection"].is_string(),
        "compaction must not silence the empty-selection diagnostic; got {result}"
    );
}

/// Below the 1 KB gate the response is exactly what it was before compaction existed.
#[tokio::test]
async fn a_short_libtest_run_is_returned_raw_as_before() {
    let stdout = "\nrunning 1 test\ntest a::b ... ok\n\ntest result: ok. 1 passed; 0 failed; \
                  0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\n";
    let (result, _ctx) = run_inline("cargo test a::b", stdout, "").await;
    assert!(
        result.get("output_id").is_none(),
        "no buffer for a short run; got {result}"
    );
    assert_eq!(result["stdout"], stdout);
}

/// A non-test command with a libtest-shaped output is not compacted: the gate is the
/// command type AND the content, and this pins the first half.
#[tokio::test]
async fn a_non_test_command_is_never_compacted() {
    let (result, _ctx) = run_inline(
        "cat saved-run.log",
        INLINE_EMPTY_WORKSPACE_STDOUT,
        INLINE_EMPTY_WORKSPACE_STDERR,
    )
    .await;
    assert!(
        result.get("output_id").is_none(),
        "only a test command is compacted; got {result}"
    );
    assert_eq!(result["stdout"], INLINE_EMPTY_WORKSPACE_STDOUT);
}

/// A buffer QUERY never mints a new buffer — that is the invariant the buffer-only arms
/// above exist for (a query answered with a handle would be queried again). A query whose
/// text mentions `cargo test` classifies as a test command, so the compaction arm must
/// exclude it on `buffer_only`, not on command type.
///
/// Built as a CONTROLLED pair: the same command and streams with `buffer_only = false`
/// must compact, which proves every OTHER guard (type, libtest summary, size, saving)
/// admits this input — so the only thing refusing the query is the gate this test names.
/// Two uncontrolled versions of this test each passed with that gate deleted, refused
/// first by the type gate and then by the size gate (mutation SURVIVED twice, 2026-09-24).
#[tokio::test]
async fn a_buffer_query_is_never_compacted_even_when_it_names_cargo_test() {
    // Load-bearing: `detect_command_type` needs `cargo test` delimited by WHITESPACE, so the
    // query names it after a shell comment; a quoted `'cargo test'` classifies Generic.
    const QUERY: &str = "grep -B2 FAILED @cmd_abc12345 # from cargo test --lib";
    let (_dir, ctx) = project_ctx().await;
    let run = |buffer_only: bool| {
        super::output::handle_successful_output(
            QUERY,
            INLINE_EMPTY_WORKSPACE_STDOUT.to_string(),
            // Load-bearing: stdout alone is under the 1 KB size gate.
            INLINE_EMPTY_WORKSPACE_STDERR.to_string(),
            0,
            buffer_only,
            None,
            std::path::Path::new("."),
            &ctx,
        )
    };

    let control = run(false).await.expect("a command returns a response");
    assert!(
        control["output_id"].is_string(),
        "control: as an ordinary command this input must compact, or the case below proves \
         nothing about buffer_only; got {control}"
    );

    let query = run(true).await.expect("a buffer query returns a response");
    assert!(
        query.get("output_id").is_none(),
        "a query must not mint a buffer; got {query}"
    );
}

/// A compacted RED: the failure stays in `stdout` and the counts are right, and the
/// summarizer's `failures` excerpt is NOT added beside it, which would say it twice.
/// Six empty sibling targets make it compactable.
#[tokio::test]
async fn a_compacted_red_keeps_its_failure_once_and_its_counts() {
    let stdout = "
running 2 tests
test a::ok_one ... ok
test a::broken ... FAILED

failures:

---- a::broken stdout ----

thread 'a::broken' (1) panicked at src/a.rs:9:5:
assertion `left == right` failed
  left: 1
 right: 2


failures:
    a::broken

test result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
"
    .to_string()
        + &"
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 7 filtered out; finished in 0.00s

"
        .repeat(6);
    let (result, _ctx) = run_inline("cargo test", &stdout, INLINE_EMPTY_WORKSPACE_STDERR).await;

    assert!(
        result["output_id"].is_string(),
        "precondition: this run was compacted; got {result}"
    );
    assert!(result["stdout"]
        .as_str()
        .unwrap()
        .contains("panicked at src/a.rs:9:5"));
    assert_eq!(result["failed"], 1);
    assert_eq!(result["passed"], 1);
    assert!(
        result.get("failures").is_none(),
        "failure detail must appear once; got {result}"
    );
}

/// The cut marker's remedy must name a call that WORKS, with this run's real handle. Its
/// original text — "Full stderr is NOT in the @cmd_* buffer — buffer reads return stdout
/// only" — was true when written (9c2b542f) and was made false by the `.err` suffix, so it
/// sent every reader away from the full stderr. Load-bearing: stdout over 10 KB (forces the
/// BUFFERED path, where this marker lives) and 30 non-progress stderr lines (over the 20-line
/// tail budget, so the marker is emitted at all).
#[tokio::test]
async fn a_cut_stderr_tail_names_the_err_handle_that_holds_the_rest() {
    let mut stdout = String::from("\nrunning 400 tests\n");
    for i in 0..400 {
        stdout.push_str(&format!("test tools::tests::case_{i:03} ... ok\n"));
    }
    stdout.push_str(
        "\ntest result: ok. 400 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s\n",
    );
    let stderr: String = (1..=30)
        .map(|i| format!("note: diagnostic line {i}\n"))
        .collect();

    let (result, ctx) = run_inline("cargo test", &stdout, &stderr).await;
    let id = result["output_id"]
        .as_str()
        .unwrap_or_else(|| panic!("precondition: this run was buffered; got {result}"));
    let rendered = result["stderr"].as_str().expect("a cut tail is rendered");

    assert!(
        rendered.contains(&format!("read_file(\"{id}.err\")")),
        "the marker must name this run's .err handle; got {rendered:?}"
    );
    assert!(
        !rendered.contains("NOT in the @cmd"),
        "the false claim must be gone; got {rendered:?}"
    );
    // And the handle it names really holds the rest.
    assert_eq!(
        ctx.output_buffer.get(&format!("{id}.err")).unwrap().stderr,
        stderr
    );
}

/// Regression for docs/issues/archive/2026-08-26-unfiltered-output-ref-carries-no-size-signal.md:
/// when the filter matched nothing, the response used to omit `stdout` entirely (absent,
/// not `""`) and attach a bare `unfiltered_output` ref with no size signal — an agent
/// could not tell a 2-line buffer from a 20,000-line one without a blind round-trip.
#[tokio::test]
async fn unfiltered_output_carries_a_line_count_and_explicit_empty_stdout() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "printf 'a\\nb\\nc\\n' | grep zzz" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        result["stdout"], "",
        "stdout must be explicitly \"\", not absent, when the filter matched nothing: {result}"
    );
    assert!(
        result.get("unfiltered_output").is_some(),
        "expected an unfiltered_output ref: {result}"
    );
    assert_eq!(
        result["unfiltered_output_lines"], 3,
        "expected the unfiltered capture's line count (3), not silence: {result}"
    );
}
// ---- Bug bc0cb248b224d1dd: a credential in command output must not reach the model -------------------
//
// Every secret below is PRODUCED by `printf 'ghp_%036d' 0`, so the command text -- which the response
// may echo, and which is what a transcript records -- never contains a token-shaped literal. The
// tell that redaction did not run is `ghp_0000`, forty characters of zeros after the prefix.

/// The 40-character token the `printf` commands below print, spelled out for the assertions.
#[cfg(unix)]
fn printed_token() -> String {
    format!("ghp_{}", "0".repeat(36))
}

#[cfg(unix)]
#[tokio::test]
async fn a_credential_in_stdout_is_redacted_and_the_response_says_so() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "printf 'HOME=/x\\nGITHUB_TOKEN=ghp_%036d\\nPATH=/bin\\n' 0" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        !result.to_string().contains(&printed_token()),
        "the token reached the response: {result}"
    );
    let stdout = result["stdout"].as_str().expect("stdout");
    assert!(
        stdout.contains("GITHUB_TOKEN=<redacted-credential>"),
        "{stdout}"
    );
    assert!(
        stdout.contains("HOME=/x") && stdout.contains("PATH=/bin"),
        "only the value is lost, not the lines around it: {stdout}"
    );
    assert_eq!(result["redacted_credentials"], 1, "{result}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_credential_on_stderr_is_redacted_too() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "printf 'ghp_%036d' 0 >&2" }), &ctx)
        .await
        .unwrap();
    assert!(!result.to_string().contains(&printed_token()), "{result}");
    assert!(
        result["stderr"]
            .as_str()
            .is_some_and(|s| s.contains("<redacted-credential>")),
        "{result}"
    );
    assert_eq!(result["redacted_credentials"], 1, "{result}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_credential_in_output_large_enough_to_buffer_is_redacted_in_the_buffer() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "{ seq 1 3000; printf 'ghp_%036d\\n' 0; seq 1 10; }" }),
            &ctx,
        )
        .await
        .unwrap();
    let output_id = result["output_id"]
        .as_str()
        .expect("~14KB of output is buffered");
    let stored = ctx.output_buffer.get(output_id).unwrap().stdout;
    assert!(
        stored.contains("<redacted-credential>"),
        "the buffer must hold the marker"
    );
    assert!(
        !stored.contains(&printed_token()),
        "a later `grep @cmd_*` must not be able to surface the token"
    );
    assert!(!result.to_string().contains(&printed_token()), "{result}");
    assert_eq!(result["redacted_credentials"], 1, "{result}");
}

/// The tee capture is the UNFILTERED stream: `env | grep PATH` shows one line inline while the buffer
/// behind `unfiltered_output` holds the whole environment. It is read from a temp file, so it never
/// passes the decode that scrubs `stdout`, and it needs its own scrub.
#[cfg(unix)]
#[tokio::test]
async fn a_credential_only_in_the_unfiltered_capture_is_redacted_there() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "printf 'ghp_%036d\\nb\\n' 0 | grep zzz" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        result["stdout"], "",
        "the filter hides the token inline: {result}"
    );
    let handle = result["unfiltered_output"].as_str().expect("tee ref");
    let stored = ctx.output_buffer.get(handle).unwrap().stdout;
    assert!(stored.contains("<redacted-credential>"), "{stored}");
    assert!(
        !stored.contains(&printed_token()),
        "the unfiltered buffer held the raw environment"
    );
    assert_eq!(result["redacted_credentials"], 1, "{result}");
}

#[cfg(unix)]
#[tokio::test]
async fn the_counts_from_the_inline_output_and_the_capture_add() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "printf 'ghp_%036d\\n' 0 | grep ghp" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.to_string().contains(&printed_token()), "{result}");
    assert_eq!(
        result["redacted_credentials"], 2,
        "one value inline plus the same value in the unfiltered capture: {result}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn output_with_no_credential_carries_no_redaction_key() {
    let (_dir, ctx) = project_ctx().await;
    for command in ["printf 'hello\\n'", "printf 'a\\nb\\n' | grep zzz"] {
        let result = RunCommand
            .call(json!({ "command": command }), &ctx)
            .await
            .unwrap();
        assert!(
            result.get("redacted_credentials").is_none(),
            "{command}: an unedited response must not claim an edit: {result}"
        );
        assert!(
            !result.to_string().contains("<redacted-credential>"),
            "{command}: {result}"
        );
    }
}

/// A `@bg_*` handle substitutes into a shell command as a file name, so a background job's log is
/// only ever read through `run_command`. That is why the foreground decode covers it, and this test
/// is the evidence rather than the assumption. The log FILE on disk still holds the raw value; the
/// last assertion says so, and would fail if that ever changed without this comment changing.
#[cfg(unix)]
#[tokio::test]
async fn a_background_log_is_redacted_when_it_is_read_back() {
    let (_dir, ctx) = project_ctx().await;
    let started = RunCommand
        .call(
            json!({ "command": "printf 'ghp_%036d\\n' 0", "run_in_background": true }),
            &ctx,
        )
        .await
        .unwrap();
    let handle = started["output_id"]
        .as_str()
        .expect("@bg handle")
        .to_string();
    let log = ctx
        .output_buffer
        .get_background(&handle)
        .expect("job")
        .log_path;
    let mut raw = String::new();
    for _ in 0..100 {
        raw = std::fs::read_to_string(&log).unwrap_or_default();
        if !raw.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        raw.contains(&printed_token()),
        "the raw log is not scrubbed on disk: {raw:?}"
    );
    let read = RunCommand
        .call(json!({ "command": format!("cat {handle}") }), &ctx)
        .await
        .unwrap();
    assert!(!read.to_string().contains(&printed_token()), "{read}");
    assert!(
        read["stdout"]
            .as_str()
            .is_some_and(|s| s.contains("<redacted-credential>")),
        "{read}"
    );
}

#[test]
fn the_interactive_result_is_redacted_once_on_the_whole_accumulated_output() {
    // a token that arrived in two reads: neither half is a token, the concatenation is
    let token = format!("ghp_{}", "0".repeat(36));
    let (first, second) = token.split_at(20);
    let mut accumulated = String::from("login: ");
    accumulated.push_str(first);
    accumulated.push_str(second);
    for (code, note) in [(0, None), (-1, Some("killed"))] {
        let result = super::interactive::interactive_response(code, &accumulated, 3, note);
        assert!(!result.to_string().contains(&token), "{result}");
        assert_eq!(result["stdout"], "login: <redacted-credential>", "{result}");
        assert_eq!(result["redacted_credentials"], 1, "{result}");
        assert_eq!(result["interactive_rounds"], 3);
        assert_eq!(result["exit_code"], code);
        assert_eq!(result.get("note").and_then(|n| n.as_str()), note);
    }
    let clean = super::interactive::interactive_response(0, "hello", 1, None);
    assert!(clean.get("redacted_credentials").is_none(), "{clean}");
}
#[test]
fn the_compact_summary_names_a_redaction_and_stays_silent_without_one() {
    use super::output::format_run_command;
    let edited = json!({ "exit_code": 0, "stdout": "a\n", "redacted_credentials": 1 });
    let plural = json!({ "exit_code": 0, "stdout": "a\n", "redacted_credentials": 3 });
    let clean = json!({ "exit_code": 0, "stdout": "a\n" });
    assert!(
        format_run_command(&edited).contains("1 credential-shaped value redacted"),
        "{}",
        format_run_command(&edited)
    );
    assert!(
        format_run_command(&plural).contains("3 credential-shaped values redacted"),
        "{}",
        format_run_command(&plural)
    );
    assert!(
        !format_run_command(&clean).contains("redacted"),
        "{}",
        format_run_command(&clean)
    );
    // the buffered shape too: the notice is unconditional across output shapes
    let buffered = json!({ "output_id": "@cmd_abc", "exit_code": 0, "redacted_credentials": 2 });
    assert!(format_run_command(&buffered).contains("2 credential-shaped values redacted"));
}

/// The line count must reflect the FULL unfiltered capture, not the (possibly
/// truncated-for-inline-storage) stored copy.
#[tokio::test]
async fn unfiltered_output_line_count_survives_inline_truncation() {
    let (_dir, ctx) = project_ctx().await;
    // Enough lines to exceed the inline-storage cap (MAX_INLINE_TOKENS * 4 bytes),
    // so the stored copy is truncated but the reported count must still be the full
    // pre-truncation line count.
    let line_count = 20_000;
    let result = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.get("unfiltered_truncated").is_some(),
        "fixture must actually exceed the inline cap to exercise truncation: {result}"
    );
    assert_eq!(
        result["unfiltered_output_lines"], line_count,
        "line count must be the full pre-truncation total, not the truncated-for-storage count: {result}"
    );
}

/// Option A of docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md:
/// `unfiltered_output_lines` describes the STREAM, and sat next to a handle serving a
/// truncated buffer with no field anywhere naming the served count. Learning it cost a
/// `wc -l` round-trip — the exact blind round-trip the parent fix set out to remove.
#[tokio::test]
async fn a_truncated_buffer_reports_the_count_it_will_actually_serve() {
    let (_dir, ctx) = project_ctx().await;
    let line_count = 20_000;
    let result = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    // Asserted FIRST: on a fixture too small to truncate every assertion below
    // passes vacuously, which is how a green here would mean nothing.
    assert!(
        result.get("unfiltered_truncated").is_some(),
        "fixture must actually exceed the inline cap: {result}"
    );
    let served = result["unfiltered_buffered_lines"]
        .as_u64()
        .unwrap_or_else(|| panic!("no unfiltered_buffered_lines: {result}"));
    assert!(
        served < line_count,
        "the served count must be SMALLER than the stream count, or the two fields \
         describe the same thing and the bug is unfixed: served={served}"
    );

    // And it must match the buffer, not merely be some smaller number.
    let handle = result["unfiltered_output"].as_str().expect("handle");
    let stored = ctx.output_buffer.get(handle).expect("entry").stdout;
    let stored_lines = stored.lines().count() as u64;
    assert_eq!(
        stored_lines,
        served + 1,
        "the buffer should hold exactly the served lines plus the one sentinel line"
    );
}

/// Option B: the warning travels WITH the data, so a reader who never looks at the
/// response still meets it. `tail` shows it, `wc -l` counts it, any slice near the end
/// hits it.
#[tokio::test]
async fn a_truncated_buffer_ends_with_a_sentinel_naming_both_counts() {
    let (_dir, ctx) = project_ctx().await;
    let line_count = 20_000;
    let result = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.get("unfiltered_truncated").is_some(),
        "fixture must actually truncate: {result}"
    );
    let served = result["unfiltered_buffered_lines"]
        .as_u64()
        .expect("served");
    let handle = result["unfiltered_output"].as_str().expect("handle");
    let stored = ctx.output_buffer.get(handle).expect("entry").stdout;

    let last = stored.lines().next_back().expect("a last line");
    assert!(
        last.starts_with(crate::tools::output_buffer::TRUNCATION_SENTINEL_PREFIX),
        "the sentinel must be the FINAL line, where tail -1 lands: {last:?}"
    );
    assert!(
        last.contains(&served.to_string()) && last.contains(&line_count.to_string()),
        "the sentinel must name both counts, so it cannot drift from the fields: {last:?}"
    );
}

/// Option C, and the load-bearing test: the only one that would have failed for the
/// reason the bug was actually reported.
///
/// The incident was a `grep` over a truncated buffer returning nothing, read as
/// "absent". A sentinel does not help there — grep prints matches, and a count of `0`
/// is byte-identical whether the tail is missing or the value genuinely does not occur.
/// The notice has to ride on the reading tool's own result.
#[tokio::test]
async fn a_grep_over_a_truncated_ref_carries_the_truncation_notice() {
    let (_dir, ctx) = project_ctx().await;
    let line_count = 20_000;
    let first = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        first.get("unfiltered_truncated").is_some(),
        "fixture must actually truncate: {first}"
    );
    let handle = first["unfiltered_output"]
        .as_str()
        .expect("handle")
        .to_string();

    // A value near the END of the capture: present in the stream, absent from the
    // stored prefix. Without the notice this returns `0` and says nothing else.
    let second = RunCommand
        .call(
            json!({ "command": format!("grep -c '^19999$' {handle}") }),
            &ctx,
        )
        .await
        .unwrap();
    let notices = second["buffer_truncated"]
        .as_array()
        .unwrap_or_else(|| panic!("a read of a truncated ref must carry a notice: {second}"));
    assert_eq!(notices.len(), 1, "one notice per distinct handle: {second}");
    let text = notices[0].as_str().expect("notice text");
    assert!(
        text.contains(&handle),
        "the notice must name WHICH handle, since a command may read several: {text}"
    );
    assert!(
        text.contains(&line_count.to_string()),
        "the notice must name the true total: {text}"
    );
}

/// Control for all three above. A complete buffer must gain neither a sentinel nor a
/// notice — a warning that fires unconditionally is one a reader learns to skip, and it
/// would corrupt the data of every non-truncated buffer besides.
#[tokio::test]
async fn a_complete_buffer_carries_no_sentinel_and_no_notice() {
    let (_dir, ctx) = project_ctx().await;
    let first = RunCommand
        .call(
            json!({ "command": "printf 'a\\nb\\nc\\n' | grep zzz" }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        first.get("unfiltered_truncated").is_none(),
        "fixture must NOT truncate, or this control proves nothing: {first}"
    );
    assert!(
        first.get("unfiltered_buffered_lines").is_none(),
        "a complete buffer has nothing to distinguish from its stream: {first}"
    );
    let handle = first["unfiltered_output"]
        .as_str()
        .expect("handle")
        .to_string();
    let stored = ctx.output_buffer.get(&handle).expect("entry").stdout;
    assert!(
        !stored.contains(crate::tools::output_buffer::TRUNCATION_SENTINEL_PREFIX),
        "a complete buffer must not have synthetic content injected: {stored:?}"
    );

    let second = RunCommand
        .call(json!({ "command": format!("grep -c a {handle}") }), &ctx)
        .await
        .unwrap();
    assert!(
        second.get("buffer_truncated").is_none(),
        "no notice may fire for a complete buffer: {second}"
    );
}

/// The `read_file` half of Option C, pinned on the RENDERED surface.
///
/// `run_command` returns its JSON straight through, so asserting on the `Value` is
/// the whole story there. `read_file` does not: `format_read_file` builds a text
/// string from a fixed set of keys and drops every other field, so a
/// `buffer_truncated` sitting correctly in the JSON reached no reader at all. That
/// is how the first cut of this fix shipped an inert field — caught by a live probe,
/// not by a test, because no test looked at this surface.
///
/// Asserting on `format_read_file` rather than on `.call()`'s Value is therefore the
/// point of the test, not an implementation detail of it.
///
/// BUG docs/issues/archive/2026-08-27-unfiltered-output-lines-counts-the-source-not-the-buffer.md
#[tokio::test]
async fn read_file_renders_the_truncation_notice_not_just_carries_it() {
    let (_dir, ctx) = project_ctx().await;
    let line_count = 20_000;
    let first = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        first.get("unfiltered_truncated").is_some(),
        "fixture must actually truncate: {first}"
    );
    let handle = first["unfiltered_output"]
        .as_str()
        .expect("handle")
        .to_string();

    let res = crate::tools::read_file::ReadFile
        .call(
            json!({ "path": handle, "start_line": 1, "end_line": 2 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        res.get("buffer_truncated").is_some(),
        "the field must be on the result: {res}"
    );

    let rendered = crate::tools::read_file::format_read_file(&res);
    // Second line exactly: below the header a reader anchors on, above content that
    // may be cut. This is `insert_below_header`'s contract, and asserting the position
    // rather than mere presence is what keeps a future "just append it" refactor from
    // silently recreating the defect on large reads.
    let second_line = rendered.lines().nth(1).unwrap_or("");
    assert!(
        second_line.contains(crate::tools::output_buffer::TRUNCATION_SENTINEL_PREFIX),
        "the notice must be the rendered second line, or it reaches nobody. \
         got: {second_line:?}\nfull:\n{rendered}"
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

    // Instructions must mention read_file (Task 7: read_markdown was folded into it).
    let instructions = parsed["instructions"].as_str().unwrap_or("");
    assert!(
        instructions.contains("read_file"),
        "instructions must reference read_file"
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
            // No recorded baseline, which is NOT read as a witnessed regeneration:
            // without one there is no evidence either way, so the version decides.
            system_prompt_sha256: None,
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
/// Seed a fully-onboarded temp project. `version` and `baseline` seed the two fields
/// the system-prompt witness reads; `prompt` seeds `.codescout/system-prompt.md`
/// (`None` leaves it absent). Kept separate from `ctx_over` so a test can re-enter the
/// same project without overwriting what the previous call recorded — the whole point
/// of the witness is what survives between two calls.
fn seed_onboarded_project(
    dir: &std::path::Path,
    version: Option<u32>,
    baseline: Option<String>,
    prompt: Option<&str>,
) {
    let config_dir = dir.join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(dir.join("main.rs"), "fn main() {}").unwrap();

    let config = crate::config::project::ProjectConfig {
        project: crate::config::project::ProjectSection {
            name: "test".into(),
            languages: vec!["rust".into()],
            encoding: "utf-8".into(),
            system_prompt: None,
            tool_timeout_secs: 60,
            onboarding_version: version,
            system_prompt_sha256: baseline,
        },
        embeddings: Default::default(),
        ignored_paths: Default::default(),
        security: Default::default(),
        memory: Default::default(),
        libraries: Default::default(),
        lsp: Default::default(),
    };
    std::fs::write(
        config_dir.join("project.toml"),
        toml::to_string_pretty(&config).unwrap(),
    )
    .unwrap();

    if let Some(body) = prompt {
        std::fs::write(config_dir.join("system-prompt.md"), body).unwrap();
    }

    let mem_dir = config_dir.join("memories");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("onboarding.md"), "Languages: rust").unwrap();
}

async fn ctx_over(dir: &std::path::Path) -> ToolContext {
    let agent = Agent::new(Some(dir.to_path_buf())).await.unwrap();
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

/// Read the two witness fields back off disk, bypassing any in-memory config — the
/// defect being guarded is what a *later session* finds on disk.
fn stored_witness(dir: &std::path::Path) -> (Option<u32>, Option<String>) {
    let config = crate::config::project::ProjectConfig::load_or_default(dir).unwrap();
    (
        config.project.onboarding_version,
        config.project.system_prompt_sha256,
    )
}
/// Simulate the one step `onboarding()` defers: a subagent writing the system prompt.
///
/// Every "the second call returns status" test needs this, and none of them used to.
/// A fresh config was stamped as current at creation, so those tests reported a fully
/// onboarded project while `.codescout/system-prompt.md` had never been written — the
/// certification-without-the-artifact defect, sitting inside the fixtures that were
/// supposed to describe a completed onboarding. The stamp is now witnessed against the
/// file, so the fixture has to produce what the real flow produces.
fn simulate_subagent_prompt_write(dir: &std::path::Path) {
    let config_dir = dir.join(".codescout");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("system-prompt.md"),
        "# Test project system prompt\n",
    )
    .unwrap();
}

#[tokio::test]
async fn onboarding_refresh_prompt_records_a_baseline_without_stamping_the_version() {
    // Site 1 of four: `handle_refresh_prompt`. Its response only *instructs* a
    // subagent, so a version stamped here certifies work that has not happened.
    let dir = tempdir().unwrap();
    seed_onboarded_project(
        dir.path(),
        Some(ONBOARDING_VERSION - 1),
        None,
        Some("v-old prompt"),
    );
    let ctx = ctx_over(dir.path()).await;

    let result = Onboarding
        .call(json!({ "refresh_prompt": true }), &ctx)
        .await
        .unwrap();
    assert!(
        result.get("subagent_prompt").is_some(),
        "the regeneration is deferred to a subagent, which is why the stamp cannot precede it"
    );

    let (version, baseline) = stored_witness(dir.path());
    assert_eq!(
        version,
        Some(ONBOARDING_VERSION - 1),
        "the version must NOT be stamped before the regeneration runs"
    );
    assert!(
        baseline.is_some(),
        "the baseline the regeneration must supersede has to be recorded, or the \
         completion can never be witnessed"
    );
}

#[tokio::test]
async fn onboarding_stale_version_records_a_baseline_without_stamping_the_version() {
    // Site 2 of four: `handle_already_onboarded`, reached by a bare call. Its write
    // was commented "prevents re-trigger across sessions" — which is precisely the
    // signal that had to survive.
    let dir = tempdir().unwrap();
    seed_onboarded_project(
        dir.path(),
        Some(ONBOARDING_VERSION - 1),
        None,
        Some("v-old prompt"),
    );
    let ctx = ctx_over(dir.path()).await;

    let result = Onboarding.call(json!({}), &ctx).await.unwrap();
    assert_eq!(result["version_stale"].as_bool(), Some(true));

    let (version, baseline) = stored_witness(dir.path());
    assert_eq!(
        version,
        Some(ONBOARDING_VERSION - 1),
        "the version must NOT be stamped before the regeneration runs"
    );
    assert!(baseline.is_some(), "the baseline has to be recorded");
}

#[tokio::test]
async fn onboarding_force_records_a_baseline_without_stamping_the_version() {
    // Site 3 of four: `perform_full_onboarding`'s tail, whose own comment called the
    // write "optimistic". Full onboarding also returns a `subagent_prompt`, so the
    // prompt file is no more written on this path than on the lightweight one — the
    // site a fix aimed only at "refresh" would have left writing the same falsehood.
    let dir = tempdir().unwrap();
    seed_onboarded_project(
        dir.path(),
        Some(ONBOARDING_VERSION - 1),
        None,
        Some("v-old prompt"),
    );
    let ctx = ctx_over(dir.path()).await;

    let result = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert!(
        result.get("subagent_prompt").is_some(),
        "full onboarding defers the prompt write too"
    );

    let (version, baseline) = stored_witness(dir.path());
    assert_eq!(
        version,
        Some(ONBOARDING_VERSION - 1),
        "force must not stamp the version before the regeneration runs either"
    );
    assert!(baseline.is_some(), "the baseline has to be recorded");
}

#[tokio::test]
async fn onboarding_force_reports_that_it_subsumed_refresh_prompt() {
    // The sibling defect: both flags accepted, `force` honoured, `refresh_prompt`
    // silently dropped. `force` genuinely is a superset, so the fix is to say so
    // rather than to refuse — and to say it on the compact line the caller reads,
    // not only in a JSON field nothing renders.
    let dir = tempdir().unwrap();
    seed_onboarded_project(dir.path(), Some(ONBOARDING_VERSION), None, Some("prompt"));
    let ctx = ctx_over(dir.path()).await;

    let result = Onboarding
        .call(json!({ "force": true, "refresh_prompt": true }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["refresh_prompt_subsumed"].as_bool(),
        Some(true),
        "the substitution must be reported, not silent"
    );
    let compact = Onboarding.format_compact(&result).unwrap_or_default();
    assert!(
        compact.contains("refresh_prompt subsumed"),
        "and must reach the compact surface, not only the JSON: got {compact:?}"
    );

    // Without the flag, no note — otherwise the field says nothing.
    let plain = Onboarding
        .call(json!({ "force": true }), &ctx)
        .await
        .unwrap();
    assert!(
        plain.get("refresh_prompt_subsumed").is_none(),
        "a note that is always present distinguishes nothing"
    );
}

#[tokio::test]
async fn onboarding_stamps_the_version_once_the_prompt_content_has_moved() {
    // The whole loop the fix exists for. A refresh is requested (baseline recorded,
    // version untouched), a subagent then rewrites the prompt, and the next call
    // witnesses the change and stamps. No production code path writes
    // `.codescout/system-prompt.md`, so the content moving is the only evidence
    // available that the deferred work ever happened.
    let dir = tempdir().unwrap();
    seed_onboarded_project(
        dir.path(),
        Some(ONBOARDING_VERSION - 1),
        None,
        Some("v-old prompt"),
    );
    let ctx = ctx_over(dir.path()).await;

    Onboarding
        .call(json!({ "refresh_prompt": true }), &ctx)
        .await
        .unwrap();
    let (version_after_request, baseline) = stored_witness(dir.path());
    assert_eq!(
        version_after_request,
        Some(ONBOARDING_VERSION - 1),
        "still behind — nothing has regenerated yet"
    );

    // The subagent does its job.
    std::fs::write(
        dir.path().join(".codescout").join("system-prompt.md"),
        "v-new prompt",
    )
    .unwrap();

    // Re-enter over the same project, without reseeding what the first call recorded.
    let ctx2 = ctx_over(dir.path()).await;
    let result = Onboarding.call(json!({}), &ctx2).await.unwrap();
    assert!(
        result.get("subagent_prompt").is_none(),
        "a witnessed regeneration must not ask for another one"
    );

    let (version, new_baseline) = stored_witness(dir.path());
    assert_eq!(
        version,
        Some(ONBOARDING_VERSION),
        "the witness must stamp the version it can now certify"
    );
    assert_ne!(
        new_baseline, baseline,
        "and re-record the content that earned it — leaving the old baseline would keep \
         vouching for this regeneration after the next ONBOARDING_VERSION bump"
    );
}

#[test]
fn tee_path_is_safe_accepts_real_platform_temp_paths() {
    use super::inner::tee_path_is_safe;
    // POSIX.
    assert!(tee_path_is_safe("/tmp/codescout-unfiltered-aB3xY9"));
    // Windows, long name — needs `:` for the drive letter.
    assert!(tee_path_is_safe(
        "C:/Users/someone/AppData/Local/Temp/codescout-unfiltered-aB3xY9"
    ));
    // Windows, 8.3 short name — needs `~`. This is the exact shape that
    // reached the shell on the dev VDI and was rejected: `%TEMP%` resolves
    // through the short name whenever the account name is long or dotted.
    assert!(tee_path_is_safe(
        "C:/Users/MAILIN~1.002/AppData/Local/Temp/codescout-unfiltered-g44yCk"
    ));
}

#[test]
fn tee_path_is_safe_rejects_shell_metacharacters() {
    use super::inner::tee_path_is_safe;
    // The interpolated path is single-quoted at the call site, so a `'` would
    // be the one character that could break out — it must never pass.
    // `'` ISOLATED. The composite fixture below carries `;` and a space too,
    // so it stays green if `'` is admitted to the allowlist — it cannot pin
    // the one character that matters. The tee path is single-quoted at the
    // call site in `inject_tee`, and that quoting is unescapable ONLY while
    // `'` is excluded here; this assertion is the whole reason that holds.
    assert!(!tee_path_is_safe("/tmp/x'y"));
    // `\` likewise: bash reads it as an escape in an unquoted word, and it is
    // a legal filename byte on Unix.
    assert!(!tee_path_is_safe("/tmp/x\\y"));
    assert!(!tee_path_is_safe("/tmp/x'; rm -rf /; echo '"));
    assert!(!tee_path_is_safe("/tmp/x;y"));
    assert!(!tee_path_is_safe("/tmp/x$(id)"));
    assert!(!tee_path_is_safe("/tmp/x`id`"));
    assert!(!tee_path_is_safe("/tmp/x y"));
    assert!(!tee_path_is_safe("/tmp/x|y"));
    assert!(!tee_path_is_safe("/tmp/x>y"));
    assert!(!tee_path_is_safe(""));
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
    //
    // The poll must NOT swallow its error arm. "still flushing" and "never ran at all"
    // (no `py` on PATH, launcher failure) both surface as a loop that ends with
    // found == false, so dropping the Err made the CI failure say only "not captured"
    // — which is what left WIN-30's MSVC red undiagnosable. Keep the last stdout and
    // the last error so the next failure names its own cause.
    let mut found = false;
    let mut last_stdout = String::new();
    let mut last_err = String::new();
    let mut errors = 0usize;
    for _ in 0..150 {
        let out = RunCommand
            .call(
                json!({ "command": format!("cat {ref_id}"), "timeout_secs": 10 }),
                &ctx,
            )
            .await;
        match out {
            Ok(v) => {
                last_stdout = v["stdout"].as_str().unwrap_or("").to_string();
                if last_stdout.contains("bg-ok 4") {
                    found = true;
                    break;
                }
            }
            Err(e) => {
                errors += 1;
                last_err = e.to_string();
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(
        found,
        "background command output not captured within 15s \
             (read errors: {errors}); last stdout: {last_stdout:?}; last error: {last_err:?}"
    );
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
            // A current version short-circuits the witness entirely, so the baseline
            // is irrelevant here — `None` keeps that explicit rather than incidental.
            system_prompt_sha256: None,
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

// ---------------------------------------------------------------------------------
// WIP-author attribution: the field must survive the ARM it is attached in.
//
// `handle_successful_output` has two attachment sites for every diagnostic — one inside
// the buffer-only arm, which returns early, and one at the bottom of the function. They
// are separate lines implementing one rule, so a mutation run against either says nothing
// about the other (`CLAUDE.md` § Testing Discipline, "mutate once per guarded SITE").
// Both survived a mutation deleting them until these two tests existed: every other test
// of this feature calls `wip_author_diagnostic` or `format_run_command` directly, which is
// the un-wired-function shape — a full green suite over code no caller reaches.
// ---------------------------------------------------------------------------------

/// A throwaway git repo holding one file that is dirty relative to HEAD.
///
/// No transcript exists for a tempdir minted seconds ago under any profile, so the engine
/// reaches stage 3 and reports the file as a COVERAGE gap. That is a real end-to-end run
/// — materialize, spawn, git, engine — with no environment variable involved.
fn dirty_git_fixture() -> Option<tempfile::TempDir> {
    let dir = tempfile::tempdir().ok()?;
    let p = dir.path();
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(p)
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
    };
    git(&["init", "-q"])?;
    git(&["config", "user.email", "t@t"])?;
    git(&["config", "user.name", "t"])?;
    std::fs::create_dir_all(p.join("src")).ok()?;
    std::fs::write(p.join("src/held.rs"), "fn main() {}\n").ok()?;
    git(&["add", "-A"])?;
    git(&["commit", "-q", "-m", "seed"])?;
    std::fs::write(p.join("src/held.rs"), "fn main() { broken\n").ok()?;
    Some(dir)
}

const HELD_RED: &str = "error[E0425]: cannot find value `broken`\n  --> src/held.rs:1:13\n";

#[tokio::test]
async fn a_red_attaches_wip_authors_on_the_main_arm() {
    let Some(dir) = dirty_git_fixture() else {
        eprintln!("skipping: git unavailable");
        return;
    };
    let ctx = project_ctx_at(dir.path()).await;
    // Establish that the engine can answer AT ALL here, as its own observation. Folding
    // this into the assertion below -- "no field? maybe python3 is missing, skip" -- is
    // satisfied by precisely the state a deleted attachment produces, and both of these
    // cases survived a mutation deleting their site until the two were separated.
    if super::attribution::wip_author_diagnostic(101, HELD_RED, dir.path())
        .await
        .is_none()
    {
        eprintln!("skipping: engine produced nothing here (python3 absent?)");
        return;
    }

    let result = super::output::handle_successful_output(
        "cargo check",
        String::new(),
        HELD_RED.to_string(),
        101,
        false,
        None,
        dir.path(),
        &ctx,
    )
    .await
    .expect("a failing command still returns a response");

    let who = result["wip_authors"]
        .as_str()
        .expect("the engine answers in this environment, so the main arm must attach it");
    assert!(
        who.contains("src/held.rs"),
        "the attached hint must name the dirty file; got: {who}"
    );
}

#[tokio::test]
async fn a_red_attaches_wip_authors_on_the_buffer_only_arm() {
    let Some(dir) = dirty_git_fixture() else {
        eprintln!("skipping: git unavailable");
        return;
    };
    let ctx = project_ctx_at(dir.path()).await;
    // Establish that the engine can answer AT ALL here, as its own observation. Folding
    // this into the assertion below -- "no field? maybe python3 is missing, skip" -- is
    // satisfied by precisely the state a deleted attachment produces, and both of these
    // cases survived a mutation deleting their site until the two were separated.
    if super::attribution::wip_author_diagnostic(101, HELD_RED, dir.path())
        .await
        .is_none()
    {
        eprintln!("skipping: engine produced nothing here (python3 absent?)");
        return;
    }

    // The buffer-only arm is reached only when `needs_summary` is true, i.e. combined
    // output over 4 * MAX_INLINE_TOKENS bytes. The padding is load-bearing: shrink it and
    // this test silently re-tests the main arm, which the case above already covers, and
    // the early-return attachment goes back to being unguarded.
    let padded = format!("{HELD_RED}{}", "note: filler\n".repeat(2_000));
    let result = super::output::handle_successful_output(
        "cargo check",
        padded,
        String::new(),
        101,
        true,
        None,
        dir.path(),
        &ctx,
    )
    .await
    .expect("a failing command still returns a response");

    let who = result["wip_authors"]
        .as_str()
        .expect("the engine answers in this environment, so the buffer-only arm must attach it");
    assert!(
        who.contains("src/held.rs"),
        "the early-return arm must attach the hint too; got: {who}"
    );
}

/// A backgrounded, STILL-RUNNING result carries no `exit_code`. The compact renderer must not
/// turn that absence into a success claim.
///
/// **The fixture below is the HISTORICAL payload shape, kept deliberately.** Production has
/// emitted `output_id` + `hint` only since 2026-09-15 (no warm-window `stdout`), so the
/// `stdout` key here is inert for these three assertions — the `None` arm never reads it. It
/// is retained because it is the defect verbatim: the buffer held a compile failure while the
/// summary claimed success. Do not "tidy" it to match the current shape and do not credit it
/// with covering one; the renderer must stay correct for both, and the shape that can still
/// reach this arm is the smaller one.
///
/// **Asserting `✓` versus `✗` cannot catch this, which is how it shipped.** Both checkmarks
/// describe a *completed* run, so a third state rendered as the first is invisible on that
/// axis however many completed cases are added. One predicate (`output_id.is_string()`) was
/// separating three states — passed, failed, still-running — and collapsed the third onto the
/// first. The assertion that bites is that the still-running shape produces NEITHER checkmark
/// and names no exit status at all.
///
/// docs/issues/archive/2026-09-14-a-backgrounded-gate-command-was-summarised-as-exit-0-while-its-buffer-held-the-failure.md
#[test]
fn a_still_running_background_result_never_asserts_an_exit_status() {
    let rendered = super::output::format_run_command(&serde_json::json!({
        "output_id": "@bg_00000001",
        "hint": "Process running. Output captured in @bg_00000001",
        "stdout": "error: could not compile `codescout` (lib) due to 1 previous error",
    }));

    assert!(
        !rendered.contains('✓'),
        "an absent exit status must not render as success — this is the defect verbatim, a \
         failed gate reported as a pass: {rendered}"
    );
    assert!(
        !rendered.contains('✗'),
        "nor as failure: the state is UNKNOWN, and guessing the other direction is the same \
         defect mirrored rather than fixed: {rendered}"
    );
    assert!(
        !rendered.contains("exit "),
        "a payload carrying no exit_code must not name one: {rendered}"
    );
}

/// The sibling of the test above, and the reason it is not redundant: a COMPLETED buffered
/// result does carry `exit_code`, and must still render its status. A fix that silenced the
/// status for every `output_id`-bearing shape would satisfy the still-running assertions and
/// destroy the reporting this function exists for.
#[test]
fn a_completed_buffered_result_still_reports_its_exit_status() {
    let rendered = super::output::format_run_command(&serde_json::json!({
        "output_id": "@cmd_00000001",
        "exit_code": 101,
        "stdout": "boom",
    }));

    assert!(
        rendered.contains('✗') && rendered.contains("exit 101"),
        "a completed result must still report its real status: {rendered}"
    );
}

// ---- the SUMMARY arm is budgeted in the unit the limit counts: serialized (escaped) bytes ----
//
// The recurring defect: a size MEASURED in one unit (raw bytes) and a DIFFERENT thing returned (the
// compact serialized whole response). `\x01` and `\x1b` are 1 raw byte and 6 serialized, so a 2,000 B
// raw cut of one is 12,000 B on the wire; two such fields cannot share a 10,003 B response, and
// `call_content` then buffered the summary a SECOND time under `@tool_*`.
// Reviewer's measurement on the merged tree: `perl -e 'print "\x01" x N; print STDERR "\x01" x N'`
// returned an `@tool_` envelope for N = 1,000..10,000 (37 of 40 points).

/// `perl` printing `n` x `ch` (a perl escape such as `\x01`) on stdout and/or stderr, then `suffix`.
fn perl_repeat(ch: &str, n: usize, out: bool, err: bool, suffix: &str) -> String {
    let mut body = String::new();
    if out {
        body += &format!("print \"{ch}\" x {n}; ");
    }
    if err {
        body += &format!("print STDERR \"{ch}\" x {n}; ");
    }
    format!("perl -e '{body}'{suffix}")
}

/// The one-handle contract of every `run_command` response: no `@tool_*` handle anywhere, any
/// `output_id` is the command's own `@cmd_*`, and the compact response is within the inline limit.
fn assert_one_inline_handle(text: &str, parsed: &Value, what: &str) {
    assert!(
        !has_tool_handle(text),
        "{what}: a second handle: {text:.160}"
    );
    assert!(
        parsed.get("buffered_bytes").is_none(),
        "{what}: the `@tool_*` re-buffering envelope: {text:.160}"
    );
    if let Some(id) = parsed.get("output_id").and_then(Value::as_str) {
        assert!(id.starts_with("@cmd_"), "{what}: output_id {id}");
    }
    assert!(
        text.len() <= 10_003,
        "{what}: {} B compact is over the inline limit",
        text.len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn control_character_streams_keep_one_handle_in_the_generic_summary() {
    let (_dir, ctx) = project_ctx().await;
    for ch in ["\\x01", "\\x1b"] {
        for n in [1_000, 1_001, 1_500, 2_000, 2_001, 3_000, 5_000, 10_000] {
            for (out, err) in [(true, false), (false, true), (true, true)] {
                let what = format!("{ch} x {n} out={out} err={err}");
                let (text, parsed) =
                    buffer_query_free(&ctx, &perl_repeat(ch, n, out, err, "")).await;
                assert_one_inline_handle(&text, &parsed, &what);
                for (present, key) in [(out, "stdout"), (err, "stderr")] {
                    if !present {
                        continue;
                    }
                    let field = parsed[key]
                        .as_str()
                        .unwrap_or_else(|| panic!("{what}: no inline {key}: {text:.160}"));
                    assert!(
                        field.starts_with(|c: char| c.is_control()),
                        "{what}: the head of {key} is missing"
                    );
                    if field.contains("bytes shown") {
                        assert!(
                            field.contains(&format!("of {n} bytes shown")),
                            "{what}: the marker misreports the stream total"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn control_character_failure_fields_keep_one_handle_in_the_test_and_build_summaries() {
    let (_dir, ctx) = project_ctx().await;
    for (kind, suffix, head) in [
        ("test", "; echo cargo test", "failures:"),
        ("build", "; echo cargo build", "error[E0308]: x"),
    ] {
        // 2,000 x \x01 is 12,000 B serialized, so every case is over the inline limit and takes
        // the summary arm: the `type` key below is that arm's, which an inline response lacks.
        for n in [2_000, 3_000, 5_000, 10_000, 60_000] {
            for m in [0usize, 1_500, 50_000] {
                let what = format!("{kind} n={n} stderr={m}");
                let body = if kind == "test" {
                    format!("print \"{head}\\n\", \"\\x01\" x {n}, \"\\nfailures:\\n\"; ")
                } else {
                    format!("print \"{head}\\n\", \"\\x01\" x {n}, \"\\n\"; ")
                };
                let err = if m > 0 {
                    format!("print STDERR \"\\x01\" x {m}; ")
                } else {
                    String::new()
                };
                let command = format!("perl -e '{body}{err}'{suffix}");
                let (text, parsed) = buffer_query_free(&ctx, &command).await;
                assert_one_inline_handle(&text, &parsed, &what);
                assert_eq!(parsed["type"], kind, "{what}: misclassified: {text:.160}");
                let key = if kind == "test" {
                    "failures"
                } else {
                    "first_error"
                };
                assert!(
                    parsed[key].as_str().is_some_and(|f| f.starts_with(head)),
                    "{what}: the head of {key} is missing: {text:.160}"
                );
                if m > 0 {
                    assert!(
                        parsed["stderr"].as_str().is_some_and(|s| !s.is_empty()),
                        "{what}: the stderr is missing: {text:.160}"
                    );
                }
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_summary_marker_reports_the_stream_total_once() {
    // 50,000 x \x01 on stdout. `summarize_generic` cut it, then the `call_content` backstop cut the
    // CUT text again and dropped the first marker: "1000 of 2065 bytes shown" for a 50,000 B stream.
    let (_dir, ctx) = project_ctx().await;
    let (text, parsed) =
        buffer_query_free(&ctx, &perl_repeat("\\x01", 50_000, true, false, "")).await;
    assert_one_inline_handle(&text, &parsed, "50,000 x \\x01");
    let stdout = parsed["stdout"].as_str().expect("an inline stdout");
    assert_eq!(
        stdout.matches("bytes shown").count(),
        1,
        "the field was cut twice: {:?}",
        stdout
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
    );
    assert!(
        stdout.contains("of 50000 bytes shown"),
        "the marker must name the STREAM total: {:?}",
        stdout
            .chars()
            .filter(|c| !c.is_control())
            .collect::<String>()
    );
}

// ---- a buffer whose first lines are blank still returns real bytes, and says why it was cut ----
//
// Reviewer's measurement on the merged tree: a stored buffer of `"\n"` + one 60 KB line, queried
// with `cat` or `grep -v`, returned NO `stdout` key at all (`truncated:true`, 1/2); `" \n"` and
// `"\n\n\n"` in front returned only whitespace. `truncate_lines_and_bytes` treated the blank line
// as "the first line", kept it, and found the wide line "not first" so left it alone.

#[cfg(unix)]
#[tokio::test]
async fn blank_leading_lines_do_not_hide_a_wide_line_and_the_hint_routes_to_it() {
    let (_dir, ctx) = project_ctx().await;
    for prefix in ["\n", " \n", "\n\n\n"] {
        for query in ["cat", "grep -v zzz"] {
            let what = format!("prefix {prefix:?} via `{query}`");
            let stored = format!("{prefix}{}", "w".repeat(60_000));
            let (id, text, parsed) =
                query_stored(&ctx, stored, String::new(), |id| format!("{query} {id}")).await;

            let stdout = parsed["stdout"]
                .as_str()
                .unwrap_or_else(|| panic!("{what}: zero bytes of a non-empty result: {text:.300}"));
            assert!(
                stdout.matches('w').count() >= 1_000,
                "{what}: only whitespace came back ({} B): {:?}",
                stdout.len(),
                stdout.chars().take(40).collect::<String>()
            );
            assert!(
                stdout.contains("bytes shown"),
                "{what}: a clipped line must say so"
            );
            assert_eq!(parsed["truncated"], true, "{what}: {text:.200}");
            assert!(!has_tool_handle(&text), "{what}: {text:.200}");
            assert!(text.len() <= 10_003, "{what}: {} B", text.len());

            // FOLLOW the hint's route for a wide line.
            let hint = parsed["hint"].as_str().unwrap_or_default();
            assert!(
                hint.contains(&format!("cut -c1-4000 {id}")),
                "{what}: the hint names no window route: {hint}"
            );
            let (t2, p2) = buffer_query(&ctx, format!("cut -c1-4000 {id}")).await;
            let window = p2["stdout"]
                .as_str()
                .unwrap_or_else(|| panic!("{what}: the hint's route returned nothing: {t2:.300}"));
            assert!(
                window.contains(&"w".repeat(4_000)),
                "{what}: the window is not the line's first 4,000 bytes"
            );
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_cut_by_bytes_does_not_claim_a_line_cap() {
    // 60 lines of 400 B: ~24 KB, so the BYTE budget binds at ~24 lines, far under the 100-line cap.
    let (_dir, ctx) = project_ctx().await;
    let stored: String = (0..60)
        .map(|i| format!("row{i:03} {}\n", "r".repeat(393)))
        .collect();
    let (_id, text, parsed) =
        query_stored(&ctx, stored, String::new(), |id| format!("cat {id}")).await;
    let shown = parsed["stdout_shown"].as_u64().expect("a truncated query");
    assert!(shown < 60, "{text:.200}");
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(
        !hint.contains("capped at 100 lines"),
        "the cut was by bytes after {shown} lines, not by a line cap: {hint}"
    );
    assert!(
        hint.contains("response") && hint.contains(&format!("{shown}/60")),
        "the hint must state the real reason and the counts: {hint}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_cut_by_the_line_cap_still_says_so() {
    // The sibling of the test above: 400 lines are cut by LINES, and the hint says so.
    let (_dir, ctx) = project_ctx().await;
    // 400 lines of 40 B: 16 KB raw, so the response is summarized/cut; the 100-line cap binds well
    // before the byte budget (~9.7 KB, ~230 lines).
    let stored: String = (0..400)
        .map(|i| format!("{i:04} {}\n", "l".repeat(35)))
        .collect();
    let (_id, _text, parsed) =
        query_stored(&ctx, stored, String::new(), |id| format!("cat {id}")).await;
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(hint.contains("capped at 100 lines"), "{hint}");
}

// ---- a libtest run that compacts to a few hundred bytes is returned compacted, not summarized ----
//
// Reviewer's measurement: a `cargo test` with 85-88 empty-target blocks (raw ~9.6 KB, ~10 KB
// serialized because every newline is 2 B) came back as `{type, exit_code, output_id, passed}` with
// NO stdout, because the serialized gate judged the RAW streams before compaction was tried; the
// compacted response was 406 B and fit with room to spare.

#[cfg(unix)]
#[tokio::test]
async fn a_libtest_run_that_compacts_to_a_few_hundred_bytes_is_returned_compacted() {
    let (dir, ctx) = project_ctx().await;
    let mut stdout = String::from(
        "\nrunning 1 test\ntest alpha::one ... ok\n\n\
         test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\n",
    );
    let mut blocks = 0;
    while blocks < 86 {
        blocks += 1;
        stdout.push_str(&format!(
            "\nrunning 0 tests\n\n\
             test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; {blocks} filtered out; finished in 0.00s\n\n"
        ));
    }
    // The shape the reviewer measured: 85-88 empty blocks, raw under the limit, serialized over it.
    assert!(
        (85..=88).contains(&blocks),
        "fixture drifted: {blocks} blocks"
    );
    assert!(
        stdout.len() <= 10_003,
        "raw must fit: {} B (else this tests a different thing)",
        stdout.len()
    );
    assert!(
        crate::tools::command_summary::inline_response_exceeds_limit(0, &stdout, "", 0),
        "the serialized raw run must be over the limit, or the gate is not what is under test"
    );
    std::fs::write(dir.path().join("libtest.out"), &stdout).unwrap();

    let (text, parsed) = buffer_query_free(&ctx, "cat libtest.out; echo cargo test").await;
    assert!(!has_tool_handle(&text), "{text:.200}");
    let out = parsed["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("the compacted stdout is missing: {text:.300}"));
    assert!(
        out.contains("codescout compacted this run") && out.contains("alpha::one"),
        "{out:.300}"
    );
    assert!(
        parsed["output_id"]
            .as_str()
            .is_some_and(|i| i.starts_with("@cmd_")),
        "{text:.200}"
    );
    assert_eq!(parsed["passed"], 1, "{text:.200}");
    assert!(
        text.len() < 1_000,
        "compacted is small, got {} B",
        text.len()
    );
}
/// A libtest stdout of one real test and `blocks` empty targets; the real test's name carries `pad`
/// extra bytes so a caller can land the total on an exact length.
fn libtest_run(blocks: usize, pad: usize) -> String {
    let mut s = format!(
        "\nrunning 1 test\ntest alpha::{}one ... ok\n\n\
         test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n\n",
        "p".repeat(pad)
    );
    for b in 1..=blocks {
        s.push_str(&format!(
            "\nrunning 0 tests\n\n\
             test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; {b} filtered out; finished in 0.00s\n\n"
        ));
    }
    s
}

#[cfg(unix)]
#[tokio::test]
async fn compaction_is_considered_up_to_exactly_the_inline_limit_in_raw_bytes() {
    // A run over the limit RAW was summarized before the gate counted serialized bytes and still is:
    // only the population that was returned inline (and compacted) is rescued. The boundary is
    // inclusive: 10,003 raw bytes is inside it and 10,004 is not.
    let base = libtest_run(86, 0).len();
    assert!(base < 10_003, "fixture: {base} B");
    let at = libtest_run(86, 10_003 - base);
    let over = libtest_run(86, 10_004 - base);
    assert_eq!((at.len(), over.len()), (10_003, 10_004));

    let (inside, _c) = run_inline("cargo test", &at, "").await;
    assert!(
        inside["stdout"]
            .as_str()
            .is_some_and(|s| s.contains("codescout compacted this run")),
        "10,003 raw bytes is inside the boundary: {inside:.200}"
    );
    let (outside, _c) = run_inline("cargo test", &over, "").await;
    assert_eq!(outside["type"], "test", "{outside:.200}");
    assert!(
        outside.get("stdout").is_none(),
        "10,004 raw bytes is over the limit and is summarized, as it always was: {outside:.200}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_compacted_response_is_judged_with_its_late_keys_and_its_real_handle_at_the_edge() {
    // The compacted response is measured BEFORE its buffer entry exists, so the gate builds it
    // around a stand-in handle of the real length and with every late key attached. Here the
    // response is landed exactly ON the limit (a `tee_skipped` note sized from a measured control
    // run), then one byte over: a stand-in shorter than the real handle, or a gate that left the
    // late keys out, calls the over-the-limit response small and returns it.
    use super::output::{handle_successful_output_with, LateKeys};
    let (_dir, ctx) = project_ctx().await;
    let stdout = libtest_run(86, 0);
    let run = |note: usize| {
        let (ctx, stdout) = (&ctx, stdout.clone());
        async move {
            handle_successful_output_with(
                "cargo test",
                stdout,
                String::new(),
                0,
                false,
                None,
                std::path::Path::new("."),
                ctx,
                LateKeys {
                    redacted: 0,
                    tee_skipped: Some("n".repeat(note)),
                },
            )
            .await
            .unwrap()
        }
    };
    let is_compacted = |r: &Value| {
        r["stdout"]
            .as_str()
            .is_some_and(|s| s.contains("codescout compacted this run"))
    };

    let control = run(1).await;
    assert!(is_compacted(&control), "precondition: {control:.200}");
    let note = 1 + (10_003 - control.to_string().len());

    let at = run(note).await;
    assert!(is_compacted(&at), "exactly on the limit stays compacted");
    assert_eq!(at.to_string().len(), 10_003);

    let over = run(note + 1).await;
    assert!(
        !is_compacted(&over),
        "one byte over the limit must not be returned compacted: {} B",
        over.to_string().len()
    );
    assert!(
        over.to_string().len() <= 10_003,
        "the fallback is a response that fits: {} B",
        over.to_string().len()
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_summary_budgets_around_the_late_keys_it_carries() {
    // The summary arm attaches the same late keys every arm does (here a 7,000 B
    // `unfiltered_output_skipped`). Two 1,500 B control-character streams are ~4 KB at the natural
    // ceilings, which fits alone; beside the key it does not, so the shares must be measured beside
    // that key, or the response lands over the limit and `call_content` buffers it a second time.
    use super::output::{handle_successful_output_with, LateKeys};
    let (_dir, ctx) = project_ctx().await;
    let stream = "\u{1}".repeat(1_500);
    let response = handle_successful_output_with(
        "perl -e 'print 1'",
        stream.clone(),
        stream.clone(),
        0,
        false,
        None,
        std::path::Path::new("."),
        &ctx,
        LateKeys {
            redacted: 0,
            tee_skipped: Some("n".repeat(7_000)),
        },
    )
    .await
    .unwrap();

    assert!(
        response["output_id"]
            .as_str()
            .is_some_and(|i| i.starts_with("@cmd_")),
        "the summary arm: {response:.200}"
    );
    assert_eq!(
        response["unfiltered_output_skipped"]
            .as_str()
            .unwrap()
            .len(),
        7_000,
        "the late key is carried whole"
    );
    let len = response.to_string().len();
    assert!(len <= 10_003, "{len} B compact is over the inline limit");
    for key in ["stdout", "stderr"] {
        assert!(
            response[key]
                .as_str()
                .unwrap()
                .contains("of 1500 bytes shown"),
            "{key}"
        );
    }
}
