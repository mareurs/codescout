//! Onboarding tool — gathers project facts (languages, entry points, hardware,
//! memory state) and produces a system prompt draft plus a subagent prompt
//! for exploring the project.

use crate::hardware::{detect_hardware_context, model_options_for_hardware};
use crate::prompts::builders::{
    build_buffered_onboarding_instructions, build_buffered_refresh_instructions, build_heading_map,
    build_language_patterns_memory, build_per_project_prompt, build_prompt_refresh_subagent_prompt,
    build_subagent_epilogue, build_subagent_preamble, build_synthesis_prompt,
    build_system_prompt_draft, build_workspace_instructions,
};
use serde_json::{json, Value};

use super::{parse_bool_param, Tool, ToolContext};

/// Bump when the `onboarding_prompt` surface (in `src/prompts/source.md`) or
/// `build_system_prompt_draft()` change in ways that affect the stored
/// per-project system prompt.
/// Do NOT bump for `server_instructions` surface changes — it is loaded fresh
/// at every MCP session start and has no cached copy.
/// See CLAUDE.md § "Onboarding Version" for the full decision table.
// Bumped 2026-05-07 for the prompt refactor: new memory shape
// (6 mandatory per project), Phase 6 CLAUDE.md flow, empty-stub convention.
// Bumped 2026-05-07 (server-instructions consolidation): removed ## Language Navigation
// from build_system_prompt_draft; language nav now injected dynamically via server_instructions.
// Bumped 2026-05-08 (scope ladder): librarian-guide updated for new project/repo/umbrella/all ladder.
// Bumped 2026-06-12 (system-prompt root-file fix): onboarding + synthesis now write
// .codescout/system-prompt.md directly via create_file instead of memory(write,
// topic="system-prompt"). The bump triggers regeneration so already-onboarded projects
// write the root file the always-on injection actually reads. See
// docs/issues/archive/2026-06-12-onboarding-writes-system-prompt-to-memory-not-root.md.
// Bumped 2026-09-03 (tool-surface collapse): the onboarding slice's CLAUDE.md-memories
// recipe called `edit_markdown`, folded into `edit_file` by Task 8, and Iron Law 5 named it
// too. An already-onboarded project caches the rendered prompt, so without this bump those
// projects keep handing agents a call naming a tool the server no longer registers.
// (`server_instructions` changes alone would NOT warrant a bump — that surface is injected
// fresh per session. See src/prompts/README.md.)
pub(crate) const ONBOARDING_VERSION: u32 = 30;

/// Returns true if the stored onboarding version is stale (needs refresh).
/// `None` means pre-versioning project — always stale.
/// Stored > compiled (downgrade) is treated as current to avoid churn.
pub(crate) fn onboarding_version_stale(stored: Option<u32>) -> bool {
    match stored {
        None => true,
        Some(v) => v < ONBOARDING_VERSION,
    }
}
/// Hash `.codescout/system-prompt.md` — the artifact `onboarding_version` certifies.
///
/// An absent file hashes as empty content (see `hash_file_or_empty`), so a project
/// whose prompt has not been written yet still yields a comparable baseline.
pub(crate) fn hash_system_prompt(root: &std::path::Path) -> String {
    crate::memory::hash::hash_file_or_empty(&root.join(".codescout").join("system-prompt.md"))
}

/// Returns true if the stored system prompt is behind the compiled templates.
///
/// A stale `onboarding_version` is necessary but **not sufficient**. Nothing in this
/// process writes `.codescout/system-prompt.md`: every write is a subagent following
/// the instructions this tool returns, and that step may never run. So the version
/// alone can only ever record that a refresh was *requested*.
///
/// `certified` is the prompt's hash as recorded at request time. A current hash that
/// differs from it is positive evidence the regeneration happened, and outranks the
/// version — which is what lets the request-time record stay honest instead of
/// certifying work in advance.
///
/// `certified == None` is **not** a difference. Without a recorded baseline there is
/// no evidence in either direction, and reading absence as change would report a
/// freshly cloned prompt as current: `project.toml` is gitignored while
/// `system-prompt.md` is tracked, so `None` beside a present file is precisely the
/// state a clone arrives in.
pub(crate) fn system_prompt_stale(
    stored_version: Option<u32>,
    certified: Option<&str>,
    current: &str,
) -> bool {
    if !onboarding_version_stale(stored_version) {
        return false;
    }
    match certified {
        Some(c) => c == current,
        None => true,
    }
}

/// Record the prompt's current content as the baseline a requested refresh must
/// supersede, **without** stamping the version.
///
/// The version deliberately stays put: it is the only signal that the prompt is
/// behind, and the regeneration this call is about has not happened yet. Writing it
/// here is the defect this replaced — following the instruction disarmed the detector
/// before the repair ran, and the stamp landed in a gitignored file while the artifact
/// it certified was tracked, so no reviewer and no `git status` could see the gap.
async fn record_refresh_request(ctx: &ToolContext) -> anyhow::Result<()> {
    let config_path = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            if config_path.exists() {
                let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                config.project.system_prompt_sha256 = Some(hash_system_prompt(&p.root));
                let toml_str = toml::to_string_pretty(&config)?;
                std::fs::write(&config_path, &toml_str)?;
            }
            Ok(config_path)
        })
        .await?;
    ctx.agent
        .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), &config_path)
        .await;
    Ok(())
}

/// Stamp a **witnessed** regeneration: the version, plus the content that earned it.
/// Returns true if a stamp was written.
///
/// This is what closes the witness, and omitting it would be a slower version of the
/// original defect rather than a smaller one. A witnessed-but-unstamped regeneration
/// reads correctly today and wrongly after the next `ONBOARDING_VERSION` bump: the old
/// baseline still differs from the current file, so the witness would keep firing on
/// evidence that predates the new templates. Re-recording `certified = current` is what
/// makes the next bump's comparison start level.
///
/// Called from both readers — the tool's own re-entry and the activation response — so
/// the stamp lands on whichever runs first rather than depending on a session calling
/// `onboarding()` again.
pub(crate) async fn stamp_witnessed_refresh(ctx: &ToolContext) -> anyhow::Result<bool> {
    let (config_path, stamped) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let config_path = p.root.join(".codescout").join("project.toml");
            if !config_path.exists() {
                return Ok((config_path, false));
            }
            let mut config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
            let current = hash_system_prompt(&p.root);
            let stored = config.project.onboarding_version;
            let witnessed = onboarding_version_stale(stored)
                && !system_prompt_stale(
                    stored,
                    config.project.system_prompt_sha256.as_deref(),
                    &current,
                );
            if witnessed {
                config.project.onboarding_version = Some(ONBOARDING_VERSION);
                config.project.system_prompt_sha256 = Some(current);
                let toml_str = toml::to_string_pretty(&config)?;
                std::fs::write(&config_path, &toml_str)?;
            }
            Ok((config_path, witnessed))
        })
        .await?;
    if stamped {
        ctx.agent
            .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), &config_path)
            .await;
    }
    Ok(stamped)
}

pub struct Onboarding;

/// Context gathered from well-known project files during onboarding.
#[derive(Debug, Default)]
pub(crate) struct GatheredContext {
    pub(crate) readme_path: Option<String>,
    pub(crate) build_file_name: Option<String>,
    pub(crate) claude_md_exists: bool,
    pub(crate) ci_files: Vec<String>,
    pub(crate) entry_points: Vec<String>,
    pub(crate) test_dirs: Vec<String>,
    /// Path to FEATURES.md if found (relative to project root)
    pub(crate) features_md: Option<String>,
    /// Discovered sub-projects (populated by discover_projects)
    pub(crate) projects: Vec<crate::workspace::DiscoveredProject>,
}

/// Read key project files up-front so the onboarding prompt can include them.
/// Detect well-known project files during onboarding.
///
/// File *contents* are intentionally not read here — inlining README/CLAUDE.md
/// into the onboarding response causes "⚠ Large MCP response" warnings and
/// duplicates CLAUDE.md that may already be in the agent's context. The agent
/// reads these files via `read_file` during Phase 1 exploration.
///
/// `projects` is the already-discovered project list from the workspace, passed in
/// to avoid a redundant `discover_projects` walk (the agent runs it at activation).
pub(crate) fn gather_project_context(
    root: &std::path::Path,
    projects: Vec<crate::workspace::DiscoveredProject>,
) -> GatheredContext {
    let mut ctx = GatheredContext::default();

    // README (try common names — record path but don't read content)
    for name in &["README.md", "README.rst", "README.txt", "README"] {
        if root.join(name).exists() {
            ctx.readme_path = Some(name.to_string());
            break;
        }
    }

    // CLAUDE.md
    ctx.claude_md_exists = root.join("CLAUDE.md").exists();

    // Build file (first match wins, ordered by popularity — name only)
    let build_files = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "build.gradle.kts",
        "build.gradle",
        "go.mod",
        "pom.xml",
        "Makefile",
        "CMakeLists.txt",
        "setup.py",
        "mix.exs",
        "Gemfile",
    ];
    for name in &build_files {
        if root.join(name).exists() {
            ctx.build_file_name = Some(name.to_string());
            break;
        }
    }

    // CI config files (just names, not contents)
    for dir in &[".github/workflows", ".gitlab", ".circleci"] {
        let ci_path = root.join(dir);
        if ci_path.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&ci_path) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".yml") || name.ends_with(".yaml") {
                        ctx.ci_files.push(format!("{}/{}", dir, name));
                    }
                }
            }
        }
    }
    ctx.ci_files.sort();

    // Entry points (check common locations)
    let entry_candidates = [
        "src/main.rs",
        "src/lib.rs",
        "src/main.py",
        "src/index.ts",
        "src/index.js",
        "src/app.ts",
        "src/app.py",
        "main.go",
        "cmd/main.go",
        "lib/main.dart",
        "index.js",
        "index.ts",
        "app.py",
        "manage.py",
    ];
    for candidate in &entry_candidates {
        if root.join(candidate).exists() {
            ctx.entry_points.push(candidate.to_string());
        }
    }

    // Test directories
    for candidate in &[
        "tests",
        "test",
        "spec",
        "src/test",
        "src/tests",
        "__tests__",
    ] {
        if root.join(candidate).is_dir() {
            ctx.test_dirs.push(candidate.to_string());
        }
    }

    // FEATURES.md — documents implemented capabilities
    for candidate in &["docs/FEATURES.md", "FEATURES.md", "docs/features.md"] {
        if root.join(candidate).exists() {
            ctx.features_md = Some(candidate.to_string());
            break;
        }
    }

    // Use the already-discovered project list passed by the caller to avoid
    // a redundant filesystem walk (discover_projects is run at activation time).
    ctx.projects = projects;

    ctx
}

/// Gather staleness state for protected memory topics.
/// Returns a JSON object keyed by topic name, suitable for inclusion
/// in the onboarding result.
fn gather_protected_memory_state(
    memory: &crate::memory::MemoryStore,
    memories_dir: &std::path::Path,
    project_root: &std::path::Path,
    protected: &[String],
) -> Value {
    use crate::memory::anchors::{anchor_path_for_topic, check_path_staleness, read_anchor_file};

    // Programmatic topics are always machine-generated — exclude from protection
    const PROGRAMMATIC: &[&str] = &["onboarding", "language-patterns"];

    let mut result = serde_json::Map::new();

    for topic in protected {
        if PROGRAMMATIC.contains(&topic.as_str()) {
            continue;
        }

        let content = match memory.read(topic) {
            Ok(Some(c)) => c,
            _ => {
                // Topic doesn't exist — signal to create fresh
                result.insert(topic.clone(), json!({ "exists": false }));
                continue;
            }
        };

        let anchor_path = anchor_path_for_topic(memories_dir, topic);
        let staleness = if anchor_path.exists() {
            match read_anchor_file(&anchor_path)
                .and_then(|af| check_path_staleness(project_root, &af))
            {
                Ok(report) => json!({
                    "stale_files": report.stale_files,
                    "untracked": false,
                }),
                Err(_) => json!({
                    "stale_files": [],
                    "untracked": true,
                }),
            }
        } else {
            json!({
                "stale_files": [],
                "untracked": true,
            })
        };

        result.insert(
            topic.clone(),
            json!({
                "exists": true,
                "content": content,
                "staleness": staleness,
            }),
        );
    }

    Value::Object(result)
}

#[async_trait::async_trait]
impl Tool for Onboarding {
    fn name(&self) -> &str {
        "onboarding"
    }

    /// Additive: writes only codescout-owned generated files, never user source. A second
    /// call sees `onboarding_version` stamped and does no further work.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::additive_closed()
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Initial project discovery: languages, key files (README, build config, \
         CLAUDE.md). Returns instructions for memories + system prompt draft, \
         OR status if already onboarded. Requires active project. force=true re-scans; \
         refresh_prompt=true rebuilds the system prompt from templates."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": {
                    "type": "boolean",
                    "description": "Force full re-scan even if already onboarded (default: false)"
                },
                "refresh_prompt": {
                    "type": "boolean",
                    "description": "Regenerate system prompt from current templates without re-exploring (default: false)"
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;
        let force = parse_bool_param(&input["force"]);
        let refresh_prompt = parse_bool_param(&input["refresh_prompt"]);

        if refresh_prompt && !force {
            return handle_refresh_prompt(ctx).await;
        }

        if !force {
            if let Some(response) = handle_already_onboarded(ctx).await? {
                return Ok(response);
            }
        }

        let mut result = perform_full_onboarding(root, ctx).await?;

        // `force` legitimately subsumes `refresh_prompt`: full onboarding re-explores
        // the project AND regenerates the system prompt (gated by
        // `onboarding_prompts_write_system_prompt_to_root_not_memory`), so the
        // combination does strictly more work rather than less. The defect was never
        // the precedence — it was accepting a parameter, honouring a different one,
        // and returning a well-formed payload that never mentioned the substitution.
        if refresh_prompt {
            result["refresh_prompt_subsumed"] = json!(true);
            result["refresh_prompt_note"] = json!(
                "refresh_prompt was subsumed by force. Full onboarding re-explores the \
                 project and regenerates the system prompt, a superset of what \
                 refresh_prompt does — so this response is the full-onboarding one. For \
                 the lightweight template-only rebuild, call \
                 onboarding(refresh_prompt=true) without force."
            );
        }
        Ok(result)
    }

    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        let val = self.call(input, ctx).await?;

        // If there's a subagent prompt, write it to a temp markdown file and return
        // compact instructions with heading navigation.
        if let Some(prompt) = val["subagent_prompt"].as_str() {
            let compact = format_onboarding(&val);

            let root = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await?;
            let tmp_dir = root.join(".codescout").join("tmp");
            std::fs::create_dir_all(&tmp_dir)?;
            let prompt_path = tmp_dir.join("onboarding-prompt.md");
            std::fs::write(&prompt_path, prompt)?;
            let rel_path = ".codescout/tmp/onboarding-prompt.md";
            let sections = build_heading_map(prompt);

            let subagent = ctx.is_subagent_capable();

            // Determine which instruction builder based on whether this is a
            // version refresh (has stored_version) or full onboarding.
            let instructions =
                if val.get("version_stale").is_some() || val.get("explicit_refresh").is_some() {
                    let stored = val["stored_version"].as_u64().map(|v| v as u32);
                    let current = val["current_version"].as_u64().unwrap_or(0) as u32;
                    build_buffered_refresh_instructions(rel_path, stored, current, subagent)
                } else {
                    build_buffered_onboarding_instructions(rel_path, subagent)
                };

            // For workspaces, also write per-project and synthesis prompt files.
            let workspace_fields = if val["workspace_mode"].as_bool().unwrap_or(false) {
                let projects_val = val["projects"].as_array();
                if let Some(projects) = projects_val {
                    let mut project_prompts = Vec::new();
                    let all_projects: Vec<(String, Vec<String>)> = projects
                        .iter()
                        .filter_map(|p| {
                            let id = p["id"].as_str()?.to_string();
                            let langs: Vec<String> = p["languages"]
                                .as_array()?
                                .iter()
                                .filter_map(|l| l.as_str().map(String::from))
                                .collect();
                            Some((id, langs))
                        })
                        .collect();

                    for p in projects {
                        let id = p["id"].as_str().unwrap_or("unknown");
                        let project = crate::workspace::DiscoveredProject {
                            id: id.to_string(),
                            relative_root: std::path::PathBuf::from(
                                p["root"].as_str().unwrap_or("."),
                            ),
                            languages: p["languages"]
                                .as_array()
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|l| l.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            manifest: p["manifest"].as_str().map(String::from),
                        };
                        let siblings: Vec<(String, Vec<String>)> = all_projects
                            .iter()
                            .filter(|(sid, _)| sid != id)
                            .cloned()
                            .collect();

                        let prompt_content = build_per_project_prompt(&project, &siblings);
                        let file_name = format!("onboarding-project-{}.md", id);
                        let file_path = tmp_dir.join(&file_name);
                        std::fs::write(&file_path, &prompt_content)?;

                        let rel = format!(".codescout/tmp/{}", file_name);
                        project_prompts.push((id.to_string(), rel));
                    }

                    // Write synthesis prompt
                    let synthesis_content = build_synthesis_prompt(&all_projects);
                    let synthesis_file = tmp_dir.join("onboarding-workspace-synthesis.md");
                    std::fs::write(&synthesis_file, &synthesis_content)?;
                    let synthesis_rel =
                        ".codescout/tmp/onboarding-workspace-synthesis.md".to_string();

                    // Build workspace-specific instructions (overrides the single-project ones)
                    let ws_instructions = build_workspace_instructions(
                        rel_path,
                        &project_prompts,
                        &synthesis_rel,
                        subagent,
                    );

                    Some((project_prompts, synthesis_rel, ws_instructions))
                } else {
                    None
                }
            } else {
                None
            };

            let response = if let Some((project_prompts, synthesis_path, ws_instructions)) =
                workspace_fields
            {
                let pp_json: Vec<Value> = project_prompts
                    .iter()
                    .map(|(id, path)| serde_json::json!({ "id": id, "path": path }))
                    .collect();

                serde_json::json!({
                    "prompt_path": rel_path,
                    "summary": compact,
                    "sections": sections,
                    "project_prompts": pp_json,
                    "synthesis_prompt_path": synthesis_path,
                    "instructions": ws_instructions,
                })
            } else {
                serde_json::json!({
                    "prompt_path": rel_path,
                    "summary": compact,
                    "sections": sections,
                    "instructions": instructions,
                })
            };

            return Ok(vec![rmcp::model::Content::text(
                serde_json::to_string_pretty(&response)
                    .unwrap_or_else(|_| format!("{{\"prompt_path\":\"{rel_path}\"}}")),
            )]);
        }

        // Single-block fast path: already-onboarded status.
        if val["onboarded"].as_bool().unwrap_or(false) {
            let msg = val["message"].as_str().unwrap_or("Already onboarded.");
            return Ok(vec![rmcp::model::Content::text(msg.to_string())]);
        }

        // Fallback
        let compact = format_onboarding(&val);
        Ok(vec![rmcp::model::Content::text(compact)])
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_onboarding(result))
    }
}

async fn handle_refresh_prompt(ctx: &ToolContext) -> anyhow::Result<Value> {
    let status = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let has_config = p.root.join(".codescout").join("project.toml").exists();
            let memories = p.memory.list()?;
            let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
            Ok((has_config, has_onboarding_memory, memories))
        })
        .await?;
    let (has_config, has_onboarding_memory, memories) = status;
    if !has_config || !has_onboarding_memory {
        return Err(super::RecoverableError::with_hint(
            "refresh_prompt requires a fully onboarded project",
            "Run onboarding() without any flags first to perform the initial onboarding.",
        )
        .into());
    }

    let (stored_version, config_languages) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok((
                p.config.project.onboarding_version,
                p.config.project.languages.clone(),
            ))
        })
        .await?;

    // Record the baseline the regeneration must supersede. The version is NOT
    // stamped here: the work this call describes has not happened yet, and stamping
    // it would consume the only signal that the prompt is behind.
    record_refresh_request(ctx).await?;

    let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

    Ok(json!({
        "onboarded": true,
        // Reports the stored version's own state, not the outcome of the
        // regeneration this call merely requests.
        "version_stale": onboarding_version_stale(stored_version),
        "explicit_refresh": true,
        "stored_version": stored_version,
        "current_version": ONBOARDING_VERSION,
        "languages": config_languages,
        "config_created": false,
        "subagent_prompt": subagent_prompt,
    }))
}

/// Returns `Some(response)` if the project is already onboarded (caller should return it),
/// or `None` if onboarding hasn't been done yet (caller should proceed with full scan).
async fn handle_already_onboarded(ctx: &ToolContext) -> anyhow::Result<Option<Value>> {
    let status = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let has_config = p.root.join(".codescout").join("project.toml").exists();
            let memories = p.memory.list()?;
            let has_onboarding_memory = memories.iter().any(|m| m == "onboarding");
            let private_memories = p.private_memory.list()?;
            Ok((
                has_config,
                has_onboarding_memory,
                memories,
                private_memories,
            ))
        })
        .await?;
    let (has_config, has_onboarding_memory, memories, private_memories) = status;
    if !has_config || !has_onboarding_memory {
        return Ok(None);
    }

    // --- Version check: refresh system prompt if stale ---
    // The stored version records a *request*, not a completed regeneration — nothing
    // in this process writes `.codescout/system-prompt.md`. So witness the artifact
    // before trusting the version: a prompt whose content has moved since the request
    // was recorded has demonstrably been regenerated, and what is owed is the stamp,
    // not another refresh. Stamping first means the read below sees the settled value.
    if let Err(e) = stamp_witnessed_refresh(ctx).await {
        tracing::warn!("could not stamp a witnessed system-prompt refresh: {e}");
    }

    let (stored_version, config_languages) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok((
                p.config.project.onboarding_version,
                p.config.project.languages.clone(),
            ))
        })
        .await?;

    // Log downgrade (no action)
    if let Some(v) = stored_version {
        if v > ONBOARDING_VERSION {
            tracing::warn!(
                "stored onboarding version ({}) is newer than compiled ({}) — skipping refresh",
                v,
                ONBOARDING_VERSION
            );
        }
    }

    if onboarding_version_stale(stored_version) {
        tracing::info!(
            "onboarding version stale: stored={:?} current={}",
            stored_version,
            ONBOARDING_VERSION
        );

        // Record the baseline this refresh must supersede. The version is NOT
        // stamped here — it is the only signal that the prompt is behind, and the
        // regeneration is still owed by a subagent that may never run. It used to
        // be stamped ("optimistic version write"), which suppressed the re-trigger
        // this comment claimed to want and certified work that had not happened.
        record_refresh_request(ctx).await?;

        let subagent_prompt = build_prompt_refresh_subagent_prompt(&memories);

        return Ok(Some(json!({
            "onboarded": true,
            "version_stale": true,
            "stored_version": stored_version,
            "current_version": ONBOARDING_VERSION,
            "languages": config_languages,
            "config_created": false,
            "subagent_prompt": subagent_prompt,
        })));
    }

    let per_project_memories = ctx.agent.workspace_project_memories().await;

    let mut message = format!(
        "Onboarding already performed. Available shared memories: {}. \
         Use `memory(action=\"read\", topic=...)` to read relevant ones as needed for your current task. \
         Do not read all memories at once — only read those relevant to what you're working on. \
         Use `memory(action=\"recall\", query=\"...\")` to search memories by meaning when the topic name isn't known. \
         To rebuild this project's stored system prompt from the current templates (e.g. after prompt-source or memory edits), call `onboarding(refresh_prompt=true)`.",
        memories.join(", ")
    );
    if !private_memories.is_empty() {
        message.push_str(&format!(
            " Private memories: {}. Read with `memory(action=\"read\", topic=..., private=true)`.",
            private_memories.join(", ")
        ));
    }
    if !per_project_memories.is_empty() {
        message.push_str(" Per-project memories (use `project_id=\"<id>\"` parameter):");
        for (id, topics) in &per_project_memories {
            message.push_str(&format!(" {}: {};", id, topics.join(", ")));
        }
    }
    let mut response = json!({
        "onboarded": true,
        "has_config": true,
        "has_onboarding_memory": true,
        "memories": memories,
        "message": message,
    });
    if !private_memories.is_empty() {
        response["private_memories"] = json!(private_memories);
    }
    if !per_project_memories.is_empty() {
        let map: serde_json::Map<String, serde_json::Value> = per_project_memories
            .into_iter()
            .map(|(id, topics)| (id, json!(topics)))
            .collect();
        response["project_memories"] = serde_json::Value::Object(map);
    }
    Ok(Some(response))
}

/// Walk the project tree and collect the set of detected languages.
fn detect_languages(root: &std::path::Path) -> std::collections::BTreeSet<String> {
    let mut languages = std::collections::BTreeSet::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .build();
    for entry in walker.flatten() {
        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
            if let Some(lang) = crate::ast::detect_language(entry.path()) {
                languages.insert(lang.to_string());
            }
        }
    }
    languages
}

/// List the project's top-level entries (directories suffixed with `/`), sorted.
fn list_top_level_entries(root: &std::path::Path) -> Vec<String> {
    let mut top_level = vec![];
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let suffix = if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                "/"
            } else {
                ""
            };
            top_level.push(format!("{}{}", name, suffix));
        }
    }
    top_level.sort();
    top_level
}

/// Build the key-files manifest for the onboarding prompt (paths only, no content).
fn build_key_files(gathered: &GatheredContext) -> Vec<String> {
    let mut key_files: Vec<String> = Vec::new();
    if let Some(ref p) = gathered.readme_path {
        key_files.push(p.clone());
    }
    if gathered.claude_md_exists {
        key_files.push("CLAUDE.md".to_string());
    }
    if let Some(ref p) = gathered.build_file_name {
        key_files.push(p.clone());
    }
    key_files
}

/// Write `.codescout/workspace.toml` for multi-project repos when it does not yet exist.
fn write_workspace_config_if_needed(
    root: &std::path::Path,
    gathered: &GatheredContext,
) -> anyhow::Result<()> {
    let workspace_config_path = crate::config::workspace::workspace_config_path(root);
    if gathered.projects.len() > 1 && !workspace_config_path.exists() {
        let ws_config = crate::config::workspace::WorkspaceConfig {
            workspace: crate::config::workspace::WorkspaceSection {
                name: root
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unnamed")
                    .to_string(),
                discovery_max_depth: 3,
            },
            resources: Default::default(),
            exclude_projects: vec![],
            projects: gathered
                .projects
                .iter()
                .map(|p| {
                    let project_abs = root.join(&p.relative_root);
                    let depends_on =
                        crate::workspace::infer_depends_on(&project_abs, root, &gathered.projects);
                    crate::config::workspace::ProjectEntry {
                        id: p.id.clone(),
                        // Persisted into workspace.toml and read back as a path —
                        // must not carry native separators (Windows).
                        root: crate::util::fs::to_forward_slash(&p.relative_root),
                        languages: p.languages.clone(),
                        depends_on,
                    }
                })
                .collect(),
        };
        let toml_str = toml::to_string_pretty(&ws_config)?;
        std::fs::write(&workspace_config_path, &toml_str)?;
    }
    Ok(())
}

/// Probe the semantic index (Qdrant) for readiness. Best-effort: returns a
/// `{ ready, files, chunks }` envelope, with `ready: false` when the retrieval
/// stack is offline.
async fn probe_index_status(ctx: &ToolContext) -> Value {
    let project_id = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok(p.project_id().to_string())
        })
        .await
        .unwrap_or_else(|_| String::new());
    if project_id.is_empty() {
        return json!({ "ready": false, "files": 0, "chunks": 0 });
    }
    let root = ctx
        .agent
        .project_root_for(ctx.workspace_override.as_deref())
        .await;
    match crate::retrieval::client::RetrievalClient::from_env(root.as_deref()).await {
        Ok(client) => {
            let coll = client.config.collection("code_chunks");
            match client.project_index_stats(&coll, &project_id).await {
                Ok((chunks, files)) => json!({
                    "ready": chunks > 0,
                    "files": files,
                    "chunks": chunks,
                }),
                Err(_) => json!({ "ready": false, "files": 0, "chunks": 0 }),
            }
        }
        Err(_) => json!({ "ready": false, "files": 0, "chunks": 0 }),
    }
}

/// Write the `onboarding` and `language-patterns` programmatic memories for the
/// root project, and — in workspace mode — for each discovered sub-project.
async fn write_onboarding_memories(
    ctx: &ToolContext,
    root: &std::path::Path,
    lang_list: &[String],
    gathered: &GatheredContext,
) -> anyhow::Result<()> {
    ctx.agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let summary = format!(
                "Languages: {}\nHas README: {}\nHas CLAUDE.md: {}\nBuild file: {}\nEntry points: {}\nTest dirs: {}",
                lang_list.join(", "),
                gathered.readme_path.is_some(),
                gathered.claude_md_exists,
                gathered.build_file_name.as_deref().unwrap_or("none"),
                if gathered.entry_points.is_empty() {
                    "none".to_string()
                } else {
                    gathered.entry_points.join(", ")
                },
                if gathered.test_dirs.is_empty() {
                    "none".to_string()
                } else {
                    gathered.test_dirs.join(", ")
                },
            );
            p.memory.write("onboarding", &summary)?;
            if let Some(patterns) = build_language_patterns_memory(lang_list) {
                p.memory.write("language-patterns", &patterns)?;
            }
            Ok(())
        })
        .await?;

    if gathered.projects.len() > 1 {
        for project in &gathered.projects {
            let mem_dir = if project.relative_root == std::path::Path::new(".") {
                root.join(".codescout").join("memories")
            } else {
                root.join(".codescout")
                    .join("projects")
                    .join(&project.id)
                    .join("memories")
            };
            if let Ok(store) = crate::memory::MemoryStore::from_dir(mem_dir) {
                let proj_summary = format!(
                    "Languages: {}\nRoot: {}\nManifest: {}",
                    project.languages.join(", "),
                    crate::util::fs::to_forward_slash(&project.relative_root),
                    project.manifest.as_deref().unwrap_or("none"),
                );
                if let Err(e) = store.write("onboarding", &proj_summary) {
                    tracing::warn!(
                        "onboarding memory write failed for sub-project '{}': {e}",
                        project.id
                    );
                }
                if let Some(patterns) = build_language_patterns_memory(&project.languages) {
                    if let Err(e) = store.write("language-patterns", &patterns) {
                        tracing::warn!(
                            "language-patterns memory write failed for sub-project '{}': {e}",
                            project.id
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// Gather per-sub-project protected-memory state for workspace mode.
/// Returns `(workspace_mode, per_project_protected)`.
async fn gather_per_project_protected(
    ctx: &ToolContext,
    root: &std::path::Path,
    gathered: &GatheredContext,
) -> (bool, Option<Value>) {
    if gathered.projects.len() <= 1 {
        return (false, None);
    }
    let protected = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok(p.config.memory.protected.clone())
        })
        .await
        .unwrap_or_default();
    let mut map = serde_json::Map::new();
    for project in &gathered.projects {
        let mem_dir = if project.relative_root == std::path::Path::new(".") {
            root.join(".codescout").join("memories")
        } else {
            root.join(".codescout")
                .join("projects")
                .join(&project.id)
                .join("memories")
        };
        let project_root = root.join(&project.relative_root);
        // Read-only: `gather_protected_memory_state` only calls `read` and
        // `exists`. `from_dir`'s `create_dir_all` made this loop materialise
        // `projects/<id>/memories` for every project in the workspace as a
        // side effect of reporting staleness.
        let store = crate::memory::MemoryStore::from_dir_readonly(mem_dir.clone());
        let state = gather_protected_memory_state(&store, &mem_dir, &project_root, &protected);
        map.insert(project.id.clone(), state);
    }
    (true, Some(Value::Object(map)))
}

async fn perform_full_onboarding(
    root: std::path::PathBuf,
    ctx: &ToolContext,
) -> anyhow::Result<Value> {
    // Hardware detection runs after the file walk (Rust futures are lazy — this
    // just creates the future; it starts executing only when .await'd below).
    let hw_future = detect_hardware_context();

    let languages = detect_languages(&root);
    let top_level = list_top_level_entries(&root);

    // Resolve hardware detection and derive model options
    let hw = hw_future.await;
    let model_options = model_options_for_hardware(&hw);
    let recommended_model = model_options
        .first()
        .expect("model_options_for_hardware guarantees ≥1 entry")
        .id
        .clone();

    // Create .codescout/project.toml if it doesn't exist
    let config_dir = root.join(".codescout");
    let config_path = config_dir.join("project.toml");
    let created_config = if !config_path.exists() {
        std::fs::create_dir_all(&config_dir)?;
        let name = root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();
        let langs: Vec<String> = languages.iter().cloned().collect();
        let config = crate::config::project::ProjectConfig {
            project: crate::config::project::ProjectSection {
                name,
                languages: langs,
                encoding: "utf-8".into(),
                system_prompt: None,
                tool_timeout_secs: 60,
                // Neither the version nor the baseline is stamped at creation: the
                // prompt this config would certify has not been written yet. The tail
                // of this function records the baseline once the file exists, which is
                // what lets a fresh project's first re-entry witness the regeneration
                // instead of finding itself already certified.
                onboarding_version: None,
                system_prompt_sha256: None,
            },
            embeddings: crate::config::project::EmbeddingsSection {
                model: recommended_model,
                ..Default::default()
            },
            ignored_paths: Default::default(),
            security: Default::default(),
            memory: Default::default(),
            libraries: Default::default(),
            lsp: Default::default(),
        };
        let toml_str = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, &toml_str)?;
        // Reload in-memory config so the version is visible within this session
        ctx.agent
            .reload_config_if_project_toml_for(ctx.workspace_override.as_deref(), &config_path)
            .await;
        true
    } else {
        false
    };

    // Gather rich context from well-known project files.
    // Pass the already-discovered project list from the workspace to avoid a
    // redundant discover_projects walk (the agent runs it at activation time).
    let discovered = ctx.agent.discovered_projects().await;
    let gathered = gather_project_context(&root, discovered);

    // Create workspace.toml for multi-project repos
    write_workspace_config_if_needed(&root, &gathered)?;

    // Probe semantic index status (best-effort Qdrant scroll).
    let index_status = probe_index_status(ctx).await;

    // Store onboarding result in memory (root + each sub-project in workspace mode)
    let lang_list: Vec<String> = languages.iter().cloned().collect();
    write_onboarding_memories(ctx, &root, &lang_list, &gathered).await?;

    // Gather protected memory state for the LLM merge flow
    let protected_memories = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let memories_dir = p.root.join(".codescout").join("memories");
            let protected = &p.config.memory.protected;
            Ok(gather_protected_memory_state(
                &p.memory,
                &memories_dir,
                &p.root,
                protected,
            ))
        })
        .await?;

    // Build the key-files manifest for the prompt (paths only, no content)
    let key_files = build_key_files(&gathered);

    // Build the onboarding instruction prompt
    let is_workspace = gathered.projects.len() > 1;
    let prompt = crate::prompts::build_onboarding_prompt(&crate::prompts::OnboardingContext {
        languages: &lang_list,
        top_level: &top_level,
        key_files: &key_files,
        ci_files: &gathered.ci_files,
        entry_points: &gathered.entry_points,
        test_dirs: &gathered.test_dirs,
        index_ready: index_status["ready"].as_bool().unwrap_or(false),
        index_files: index_status["files"].as_u64().unwrap_or(0) as usize,
        index_chunks: index_status["chunks"].as_u64().unwrap_or(0) as usize,
        projects: &gathered.projects,
        is_workspace,
    });

    // Build the system prompt draft scaffold
    let libraries: Vec<crate::library::registry::LibraryEntry> = ctx
        .agent
        .library_registry()
        .await
        .map(|r| r.all().to_vec())
        .unwrap_or_default();
    let mut system_prompt_draft = build_system_prompt_draft(
        &lang_list,
        &gathered.entry_points,
        Some(&root),
        Some(&gathered.projects),
        &libraries,
    );
    crate::prompts::builders::append_preferences_section(&ctx.agent, &mut system_prompt_draft)
        .await;

    let discovered_projects: Vec<serde_json::Value> = gathered
        .projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "root": crate::util::fs::to_forward_slash(&p.relative_root),
                "languages": p.languages,
                "manifest": p.manifest,
            })
        })
        .collect();

    let features_suggestion = gathered.features_md.is_none().then_some(
        "No FEATURES.md found. Consider creating docs/FEATURES.md to document \
         implemented capabilities — helps agents understand what's already built \
         and avoid re-suggesting existing features.",
    );

    // Per-project protected memory state for workspace mode.
    let (workspace_mode, per_project_protected) =
        gather_per_project_protected(ctx, &root, &gathered).await;

    // Build the subagent prompt by concatenating preamble + onboarding prompt +
    // system prompt draft + gathered data + epilogue
    let subagent_prompt = {
        let mut sp = build_subagent_preamble();
        sp.push_str(&prompt);
        if !system_prompt_draft.is_empty() {
            sp.push_str("\n\n## System Prompt Draft\n\n");
            sp.push_str(&system_prompt_draft);
        }
        if let Some(suggestion) = features_suggestion {
            sp.push_str(&format!("\n\n> {suggestion}"));
        }
        // Append gathered data that the subagent needs
        sp.push_str("\n\n## Gathered Data\n\n");
        sp.push_str(&format!(
            "**Hardware:** {}\n\n",
            serde_json::to_string_pretty(&hw).unwrap_or_default()
        ));
        sp.push_str(&format!(
            "**Model options:** {}\n\n",
            serde_json::to_string_pretty(&model_options).unwrap_or_default()
        ));
        if !protected_memories.is_null() {
            sp.push_str(&format!(
                "**Protected memories:** {}\n\n",
                serde_json::to_string_pretty(&protected_memories).unwrap_or_default()
            ));
        }
        if workspace_mode {
            if let Some(ref ppm) = per_project_protected {
                if !ppm.is_null() {
                    sp.push_str(&format!(
                        "**Per-project protected memories:** {}\n\n",
                        serde_json::to_string_pretty(ppm).unwrap_or_default()
                    ));
                }
            }
        }
        sp.push_str(&build_subagent_epilogue());
        sp
    };

    // Record the baseline this onboarding's regeneration must supersede — the third
    // site that used to stamp the version optimistically. Full onboarding returns a
    // `subagent_prompt` too, so the prompt file is no more written by the time we
    // reach here than on the lightweight refresh path.
    record_refresh_request(ctx).await?;

    Ok(json!({
        "languages": lang_list,
        "top_level": top_level,
        "config_created": created_config,
        "has_readme": gathered.readme_path.is_some(),
        "has_claude_md": gathered.claude_md_exists,
        "build_file": gathered.build_file_name,
        "entry_points": gathered.entry_points,
        "test_dirs": gathered.test_dirs,
        "ci_files": gathered.ci_files,
        "features_md": gathered.features_md,
        "index_status": index_status,
        "workspace_mode": workspace_mode,
        "projects": discovered_projects,
        "subagent_prompt": subagent_prompt,
    }))
}

fn format_onboarding(result: &Value) -> String {
    let langs = result["languages"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "?".to_string());
    let created = result["config_created"].as_bool().unwrap_or(false);
    let config_note = if created { " · config created" } else { "" };
    let workspace_note = if result["workspace_mode"].as_bool().unwrap_or(false) {
        let count = result["projects"].as_array().map(|a| a.len()).unwrap_or(0);
        format!(" · workspace ({count} projects)")
    } else {
        String::new()
    };
    // Surfaced on the compact line, not only in the JSON: the substitution is only
    // reported if it reaches the surface the caller actually reads.
    let subsumed_note = if result["refresh_prompt_subsumed"].as_bool().unwrap_or(false) {
        " · refresh_prompt subsumed by force (full onboarding regenerates the prompt too)"
    } else {
        ""
    };
    format!("[{langs}]{config_note}{workspace_note}{subsumed_note}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn onboarding_version_stale_returns_true_for_none() {
        assert!(
            onboarding_version_stale(None),
            "None (pre-versioning) should be treated as stale"
        );
    }

    #[test]
    fn onboarding_version_stale_returns_true_for_older_version() {
        assert!(
            onboarding_version_stale(Some(ONBOARDING_VERSION - 1)),
            "stored < current should be treated as stale"
        );
    }

    #[test]
    fn onboarding_version_stale_returns_false_for_downgrade() {
        assert!(
            !onboarding_version_stale(Some(ONBOARDING_VERSION + 1)),
            "downgrade (stored > current) should not be treated as stale"
        );
    }
    #[test]
    fn system_prompt_stale_is_true_while_the_prompt_has_not_moved_since_the_request() {
        // Version behind, and the file is byte-identical to the baseline recorded
        // when the refresh was requested: the work was asked for and has not
        // happened. This is the state the old eager stamp reported as current.
        assert!(
            system_prompt_stale(Some(ONBOARDING_VERSION - 1), Some("abc"), "abc"),
            "an unmoved prompt with a behind version is stale"
        );
    }

    #[test]
    fn system_prompt_stale_is_false_once_the_prompt_content_has_moved() {
        // The only positive evidence available that a subagent did the work: nothing
        // in-process writes the file, so its content changing is the whole signal.
        assert!(
            !system_prompt_stale(Some(ONBOARDING_VERSION - 1), Some("abc"), "def"),
            "content moving since the recorded baseline witnesses the regeneration"
        );
    }

    #[test]
    fn system_prompt_stale_treats_an_absent_baseline_as_no_evidence_not_as_change() {
        // A fresh clone arrives in exactly this state: `project.toml` is gitignored so
        // nothing is stored, while `system-prompt.md` is tracked and present. Reading
        // the missing baseline as a difference would report another machine's stale
        // prompt as current — the original defect, reintroduced at clone time.
        assert!(
            system_prompt_stale(None, None, "def"),
            "no stored version and no baseline is stale, not witnessed"
        );
        assert!(
            system_prompt_stale(Some(ONBOARDING_VERSION - 1), None, "def"),
            "a behind version with no baseline is stale, not witnessed"
        );
    }

    #[test]
    fn system_prompt_stale_is_false_when_the_version_is_current_whatever_the_hashes_say() {
        // Once stamped, the version is authoritative. The witness exists only to
        // rescue a version that is behind; it must never invalidate a current one, or
        // an ordinary hand-edit to the prompt would read as a regression.
        assert!(!system_prompt_stale(
            Some(ONBOARDING_VERSION),
            Some("abc"),
            "abc"
        ));
        assert!(!system_prompt_stale(
            Some(ONBOARDING_VERSION),
            Some("abc"),
            "def"
        ));
        assert!(
            !system_prompt_stale(Some(ONBOARDING_VERSION + 1), None, "def"),
            "a downgrade stays current, matching onboarding_version_stale"
        );
    }

    #[test]
    fn hash_system_prompt_treats_an_absent_prompt_as_empty_content() {
        // Load-bearing: a fresh project records its baseline at the tail of full
        // onboarding, BEFORE any subagent has written the prompt. If this errored or
        // returned a distinct sentinel for absence, that project would have no
        // comparable baseline and its first regeneration could never be witnessed.
        let dir = tempfile::tempdir().unwrap();
        let absent = hash_system_prompt(dir.path());

        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(dir.path().join(".codescout").join("system-prompt.md"), b"").unwrap();
        assert_eq!(
            absent,
            hash_system_prompt(dir.path()),
            "absent and empty must hash alike — the baseline has to exist before the file does"
        );

        std::fs::write(
            dir.path().join(".codescout").join("system-prompt.md"),
            b"# prompt",
        )
        .unwrap();
        assert_ne!(
            absent,
            hash_system_prompt(dir.path()),
            "writing content must move the hash, or no regeneration is ever witnessed"
        );
    }
}
