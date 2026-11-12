//! Onboarding tool — gathers project facts (languages, entry points, hardware,
//! memory state) and produces a system prompt draft plus a subagent prompt
//! for exploring the project.

use crate::hardware::{detect_hardware_context, model_options_for_hardware};
use crate::librarian::identity::repo_name::{self, NameError, RepoName, REPO_FILE_REL};
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
// Bumped 2026-09-05 (call-form gate): the onboarding slice's artifact-tracking step read
// `librarian_context(topic)` — a tool retired by the 2026-05-02 librarian-tools-collapse, so for
// four months every onboarded project cached a prompt instructing its agents to invoke a name the
// server does not register. It survived because `prompt_surfaces_reference_only_real_tools` needed
// the closing backtick to follow the identifier immediately and so could not see a CALL; a
// call-form pass now closes that. Same reasoning as the 2026-09-03 bump: already-onboarded
// projects cache the rendered prompt, so the fix does not reach them without this.
// docs/issues/2026-09-05-the-prompt-surface-gate-misses-a-backticked-tool-name-in-call-form.md
// Bumped 2026-11-11 (index banner): Phase 1 of the onboarding slice told the agent to announce
// "Semantic index is ready" whenever stored chunks exist. Stored chunks do not show that the
// embedder can answer a query, so the slice now says "built" and names the `grep` fallback.
// Already-onboarded projects cache the rendered prompt, so they need this bump to get it.
// docs/issues/2026-11-10-semantic-ready-banner-unreachable-embedder.md
pub(crate) const ONBOARDING_VERSION: u32 = 34;

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

/// Change keys in the `[project]` table of an existing `project.toml` and write nothing from the
/// merged config.
///
/// The two stamping writers used to load the MERGED config (global config under the project's
/// file), set a field and serialise the whole struct back. That wrote every merged value into
/// the project's file: a stricter global `[security]` was copied there as the project's own, and
/// from then on shadowed any later change to the global config. This patches the project's RAW
/// table instead, so a value the user did not write is not added.
///
/// **It does not preserve the file's formatting.** The file is parsed and re-serialised with the
/// `toml` crate (not `toml_edit`, which is not a dependency): comments are dropped, tables are
/// re-sorted and inline tables are rewritten. What survives is the data, not the layout.
fn patch_project_table(
    config_path: &std::path::Path,
    set: impl FnOnce(&mut toml::Table),
) -> anyhow::Result<()> {
    // A bounded regular-file read, like the loader that ran first: a path swapped in since then
    // (or a caller that did not load) must not be a FIFO that holds this read, and a refusal
    // returns before the write below ever opens the path.
    let text = crate::util::fs::read_regular_config_text(config_path).map_err(|e| match e {
        // The error `read_to_string` gave, unchanged.
        crate::util::fs::BoundedReadError::Io(io) => anyhow::Error::from(io),
        refused => anyhow::anyhow!("{}: {refused}", config_path.display()),
    })?;
    let mut doc: toml::Table = toml::from_str(&text).map_err(|e| {
        anyhow::anyhow!(
            "parsing {}: {}",
            config_path.display(),
            crate::config::project::describe_toml_error(Some(&text), &e)
        )
    })?;
    let project = doc
        .entry("project")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("[project] in project.toml is not a table"))?;
    set(project);
    std::fs::write(config_path, toml::to_string_pretty(&doc)?)?;
    Ok(())
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
                // Still loaded, so a file that does not load is refused here as before.
                crate::config::project::ProjectConfig::load_or_default(&p.root)?;
                let baseline = hash_system_prompt(&p.root);
                patch_project_table(&config_path, |project| {
                    project.insert("system_prompt_sha256".into(), toml::Value::String(baseline));
                })?;
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
            let config = crate::config::project::ProjectConfig::load_or_default(&p.root)?;
            let current = hash_system_prompt(&p.root);
            let stored = config.project.onboarding_version;
            let witnessed = onboarding_version_stale(stored)
                && !system_prompt_stale(
                    stored,
                    config.project.system_prompt_sha256.as_deref(),
                    &current,
                );
            if witnessed {
                patch_project_table(&config_path, |project| {
                    project.insert(
                        "onboarding_version".into(),
                        toml::Value::Integer(i64::from(ONBOARDING_VERSION)),
                    );
                    project.insert("system_prompt_sha256".into(), toml::Value::String(current));
                })?;
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
            Ok(None) => {
                // Topic doesn't exist — signal to create fresh
                result.insert(topic.clone(), json!({ "exists": false }));
                continue;
            }
            Err(e) => {
                // A topic that is there but cannot be read (a link, a directory, no
                // permission) is NOT the create-fresh case: the protocol would then
                // write over what the user keeps there. No `exists` key, so neither
                // `exists == false` nor `exists == true` matches; `error` says why.
                result.insert(
                    topic.clone(),
                    json!({
                        "unreadable": true,
                        "error": e.to_string(),
                    }),
                );
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
         OR status if already onboarded. Requires active project."
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
                },
                "repo_name": {
                    "type": "string",
                    "description": "Confirm the repo name (lowercase letters, digits, hyphens). Writes .codescout/repo.toml at the git root."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;

        // A name the caller confirmed is recorded and answered on its own: the call is the
        // confirmation the earlier response asked for, not a request to onboard again.
        // The name file belongs to the repo, so it lives at the repo root, found once per call.
        let repo_root = repo_root_of(&root);
        if let Some(raw) = input["repo_name"].as_str() {
            return set_repo_name(&repo_root, raw);
        }

        let mut result = onboarding_branch(&input, ctx, root.clone()).await?;
        result["repo_name"] = repo_name_block(ctx, &repo_root).await;
        Ok(result)
    }

    /// `onboarding` builds its own response blocks, so it never reaches the dispatcher's
    /// notice injection: the failed-reload notice is attached here, on the success blocks and
    /// on an error, like on every other response.
    ///
    /// It reaches the engines' post phase (`engines::coordinator::run_post`) on its success
    /// path, below. The call sits in this body, not in a helper, because
    /// `every_call_content_override_reaches_run_post_or_is_declared` looks for it here (CIW-11).
    ///
    /// The emission's `_guide_hint` is dropped: it is a field written into a primary JSON
    /// block, and `onboarding` builds its blocks as text. The blocks the engines append carry
    /// the guide content itself. `overflowing` is `false` because `onboarding` answers with a
    /// short instruction block and a path, never an oversized envelope, so the
    /// progressive-disclosure hint would not apply.
    async fn call_content(
        &self,
        input: Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<Vec<rmcp::model::Content>> {
        // Captured BEFORE `self.call` consumes `input`, as in the trait's default.
        let selector = self.selector_key(&input);
        let notice = super::core::types::config_load_notice(ctx).await;
        let result: anyhow::Result<(Vec<rmcp::model::Content>, Value)> = async {
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

                let subagent = ctx.client_can_spawn_subagents();

                // Determine which instruction builder based on whether this is a
                // version refresh (has stored_version) or full onboarding.
                let instructions = if val.get("version_stale").is_some()
                    || val.get("explicit_refresh").is_some()
                {
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

                let mut response = if let Some((project_prompts, synthesis_path, ws_instructions)) =
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

                if let Some(block) = val.get("repo_name") {
                    response["repo_name"] = block.clone();
                }

                return Ok((
                    vec![rmcp::model::Content::text(
                        serde_json::to_string_pretty(&response)
                            .unwrap_or_else(|_| format!("{{\"prompt_path\":\"{rel_path}\"}}")),
                    )],
                    val,
                ));
            }

            // Single-block fast path: already-onboarded status.
            if val["onboarded"].as_bool().unwrap_or(false) {
                let msg = val["message"].as_str().unwrap_or("Already onboarded.");
                return Ok((
                    vec![rmcp::model::Content::text(with_repo_name(msg, &val))],
                    val,
                ));
            }

            // `onboarding(repo_name=...)` answers with the repo-name block alone.
            if val.as_object().is_some_and(|o| o.len() == 1) && val.get("repo_name").is_some() {
                let text = serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string());
                return Ok((vec![rmcp::model::Content::text(text)], val));
            }

            // Fallback
            let compact = with_repo_name(&format_onboarding(&val), &val);
            Ok((vec![rmcp::model::Content::text(compact)], val))
        }
        .await;
        match result {
            Ok((mut out, val)) => {
                if let Some(n) = &notice {
                    super::core::types::attach_workspace_notice_to_content(&mut out, n);
                }
                let mut emitted = ctx.guide_hints_emitted.lock();
                let post = crate::engines::coordinator::PostCtx {
                    selector: selector.as_deref(),
                    value: &val,
                    content_topic: self.relevant_guide_topic(&val),
                    overflowing: false,
                };
                let crate::engines::coordinator::Emission {
                    blocks, deliveries, ..
                } = crate::engines::coordinator::run_post(&post, &mut emitted);
                crate::usage::deliveries::record(deliveries);
                out.extend(blocks);
                Ok(out)
            }
            Err(e) => Err(super::core::types::attach_workspace_notice_to_error(
                e, notice,
            )),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_onboarding(result))
    }
}

/// The `force` / `refresh_prompt` / already-onboarded branches of `onboarding`.
async fn onboarding_branch(
    input: &Value,
    ctx: &ToolContext,
    root: std::path::PathBuf,
) -> anyhow::Result<Value> {
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

/// How long `onboarding` waits for the user to name the repo before it returns the suggestion
/// for the agent to ask in chat. The call holds the project's write guard and runs inside the
/// server's tool timeout, so the wait stays well under it.
const ONBOARDING_ELICIT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// What the user is asked for when the repo has no name yet.
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct RepoNameInput {
    /// Repo name: lowercase letters, digits and single hyphens
    name: String,
}
rmcp::elicit_safe!(RepoNameInput);

/// The decision `repo_name_block` acts on.
#[derive(Debug)]
enum RepoNameOutcome {
    /// `.codescout/repo.toml` already holds this name.
    Confirmed(RepoName),
    /// The user supplied this name and the file does not exist: write it.
    Write(RepoName),
    /// Nobody supplied a name: offer this suggestion.
    Unconfirmed(RepoName),
    /// The file exists and cannot be read: report it, write nothing.
    Unreadable(String),
    /// The user supplied a name that is not canonical.
    Invalid { input: String, error: NameError },
}

/// Pure decision over what `repo_name::read` found and what the user answered. An existing
/// name always wins over an answer, and an unreadable file is never a licence to write.
fn repo_name_outcome(
    existing: anyhow::Result<Option<RepoName>>,
    elicited: Option<String>,
    suggestion: &RepoName,
) -> RepoNameOutcome {
    match existing {
        Err(e) => RepoNameOutcome::Unreadable(names_repo_file(format!("{e:#}"))),
        Ok(Some(name)) => RepoNameOutcome::Confirmed(name),
        Ok(None) => match elicited.as_deref().map(str::trim) {
            None | Some("") => RepoNameOutcome::Unconfirmed(suggestion.clone()),
            Some(answer) => match RepoName::parse(answer) {
                Ok(name) => RepoNameOutcome::Write(name),
                Err(error) => RepoNameOutcome::Invalid {
                    input: answer.to_string(),
                    error,
                },
            },
        },
    }
}

/// The root the repo-name file lives under: the git root of `project_root` (the nearest ancestor
/// holding `.git`, a directory or a file, as `CurrentProject` finds it), else `project_root`
/// itself when no git is above it. Keys are repo-relative paths, so every project in one repo
/// shares this root and the one name kept there.
fn repo_root_of(project_root: &std::path::Path) -> std::path::PathBuf {
    crate::librarian::current_project::lookup_git_root(project_root)
        .unwrap_or_else(|| project_root.to_path_buf())
}

/// The `repo_name` block of an `onboarding` response. Reads the file once, asks the user
/// only when it is absent, and writes only a validated answer to an absent file. `root` is the
/// repo root.
async fn repo_name_block(ctx: &ToolContext, root: &std::path::Path) -> Value {
    let existing = repo_name::read(root);
    let suggestion = repo_name::suggest(root);
    let elicited = if matches!(existing, Ok(None)) {
        let prompt = format!(
            "Confirm the name of the repo at {root}. Suggested: `{s}`. Use lowercase letters, \
             digits and single hyphens. The name labels this repo in citations of the form \
             [[repo/key]].",
            root = root.display(),
            s = suggestion
        );
        // A declined, cancelled, unanswered or unsupported prompt is the same as no answer.
        // The wait is bounded: this call holds the project's write guard and runs inside the
        // server's tool timeout, and an answer that arrives too late is lost with the response.
        ctx.elicit_with_timeout::<RepoNameInput>(prompt, ONBOARDING_ELICIT_TIMEOUT)
            .await
            .ok()
            .flatten()
            .map(|input| input.name)
    } else {
        None
    };
    repo_name_block_for(root, existing, elicited, &suggestion)
}

/// The block for what `read` found and what the user answered: writes a valid answer to an
/// absent file and maps every outcome to its response shape.
fn repo_name_block_for(
    root: &std::path::Path,
    existing: anyhow::Result<Option<RepoName>>,
    elicited: Option<String>,
    suggestion: &RepoName,
) -> Value {
    let unconfirmed = |s: &RepoName| {
        json!({
            "suggested": s.as_str(),
            "confirmed": false,
            "instruction": format!(
                "Ask the user to confirm or edit the repo name `{s}`, then call \
                 onboarding(repo_name=\"<name>\")."
            ),
        })
    };
    match repo_name_outcome(existing, elicited, suggestion) {
        RepoNameOutcome::Confirmed(n) => json!({"name": n.as_str(), "confirmed": true}),
        RepoNameOutcome::Write(n) => match repo_name::write(root, &n) {
            Ok(path) => json!({
                "name": n.as_str(),
                "confirmed": true,
                "written": path.display().to_string(),
            }),
            Err(e) => json!({"error": names_repo_file(format!("{e:#}")), "confirmed": false}),
        },
        RepoNameOutcome::Unconfirmed(s) => unconfirmed(&s),
        RepoNameOutcome::Unreadable(msg) => json!({"error": msg, "confirmed": false}),
        RepoNameOutcome::Invalid { input, error } => {
            let mut block = unconfirmed(suggestion);
            block["error"] = json!(format!("repo name `{input}` is not canonical: {error}"));
            block
        }
    }
}

/// `msg`, prefixed with the file's path when it does not already name `repo.toml`.
fn names_repo_file(msg: String) -> String {
    if msg.contains("repo.toml") {
        msg
    } else {
        format!("{REPO_FILE_REL}: {msg}")
    }
}

/// `onboarding(repo_name=...)`: record the name the user confirmed at the repo root `root`. A
/// valid name replaces an
/// existing valid one; a file that cannot be read is reported and left as it is.
fn set_repo_name(root: &std::path::Path, raw: &str) -> anyhow::Result<Value> {
    let name = RepoName::parse(raw.trim()).map_err(|e| {
        super::RecoverableError::with_hint(
            format!("repo name {raw} is not canonical: {e} ({REPO_FILE_REL} not written)"),
            "Use lowercase letters, digits and single hyphens, e.g. claude-plugins.",
        )
    })?;
    repo_name::read(root).map_err(|e| {
        super::RecoverableError::with_hint(
            format!("{e:#}"),
            format!(
                "Fix or delete {REPO_FILE_REL}, then call onboarding(repo_name=\"{name}\") again."
            ),
        )
    })?;
    let written = repo_name::write(root, &name).map_err(|e| {
        super::RecoverableError::with_hint(
            names_repo_file(format!("{e:#}")),
            format!(
                "Check that {REPO_FILE_REL} and its directory are writable, then call \
                 onboarding(repo_name=\"{name}\") again."
            ),
        )
    })?;
    Ok(json!({
        "repo_name": {
            "name": name.as_str(),
            "confirmed": true,
            "written": written.display().to_string(),
        }
    }))
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
/// `{ ready, files, chunks }` envelope. When the probe could not look at the index (no project,
/// no retrieval client, a failed statistics call) `ready` is `false` and the envelope also
/// carries `unchecked`, the reason. `ready: false` without it is an answer: the index is empty.
async fn probe_index_status(ctx: &ToolContext) -> Value {
    let unchecked =
        |reason: &str| json!({ "ready": false, "files": 0, "chunks": 0, "unchecked": reason });
    let project_id = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok(p.project_id().to_string())
        })
        .await
        .unwrap_or_else(|_| String::new());
    if project_id.is_empty() {
        return unchecked("no project to check");
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
                Err(_) => unchecked("the retrieval store did not answer the statistics call"),
            }
        }
        Err(_) => unchecked("the retrieval client could not be created"),
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
            let mem_dir = crate::workspace::member_memory_dir(
                root,
                &project.id,
                Some(project.relative_root.as_path()),
            );
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
        let mem_dir = crate::workspace::member_memory_dir(
            root,
            &project.id,
            Some(project.relative_root.as_path()),
        );
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
    // The recommended entry names BOTH fields it wants set. Reading a single
    // `id` string here is what let the former `id: "url"` sentinel reach
    // `[embeddings].model`; `OptionTarget` splits them so a server option
    // configures `url` and a model option configures `model`.
    let recommended_embeddings = model_options
        .first()
        .expect("model_options_for_hardware guarantees ≥1 entry")
        .embeddings_section();

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
            embeddings: recommended_embeddings,
            ignored_paths: Default::default(),
            security: Default::default(),
            memory: Default::default(),
            libraries: Default::default(),
            lsp: Default::default(),
        };
        // `[security]` is left OUT of the file: the file is merged over the global config, so an
        // explicit table of defaults here would override a stricter global `shell_command_mode`
        // or `indexing_enabled` for this project. Absent, the project inherits the global
        // value or the same defaults it was just given.
        let mut doc = toml::Value::try_from(&config)?;
        if let Some(table) = doc.as_table_mut() {
            table.remove("security");
        }
        let toml_str = toml::to_string_pretty(&doc)?;
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
    let discovered = ctx
        .agent
        .discovered_projects_at(ctx.workspace_override.as_deref())
        .await;
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
        index_unchecked: index_status["unchecked"].as_str(),
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
    crate::prompts::builders::append_preferences_section(
        &ctx.agent,
        ctx.workspace_override.as_deref(),
        &mut system_prompt_draft,
    )
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

/// `text` followed by the response's `repo_name` block, so the wire text carries what the
/// JSON value does. `text` unchanged when the value has no block.
fn with_repo_name(text: &str, val: &Value) -> String {
    match val.get("repo_name") {
        Some(block) => format!("{text}\nrepo_name: {block}"),
        None => text.to_string(),
    }
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
    fn patch_project_table_errors_never_quote_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("project.toml");
        std::fs::write(&p, "db.internal:5432:app:alice:SECRETMARKER-31c9\n").unwrap();
        let err = format!("{:#}", patch_project_table(&p, |_| {}).unwrap_err());
        assert!(err.contains("parsing"), "control: {err}");
        assert!(!err.contains("SECRETMARKER"), "{err}");
    }

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

    /// Review LOW-3. A protected topic that cannot be read (a link, a directory) used to
    /// become `{"exists": false}`, and the onboarding protocol says `exists == false`
    /// means "create fresh" — which would then be refused by the write side, or, worse,
    /// taken as licence to replace what the user keeps there. It carries `unreadable`
    /// and the reason instead, and NO `exists` key, so neither protocol branch matches.
    #[cfg(unix)]
    #[test]
    fn a_protected_topic_that_cannot_be_read_is_unreadable_not_missing() {
        let dir = tempfile::tempdir().unwrap();
        let memories_dir = dir.path().join(".codescout/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(dir.path().join("secret"), "SECRET-BYTES").unwrap();
        std::os::unix::fs::symlink(dir.path().join("secret"), memories_dir.join("gotchas.md"))
            .unwrap();
        let store = crate::memory::MemoryStore::from_dir_readonly(memories_dir.clone());

        let state = gather_protected_memory_state(
            &store,
            &memories_dir,
            dir.path(),
            &["gotchas".to_string(), "absent".to_string()],
        );

        let linked = &state["gotchas"];
        assert_eq!(linked["unreadable"], true, "got {linked}");
        assert!(
            linked.get("exists").is_none(),
            "no protocol branch may match: {linked}"
        );
        assert!(linked["error"].as_str().unwrap().contains("regular file"));
        assert!(!state.to_string().contains("SECRET-BYTES"));
        // A genuinely absent topic is still the create-fresh case.
        assert_eq!(state["absent"], json!({ "exists": false }));
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

    // ---- repo name confirmation ----

    #[test]
    fn outcome_keeps_a_confirmed_name_and_never_elicits_over_it() {
        let s = RepoName::parse("suggested").unwrap();
        let have = RepoName::parse("codescout").unwrap();
        assert!(
            matches!(repo_name_outcome(Ok(Some(have)), Some("other".into()), &s),
            RepoNameOutcome::Confirmed(n) if n.as_str() == "codescout")
        );
    }

    #[test]
    fn outcome_writes_an_elicited_name_and_reports_an_invalid_one() {
        let s = RepoName::parse("suggested").unwrap();
        assert!(
            matches!(repo_name_outcome(Ok(None), Some(" codescout ".into()), &s),
            RepoNameOutcome::Write(n) if n.as_str() == "codescout")
        );
        assert!(matches!(
            repo_name_outcome(Ok(None), Some("Code Scout".into()), &s),
            RepoNameOutcome::Invalid { .. }
        ));
    }

    #[test]
    fn outcome_is_unconfirmed_without_elicitation_or_with_an_empty_answer() {
        let s = RepoName::parse("suggested").unwrap();
        assert!(
            matches!(repo_name_outcome(Ok(None), None, &s), RepoNameOutcome::Unconfirmed(n) if n.as_str() == "suggested")
        );
        assert!(matches!(
            repo_name_outcome(Ok(None), Some("  ".into()), &s),
            RepoNameOutcome::Unconfirmed(_)
        ));
    }

    #[test]
    fn outcome_never_overwrites_an_unreadable_file() {
        let s = RepoName::parse("suggested").unwrap();
        let out = repo_name_outcome(
            Err(anyhow::anyhow!("reading .codescout/repo.toml: bad")),
            Some("x".into()),
            &s,
        );
        assert!(matches!(out, RepoNameOutcome::Unreadable(m) if m.contains("repo.toml")));
    }
    /// With no project to look at, the probe did not look at any index: its answer is "unknown",
    /// which `ready: false` alone cannot say. The envelope carries `unchecked` with the reason.
    /// BUG docs/issues/2026-11-11-onboarding-probe-failure-is-reported-as-index-not-built.md
    #[tokio::test]
    async fn a_probe_with_no_project_says_it_did_not_check_the_index() {
        let agent = crate::agent::Agent::new(None).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
            guide_hints_emitted: std::sync::Arc::new(parking_lot::Mutex::new(Default::default())),
            workspace_override: None,
        };
        let status = probe_index_status(&ctx).await;
        assert_eq!(status["ready"], json!(false), "{status}");
        assert_eq!(status["chunks"], json!(0), "{status}");
        assert!(
            status["unchecked"]
                .as_str()
                .is_some_and(|reason| reason.contains("no project")),
            "{status}"
        );
    }

    async fn ctx_over(root: &std::path::Path) -> ToolContext {
        let agent = crate::agent::Agent::new(Some(root.to_path_buf()))
            .await
            .unwrap();
        ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
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

    async fn repo_name_ctx() -> (tempfile::TempDir, ToolContext) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let ctx = ctx_over(dir.path()).await;
        (dir, ctx)
    }

    #[tokio::test]
    async fn the_repo_name_parameter_refuses_a_non_canonical_name_and_writes_nothing() {
        let (dir, ctx) = repo_name_ctx().await;
        let err = Onboarding
            .call(json!({"repo_name": "Bad Name"}), &ctx)
            .await
            .unwrap_err();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("a RecoverableError");
        assert!(rec.message.contains("repo.toml"), "{}", rec.message);
        let hint = rec.hint().expect("a hint");
        assert!(
            hint.contains("lowercase letters, digits and single hyphens"),
            "{hint}"
        );
        assert!(!dir.path().join(REPO_FILE_REL).exists());
    }

    #[tokio::test]
    async fn the_repo_name_parameter_writes_repo_toml_and_confirms() {
        let (dir, ctx) = repo_name_ctx().await;
        let out = Onboarding
            .call(json!({"repo_name": "codescout"}), &ctx)
            .await
            .unwrap();
        assert_eq!(out["repo_name"]["name"], "codescout");
        assert_eq!(out["repo_name"]["confirmed"], true);
        assert!(
            out["repo_name"]["written"]
                .as_str()
                .unwrap()
                .ends_with("repo.toml"),
            "{out}"
        );
        let on_disk = repo_name::read(dir.path()).unwrap().unwrap();
        assert_eq!(on_disk.as_str(), "codescout");

        // The wire text is the block, not a "[?]" onboarding summary.
        let blocks = Onboarding
            .call_content(json!({"repo_name": "codescout"}), &ctx)
            .await
            .unwrap();
        let text = blocks[0].as_text().unwrap().text.clone();
        assert!(text.contains("\"confirmed\": true"), "{text}");
        assert!(!text.contains("[?]"), "{text}");
    }

    #[tokio::test]
    async fn the_repo_name_parameter_never_writes_over_an_unreadable_file() {
        let (dir, ctx) = repo_name_ctx().await;
        let file = dir.path().join(REPO_FILE_REL);
        std::fs::write(&file, "name = [not toml").unwrap();
        let err = Onboarding
            .call(json!({"repo_name": "codescout"}), &ctx)
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("repo.toml"), "{err:#}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "name = [not toml");
    }

    #[tokio::test]
    async fn every_onboarding_response_carries_the_repo_name_block() {
        let (dir, ctx) = repo_name_ctx().await;
        // No client to ask and no file: a suggestion, unconfirmed, nothing written.
        let first = Onboarding.call(json!({}), &ctx).await.unwrap();
        let block = &first["repo_name"];
        assert_eq!(block["confirmed"], false, "{first}");
        let suggested = block["suggested"].as_str().expect("a suggestion");
        assert!(
            block["instruction"]
                .as_str()
                .unwrap()
                .contains(&format!("`{suggested}`")),
            "{block}"
        );
        assert!(!dir.path().join(REPO_FILE_REL).exists());

        // A second call: the project has no `onboarding` memory and no stamped version, so it
        // does not reach the plain already-onboarded text. It answers with the subagent-prompt
        // JSON, and the wire text of that response carries the block.
        let blocks = Onboarding.call_content(json!({}), &ctx).await.unwrap();
        let text = blocks
            .iter()
            .filter_map(|c| c.as_text().map(|t| t.text.clone()))
            .collect::<String>();
        assert!(text.contains("\"prompt_path\""), "{text}");
        assert!(text.contains("\"suggested\""), "{text}");

        // With the file present, the block confirms the stored name and writes nothing.
        repo_name::write(dir.path(), &RepoName::parse("codescout").unwrap()).unwrap();
        let again = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(
            again["repo_name"],
            json!({"name": "codescout", "confirmed": true})
        );
    }

    // Keys are repo-relative paths, so two projects in one repo share one key space and one
    // name: the name file lives at the git root, not at each project's root.
    #[tokio::test]
    async fn the_repo_name_lives_at_the_git_root_of_a_nested_project() {
        let tmp = tempfile::tempdir().unwrap();
        let repo = tmp.path().join("repo");
        let sub = repo.join("crates/sub");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        std::fs::create_dir_all(sub.join(".codescout")).unwrap();
        std::fs::write(sub.join("main.rs"), "fn main() {}").unwrap();
        let ctx = ctx_over(&sub).await;

        // Before any name: the suggestion comes from the repo root, not the project's folder.
        let first = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(first["repo_name"]["suggested"], "repo", "{first}");

        let out = Onboarding
            .call(json!({"repo_name": "myname"}), &ctx)
            .await
            .unwrap();
        let written = out["repo_name"]["written"].as_str().unwrap();
        assert_eq!(
            std::fs::canonicalize(written).unwrap(),
            std::fs::canonicalize(repo.join(REPO_FILE_REL)).unwrap(),
            "{out}"
        );
        assert!(repo.join(REPO_FILE_REL).is_file());
        assert!(!sub.join(REPO_FILE_REL).exists());

        // A call without the parameter reads the same file.
        let again = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(
            again["repo_name"],
            json!({"name": "myname", "confirmed": true}),
            "{again}"
        );
        assert!(!sub.join(REPO_FILE_REL).exists());
    }

    // With no git above the project, the project root is the repo root.
    #[tokio::test]
    async fn without_git_the_repo_name_lives_at_the_project_root() {
        let (dir, ctx) = repo_name_ctx().await;
        assert!(
            crate::librarian::current_project::lookup_git_root(dir.path()).is_none(),
            "the temp dir sits under a git checkout"
        );
        let out = Onboarding
            .call(json!({"repo_name": "plain"}), &ctx)
            .await
            .unwrap();
        let written = out["repo_name"]["written"].as_str().unwrap();
        assert_eq!(
            std::fs::canonicalize(written).unwrap(),
            std::fs::canonicalize(dir.path().join(REPO_FILE_REL)).unwrap(),
            "{out}"
        );
    }

    #[tokio::test]
    async fn a_malformed_repo_toml_is_reported_by_onboarding_and_left_alone() {
        let (dir, ctx) = repo_name_ctx().await;
        let file = dir.path().join(REPO_FILE_REL);
        std::fs::write(&file, "name = [not toml").unwrap();
        let out = Onboarding.call(json!({}), &ctx).await.unwrap();
        assert_eq!(out["repo_name"]["confirmed"], false, "{out}");
        assert!(
            out["repo_name"]["error"]
                .as_str()
                .unwrap()
                .contains("repo.toml"),
            "{out}"
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "name = [not toml");
    }

    /// The plain already-onboarded text: a project with a stamped current version and an
    /// `onboarding` memory, so `handle_already_onboarded` answers with its `message`.
    #[tokio::test]
    async fn the_already_onboarded_text_carries_the_repo_name_block() {
        let dir = tempfile::tempdir().unwrap();
        let cs = dir.path().join(".codescout");
        std::fs::create_dir_all(cs.join("memories")).unwrap();
        std::fs::write(
            cs.join("project.toml"),
            format!("[project]\nname = \"t\"\nonboarding_version = {ONBOARDING_VERSION}\n"),
        )
        .unwrap();
        std::fs::write(cs.join("memories").join("onboarding.md"), "x").unwrap();
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        let ctx = ctx_over(dir.path()).await;

        let blocks = Onboarding.call_content(json!({}), &ctx).await.unwrap();
        let text = blocks[0].as_text().unwrap().text.clone();
        assert!(text.contains("Onboarding already performed"), "{text}");
        assert!(text.contains("\nrepo_name: {"), "{text}");
        assert!(text.contains("\"suggested\""), "{text}");
        assert!(!dir.path().join(REPO_FILE_REL).exists());
    }

    fn for_block(dir: &std::path::Path, elicited: Option<&str>) -> Value {
        let suggestion = RepoName::parse("suggested").unwrap();
        repo_name_block_for(dir, Ok(None), elicited.map(String::from), &suggestion)
    }

    #[test]
    fn an_elicited_name_is_written_and_reported() {
        let dir = tempfile::tempdir().unwrap();
        let block = for_block(dir.path(), Some("codescout"));
        assert_eq!(block["name"], "codescout");
        assert_eq!(block["confirmed"], true);
        assert!(block["written"].as_str().unwrap().ends_with("repo.toml"));
        let on_disk = repo_name::read(dir.path()).unwrap().unwrap();
        assert_eq!(on_disk.as_str(), "codescout");
    }

    #[test]
    fn an_elicited_name_that_is_not_canonical_is_reported_and_nothing_is_written() {
        let dir = tempfile::tempdir().unwrap();
        let block = for_block(dir.path(), Some("Code Scout"));
        assert_eq!(block["confirmed"], false);
        assert_eq!(block["suggested"], "suggested");
        assert!(block["instruction"]
            .as_str()
            .unwrap()
            .contains("`suggested`"));
        let error = block["error"].as_str().unwrap();
        assert!(error.contains("Code Scout"), "{error}");
        assert!(!dir.path().join(REPO_FILE_REL).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_write_that_fails_is_reported_as_an_error_block() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let cs = dir.path().join(".codescout");
        std::fs::create_dir_all(&cs).unwrap();
        std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o500)).unwrap();
        // Root ignores directory permissions: nothing to observe then.
        let probe = cs.join("probe");
        if std::fs::File::create(&probe).is_ok() {
            std::fs::remove_file(&probe).unwrap();
            std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }
        let block = for_block(dir.path(), Some("codescout"));
        std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(block["confirmed"], false, "{block}");
        assert!(
            block["error"].as_str().unwrap().contains("repo.toml"),
            "{block}"
        );
        assert!(block.get("written").is_none(), "{block}");
        assert!(!dir.path().join(REPO_FILE_REL).exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn the_repo_name_parameter_reports_a_failed_write_as_a_recoverable_error() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, ctx) = repo_name_ctx().await;
        let cs = dir.path().join(".codescout");
        std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o500)).unwrap();
        let probe = cs.join("probe");
        if std::fs::File::create(&probe).is_ok() {
            std::fs::remove_file(&probe).unwrap();
            std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o700)).unwrap();
            return;
        }
        let err = Onboarding
            .call(json!({"repo_name": "codescout"}), &ctx)
            .await
            .unwrap_err();
        std::fs::set_permissions(&cs, std::fs::Permissions::from_mode(0o700)).unwrap();
        let rec = err
            .downcast_ref::<crate::tools::RecoverableError>()
            .expect("a RecoverableError");
        assert!(rec.message.contains("repo.toml"), "{}", rec.message);
        assert!(rec.hint().is_some());
    }
}

/// What onboarding writes into `project.toml` must not override a STRICTER global config.
///
/// The file is merged over `~/.config/codescout/config.toml`, so every key written into it
/// wins. Onboarding serialised a whole `ProjectConfig`, so it wrote an explicit `[security]`
/// table of DEFAULTS (`shell_command_mode = "warn"`, `indexing_enabled = true`, ...) and
/// silently relaxed a global `shell_command_mode = "disabled"` / `indexing_enabled = false`
/// for that project. Each test runs the real onboarding code path in a child process whose
/// `XDG_CONFIG_HOME` holds the stricter global config.
#[cfg(all(test, unix))]
mod global_strictness_tests {
    use super::*;
    use crate::agent::Agent;

    const STRICT_GLOBAL: &str =
        "[security]\nshell_command_mode = \"disabled\"\nindexing_enabled = false\n";

    fn ctx(agent: Agent) -> ToolContext {
        ToolContext {
            agent,
            lsp: crate::lsp::LspManager::new_arc(),
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

    /// Run `body(root, ctx)` in a child whose global config is `STRICT_GLOBAL`, over a project
    /// holding `project_toml` (or none). Bodies assert their own controls: the project's own
    /// `[security]`, where it has one, legitimately overrides the global config.
    fn in_strict_global(
        name: &'static str,
        project_toml: Option<&'static str>,
        body: impl for<'a> FnOnce(
                &'a std::path::Path,
                &'a ToolContext,
            )
                -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + 'a>>
            + 'static,
    ) {
        let fixture = tempfile::tempdir().unwrap();
        let xdg = fixture.path().canonicalize().unwrap().join("xdg");
        crate::platform::test_child::run(
            module_path!(),
            name,
            &[("XDG_CONFIG_HOME", Some(xdg.to_str().unwrap()))],
            move || {
                let xdg = std::path::PathBuf::from(std::env::var("XDG_CONFIG_HOME").unwrap());
                std::fs::create_dir_all(xdg.join("codescout")).unwrap();
                std::fs::write(xdg.join("codescout").join("config.toml"), STRICT_GLOBAL).unwrap();
                let root = xdg.parent().unwrap().join("proj");
                std::fs::create_dir_all(root.join(".codescout")).unwrap();
                std::fs::write(root.join("main.rs"), "fn main() {}").unwrap();
                if let Some(t) = project_toml {
                    std::fs::write(root.join(".codescout").join("project.toml"), t).unwrap();
                }
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let agent = Agent::new(Some(root.clone())).await.unwrap();
                        let ctx = ctx(agent);
                        body(&root, &ctx).await;
                    });
            },
        );
    }

    async fn effective(root: &std::path::Path) -> (String, bool) {
        let c = crate::config::project::ProjectConfig::load_or_default(root).unwrap();
        (c.security.shell_command_mode, c.security.indexing_enabled)
    }

    fn file(root: &std::path::Path) -> String {
        std::fs::read_to_string(root.join(".codescout").join("project.toml")).unwrap()
    }

    #[test]
    fn a_created_project_toml_does_not_relax_a_stricter_global_config() {
        in_strict_global(
            "a_created_project_toml_does_not_relax_a_stricter_global_config",
            None,
            |root, ctx| {
                Box::pin(async move {
                    assert_eq!(
                        effective(root).await,
                        ("disabled".to_string(), false),
                        "control: the stricter global config is in force before onboarding"
                    );
                    let result = Onboarding.call(json!({}), ctx).await.unwrap();
                    assert_eq!(
                        result["config_created"], true,
                        "control: onboarding wrote it"
                    );
                    assert_eq!(
                        effective(root).await,
                        ("disabled".to_string(), false),
                        "onboarding's project.toml relaxed the stricter global config; file:\n{}",
                        file(root)
                    );
                })
            },
        );
    }

    #[test]
    fn recording_a_refresh_request_does_not_freeze_the_global_values_into_project_toml() {
        in_strict_global(
            "recording_a_refresh_request_does_not_freeze_the_global_values_into_project_toml",
            Some("[project]\nname = \"t\"\n"),
            |root, ctx| {
                Box::pin(async move {
                    record_refresh_request(ctx).await.unwrap();
                    assert!(
                        file(root).contains(&hash_system_prompt(root)),
                        "control: the baseline was recorded: {}",
                        file(root)
                    );
                    assert_eq!(
                        effective(root).await,
                        ("disabled".to_string(), false),
                        "file:\n{}",
                        file(root)
                    );
                    // Measured: writing the MERGED config back copies the global values into
                    // the project's file (it does not relax them at that moment), where they
                    // then shadow any later change to the global config.
                    assert!(
                        !file(root).contains("[security]"),
                        "the global values were frozen into project.toml:\n{}",
                        file(root)
                    );
                })
            },
        );
    }

    #[test]
    fn stamping_a_witnessed_refresh_does_not_freeze_the_global_values_into_project_toml() {
        in_strict_global(
            "stamping_a_witnessed_refresh_does_not_freeze_the_global_values_into_project_toml",
            Some(
                "[project]\nname = \"t\"\nonboarding_version = 1\nsystem_prompt_sha256 = \"abc\"\n",
            ),
            |root, ctx| {
                Box::pin(async move {
                    assert!(
                        stamp_witnessed_refresh(ctx).await.unwrap(),
                        "control: the stamp was written"
                    );
                    assert!(
                        file(root).contains(&format!("onboarding_version = {ONBOARDING_VERSION}")),
                        "control: the version was stamped:\n{}",
                        file(root)
                    );
                    assert!(
                        file(root).contains(&hash_system_prompt(root)),
                        "control: the baseline was stamped:\n{}",
                        file(root)
                    );
                    assert_eq!(
                        effective(root).await,
                        ("disabled".to_string(), false),
                        "file:\n{}",
                        file(root)
                    );
                    assert!(
                        !file(root).contains("[security]"),
                        "the global values were frozen into project.toml:\n{}",
                        file(root)
                    );
                })
            },
        );
    }

    /// The control that keeps the fix honest: a project's OWN explicit `[security]` is the
    /// user's, and a stamp must leave it exactly as written.
    #[test]
    fn a_stamp_keeps_the_projects_own_security_table_as_written() {
        in_strict_global(
            "a_stamp_keeps_the_projects_own_security_table_as_written",
            Some("[project]\nname = \"t\"\n\n[security]\nshell_command_mode = \"warn\"\n"),
            |root, ctx| {
                Box::pin(async move {
                    record_refresh_request(ctx).await.unwrap();
                    let text = file(root);
                    assert!(
                        text.contains("shell_command_mode = \"warn\""),
                        "the project's own key was lost: {text}"
                    );
                    assert!(
                        !text.contains("indexing_enabled"),
                        "a key the project never wrote was materialised: {text}"
                    );
                })
            },
        );
    }
}
/// `patch_project_table` re-reads `project.toml` after the loader did, and `hash_system_prompt`
/// reads `.codescout/system-prompt.md` with no loader before it: both paths are repository
/// content.
/// docs/issues/2026-10-20-reads-of-repository-files-that-remain-unbounded-after-the-config-loader-fix.md
#[cfg(all(test, unix))]
mod bounded_read_tests {
    use super::*;
    use crate::util::fs::special_file_fixtures::{make_fifo, within_deadline};

    /// A FIFO (or a link to one) at the path held the re-read for good, before the write below
    /// it could be refused. Now it returns an error that names the path, quotes nothing, and
    /// the path is left a FIFO: nothing was written through it.
    #[test]
    fn patch_project_table_refuses_a_fifo_or_a_link_to_one_and_writes_nothing() {
        for linked in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let p = dir.path().join("project.toml");
            if linked {
                let fifo = dir.path().join("elsewhere-pipe");
                make_fifo(&fifo);
                std::os::unix::fs::symlink(&fifo, &p).unwrap();
            } else {
                make_fifo(&p);
            }
            let path = p.clone();
            let err = within_deadline("patch_project_table on a FIFO", move || {
                patch_project_table(&path, |t| {
                    t.insert("touched".into(), toml::Value::Boolean(true));
                })
            })
            .unwrap_err();
            let msg = format!("{err:#}");
            assert!(msg.contains("not a regular file"), "linked={linked}: {msg}");
            assert!(msg.contains("project.toml"), "names the path: {msg}");
            let ft = std::fs::metadata(&p).unwrap().file_type();
            assert!(!ft.is_file(), "still a FIFO, nothing written through it");
        }
    }

    /// Control: a regular `project.toml` (and a link to one) is patched as before, the other
    /// keys survive; a missing file keeps the I/O error it always gave.
    #[test]
    fn patch_project_table_still_patches_a_regular_file_and_reports_a_missing_one_as_before() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("project.toml");
        std::fs::write(&p, "[project]\nname = \"keep\"\n").unwrap();
        patch_project_table(&p, |t| {
            t.insert("added".into(), toml::Value::Integer(7));
        })
        .unwrap();
        let doc: toml::Table = toml::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(doc["project"]["name"].as_str(), Some("keep"));
        assert_eq!(doc["project"]["added"].as_integer(), Some(7));

        let real = dir.path().join("real.toml");
        std::fs::write(&real, "[project]\nname = \"linked\"\n").unwrap();
        let link = dir.path().join("link.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        patch_project_table(&link, |t| {
            t.insert("added".into(), toml::Value::Integer(8));
        })
        .unwrap();
        let doc: toml::Table = toml::from_str(&std::fs::read_to_string(&real).unwrap()).unwrap();
        assert_eq!(doc["project"]["added"].as_integer(), Some(8));

        let err = patch_project_table(&dir.path().join("missing.toml"), |_| {}).unwrap_err();
        assert!(
            err.downcast_ref::<std::io::Error>()
                .is_some_and(|e| e.kind() == std::io::ErrorKind::NotFound),
            "an io::Error as before: {err:#}"
        );
    }

    /// Exactly at the limit is patched; one byte over is refused with the limit named and the file
    /// left byte for byte as it was. The padding is TOML whitespace, so a read without the limit
    /// would parse the over-limit file and rewrite it.
    #[test]
    fn patch_project_table_refuses_a_file_over_the_limit_and_leaves_it_unwritten() {
        let limit = crate::util::fs::MAX_CONFIG_FILE_BYTES as usize;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("project.toml");
        let mut body = "[project]\nname = \"keep\"\n".to_string();
        body.push_str(&" ".repeat(limit - body.len()));
        std::fs::write(&p, &body).unwrap();
        patch_project_table(&p, |t| {
            t.insert("added".into(), toml::Value::Integer(1));
        })
        .unwrap();
        assert!(std::fs::read_to_string(&p).unwrap().contains("added"));

        std::fs::write(&p, format!("{body} ")).unwrap();
        let before = std::fs::read(&p).unwrap();
        let err = patch_project_table(&p, |t| {
            t.insert("added".into(), toml::Value::Integer(2));
        })
        .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("byte limit"), "{msg}");
        assert!(!msg.contains("keep"), "no content in the error: {msg}");
        assert_eq!(std::fs::read(&p).unwrap(), before, "left unwritten");
    }

    /// A FIFO or a link to a device at `system-prompt.md` hashes as an absent prompt (empty
    /// content), without a hang; a regular prompt hashes its bytes.
    #[test]
    fn hash_system_prompt_treats_a_special_file_as_absent_and_hashes_a_regular_one() {
        let dir = tempfile::tempdir().unwrap();
        let cs = dir.path().join(".codescout");
        std::fs::create_dir_all(&cs).unwrap();
        let absent = hash_system_prompt(dir.path());
        let file = cs.join("system-prompt.md");

        make_fifo(&file);
        let root = dir.path().to_path_buf();
        let got = within_deadline("hash_system_prompt on a FIFO", move || {
            hash_system_prompt(&root)
        });
        assert_eq!(got, absent, "FIFO");

        std::fs::remove_file(&file).unwrap();
        std::os::unix::fs::symlink("/dev/null", &file).unwrap();
        assert_eq!(hash_system_prompt(dir.path()), absent, "device link");

        std::fs::remove_file(&file).unwrap();
        std::fs::write(&file, "Always use pytest.\n").unwrap();
        let regular = hash_system_prompt(dir.path());
        assert_ne!(regular, absent);
        assert_eq!(
            regular,
            crate::memory::hash::hash_file(&file).unwrap(),
            "the same bytes hash the same through both"
        );
    }
}
