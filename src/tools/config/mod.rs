//! Configuration and project management tools.

use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
use crate::tools::onboarding::{onboarding_version_stale, ONBOARDING_VERSION};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct Workspace;

#[async_trait::async_trait]
impl Tool for Workspace {
    fn name(&self) -> &str {
        "workspace"
    }

    fn description(&self) -> &str {
        "Project workspace operations. Actions: \
         `activate` (switch active project; pass `path` and optional `read_only`), \
         `status` (current project + index + memories), \
         `list_projects` (workspace members)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["activate", "status", "list_projects"],
                    "description": "Operation to perform."
                },
                "path": {
                    "type": "string",
                    "description": "For action='activate': project path or workspace project id."
                },
                "read_only": {
                    "type": "boolean",
                    "description": "For action='activate': open in read-only mode (default: false)."
                },
                "post_compact": {
                    "type": "boolean",
                    "description": "Flush all LSP clients after context compaction. Implies action='status' when action is omitted."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let post_compact = input
            .get("post_compact")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let action = match input.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None if post_compact => "status",
            None => {
                return Err(super::RecoverableError::with_hint(
                    "workspace requires 'action' parameter",
                    "Pass action='activate' | 'status' | 'list_projects'.",
                )
                .into());
            }
        };
        match action {
            "activate" => ActivateProject.call(input, ctx).await,
            "status" => ProjectStatus.call(input, ctx).await,
            "list_projects" => {
                let full = ProjectStatus.call(json!({}), ctx).await?;
                Ok(json!({ "workspace": full.get("workspace") }))
            }
            other => Err(super::RecoverableError::with_hint(
                format!("unknown workspace action: {}", other),
                "Valid actions: 'activate', 'status', 'list_projects'.",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // `activate` responses carry `auto_libs` or `project_root` at the top level;
        // `status` responses carry `index`/`memory_staleness`; `list_projects` carries
        // only `workspace`. Use shape detection.
        if result.get("project_hints").is_some() {
            Some(format_activate_project(result))
        } else {
            Some(format_project_status(result))
        }
    }
}

pub struct ActivateProject;
impl ActivateProject {
    pub const NAME: &'static str = "activate_project";
}

pub struct ProjectStatus;

#[async_trait::async_trait]
impl Tool for ActivateProject {
    fn name(&self) -> &str {
        Self::NAME
    }
    fn description(&self) -> &str {
        "Switch the active project to the given path. All subsequent tool calls \
         operate relative to this project root. Response includes `project_hints` \
         (primary language, manifest, entry points, build commands) so agents have \
         context even without running onboarding."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the project root" },
                "read_only": { "type": "boolean", "description": "Activate in read-only mode (default: true for non-home projects, false for home)" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        ctx.guide_hints_emitted.lock().clear();
        let path = super::require_str_param(&input, "path")?;
        let read_only = optional_bool_param(&input, "read_only");

        // Focus-switch path: bare project ID (no path separator)
        if !path.contains('/') && !path.contains('\\') {
            let is_project_id = {
                let inner = ctx.agent.inner.read().await;
                inner
                    .default_workspace()
                    .map(|ws| ws.projects.iter().any(|p| p.discovered.id == path))
                    .unwrap_or(false)
            };
            if is_project_id {
                ctx.agent.activate_within_workspace(path, read_only).await?;
                let scenario = if ctx.agent.is_home().await {
                    HintScenario::ReturnToHome
                } else {
                    HintScenario::SwitchAway
                };
                let project_root = ctx
                    .agent
                    .require_project_root_for(ctx.workspace_override.as_deref())
                    .await?;
                let prewarm_langs = ctx
                    .agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.languages.clone())
                    })
                    .await
                    .unwrap_or_default();
                crate::lsp::prewarm_lsp_background(
                    ctx.lsp.clone(),
                    project_root.clone(),
                    &prewarm_langs,
                );
                let auto_registered =
                    crate::library::auto_register::auto_register_deps(&project_root, ctx).await;
                return build_activation_response(ctx, scenario, &auto_registered).await;
            }
        }

        // Full-activation path
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("path '{}' is not a directory", path),
                "Provide an absolute path to an existing directory.",
            )
            .into());
        }
        let root = root.canonicalize().unwrap_or(root);
        let had_home = ctx.agent.home_root().await.is_some();
        let mut timer = crate::perf::PhaseTimer::start("activate_project");

        ctx.agent.activate(root.clone(), read_only).await?;
        timer.lap("agent_activate");

        let prewarm_langs = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok(p.config.project.languages.clone())
            })
            .await
            .unwrap_or_default();
        crate::lsp::prewarm_lsp_background(ctx.lsp.clone(), root.clone(), &prewarm_langs);

        let scenario = if !had_home {
            HintScenario::FirstActivation
        } else if ctx.agent.is_home().await {
            HintScenario::ReturnToHome
        } else {
            HintScenario::SwitchAway
        };

        let concurrent_warning = ctx.agent.note_activation(&root).await;
        timer.lap("note_activation");
        let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
        timer.lap("auto_register_deps");
        let mut resp = build_activation_response(ctx, scenario, &auto_registered).await?;
        timer.lap("build_response");
        if let Some(w) = concurrent_warning {
            if let Some(obj) = resp.as_object_mut() {
                obj.insert("concurrent_activation_warning".to_string(), json!(w));
            }
        }
        timer.finish();
        Ok(resp)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_activate_project(result))
    }
}

#[async_trait::async_trait]
impl Tool for ProjectStatus {
    fn name(&self) -> &str {
        "project_status"
    }

    fn description(&self) -> &str {
        "Active project state: languages, embedding model, index health summary, and memory staleness. \
         Pass post_compact=true after context compaction to flush stale LSP position caches — \
         clients restart lazily on the next LSP call. \
         Call index_status() for detailed index info and live progress."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "post_compact": {
                    "type": "boolean",
                    "description": "Set true after context compaction to flush stale LSP position caches. \
                                    LSP clients restart lazily on the next navigation call."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::agent::IndexingState;

        // --- PostCompact cache flush ---
        if parse_bool_param(&input["post_compact"]) {
            ctx.lsp.shutdown_all().await;
            // Re-arm guide hints: compaction summarized the guide bodies out
            // of context, so allow them to re-inject. A bare /mcp restart keeps
            // them (persisted ledger); only compaction clears. See
            // docs/issues/2026-06-14-get-guide-reinjects-on-mcp-restart.md.
            ctx.guide_hints_emitted.lock().clear();
            tracing::info!("PostCompact: flushed all LSP clients; they will restart lazily.");
            return Ok(json!({
                "flushed": true,
                "hint": "LSP position caches cleared. Clients restart automatically on the next navigation call (symbol_at, references)."
            }));
        }

        // --- Essential config + library section ---
        let (root, languages, embeddings_model, lib_count, lib_indexed) = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                let lib_count = p.library_registry.all().len();
                let lib_indexed = p
                    .library_registry
                    .all()
                    .iter()
                    .filter(|e| e.indexed)
                    .count();
                Ok((
                    p.root.clone(),
                    p.config.project.languages.clone(),
                    p.config.embeddings.model.clone(),
                    lib_count,
                    lib_indexed,
                ))
            })
            .await?;

        let mut result = json!({
            "project_root": root.display().to_string(),
            "languages": languages,
            "embeddings_model": embeddings_model,
            "libraries": { "count": lib_count, "indexed": lib_indexed },
        });

        // --- Index section ---
        // Running state takes priority over DB stats.
        let indexing_state = ctx
            .agent
            .indexing
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if let IndexingState::Running {
            done,
            total,
            eta_secs,
        } = indexing_state
        {
            result["index"] = json!({
                "status": "running",
                "done": done,
                "total": total,
                "eta_secs": eta_secs,
                "hint": "Call index_status() for detailed breakdown.",
            });
        } else {
            // Resolve project_id + ask Qdrant for stats. When the retrieval
            // stack is offline or the project has no chunks indexed, fall
            // through to the same "not_indexed" envelope the legacy sqlite
            // path returned.
            let project_id = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok(p.project_id().to_string())
                })
                .await?;
            let qdrant_stats = match crate::retrieval::client::RetrievalClient::from_env().await {
                Ok(client) => {
                    let coll = client.config.collection("code_chunks");
                    client.project_index_stats(&coll, &project_id).await.ok()
                }
                Err(_) => None,
            };
            match qdrant_stats {
                Some((chunks, files)) if chunks > 0 => {
                    result["index"] = json!({
                        "status": "up_to_date",
                        "files": files,
                        "chunks": chunks,
                        "hint": "Call index(action='status') for full Qdrant collection details.",
                    });
                }
                _ => {
                    result["index"] = json!({
                        "status": "not_indexed",
                        "hint": "Run index(action='build') to build the index.",
                    });
                }
            }
        }

        // --- Memory staleness section ---
        let staleness_result = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                let memories_dir = p.root.join(".codescout").join("memories");
                crate::memory::anchors::check_all_memories(&p.root, &memories_dir)
            })
            .await;
        match staleness_result {
            Ok(staleness) => {
                result["memory_staleness"] = staleness;
            }
            Err(e) => {
                tracing::debug!("memory staleness check failed: {e}");
            }
        }

        // --- Workspace section ---
        let workspace_toml_path = crate::config::workspace::workspace_config_path(&root);
        let workspace_info = if workspace_toml_path.exists() {
            std::fs::read_to_string(&workspace_toml_path)
                .ok()
                .and_then(|s| toml::from_str::<crate::config::workspace::WorkspaceConfig>(&s).ok())
                .map(|ws| {
                    json!({
                        "name": ws.workspace.name,
                        "projects": ws.projects.iter().map(|p| json!({
                            "id": p.id,
                            "root": p.root,
                            "languages": p.languages,
                            "depends_on": p.depends_on,
                        })).collect::<Vec<_>>(),
                        "resources": {
                            "max_lsp_clients": ws.resources.max_lsp_clients,
                            "idle_timeout_secs": ws.resources.idle_timeout_secs,
                        },
                    })
                })
        } else {
            None
        };
        result["workspace"] = json!(workspace_info);

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_project_status(result))
    }
}

/// Determines the hint text shown after activation.
enum HintScenario {
    /// First-ever activation (home project, session start)
    FirstActivation,
    /// Returning to the home project after visiting another
    ReturnToHome,
    /// Switching to a non-home project
    SwitchAway,
}

/// Best-effort Qdrant probe: does this project have any chunks indexed?
///
/// Returns `false` when the retrieval stack is offline or the scroll fails.
/// Used by `build_activation_response` to populate the `index.status` field —
/// callers treat `false` as "not indexed" and surface a build hint.
///
/// `_project_root` is accepted for forward-compat in case future probes need
/// to consult on-disk artefacts alongside Qdrant.
async fn check_has_index(project_id: &str, _project_root: &std::path::Path) -> bool {
    match crate::retrieval::client::RetrievalClient::from_env().await {
        Ok(client) => {
            let coll = client.config.collection("code_chunks");
            client
                .project_index_stats(&coll, project_id)
                .await
                .map(|(chunks, _files)| chunks > 0)
                .unwrap_or(false)
        }
        Err(_) => false,
    }
}

/// Build the activation response JSON for both full-activation and focus-switch paths.
async fn build_activation_response(
    ctx: &ToolContext,
    scenario: HintScenario,
    auto_registered: &[crate::library::auto_register::RegisteredDep],
) -> anyhow::Result<Value> {
    let mut timer = crate::perf::PhaseTimer::start("activation_response");
    let (
        project_name,
        project_root_str,
        project_root_path,
        languages,
        read_only,
        memories,
        security_profile,
        stored_onboarding_version,
    ) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            let memories = p.memory.list().unwrap_or_default();
            // Only carry the security profile out of the project lock; it is
            // surfaced later only when it departs from the sandboxed default.
            let security_profile = if !p.read_only {
                Some(p.config.security.profile)
            } else {
                None
            };
            Ok((
                p.config.project.name.clone(),
                p.root.display().to_string(),
                p.root.clone(),
                p.config.project.languages.clone(),
                p.read_only,
                memories,
                security_profile,
                p.config.project.onboarding_version,
            ))
        })
        .await?;

    timer.lap("project_snapshot");

    // has_index probe via Qdrant — best-effort. When the retrieval stack is
    // offline (common in tests), report false rather than erroring out the
    // activation response.
    let has_index = check_has_index(&project_name, &project_root_path).await;
    timer.lap("check_has_index");

    let version_stale = onboarding_version_stale(stored_onboarding_version);

    let index = if has_index {
        json!({"status": "indexed"})
    } else {
        json!({"status": "not_indexed", "hint": "Run index(action='build') to enable semantic_search."})
    };

    let workspace = ctx.agent.workspace_summary().await;
    timer.lap("workspace_summary");
    let workspace_json = workspace.as_ref().map(|projects| {
        projects
            .iter()
            .map(|p| {
                json!({
                    "id": p.id,
                    "root": p.root,
                    "languages": p.languages,
                    "depends_on": p.depends_on,
                })
            })
            .collect::<Vec<_>>()
    });

    let home_root = ctx.agent.home_root().await;
    let hint = match scenario {
        HintScenario::FirstActivation => format!(
            "CWD: {}. Run workspace(action='status') for health checks and memory staleness.",
            project_root_str
        ),
        HintScenario::ReturnToHome => format!(
            "Returned to home project. CWD: {}. Run workspace(action='status') to check memory staleness.",
            project_root_str
        ),
        HintScenario::SwitchAway if read_only => {
            let home_str = home_root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            format!(
                "Browsing {} (read-only). CWD: {} — remember to workspace(action='activate', path=\"{}\") when done.",
                project_name, project_root_str, home_str,
            )
        }
        HintScenario::SwitchAway => {
            let home_str = home_root
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            format!(
                "Switched project (read-write). CWD: {} — remember to workspace(action='activate', path=\"{}\") when done.",
                project_root_str, home_str,
            )
        }
    };

    // Manifest-derived hints so agents that never call `onboarding` still have
    // project context (primary language, entry points, build commands). When
    // an `onboarding` memory is present these are redundant but cheap enough
    // to always include.
    let hints =
        crate::mcp_resources::project_hints::probe_project_hints(&project_root_path, &memories);
    timer.lap("probe_project_hints");

    // Legacy sqlite memory db detection — surfaces a one-line migration hint
    // when `.codescout/embeddings/project.db` exists on disk. This file is
    // produced by the pre-Qdrant codescout (`embed::index`); after a
    // successful `codescout migrate-memories` run + manual delete the field
    // drops out of subsequent activations.
    let legacy_db_path = project_root_path.join(".codescout/embeddings/project.db");
    let legacy_db_present = legacy_db_path.exists();

    let mut result = json!({
        "status": "ok",
        "project": project_name,
        "project_root": project_root_str,
        "read_only": read_only,
        "languages": languages,
        "index": index,
        "memories": memories,
        "project_hints": hints,
        "hint": hint,
    });

    if legacy_db_present {
        result["legacy_semantic_index"] = json!({
            "path": legacy_db_path.display().to_string(),
            "hint": "Run `codescout migrate-memories` to port memories to Qdrant, then delete this file.",
        });
    }

    if let Some(ws) = workspace_json {
        result["workspace"] = json!(ws);
    }

    // Surface `security_profile` only when it departs from the sandboxed
    // `default` — `root` disables every path/command gate, which is worth
    // flagging on activation. `shell_enabled` is intentionally omitted: it has
    // defaulted to true for all projects, so reporting it carried no signal.
    if let Some(profile) = security_profile {
        if profile != crate::util::path_security::SecurityProfile::Default {
            result["security_profile"] = json!(profile);
        }
    }

    if !auto_registered.is_empty() {
        let without_source = auto_registered
            .iter()
            .filter(|r| !r.source_available)
            .count();
        result["auto_registered_libs"] = json!({
            "count": auto_registered.len(),
            "without_source": without_source,
        });
    }

    // stored > ONBOARDING_VERSION (downgrade scenario) intentionally treated as current by onboarding_version_stale
    if version_stale {
        result["system_prompt_stale"] = json!({
            "stored_version": stored_onboarding_version,
            "current_version": ONBOARDING_VERSION,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        });
    }

    timer.finish();
    Ok(result)
}

fn format_activate_project(result: &Value) -> String {
    let name = result["project"].as_str().unwrap_or("?");
    let ro = result["read_only"].as_bool().unwrap_or(true);
    let mode = if ro { "ro" } else { "rw" };
    let mem_count = result["memories"].as_array().map(|a| a.len()).unwrap_or(0);
    let index_status = result["index"]["status"].as_str().unwrap_or("unknown");

    let mut parts = vec![format!(
        "activated · {name} ({mode}) · {mem_count} memories · index: {index_status}"
    )];

    if let Some(ws) = result["workspace"].as_array() {
        parts.push(format!("{} workspace projects", ws.len()));
    }

    if let Some(libs) = result["auto_registered_libs"].as_object() {
        let count = libs.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
        let without = libs
            .get("without_source")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if without > 0 {
            parts.push(format!(
                "auto-registered {} libs ({} without source)",
                count, without
            ));
        } else {
            parts.push(format!("auto-registered {} libs", count));
        }
    }

    let body = parts.join(" · ");

    // Prepend severity-ordered banners. System prompt staleness is higher
    // priority than the legacy-index hint — show both when both fire.
    let legacy_banner = if result["legacy_semantic_index"].is_object() {
        Some("⚠ LEGACY INDEX: run `codescout migrate-memories` to port memories to Qdrant.")
    } else {
        None
    };

    if let Some(stale) = result["system_prompt_stale"].as_object() {
        let stored_label = match stale.get("stored_version").and_then(|v| v.as_u64()) {
            Some(v) => format!("v{v}"),
            None => "none".to_string(),
        };
        let current = stale
            .get("current_version")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let mut out = format!(
            "⚠ SYSTEM PROMPT STALE ({stored_label} → v{current}): run onboarding(action=\"refresh_prompt\") now."
        );
        if let Some(b) = legacy_banner {
            out.push('\n');
            out.push_str(b);
        }
        out.push('\n');
        out.push_str(&body);
        out
    } else if let Some(b) = legacy_banner {
        format!("{b}\n{body}")
    } else {
        body
    }
}

fn format_project_status(result: &Value) -> String {
    let root = result["project_root"].as_str().unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    let status = result["index"]["status"].as_str().unwrap_or("unknown");
    let index_str = match status {
        "up_to_date" | "behind" => {
            let files = result["index"]["files"].as_u64().unwrap_or(0);
            let chunks = result["index"]["chunks"].as_u64().unwrap_or(0);
            format!("index:{files}f/{chunks}c ({status})")
        }
        "running" => {
            let done = result["index"]["done"].as_u64().unwrap_or(0);
            let total = result["index"]["total"].as_u64().unwrap_or(0);
            format!("index:running {done}/{total}")
        }
        _ => "index:none".to_string(),
    };
    format!("status · {name} · {index_str}")
}

#[cfg(test)]
mod tests;
