//! Configuration and project management tools.

use super::{optional_bool_param, parse_bool_param, Tool, ToolContext};
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
            ))
        })
        .await?;

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
mod tests {
    use super::*;
    use crate::agent::Agent;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn lsp() -> Arc<dyn crate::lsp::LspProvider> {
        crate::lsp::LspManager::new_arc()
    }

    #[tokio::test]
    async fn activate_and_get_config() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // No project initially
        assert!(ProjectStatus.call(json!({}), &ctx).await.is_err());

        // Activate
        let result = ActivateProject
            .call(
                json!({
                    "path": dir.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");

        // Now project_status works
        let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        assert!(!status["project_root"].as_str().unwrap().is_empty());
        assert!(status["languages"].is_array());
        assert!(status["embeddings_model"].is_string());
    }

    #[tokio::test]
    async fn activate_surfaces_project_hints_from_cargo_toml() {
        // Agents that never call `onboarding` should still see primary language,
        // manifest, entry points, and build commands in the activate response.
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"demo\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = ActivateProject
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let hints = &result["project_hints"];
        assert_eq!(hints["primary_language"], "rust");
        assert_eq!(hints["manifest"], "Cargo.toml");
        assert_eq!(hints["entry_points"], json!(["src/main.rs"]));
        assert!(
            hints["build_commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "cargo test"),
            "hints must include cargo test: {hints:?}"
        );
        assert_eq!(hints["onboarded"], false);
    }

    #[tokio::test]
    async fn activate_hints_empty_for_unrecognised_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        // No manifest file.

        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = ActivateProject
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();

        let hints = &result["project_hints"];
        assert!(hints["primary_language"].is_null());
        assert!(hints["manifest"].is_null());
        assert_eq!(hints["entry_points"], json!([]));
        assert_eq!(hints["build_commands"], json!([]));
    }

    #[tokio::test]
    async fn activate_nonexistent_path_errors() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(
                json!({
                    "path": "/nonexistent/path/xyz"
                }),
                &ctx,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn activate_replaces_previous_project() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();

        let ctx = ToolContext {
            agent: Agent::new(Some(dir1.path().to_path_buf())).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // Activate dir2
        ActivateProject
            .call(
                json!({
                    "path": dir2.path().to_str().unwrap()
                }),
                &ctx,
            )
            .await
            .unwrap();

        let status = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let root = status["project_root"].as_str().unwrap();
        assert!(root.contains(dir2.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn project_status_returns_all_sections() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let tool = ProjectStatus;
        let result = tool.call(json!({}), &ctx).await.unwrap();
        assert!(result["project_root"].is_string(), "missing project_root");
        assert!(result["languages"].is_array(), "missing languages field");
        assert!(
            result["embeddings_model"].is_string(),
            "missing embeddings_model field"
        );
        assert!(result.get("index").is_some(), "missing index section");
        assert!(
            result.get("libraries").is_some(),
            "missing libraries section"
        );
    }

    #[tokio::test]
    async fn project_status_compact_shape() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();

        // Flat config fields — no blob
        assert!(result["languages"].is_array(), "missing languages");
        assert!(
            result["embeddings_model"].is_string(),
            "missing embeddings_model"
        );
        assert!(
            result.get("config").is_none(),
            "config blob must be removed"
        );

        // Index section has status field, no drift
        assert!(
            result["index"]["status"].is_string(),
            "index.status must be present"
        );
        assert!(
            result["index"].get("drift").is_none(),
            "drift must not appear in project_status"
        );

        // Libraries section still present
        assert!(result["libraries"].is_object(), "libraries section missing");
    }

    #[tokio::test]
    async fn project_status_includes_memory_staleness() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // Create memories dir and a memory file
        let memories_dir = dir.path().join(".codescout/memories");
        std::fs::create_dir_all(&memories_dir).unwrap();
        std::fs::write(memories_dir.join("architecture.md"), "# Arch").unwrap();

        // Create anchored file and sidecar
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/server.rs"), "fn main() {}").unwrap();

        let anchors =
            crate::memory::anchors::seed_anchors(dir.path(), "Uses `src/server.rs`.").unwrap();
        crate::memory::anchors::write_anchor_file(
            &memories_dir.join("architecture.anchors.toml"),
            &anchors,
        )
        .unwrap();

        // Before change — should be fresh
        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let staleness = &result["memory_staleness"];
        assert!(staleness["stale"].as_array().unwrap().is_empty());
        assert!(staleness["fresh"]
            .as_array()
            .unwrap()
            .contains(&json!("architecture")));

        // Modify the anchored file
        std::fs::write(dir.path().join("src/server.rs"), "fn changed() {}").unwrap();

        let result = ProjectStatus.call(json!({}), &ctx).await.unwrap();
        let staleness = &result["memory_staleness"];
        let stale = staleness["stale"].as_array().unwrap();
        assert_eq!(stale.len(), 1);
        assert_eq!(stale[0]["topic"], "architecture");
        assert!(stale[0]["changed_files"]
            .as_array()
            .unwrap()
            .contains(&json!("src/server.rs")));
    }

    #[tokio::test]
    async fn activate_includes_cwd_hint() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(None).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let input = json!({ "path": dir.path().to_str().unwrap() });
        let result = ActivateProject.call(input, &ctx).await.unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(
            hint.starts_with("CWD: "),
            "hint should start with CWD: but was: {hint}"
        );
        assert!(hint.contains(dir.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn activate_hint_shows_switched_when_away_from_home() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let input = json!({ "path": dir2.path().to_str().unwrap() });
        let result = ActivateProject.call(input, &ctx).await.unwrap();
        let hint = result["hint"].as_str().unwrap();
        // Non-home default is RO: "Browsing … (read-only). CWD: … — remember to workspace(action='activate', …)"
        assert!(
            hint.contains("remember to workspace"),
            "hint should warn to switch back: {hint}"
        );
        assert!(
            hint.contains(dir2.path().to_str().unwrap()),
            "should contain new path: {hint}"
        );
        assert!(
            hint.contains(dir1.path().to_str().unwrap()),
            "should contain home path: {hint}"
        );
    }

    #[tokio::test]
    async fn activate_hint_shows_returned_when_back_home() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        std::fs::create_dir_all(dir1.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(dir2.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir1.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        // Switch away
        ActivateProject
            .call(json!({ "path": dir2.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        // Return home
        let result = ActivateProject
            .call(json!({ "path": dir1.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(hint.contains("Returned to home project"), "hint: {hint}");
        assert!(hint.contains(dir1.path().to_str().unwrap()));
    }

    #[tokio::test]
    async fn project_status_shows_workspace_projects() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create multi-project structure
        std::fs::write(root.join("build.gradle.kts"), "").unwrap();
        let mcp = root.join("mcp-server");
        std::fs::create_dir_all(&mcp).unwrap();
        std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

        // Create workspace.toml
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
id = "mcp-server"
root = "mcp-server"
languages = ["typescript"]
depends_on = ["test"]
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
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = ProjectStatus
            .call(serde_json::json!({}), &ctx)
            .await
            .unwrap();
        let ws = result.get("workspace");
        assert!(
            ws.is_some(),
            "project_status should include workspace section"
        );
        let projects = ws.unwrap().get("projects").unwrap().as_array().unwrap();
        assert_eq!(projects.len(), 2);
    }

    #[tokio::test]
    async fn activate_project_switches_focus_by_id() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        // Create multi-project structure
        std::fs::write(root.join("build.gradle.kts"), "").unwrap();
        let mcp = root.join("mcp-server");
        std::fs::create_dir_all(&mcp).unwrap();
        std::fs::write(mcp.join("package.json"), r#"{"scripts":{"build":"tsc"}}"#).unwrap();

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
        };

        // Initially focused on root project
        let root_path = ctx.agent.require_project_root().await.unwrap();
        assert_eq!(root_path, root.to_path_buf());

        // Switch focus to mcp-server by ID
        let result = ActivateProject
            .call(serde_json::json!({"path": "mcp-server"}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");

        // Now focused on mcp-server
        let new_root = ctx.agent.require_project_root().await.unwrap();
        assert_eq!(new_root, root.join("mcp-server"));
    }

    #[tokio::test]
    async fn activate_project_unknown_id_with_no_slash_returns_error() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join("Cargo.toml"), "[package]\nname=\"test\"\n").unwrap();

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
        };

        // "unknown-project" has no slash and does not exist as a project ID or a path
        let result = ActivateProject
            .call(serde_json::json!({"path": "unknown-project"}), &ctx)
            .await;
        // Should fail: not a known project ID, and not a valid directory path
        assert!(
            result.is_err() || result.as_ref().unwrap().get("error").is_some(),
            "expected error or error field, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn post_compact_flushes_lsp_clients_and_returns_flushed() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let agent = Agent::new(Some(dir.path().to_path_buf())).await.unwrap();
        let ctx = ToolContext {
            agent,
            lsp: lsp(),
            output_buffer: Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // post_compact=true should return flushed:true without the normal status fields
        let result = ProjectStatus
            .call(json!({"post_compact": true}), &ctx)
            .await
            .unwrap();
        assert_eq!(result["flushed"], json!(true), "expected flushed:true");
        assert!(result["hint"].is_string(), "expected hint string");
        // Normal status fields must NOT be present in the compact-flush response
        assert!(
            result.get("project_root").is_none(),
            "post_compact response must not include project_root"
        );

        // post_compact=false (or absent) should return the normal status response
        let result = ProjectStatus
            .call(json!({"post_compact": false}), &ctx)
            .await
            .unwrap();
        assert!(
            result["project_root"].is_string(),
            "normal call must include project_root"
        );
    }

    #[test]
    fn format_activate_project_rw_compact() {
        let result = json!({
            "status": "ok",
            "project": "my-project",
            "project_root": "/home/user/my-project",
            "read_only": false,
            "memories": ["arch", "conventions", "gotchas"],
            "index": {"status": "not_indexed"},
            "hint": "CWD: /home/user/my-project"
        });
        let compact = format_activate_project(&result);
        assert_eq!(
            compact,
            "activated · my-project (rw) · 3 memories · index: not_indexed"
        );
    }

    #[test]
    fn format_activate_project_ro_with_workspace() {
        let result = json!({
            "status": "ok",
            "project": "sub-lib",
            "project_root": "/home/user/mono/sub-lib",
            "read_only": true,
            "memories": [],
            "index": {"status": "indexed"},
            "workspace": [
                {"id": "main", "root": ".", "languages": ["rust"]},
                {"id": "sub-lib", "root": "libs/sub-lib", "languages": ["rust"]},
            ],
            "hint": "Browsing sub-lib (read-only)."
        });
        let compact = format_activate_project(&result);
        assert_eq!(
            compact,
            "activated · sub-lib (ro) · 0 memories · index: indexed · 2 workspace projects"
        );
    }

    #[test]
    fn format_activate_project_with_auto_libs() {
        let result = json!({
            "status": "ok",
            "project": "web",
            "project_root": "/home/user/web",
            "read_only": false,
            "memories": ["arch"],
            "index": {"status": "not_indexed"},
            "auto_registered_libs": {"count": 12, "without_source": 3},
            "hint": "CWD: ..."
        });
        let compact = format_activate_project(&result);
        assert_eq!(compact, "activated · web (rw) · 1 memories · index: not_indexed · auto-registered 12 libs (3 without source)");
    }

    #[test]
    fn format_activate_project_auto_libs_all_with_source() {
        let result = json!({
            "status": "ok",
            "project": "app",
            "project_root": "/home/user/app",
            "read_only": false,
            "memories": [],
            "index": {"status": "indexed"},
            "auto_registered_libs": {"count": 5, "without_source": 0},
            "hint": "CWD: ..."
        });
        let compact = format_activate_project(&result);
        assert_eq!(
            compact,
            "activated · app (rw) · 0 memories · index: indexed · auto-registered 5 libs"
        );
    }

    #[tokio::test]
    async fn activate_project_rw_includes_security_fields() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(
                json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");
        assert!(
            result["security_profile"].is_string(),
            "RW should include security_profile"
        );
        assert!(
            !result["shell_enabled"].is_null(),
            "RW should include shell_enabled"
        );
    }

    #[tokio::test]
    async fn activate_project_ro_excludes_security_fields() {
        let home = tempdir().unwrap();
        let other = tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(other.path().join(".codescout")).unwrap();
        // Start with a home project (always RW)
        let ctx = ToolContext {
            agent: Agent::new(Some(home.path().to_path_buf())).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        // Now activate another project as RO
        let result = ActivateProject
            .call(
                json!({"path": other.path().to_str().unwrap(), "read_only": true}),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");
        assert!(
            result["security_profile"].is_null(),
            "RO should not include security_profile"
        );
        assert!(
            result["shell_enabled"].is_null(),
            "RO should not include shell_enabled"
        );
    }

    #[tokio::test]
    async fn activate_project_includes_memories_and_index() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        assert!(
            result["memories"].is_array(),
            "should include memories array"
        );
        assert!(result["index"].is_object(), "should include index object");
        assert!(
            result["index"]["status"].is_string(),
            "index should have status"
        );
    }

    #[tokio::test]
    async fn activate_project_rw_hint_promotes_project_status() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(
                json!({"path": dir.path().to_str().unwrap(), "read_only": false}),
                &ctx,
            )
            .await
            .unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(
            hint.contains("workspace(action='status')"),
            "RW hint should promote workspace status, got: {hint}"
        );
    }

    #[tokio::test]
    async fn activate_project_single_project_no_workspace() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        assert!(
            result["workspace"].is_null(),
            "single-project should have null workspace"
        );
    }

    #[tokio::test]
    async fn activate_project_focus_switch_returns_full_response() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        // Create a sub-project
        let sub = root.join("packages").join("api");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            sub.join("package.json"),
            r#"{"name":"api","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();

        let ctx = ToolContext {
            agent: Agent::new(Some(root)).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // Focus-switch by ID
        let result = ActivateProject
            .call(json!({"path": "api"}), &ctx)
            .await
            .unwrap();

        assert_eq!(result["status"], "ok");
        assert!(result["project"].is_string(), "should have project name");
        assert!(result["languages"].is_array(), "should have languages");
        assert!(result["memories"].is_array(), "should have memories");
        assert!(result["index"].is_object(), "should have index");
        assert!(!result["read_only"].is_null(), "should have read_only");
    }

    #[tokio::test]
    async fn activate_project_workspace_includes_depends_on() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let sub_a = root.join("packages").join("core");
        let sub_b = root.join("packages").join("web");
        std::fs::create_dir_all(&sub_a).unwrap();
        std::fs::create_dir_all(&sub_b).unwrap();
        std::fs::write(
            sub_a.join("package.json"),
            r#"{"name":"core","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();
        std::fs::write(
            sub_b.join("package.json"),
            r#"{"name":"web","scripts":{"build":"tsc"}}"#,
        )
        .unwrap();

        let ctx = ToolContext {
            agent: Agent::new(Some(root)).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        let result = ActivateProject
            .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
            .await
            .unwrap();

        if let Some(ws) = result["workspace"].as_array() {
            for entry in ws {
                assert!(
                    entry["depends_on"].is_array(),
                    "each workspace entry should have depends_on"
                );
            }
        }
    }

    #[tokio::test]
    async fn activate_project_ro_hint_warns_switch_back() {
        let home = tempdir().unwrap();
        let other = tempdir().unwrap();
        std::fs::create_dir_all(home.path().join(".codescout")).unwrap();
        std::fs::create_dir_all(other.path().join(".codescout")).unwrap();

        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };

        // Activate home first
        ActivateProject
            .call(json!({"path": home.path().to_str().unwrap()}), &ctx)
            .await
            .unwrap();

        // Activate other as RO
        let result = ActivateProject
            .call(
                json!({"path": other.path().to_str().unwrap(), "read_only": true}),
                &ctx,
            )
            .await
            .unwrap();

        let hint = result["hint"].as_str().unwrap();
        assert!(
            hint.contains("remember to workspace"),
            "RO hint should warn about switching back, got: {hint}"
        );
        assert!(
            hint.contains("read-only"),
            "RO hint should mention read-only, got: {hint}"
        );
    }

    #[test]
    fn activate_project_auto_libs_is_summary_not_array() {
        let result = json!({
            "status": "ok",
            "project": "test",
            "project_root": "/tmp/test",
            "read_only": false,
            "memories": [],
            "index": {"status": "not_indexed"},
            "auto_registered_libs": {"count": 5, "without_source": 2},
        });
        assert!(result["auto_registered_libs"].is_object());
        assert_eq!(result["auto_registered_libs"]["count"], 5);
        assert_eq!(result["auto_registered_libs"]["without_source"], 2);
    }

    #[tokio::test]
    async fn activate_project_memories_graceful_on_error() {
        // A project with no .codescout dir should still activate with memories: []
        let dir = tempdir().unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = ActivateProject
            .call(json!({"path": dir.path().to_str().unwrap()}), &ctx)
            .await
            .unwrap();
        let memories = result["memories"].as_array().unwrap();
        assert!(
            memories.is_empty(),
            "empty project should have empty memories array"
        );
    }

    #[tokio::test]
    async fn workspace_action_activate_dispatches_to_activate_project() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let result = Workspace
            .call(
                json!({
                    "action": "activate",
                    "path": dir.path().to_str().unwrap(),
                    "read_only": false,
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert_eq!(result["status"], "ok");
        assert!(result.get("project_hints").is_some());
    }

    #[tokio::test]
    async fn workspace_action_status_dispatches_to_project_status() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        ActivateProject
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let result = Workspace
            .call(json!({ "action": "status" }), &ctx)
            .await
            .unwrap();
        assert!(result["project_root"].is_string());
        assert!(result["languages"].is_array());
        assert!(result["index"].is_object());
    }

    #[tokio::test]
    async fn workspace_action_list_projects_returns_workspace_field() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        ActivateProject
            .call(json!({ "path": dir.path().to_str().unwrap() }), &ctx)
            .await
            .unwrap();
        let result = Workspace
            .call(json!({ "action": "list_projects" }), &ctx)
            .await
            .unwrap();
        // The result must contain the "workspace" key (value may be null when no
        // workspace.toml is present — that's still a successful list_projects call).
        assert!(result.as_object().unwrap().contains_key("workspace"));
        // And no other fields should leak through.
        assert_eq!(result.as_object().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn workspace_action_unknown_errors() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
            peer: None,
            section_coverage: std::sync::Arc::new(std::sync::Mutex::new(
                crate::tools::section_coverage::SectionCoverage::new(),
            )),
        };
        let err = Workspace
            .call(json!({ "action": "wat" }), &ctx)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("unknown workspace action"),
            "expected unknown action error, got: {err}"
        );
    }
}
