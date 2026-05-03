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
                    "description": "For action='status': flush all LSP clients (used after context compaction)."
                }
            },
            "required": ["action"]
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let action = input
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                super::RecoverableError::with_hint(
                    "workspace requires 'action' parameter",
                    "Pass action='activate' | 'status' | 'list_projects'.",
                )
            })?;
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
        let path = super::require_str_param(&input, "path")?;
        let read_only = optional_bool_param(&input, "read_only");

        // Focus-switch path: bare project ID (no path separator)
        if !path.contains('/') && !path.contains('\\') {
            let is_project_id = {
                let inner = ctx.agent.inner.read().await;
                inner
                    .workspace
                    .as_ref()
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
                let project_root = ctx.agent.require_project_root().await?;
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

        ctx.agent.activate(root.clone(), read_only).await?;

        let scenario = if !had_home {
            HintScenario::FirstActivation
        } else if ctx.agent.is_home().await {
            HintScenario::ReturnToHome
        } else {
            HintScenario::SwitchAway
        };

        let auto_registered = crate::library::auto_register::auto_register_deps(&root, ctx).await;
        build_activation_response(ctx, scenario, &auto_registered).await
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
            tracing::info!("PostCompact: flushed all LSP clients; they will restart lazily.");
            return Ok(json!({
                "flushed": true,
                "hint": "LSP position caches cleared. Clients restart automatically on the next navigation call (symbol_at, references)."
            }));
        }

        // --- Essential config + library section ---
        let (root, languages, embeddings_model, lib_count, lib_indexed) = ctx
            .agent
            .with_project(|p| {
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
            let db_path = crate::embed::index::project_db_path(&root);
            if !db_path.exists() {
                result["index"] = json!({
                    "status": "not_indexed",
                    "hint": "Run index_project() to build the index.",
                });
            } else {
                let root2 = root.clone();
                let index_result = tokio::task::spawn_blocking(move || {
                    let conn = crate::embed::index::open_db(&root2)?;
                    let stats = crate::embed::index::index_stats(&conn)?;
                    let staleness = crate::embed::index::check_index_staleness(&conn, &root2).ok();
                    anyhow::Ok((stats, staleness))
                })
                .await;

                match index_result {
                    Ok(Ok((stats, staleness))) => {
                        let status = match staleness.as_ref() {
                            Some(s) if s.stale => "behind",
                            _ => "up_to_date",
                        };
                        result["index"] = json!({
                            "status": status,
                            "files": stats.file_count,
                            "chunks": stats.chunk_count,
                            "last_updated": stats.indexed_at,
                            "hint": "Call index_status() for model info, by_source breakdown, drift, and progress details.",
                        });
                    }
                    _ => {
                        result["index"] = json!({
                            "status": "not_indexed",
                            "hint": "Run index_project() to build the index.",
                        });
                    }
                }
            }
        }

        // --- Memory staleness section ---
        let staleness_result = ctx
            .agent
            .with_project(|p| {
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

/// Build the activation response JSON for both full-activation and focus-switch paths.
async fn build_activation_response(
    ctx: &ToolContext,
    scenario: HintScenario,
    auto_registered: &[crate::library::auto_register::RegisteredDep],
) -> anyhow::Result<Value> {
    let (
        project_name,
        project_root_str,
        project_root_path,
        languages,
        read_only,
        memories,
        has_index,
        security,
        stored_onboarding_version,
    ) = ctx
        .agent
        .with_project(|p| {
            let memories = p.memory.list().unwrap_or_default();
            let has_index = crate::embed::index::project_db_path(&p.root).exists();
            let security = if !p.read_only {
                Some((p.config.security.profile, p.config.security.shell_enabled))
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
                has_index,
                security,
                p.config.project.onboarding_version,
            ))
        })
        .await?;

    let version_stale = onboarding_version_stale(stored_onboarding_version);

    let index = if has_index {
        json!({"status": "indexed"})
    } else {
        json!({"status": "not_indexed", "hint": "Run index_project() to enable semantic_search."})
    };

    let workspace = ctx.agent.workspace_summary().await;
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

    if let Some(ws) = workspace_json {
        result["workspace"] = json!(ws);
    }

    if let Some((profile, shell)) = security {
        result["security_profile"] = json!(profile);
        result["shell_enabled"] = json!(shell);
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

    if version_stale {
        result["system_prompt_stale"] = json!({
            "stored_version": stored_onboarding_version,
            "current_version": ONBOARDING_VERSION,
            "action": "Run onboarding(action=\"refresh_prompt\") — tool names or signatures have changed."
        });
    }

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

    parts.join(" · ")
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
