//! Indexing tools: IndexProject, IndexStatus, Index.

use super::super::{optional_f64_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

pub struct IndexProject;
pub struct IndexStatus;
pub struct Index;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
struct IndexConfirm {
    /// Confirm indexing this directory
    confirm: bool,
}
rmcp::elicit_safe!(IndexConfirm);

#[async_trait::async_trait]
impl Tool for IndexProject {
    fn name(&self) -> &str {
        "index_project"
    }

    fn is_write(&self, _input: &Value) -> bool {
        true
    }

    fn description(&self) -> &str {
        "Build or incrementally update the semantic search index for the active project. \
         Use scope='lib:<name>' to index a registered library (replaces index_library)."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "force": { "type": "boolean", "default": false,
                    "description": "Force full reindex, ignoring cached file hashes" },
                "scope": {
                    "type": "string",
                    "default": "project",
                    "description": "Scope to index: 'project' (default) to index the active project, or 'lib:<name>' to index a registered library. Replaces index_library."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::agent::IndexingState;

        let scope_str = input["scope"].as_str().unwrap_or("project");

        // Library scope: delegate to library indexing logic (replaces index_library tool)
        if let Some(lib_name) = scope_str.strip_prefix("lib:") {
            let force = parse_bool_param(&input["force"]);

            // Guard against concurrent runs — mirror the project-scope branch
            // so two concurrent `index_project(scope="lib:foo")` calls (or a
            // lib + project call together) don't race on the shared
            // `libraries.json` rewrite or the sqlite busy-timeout.
            {
                let mut state = ctx.agent.indexing.lock().unwrap_or_else(|e| e.into_inner());
                if matches!(*state, IndexingState::Running { .. }) {
                    return Ok(json!({
                        "status": "already_running",
                        "hint": "Use index(action='status') to check progress.",
                    }));
                }
                *state = IndexingState::Running {
                    done: 0,
                    total: 0,
                    eta_secs: None,
                };
            }

            // Ensure we always reset indexing state on every exit path from
            // the lib-scope branch — success, error, or early return.
            struct StateGuard {
                indexing: std::sync::Arc<std::sync::Mutex<IndexingState>>,
                active: bool,
            }
            impl Drop for StateGuard {
                fn drop(&mut self) {
                    if self.active {
                        let mut s = self.indexing.lock().unwrap_or_else(|e| e.into_inner());
                        if matches!(*s, IndexingState::Running { .. }) {
                            *s = IndexingState::Idle;
                        }
                    }
                }
            }
            let _state_guard = StateGuard {
                indexing: ctx.agent.indexing.clone(),
                active: true,
            };

            let (root, lib_path) = {
                let inner = ctx.agent.inner.read().await;
                let project = inner.active_project().ok_or_else(|| {
                    crate::tools::RecoverableError::with_hint(
                        "No active project. Use workspace(action='activate') first.",
                        "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                    )
                })?;
                let entry = project.library_registry.lookup(lib_name).ok_or_else(|| {
                    crate::tools::RecoverableError::with_hint(
                        format!("Library '{}' not found in registry.", lib_name),
                        "Use library(action='list') to see registered libraries.",
                    )
                })?;
                if !entry.source_available {
                    return Err(crate::tools::RecoverableError::with_hint(
                        format!(
                            "Library '{}' source code is not available locally.",
                            lib_name
                        ),
                        "Download sources using the project's build tool, then call \
                         library(action='register', path=\"/path/to/source\", name, language) and retry.",
                    )
                    .into());
                }
                (project.root.clone(), entry.path.clone())
            };

            let source = format!("lib:{}", lib_name);
            crate::embed::index::build_library_index(&root, &lib_path, &source, force).await?;

            // Read current version from lockfile and write back
            let versions = crate::library::versions::resolve_dependency_versions(&root);
            let current_version = crate::library::versions::find_version(&versions, lib_name);
            if current_version.is_none() {
                tracing::debug!(
                    "version tracking not available for library '{}' — unsupported lockfile ecosystem",
                    lib_name
                );
            }

            {
                let mut inner = ctx.agent.inner.write().await;
                let project = inner.active_project_mut().ok_or_else(|| {
                    crate::tools::RecoverableError::with_hint(
                        "No active project. Use workspace(action='activate') first.",
                        "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                    )
                })?;
                if let Some(entry) = project.library_registry.lookup_mut(lib_name) {
                    entry.indexed = true;
                    if let Some(ver) = &current_version {
                        entry.version = Some(ver.clone());
                        entry.version_indexed = Some(ver.clone());
                        entry.nudge_dismissed = false;
                    }
                }
                let registry_path = project.root.join(".codescout").join("libraries.json");
                project.library_registry.save(&registry_path)?;
            }

            // Write version_indexed to lib_meta table
            if let Some(ver) = &current_version {
                let ver2 = ver.clone();
                let root2 = root.clone();
                let lib_name2 = lib_name.to_string();
                tokio::task::spawn_blocking(move || {
                    let lib_conn =
                        crate::embed::index::open_lib_db(&root2, &lib_name2)?;
                    lib_conn.execute(
                        "INSERT OR REPLACE INTO lib_meta (key, value) VALUES ('version_indexed', ?)",
                        rusqlite::params![ver2],
                    )?;
                    anyhow::Ok(())
                })
                .await??;
            }

            let source2 = source.clone();
            let root2 = root.clone();
            let (file_count, chunk_count) = tokio::task::spawn_blocking(move || {
                let conn = crate::embed::index::open_db(&root2)?;
                let by_source = crate::embed::index::index_stats_by_source(&conn)?;
                let lib_stats = by_source.get(&source2);
                anyhow::Ok((
                    lib_stats.map_or(0, |s| s.file_count),
                    lib_stats.map_or(0, |s| s.chunk_count),
                ))
            })
            .await??;

            return Ok(json!({
                "status": "ok",
                "library": lib_name,
                "source": source,
                "files_indexed": file_count,
                "chunks": chunk_count,
            }));
        }

        let force = parse_bool_param(&input["force"]);
        let root = ctx.agent.require_project_root().await?;

        // ── Preflight scope check ───────────────────────────────────────
        // Stat-walk the root to estimate size + detect broad roots (home, system).
        // Requires user confirmation via elicitation if either trigger fires.
        // Runs BEFORE the concurrent-run guard so that a declined or unavailable
        // elicitation never leaves IndexingState stuck in Running.
        {
            use crate::embed::preflight::{check_index_scope, PreflightVerdict};

            let (max_bytes, ignored) = {
                let inner = ctx.agent.inner.read().await;
                let project = inner.active_project();
                let max_bytes = project
                    .map(|p| p.config.security.max_index_bytes)
                    .unwrap_or(500 * 1024 * 1024);
                let ignored = project
                    .map(|p| p.config.ignored_paths.patterns.clone())
                    .unwrap_or_default();
                (max_bytes, ignored)
            };
            let preflight_root = root.clone();
            let verdict = tokio::task::spawn_blocking(move || {
                check_index_scope(&preflight_root, max_bytes, &ignored)
            })
            .await
            .map_err(|e| anyhow::anyhow!("preflight task join error: {e}"))??;

            if let PreflightVerdict::RequiresConfirmation(info) = verdict {
                tracing::info!(
                    root = ?info.root,
                    file_count = info.file_count,
                    approx_bytes = info.approx_bytes,
                    suspicious = ?info.suspicious_reason,
                    size_over = info.size_exceeds_threshold,
                    "index_project preflight requires confirmation"
                );
                let msg = info.elicitation_message();
                match ctx.elicit::<IndexConfirm>(msg).await? {
                    Some(IndexConfirm { confirm: true }) => {
                        tracing::info!(root = ?info.root, "index scope confirmed by user");
                    }
                    Some(IndexConfirm { confirm: false }) => {
                        return Err(crate::tools::RecoverableError::with_hint(
                            "Indexing aborted — user did not confirm the scope",
                            "Activate a more specific project root, or raise \
                             security.max_index_bytes in .codescout/project.toml, then retry.",
                        )
                        .into());
                    }
                    None => {
                        // No peer, client lacks elicitation capability, or no content returned.
                        // For this guard, the safe default is to refuse — never silently proceed.
                        return Err(crate::tools::RecoverableError::with_hint(
                            "index_project needs confirmation but client does not support elicitation",
                            "Raise security.max_index_bytes in .codescout/project.toml, \
                             or activate a narrower project root, then retry.",
                        )
                        .into());
                    }
                }
            }
        }
        // ────────────────────────────────────────────────────────────────

        // Guard against concurrent runs.
        {
            let mut state = ctx.agent.indexing.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(*state, IndexingState::Running { .. }) {
                return Ok(json!({
                    "status": "already_running",
                    "hint": "Use index(action='status') to check progress."
                }));
            }
            *state = IndexingState::Running {
                done: 0,
                total: 0,
                eta_secs: None,
            };
        }

        let state_arc = ctx.agent.indexing.clone();
        let progress = ctx.progress.clone();
        // Progress notifications from background tasks crash Claude Code 2.x
        // (it closes the stdin pipe on receiving unsolicited notifications/progress).
        // Disable until Claude Code supports MCP progress properly.
        // See BUG-038 in docs/TODO-tool-misbehaviors.md.
        //
        // if let Some(p) = &progress {
        //     p.report(0, None).await;
        //     p.report_text("indexing project").await;
        // }

        // Separate clone: progress_cb captures this; state_arc is used after build_index returns.
        let state_arc_cb = ctx.agent.indexing.clone();
        let progress_cb_progress = progress.clone();
        let progress_cb: Option<crate::embed::index::ProgressCb> =
            Some(Box::new(move |done, total, eta_secs| {
                {
                    let mut s = state_arc_cb.lock().unwrap_or_else(|e| e.into_inner());
                    *s = IndexingState::Running {
                        done,
                        total,
                        eta_secs,
                    };
                }
                // Fire MCP progress notification from within this sync callback.
                if let Some(p) = progress_cb_progress.clone() {
                    tokio::spawn(async move {
                        p.report(
                            done as u32,
                            if total > 0 { Some(total as u32) } else { None },
                        )
                        .await;
                    });
                }
            }));

        // Capture the dirty-files Arc before spawning so the task can clear it on success.
        let dirty_files_arc = ctx.agent.dirty_files_arc().await;

        tokio::spawn(async move {
            // Progress callback disabled — see comment above re: Claude Code crash.
            let _progress_cb = progress_cb;
            let result = crate::embed::index::build_index(&root, force, None).await;

            // Gather post-index stats *before* locking the mutex so that a
            // MutexGuard (which is !Send) is never held across an await point.
            let stats = if result.is_ok() {
                tokio::task::spawn_blocking({
                    let root = root.clone();
                    move || {
                        crate::embed::index::open_db(&root)
                            .and_then(|conn| crate::embed::index::index_stats(&conn))
                            .map(|s| (s.file_count, s.chunk_count))
                            .unwrap_or((0, 0))
                    }
                })
                .await
                .unwrap_or((0, 0))
            } else {
                (0, 0)
            };

            {
                // Drop the MutexGuard before any `.await` — MutexGuard is !Send.
                let mut state = state_arc.lock().unwrap_or_else(|e| e.into_inner());
                *state = match result {
                    Ok(report) => {
                        // Indexing succeeded — files are now fresh, clear the dirty set.
                        if let Some(ref arc) = dirty_files_arc {
                            arc.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        }
                        IndexingState::Done {
                            files_indexed: report.indexed,
                            files_deleted: report.deleted,
                            detail: report.skipped_msg,
                            total_files: stats.0,
                            total_chunks: stats.1,
                        }
                    }
                    Err(e) => IndexingState::Failed(e.to_string()),
                };
            }
            // Completion progress disabled — see BUG-038.
            // if let Some(p) = &progress {
            //     p.report_text("indexing complete").await;
            //     p.report(1, Some(1)).await;
            // }
        });

        Ok(json!({
            "status": "started",
            "hint": "Indexing is running in the background. Use index(action='status') to check when complete."
        }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_index_project(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresEmbeddings
    }
}

#[async_trait::async_trait]
impl Tool for IndexStatus {
    fn name(&self) -> &str {
        "index_status"
    }
    fn description(&self) -> &str {
        "Show index stats: file count, chunk count, model, last update. \
         Optionally query semantic drift scores when threshold or path is provided."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "threshold": {
                    "type": "number",
                    "description": "Minimum avg_drift to include (default: 0.1). Range 0.0-1.0. When provided, includes drift data in response."
                },
                "path": {
                    "type": "string",
                    "description": "Glob pattern to filter drift files (e.g. 'src/tools/%'). Uses SQL LIKE syntax."
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["exploring", "full"],
                    "description": "Output detail for drift: 'exploring' (default) shows scores only, 'full' includes most-drifted chunk content."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let (root, model, drift_enabled) = {
            let inner = ctx.agent.inner.read().await;
            let p = inner.active_project().ok_or_else(|| {
                crate::tools::RecoverableError::with_hint(
                    "No active project. Use workspace(action='activate') first.",
                    "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                )
            })?;
            (
                p.root.clone(),
                p.config.embeddings.model.clone(),
                p.config.embeddings.drift_detection_enabled,
            )
        };

        let db_path = crate::embed::index::project_db_path(&root);
        if !db_path.exists() {
            return Ok(json!({
                "indexed": false,
                "message": "No index found. Run index(action='build') first.",
            }));
        }

        // Sync SQLite off async runtime
        let db_path_str = db_path.display().to_string();
        let root2 = root.clone();
        let (stats, by_source, staleness, last_commit) = tokio::task::spawn_blocking(move || {
            let conn = crate::embed::index::open_db(&root2)?;
            let stats = crate::embed::index::index_stats(&conn)?;
            let by_source = crate::embed::index::index_stats_by_source(&conn)?;
            let staleness = crate::embed::index::check_index_staleness(&conn, &root2).ok();
            let last_commit = crate::embed::index::get_last_indexed_commit(&conn)
                .ok()
                .flatten();
            anyhow::Ok((stats, by_source, staleness, last_commit))
        })
        .await??;

        let by_source_json: serde_json::Map<String, Value> = by_source
            .iter()
            .map(|(source, ss)| {
                (
                    source.clone(),
                    json!({ "files": ss.file_count, "chunks": ss.chunk_count }),
                )
            })
            .collect();

        // Build base response
        let mut result = json!({
            "indexed": true,
            "queryable": true,
            "configured_model": model,
            "indexed_with_model": stats.model,
            "indexed_at": stats.indexed_at,
            "file_count": stats.file_count,
            "chunk_count": stats.chunk_count,
            "embedding_count": stats.embedding_count,
            "db_path": db_path_str,
            "by_source": by_source_json,
        });

        // Git-sync info — framed as informational, not a quality signal
        if let Some(staleness) = staleness {
            result["git_sync"] = if staleness.stale {
                json!({
                    "status": "behind",
                    "behind_commits": staleness.behind_commits,
                    "note": "Recent commits are not yet indexed. All previously indexed code is still queryable — run index(action='build') to include new code."
                })
            } else {
                json!({ "status": "up_to_date" })
            };
            if let Some(commit) = last_commit {
                result["last_indexed_commit"] = json!(commit);
            }
        }

        // Include drift data when threshold or path is provided
        let wants_drift = input.get("threshold").is_some() || input.get("path").is_some();
        if wants_drift {
            use crate::tools::output::OutputGuard;

            if !drift_enabled {
                result["drift"] = json!({
                    "status": "disabled",
                    "hint": "Drift detection is opted out. Re-enable it in .codescout/project.toml:\n[embeddings]\ndrift_detection_enabled = true"
                });
            } else {
                let threshold = optional_f64_param(&input, "threshold")
                    .map(|v| v as f32)
                    .unwrap_or(0.1);
                let path_filter = input["path"].as_str().map(|s| s.to_string());
                let guard = OutputGuard::from_input(&input);

                // Sync SQLite off async runtime
                let root3 = root.clone();
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
                        let mut obj = json!({
                            "file_path": r.file_path,
                            "avg_drift": r.avg_drift,
                            "max_drift": r.max_drift,
                            "chunks_added": r.chunks_added,
                            "chunks_removed": r.chunks_removed,
                        });
                        if guard.should_include_body() {
                            if let Some(chunk) = &r.max_drift_chunk {
                                obj["max_drift_chunk"] = json!(chunk);
                            }
                        }
                        obj
                    })
                    .collect();

                let (items, overflow) =
                    guard.cap_items(items, "Use detail_level='full' with offset for pagination");
                let total = overflow.as_ref().map_or(items.len(), |o| o.total);
                let mut drift_result = json!({
                    "note": "Drift scores are informational — they do not affect query results. High drift means code has changed since indexing; run index(action='build') to update.",
                    "results": items,
                    "total": total,
                });
                if let Some(ov) = overflow {
                    drift_result["overflow"] = OutputGuard::overflow_json(&ov);
                }
                result["drift"] = drift_result;
            }
        }

        // Append background indexing state if not idle.
        {
            use crate::agent::IndexingState;
            let state = ctx.agent.indexing.lock().unwrap_or_else(|e| e.into_inner());
            match &*state {
                IndexingState::Idle => {}
                IndexingState::Running {
                    done,
                    total,
                    eta_secs,
                } => {
                    result["indexing"] = json!({
                        "status": "running",
                        "done": done,
                        "total": total,
                        "eta_secs": eta_secs,
                    });
                }
                IndexingState::Done {
                    files_indexed,
                    files_deleted,
                    detail,
                    total_files,
                    total_chunks,
                } => {
                    result["indexing"] = json!({
                        "status": "done",
                        "files_indexed": files_indexed,
                        "files_deleted": files_deleted,
                        "detail": detail,
                        "total_files": total_files,
                        "total_chunks": total_chunks,
                    });
                }
                IndexingState::Failed(e) => {
                    result["indexing"] = json!({ "status": "failed", "error": e });
                }
            }
        }

        // Append per-library indexing states (non-idle only)
        let lib_states = ctx.agent.library_states_summary();
        if !lib_states.is_empty() {
            result["libraries"] = serde_json::to_value(&lib_states)?;
        }

        Ok(result)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_index_status(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresEmbeddings
    }
}

#[async_trait::async_trait]
impl Tool for Index {
    fn name(&self) -> &str {
        "index"
    }

    fn is_write(&self, input: &Value) -> bool {
        input.get("action").and_then(Value::as_str) == Some("build")
    }

    fn description(&self) -> &str {
        "Semantic index operations. Actions: \
         `build` (build/update the project's semantic index; pass `scope='lib:<name>'` to index a registered library), \
         `status` (show index stats and optional drift scores)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["build", "status"],
                    "description": "Operation to perform."
                },
                "force": {
                    "type": "boolean",
                    "default": false,
                    "description": "For action='build': force full reindex, ignoring cached file hashes."
                },
                "scope": {
                    "type": "string",
                    "default": "project",
                    "description": "For action='build': 'project' (default) or 'lib:<name>' to index a registered library."
                },
                "threshold": {
                    "type": "number",
                    "description": "For action='status': minimum avg_drift to include (range 0.0-1.0). When provided, includes drift data."
                },
                "path": {
                    "type": "string",
                    "description": "For action='status': glob pattern to filter drift files (SQL LIKE syntax)."
                },
                "detail_level": {
                    "type": "string",
                    "enum": ["exploring", "full"],
                    "description": "For action='status': output detail for drift entries."
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
                crate::tools::RecoverableError::with_hint(
                    "index requires 'action' parameter",
                    "Pass action='build' or action='status'.",
                )
            })?;
        match action {
            "build" => IndexProject.call(input, ctx).await,
            "status" => IndexStatus.call(input, ctx).await,
            other => Err(crate::tools::RecoverableError::with_hint(
                format!("unknown index action: {}", other),
                "Valid actions: 'build', 'status'.",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result.get("indexed").is_some() || result.get("file_count").is_some() {
            IndexStatus.format_compact(result)
        } else {
            IndexProject.format_compact(result)
        }
    }

    fn availability(&self, caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        IndexProject.availability(caps)
    }
}

fn format_index_project(result: &Value) -> String {
    let status = result["status"].as_str().unwrap_or("?");
    format!("index {status}")
}
pub(crate) fn format_index_status(result: &Value) -> String {
    let indexed = result["indexed"].as_bool().unwrap_or(false);
    if !indexed {
        return "not indexed".to_string();
    }
    let files = result["file_count"].as_u64().unwrap_or(0);
    let chunks = result["chunk_count"].as_u64().unwrap_or(0);

    let mut out = format!("good · queryable · {files} files · {chunks} chunks");

    if let Some(model) = result["indexed_with_model"].as_str() {
        out.push_str(&format!(" · {model}"));
    }
    if let Some(ts) = result["indexed_at"].as_str() {
        out.push_str(&format!(" · {ts}"));
    }
    if result["git_sync"]["status"].as_str() == Some("behind") {
        if let Some(behind) = result["git_sync"]["behind_commits"]
            .as_u64()
            .filter(|&n| n > 0)
        {
            out.push_str(&format!(
                " · {behind} commits not yet indexed (queryable, run index(action='build') to catch up)"
            ));
        }
    }
    out
}
