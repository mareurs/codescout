//! Configuration and project management tools.

use super::{Tool, ToolContext};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ActivateProject;
pub struct ProjectStatus;

#[async_trait::async_trait]
impl Tool for ActivateProject {
    fn name(&self) -> &str {
        "activate_project"
    }
    fn description(&self) -> &str {
        "Switch the active project to the given path. All subsequent tool calls \
         operate relative to this project root."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["path"],
            "properties": {
                "path": { "type": "string", "description": "Absolute path to the project root" }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let path = super::require_str_param(&input, "path")?;
        let root = PathBuf::from(path);
        if !root.is_dir() {
            return Err(super::RecoverableError::with_hint(
                format!("path '{}' is not a directory", path),
                "Provide an absolute path to an existing directory.",
            )
            .into());
        }
        // Capture before activate() — activate sets home_root on first call
        let had_home = ctx.agent.home_root().await.is_some();

        ctx.agent.activate(root).await?;

        let config = ctx
            .agent
            .with_project(|p| {
                Ok(json!({
                    "project_root": p.root.display().to_string(),
                    "config": serde_json::to_value(&p.config)?,
                }))
            })
            .await?;

        // Build CWD hint
        let project_root_str = config["project_root"].as_str().unwrap_or("?");
        let is_home = ctx.agent.is_home().await;
        let home = ctx.agent.home_root().await;

        let hint = if !had_home {
            format!("CWD: {}", project_root_str)
        } else if is_home {
            format!("Returned to original project. CWD: {}", project_root_str)
        } else {
            format!(
                "Switched project. CWD: {} (home: {})",
                project_root_str,
                home.as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default()
            )
        };

        Ok(json!({ "status": "ok", "activated": config, "hint": hint }))
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
        "Active project state: config, semantic index health, usage telemetry, and library summary. \
         Pass threshold (float) to include drift scores. Pass window ('1h','24h','7d','30d') for usage window."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "threshold": {
                    "type": "number",
                    "description": "Min avg_drift to include (0.0-1.0). When provided, adds drift data to index section."
                },
                "path": {
                    "type": "string",
                    "description": "Glob pattern to filter drift files (SQL LIKE syntax, e.g. 'src/tools/%')."
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["exploring", "full"],
                    "description": "Drift output detail. 'full' includes most-drifted chunk content."
                }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        // --- Config + library section ---
        let (root, config_val, lib_count, lib_indexed) = ctx
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
                    serde_json::to_value(&p.config)?,
                    lib_count,
                    lib_indexed,
                ))
            })
            .await?;

        let mut result = json!({
            "project_root": root.display().to_string(),
            "config": config_val,
            "libraries": { "count": lib_count, "indexed": lib_indexed },
        });

        // --- Index section ---
        let db_path = crate::embed::index::db_path(&root);
        if !db_path.exists() {
            result["index"] = json!({ "indexed": false });
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
                    let mut index_section = json!({
                        "indexed": true,
                        "files": stats.file_count,
                        "chunks": stats.chunk_count,
                        "last_updated": stats.indexed_at,
                        "model": stats.model,
                    });
                    if let Some(s) = staleness {
                        index_section["git_sync"] = if s.stale {
                            json!({
                                "status": "behind",
                                "behind_commits": s.behind_commits,
                                "note": "Recent commits are not yet indexed. All previously indexed code is still queryable."
                            })
                        } else {
                            json!({ "status": "up_to_date" })
                        };
                    }

                    // Drift — only if threshold or path param provided
                    let wants_drift =
                        input.get("threshold").is_some() || input.get("path").is_some();
                    if wants_drift {
                        use crate::tools::output::OutputGuard;
                        let (root3, drift_enabled) = ctx
                            .agent
                            .with_project(|p| {
                                Ok((p.root.clone(), p.config.embeddings.drift_detection_enabled))
                            })
                            .await?;
                        if !drift_enabled {
                            index_section["drift"] = json!({
                                "status": "disabled",
                                "hint": "Set embeddings.drift_detection_enabled = true in .codescout/project.toml"
                            });
                        } else {
                            let threshold =
                                input["threshold"].as_f64().map(|v| v as f32).unwrap_or(0.1);
                            let path_filter = input["path"].as_str().map(|s| s.to_string());
                            let guard = OutputGuard::from_input(&input);
                            let rows = tokio::task::spawn_blocking(move || {
                                let conn = crate::embed::index::open_db(&root3)?;
                                crate::embed::index::query_drift_report(
                                    &conn,
                                    Some(threshold),
                                    path_filter.as_deref(),
                                )
                            })
                            .await??;
                            let items: Vec<Value> = rows
                                .iter()
                                .map(|r| {
                                    let mut obj = serde_json::Map::new();
                                    obj.insert("file_path".into(), json!(r.file_path));
                                    obj.insert("avg_drift".into(), json!(r.avg_drift));
                                    obj.insert("max_drift".into(), json!(r.max_drift));
                                    if guard.should_include_body() {
                                        if let Some(chunk) = &r.max_drift_chunk {
                                            obj.insert("max_drift_chunk".into(), json!(chunk));
                                        }
                                    }
                                    Value::Object(obj)
                                })
                                .collect();
                            let (items, overflow) = guard.cap_items(
                                items,
                                "Use detail_level='full' with offset for pagination",
                            );
                            let total = overflow.as_ref().map_or(items.len(), |o| o.total);
                            let mut drift_result = json!({ "results": items, "total": total });
                            if let Some(ov) = overflow {
                                drift_result["overflow"] = OutputGuard::overflow_json(&ov);
                            }
                            index_section["drift"] = drift_result;
                        }
                    }

                    result["index"] = index_section;
                }
                _ => {
                    result["index"] = json!({ "indexed": false, "error": "failed to read index" });
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

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_project_status(result))
    }
}

fn format_activate_project(result: &Value) -> String {
    let root = result["activated"]["project_root"]
        .as_str()
        .or_else(|| result["path"].as_str())
        .unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    if let Some(hint) = result["hint"].as_str() {
        format!("activated · {name} · {hint}")
    } else {
        format!("activated · {name}")
    }
}

fn format_project_status(result: &Value) -> String {
    let root = result["project_root"].as_str().unwrap_or("?");
    let name = std::path::Path::new(root)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(root);
    let indexed = result["index"]["indexed"].as_bool().unwrap_or(false);
    let index_str = if indexed {
        let files = result["index"]["files"].as_u64().unwrap_or(0);
        let chunks = result["index"]["chunks"].as_u64().unwrap_or(0);
        format!("index:{files}f/{chunks}c")
    } else {
        "index:none".to_string()
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
        assert!(status["project_root"].as_str().unwrap().len() > 0);
        assert!(status["config"]["project"]["name"].is_string());
    }

    #[tokio::test]
    async fn activate_nonexistent_path_errors() {
        let ctx = ToolContext {
            agent: Agent::new(None).await.unwrap(),
            lsp: lsp(),
            output_buffer: std::sync::Arc::new(crate::tools::output_buffer::OutputBuffer::new(20)),
            progress: None,
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
        };
        let tool = ProjectStatus;
        let result = tool.call(json!({}), &ctx).await.unwrap();
        assert!(result["project_root"].is_string(), "missing project_root");
        assert!(result["config"].is_object(), "missing config section");
        assert!(result.get("index").is_some(), "missing index section");
        assert!(
            result.get("libraries").is_some(),
            "missing libraries section"
        );
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
        };
        let input = json!({ "path": dir2.path().to_str().unwrap() });
        let result = ActivateProject.call(input, &ctx).await.unwrap();
        let hint = result["hint"].as_str().unwrap();
        assert!(hint.starts_with("Switched project."), "hint: {hint}");
        assert!(
            hint.contains(dir2.path().to_str().unwrap()),
            "should contain new path"
        );
        assert!(
            hint.contains(dir1.path().to_str().unwrap()),
            "should contain home path"
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
        assert!(
            hint.starts_with("Returned to original project."),
            "hint: {hint}"
        );
        assert!(hint.contains(dir1.path().to_str().unwrap()));
    }
}
