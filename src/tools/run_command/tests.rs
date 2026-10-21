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
        &Default::default(), // notes
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
        &Default::default(), // notes
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
        "echo x",
        "echo x",
        30,
        false, // acknowledge_risk
        None,  // cwd_param
        true,  // buffer_only
        true,  // run_in_background
        &root,
        &security,
        &ctx,
        &Default::default(),
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
        "echo x",
        "echo x",
        30,
        false, // acknowledge_risk
        None,  // cwd_param
        false, // buffer_only
        false, // run_in_background
        &root,
        &security,
        &ctx,
        &Default::default(),
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
    run_query(ctx, json!({ "command": command, "timeout_secs": 10 })).await
}

/// [`buffer_query`] with the whole `input`, for a test that sets more than the command.
async fn run_query(ctx: &ToolContext, input: Value) -> (String, Value) {
    let content = RunCommand.call_content(input, ctx).await.unwrap();
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
    // `grep status` is not a slice of the buffer, so the route appends to the query rather than
    // naming the handle: on the handle, `cut` reads the SOURCE's first lines, which need not hold
    // the line the query printed.
    assert!(
        stdout.contains("appending `| cut -c1-4000` to this query"),
        "the marker names a working route for the query's own line: {:.400}",
        stdout.chars().skip(1000).collect::<String>()
    );
    assert_eq!(
        parsed["truncated"], true,
        "one line shown whole-by-count but clipped by bytes is still truncated: {text:.300}"
    );
    let hint = parsed["hint"].as_str().unwrap_or_default();
    assert!(
        hint.contains("appending `| cut -c1-4000` to this query"),
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
async fn a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs() {
    // A slice of a `@tool_*` ref is paged like any other ref: `sed -n` on the handle, in real
    // numbers. This test pinned `read_file("@tool_X", json_path="$.<field>")` until 2026-10-08: a
    // route with a placeholder in it, offered to every query on the ref, slice or not.
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
        !hint.contains("<field>") && !hint.contains("@tool_abc"),
        "{hint}"
    );
    let shown = parsed["stdout_shown"].as_u64().unwrap();
    let route = format!("sed -n '{},{}p' {id}", shown + 1, shown + 100);
    assert!(hint.contains(&route), "{hint}");

    // Follow it: the page after the three short lines is the wide row, which comes back clipped.
    let (t2, p2) = buffer_query(&ctx, route.clone()).await;
    assert!(
        p2["stdout"]
            .as_str()
            .unwrap_or_default()
            .starts_with("    \"rrrr"),
        "`{route}` did not continue the slice: {t2:.300}"
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
    // `cat`: a plain slice, whose routes name the handle. A non-slice query's route appends to the
    // query instead; that one is followed in
    // `a_wide_line_a_non_slice_query_prints_is_windowed_by_appending_to_the_query`.
    let (_t, first) = buffer_query(&ctx, format!("cat {id}")).await;
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

    // FOLLOW the rerun route the marker gives, as it states it: it returns a window of the stream
    // that was cut.
    let route = wrapped_route_in(stderr, &awk(&id, ""))
        .unwrap_or_else(|| panic!("no route in the marker: {stderr:.0}"));
    let (t2, p2) = buffer_query(&ctx, route).await;
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
                ..Default::default()
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
                ..Default::default()
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
                    ..Default::default()
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
            ..Default::default()
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
    let records_wip = |v: &Value| {
        v[crate::tools::CUT_FIELDS_KEY]
            .as_array()
            .is_some_and(|a| a.contains(&json!("wip_authors")))
    };
    assert!(
        !records_wip(&small),
        "an uncut diagnostic is not recorded as cut"
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
        records_wip(&big),
        "the cut is recorded, so the backstop does not cut it again: {big:.300}"
    );
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
// A query of `<handle>.err` pages the STDERR stream. Its hint must name that same stream: with the
// bare handle the next page is a page of stdout, which here is one short line, so a reader who
// follows the hint gets nothing back and may take that for the end of the output.
#[cfg(unix)]
#[tokio::test]
async fn a_next_page_hint_on_an_err_query_names_the_err_stream_and_that_route_works() {
    let (_dir, ctx) = project_ctx().await;
    let stderr: String = (1..=300).map(|i| format!("{i:>40}\n")).collect();
    let id = ctx
        .output_buffer
        .store("cmd".into(), "x\n".into(), stderr, 0);

    let first = RunCommand
        .call(
            json!({ "command": format!("cat {id}.err"), "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .unwrap();
    let hint = first["hint"].as_str().unwrap_or("").to_string();
    assert!(
        hint.contains(&format!("p' {id}.err")),
        "the next-page route must name the queried stream `{id}.err`, got: {hint}"
    );

    // Follow it: pull the `sed -n '<a>,<b>p' <ref>` command out of the hint and run it.
    let from = hint.find("sed -n '").expect("a next-page sed route");
    let rest = &hint[from..];
    let route = rest[..rest.find(". ").unwrap_or(rest.len())].to_string();
    let page = RunCommand
        .call(json!({ "command": route, "timeout_secs": 5 }), &ctx)
        .await
        .unwrap();
    let body = page["stdout"].as_str().unwrap_or("");
    assert!(
        body.contains("101"),
        "following the hint must return the next page of stderr, got: {body:.200} (route {route})"
    );
}
// The wide-line remedy, in the clipped line's marker AND in the hint, must read the stream the
// query read: `cut -c1-4000 @cmd_X` on a wide STDERR line cuts stdout instead.
#[cfg(unix)]
#[tokio::test]
async fn a_wide_line_remedy_on_an_err_query_names_the_err_stream() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx.output_buffer.store(
        "cmd".into(),
        "x\n".into(),
        format!("{}\n", "a".repeat(30_000)),
        0,
    );

    let resp = RunCommand
        .call(
            json!({ "command": format!("cat {id}.err"), "timeout_secs": 5 }),
            &ctx,
        )
        .await
        .unwrap();
    let remedy = format!("cut -c1-4000 {id}.err");
    let stdout = resp["stdout"].as_str().unwrap_or("");
    let hint = resp["hint"].as_str().unwrap_or("");
    assert!(
        stdout.contains(&remedy),
        "the clipped line's marker must name `{id}.err`: {stdout:.0}"
    );
    assert!(
        hint.contains(&remedy),
        "the hint must name `{id}.err`: {hint}"
    );
}
/// Every distinct buffer handle (`@cmd_`, `@tool_`, `@file_` and eight hex digits) `text` names.
fn handles_named(text: &str) -> std::collections::BTreeSet<String> {
    regex::Regex::new(r"@(?:cmd|tool|file)_[0-9a-f]{8}")
        .expect("static pattern")
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}

/// Each escape class the response limit counts differently: 1, 2, 2, 6 serialized bytes per char,
/// and two multibyte chars serde_json writes raw.
// cap-class: NOT_A_CAP — a test fixture listing escape classes, not a bound on any result
const ESCAPE_UNITS: [&str; 6] = ["a", "\"", "\\", "\u{1}", "é", "日"];

/// Serialized bytes of one `unit`, quotes excluded.
fn escaped_unit_len(unit: &str) -> usize {
    serde_json::to_string(unit).unwrap().len() - 2
}

// A query of `<handle>.err` READS the stored stderr: that stream is its payload, its stdout. The
// bounded `stderr` field exists so a query of the BARE handle still shows the command's stderr; on
// a `.err` query it repeated the payload, took lines and bytes from it, and reported `stderr N/M`
// for the stream the reader was already paging. Measured before the fix: `stdout 80/300, stderr
// 20/300`.
#[cfg(unix)]
#[tokio::test]
async fn an_err_query_carries_the_stored_stderr_once_and_its_routes_work() {
    let (_dir, ctx) = project_ctx().await;
    let stderr: String = (1..=300).map(|i| format!("{i:>40}\n")).collect();
    let (id, text, parsed) =
        query_stored(&ctx, "x\n".into(), stderr, |id| format!("cat {id}.err")).await;

    assert_eq!(
        handles_named(&text).into_iter().collect::<Vec<_>>(),
        vec![id.clone()],
        "{text:.300}"
    );
    assert!(text.len() <= crate::tools::INLINE_MAX_RESPONSE_LEN);
    for key in ["stderr", "stderr_shown", "stderr_total"] {
        assert!(
            parsed.get(key).is_none(),
            "the payload must not come back again as `{key}`: {text:.400}"
        );
    }
    let hint = parsed["hint"]
        .as_str()
        .unwrap_or_else(|| panic!("a capped query owes a hint: {text:.300}"));
    assert!(!hint.contains("stderr"), "no side-channel note: {hint}");
    assert_eq!(
        parsed["stdout_shown"],
        json!(crate::tools::command_summary::BUFFER_QUERY_INLINE_CAP),
        "the whole line budget goes to the payload: {hint}"
    );
    assert_eq!(parsed["stdout_total"], json!(300), "{hint}");

    // Follow every route the hint names, on the stream it names.
    let page = regex::Regex::new(r"sed -n '\d+,\d+p' @[A-Za-z0-9_]+(?:\.err)?")
        .unwrap()
        .find(hint)
        .unwrap_or_else(|| panic!("no next-page route: {hint}"))
        .as_str()
        .to_string();
    assert!(page.ends_with(&format!("{id}.err")), "{page}");
    let (page_text, next) = buffer_query_free(&ctx, &page).await;
    let body = next["stdout"].as_str().unwrap_or_default();
    assert!(
        body.starts_with(&format!("{:>40}\n", 101)),
        "the next page is lines 101.. of stderr: {page_text:.300}"
    );
    let grep_on = regex::Regex::new(r"grep 'keyword' (@[A-Za-z0-9_]+(?:\.err)?)")
        .unwrap()
        .captures(hint)
        .unwrap_or_else(|| panic!("no grep route: {hint}"))[1]
        .to_string();
    assert_eq!(grep_on, format!("{id}.err"), "{hint}");
    let (found_text, found) = buffer_query_free(&ctx, &format!("grep -c ' 250$' {grep_on}")).await;
    assert_eq!(
        found["stdout"].as_str().map(str::trim),
        Some("1"),
        "the grep route searches stderr: {found_text:.300}"
    );
}

// The same rule across the byte edge of the response, in every escape class: a `.err` query whose
// payload serializes to 9,300..10,500 B carries it once, mints no handle of its own, and fits.
#[cfg(unix)]
#[tokio::test]
async fn an_err_query_across_the_byte_edge_keeps_one_handle_and_one_copy() {
    let (_dir, ctx) = project_ctx().await;
    for unit in ESCAPE_UNITS {
        let line = format!("{}\n", unit.repeat(40));
        let line_cost = 40 * escaped_unit_len(unit) + 2;
        // Both arms must be reached, or the sweep is not across the edge: whole, and cut.
        let (mut whole, mut cut) = (0, 0);
        for target in (9_300..=10_500).step_by(150) {
            let stderr = line.repeat(target / line_cost + 1);
            let (id, text, parsed) =
                query_stored(&ctx, "x\n".into(), stderr, |id| format!("cat {id}.err")).await;
            let named = handles_named(&text);
            assert!(
                named.iter().all(|h| *h == id),
                "{unit:?} {target}: a second handle {named:?}"
            );
            assert!(
                parsed.get("output_id").is_none(),
                "{unit:?} {target}: {text:.200}"
            );
            assert!(
                text.len() <= crate::tools::INLINE_MAX_RESPONSE_LEN,
                "{unit:?} {target}: {} B",
                text.len()
            );
            assert!(
                parsed.get("stderr").is_none(),
                "{unit:?} {target}: the payload came back twice: {text:.200}"
            );
            if parsed.get("truncated").is_some() {
                cut += 1;
            } else {
                whole += 1;
            }
        }
        assert!(whole > 0 && cut > 0, "{unit:?}: whole {whole}, cut {cut}");
    }
}
// A summarized test run whose stderr ends in ONE line wider than the stderr field: the end of that
// line is usually the verdict, and the field kept only its head. Elided in the middle, in escaped
// bytes like every other field, both ends survive, across the response's byte edge and every
// escape class; the header's `read_file("<handle>.err")` route resolves to the whole line.
#[cfg(unix)]
#[tokio::test]
async fn a_wide_last_stderr_line_keeps_its_end_across_the_byte_edge() {
    let (dir, ctx) = project_ctx().await;
    for unit in ESCAPE_UNITS {
        let mut summarized = 0;
        for target in (9_300..=10_500).step_by(300) {
            let body = unit.repeat(target / escaped_unit_len(unit));
            std::fs::write(
                dir.path().join("wide.log"),
                format!("START{body}THE_VERDICT\n"),
            )
            .unwrap();
            let (text, parsed) = buffer_query_free(&ctx, "cat wide.log >&2; echo cargo test").await;
            assert!(!has_tool_handle(&text), "{unit:?} {target}: {text:.200}");
            assert!(
                handles_named(&text).len() <= 1,
                "{unit:?} {target}: {text:.300}"
            );
            if parsed.get("output_id").is_none() {
                assert!(
                    text.len() <= crate::tools::INLINE_MAX_RESPONSE_LEN,
                    "{unit:?} {target}: {} B",
                    text.len()
                );
            }
            let stderr = parsed["stderr"]
                .as_str()
                .unwrap_or_else(|| panic!("{unit:?} {target}: no stderr: {text:.300}"));
            assert!(
                stderr.contains("START") && stderr.contains("THE_VERDICT"),
                "{unit:?} {target}: both ends of the line must survive: {stderr:.300}"
            );
            if let Some(id) = parsed["output_id"].as_str() {
                let route = format!("read_file(\"{id}.err\")");
                assert!(stderr.contains(&route), "{unit:?} {target}: {stderr:.300}");
                let whole = ctx
                    .output_buffer
                    .get_stream(&format!("{id}.err"))
                    .unwrap_or_else(|| panic!("{unit:?} {target}: {route} resolves nothing"));
                assert!(whole.ends_with("THE_VERDICT\n"), "{unit:?} {target}");
                summarized += 1;
            }
        }
        assert!(
            summarized > 0,
            "{unit:?}: no case reached the summarized stderr"
        );
    }
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

    // A newer mtime without changed bytes must stay silent. The mtime is an HOUR ahead, not a
    // couple of seconds: `capped_stamp` stamps a read of the file with its mtime capped at the
    // clock, so with a lead of 2 s any delay over 2 s between setting the mtime and the first read
    // (a loaded gate can add one) lets the stamp reach the mtime, and the second
    // edit, written with the same mtime, is not above the stamp: the handle serves the old text and
    // says nothing. An hour is out of reach of any load delay. Proved on the old lead of 2 s: the
    // same scenario with a 3 s pause before the first read fails every time, and passes with an hour.
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future)).unwrap();
    let unchanged = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .unwrap();
    assert_eq!(unchanged["stdout"], "original");

    fs::write(&path, "edited").unwrap();
    filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future)).unwrap();
    let result = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .unwrap();
    let stdout = result["stdout"].as_str().unwrap();
    assert!(
        stdout.starts_with(&format!("↻ {} refreshed from disk", id)),
        "expected change notice, got: {stdout:?}"
    );
    assert!(
        stdout.ends_with("edited"),
        "the command must read the new bytes: {stdout:?}"
    );
    let second = RunCommand
        .call(json!({ "command": format!("cat {}", id) }), &ctx)
        .await
        .unwrap();
    assert_eq!(second["stdout"], "edited", "notice must be reported once");
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
    // use cargo as the canonical unbounded LHS sentinel. A BACKGROUND run is always refused (a
    // foreground pipe ending in a terminal filter is run with a capture instead), and the refusal
    // comes before anything runs, so this does not run `cargo test`.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo test | grep FAILED", "run_in_background": true }),
            &ctx,
        )
        .await
        .expect_err("IL3 should block background `cargo test | grep`");
    let msg = err.to_string();
    assert!(msg.contains("IL3 violation"), "missing IL3 marker: {msg}");
    assert!(msg.contains("@bg_"), "missing rewrite hint: {msg}");
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

#[tokio::test]
async fn the_interactive_drain_keeps_one_streams_bytes_when_the_other_is_already_at_eof() {
    use super::interactive::drain_with_settle;
    use tokio::io::{empty, BufReader};
    // `tokio::select!` picks among ready arms at random, so one pass proves little: a drain
    // that ends at the first EOF loses the other stream's bytes on about half of the passes.
    // 64 passes make that loss certain for all practical purposes.
    for pass in 0..64 {
        let mut out = BufReader::new(&b"hello\n"[..]);
        let mut err = BufReader::new(empty());
        let (text, both_eof) = drain_with_settle(&mut out, &mut err, 150).await;
        assert_eq!(text, "hello\n", "stdout bytes, pass {pass}");
        assert!(both_eof, "pass {pass}");

        let mut out = BufReader::new(empty());
        let mut err = BufReader::new(&b"oops\n"[..]);
        let (text, both_eof) = drain_with_settle(&mut out, &mut err, 150).await;
        assert_eq!(text, "oops\n", "stderr bytes, pass {pass}");
        assert!(both_eof, "pass {pass}");
    }
}

#[tokio::test]
async fn the_interactive_drain_ends_on_the_settle_window_while_a_pipe_is_open_and_quiet() {
    use super::interactive::drain_with_settle;
    use tokio::io::{duplex, AsyncWriteExt, BufReader};
    // A REPL that printed its prompt and waits: both pipes stay open, one holds bytes.
    let (mut out_writer, out_reader) = duplex(64);
    let (_err_writer, err_reader) = duplex(64);
    out_writer.write_all(b"prompt> ").await.unwrap();
    let mut out = BufReader::new(out_reader);
    let mut err = BufReader::new(err_reader);
    let started = std::time::Instant::now();
    let (text, both_eof) = drain_with_settle(&mut out, &mut err, 50).await;
    assert_eq!(text, "prompt> ");
    assert!(
        !both_eof,
        "an open pipe is not EOF: the caller must elicit, not report an exit"
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "the drain must end on the settle window, not wait for a pipe that never closes"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn child_exit_waits_once_both_pipes_are_closed_and_only_that_long() {
    use super::interactive::child_exit;
    use std::time::{Duration, Instant};
    let spawn = |script: &str| {
        tokio::process::Command::new("sh")
            .arg("-c")
            .arg(script)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .unwrap()
    };
    // Closes both pipes, then lives 300 ms more before exiting 0: what the drain sees when a
    // command finishes, since a process closes its descriptors before it can be waited on.
    let mut closing = spawn("exec 1>&- 2>&-; sleep 0.3");
    assert!(
        child_exit(&mut closing, false, Duration::from_secs(5))
            .await
            .unwrap()
            .is_none(),
        "without EOF on both pipes it is a single try_wait, and the child has not exited yet"
    );
    let status = child_exit(&mut closing, true, Duration::from_secs(5))
        .await
        .unwrap();
    assert!(
        status.is_some_and(|s| s.success()),
        "with both pipes closed the exit is waited for: {status:?}"
    );

    // Pipes closed but the child lives on: it is reported as running once the bound passes.
    let mut lingering = spawn("exec 1>&- 2>&-; sleep 5");
    let started = Instant::now();
    assert!(child_exit(&mut lingering, true, Duration::from_millis(100))
        .await
        .unwrap()
        .is_none());
    assert!(started.elapsed() < Duration::from_secs(3));
    let _ = lingering.kill().await;
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
    // Enough lines to exceed the capture store limit (CAPTURE_STORE_LIMIT_BYTES, 4 MiB;
    // `seq 1 1000000` is ~6.9 MB), so the stored copy is truncated but the reported count
    // must still be the full pre-truncation line count.
    let line_count = 1_000_000;
    let result = RunCommand
        .call(
            json!({ "command": format!("seq 1 {line_count} | grep zzz") }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.get("unfiltered_truncated").is_some(),
        "fixture must actually exceed the store limit to exercise truncation: {result}"
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
    let line_count = 1_000_000;
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
    let line_count = 1_000_000;
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
    let line_count = 1_000_000;
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
            json!({ "command": format!("grep -c '^999999$' {handle}") }),
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
/// The unfiltered capture is the source `run_command` re-reads when a filter hid what the caller
/// wanted, so the limit it is stored under decides how often "re-read it" works. At 10 KB a
/// 100,000-line stream was cut to ~2,200 lines. A capture under `CAPTURE_STORE_LIMIT_BYTES` is kept
/// whole: no marker, no cut record, and the handle holds every line.
#[cfg(unix)]
#[tokio::test]
async fn a_capture_under_the_store_limit_is_kept_whole() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 100000 | grep 77777" }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["unfiltered_output_lines"], 100_000,
        "the count is the stream's: {result}"
    );
    assert!(
        result.get("unfiltered_truncated").is_none(),
        "a capture under the limit must not be cut: {result}"
    );
    assert!(
        result.get("unfiltered_buffered_lines").is_none(),
        "a whole capture has nothing to distinguish from its stream: {result}"
    );
    let handle = result["unfiltered_output"]
        .as_str()
        .expect("handle")
        .to_string();
    let counted = RunCommand
        .call(json!({ "command": format!("wc -l {handle}") }), &ctx)
        .await
        .unwrap();
    let stdout = counted["stdout"].as_str().expect("stdout");
    assert!(
        stdout.trim_start().starts_with("100000"),
        "the handle must serve every line of the stream: {counted}"
    );
}
/// The splice wraps the filter in a group that drains what the filter leaves unread, so an
/// early-exit filter (`head -1`) cannot close the pipe and stop the producer mid-stream. The
/// newline before `s=$?` (not `;`) keeps a trailing `# comment` in the filter from swallowing the
/// closing brace. `filter` is the text after the spliced `|`.
#[test]
fn inject_tee_in_wraps_the_filter_in_a_draining_group() {
    let dir = tempdir().unwrap();
    let injected = super::inner::inject_tee_in("seq 1 3 | head -1", false, dir.path()).unwrap();
    let cap = injected
        .capture
        .as_ref()
        .expect("a terminal filter must be captured")
        .0
        .clone();
    assert_eq!(
        injected.command,
        format!(
            "seq 1 3 | tee '{cap}' | {{ head -1\ns=$?; head -c {} >/dev/null; exit $s; }}",
            super::output::CAPTURE_STORE_LIMIT_BYTES
        ),
        "the filter must run inside the bounded draining group"
    );
    assert_eq!(injected.filter.as_deref(), Some("head -1"));

    // Control: nothing captured, nothing to name.
    let plain = super::inner::inject_tee_in("echo hi", false, dir.path()).unwrap();
    assert_eq!(plain.filter, None);
}

/// `| head -N` closed the pipe after N lines, and the capture was cut to whatever had reached
/// `tee` by then (1860 lines of a 300000-line stream), yet the response reported that partial
/// stream as if it were the whole. With the drain the producer runs to completion.
/// docs/issues/2026-10-21-head-capture-looks-complete-but-is-partial.md
#[cfg(unix)]
#[tokio::test]
async fn a_head_pipe_still_captures_the_whole_stream() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "seq 1 300000 | head -3", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["stdout"], "1\n2\n3\n", "{result}");
    assert_eq!(
        result["unfiltered_output_lines"], 300_000,
        "the capture must hold the whole stream, not what arrived before head closed: {result}"
    );
    assert!(
        result.get("unfiltered_truncated").is_none(),
        "a 300000-line stream is ~1.9 MiB, under the capture limit: {result}"
    );
}

/// The drain group ends in `exit $s`, so the filter's own status is the pipeline's.
#[cfg(unix)]
#[tokio::test]
async fn a_filter_exit_status_survives_the_drain() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 10 | grep zzz" }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["exit_code"], 1,
        "grep's no-match status must survive: {result}"
    );
    assert!(result.get("unfiltered_output").is_some(), "{result}");
}

/// A `# comment` after the filter lands on the filter's line; the newline before `s=$?` is what
/// keeps it from commenting out the rest of the group.
#[cfg(unix)]
#[tokio::test]
async fn a_comment_after_the_filter_does_not_break_the_drain() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 10 | head -2 # why" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["stdout"], "1\n2\n", "{result}");
    assert_eq!(result["exit_code"], 0, "{result}");
}

/// The group is on the filter side of the pipe only: the producer's stderr must not be piped
/// into the filter.
#[cfg(unix)]
#[tokio::test]
async fn a_producers_stderr_is_not_piped_into_the_filter() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "(echo out; echo err >&2) | grep err" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["stdout"], "", "{result}");
    assert!(
        result["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("err"),
        "the producer's stderr must reach the response: {result}"
    );
}
/// The drain is bounded by the capture store limit. An endless producer into an early-exit filter
/// ended at once before the drain (SIGPIPE); an unbounded drain would read it until the timeout and
/// fill the disk. With the bound, `yes` is stopped by the closed pipe once the bound is read, and
/// the capture is exactly what the handle can keep, so it is flagged truncated.
#[cfg(unix)]
#[tokio::test]
async fn an_endless_producer_into_head_ends_at_once_with_a_bounded_capture() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "yes | head -1", "timeout_secs": 10 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.get("timed_out").is_none(),
        "the bounded drain must let an endless producer end: {result}"
    );
    assert_eq!(result["stdout"], "y\n", "{result}");
    assert_eq!(
        result["unfiltered_truncated"], true,
        "an endless stream cannot be captured whole: {result}"
    );
}

/// The password idiom: an endless random stream trimmed to a few bytes.
#[cfg(unix)]
#[tokio::test]
async fn a_urandom_password_idiom_ends_at_once() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({
                "command": "cat /dev/urandom | LC_ALL=C tr -dc a-z | head -c 20",
                "timeout_secs": 10,
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(
        result.get("timed_out").is_none(),
        "the bounded drain must let an endless producer end: {result}"
    );
    let stdout = result["stdout"].as_str().expect("stdout");
    assert!(
        stdout.len() == 20 && stdout.bytes().all(|b| b.is_ascii_lowercase()),
        "expected 20 lowercase letters, got {stdout:?}: {result}"
    );
}
/// The cost the bounded drain does not remove: a SLOW endless producer writes less than the bound
/// before `timeout_secs`, so the drain never closes the pipe and the command runs to the timeout.
/// Before the drain SIGPIPE ended it at once. Until the timeout path recovers output (plan
/// Task 4) the response is a bare `timed_out`; this pins the cost so Task 4 changes it on purpose.
#[cfg(unix)]
#[tokio::test]
async fn a_slow_endless_producer_into_head_runs_to_the_timeout() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({
                "command": "(while :; do echo x; sleep 0.2; done) | head -1",
                "timeout_secs": 2,
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["timed_out"], true, "{result}");
}

/// Past the store limit the drain stops the stream, so the capture file is a prefix of it and
/// `unfiltered_output_lines` (the capture's line count) is a LOWER BOUND of the stream, which
/// `unfiltered_truncated` marks. The same stream through `grep` reads to its end and reports
/// 1,000,000; pinning "strictly between" is what keeps this from being read as the total.
#[cfg(unix)]
#[tokio::test]
async fn a_head_pipe_over_the_store_limit_reports_a_lower_bound_and_the_truncated_flag() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "seq 1 1000000 | head -3", "timeout_secs": 30 }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["stdout"], "1\n2\n3\n", "{result}");
    assert_eq!(
        result["unfiltered_truncated"], true,
        "a stream cut by the drain must be marked: {result}"
    );
    let lines = result["unfiltered_output_lines"]
        .as_u64()
        .expect("a line count");
    assert!(
        lines > 500_000 && lines < 1_000_000,
        "a lower bound of the 1,000,000-line stream, not its total: {result}"
    );
}

/// R15: a `;` tail after the filter means the text after the first pipe is not ONE pipeline, so the
/// command is not spliced and there is no fallback to another pipe. It runs exactly as written (the
/// tail runs, the status is the tail's) and has no capture.
#[cfg(unix)]
#[tokio::test]
async fn a_semicolon_tail_after_the_filter_runs_untouched_with_no_capture() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 10 | head -2; echo done" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["stdout"], "1\n2\ndone\n", "{result}");
    assert_eq!(result["exit_code"], 0, "{result}");
    assert!(
        result.get("unfiltered_output").is_none(),
        "nothing was spliced, so no capture may be offered; got: {result}"
    );
}
/// In a chain the tee goes at the FIRST pipe, so the handle is the producer's own stream. With the
/// splice at the last pipe, `seq 1 1000 | grep 7 | head -2` captured grep's 271 lines.
#[test]
fn inject_tee_in_splices_a_chain_at_its_first_pipe() {
    let dir = tempdir().unwrap();
    let injected =
        super::inner::inject_tee_in("seq 1 3 | grep 2 | head -1", false, dir.path()).unwrap();
    let cap = injected
        .capture
        .as_ref()
        .expect("a terminal filter must be captured")
        .0
        .clone();
    assert_eq!(
        injected.command,
        format!(
            "seq 1 3 | tee '{cap}' | {{ grep 2 | head -1\ns=$?; head -c {} >/dev/null; exit $s; }}",
            super::output::CAPTURE_STORE_LIMIT_BYTES
        ),
    );
    assert_eq!(injected.filter.as_deref(), Some("grep 2 | head -1"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_chain_keeps_the_producers_whole_stream_in_the_handle() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 1000 | grep 7 | head -2" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["stdout"], "7\n17\n", "{result}");
    assert_eq!(
        result["unfiltered_output_lines"], 1000,
        "the handle must hold the producer's stream, not grep's output: {result}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_chain_keeps_the_last_stages_exit_status() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 10 | sort | grep zzz" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["exit_code"], 1, "{result}");
}

#[cfg(unix)]
#[tokio::test]
async fn an_endless_producer_through_a_chain_into_head_ends_at_once() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "yes | grep y | head -1", "timeout_secs": 10 }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.get("timed_out").is_none(), "{result}");
    assert_eq!(result["stdout"], "y\n", "{result}");
    assert_eq!(result["unfiltered_truncated"], true, "{result}");
}

/// The `|` inside quotes is data: the first REAL pipe is the splice point.
#[cfg(unix)]
#[tokio::test]
async fn a_quoted_pipe_before_a_chain_is_not_its_first_pipe() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "echo 'a|b' | grep a | head -1" }), &ctx)
        .await
        .unwrap();
    assert_eq!(result["stdout"], "a|b\n", "{result}");
    assert_eq!(result["unfiltered_output_lines"], 1, "{result}");
}

/// Over the limit the capture is cut at a line boundary and marked, and the count is still the
/// whole stream's. `seq 1 1000000` is ~6.9 MB, so a cut at 4 MiB keeps roughly 600,000 lines.
#[cfg(unix)]
#[tokio::test]
async fn a_capture_over_the_store_limit_is_cut_and_marked() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 1000000 | grep 99999" }), &ctx)
        .await
        .unwrap();
    assert_eq!(
        result["unfiltered_truncated"],
        json!(true),
        "a capture over the limit must say it was cut: {result}"
    );
    assert_eq!(
        result["unfiltered_output_lines"], 1_000_000,
        "the count is on the FULL capture, before the cut: {result}"
    );
    let served = result["unfiltered_buffered_lines"]
        .as_u64()
        .unwrap_or_else(|| panic!("no unfiltered_buffered_lines: {result}"));
    assert!(
        served > 500_000 && served < 1_000_000,
        "a cut at 4 MiB keeps most of ~6.9 MB, not all and not ~2,200 lines: served={served}"
    );
    let handle = result["unfiltered_output"].as_str().expect("handle");
    let stored = ctx.output_buffer.get(handle).expect("entry").stdout;
    assert!(
        stored
            .lines()
            .next_back()
            .is_some_and(|l| l.starts_with(crate::tools::output_buffer::TRUNCATION_SENTINEL_PREFIX)),
        "the marker travels with the data"
    );
}

/// Bug bc0cb248b224d1dd, at the new size: the scrub runs on the whole capture before the cut, so a
/// credential that now lands inside the kept 4 MiB (it sat past the old 10 KB cut) is redacted in
/// the handle, not merely absent from it.
#[cfg(unix)]
#[tokio::test]
async fn a_credential_in_a_large_capture_is_redacted_in_the_handle() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "{ seq 1 3000; printf 'ghp_%036d\\n' 0; seq 1 10; } | grep zzz" }),
            &ctx,
        )
        .await
        .unwrap();
    assert_eq!(result["stdout"], "", "the filter hides it inline: {result}");
    let handle = result["unfiltered_output"].as_str().expect("tee ref");
    let stored = ctx.output_buffer.get(handle).unwrap().stdout;
    assert!(
        stored.contains("<redacted-credential>"),
        "the credential sits past the old 10 KB cut and must be in the handle, redacted"
    );
    assert!(
        !stored.contains(&printed_token()),
        "a later `grep @cmd_*` must not be able to surface the token"
    );
    assert!(!result.to_string().contains(&printed_token()), "{result}");
    assert_eq!(result["redacted_credentials"], 1, "{result}");
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
    let line_count = 1_000_000;
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
    // Chained pipes off an unbounded LHS still block in the background (was originally
    // cat|grep|head; cat is now bounded — substitute cargo). Background, so it is refused
    // before anything runs; a foreground call would run `cargo test`.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo test | grep zzz | head -5", "run_in_background": true }),
            &ctx,
        )
        .await
        .expect_err("IL3 should block background `cargo test | grep | head`");
    assert!(err.to_string().contains("IL3 violation"));
}

#[tokio::test]
async fn il3_blocks_unbounded_pipe_pre_exec() {
    // IL3 fires before exec — verifies the gate does not depend on actual
    // output size. (Original test built a >MAX_INLINE_TOKENS file behind
    // `cat big.txt | grep`; cat is now bounded-LHS, so we use cargo as the
    // unconditionally-unbounded LHS.) Background, so it is refused before anything runs (a
    // foreground pipe ending in a terminal filter would run `cargo test`).
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo test | grep line0", "run_in_background": true }),
            &ctx,
        )
        .await
        .expect_err("IL3 should block regardless of payload size");
    assert!(err.to_string().contains("IL3 violation"));
}
#[cfg(unix)]
#[tokio::test]
async fn a_foreground_pipe_from_an_unbounded_producer_is_not_refused() {
    // `cargo` is on the gate's unbounded-producer list, and `head` is a terminal filter: the pipe
    // is run to the end through the capture and the full stream is kept as `unfiltered_output`,
    // so there is nothing for the gate to refuse. The producer is `cargo --version` on purpose: a
    // foreground `cargo test | grep` would run `cargo test` for real.
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "cargo --version | head -1", "timeout_secs": 20 }),
            &ctx,
        )
        .await
        .expect("a foreground pipe with a terminal filter is run, not refused");
    assert!(
        result["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("cargo"),
        "the command must have run; got: {result}"
    );
    assert!(
        result["unfiltered_output"].is_string(),
        "the full stream must be offered as a buffer; got: {result}"
    );
}

#[tokio::test]
async fn a_background_pipe_from_an_unbounded_producer_is_still_refused() {
    // A background run sends its stdout straight to a log and keeps no unfiltered stream, so a
    // pipe from an unbounded producer would lose what its filter drops. It is refused before
    // anything runs.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo --version | head -1", "run_in_background": true }),
            &ctx,
        )
        .await
        .expect_err("a background pipe from an unbounded producer must be refused");
    let msg = err.to_string();
    assert!(
        msg.starts_with("IL3 violation"),
        "missing IL3 marker: {msg}"
    );
    assert!(
        msg.contains("background"),
        "must say why it is background: {msg}"
    );
    assert!(msg.contains("@bg_"), "must name the buffer to query: {msg}");
}
#[cfg(unix)]
#[tokio::test]
async fn a_foreground_pipe_the_capture_cannot_serve_is_still_refused() {
    // The last stage (`cat`) is not a terminal filter, so no unfiltered capture is kept and the
    // `grep` in the middle would drop output for good: refused, as it always was. `cargo
    // --version` is the producer so that nothing here does real work if the gate lets it through.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo --version | grep cargo | cat", "timeout_secs": 20 }),
            &ctx,
        )
        .await
        .expect_err("a foreground pipe with no terminal filter last must be refused");
    let msg = err.to_string();
    assert!(
        msg.starts_with("IL3 violation"),
        "missing IL3 marker: {msg}"
    );
    assert!(
        msg.contains("unfiltered_output"),
        "must offer the route that keeps the stream: {msg}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_foreground_chain_ending_in_a_filter_is_not_refused() {
    // A chain whose last stage is a terminal filter is captured at its FIRST pipe, so the handle is
    // the producer's own stream: one line here, not what `grep` passed on.
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "cargo --version | grep cargo | head -1", "timeout_secs": 20 }),
            &ctx,
        )
        .await
        .expect("a foreground chain ending in a terminal filter is run, not refused");
    assert!(
        result["stdout"]
            .as_str()
            .unwrap_or_default()
            .contains("cargo"),
        "the command must have run; got: {result}"
    );
    assert_eq!(result["unfiltered_output_lines"], 1, "{result}");
}

#[tokio::test]
async fn a_background_pipe_is_refused_whatever_its_last_stage() {
    // A background run keeps no unfiltered stream even when the pipe ends in a terminal filter.
    let (_dir, ctx) = project_ctx().await;
    let err = RunCommand
        .call(
            json!({ "command": "cargo --version | grep cargo | head -1", "run_in_background": true }),
            &ctx,
        )
        .await
        .expect_err("a background pipe must be refused whatever its last stage");
    let msg = err.to_string();
    assert!(
        msg.starts_with("IL3 violation"),
        "missing IL3 marker: {msg}"
    );
    assert!(msg.contains("background"), "{msg}");
}

#[test]
fn pipe_refusal_decides_the_three_branches() {
    use super::pipe_refusal;
    // Background: the detector's verdict, whatever the last stage is.
    for command in ["cargo test | grep x | head -1", "cargo test | grep x | cat"] {
        let hint = pipe_refusal(command, true).expect(command);
        assert_eq!(
            Some(hint),
            crate::util::path_security::detect_il3_violation(command),
            "{command}"
        );
    }
    // Foreground, the capture cannot serve it: refused, and the text says why and names the
    // segment and both routes.
    let hint = pipe_refusal("cargo test | grep x | cat", false).expect("refused");
    assert!(hint.starts_with("IL3 violation"), "{hint}");
    assert!(hint.contains("`cargo test | grep x | cat`"), "{hint}");
    assert!(hint.contains("terminal filter"), "{hint}");
    assert!(hint.contains("unfiltered_output"), "{hint}");
    assert!(hint.contains("run_command(\"cargo test\")"), "{hint}");
    assert!(hint.contains("@cmd_"), "{hint}");
    // `|&` pipes stderr too, and the capture leaves such a command untouched: still refused.
    assert!(pipe_refusal("cargo test |& grep x | head -1", false).is_some());
    // Foreground, the capture serves it: not refused.
    assert_eq!(pipe_refusal("cargo test | grep x | head -1", false), None);
    assert_eq!(pipe_refusal("cargo test | grep FAILED", false), None);
    // R15: a `;` tail after the first pipe means no splice and no fallback, so it is refused.
    assert!(pipe_refusal("cargo test | grep x | head -1; echo done", false).is_some());
    // R16: a buffer ref disables the capture (`resolve_refs` may mark the call `buffer_only`), so the
    // detector firing on another segment still refuses in the foreground.
    assert!(pipe_refusal("cargo --version | grep x | head -1; grep y @cmd_abc", false).is_some());
    // Nothing for the detector to refuse: not refused in either mode.
    assert_eq!(pipe_refusal("ls | head -1", false), None);
    assert_eq!(pipe_refusal("ls | head -1", true), None);
}

#[test]
fn pipe_refusal_names_every_terminal_filter_the_capture_knows() {
    // The foreground refusal lists the terminal filters in prose; the capture decides from
    // `detect_terminal_filter`. Every listed name must be one the capture accepts, and the list
    // must be the whole set (13).
    let listed: Vec<&str> = super::TERMINAL_FILTER_NAMES.split(", ").collect();
    assert_eq!(listed.len(), 13, "{listed:?}");
    for name in &listed {
        assert!(
            crate::tools::command_summary::detect_terminal_filter(&format!("x | {name} a"))
                .is_some(),
            "`{name}` is listed but is not a terminal filter"
        );
    }
    assert!(
        crate::tools::command_summary::detect_terminal_filter("x | cat").is_none(),
        "the premise: `cat` is not one"
    );
}
/// A command with a pipe nested inside `( )`, `$( )`, `$(( ))` or `<( )` must run exactly as it does
/// bare. The splice used to land on the nested pipe and turn it into a syntax error.
#[cfg(unix)]
#[tokio::test]
async fn a_pipe_nested_in_a_group_or_substitution_runs_as_it_does_bare() {
    let (_dir, ctx) = project_ctx().await;
    for (command, want_stdout) in [
        ("(seq 1 5 | grep 2) | head -1", "2\n"),
        ("echo $(seq 1 5 | wc -l) | grep 5 | head -1", "5\n"),
        ("echo $((1|2)) | grep 3 | head -1", "3\n"),
        ("diff <(seq 3 | sort) <(seq 3) | head -1", ""),
        ("{ seq 1 5 | grep 4; } | head -1", "4\n"),
    ] {
        let result = RunCommand
            .call(json!({ "command": command, "timeout_secs": 20 }), &ctx)
            .await
            .unwrap_or_else(|e| panic!("`{command}` must run, not be refused: {e}"));
        assert_eq!(result["exit_code"], 0, "`{command}`: {result}");
        assert_eq!(
            result["stdout"].as_str().unwrap_or_default(),
            want_stdout,
            "`{command}`: {result}"
        );
    }
}

/// With no pipe at nesting depth 0 there is nothing to splice at: the command runs untouched, so
/// it has no `unfiltered_output`. (The splice used to land inside the `$( )`.)
#[cfg(unix)]
#[tokio::test]
async fn a_command_whose_only_pipe_is_nested_runs_untouched() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "echo $(seq 3 | head -1)", "timeout_secs": 20 }),
            &ctx,
        )
        .await
        .expect("a nested pipe is run, not refused");
    assert_eq!(result["exit_code"], 0, "{result}");
    assert_eq!(
        result["stdout"].as_str().unwrap_or_default(),
        "1\n",
        "{result}"
    );
    assert!(
        result.get("unfiltered_output").is_none(),
        "nothing was spliced, so no capture may be offered; got: {result}"
    );
}

/// A compound whose FIRST pipe is at depth 0 is captured at that pipe: the handle holds the
/// producer's own stream (1000 lines), not what `grep` passed on.
#[cfg(unix)]
#[tokio::test]
async fn a_compound_with_a_depth_zero_pipe_is_captured_at_its_first_pipe() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(
            json!({ "command": "cd /tmp && seq 1 1000 | grep 7 | head -2", "timeout_secs": 20 }),
            &ctx,
        )
        .await
        .expect("a compound ending in a filter is run");
    assert_eq!(
        result["stdout"].as_str().unwrap_or_default(),
        "7\n17\n",
        "{result}"
    );
    assert_eq!(result["unfiltered_output_lines"], 1000, "{result}");
}

/// R15: when the splice conditions do not hold, the command is NOT captured, and the foreground
/// refusal applies if the detector fires. A `;` tail after the pipe made the old fallback capture a
/// different segment's stream while the first command's trimmed output was lost.
#[cfg(unix)]
#[tokio::test]
async fn a_foreground_pipe_with_a_list_tail_is_refused_not_captured_elsewhere() {
    let (_dir, ctx) = project_ctx().await;
    for command in [
        "cargo --version | grep cargo | head -1; echo done",
        "cargo --version | grep cargo && ls | head -1",
        "cargo --version | grep cargo | cat; echo | head -1",
    ] {
        let err = RunCommand
            .call(json!({ "command": command, "timeout_secs": 20 }), &ctx)
            .await
            .expect_err(command);
        assert!(
            err.to_string().starts_with("IL3 violation"),
            "`{command}`: {err}"
        );
    }
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

            // FOLLOW the hint's route for a wide line. `cat` is a slice and its route names the
            // handle; `grep -v` is not, and its route appends to the query.
            let hint = parsed["hint"].as_str().unwrap_or_default();
            let route = if query == "cat" {
                assert!(
                    hint.contains(&format!("cut -c1-4000 {id}")),
                    "{what}: the hint names no window route: {hint}"
                );
                format!("cut -c1-4000 {id}")
            } else {
                assert!(
                    hint.contains("appending `| cut -c1-4000` to this query"),
                    "{what}: the hint names no window route: {hint}"
                );
                format!("{query} {id} | cut -c1-4000")
            };
            let (t2, p2) = buffer_query(&ctx, route).await;
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
                    ..Default::default()
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
/// A libtest run whose compacted response carries 25 stderr warning lines, more than the
/// summarizer's 20-line stderr budget, so the summarizer records `stderr` as cut. `pad` extra bytes
/// go into the one test's name, which the compacted stdout keeps byte for byte; the quotes double
/// when serialized, and the empty targets are what makes the compaction pay.
fn warned_libtest_run(pad: usize) -> (String, String) {
    let stderr = (0..25)
        .map(|i| format!("warning: unused {i:02} {}\n", "\"".repeat(170)))
        .collect();
    (libtest_run(40, pad), stderr)
}

#[cfg(unix)]
#[tokio::test]
async fn a_compacted_run_is_judged_without_the_cut_record_it_never_delivers() {
    // The summarizer's record (`_cut_fields: ["stderr"]`) survived compaction, which REPLACES that
    // `stderr`, and the gate counted it: 25 B the caller never receives. A compacted response
    // within 25 B of the limit was therefore summarized, though delivered it fits. Swept across
    // that whole window through the real tool, plus one byte over.
    use super::output::{handle_successful_output_with, LateKeys};
    const RECORD: &str = r#","_cut_fields":["stderr"]"#;
    const CMD: &str = ": cargo test ; cat out.txt; cat err.txt >&2";
    let limit = crate::tools::INLINE_MAX_RESPONSE_LEN;
    let (dir, ctx) = project_ctx().await;
    let is_compacted = |r: &Value| {
        r["stdout"]
            .as_str()
            .is_some_and(|s| s.contains("codescout compacted this run"))
    };
    let write = |pad: usize| {
        let (stdout, stderr) = warned_libtest_run(pad);
        assert!(
            stdout.len() + stderr.len() <= limit,
            "fixture: the raw run must fit, or compaction is never considered"
        );
        std::fs::write(dir.path().join("out.txt"), &stdout).unwrap();
        std::fs::write(dir.path().join("err.txt"), &stderr).unwrap();
    };

    // Control: far enough under the limit to compact on any version of the gate.
    let (stdout, stderr) = warned_libtest_run(0);
    let control = handle_successful_output_with(
        CMD,
        stdout,
        stderr,
        0,
        false,
        None,
        dir.path(),
        &ctx,
        LateKeys::default(),
    )
    .await
    .unwrap();
    assert!(is_compacted(&control), "precondition: {control:.300}");
    let d0 = crate::tools::delivered_len(&control);
    assert!(
        d0 + RECORD.len() <= limit,
        "the control must compact even counting the record: {d0} B"
    );
    let need = limit - d0;

    // Warm the guide ledger through the real tool, so no `_guide_hint` lands in a measured block.
    write(0);
    let (warm, _) = buffer_query_free(&ctx, CMD).await;
    assert!(!has_tool_handle(&warm), "{warm:.300}");

    for k in 0..RECORD.len() {
        write(need - k);
        let (text, parsed) = buffer_query_free(&ctx, CMD).await;
        assert!(
            is_compacted(&parsed),
            "k={k}: delivered {} B, {} B counting the record: summarized instead of compacted: \
             {text:.200}",
            limit - k,
            limit - k + RECORD.len()
        );
        assert_eq!(text.len(), limit - k, "k={k}");
        assert!(
            text.len() + RECORD.len() > limit,
            "k={k}: inside the window"
        );
        assert!(!has_tool_handle(&text), "k={k}: one handle: {text:.200}");
        assert!(
            parsed["output_id"]
                .as_str()
                .is_some_and(|i| i.starts_with("@cmd_")),
            "k={k}: {text:.200}"
        );
        assert!(parsed.get(crate::tools::CUT_FIELDS_KEY).is_none(), "k={k}");
        assert!(
            parsed.get("_guide_hint").is_none(),
            "k={k}: ledger not warm"
        );
    }

    write(need + 1);
    let (text, parsed) = buffer_query_free(&ctx, CMD).await;
    assert!(
        !is_compacted(&parsed),
        "one byte over the limit must not be returned compacted: {} B",
        text.len()
    );
    assert!(!has_tool_handle(&text), "{text:.200}");
    assert!(text.len() <= limit, "{} B", text.len());
    assert!(parsed.get(crate::tools::CUT_FIELDS_KEY).is_none());
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
            ..Default::default()
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
    // Measured as delivered: `call_content`'s backstop strips the cut record first. The record
    // itself must name both streams this summary cut, so the backstop spares them.
    assert_eq!(
        response[crate::tools::CUT_FIELDS_KEY],
        serde_json::json!(["stdout", "stderr"]),
        "{response:.200}"
    );
    let len = crate::tools::delivered_len(&response);
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

// ---- a truncated buffer's notice is counted by the gate that sizes the query ----
//
// `buffer_truncated` (about 280 B) was attached by `RunCommand::call` AFTER
// `handle_successful_output_with` had sized the response, so a query of a truncated `@cmd_*`
// buffer whose answer the gate judged to fit went over the inline limit by the notice, and
// `call_content` re-buffered it under a second handle. `timeout_hint` was attached the same way.

/// The seven escape classes the inline limit counts differently: 1, 2, 2, 6 and 6 serialized
/// bytes per char (ESC also meets the ANSI strip a buffer query applies), and two multibyte chars
/// serde_json writes raw.
// cap-class: NOT_A_CAP — a test fixture listing escape classes, not a bound on any result
const NOTICE_UNITS: [&str; 7] = ["a", "\"", "\\", "\u{1}", "\u{1b}", "\u{20ac}", "\u{1F600}"];

/// A 30-line `@cmd_*` buffer whose payload serializes to about `target` bytes, stored as a PREFIX
/// of a ten-times-longer capture so that every read of it carries `buffer_truncated`.
fn truncated_cmd_buffer(ctx: &ToolContext, unit: &str, target: usize) -> String {
    let per_line = target / 30;
    let line = unit.repeat((per_line.saturating_sub(2) / escaped_unit_len(unit)).max(1));
    let body = format!("{}\n", vec![line; 30].join("\n"));
    ctx.output_buffer.store_truncated(
        "probe".into(),
        body,
        String::new(),
        0,
        Some(crate::tools::output_buffer::Truncation {
            kept_lines: 30,
            total_lines: 300,
        }),
    )
}

#[cfg(unix)]
#[tokio::test]
async fn a_truncated_buffer_counts_its_notice_in_every_query() {
    let (_dir, ctx) = project_ctx().await;
    let (mut whole, mut cut, mut largest) = (0, 0, 0);
    let mut failures = Vec::new();
    for unit in NOTICE_UNITS {
        for target in (9_300..=10_500).step_by(50) {
            let id = truncated_cmd_buffer(&ctx, unit, target);
            for (command, timeout) in [
                (format!("cat {id}"), 10),
                (format!("sed -n '1,30p' {id}"), 10),
                (format!("grep -v NO_SUCH_LINE {id}"), 10),
                // `timeout_secs: 0` is corrected to 30 and says so in `timeout_hint`, another key
                // `call` attaches to every response shape.
                (format!("cat {id}"), 0),
            ] {
                let label = format!("{unit:?} {target} `{command}` timeout={timeout}");
                let (text, parsed) =
                    run_query(&ctx, json!({ "command": command, "timeout_secs": timeout })).await;
                let named = handles_named(&text);
                let notice = parsed["buffer_truncated"][0]
                    .as_str()
                    .is_some_and(|n| n.contains(&id));
                let hint_ok = timeout != 0 || parsed.get("timeout_hint").is_some();
                if has_tool_handle(&text)
                    || named.iter().any(|h| *h != id)
                    || parsed.get("output_id").is_some()
                    || text.len() > crate::tools::INLINE_MAX_RESPONSE_LEN
                    || !notice
                    || !hint_ok
                {
                    failures.push(format!(
                        "{label}: {} B, buffered_bytes {}: {text:.200}",
                        text.len(),
                        parsed["buffered_bytes"]
                    ));
                    continue;
                }
                largest = largest.max(text.len());
                if parsed.get("truncated").is_some() {
                    cut += 1;
                } else {
                    whole += 1;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} queries broke the one-handle limit; first: {:#?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
    // Both arms reached, or the sweep is not across the edge.
    assert!(whole > 0 && cut > 0, "whole {whole}, cut {cut}");
    eprintln!("notice sweep: whole {whole}, cut {cut}, largest response {largest} B");
}

// ---- a refreshed `@file_*` handle's `↻` line is priced by the gate ----
//
// `RunCommand::call` prepended `↻ <handle> refreshed from disk …` to `stdout` AFTER the gate had
// sized the response, the same defect as the notice above in a different channel: a query of a
// stale `@file_*` handle that exactly fit went over the limit by that line (about 70 B).

#[cfg(unix)]
#[tokio::test]
async fn a_refreshed_file_handle_prices_its_refresh_line_in_every_query() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("stale.txt");
    let (mut whole, mut cut, mut summarized, mut largest) = (0, 0, 0, 0);
    let mut failures = Vec::new();
    for unit in NOTICE_UNITS {
        for target in (9_300..=10_500usize).step_by(50) {
            let line =
                unit.repeat(((target / 30).saturating_sub(2) / escaped_unit_len(unit)).max(1));
            let body = format!("{}\n", vec![line; 30].join("\n"));
            for shape in ["cat", "sed", "grep", "mixed"] {
                // A fresh stale handle per query: a refresh re-reads the file and is then current.
                std::fs::write(&path, &body).unwrap();
                let id = ctx
                    .output_buffer
                    .store_file(path.to_string_lossy().to_string(), "original".into());
                let future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
                filetime::set_file_mtime(&path, filetime::FileTime::from_system_time(future))
                    .unwrap();
                let command = match shape {
                    "cat" => format!("cat {id}"),
                    "sed" => format!("sed -n '1,30p' {id}"),
                    "grep" => format!("grep -v NO_SUCH_LINE {id}"),
                    // Not a buffer query (a path-like word revokes buffer-only status): summarized
                    // under its own `@cmd_*`, `stdout` and all.
                    _ => format!("cat {id}; seq 1 4000; true ./not-a-buffer"),
                };
                let label = format!("{unit:?} {target} `{command}`");
                let (text, parsed) = buffer_query(&ctx, command).await;
                let own_id = parsed["output_id"].as_str().unwrap_or_default();
                let refreshed = parsed["stdout"]
                    .as_str()
                    .is_some_and(|s| s.starts_with(&format!("↻ {id} refreshed from disk")));
                if has_tool_handle(&text)
                    || !(own_id.is_empty() || (shape == "mixed" && own_id.starts_with("@cmd_")))
                    || text.len() > crate::tools::INLINE_MAX_RESPONSE_LEN
                    || !refreshed
                {
                    failures.push(format!(
                        "{label}: {} B, buffered_bytes {}: {text:.200}",
                        text.len(),
                        parsed["buffered_bytes"]
                    ));
                    continue;
                }
                largest = largest.max(text.len());
                if !own_id.is_empty() {
                    summarized += 1;
                } else if parsed.get("truncated").is_some() {
                    cut += 1;
                } else {
                    whole += 1;
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} queries broke the one-handle limit; first: {:#?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
    assert!(
        whole > 0 && cut > 0 && summarized > 0,
        "whole {whole}, cut {cut}, summarized {summarized}"
    );
    eprintln!(
        "refresh sweep: whole {whole}, cut {cut}, summarized {summarized}, largest {largest} B"
    );
}

// ---- a libtest run naming a stale `@file_*` handle is not compacted out of its refresh line ----
//
// `compacted_fits` does not price `LateKeys::stdout_prefix`, and the compacted arm does not
// prepend it, so such a run must take the raw gate. Without that guard the refresh line was simply
// absent from a compacted response.

/// `blocks` empty libtest targets (each dropped by compaction) and one passing test: the shape a
/// filtered `cargo test` prints, as a file so its path makes the command not a buffer query.
fn libtest_noise(blocks: usize) -> String {
    let empty = "running 0 tests\n\ntest result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; \
                 3 filtered out; finished in 0.00s\n\n";
    format!(
        "{}running 1 test\ntest a ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; \
         0 measured; 0 filtered out; finished in 0.00s\n\n",
        empty.repeat(blocks)
    )
}

/// Store `body` behind a `@file_*` handle that is stale (the file is newer than the entry), so the
/// next command naming it re-reads it and says so.
fn stale_file_handle(ctx: &ToolContext, path: &std::path::Path, body: &str) -> String {
    std::fs::write(path, body).unwrap();
    let id = ctx
        .output_buffer
        .store_file(path.to_string_lossy().to_string(), "original".into());
    let future = std::time::SystemTime::now() + std::time::Duration::from_secs(3600);
    filetime::set_file_mtime(path, filetime::FileTime::from_system_time(future)).unwrap();
    id
}

#[cfg(unix)]
#[tokio::test]
async fn a_compactable_test_run_keeps_the_refresh_line_of_a_stale_handle() {
    let (dir, ctx) = project_ctx().await;
    std::fs::write(dir.path().join("noise.txt"), libtest_noise(60)).unwrap();
    let id = stale_file_handle(&ctx, &dir.path().join("stale.txt"), "PAYLOAD\n");
    let (text, parsed) = buffer_query(&ctx, format!("cat {id} ./noise.txt; echo cargo test")).await;
    let stdout = parsed["stdout"].as_str().unwrap_or_default();
    assert!(
        stdout.starts_with(&format!("↻ {id} refreshed from disk")),
        "the refresh line is missing: {text:.400}"
    );
    assert!(stdout.contains("PAYLOAD"), "{text:.400}");
    assert!(!has_tool_handle(&text), "{text:.300}");
}

// The same command across the response's byte edge, in the escape classes, with the payload behind
// the stale handle growing: one handle at most, no response over the limit, the line always shown.
#[cfg(unix)]
#[tokio::test]
async fn a_test_run_naming_a_stale_handle_keeps_one_handle_across_the_byte_edge() {
    let (dir, ctx) = project_ctx().await;
    let noise = libtest_noise(20);
    let noise_cost = serde_json::to_string(&noise).unwrap().len() - 2;
    std::fs::write(dir.path().join("noise.txt"), &noise).unwrap();
    let path = dir.path().join("stale.txt");
    let (mut inline, mut summarized, mut largest) = (0, 0, 0);
    let mut failures = Vec::new();
    for unit in ["a", "\"", "\u{1}"] {
        for total in (9_300..=10_500usize).step_by(50) {
            let width = (total.saturating_sub(noise_cost) / 30).saturating_sub(2);
            let line = unit.repeat((width / escaped_unit_len(unit)).max(1));
            let id = stale_file_handle(&ctx, &path, &format!("{}\n", vec![line; 30].join("\n")));
            let command = format!("cat {id} ./noise.txt; echo cargo test");
            let (text, parsed) = buffer_query(&ctx, command).await;
            let own_id = parsed["output_id"].as_str().unwrap_or_default();
            let refreshed = parsed["stdout"]
                .as_str()
                .is_some_and(|s| s.starts_with(&format!("↻ {id} refreshed from disk")));
            if has_tool_handle(&text)
                || !(own_id.is_empty() || own_id.starts_with("@cmd_"))
                || text.len() > crate::tools::INLINE_MAX_RESPONSE_LEN
                || !refreshed
            {
                failures.push(format!("{unit:?} {total}: {} B: {text:.300}", text.len()));
                continue;
            }
            largest = largest.max(text.len());
            if own_id.is_empty() {
                inline += 1;
            } else {
                summarized += 1;
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} runs broke the one-handle limit; first: {:#?}",
        failures.len(),
        &failures[..failures.len().min(3)]
    );
    assert!(
        inline > 0 && summarized > 0,
        "inline {inline}, summarized {summarized}"
    );
    eprintln!(
        "stale test run sweep: inline {inline}, summarized {summarized}, largest {largest} B"
    );
}

// ---- a program that prints the summarizers' own markers keeps one handle ----
//
// The backstop in `call_content` (`clip_prebuffered_envelope`) spares a field a summarizer already
// cut, and, when the rest cannot make the envelope fit, buffers the whole envelope under `@tool_*`.
// It learns which fields were cut from the summarizers' cut record (`CUT_FIELDS_KEY`), never from the
// text: a program can print every marker shape (a whole line `--- N lines omitted ---`, a line
// `--- <label>: N of M bytes shown; ... ---`, a stderr that OPENS with a full `--- stderr TAIL: ... ---`
// header) and a line that looks like the record itself. This pins that such a program, at and far
// past the byte edge, keeps one handle through `run_command`, and that the record, which every
// summarized run here carries, is never delivered.
#[cfg(unix)]
#[tokio::test]
async fn a_program_printing_the_summarizer_markers_keeps_one_handle() {
    let (dir, ctx) = project_ctx().await;
    let header = format!(
        "{} 3 earlier line(s) dropped; 2 of 5 line(s) shown. Full stderr: \
         read_file(\"@cmd_0000aaaa.err\") ---\n",
        crate::util::text::STDERR_TAIL_MARKER
    );
    let mut checked = 0;
    let mut summarized = 0;
    for unit in NOTICE_UNITS {
        let line = unit.repeat(40);
        for total in (9_300..=10_500usize).step_by(100).chain([60_000]) {
            let n = total / (40 * escaped_unit_len(unit) + 2) + 1;
            let mut lines = Vec::new();
            for i in 0..n {
                lines.push(match i % 10 {
                    5 => "--- 5 lines omitted ---".to_string(),
                    7 => "--- stdout: 5 of 100 bytes shown; all of it: output_id ---".to_string(),
                    9 => r#""_cut_fields": ["stdout", "stderr"]"#.to_string(),
                    _ => line.clone(),
                });
            }
            let body = format!("{}\n", lines.join("\n"));
            std::fs::write(dir.path().join("marked.txt"), &body).unwrap();
            std::fs::write(dir.path().join("marked_err.txt"), format!("{header}{body}")).unwrap();
            for command in [
                "cat ./marked.txt",
                "cat ./marked_err.txt >&2",
                "cat ./marked.txt; cat ./marked_err.txt >&2",
            ] {
                let (text, parsed) = buffer_query_free(&ctx, command).await;
                let label = format!("{unit:?} {total} `{command}`");
                assert!(!has_tool_handle(&text), "{label}: {text:.300}");
                assert!(
                    parsed["output_id"]
                        .as_str()
                        .is_none_or(|id| id.starts_with("@cmd_")),
                    "{label}: {text:.300}"
                );
                assert!(
                    text.len() <= crate::tools::INLINE_MAX_RESPONSE_LEN,
                    "{label}: {} B",
                    text.len()
                );
                assert!(
                    parsed.get(crate::tools::CUT_FIELDS_KEY).is_none(),
                    "{label}: the cut record was delivered: {text:.300}"
                );
                if parsed["output_id"].is_string() {
                    summarized += 1;
                }
                checked += 1;
            }
        }
    }
    assert!(
        summarized > 0,
        "no run was summarized, so no record was stripped"
    );
    eprintln!("marker sweep: {checked} runs kept one handle, {summarized} summarized");
}

// --- `jobs` and the response shapes the inline-or-summary gate never measured -------------------

/// The seven content classes of every byte-bound sweep. Per character they are 1, 1, 1, 1, 1, 3
/// and 4 raw bytes and serialize to 1, 2, 2, 6, 6, 3 and 4 bytes.
const LATE_RC_CLASSES: [&str; 7] = ["a", "\"", "\\", "\u{1}", "\u{1b}", "€", "😀"];

/// (jobs, raw bytes of each job's command): three short ones, three of 4 KB, and fifty of 4 KB. The
/// buffer keeps the newest 20 jobs live, so the fifty-job command names 20 jobs `jobs` can report.
const LATE_RC_SIZES: [(usize, usize); 3] = [(3, 100), (3, 4096), (50, 4096)];

/// The response shapes `run_command` builds around a command that names `@bg_*` handles.
#[derive(Clone, Copy, Debug)]
enum JobsShape {
    /// Ran to completion: built by `handle_successful_output_with`.
    Foreground,
    /// A dangerous command held for acknowledgement: the `pending_ack` shape.
    PendingAck,
    /// Killed at `timeout_secs`: the `timed_out` shape.
    Timeout,
    /// `run_in_background`: the spawn-time `output_id` shape.
    Background,
}

impl JobsShape {
    fn input(self, refs: &str) -> Value {
        match self {
            JobsShape::Foreground => json!({ "command": format!("echo hi {refs}") }),
            JobsShape::PendingAck => {
                json!({ "command": format!("rm -rf /tmp/late-rc-nonexistent-dir {refs}") })
            }
            JobsShape::Timeout => {
                json!({ "command": format!("sleep 3; : /dev/null {refs}"), "timeout_secs": 1 })
            }
            JobsShape::Background => json!({
                "command": format!(": /dev/null {refs}"),
                "run_in_background": true,
            }),
        }
    }

    /// The key that says what happened, which the DELIVERED response must carry at top level.
    fn status_failure(self, parsed: &Value) -> Option<String> {
        let ok = match self {
            JobsShape::Foreground => parsed["exit_code"] == 0,
            JobsShape::PendingAck => parsed["pending_ack"]
                .as_str()
                .is_some_and(|h| h.starts_with("@ack_")),
            JobsShape::Timeout => parsed["timed_out"] == true,
            JobsShape::Background => parsed["output_id"]
                .as_str()
                .is_some_and(|h| h.starts_with("@bg_")),
        };
        (!ok).then(|| "its status key is not in the delivered response".to_string())
    }
}

/// Store `n` running background jobs whose commands are `unit` repeated to at most `size` raw
/// bytes, each with a real log file under `dir`. Returns the handles in store order.
fn store_jobs(
    ctx: &ToolContext,
    dir: &std::path::Path,
    n: usize,
    unit: &str,
    size: usize,
) -> Vec<String> {
    (0..n)
        .map(|i| {
            let log_path = dir.join(format!("late-rc-job-{i}.log"));
            std::fs::write(&log_path, "log line\n").unwrap();
            ctx.output_buffer
                .store_background(crate::tools::output_buffer::BackgroundJob {
                    log_path,
                    command: unit.repeat(size / unit.len()),
                    state: crate::tools::output_buffer::JobState::Running,
                })
        })
        .collect()
}

/// What a DELIVERED response naming background jobs must hold, as failure lines (none when it
/// holds), so a sweep reports every failing case with its bytes instead of the first.
fn jobs_response_failures(
    label: &str,
    shape: JobsShape,
    text: &str,
    parsed: &Value,
    ctx: &ToolContext,
    handles: &[String],
) -> Vec<String> {
    let bytes = text.len();
    let mut out = Vec::new();
    if bytes > 10_003 {
        out.push(format!(
            "{label}: delivered {bytes} B, over the 10,003 B limit"
        ));
    }
    if text.contains("@tool_") {
        out.push(format!(
            "{label}: a @tool_* handle was minted ({bytes} B delivered, {} B buffered)",
            parsed["buffered_bytes"]
        ));
    }
    let actionable = ["output_id", "pending_ack"]
        .iter()
        .filter(|k| parsed.get(**k).is_some())
        .count();
    if actionable > 1 {
        out.push(format!("{label}: {actionable} actionable handles"));
    }
    if let Some(why) = shape.status_failure(parsed) {
        out.push(format!("{label}: {why} ({bytes} B delivered)"));
    }
    if !out.is_empty() {
        return out;
    }
    // Every live job the command names is either listed or counted as not listed.
    let live = handles
        .iter()
        .filter(|h| ctx.output_buffer.get_background(h).is_some())
        .count();
    let listed = parsed["jobs"].as_array().cloned().unwrap_or_default();
    let not_listed = parsed["jobs_not_listed"]
        .as_str()
        .map(|s| {
            s.split(' ')
                .next()
                .and_then(|n| n.parse::<usize>().ok())
                .unwrap_or_else(|| panic!("{label}: `jobs_not_listed` leads with no count: {s}"))
        })
        .unwrap_or(0);
    let shed = parsed["envelope_omitted"]
        .as_str()
        .is_some_and(|s| s.contains("jobs"));
    if listed.len() > JOBS_LISTED_MAX {
        out.push(format!(
            "{label}: {} jobs listed, over JOBS_LISTED_MAX {JOBS_LISTED_MAX}",
            listed.len()
        ));
    }
    if !shed && listed.len() + not_listed != live {
        out.push(format!(
            "{label}: {} listed + {not_listed} not listed != {live} live jobs named",
            listed.len()
        ));
    }
    for job in &listed {
        let handle = job["handle"].as_str().unwrap_or_default();
        let Some(stored) = ctx.output_buffer.get_background(handle) else {
            out.push(format!("{label}: listed {handle}, which is not a live job"));
            continue;
        };
        let shown = job["command"].as_str().unwrap_or_default();
        if shown != stored.command
            && !shown.contains("bytes shown; the rest is the command that started this job")
        {
            out.push(format!(
                "{label}: {handle}'s command was cut with no marker"
            ));
        }
        let escaped = crate::util::text::json_escaped_len(shown);
        if escaped > JOB_COMMAND_BYTES {
            out.push(format!(
                "{label}: {handle}'s command is {escaped} escaped bytes, over {JOB_COMMAND_BYTES}"
            ));
        }
    }
    out
}

/// Run every class x size of `shape` through the real tool; the failure lines of every case.
async fn jobs_sweep(shape: JobsShape) -> Vec<String> {
    let (dir, ctx) = project_ctx().await;
    let mut failures = Vec::new();
    for unit in LATE_RC_CLASSES {
        for (n, size) in LATE_RC_SIZES {
            // A command can only name a LIVE job (an evicted handle is refused before anything
            // runs), so of the fifty stored it names the twenty the buffer keeps.
            let handles: Vec<String> = store_jobs(&ctx, dir.path(), n, unit, size)
                .into_iter()
                .filter(|h| ctx.output_buffer.get_background(h).is_some())
                .collect();
            let label = format!("{shape:?} {}x{size} B of {unit:?}", handles.len());
            let (text, parsed) = run_query(&ctx, shape.input(&handles.join(" "))).await;
            eprintln!("{label}: {} B delivered", text.len());
            failures.extend(jobs_response_failures(
                &label, shape, &text, &parsed, &ctx, &handles,
            ));
        }
    }
    failures
}

#[cfg(unix)]
#[tokio::test]
async fn a_foreground_run_naming_many_large_jobs_stays_inline() {
    let failures = jobs_sweep(JobsShape::Foreground).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_pending_ack_naming_many_large_jobs_delivers_its_ack_handle() {
    let failures = jobs_sweep(JobsShape::PendingAck).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_timeout_naming_many_large_jobs_delivers_its_timed_out_status() {
    let failures = jobs_sweep(JobsShape::Timeout).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_background_spawn_naming_many_large_jobs_stays_inline() {
    let failures = jobs_sweep(JobsShape::Background).await;
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// `jobs` lists the first `JOBS_LISTED_MAX` jobs in the command's order and counts the rest in
/// `jobs_not_listed`, whose route, parsed out of the note and run, lists each omitted job.
#[cfg(unix)]
#[tokio::test]
async fn jobs_past_the_cap_are_counted_and_listed_by_the_named_route() {
    let (dir, ctx) = project_ctx().await;
    let handles = store_jobs(&ctx, dir.path(), JOBS_LISTED_MAX + 4, "a", 100);
    let (text, parsed) = run_query(
        &ctx,
        json!({ "command": format!("echo hi {}", handles.join(" ")) }),
    )
    .await;
    let listed: Vec<&str> = parsed["jobs"]
        .as_array()
        .unwrap_or_else(|| panic!("no jobs: {text:.400}"))
        .iter()
        .map(|j| j["handle"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        listed,
        handles[..JOBS_LISTED_MAX],
        "the first jobs, in order"
    );
    let note = parsed["jobs_not_listed"]
        .as_str()
        .unwrap_or_else(|| panic!("no jobs_not_listed: {text:.400}"));
    let total = JOBS_LISTED_MAX + 4;
    assert!(
        note.starts_with(&format!(
            "4 more of the {total} jobs this command names are not listed"
        )),
        "{note}"
    );
    assert!(
        note.contains(&format!(
            "those named after {}",
            handles[JOBS_LISTED_MAX - 1]
        )),
        "{note}"
    );
    let route = regex::Regex::new(r#"run_command\("([^"]+)"\)"#)
        .expect("static pattern")
        .captures(note)
        .unwrap_or_else(|| panic!("no route in the note: {note}"))[1]
        .to_string();
    assert_eq!(route, "true @bg_…");
    for omitted in &handles[JOBS_LISTED_MAX..] {
        let (text, parsed) =
            run_query(&ctx, json!({ "command": route.replace("@bg_…", omitted) })).await;
        assert_eq!(parsed["exit_code"], 0, "the route ran: {text:.400}");
        assert_eq!(
            parsed["jobs"],
            json!([{"handle": omitted, "state": "running", "command": "a".repeat(100)}]),
            "the route lists the omitted job whole: {text:.400}"
        );
    }
}

/// A job's `command` is cut in ESCAPED bytes, middle elided behind the marker, head and tail kept.
#[cfg(unix)]
#[tokio::test]
async fn a_job_command_is_clipped_in_escaped_bytes_with_a_marker() {
    let (dir, ctx) = project_ctx().await;
    for unit in LATE_RC_CLASSES {
        let log_path = dir.path().join("late-rc-job-clip.log");
        std::fs::write(&log_path, "log line\n").unwrap();
        let id = ctx
            .output_buffer
            .store_background(crate::tools::output_buffer::BackgroundJob {
                log_path,
                command: format!("HEAD{}TAIL", unit.repeat(4096 / unit.len())),
                state: crate::tools::output_buffer::JobState::Running,
            });
        let (text, parsed) = run_query(&ctx, json!({ "command": format!("echo hi {id}") })).await;
        let shown = parsed["jobs"][0]["command"].as_str().unwrap_or_default();
        assert!(
            shown.contains("bytes shown; the rest is the command that started this job"),
            "{unit:?}: no marker: {text:.400}"
        );
        assert!(
            shown.starts_with("HEAD") && shown.ends_with("TAIL"),
            "{unit:?}: head or tail lost: {shown}"
        );
        let escaped = crate::util::text::json_escaped_len(shown);
        assert!(
            escaped <= JOB_COMMAND_BYTES && escaped + 12 >= JOB_COMMAND_BYTES,
            "{unit:?}: {escaped} escaped bytes against a budget of {JOB_COMMAND_BYTES}"
        );
    }
}

/// Every shape, over the limit by a key that is not `jobs`: one `buffer_truncated` notice per
/// truncated buffer the command names. At the server's buffer capacity (50) the notices alone are
/// over the limit, so the foreground gate cannot fit them by cutting stdout and the other shapes
/// were never measured; only shedding them, not the bound on `jobs`, keeps the status key and the
/// jobs in front of the caller.
#[cfg(unix)]
#[tokio::test]
async fn a_shape_over_the_limit_by_buffer_notices_keeps_its_status() {
    let (dir, mut ctx) = project_ctx().await;
    ctx.output_buffer = std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(50));
    let cmd_refs: Vec<String> = (0..45)
        .map(|_| {
            ctx.output_buffer.store_truncated(
                "cmd".into(),
                "kept\n".into(),
                String::new(),
                0,
                Some(crate::tools::output_buffer::Truncation {
                    kept_lines: 1,
                    total_lines: 1_000_000,
                }),
            )
        })
        .collect();
    let mut failures = Vec::new();
    for shape in [
        JobsShape::Foreground,
        JobsShape::PendingAck,
        JobsShape::Timeout,
        JobsShape::Background,
    ] {
        let jobs = store_jobs(&ctx, dir.path(), 3, "a", 100);
        let refs = format!("{} {}", cmd_refs.join(" "), jobs.join(" "));
        let (text, parsed) = run_query(&ctx, shape.input(&refs)).await;
        let label = format!("{shape:?} + 45 buffer notices");
        eprintln!("{label}: {} B delivered", text.len());
        let mut these = jobs_response_failures(&label, shape, &text, &parsed, &ctx, &jobs);
        if these.is_empty() {
            // The notices are what was over, so they are what is shed, and the jobs survive.
            if parsed["jobs"].as_array().map(Vec::len) != Some(3) {
                these.push(format!(
                    "{label}: the three short jobs were shed: {text:.400}"
                ));
            }
            if !parsed["envelope_omitted"]
                .as_str()
                .is_some_and(|s| s.contains("buffer_truncated"))
            {
                these.push(format!(
                    "{label}: no marker names the shed notices: {text:.400}"
                ));
            }
        }
        failures.extend(these);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- the cut-off hint of a NON-SLICE buffer query pages the query's own output ----
//
// BUG docs/issues/archive/2026-10-08-the-cut-off-hint-of-a-run-command-over-a-buffer-pages-the-source-not-its-own-output.md
// `grep -n "" @cmd_X` over `seq 1 3000` was cut at line 100 and advised
// `Next page: sed -n '101,200p' @cmd_X`, which prints source lines `101`.. without the `N:` prefix,
// and for a filtering grep prints lines that never matched. The query's own output is stored
// nowhere, so the only route to the rest of it is the same query with a pager appended. Every test
// here RUNS the route the hint names.

/// `1\n2\n…\n{n}\n`, what `seq 1 n` prints.
fn seq_lines(n: usize) -> String {
    (1..=n).map(|i| format!("{i}\n")).collect()
}

/// The pager stage a non-slice hint tells the reader to append, parsed out of the hint.
fn appended_pager(hint: &str) -> Option<String> {
    regex::Regex::new(r"append `(\| sed -n '\d+,\d+p')` to this query")
        .expect("static pattern")
        .captures(hint)
        .map(|c| c[1].to_string())
}

/// Run `query` as a buffer query and return `(stdout lines, hint)`.
async fn lines_and_hint(ctx: &ToolContext, query: String) -> (Vec<String>, String, Value) {
    let (text, parsed) = buffer_query(ctx, query.clone()).await;
    let lines = parsed["stdout"]
        .as_str()
        .unwrap_or_else(|| panic!("`{query}` returned no stdout: {text:.300}"))
        .lines()
        .map(str::to_string)
        .collect();
    let hint = parsed["hint"].as_str().unwrap_or_default().to_string();
    (lines, hint, parsed)
}

/// Follow the hints of a buffer query to the end of its output, using ONLY the edit each hint
/// names: append the stage it quotes, or replace the query's last stage with the one it quotes. No
/// arithmetic of the caller's own. Returns every line collected, the first page included.
///
/// Every page is checked: none makes a second handle, and an empty page names no further route,
/// so the walk cannot loop on nothing.
async fn follow_pager_to_the_end(ctx: &ToolContext, query: &str) -> Vec<String> {
    let mut all = Vec::new();
    let mut q = query.to_string();
    for _ in 0..1_000 {
        let (text, parsed) = buffer_query(ctx, q.clone()).await;
        assert!(
            parsed.get("output_id").is_none(),
            "`{q}` made a handle: {text:.300}"
        );
        assert!(
            handles_named(&text).is_subset(&handles_named(&q)),
            "`{q}` named a handle it did not query (re-buffered?): {text:.300}"
        );
        let page: Vec<String> = parsed["stdout"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        let hint = parsed["hint"].as_str().unwrap_or_default();
        let next = next_route_in_hint(&q, hint);
        assert!(
            !page.is_empty() || next.is_none(),
            "`{q}` printed nothing and still names a route: {hint}"
        );
        all.extend(page);
        match next {
            Some(n) => q = n,
            None => return all,
        }
    }
    panic!("`{query}`: no end after 1,000 pages")
}

/// The next query a hint names, built only by the edit it states.
fn next_route_in_hint(query: &str, hint: &str) -> Option<String> {
    if let Some(stage) = appended_pager(hint) {
        return Some(format!("{query} {stage}"));
    }
    let stage = regex::Regex::new(r"replace that last stage with `(sed -n '\d+,\d+p')`")
        .expect("static pattern")
        .captures(hint)?[1]
        .to_string();
    let last = regex::Regex::new(r"\|\s*sed -n '\d+,\d+p'\s*$")
        .expect("static pattern")
        .find(query)
        .unwrap_or_else(|| panic!("the hint says replace, `{query}` ends in no sed: {hint}"));
    Some(format!("{}| {stage}", &query[..last.start()]))
}

/// What `query` prints when run by `sh` over real files: each `(handle, path)` pair substitutes a
/// handle with a file holding the same bytes. The oracle a followed walk is compared against.
fn shell_truth(
    dir: &std::path::Path,
    query: &str,
    subs: &[(String, std::path::PathBuf)],
) -> String {
    let mut cmd = query.to_string();
    for (handle, path) in subs {
        cmd = cmd.replace(handle.as_str(), &path.display().to_string());
    }
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap()
}

#[cfg(unix)]
#[tokio::test]
async fn a_numbered_grep_over_a_buffer_pages_its_own_output_to_the_end() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);
    let query = format!("grep -n \"\" {id}");

    let (shown, hint, parsed) = lines_and_hint(&ctx, query.clone()).await;
    assert_eq!(parsed["stdout_total"], json!(3000), "{parsed}");
    assert!(
        !hint.contains(&format!("sed -n '101,200p' {id}")),
        "the route pages the SOURCE, which prints `101`, not `101:101`: {hint}"
    );
    assert_eq!(
        appended_pager(&hint).as_deref(),
        Some("| sed -n '101,200p'"),
        "{hint}"
    );
    assert!(hint.contains("stored nowhere"), "{hint}");
    assert_eq!(shown.last().map(String::as_str), Some("100:100"));

    // The route, executed, returns the very next lines of THIS output.
    let (page, _, _) = lines_and_hint(&ctx, format!("{query} | sed -n '101,200p'")).await;
    let want: Vec<String> = (101..=200).map(|i| format!("{i}:{i}")).collect();
    assert_eq!(page, want);

    // Followed to the end, the pages are exactly the whole output.
    let all = follow_pager_to_the_end(&ctx, &query).await;
    let full: Vec<String> = (1..=3000).map(|i| format!("{i}:{i}")).collect();
    assert_eq!(all, full);
}

#[cfg(unix)]
#[tokio::test]
async fn a_filtering_grep_over_a_buffer_pages_to_its_next_matches_not_the_next_source_lines() {
    let (_dir, ctx) = project_ctx().await;
    // 10,000 lines, so the ~3,400 matches overflow the response: a query that FITS is returned
    // whole at any line count and gets no hint at all (813 matches over 3,000 lines did).
    let id = ctx
        .output_buffer
        .store("seq 1 10000".into(), seq_lines(10_000), String::new(), 0);
    let query = format!("grep -n 7 {id}");
    let matches: Vec<String> = (1..=10_000)
        .filter(|i| i.to_string().contains('7'))
        .map(|i| format!("{i}:{i}"))
        .collect();
    assert!(matches.len() > 200, "the premise needs a second page");

    let (_, hint, _) = lines_and_hint(&ctx, query.clone()).await;
    let pager = appended_pager(&hint).unwrap_or_else(|| panic!("no appended pager: {hint}"));
    let (page, _, _) = lines_and_hint(&ctx, format!("{query} {pager}")).await;
    assert_eq!(
        page,
        matches[100..200].to_vec(),
        "the NEXT MATCHES, via `{pager}`"
    );
    assert_ne!(
        page.first().map(String::as_str),
        Some("101"),
        "not source line 101"
    );
    assert_eq!(follow_pager_to_the_end(&ctx, &query).await, matches);
}

#[cfg(unix)]
#[tokio::test]
async fn a_query_that_merely_contains_a_ranged_sed_is_numbered_from_its_own_first_line() {
    // `grep … | sed -n '5,2000p'` is NOT a slice of the buffer starting at line 5. Read as one, the
    // hint said `sed -n '105,204p' @cmd_X`, which prints source lines without the `N:` prefix. It
    // IS lines 5.. of the grep's output, so the next page replaces its last stage: never a `sed`
    // on the handle, and never a second `sed` stacked on the first.
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);
    // To line 2,000, so the output overflows the response and is cut (296 lines would fit whole).
    let query = format!("grep -n \"\" {id} | sed -n '5,2000p'");

    let (_, hint, _) = lines_and_hint(&ctx, query.clone()).await;
    assert!(
        !hint.contains(&format!("p' {id}")),
        "a route on the handle: {hint}"
    );
    assert!(
        appended_pager(&hint).is_none(),
        "a stacked second sed: {hint}"
    );
    let next = next_route_in_hint(&query, &hint).unwrap_or_else(|| panic!("no route: {hint}"));
    assert_eq!(next, format!("grep -n \"\" {id} | sed -n '105,204p'"));
    let (page, _, _) = lines_and_hint(&ctx, next).await;
    let want: Vec<String> = (105..=204).map(|i| format!("{i}:{i}")).collect();
    assert_eq!(page, want);
}

#[cfg(unix)]
#[tokio::test]
async fn a_non_slice_query_of_the_err_stream_pages_that_stream() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("cmd".into(), "x\n".into(), seq_lines(3000), 0);
    let query = format!("grep -n \"\" {id}.err");

    let (_, hint, _) = lines_and_hint(&ctx, query.clone()).await;
    assert!(
        hint.contains(&format!("grep 'keyword' {id}.err ")),
        "the search route names the stream queried: {hint}"
    );
    assert!(
        !hint.contains(&format!("p' {id}")),
        "no source page route: {hint}"
    );
    let pager = appended_pager(&hint).unwrap_or_else(|| panic!("no appended pager: {hint}"));
    let (page, _, _) = lines_and_hint(&ctx, format!("{query} {pager}")).await;
    let want: Vec<String> = (101..=200).map(|i| format!("{i}:{i}")).collect();
    assert_eq!(page, want, "lines 101.. of the STDERR, numbered");
}

#[cfg(unix)]
#[tokio::test]
async fn a_pager_the_il3_gate_or_a_list_would_defeat_is_never_offered() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);
    let gated = format!("cargo --version | cat - {id}");
    // The premise, pinned against the gate itself: appending a pager to this query is refused,
    // because the pipe's left-hand side is `cargo` and the buffer is only in its second stage. The
    // joined command names a buffer, so the capture is not promised (`tee_splice_point`) and the
    // foreground gate (`pipe_refusal`, what `run_command` asks) refuses it.
    let joined = format!("{gated} | sed -n '101,200p'");
    assert!(
        crate::util::path_security::detect_il3_violation(&joined).is_some(),
        "the premise needs a query the detector would refuse a pager on"
    );
    assert!(
        super::pipe_refusal(&joined, false).is_some(),
        "the premise needs a query the foreground gate would refuse a pager on"
    );
    // A list: an appended stage would read the LAST command only.
    let listed = format!("grep -c 1 {id}; grep -n \"\" {id}");

    for query in [gated, listed] {
        let (_, hint, parsed) = lines_and_hint(&ctx, query.clone()).await;
        assert_eq!(parsed["truncated"], true, "`{query}`: {parsed}");
        assert!(
            !hint.contains("| sed"),
            "`{query}`: a route that cannot work: {hint}"
        );
        assert!(hint.contains("narrow the query"), "`{query}`: {hint}");
        // What it DOES name runs, and is not refused.
        let search = format!("grep '2999' {id}");
        assert!(
            hint.contains(&format!("grep 'keyword' {id} ")),
            "`{query}`: {hint}"
        );
        let (found, _, _) = lines_and_hint(&ctx, search).await;
        assert_eq!(found, vec!["2999".to_string()]);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_non_slice_hint_does_not_grow_with_the_query() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);
    let short = format!("grep -n \"\" {id}");
    // Same output: the second pattern never matches a digit line.
    let long = format!("grep -n -e \"\" -e \"{}\" {id}", "z".repeat(4_000));

    let (a, hint_short, _) = lines_and_hint(&ctx, short).await;
    let (b, hint_long, _) = lines_and_hint(&ctx, long).await;
    assert_eq!(a, b, "the premise: both queries print the same lines");
    assert_eq!(
        hint_short, hint_long,
        "a 4 KB query must not widen the hint: the query is never echoed"
    );
    assert!(hint_long.len() < 400, "{} B: {hint_long}", hint_long.len());
}

#[cfg(unix)]
#[tokio::test]
async fn a_wide_line_a_non_slice_query_prints_is_windowed_by_appending_to_the_query() {
    // The wide line is source line 300, behind 299 lines of 50 B, so `cut -c1-4000 @cmd_X` (which
    // cuts source lines 1..) overflows the response and is cut itself at line 100: it never
    // reaches the line the query printed clipped.
    let (_dir, ctx) = project_ctx().await;
    let wide = "WIDE".repeat(3_000);
    let head: String = (1..=299).map(|i| format!("{i:>50}\n")).collect();
    let stored = format!("{head}{wide}\n{}", {
        (301..=400).map(|i| format!("{i}\n")).collect::<String>()
    });
    let id = ctx
        .output_buffer
        .store("cmd".into(), stored, String::new(), 0);

    let (plain_cut, _, _) = lines_and_hint(&ctx, format!("cut -c1-4000 {id}")).await;
    assert!(
        !plain_cut.iter().any(|l| l.starts_with("WIDE")),
        "the premise: the source route does not reach line 300"
    );

    for query in [format!("grep WIDE {id}"), format!("sed -n '300,301p' {id}")] {
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        let stdout = parsed["stdout"].as_str().unwrap_or_default();
        let hint = parsed["hint"].as_str().unwrap_or_default();
        assert!(
            stdout.starts_with("WIDEWIDE") && stdout.contains("bytes shown"),
            "{text:.200}"
        );
        for (what, surface) in [("marker", stdout), ("hint", hint)] {
            assert!(
                surface.contains("appending `| cut -c1-4000` to this query"),
                "`{query}`: the {what} names no route that reaches the line"
            );
            assert!(
                !surface.contains(&format!("cut -c1-4000 {id}")),
                "`{query}`: the {what} names the source route"
            );
        }
        let (window, _, _) = lines_and_hint(&ctx, format!("{query} | cut -c1-4000")).await;
        assert_eq!(window[0], wide[..4_000], "`{query} | cut -c1-4000`");
    }

    // The grep printed ONE line and showed it: there is no next page to advise.
    let (_, hint, _) = lines_and_hint(&ctx, format!("grep WIDE {id}")).await;
    assert!(!hint.contains("sed -n"), "{hint}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_slice_piped_into_another_stage_is_not_a_slice() {
    // `sed -n '1,3000p' @cmd_X` is a slice; with `| grep -n ""` after it, it is not. A pattern that
    // was not anchored to the whole command read the first stage and advised `sed` on the handle.
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);
    let query = format!("sed -n '1,3000p' {id} | grep -n \"\"");

    let (_, hint, _) = lines_and_hint(&ctx, query.clone()).await;
    assert!(
        !hint.contains(&format!("p' {id}")),
        "a source page route: {hint}"
    );
    let pager = appended_pager(&hint).unwrap_or_else(|| panic!("no appended pager: {hint}"));
    let (page, _, _) = lines_and_hint(&ctx, format!("{query} {pager}")).await;
    let want: Vec<String> = (101..=200).map(|i| format!("{i}:{i}")).collect();
    assert_eq!(page, want);
}

#[cfg(unix)]
#[tokio::test]
async fn a_non_slice_query_cut_by_bytes_fits_beside_its_longest_hint() {
    // A non-slice hint names a next page only while lines remain, so it is LONGER at
    // `shown < total` than at `shown == total`. The room for stdout is measured from a skeleton
    // hint; measured at `shown == total` alone it is ~190 B too generous, and a response cut by
    // the byte budget crosses the inline limit and is re-buffered under `@tool_*`.
    let (_dir, ctx) = project_ctx().await;
    let mut over = Vec::new();
    for width in 97..=140usize {
        let stored: String = (0..200)
            .map(|i| format!("row{i:03} {}\n", "r".repeat(width - 7)))
            .collect();
        let (_id, text, parsed) =
            query_stored(&ctx, stored, String::new(), |id| format!("grep row {id}")).await;
        if has_tool_handle(&text) || crate::tools::exceeds_inline_limit(&text) {
            over.push(width);
            continue;
        }
        assert_eq!(parsed["truncated"], true, "width {width}: {text:.200}");
        assert!(
            appended_pager(parsed["hint"].as_str().unwrap_or_default()).is_some(),
            "width {width}: the premise is the longer hint: {}",
            parsed["hint"]
        );
    }
    assert!(
        over.is_empty(),
        "widths whose response crossed the limit: {over:?}"
    );
}

// ---- following ONLY the hints reaches every line of a non-slice query's output, exactly once ----
//
// A reviewer's probe on 2c4967b5: `grep row @B` over 1,000 rows of 200 B showed 48 lines and said
// "append `| sed -n '49,148p'` … then step the range by 100". That page was cut again at 48 lines
// (49..96), the next step started at 149, and following the hint literally collected 528 of 1,000
// lines. Page 2's own hint was computed against page 2's output and would have stacked a second
// `sed`. Now each page names the next one from what it actually showed, by replacing its last stage.

#[cfg(unix)]
#[tokio::test]
async fn following_only_the_hints_returns_every_line_of_a_non_slice_query_once() {
    let (dir, ctx) = project_ctx().await;
    let wide: String = (1..=1000)
        .map(|i| format!("row{i:04} {}\n", "r".repeat(190)))
        .collect();
    // Widths 50..400 B, deterministic, so a page holds a varying number of lines.
    let varied: String = (1..=600usize)
        .map(|i| format!("v{i:04} {}\n", "x".repeat(44 + (i * 7919) % 351)))
        .collect();
    let seq3000 = seq_lines(3000);
    let seq10000 = seq_lines(10_000);
    let errs: String = (1..=30).map(|i| format!("warn{i}\n")).collect();

    let mut subs = Vec::new();
    let mut store = |name: &str, out: &str, err: &str| {
        let id = ctx
            .output_buffer
            .store(name.into(), out.to_string(), err.to_string(), 0);
        let path = dir.path().join(format!("{name}.txt"));
        std::fs::write(&path, out).unwrap();
        subs.push((id.clone(), path));
        id
    };
    let b = store("wide", &wide, "");
    let v = store("varied", &varied, "");
    let s = store("seq3000", &seq3000, "");
    let t = store("seq10000", &seq10000, "");
    // 30 stored stderr lines: the stdout line budget drops to 80.
    let c = store("witherr", &seq3000, &errs);

    let mut queries = vec![
        format!("grep row {b}"),
        format!("grep -n \"\" {v}"),
        format!("grep -n \"\" {s}"),
        format!("grep -n 7 {t}"),
        format!("grep -n \"\" {c}"),
        format!("awk '{{print NR\": \"$0}}' {v}"),
    ];
    if std::process::Command::new("rg")
        .arg("--version")
        .output()
        .is_ok()
    {
        queries.push(format!("rg -n row {b}"));
    } else {
        eprintln!("rg not installed: the rg case did not run");
    }
    for query in queries {
        let truth = shell_truth(dir.path(), &query, &subs);
        assert!(
            truth.lines().count() > 200,
            "`{query}`: the premise needs pages"
        );
        let got = follow_pager_to_the_end(&ctx, &query).await;
        assert_eq!(
            format!("{}\n", got.join("\n")),
            truth,
            "`{query}`: following the hints did not return the output, byte for byte"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_list_hidden_by_quoting_the_splitter_does_not_model_is_offered_no_pager() {
    // Each was called one pipeline on 2c4967b5, offered `| sed -n '101,200p'`, and that route paged
    // the LAST command of the list. Now they are withheld: the hint says to narrow the query.
    let (dir, ctx) = project_ctx().await;
    let rows: String = (1..=3000).map(|i| format!("{i} a'b\"c#d|e;f\n")).collect();
    let path = dir.path().join("rows.txt");
    std::fs::write(&path, &rows).unwrap();
    let x = ctx
        .output_buffer
        .store("gen".into(), rows, String::new(), 0);
    let subs = vec![(x.clone(), path)];
    let mut ran = 0;
    for query in [
        format!(r#"grep -c $'\'' {x}; grep -n "" {x}"#),
        format!(r#"echo "$(grep -c '"' {x})"; grep -n "" {x}"#),
        format!("grep -c \"`echo '\"'`\" {x}; grep -n \"\" {x}"),
        format!(r#"grep -n "$(echo '"')" {x} # x"#),
        format!(r#"grep -n "$(echo '"')" {x}; grep -c a {x}"#),
    ] {
        // The premise is output worth cutting. A `sh` that rejects the syntax prints nothing for the
        // query (dash rejects `$'...'`, which Ubuntu CI runs as `/bin/sh`), so there is nothing to
        // page and no hint to check. The oracle is the same `sh` the tool runs.
        if shell_truth(dir.path(), &query, &subs).is_empty() {
            eprintln!("`{query}`: this sh prints nothing for it, skipped");
            continue;
        }
        ran += 1;
        let (_, hint, parsed) = lines_and_hint(&ctx, query.clone()).await;
        assert_eq!(
            parsed["truncated"], true,
            "`{query}`: the premise is a cut: {parsed}"
        );
        assert!(!hint.contains("| sed"), "`{query}`: {hint}");
        assert!(
            next_route_in_hint(&query, &hint).is_none(),
            "`{query}`: {hint}"
        );
        assert!(hint.contains("narrow the query"), "`{query}`: {hint}");
    }
    // Only the `$'...'` row may be lost to a stricter `sh`: the test must still run the rest.
    assert!(ran >= 4, "only {ran} of 5 queries ran");
}

#[cfg(unix)]
#[tokio::test]
async fn a_query_with_a_trailing_newline_is_offered_no_route_it_cannot_take() {
    // The caller appends to its OWN text: `grep -n '' @X\n | sed -n '101,200p'` runs as two lines,
    // and the first prints lines 1.. again.
    let (_dir, ctx) = project_ctx().await;
    let a = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    let query = format!("grep -n '' {a}\n");
    let (_, hint, parsed) = lines_and_hint(&ctx, query.clone()).await;
    assert_eq!(parsed["truncated"], true, "{parsed}");
    if let Some(p) = appended_pager(&hint) {
        let (page, _, _) = lines_and_hint(&ctx, format!("{query} {p}")).await;
        assert_eq!(page.first().map(String::as_str), Some("101:101"), "{hint}");
    }
    assert!(hint.contains("narrow the query"), "{hint}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_tool_ref_query_gets_routes_with_real_numbers_and_each_one_runs() {
    // The `@tool_*` arm told every query, `grep -n '"i"' @tool_X` included, to
    // `read_file("@tool_X", json_path="$.<field>")` or `start_line=N, end_line=M`: routes on the
    // SOURCE, with literal placeholders. Now a `@tool_*` ref takes the same two-population rule.
    let (dir, ctx) = project_ctx().await;
    let rows: Vec<Value> = (0..3000).map(|i| json!({ "i": i, "s": "x" })).collect();
    let blob = serde_json::to_string(&json!({ "rows": rows })).unwrap();
    let t = ctx.output_buffer.store_tool("probe", blob.clone());
    // `run_command` reads a `@tool_*` ref pretty-printed: that is the oracle's file too.
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&blob).unwrap()).unwrap();
    let path = dir.path().join("tool.json");
    std::fs::write(&path, &pretty).unwrap();
    let subs = vec![(t.clone(), path)];

    // A slice: the next page is a `sed` on the handle, in real numbers.
    let (shown, hint, _) = lines_and_hint(&ctx, format!("cat {t}")).await;
    for placeholder in ["<field>", "start_line=N", "end_line=M", "@ref"] {
        assert!(!hint.contains(placeholder), "{placeholder}: {hint}");
    }
    let page = regex::Regex::new(r"Next page: (sed -n '\d+,\d+p' @tool_[0-9a-f]+)\.")
        .unwrap()
        .captures(&hint)
        .unwrap_or_else(|| panic!("no slice route: {hint}"))[1]
        .to_string();
    let (next, _, _) = lines_and_hint(&ctx, page.clone()).await;
    let want: Vec<String> = pretty.lines().map(str::to_string).collect();
    assert_eq!(
        next[0],
        want[shown.len()],
        "`{page}` does not continue the slice"
    );

    // Not a slice: the query's own output, paged to the end by the hints alone.
    let query = format!("grep -n '\"i\"' {t}");
    let (_, hint, _) = lines_and_hint(&ctx, query.clone()).await;
    assert!(!hint.contains("read_file"), "a route on the source: {hint}");
    let got = follow_pager_to_the_end(&ctx, &query).await;
    assert_eq!(
        format!("{}\n", got.join("\n")),
        shell_truth(dir.path(), &query, &subs)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_list_query_own_stderr_marker_does_not_name_a_stage_to_append() {
    // The marker on a query's OWN stderr said to append `2>&1 >/dev/null | cut -c1-4000`. On a list
    // that stage reads the LAST command's stderr only.
    let (_dir, ctx) = project_ctx().await;
    let a = ctx
        .output_buffer
        .store("seq".into(), seq_lines(10), String::new(), 0);
    let query = format!("grep -c 1 {a}; printf '%3000s\\n' | tr ' ' e >&2");
    let (_, parsed) = buffer_query(&ctx, query.clone()).await;
    let stderr = parsed["stderr"].as_str().unwrap_or_default();
    assert!(
        stderr.contains("bytes shown"),
        "the premise is a cut stderr: {parsed}"
    );
    assert!(!stderr.contains("2>&1 >/dev/null"), "{stderr:.0}");
    assert!(stderr.contains("narrow the query"), "{stderr}");
}

#[cfg(unix)]
#[tokio::test]
async fn a_background_log_query_names_its_handle_and_the_routes_run() {
    // `queried_stream` could not name `@bg_*`, so the hint said `@ref`: a placeholder.
    let (dir, ctx) = project_ctx().await;
    let started = RunCommand
        .call(
            json!({ "command": "seq 1 3000", "run_in_background": true, "timeout_secs": 10 }),
            &ctx,
        )
        .await
        .unwrap();
    let bg = started["output_id"].as_str().unwrap().to_string();
    assert!(bg.starts_with("@bg_"), "{started}");
    let mut done = false;
    for _ in 0..100 {
        let (_, parsed) = buffer_query(&ctx, format!("grep -c '' {bg}")).await;
        if parsed["stdout"].as_str().unwrap_or_default().trim() == "3000" {
            done = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(done, "the background job never wrote 3,000 lines");

    let query = format!("grep -n \"\" {bg}");
    let (_, hint, parsed) = lines_and_hint(&ctx, query.clone()).await;
    assert_eq!(parsed["truncated"], true, "{parsed}");
    assert!(!hint.contains("@ref"), "{hint}");
    assert!(hint.contains(&format!("grep 'keyword' {bg} ")), "{hint}");
    let path = dir.path().join("bg.txt");
    std::fs::write(&path, seq_lines(3000)).unwrap();
    let got = follow_pager_to_the_end(&ctx, &query).await;
    assert_eq!(
        format!("{}\n", got.join("\n")),
        shell_truth(dir.path(), &query, &[(bg.clone(), path)])
    );
    // A slice of the log keeps the route on the handle, and it continues the slice.
    let (_, hint, _) = lines_and_hint(&ctx, format!("cat {bg}")).await;
    assert!(hint.contains(&format!("sed -n '101,200p' {bg}")), "{hint}");
    let (page, _, _) = lines_and_hint(&ctx, format!("sed -n '101,200p' {bg}")).await;
    assert_eq!(page.first().map(String::as_str), Some("101"));
}

/// A reviewer's adversarial table (2026-10-08): 79 query shapes, plus one of ours, over one buffer whose lines carry
/// every quote and separator (`'`, `"`, `#`, `|`, `;`). For each, the route class the hint offers
/// is pinned, and every offered route is FOLLOWED by the hints alone to the end of the output and
/// compared with what `sh` prints for the same query over a real file. `withheld` marks a shape
/// the hint refuses a pager for although one would work: the price of not modelling `$(…)`,
/// backticks, `$'…'`, comments, heredocs, here-strings and lists.
#[cfg(unix)]
#[tokio::test]
async fn adversarial_query_shapes_get_a_route_that_returns_their_own_output() {
    let (dir, ctx) = project_ctx().await;
    let a_out: String = (1..=3000).map(|i| format!("{i} a'b\"c#d|e;f\n")).collect();
    let a = ctx
        .output_buffer
        .store("gen".into(), a_out.clone(), String::new(), 0);
    let e_err: String = (1..=3000).map(|i| format!("e{i}\n")).collect();
    let e = ctx
        .output_buffer
        .store("gen".into(), "x\n".into(), e_err.clone(), 0);
    let (pa, pe_out, pe_err) = (
        dir.path().join("a.txt"),
        dir.path().join("e.txt"),
        dir.path().join("e.err.txt"),
    );
    std::fs::write(&pa, &a_out).unwrap();
    std::fs::write(&pe_out, "x\n").unwrap();
    std::fs::write(&pe_err, &e_err).unwrap();
    let subs = vec![
        (format!("{e}.err"), pe_err),
        (e.clone(), pe_out),
        (a.clone(), pa),
    ];
    // (query template, expected route class)
    check_adversarial_table(&dir, &ctx, &subs, &a, &e, adversarial_table()).await;
}
fn adversarial_table() -> Vec<(&'static str, &'static str)> {
    let table = vec![
        (r#"grep -n "" {A}"#, "pager"),
        (r#"grep -n 'a' {A}"#, "pager"),
        (r#"grep -n "a'b" {A}"#, "pager"),
        (r#"grep -n 'e;f' {A}"#, "pager"),
        (r#"grep -n 'd|e' {A}"#, "pager"),
        (r##"grep -n "#d" {A}"##, "pager"),
        (r#"grep -n 'c#d' {A}"#, "pager"),
        (r#"grep -n "" {A} 2>&1"#, "pager"),
        (r#"grep -n "" {A} 2>/dev/null"#, "handle"), // `2>/dev/null` names a path: not a buffer query, it makes its own handle
        (r#"grep -n "" {A} |& cat"#, "pager"),
        (r#"grep -n "" {A} | cat"#, "pager"),
        (r#"grep -n "" {A} | sed -n '1,500p'"#, "pager"),
        (r#"grep -n "" {A} | head -n 300"#, "none"), // 300 short lines fit whole
        (r#"grep -n "" {A} | sed -n '5,400p'"#, "none"), // 396 short lines fit whole
        (r#"cat {A} | grep -n a"#, "pager"),
        (r#"grep -n "" {E}.err"#, "pager"),
        (r#"awk '{print NR": "$0}' {A}"#, "pager"),
        (r#"awk '{print NR"|"$0}' {A}"#, "pager"),
        (r#"cut -c1-5 {A}"#, "pager"),
        (r#"cat -n {A}"#, "pager"),
        (r#"tac {A}"#, "pager"),
        (r#"grep -n \"c {A}"#, "pager"),
        (r#"grep -n \' {A}"#, "pager"),
        (r#"grep -n d\|e {A}"#, "pager"),
        (r#"grep -n "$(printf a)" {A}"#, "handle"), // a word starting with `$` reads as a path: not a buffer query
        (r#"grep -n "$(echo 'a|b' | cut -c1)" {A}"#, "handle"), // same
        (r#"grep -n $(echo a) {A}"#, "handle"),     // same
        (r#"grep -n $(echo a | cat) {A}"#, "handle"), // same
        (r#"grep -n `echo a` {A}"#, "withheld"), // a backtick: not modelled, so refused although the route would work
        (r#"grep -n `echo a | cat` {A}"#, "withheld"), // same
        (r#"cat <(grep -n "" {A})"#, "pager"),
        (r#"(grep -n "" {A})"#, "pager"),
        (r#"{ grep -n "" {A}; }"#, "withheld"), // a `;` inside braces is a list to the splitter
        (r#"(grep -c a {A}; grep -n "" {A})"#, "withheld"), // a list in a subshell
        (r#"grep -n "" {A} # comment"#, "withheld"), // a comment would swallow the stage
        (r#"grep -n a#b {A}"#, "none"),         // no line matches
        (r#"grep -n "" {A} > /dev/null"#, "none"), // stdout goes to /dev/null
        (r#"grep -n "" {A} | tee /dev/null"#, "handle"), // `/dev/null` names a path
        (r#"! grep -n "" {A}"#, "pager"),
        ("grep -n \"\" {A} \\\n| cat", "pager"),
        ("grep -n \"a\nb\" {A}", "pager"),
        ("grep -n '' {A}\n", "withheld"), // a trailing newline: the appended stage would run as a second line
        (r#"grep -n "" {A};"#, "withheld"), // a trailing `;`
        (r#"grep -n "" {A} &"#, "withheld"), // a background `&`
        (r#"grep -n "" {A} && true"#, "withheld"), // a list
        (r#"grep -n "" {A} || true"#, "withheld"), // a list
        (r#"true && grep -n "" {A}"#, "withheld"), // a list
        (r#"grep -c a {A}; grep -n "" {A}"#, "withheld"), // a list
        (r#"grep -n "" {A} <<< "x""#, "withheld"), // a here-string `<<<`: not modelled
        ("grep -f - -n {A} <<EOF\na\nEOF", "withheld"), // a heredoc: its body follows the line a stage would join
        (r#"grep -n "" {A} | sed 's/a/ä/'"#, "handle"), // `s/a/ä/` contains a slash and reads as a path
        (r#"grep -n "ä|ö" {A}"#, "none"),               // no line matches
        (r#"grep -c $'\'' {A}; grep -n "" {A}"#, "withheld"), // `$'…'`: not modelled
        (r#"echo "$(grep -c '"' {A})"; grep -n "" {A}"#, "withheld"), // `$(…)` and a list
        (r#"grep -n $'a\'b' {A}"#, "withheld"), // `$'…'`: not modelled, although the route would work
        (r#"grep -n "" {A} | sed -n '1,5p'"#, "none"), // 5 lines fit
        (r#"grep -n "" {A} | head -n 3"#, "none"), // 3 lines fit
        (r#"grep -n "" {A} 2>&1 >/dev/null"#, "none"), // stdout goes to /dev/null
        (r#"echo {A} | xargs grep -n """#, "pager"),
        (
            r#"grep -n "" {A} | while read l; do echo "$l"; done"#,
            "handle",
        ), // `$l` reads as a path
        (r#"for f in {A}; do grep -n "" $f; done"#, "handle"), // `$f` reads as a path
        (r#"if true; then grep -n "" {A}; fi"#, "withheld"), // `if …; fi` is a list to the splitter
        (r#"grep -n "" {A} | awk 'NR>0' # x"#, "withheld"),  // a comment would swallow the stage
        (r#"grep -n "" "{A}""#, "pager"),
        (r#"rg -n "" {A}"#, "pager"),
        (r#"rg -n "" {A} | cat"#, "pager"),
        (r#"cargo --version | cat - {A}"#, "withheld"), // IL3 refuses a pager after a `cargo` left-hand side
        (r#"git --version | cat - {A}"#, "withheld"), // IL3 refuses a pager after a `git` left-hand side
        (r#"grep -n "\\" {A} ; true"#, "none"),       // no line matches
        ("grep -c \"`echo '\"'`\" {A}; grep -n \"\" {A}", "withheld"), // a backtick and a list
        (r#"grep -n "$(echo '"')" {A} # x"#, "withheld"), // `$(…)` hiding a comment
        (r#"grep -n "" {A} | tee /dev/stderr"#, "handle"), // `/dev/stderr` names a path
        (r#"shuf {A}"#, "random"), // `shuf` prints a new order on every run: a re-run route cannot continue it
        (r#"grep -n "" {A} | sed -n '1,5000p' | cat"#, "pager"),
        (r#"grep -n "" {A} ${x#y}"#, "handle"), // `${x#y}` reads as a path
        (r#"grep -n "" {A} 1>&2 | cat"#, "none"), // stdout goes to stderr
        (r#"grep -n "" {A} >&2"#, "none"),      // stdout goes to stderr
        (r#"exec 2>&1; grep -n "" {A}"#, "withheld"), // a list
        (r#"grep -n "$(echo '"')" {A}; grep -c a {A}"#, "withheld"), // `$(…)` and a list
        // Not the reviewer's: a list that ENDS like a slice (`cat <ref>`) is no slice.
        (r#"grep -c a {A}; cat {A}"#, "withheld"), // a list
    ];
    assert!(!table.is_empty());
    table
}
async fn check_adversarial_table(
    dir: &tempfile::TempDir,
    ctx: &ToolContext,
    subs: &[(String, std::path::PathBuf)],
    a: &str,
    e: &str,
    table: Vec<(&str, &str)>,
) {
    let has_rg = std::process::Command::new("rg")
        .arg("--version")
        .output()
        .is_ok();
    let window_re = regex::Regex::new(r"(?s)^(.*\S)\s*\|\s*sed -n '(\d+),\d+p'\s*$").unwrap();
    let mut report = Vec::new();
    let mut failures = Vec::new();
    for (i, (template, want)) in table.iter().enumerate() {
        let query = template.replace("{A}", a).replace("{E}", e);
        if query.starts_with("rg") && !has_rg {
            eprintln!("#{i}: rg not installed, skipped");
            continue;
        }
        // Every class but `none` needs output to cut. A `sh` that rejects the syntax (dash: `|&`,
        // `<(`, `<<<`, `$'...'`) prints nothing for the row, so it has no hint to classify.
        if !matches!(*want, "none" | "refused") && shell_truth(dir.path(), &query, subs).is_empty()
        {
            eprintln!("#{i}: this sh prints nothing for {template:?}, skipped");
            continue;
        }
        let input = json!({ "command": query, "timeout_secs": 30 });
        let content = match RunCommand.call_content(input, ctx).await {
            Ok(c) => c,
            Err(_) => {
                report.push(format!("#{i:02} refused   {template:?}"));
                if *want != "refused" {
                    failures.push(format!("#{i} {template:?}: refused, want {want}"));
                }
                continue;
            }
        };
        let text = content[0]
            .as_text()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let parsed: Value = serde_json::from_str(&text).unwrap_or(json!({}));
        let hint = parsed["hint"].as_str().unwrap_or_default();
        let truncated = parsed["truncated"] == json!(true);
        let class = if parsed.get("output_id").is_some() {
            "handle"
        } else if next_route_in_hint(&query, hint).is_some() {
            "pager"
        } else if hint.contains("narrow the query") {
            "withheld"
        } else if hint.contains("Next page: sed -n") {
            "slice"
        } else if truncated {
            "other"
        } else {
            "none"
        };
        report.push(format!("#{i:02} {class:<9} {template:?}"));
        if class != *want && !(*want == "random" && class == "pager") {
            failures.push(format!("#{i} {template:?}: class {class}, want {want}"));
        }
        // Every offered route, followed by the hints alone, returns the query's own output. A query
        // ending in `| sed -n 'A,Bp'` is read as a window over the query before it, and its hints
        // page THAT output from line A on, past B: the hint says so in those words.
        if class == "pager" && *want != "random" {
            let window = window_re
                .captures(&query)
                .map(|c| (c[1].to_string(), c[2].parse::<usize>().unwrap()));
            let (oracle, skip) = window.unwrap_or((query.clone(), 1));
            let truth: Vec<String> = shell_truth(dir.path(), &oracle, subs)
                .lines()
                .skip(skip - 1)
                .map(str::to_string)
                .collect();
            let got = follow_pager_to_the_end(ctx, &query).await;
            if got != truth {
                failures.push(format!(
                    "#{i} {template:?}: following the hints returned {} lines, want {}",
                    got.len(),
                    truth.len()
                ));
            }
        }
    }
    // A floor on the rows EVALUATED (`report` gets one line per row that was not skipped): a loop
    // that visited none of them would otherwise pass.
    assert!(
        report.len() >= 70,
        "only {} of {} rows were evaluated: the table moved or the loop is empty",
        report.len(),
        table.len()
    );
    assert!(
        failures.is_empty(),
        "{}\n\nall:\n{}",
        failures.join("\n"),
        report.join("\n")
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_pager_window_that_fits_only_without_its_hint_is_cut_not_rebuffered() {
    // A full 100-line window that fits whole carries the route to the next window. The inline
    // gate must count that hint: a page that fits only WITHOUT it would otherwise be returned
    // whole, cross the limit with the hint added, and be re-buffered under `@tool_*`. Widths are
    // swept so one lands in the ~180 B the hint costs.
    let (_dir, ctx) = project_ctx().await;
    let mut seen_full = false;
    for width in 85..=100usize {
        let stored: String = (0..100)
            .map(|i| format!("p{i:03} {}\n", "q".repeat(width - 5)))
            .collect();
        let id = ctx
            .output_buffer
            .store("cmd".into(), stored, String::new(), 0);
        let query = format!("grep p {id} | sed -n '1,100p'");
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        assert!(
            handles_named(&text).is_subset(&handles_named(&query)),
            "width {width}: re-buffered: {text:.200}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "width {width}: {} B",
            text.len()
        );
        let hint = parsed["hint"].as_str().unwrap_or_default();
        let route = next_route_in_hint(&query, hint)
            .unwrap_or_else(|| panic!("width {width}: no route: {hint}"));
        seen_full |= parsed.get("truncated").is_none();
        if parsed.get("truncated").is_none() {
            assert_eq!(
                route,
                format!("grep p {id} | sed -n '101,200p'"),
                "width {width}"
            );
        }
    }
    assert!(seen_full, "the premise: some width fits whole");
}

// A trailing `| sed -n '...p'` is the pager the hint itself advised, whatever its quoting: the next
// page REPLACES it. Unquoted, single- and double-quoted forms are all recognised.
#[cfg(unix)]
#[tokio::test]
async fn a_trailing_pager_is_replaced_in_every_quote_form() {
    let (_dir, ctx) = project_ctx().await;
    let x = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    for pager in ["sed -n '1,100p'", "sed -n \"1,100p\"", "sed -n 1,100p"] {
        let q = format!("grep -n \"\" {x} | {pager}");
        let (_, hint, _) = lines_and_hint(&ctx, q.clone()).await;
        assert!(
            hint.contains("replace that last stage with `sed -n '101,200p'`"),
            "{q}: {hint}"
        );
    }
}

// A window that did not fill (50 of 100 lines) is the end of the output: it names no next window.
#[cfg(unix)]
#[tokio::test]
async fn a_short_last_window_names_no_next_window() {
    let (_dir, ctx) = project_ctx().await;
    let x = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    let q = format!("grep -n \"\" {x} | sed -n '2951,3050p'");
    let (lines, hint, _) = lines_and_hint(&ctx, q.clone()).await;
    assert_eq!(lines.len(), 50, "{q}");
    assert!(
        hint.is_empty(),
        "a window that did not fill names no next one: {hint}"
    );
}

// `sed -n '5,3p'` is a reversed range: it prints line 5 only and is no pager. A mutant that dropped
// the `b >= a` guard panicked with an overflow in debug builds.
#[cfg(unix)]
#[tokio::test]
async fn a_reversed_trailing_range_is_not_a_pager() {
    let (_dir, ctx) = project_ctx().await;
    let x = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    let q = format!("grep -n \"\" {x} | sed -n '5,3p'");
    let (lines, hint, _) = lines_and_hint(&ctx, q.clone()).await;
    assert_eq!(lines, vec!["5:5".to_string()], "{q}");
    assert!(hint.is_empty(), "{hint}");
}

// A slice that starts after line 1 of its ref (`sed -n '5,10p' <ref>`) clips a wide line that is
// not the ref's line 1, so `cut -c1-4000 <ref>` would never reach it: the route is appended instead.
#[cfg(unix)]
#[tokio::test]
async fn a_ranged_slices_wide_first_line_route_is_not_on_the_handle() {
    let (_dir, ctx) = project_ctx().await;
    let marker = "WIDEMARK";
    let stored = format!("a\nb\nc\nd\n{marker}{}\nf\ng\n", "w".repeat(50_000));
    let id = ctx
        .output_buffer
        .store("cmd".into(), stored, String::new(), 0);
    let q = format!("sed -n '5,10p' {id}");
    let (_, hint, _) = lines_and_hint(&ctx, q.clone()).await;
    assert!(
        !hint.contains(&format!("cut -c1-4000 {id}")),
        "the handle's line 1 is not this output's line 1: {hint}"
    );
    assert!(
        hint.contains("appending `| cut -c1-4000` to this query"),
        "{hint}"
    );
    let (lines, _, _) = lines_and_hint(&ctx, format!("{q} | cut -c1-4000")).await;
    assert!(lines[0].starts_with(marker), "{:.40}", lines[0]);
}

// A quoted newline inside the base query does not make it a list: the trailing pager is still replaced.
#[cfg(unix)]
#[tokio::test]
async fn a_base_with_a_quoted_newline_still_gets_its_pager_replaced() {
    let (_dir, ctx) = project_ctx().await;
    let x = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    let q = format!("grep -n \"1\n2\" {x} | sed -n '1,100p'");
    let (lines, hint, _) = lines_and_hint(&ctx, q.clone()).await;
    assert_eq!(lines.len(), 100, "{q}");
    assert!(
        hint.contains("replace that last stage with `sed -n '101,200p'`"),
        "{q}: {hint}"
    );
}

// `q \r\n`: the `\r` is an argument and the `\n` ends the line, so an appended stage would run as a
// second command. No stage is offered.
#[cfg(unix)]
#[tokio::test]
async fn a_trailing_crlf_after_a_word_gets_no_appended_stage() {
    let (_dir, ctx) = project_ctx().await;
    let x = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    let q = format!("grep -n '' {x} \r\n");
    let (_, hint, parsed) = lines_and_hint(&ctx, q.clone()).await;
    assert_eq!(parsed["truncated"], true, "{parsed}");
    assert!(appended_pager(&hint).is_none(), "{q:?}: {hint}");
    assert!(hint.contains("narrow the query"), "{hint}");
}

// ---- the pool's byte budget ----

/// An output bigger than the pool's byte limit (lowered to the 1 MiB floor here), through the
/// real `RunCommand`: the pool keeps a marked prefix, the response names how many lines were
/// kept and dropped, a later read of the handle carries the notice, and line N of the handle is
/// still line N of the output.
#[cfg(unix)]
#[tokio::test]
async fn an_oversized_output_is_a_marked_prefix_and_the_response_says_so() {
    let (_dir, mut ctx) = project_ctx().await;
    let limit = crate::tools::output_buffer::MIN_POOL_BYTES;
    ctx.output_buffer = std::sync::Arc::new(
        crate::tools::output_buffer::OutputBuffer::with_byte_limit(50, limit),
    );
    let (text, parsed) = buffer_query_free(&ctx, "seq 1 300000").await;
    let id = parsed["output_id"]
        .as_str()
        .unwrap_or_else(|| panic!("no output_id: {text:.400}"))
        .to_string();
    let t = ctx
        .output_buffer
        .get(&id)
        .unwrap()
        .truncated
        .expect("the pool cut the output");
    assert_eq!(t.total_lines, 300_000);
    let counts = format!(
        "{} of 300000 lines kept, {} omitted",
        t.kept_lines,
        300_000 - t.kept_lines
    );
    let names_the_cut = |v: &Value| {
        v.as_array().is_some_and(|a| {
            a.iter().any(|n| {
                n.as_str()
                    .is_some_and(|s| s.contains(&counts) && s.contains(&id))
            })
        })
    };
    assert!(
        names_the_cut(&parsed["output_truncated"]),
        "the response must say what was kept and dropped: {text:.800}"
    );
    assert!(ctx.output_buffer.bytes_of(&id).unwrap() <= limit / 2);
    ctx.output_buffer.assert_accounting();

    let n = t.kept_lines / 2;
    let (text, parsed) = buffer_query(&ctx, format!("sed -n '{n}p' {id}")).await;
    assert_eq!(
        parsed["stdout"].as_str().map(str::trim),
        Some(n.to_string().as_str()),
        "line {n} of the handle is line {n} of the output: {text:.400}"
    );
    assert!(
        names_the_cut(&parsed["buffer_truncated"]),
        "a later read of the handle carries the notice: {text:.800}"
    );
}

/// The pool lowered to its floor, for the budget tests below.
async fn floor_ctx() -> (tempfile::TempDir, ToolContext) {
    let (dir, mut ctx) = project_ctx().await;
    ctx.output_buffer =
        std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::with_byte_limit(
            50,
            crate::tools::output_buffer::MIN_POOL_BYTES,
        ));
    (dir, ctx)
}

/// Fix 3 (review probe RPG): the `grep` tool over a handle that holds only a prefix carries the
/// notice, for the bare handle and for `.err`. `^299999$` is in the output and past the prefix,
/// so a bare `0 matches` would read as "not printed".
#[cfg(unix)]
#[tokio::test]
async fn a_grep_of_a_cut_handle_carries_the_truncation_notice() {
    let (_dir, ctx) = floor_ctx().await;
    let (_, parsed) = buffer_query_free(&ctx, "seq 1 300000").await;
    let id = parsed["output_id"].as_str().unwrap().to_string();
    let grep = |path: String| {
        let ctx = &ctx;
        async move {
            let blocks = crate::tools::grep::Grep
                .call_content(json!({ "pattern": "^299999$", "path": path }), ctx)
                .await
                .unwrap();
            crate::tools::hint_probe::primary_text(&blocks)
        }
    };
    let text = grep(id.clone()).await;
    assert!(text.starts_with("0 matches"), "{text}");
    assert!(
        text.contains(crate::tools::output_buffer::TRUNCATION_SENTINEL_PREFIX)
            && text.contains(&id),
        "a zero over a cut handle must say the handle is cut: {text}"
    );

    let err_id = ctx.output_buffer.store(
        "e".into(),
        String::new(),
        (1..=300_000).map(|i| format!("{i}\n")).collect(),
        1,
    );
    let text = grep(format!("{err_id}.err")).await;
    assert!(
        text.contains(&format!("{err_id}.err")) && text.contains("lines kept"),
        "a grep of a cut stderr carries the stderr notice: {text}"
    );
}

/// Fix 7: the tee capture keeps the line that empties its budget, however long, so the pool can
/// cut it. The response must report the HANDLE (0 of 1 lines kept for one 5 MB line, over the
/// 4 MiB capture limit, not 1) and announce the pool's cut.
#[cfg(unix)]
#[tokio::test]
async fn a_tee_capture_the_pool_cuts_reports_the_handle_it_kept() {
    let (_dir, ctx) = floor_ctx().await;
    let (text, parsed) =
        buffer_query_free(&ctx, "head -c 5000000 /dev/zero | tr '\\0' x | grep -c x").await;
    let tee = parsed["unfiltered_output"]
        .as_str()
        .unwrap_or_else(|| panic!("no tee capture: {text:.600}"))
        .to_string();
    let held = ctx
        .output_buffer
        .stored_truncation(&tee)
        .expect("the capture is cut");
    assert_eq!(
        held.kept_lines, 0,
        "one line longer than the share keeps nothing"
    );
    assert_eq!(
        parsed["unfiltered_buffered_lines"],
        json!(held.kept_lines),
        "the response must report the handle's own count: {text:.600}"
    );
    assert!(
        parsed["output_truncated"].as_array().is_some_and(|a| a
            .iter()
            .any(|n| n.as_str().is_some_and(|s| s.contains(&tee)))),
        "the pool's cut is announced: {text:.800}"
    );
}

/// Review F6(c): a tee capture cut only by its own budget, which the pool keeps as it was given,
/// announces no pool cut. `output_truncated` names a cut the POOL made, and there is none.
#[cfg(unix)]
#[tokio::test]
async fn a_tee_capture_the_pool_keeps_as_given_announces_no_pool_cut() {
    let (_dir, ctx) = project_ctx().await;
    let result = RunCommand
        .call(json!({ "command": "seq 1 1000000 | grep zzz" }), &ctx)
        .await
        .unwrap();
    let tee = result["unfiltered_output"]
        .as_str()
        .unwrap_or_else(|| panic!("no tee capture: {result}"))
        .to_string();
    assert!(
        result.get("unfiltered_truncated").is_some(),
        "the capture's own budget cut it: {result}"
    );
    let held = ctx
        .output_buffer
        .stored_truncation(&tee)
        .expect("the handle records the capture's own cut");
    assert_eq!(result["unfiltered_buffered_lines"], json!(held.kept_lines));
    assert!(
        result
            .get("output_truncated")
            .is_none_or(|v| !v.to_string().contains(&tee)),
        "no pool cut is announced for {tee}: {result}"
    );
}

/// R-A: a handle the pool dropped to stay under its byte limit is refused with that cause named,
/// not only "the session reset" (review probe S2: an output of the whole limit was evicted by the
/// next small one, and the refusal named the wrong cause).
#[tokio::test]
async fn a_dropped_handle_is_refused_naming_the_byte_limit_and_the_route() {
    let (_dir, ctx) = floor_ctx().await;
    let gone = ctx.output_buffer.store(
        "a".into(),
        "a".repeat(crate::tools::output_buffer::MIN_POOL_BYTES / 2),
        String::new(),
        0,
    );
    ctx.output_buffer.store(
        "b".into(),
        "b".repeat(crate::tools::output_buffer::MIN_POOL_BYTES / 2),
        String::new(),
        0,
    );
    ctx.output_buffer
        .store("c".into(), "c".into(), String::new(), 0);
    assert_eq!(
        ctx.output_buffer.bytes_of(&gone),
        None,
        "precondition: dropped"
    );
    let err = crate::tools::read_file::ReadFile
        .call(json!({ "path": gone }), &ctx)
        .await
        .unwrap_err();
    let rec = err
        .downcast_ref::<crate::tools::RecoverableError>()
        .expect("recoverable");
    let hint = rec
        .guidance
        .as_ref()
        .map(|g| g.text().to_string())
        .unwrap_or_default();
    assert!(
        hint.contains("byte limit") && hint.contains("Re-run the command"),
        "{hint}"
    );
}

// ---- a PLAIN SLICE window of 100 lines that ends before the buffer's last line names the next one ----
//
// BUG docs/issues/archive/2026-10-08-a-plain-slice-window-of-100-lines-carries-no-next-page-hint.md
// `cat <ref>` over 3,000 short lines is cut at 100 and names `sed -n '101,200p' <ref>`. That page
// prints exactly 100 lines, which fit whole, so it was not cut and carried no hint: a reader
// following the hints alone stopped at line 200. A trailing pager had the rule already
// (`full_page_hint`); a plain slice knows more, the stream's own line count, so it names the next
// window only while one is left.

/// The next query a hint names, by whichever edit it states: a pager edit ([`next_route_in_hint`])
/// or, for a plain slice, the `sed -n` page on the handle it quotes.
fn next_query_in_hint(query: &str, hint: &str) -> Option<String> {
    if let Some(n) = next_route_in_hint(query, hint) {
        return Some(n);
    }
    regex::Regex::new(
        r"Next page: (sed -n '\d+,\d+p' @(?:cmd|file|tool|bg)_[0-9a-f]+(?:\.err)?)\.(?:\s|$)",
    )
    .expect("static pattern")
    .captures(hint)
    .map(|c| c[1].to_string())
}

/// Follow ONLY the hints from `query` to where they stop, whichever kind of page each names.
/// Returns every line collected, every query run and the size of every page. Each page is checked:
/// none makes a second handle, and an empty page names no further route.
async fn follow_every_hint(
    ctx: &ToolContext,
    query: &str,
) -> (Vec<String>, Vec<String>, Vec<usize>, Vec<usize>) {
    let stated = regex::Regex::new(r"which has (\d+) lines").expect("static pattern");
    let mut all = Vec::new();
    let mut ran = Vec::new();
    let mut sizes = Vec::new();
    let mut totals = Vec::new();
    let mut q = query.to_string();
    for _ in 0..1_000 {
        let (text, parsed) = buffer_query(ctx, q.clone()).await;
        assert!(
            parsed.get("output_id").is_none(),
            "`{q}` made a handle: {text:.300}"
        );
        assert!(
            handles_named(&text).is_subset(&handles_named(&q)),
            "`{q}` named a handle it did not query (re-buffered?): {text:.300}"
        );
        ran.push(q.clone());
        let page: Vec<String> = parsed["stdout"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        let hint = parsed["hint"].as_str().unwrap_or_default();
        // Every page after the first is the answer to a hint, and a hint that points at an empty
        // page is a wrong hint: the walk may not end on one. (It used to assert only that an
        // empty page names no further route, which an empty LAST page satisfies.)
        assert!(
            !page.is_empty() || ran.len() == 1,
            "`{q}` printed nothing: the hint before it named a page past the end"
        );
        if let Some(c) = stated.captures(hint) {
            totals.push(c[1].parse().expect("digits"));
        }
        let next = next_query_in_hint(&q, hint);
        sizes.push(page.len());
        all.extend(page);
        match next {
            Some(n) => q = n,
            None => return (all, ran, sizes, totals),
        }
    }
    panic!("`{query}`: no end after 1,000 pages")
}

/// The line count `sh` gives the stream behind `handle` (`grep -c ''` over the same bytes the
/// query reads), the number a hint's `which has N lines` has to equal.
fn sh_line_count(
    dir: &std::path::Path,
    handle: &str,
    subs: &[(String, std::path::PathBuf)],
) -> usize {
    shell_truth(dir, &format!("grep -c '' {handle}"), subs)
        .trim()
        .parse()
        .expect("a count")
}

/// Every `which has N lines` a walk met says `n`, and the walk met at least `at_least` of them.
fn assert_stated_totals(totals: &[usize], n: usize, at_least: usize) {
    assert!(
        totals.len() >= at_least && totals.iter().all(|&t| t == n),
        "the stream has {n} lines; the hints said {totals:?}"
    );
}

/// `got` equals `want`, line for line; the failure names the first difference instead of dumping
/// both sides (a followed walk is thousands of lines).
fn assert_same_lines(got: &[String], want: &[String], what: &str) {
    if got != want {
        let at = got
            .iter()
            .zip(want)
            .position(|(g, w)| g != w)
            .unwrap_or(got.len().min(want.len()));
        panic!(
            "{what}: {} lines, want {}; first difference at line {}: got {:?}, want {:?}",
            got.len(),
            want.len(),
            at + 1,
            got.get(at),
            want.get(at)
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_plain_slice_window_names_the_next_one_and_the_hints_reach_the_last_line_once() {
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq 1 3000".into(), seq_lines(3000), String::new(), 0);

    // The reviewer's reproduction: the second page of `cat`, run alone.
    let (page, hint, parsed) = lines_and_hint(&ctx, format!("sed -n '101,200p' {id}")).await;
    assert!(parsed.get("truncated").is_none(), "{parsed}");
    assert_eq!(page.len(), 100);
    assert!(
        hint.contains(&format!("Next page: sed -n '201,300p' {id}.")),
        "a full window that ends at line 200 of 3,000 names the next one: {hint:?}"
    );

    // Followed from `cat`: all 3,000 lines, each once, in 30 pages, and the last page is silent.
    let (all, ran, _, totals) = follow_every_hint(&ctx, &format!("cat {id}")).await;
    let full: Vec<String> = (1..=3000).map(|i| i.to_string()).collect();
    assert_same_lines(&all, &full, "cat, then the hints");
    assert_stated_totals(&totals, 3000, 20);
    assert_eq!(ran.len(), 30, "{ran:?}");
    assert_eq!(ran[29], format!("sed -n '2901,3000p' {id}"));
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_plain_slice_window_names_a_next_one_only_while_a_line_is_left() {
    // The window `sed -n '101,200p'` over a stream of N lines: it ends AT the last line when N is
    // 200, so nothing follows; at 201 one line does. Each side of the edge is a separate case, so
    // neither `<` for `<=` nor an off-by-one in the start of the next window passes.
    let (_dir, ctx) = project_ctx().await;
    for (total, want_next) in [(199usize, false), (200, false), (201, true), (250, true)] {
        let id = ctx
            .output_buffer
            .store("gen".into(), seq_lines(total), String::new(), 0);
        let (page, hint, parsed) = lines_and_hint(&ctx, format!("sed -n '101,200p' {id}")).await;
        assert!(parsed.get("truncated").is_none(), "{total}: {parsed}");
        assert_eq!(page.len(), 100.min(total - 100), "{total}");
        if want_next {
            assert!(
                hint.contains(&format!("Next page: sed -n '201,300p' {id}.")),
                "{total} lines: {hint:?}"
            );
        } else {
            assert!(
                hint.is_empty(),
                "{total} lines: ends at the last line: {hint:?}"
            );
        }
    }
    // The first window of a stream of exactly 100 lines, and of 101.
    for (total, want_next) in [(100usize, false), (101, true)] {
        let id = ctx
            .output_buffer
            .store("gen".into(), seq_lines(total), String::new(), 0);
        let (_, hint, _) = lines_and_hint(&ctx, format!("sed -n '1,100p' {id}")).await;
        assert_eq!(
            hint.contains(&format!("Next page: sed -n '101,200p' {id}.")),
            want_next,
            "{total} lines: {hint:?}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn only_a_window_of_exactly_100_lines_names_a_next_page() {
    // A caller's own narrower or wider slice is not a page of the hint's: it keeps its answer
    // bare, as before. (Wider than 100 short lines fits whole; narrower prints less than a page.)
    let (_dir, ctx) = project_ctx().await;
    let id = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    for q in [
        format!("sed -n '101,199p' {id}"),
        format!("sed -n '101,150p' {id}"),
        format!("sed -n '101,201p' {id}"),
        format!("sed -n '101,300p' {id}"),
        // A reversed range prints line 101 only.
        format!("sed -n '101,100p' {id}"),
    ] {
        let (_, hint, parsed) = lines_and_hint(&ctx, q.clone()).await;
        assert!(parsed.get("truncated").is_none(), "`{q}`: {parsed}");
        assert!(hint.is_empty(), "`{q}`: {hint:?}");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn the_hints_alone_reach_the_last_line_once_when_pages_are_cut_by_bytes() {
    // A page of 100 lines is sometimes cut by the byte budget short of 100, so the next window
    // starts mid-way, and sometimes fits whole as 100 lines. The premise is both kinds of page;
    // following the hints must still return every line once, in order, and stop.
    let (_dir, ctx) = project_ctx().await;
    // Blocks of 90 short lines then 90 long ones, so a window holds a different mix each time.
    let width = |i: usize| {
        if (i / 90).is_multiple_of(2) {
            10 + i % 20
        } else {
            150 + (i * 7919) % 100
        }
    };
    let varied: Vec<String> = (1..=1200usize)
        .map(|i| format!("v{i:04} {}", "x".repeat(width(i))))
        .collect();
    let text: String = varied.iter().map(|l| format!("{l}\n")).collect();
    let id = ctx
        .output_buffer
        .store("gen".into(), text, String::new(), 0);
    let (all, ran, sizes, totals) = follow_every_hint(&ctx, &format!("cat {id}")).await;
    assert_stated_totals(&totals, 1200, 1);
    assert!(
        sizes.contains(&100) && sizes.iter().any(|&n| n < 100 && n > 30),
        "the premise needs whole 100-line pages and byte-cut ones: {sizes:?}"
    );
    assert_same_lines(&all, &varied, &format!("{} pages", ran.len()));
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_window_of_the_err_stream_pages_that_stream() {
    let (_dir, ctx) = project_ctx().await;
    // Wide enough that `cat` of the stream is cut (800 short lines would fit whole: one page, no walk).
    let rows: Vec<String> = (1..=800)
        .map(|i| format!("{i} {}", "e".repeat(40)))
        .collect();
    let err: String = rows.iter().map(|l| format!("{l}\n")).collect();
    let id = ctx.output_buffer.store("cmd".into(), "x\n".into(), err, 0);
    let (page, hint, _) = lines_and_hint(&ctx, format!("sed -n '101,200p' {id}.err")).await;
    assert_eq!(page.len(), 100);
    assert!(
        hint.contains(&format!("Next page: sed -n '201,300p' {id}.err.")),
        "the next window is on the stream that was read: {hint:?}"
    );
    let (all, ran, sizes, totals) = follow_every_hint(&ctx, &format!("cat {id}.err")).await;
    assert!(sizes.len() > 4, "the premise needs pages: {sizes:?}");
    assert_same_lines(
        &all,
        &rows,
        &format!("{} pages of the err stream", ran.len()),
    );
    assert_stated_totals(&totals, 800, 3);
}

#[cfg(unix)]
#[tokio::test]
async fn a_tool_ref_counts_its_lines_as_the_sed_that_reads_it_does() {
    // `run_command` hands a `@tool_*` ref to the shell PRETTY-PRINTED. The hint's end-of-stream test
    // must count that text: counted as the stored compact JSON (one line) no window ever has a
    // line left, and counted with `\n` expanded (as `read_file` does) a last window names a next
    // one that prints nothing.
    //
    // The pretty text is a MULTIPLE OF 100 lines (724 rows of 4 lines and 4 more: 2,900), so the
    // last window is a FULL page that ends at the last line. A fixture that ends mid-window (the
    // 2,804 lines of 700 rows did) has no full page near the end for a miscount to hint at, and
    // the `\n`-expanded count (each row's `"x\ny"` is two lines to `read_file`) survived it.
    let (dir, ctx) = project_ctx().await;
    let rows: Vec<Value> = (0..724).map(|i| json!({ "i": i, "s": "x\ny" })).collect();
    let blob = serde_json::to_string(&json!({ "rows": rows })).unwrap();
    let t = ctx.output_buffer.store_tool("probe", blob.clone());
    let pretty =
        serde_json::to_string_pretty(&serde_json::from_str::<Value>(&blob).unwrap()).unwrap();
    let path = dir.path().join("tool.json");
    std::fs::write(&path, &pretty).unwrap();
    let want: Vec<String> = pretty.lines().map(str::to_string).collect();
    assert_eq!(want.len(), 2_900, "the premise: a multiple of 100 lines");
    let subs = [(t.clone(), path)];
    assert_eq!(sh_line_count(dir.path(), &t, &subs), 2_900);

    let (all, ran, _, totals) = follow_every_hint(&ctx, &format!("cat {t}")).await;
    assert_same_lines(&all, &want, &format!("{} pages", ran.len()));
    assert_eq!(ran.len(), 29, "29 pages of 100 lines");
    assert_stated_totals(&totals, 2_900, 20);
    // The oracle: the real `sh` over the pretty text prints the same lines.
    // (The pretty text has no final newline, so `cat` prints none either.)
    let truth = shell_truth(dir.path(), &format!("cat {t}"), &subs);
    assert!(
        all.join("\n") == truth,
        "the walk differs from what `cat` prints"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_tool_ref_that_is_not_json_is_counted_as_stored() {
    // `resolve_refs` pretty-prints a `@tool_*` ref only when it parses as JSON; any other text is
    // handed to the shell as it is stored. The count has the same fallback: a text that failed to
    // parse and was counted as nothing would never have a line left.
    let (dir, ctx) = project_ctx().await;
    let full: Vec<String> = (1..=500)
        .map(|i| format!("row{i} {}", "t".repeat(40)))
        .collect();
    let text: String = full.iter().map(|l| format!("{l}\n")).collect();
    let t = ctx.output_buffer.store_tool("probe", text.clone());
    assert!(serde_json::from_str::<Value>(&text).is_err(), "the premise");
    let path = dir.path().join("plain.txt");
    std::fs::write(&path, &text).unwrap();
    assert_eq!(sh_line_count(dir.path(), &t, &[(t.clone(), path)]), 500);
    let (all, ran, sizes, totals) = follow_every_hint(&ctx, &format!("cat {t}")).await;
    assert!(sizes.len() > 4, "the premise needs pages: {sizes:?}");
    assert_same_lines(&all, &full, &format!("{} pages", ran.len()));
    assert_stated_totals(&totals, 500, 2);
}

#[cfg(unix)]
#[tokio::test]
async fn a_file_ref_full_window_names_the_next_one() {
    let (dir, ctx) = project_ctx().await;
    let path = dir.path().join("big.txt");
    let full: Vec<String> = (1..=500)
        .map(|i| format!("row{i} {}", "f".repeat(40)))
        .collect();
    let text: String = full.iter().map(|l| format!("{l}\n")).collect();
    std::fs::write(&path, &text).unwrap();
    let id = ctx
        .output_buffer
        .store_file(path.to_string_lossy().to_string(), text);
    assert!(id.starts_with("@file_"), "{id}");
    let (all, ran, sizes, totals) = follow_every_hint(&ctx, &format!("cat {id}")).await;
    assert!(sizes.len() > 4, "the premise needs pages: {sizes:?}");
    assert_same_lines(&all, &full, &format!("{} pages", ran.len()));
    assert_stated_totals(&totals, 500, 2);
}

#[cfg(unix)]
#[tokio::test]
async fn a_background_log_full_window_names_the_next_one() {
    let (_dir, ctx) = project_ctx().await;
    let started = RunCommand
        .call(
            json!({
                "command": "seq -f 'row%g_gggggggggggggggggggggggggggggggggggggggg' 1 500",
                "run_in_background": true,
                "timeout_secs": 10
            }),
            &ctx,
        )
        .await
        .unwrap();
    let bg = started["output_id"].as_str().unwrap().to_string();
    let mut done = false;
    for _ in 0..100 {
        let (_, parsed) = buffer_query(&ctx, format!("grep -c '' {bg}")).await;
        if parsed["stdout"].as_str().unwrap_or_default().trim() == "500" {
            done = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(done, "the background job never wrote 500 lines");
    let (all, ran, sizes, totals) = follow_every_hint(&ctx, &format!("cat {bg}")).await;
    assert!(sizes.len() > 4, "the premise needs pages: {sizes:?}");
    // 500 lines, a multiple of the page: the last window is a full page that ends at the last line,
    // so a count that is too high by half a page hints at an empty page and the walk fails.
    let full: Vec<String> = (1..=500)
        .map(|i| format!("row{i}_{}", "g".repeat(40)))
        .collect();
    assert_same_lines(&all, &full, &format!("{} pages", ran.len()));
    assert_stated_totals(&totals, 500, 2);
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_plain_slice_window_that_fits_only_without_its_hint_is_cut_not_rebuffered() {
    // The sibling of `a_full_pager_window_that_fits_only_without_its_hint_is_cut_not_rebuffered`
    // for a plain slice. The hint must be inside the inline gate: a page that fits only WITHOUT it
    // would otherwise come back whole, cross the limit with the hint added, and be re-buffered
    // under `@tool_*`. Widths are swept so one lands in the ~150 B the hint costs.
    let (_dir, ctx) = project_ctx().await;
    let mut seen_full = false;
    let mut seen_cut = false;
    for width in 85..=100usize {
        let stored: String = (0..150)
            .map(|i| format!("p{i:03} {}\n", "q".repeat(width - 5)))
            .collect();
        let id = ctx
            .output_buffer
            .store("cmd".into(), stored, String::new(), 0);
        let query = format!("sed -n '1,100p' {id}");
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        assert!(
            handles_named(&text).is_subset(&handles_named(&query)),
            "width {width}: re-buffered: {text:.200}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "width {width}: {} B",
            text.len()
        );
        let hint = parsed["hint"].as_str().unwrap_or_default();
        let next = next_query_in_hint(&query, hint)
            .unwrap_or_else(|| panic!("width {width}: no route: {hint}"));
        let shown = parsed["stdout"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .count();
        // Whole or cut, the next page starts right after the last line shown.
        assert_eq!(
            next,
            format!("sed -n '{},{}p' {id}", shown + 1, shown + 100),
            "width {width}"
        );
        seen_full |= parsed.get("truncated").is_none();
        seen_cut |= parsed.get("truncated").is_some();
    }
    assert!(seen_full, "the premise: some width fits whole");
    assert!(seen_cut, "the premise: some width is cut");
}

#[cfg(unix)]
#[tokio::test]
async fn a_full_plain_slice_window_stays_inside_the_limit_at_every_byte_near_the_edge() {
    // `..._is_cut_not_rebuffered` steps the width of every line, so its sizes are 100 B apart. This
    // one steps the LAST line of the window one byte at a time across the edge where the response
    // (stdout and hint) crosses the inline limit: whole below it, cut above it, and at no byte
    // re-buffered or over the limit, with the next page starting right after the last line shown.
    let (_dir, ctx) = project_ctx().await;
    let (mut whole, mut cut) = (0, 0);
    for last in 200..=800usize {
        let mut stored: String = (0..99)
            .map(|i| format!("p{i:03} {}\n", "q".repeat(85)))
            .collect();
        stored.push_str(&format!("last {}\n", "z".repeat(last)));
        stored.push_str(&"tail\n".repeat(50));
        let id = ctx
            .output_buffer
            .store("cmd".into(), stored, String::new(), 0);
        let query = format!("sed -n '1,100p' {id}");
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        assert!(
            handles_named(&text).is_subset(&handles_named(&query)),
            "last line {last} B: re-buffered: {text:.200}"
        );
        assert!(
            !crate::tools::exceeds_inline_limit(&text),
            "last line {last} B: {} B",
            text.len()
        );
        let hint = parsed["hint"].as_str().unwrap_or_default();
        let shown = parsed["stdout"]
            .as_str()
            .unwrap_or_default()
            .lines()
            .count();
        assert_eq!(
            next_query_in_hint(&query, hint),
            Some(format!("sed -n '{},{}p' {id}", shown + 1, shown + 100)),
            "last line {last} B: {hint}"
        );
        if parsed.get("truncated").is_some() {
            cut += 1;
        } else {
            whole += 1;
        }
    }
    assert!(
        whole > 0 && cut > 0,
        "the sweep must cross the edge: {whole} whole, {cut} cut"
    );
}

// ---- the own-stderr remedy of a buffer query reaches EVERY stage of its pipeline ----
//
// BUG docs/issues/archive/2026-10-08-own-stderr-remedy-of-a-multi-stage-pipeline-redirects-only-its-last-command.md
// The marker on a cut query-own stderr said to append `2>&1 >/dev/null | cut -c1-4000`. A redirection
// applies to the LAST command of a pipeline: for `cat @X | tr -d '\n' >&2 | cat` the appended form
// read the last `cat`'s stderr (nothing) and left the `tr`'s on the terminal; a stage that itself
// ends in `>&2` swallowed it (`>&2 2>&1 >/dev/null` leaves fd 1 on the old stderr). The marker now
// names the query inside a subshell, `( QUERY; ) 2>&1 >/dev/null | cut -c1-4000`: when `QUERY` is
// one redirected subshell, dash drops its redirects inside the brace form. These tests run the
// recovery route under the real `sh` and compare bytes with the stream that was cut.

/// What `query` writes to STDERR when run by `sh` over real files: [`shell_truth`]'s twin.
fn shell_stderr(
    dir: &std::path::Path,
    query: &str,
    subs: &[(String, std::path::PathBuf)],
) -> String {
    let mut cmd = query.to_string();
    for (handle, path) in subs {
        cmd = cmd.replace(handle.as_str(), &path.display().to_string());
    }
    let out = std::process::Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    String::from_utf8(out.stderr).unwrap()
}

/// What `cut -c1-4000` prints for `text`, ASCII: every line cut to 4,000 characters, newlines kept.
fn cut_4000(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| {
            let body = line.trim_end_matches('\n');
            let nl = &line[body.len()..];
            format!("{}{nl}", body.chars().take(4000).collect::<String>())
        })
        .collect()
}

/// Two streams equal but for their final newline. `cut` terminates a last line that had none (GNU
/// does; the standard leaves a file without a final newline unspecified), so a stream that ends
/// mid-line, as `tr -d '\n'` makes one, comes back one byte longer.
fn same_stream(a: &str, b: &str) -> bool {
    a.trim_end_matches('\n') == b.trim_end_matches('\n')
}

/// The route the own-stderr marker names: substitute the WHOLE query into the actual template,
/// keeping the wrapper and stage exactly as emitted.
fn wrapped_route_in(stderr: &str, query: &str) -> Option<String> {
    let template = regex::Regex::new(r"`([^`]*QUERY[^`]*)`")
        .expect("static pattern")
        .captures(stderr)?[1]
        .to_string();
    Some(template.replacen("QUERY", query, 1))
}

#[cfg(unix)]
#[tokio::test]
async fn the_own_stderr_route_reads_the_stream_that_was_cut_whatever_the_shape_of_the_pipeline() {
    let (dir, ctx) = project_ctx().await;
    // 40 lines of 115 B and one of 5,000 B: the first 20 lines alone are over the 2,000 B the
    // response carries (so the cut is the byte cut, whose marker holds the remedy), and a line
    // `cut -c1-4000` has to cut.
    let mut lines: Vec<String> = (1..=40)
        .map(|i| format!("r{i:02} {}", "a".repeat(110)))
        .collect();
    lines.push("W".repeat(5_000));
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    let a = ctx
        .output_buffer
        .store("gen".into(), text.clone(), String::new(), 0);
    let pa = dir.path().join("a.txt");
    std::fs::write(&pa, &text).unwrap();
    let subs = vec![(a.clone(), pa)];
    let awk_err = r#"awk 'NR==1{print} END{ for(i=0;i<5000;i++) s=s "E"; print s | "cat 1>&2"}'"#;

    // (query, whether the OLD route, the stage appended, reads the right stream). Both are
    // executed below: a shape that did not discriminate would leave the fix unproved.
    let shapes: Vec<(String, bool)> = vec![
        // One command whose stderr is its own: appending was right, wrapping is too.
        (format!("{awk_err} {a}"), true),
        (format!("({awk_err} {a})"), true),
        // One command that redirects its own stdout: `>&2 2>&1 >/dev/null` leaves fd 1 on the
        // terminal's stderr, and the pipe gets nothing.
        (format!("cut -c1-9000 {a} >&2"), false),
        (format!("! cut -c1-9000 {a} >&2"), false),
        // A pipeline whose stderr writer is its first, middle and last stage.
        (format!("{awk_err} {a} | cat"), false),
        (format!("cat {a} >&2 | tr a b"), false),
        (format!("cat {a} >&2 | cat | cat"), false),
        (format!("cat {a} | tr a b >&2 | cat"), false),
        (format!("cat {a} | cat >&2 | wc -l"), false),
        (format!("cat {a} | tr a b >&2"), false),
        // The reviewer's input.
        (format!("cat {a} | tr -d '\\n' >&2 | cat"), false),
    ];
    let mut old_wrong = 0;
    for (query, old_ok) in &shapes {
        let expected = cut_4000(&shell_stderr(dir.path(), query, &subs));
        assert!(
            expected.len() > 2_000,
            "`{query}`: the premise is a stderr the response cuts: {} B",
            expected.len()
        );

        // The old route, executed: right or wrong as recorded.
        let old = shell_truth(
            dir.path(),
            &format!("{query} 2>&1 >/dev/null | cut -c1-4000"),
            &subs,
        );
        assert_eq!(
            same_stream(&old, &expected),
            *old_ok,
            "`{query}`: the appended route read {} B, the cut stream is {} B",
            old.len(),
            expected.len()
        );
        old_wrong += usize::from(!old_ok);

        // The wrapped route, under the same `sh`: exactly the stream that was cut, cut again.
        let wrapped = shell_truth(
            dir.path(),
            &format!("( {query}; ) 2>&1 >/dev/null | cut -c1-4000"),
            &subs,
        );
        assert!(
            same_stream(&wrapped, &expected),
            "`{query}`: the wrapped route read {} B, the cut stream is {} B",
            wrapped.len(),
            expected.len()
        );

        // Through the tool: the marker of the cut stderr names that route, and running it, as
        // the marker states it, returns those bytes. Claimed only for a result under the inline
        // limit, as here (<= ~9 KB): the route holds `>/dev/null`, which makes it a query that is
        // not buffer-only, so a larger answer would be buffered under a handle of its own (the
        // stream stays reachable through it).
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        let stderr = parsed["stderr"]
            .as_str()
            .unwrap_or_else(|| panic!("`{query}`: no stderr: {text:.300}"));
        assert!(stderr.contains("bytes shown"), "`{query}`: {stderr:.0}");
        let route = wrapped_route_in(stderr, query)
            .unwrap_or_else(|| panic!("`{query}`: no route in the marker: {stderr:.0}"));
        assert_eq!(
            route,
            format!("( {query}; ) 2>&1 >/dev/null | cut -c1-4000")
        );
        let (t2, p2) = buffer_query(&ctx, route.clone()).await;
        assert!(
            p2.get("output_id").is_none() && !has_tool_handle(&t2),
            "`{route}` made a handle: {t2:.300}"
        );
        assert!(
            same_stream(p2["stdout"].as_str().unwrap_or_default(), &expected),
            "`{route}` returned {} B, the cut stream is {} B: {t2:.200}",
            p2["stdout"].as_str().unwrap_or_default().len(),
            expected.len()
        );
    }
    assert!(
        old_wrong >= 8,
        "the premise: the appended route reads the wrong stream for most pipelines"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn an_own_stderr_recovery_route_preserves_redirected_subshell_output() {
    let (dir, ctx) = project_ctx().await;
    for (label, expected, marker) in [
        ("lines", "payload\n".repeat(30), "20 of 30 lines shown"),
        ("bytes", format!("{}\n", "x".repeat(3000)), "bytes shown"),
    ] {
        let (a, sub) = stored_and_real(dir.path(), &ctx, &expected, &format!("{label}.txt"));
        let query = format!("(cat {a}) >&2");
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        let stderr = parsed["stderr"].as_str().expect("the query's own stderr");
        assert!(stderr.contains(marker), "{label}: {text:.300}");
        let route = wrapped_route_in(stderr, &query)
            .unwrap_or_else(|| panic!("{label}: no recovery route: {stderr:.300}"));
        assert_eq!(
            shell_truth(dir.path(), &route, &[sub]),
            expected,
            "{label}: the emitted route must preserve the subshell's redirected stdout"
        );
        let (recovered_text, recovered) = buffer_query(&ctx, route).await;
        assert_eq!(
            recovered["stdout"].as_str().unwrap_or_default(),
            expected,
            "{label}: the recovery route through run_command: {recovered_text:.300}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn the_wrapped_own_stderr_route_matches_the_cut_stream_for_every_shape_the_marker_accepts() {
    // The 80-query adversarial table, every shape one pipeline by `is_one_pipeline` (the ones the
    // marker offers a route for; that predicate is the only production code this test calls, the
    // rest is `sh`): the wrapped route, run by `sh`, prints what `cut -c1-4000` makes of the stream
    // that query writes to stderr. Most rows write none to the stream the route reads, so
    // "both empty" says little: it holds for a wrapped text that is VALID `sh` and, equally, for one
    // that is not (`{ echo hi } 2>&1 >/dev/null | cut` prints a syntax error on the OUTER shell's
    // stderr and nothing on stdout). So the outer shell's stderr is asserted empty for every route,
    // which only valid text satisfies; and the rows that DO write to the stream (`>&2`,
    // `tee /dev/stderr`, `1>&2 | cat`) make it a byte comparison.
    let (dir, ctx) = project_ctx().await;
    let a_out: String = (1..=3000).map(|i| format!("{i} a'b\"c#d|e;f\n")).collect();
    let a = ctx
        .output_buffer
        .store("gen".into(), a_out.clone(), String::new(), 0);
    let e_err: String = (1..=3000).map(|i| format!("e{i}\n")).collect();
    let e = ctx
        .output_buffer
        .store("gen".into(), "x\n".into(), e_err.clone(), 0);
    let (pa, pe_out, pe_err) = (
        dir.path().join("a.txt"),
        dir.path().join("e.txt"),
        dir.path().join("e.err.txt"),
    );
    std::fs::write(&pa, &a_out).unwrap();
    std::fs::write(&pe_out, "x\n").unwrap();
    std::fs::write(&pe_err, &e_err).unwrap();
    let subs = vec![
        (format!("{e}.err"), pe_err),
        (e.clone(), pe_out),
        (a.clone(), pa),
    ];
    let (mut wrapped_rows, mut nonempty) = (0, 0);
    // Each row as it is, and with its last command's stdout sent to stderr (`>&2`): the second
    // puts the row's own output in the stream the route reads, so most rows stop being empty.
    let mut queries: Vec<(String, String)> = Vec::new();
    for (template, _) in adversarial_table() {
        let query = template.replace("{A}", &a).replace("{E}", &e);
        queries.push((template.to_string(), query.clone()));
        queries.push((format!("{template} >&2"), format!("{query} >&2")));
    }
    for (template, query) in queries {
        // Two writers on one stream (`tee /dev/stderr` and the added `>&2`) interleave in no fixed
        // order, so no byte comparison of the stream is possible.
        if template.contains("/dev/stderr") && template.ends_with(" >&2") {
            continue;
        }
        if !crate::util::path_security::is_one_pipeline(&query)
            || query.starts_with("rg")
            || query.starts_with("shuf")
        {
            continue;
        }
        let own = shell_stderr(dir.path(), &query, &subs);
        // A `sh` that rejects the syntax (dash: `|&`, `<(`) writes a parse error that names its
        // own text, which is no stream of the query's to compare.
        if own.to_lowercase().contains("syntax error") {
            eprintln!("{template:?}: this sh rejects the syntax, skipped");
            continue;
        }
        wrapped_rows += 1;
        nonempty += usize::from(!own.is_empty());
        let route = format!("( {query}; ) 2>&1 >/dev/null | cut -c1-4000");
        let wrapped = shell_truth(dir.path(), &route, &subs);
        let outer = shell_stderr(dir.path(), &route, &subs);
        assert!(
            outer.is_empty(),
            "{template:?}: the wrapped route is not valid sh here: {outer:.200}"
        );
        assert!(
            same_stream(&wrapped, &cut_4000(&own)),
            "{template:?}: the wrapped route read {} B, the stream is {} B",
            wrapped.len(),
            cut_4000(&own).len()
        );
    }
    eprintln!("{wrapped_rows} rows compared, {nonempty} write to stderr");
    assert!(
        wrapped_rows >= 75,
        "only {wrapped_rows} rows were compared: the table moved"
    );
    assert!(
        nonempty >= 30,
        "only {nonempty} rows write to stderr: the comparison proves too little"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_shape_the_wrap_cannot_take_gets_the_narrow_advice_not_a_route() {
    // A list, a comment, a heredoc: wrapped, the `; }` would close the wrong thing or be swallowed.
    // The marker says to narrow the query, and names no route. (`$(…)` is refused by the same
    // predicate, `can_wrap_query`'s unit test says so, but no marker reaches it: a word that starts
    // with `$` reads as a path, so such a query is not a buffer query and its stderr is not cut.)
    let (_dir, ctx) = project_ctx().await;
    let a = ctx
        .output_buffer
        .store("seq".into(), seq_lines(10), String::new(), 0);
    let big = ctx.output_buffer.store(
        "gen".into(),
        format!("{}\n", "e".repeat(3_000)),
        String::new(),
        0,
    );
    let noisy = "printf '%3000s\\n' | tr ' ' e >&2";
    for query in [
        format!("grep -c 1 {a}; {noisy}"),
        format!("grep -c 1 {a} && {noisy}"),
        format!("{noisy} # {a}"),
        format!("cat {big} >&2 <<EOF\nx\nEOF"),
    ] {
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        let stderr = parsed["stderr"].as_str().unwrap_or_default();
        assert!(
            stderr.contains("bytes shown"),
            "`{query}`: the premise is a cut stderr: {text:.300}"
        );
        assert!(!stderr.contains("QUERY"), "`{query}`: {stderr:.0}");
        assert!(
            stderr.contains("narrow the query"),
            "`{query}`: {stderr:.0}"
        );
    }
}
// ---- an own stderr cut ONLY by the 20-line cap carries the route to the rest ----
//
// `cat @A >&2` over 3,000 short lines answered `{"stderr":"1\n..\n20","stderr_shown":20,
// "stderr_total":3000}`: the 20 lines fit the byte budget, so the byte cut (whose marker holds the
// remedy) never ran and the counters were all the caller had. The query's own stderr is stored
// nowhere, so without the marker no route to line 21 exists in the response at all.

/// `seq 1 n` as a stored buffer and as a real file: the pair `shell_truth` substitutes.
fn stored_and_real(
    dir: &std::path::Path,
    ctx: &ToolContext,
    text: &str,
    name: &str,
) -> (String, (String, std::path::PathBuf)) {
    let handle = ctx
        .output_buffer
        .store("gen".into(), text.to_string(), String::new(), 0);
    let path = dir.join(name);
    std::fs::write(&path, text).unwrap();
    (handle.clone(), (handle, path))
}
/// Each window `(a, b)` of `expected`, read through the tool as `route | sed -n 'a,bp'`, equals the
/// lines `a..=b` of the stream `sh` prints for the route. The route is no buffer query (it holds
/// `>/dev/null`), so past ~10 KB it comes back as a summary and the pager is what returns a window
/// whole; every window here is under the inline limit.
async fn assert_stream_windows_through_the_tool(
    ctx: &ToolContext,
    route: &str,
    expected: &str,
    windows: &[(usize, usize)],
) {
    let lines: Vec<&str> = expected.lines().collect();
    for &(a, b) in windows {
        let query = format!("{route} | sed -n '{a},{b}p'");
        let (text, parsed) = buffer_query(ctx, query.clone()).await;
        let got: Vec<&str> = parsed["stdout"]
            .as_str()
            .unwrap_or_else(|| panic!("`{query}` printed nothing: {text:.300}"))
            .lines()
            .collect();
        assert_eq!(got, lines[a - 1..b.min(lines.len())], "`{query}`");
    }
}

#[cfg(unix)]
#[tokio::test]
async fn an_own_stderr_cut_only_by_the_line_cap_names_the_route_to_the_rest() {
    let (dir, ctx) = project_ctx().await;
    let (a, sub) = stored_and_real(dir.path(), &ctx, &seq_lines(3000), "a.txt");
    let subs = vec![sub];
    let query = format!("cat {a} >&2");

    let (text, parsed) = buffer_query(&ctx, query.clone()).await;
    assert_eq!(parsed["stderr_shown"], 20, "{text:.300}");
    assert_eq!(parsed["stderr_total"], 3000, "{text:.300}");
    let stderr = parsed["stderr"].as_str().expect("the query's own stderr");
    assert!(
        !stderr.contains("bytes shown"),
        "the premise is a cut by the LINE cap alone: {stderr:.0}"
    );
    assert!(
        stderr.contains("20 of 3000 lines shown"),
        "a cut by lines says so: {stderr:.0}"
    );
    assert!(stderr.contains("stored nowhere"), "{stderr:.0}");

    // The route the marker names, executed under the real `sh`: all 3,000 lines of the stream
    // that was cut.
    let route = wrapped_route_in(stderr, &query)
        .unwrap_or_else(|| panic!("no route in the marker: {stderr:.0}"));
    assert_eq!(
        route,
        format!("( {query}; ) 2>&1 >/dev/null | cut -c1-4000")
    );
    let expected = cut_4000(&shell_stderr(dir.path(), &query, &subs));
    assert_eq!(expected.lines().count(), 3000);
    assert!(same_stream(
        &shell_truth(dir.path(), &route, &subs),
        &expected
    ));

    // And through the tool. The route holds `>/dev/null`, so it is no buffer query and its 3,000
    // lines come back as a summary behind a handle of its own; a window of it, by a pager the
    // caller appends, is what comes back whole.
    assert_stream_windows_through_the_tool(
        &ctx,
        &route,
        &expected,
        &[(21, 120), (1000, 1099), (2901, 3000)],
    )
    .await;
}
#[cfg(unix)]
#[tokio::test]
async fn the_route_of_a_line_cut_own_stderr_reads_the_stream_for_every_pipeline_shape() {
    // The wrapped route of the byte cut is proved for these shapes; a cut by LINES takes the same
    // remedy, and each shape here is run twice: over short lines (the 20 shown fit the byte
    // budget: cut by lines alone) and over 115 B lines (the 20 shown are 2,300 B: cut by lines AND
    // bytes, the byte marker standing for both).
    let (dir, ctx) = project_ctx().await;
    let short = seq_lines(3000);
    let long: String = (1..=3000)
        .map(|i| format!("r{i:04} {}\n", "a".repeat(108)))
        .collect();
    for (what, text, needle, windows) in [
        (
            "short lines",
            short,
            "20 of 3000 lines shown",
            vec![(21, 120), (2901, 3000)],
        ),
        (
            "long lines",
            long,
            "bytes shown",
            vec![(21, 60), (2961, 3000)],
        ),
    ] {
        let (a, sub) = stored_and_real(
            dir.path(),
            &ctx,
            &text,
            &format!("{}.txt", what.replace(' ', "_")),
        );
        let subs = vec![sub];
        let shapes = [
            // One command.
            format!("cat {a} >&2"),
            format!("! cat {a} >&2"),
            format!("(cat {a} >&2)"),
            // A pipeline whose stderr writer is its first, middle and last stage.
            format!("cat {a} >&2 | tr a b"),
            format!("cat {a} | tr a b >&2 | cat"),
            format!("cat {a} | cat >&2 | wc -l"),
            format!("cat {a} | tr a b >&2"),
        ];
        for query in shapes {
            let expected = cut_4000(&shell_stderr(dir.path(), &query, &subs));
            assert_eq!(expected.lines().count(), 3000, "{what}: `{query}`");

            let (text, parsed) = buffer_query(&ctx, query.clone()).await;
            assert_eq!(parsed["stderr_shown"], 20, "{what}: `{query}`: {text:.300}");
            assert_eq!(parsed["stderr_total"], 3000, "{what}: `{query}`");
            let stderr = parsed["stderr"]
                .as_str()
                .unwrap_or_else(|| panic!("{what}: `{query}`: no stderr: {text:.300}"));
            assert!(stderr.contains(needle), "{what}: `{query}`: {stderr:.0}");
            assert_eq!(
                stderr.contains("bytes shown"),
                what == "long lines",
                "{what}: `{query}`: the premise is which cut ran"
            );
            let route = wrapped_route_in(stderr, &query)
                .unwrap_or_else(|| panic!("{what}: `{query}`: no route in {stderr:.0}"));
            assert_eq!(
                route,
                format!("( {query}; ) 2>&1 >/dev/null | cut -c1-4000")
            );

            // The route, executed under the real `sh`, prints the stream that was cut ...
            assert!(
                same_stream(&shell_truth(dir.path(), &route, &subs), &expected),
                "{what}: `{route}` does not print the cut stream"
            );
            // ... and through the tool, a window of it comes back whole.
            assert_stream_windows_through_the_tool(&ctx, &route, &expected, &windows).await;
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_stderr_that_fits_whole_is_silent_and_one_line_over_the_cap_is_not() {
    // The line cap is 20: 20 lines are the stream whole (no marker, no counters), 21 are not. Both
    // sources, since the marker's remedy differs: the query's own stderr names the rerun route, a
    // stored stderr names its `.err` handle.
    let (dir, ctx) = project_ctx().await;
    for n in [1usize, 19, 20, 21, 22] {
        let text = seq_lines(n);
        let (a, _) = stored_and_real(dir.path(), &ctx, &text, "own.txt");
        let (text_own, own) = buffer_query(&ctx, format!("cat {a} >&2")).await;
        let (id, text_stored, stored) = query_stored(&ctx, "x\n".into(), text.clone(), |id| {
            format!("grep -c x {id}")
        })
        .await;
        for (source, parsed, resp) in [("own", &own, &text_own), ("stored", &stored, &text_stored)]
        {
            if n <= 20 {
                assert_eq!(parsed["stderr"], text.as_str(), "{source} {n}: {resp:.300}");
                assert!(parsed.get("stderr_shown").is_none(), "{source} {n}");
                assert!(parsed.get("stderr_total").is_none(), "{source} {n}");
                continue;
            }
            let stderr = parsed["stderr"].as_str().expect("a cut stderr");
            assert!(
                stderr.starts_with(&seq_lines(20)),
                "{source} {n}: the first twenty lines stay: {stderr:.80}"
            );
            assert!(
                stderr.contains(&format!("20 of {n} lines shown")),
                "{source} {n}: {stderr:.0}"
            );
            assert_eq!(parsed["stderr_shown"], 20, "{source} {n}");
            assert_eq!(parsed["stderr_total"], n, "{source} {n}");
        }
        if n > 20 {
            let stored_stderr = stored["stderr"].as_str().unwrap_or_default();
            assert!(
                stored_stderr.contains(&format!("all of it: {id}.err")),
                "{n}: {stored_stderr:.0}"
            );
            assert!(!stored_stderr.contains("QUERY"), "{n}: {stored_stderr:.0}");
            let own_stderr = own["stderr"].as_str().unwrap_or_default();
            assert!(
                own_stderr.contains("stored nowhere"),
                "{n}: {own_stderr:.0}"
            );
            assert!(!own_stderr.contains(".err"), "{n}: {own_stderr:.0}");
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_shape_the_wrap_cannot_take_and_cut_by_lines_alone_gets_the_narrow_advice() {
    // The gate is the byte cut's own (`can_wrap_query`): a list, a trailing comment, a heredoc.
    // 3,000 short lines are cut by the LINE cap alone here, and the marker says to narrow the
    // query, naming no route that would read the wrong stream.
    let (_dir, ctx) = project_ctx().await;
    let a = ctx
        .output_buffer
        .store("seq".into(), seq_lines(3000), String::new(), 0);
    for query in [
        format!("grep -c 1 {a}; cat {a} >&2"),
        format!("grep -c 1 {a} && cat {a} >&2"),
        format!("cat {a} >&2 # {a}"),
        format!("cat {a} >&2 <<EOF\nx\nEOF"),
    ] {
        let (text, parsed) = buffer_query(&ctx, query.clone()).await;
        let stderr = parsed["stderr"].as_str().unwrap_or_default();
        assert!(
            stderr.contains("20 of 3000 lines shown"),
            "`{query}`: the premise is a stderr cut by lines alone: {text:.300}"
        );
        assert!(!stderr.contains("bytes shown"), "`{query}`: {stderr:.0}");
        assert!(!stderr.contains("QUERY"), "`{query}`: {stderr:.0}");
        assert!(
            stderr.contains("narrow the query"),
            "`{query}`: {stderr:.0}"
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn a_line_cut_stderr_stays_inside_the_byte_budget_at_every_width_across_the_edge() {
    // Twenty shown lines of width w are 20w + 38 escaped bytes (the 19 newlines between them cost
    // two each): 1,698 to 2,118 as w goes 83 to 104, across the 2,000 B budget and across the
    // (budget - line marker) edge below it. The marker is part of the budget, so the escaped
    // length is never over it; and whichever cut ran, a marker says so and the route is in it.
    // Three zones: the lines and their marker fit; the lines fit but not beside the marker, so the
    // byte cut makes room for the marker it then carries; the lines alone are over the budget, and
    // the byte cut uses all of it.
    let (dir, ctx) = project_ctx().await;
    let (mut by_lines, mut squeezed, mut by_bytes) = (0, 0, 0);
    for w in 83..=104usize {
        let text: String = (0..30).map(|i| format!("{i:0>w$}\n")).collect();
        let (a, _) = stored_and_real(dir.path(), &ctx, &text, "w.txt");
        let query = format!("cat {a} >&2");
        let (resp, parsed) = buffer_query(&ctx, query.clone()).await;
        let stderr = parsed["stderr"]
            .as_str()
            .unwrap_or_else(|| panic!("w={w}: no stderr: {resp:.300}"));
        let escaped = crate::util::text::json_escaped_len(stderr);
        assert!(escaped <= 2_000, "w={w}: {escaped} B escaped");
        assert_eq!(parsed["stderr_total"], 30, "w={w}");
        assert!(
            wrapped_route_in(stderr, &query).is_some(),
            "w={w}: no route in {stderr:.0}"
        );
        if stderr.contains("bytes shown") {
            if 20 * w + 38 > 2_000 {
                by_bytes += 1;
                assert!(
                    escaped >= 2_000 - 3,
                    "w={w}: only {escaped} B of the budget"
                );
            } else {
                squeezed += 1;
            }
        } else {
            by_lines += 1;
            assert!(
                stderr.contains("20 of 30 lines shown"),
                "w={w}: {stderr:.0}"
            );
        }
        assert!(!has_tool_handle(&resp), "w={w}: {resp:.200}");
    }
    assert!(
        by_lines >= 3 && squeezed >= 3 && by_bytes >= 3,
        "the sweep must cross both edges: {by_lines} by lines, {squeezed} squeezed, {by_bytes} by bytes"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn the_line_marker_is_inside_the_inline_gate_at_the_exact_edge() {
    // The query's own stderr is 30 lines, cut to 20 behind a marker the response carries. Sized
    // from a MEASURED response, as the counters' edge test is: stdout of N bytes makes the compact
    // response `base + N`, so N* lands it on 10,003. A gate that did not count the marker would
    // call N*+1 small, and the response would go over the limit.
    let probe = |n: usize| async move {
        let (_dir, ctx) = project_ctx().await;
        let (_id, text, parsed) = query_stored(&ctx, "a".repeat(n), String::new(), |id| {
            format!(
                "awk 'BEGIN{{ for(i=0;i<30;i++) print \"e\" i | \"cat 1>&2\" }} {{print}}' {id}"
            )
        })
        .await;
        (text, parsed)
    };
    let (small, small_parsed) = probe(100).await;
    assert!(
        small_parsed["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("20 of 30 lines shown"),
        "the premise is the line marker: {small:.300}"
    );
    let n_edge = 10_003 - (small.len() - 100);

    let (at, at_parsed) = probe(n_edge).await;
    assert_eq!(at.len(), 10_003, "the measured size is not the edge");
    assert!(at_parsed.get("truncated").is_none(), "cut for nothing");

    let (over, over_parsed) = probe(n_edge + 1).await;
    assert_eq!(over_parsed["truncated"], true, "one byte over must be cut");
    assert!(!has_tool_handle(&over), "{over:.200}");
    assert!(
        over.len() <= 10_003,
        "{} B: the marker was not counted by the gate",
        over.len()
    );
    assert!(
        over_parsed["stderr"]
            .as_str()
            .unwrap_or_default()
            .contains("20 of 30 lines shown"),
        "{over:.300}"
    );
}
