//! Indexing tools: IndexProject, IndexStatus, Index.

use super::super::{parse_bool_param, Tool, ToolContext};
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

        // Resolve project_id up front — sync_project needs it as the
        // multi-tenant namespace inside the shared Qdrant collection.
        let project_id = ctx
            .agent
            .with_project(|p| Ok(p.project_id().to_string()))
            .await?;

        // Capture the dirty-files Arc before spawning so the task can clear it on success.
        let dirty_files_arc = ctx.agent.dirty_files_arc().await;

        tokio::spawn(async move {
            // Progress notifications are intentionally not wired through
            // sync_project yet (BUG-038 — Claude Code 2.x crashes on
            // unsolicited progress; tracked separately). IndexingState stays
            // at Running{done:0, total:0} until completion sets Done/Failed.
            let _progress = progress;

            let sync_result = async {
                let client = crate::retrieval::client::RetrievalClient::from_env().await?;
                let opts = crate::retrieval::sync::SyncOpts {
                    force_reindex: force,
                    ..Default::default()
                };
                client.sync_project(&project_id, &root, opts).await
            }
            .await;

            // Drop the MutexGuard before any `.await` — MutexGuard is !Send.
            {
                let mut state = state_arc.lock().unwrap_or_else(|e| e.into_inner());
                *state = match sync_result {
                    Ok(report) => {
                        // Indexing succeeded — files are now fresh, clear the dirty set.
                        if let Some(ref arc) = dirty_files_arc {
                            arc.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        }
                        IndexingState::Done {
                            files_indexed: report.added + report.updated,
                            files_deleted: report.deleted,
                            detail: format!("elapsed_ms={}", report.elapsed_ms),
                            // Total counts now live in Qdrant — IndexStatus
                            // re-route (task #91) will scroll the collection
                            // for these. For now leave 0 to avoid a sqlite
                            // round-trip that step 8 will delete anyway.
                            total_files: 0,
                            total_chunks: 0,
                        }
                    }
                    Err(e) => IndexingState::Failed(e.to_string()),
                };
            }
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
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let project_id = ctx
            .agent
            .with_project(|p| Ok(p.project_id().to_string()))
            .await?;

        // Try the Qdrant-backed status. If the retrieval stack is offline or
        // the project has no chunks indexed, return a "not indexed" envelope
        // that callers can branch on the same way they did against the
        // legacy sqlite "no db" path.
        let mut result = match crate::retrieval::client::RetrievalClient::from_env().await {
            Ok(client) => {
                let collection = client.config.collection("code_chunks");
                match client
                    .qdrant
                    .project_index_stats(&collection, &project_id)
                    .await
                {
                    Ok((0, 0)) => json!({
                        "indexed": false,
                        "project_id": project_id,
                        "message": format!(
                            "No chunks indexed for project '{project_id}' in collection '{collection}'. Run index(action='build')."
                        ),
                    }),
                    Ok((chunk_count, file_count)) => json!({
                        "indexed": true,
                        "queryable": true,
                        "project_id": project_id,
                        "collection": collection,
                        "file_count": file_count,
                        "chunk_count": chunk_count,
                    }),
                    Err(e) => json!({
                        "indexed": false,
                        "project_id": project_id,
                        "message": format!("Qdrant scroll failed: {e}"),
                    }),
                }
            }
            Err(e) => json!({
                "indexed": false,
                "project_id": project_id,
                "message": format!(
                    "Retrieval stack offline: {e}. Run scripts/retrieval-stack.sh up."
                ),
            }),
        };

        // Append background indexing state (agent-tracked, independent of
        // the Qdrant collection state — surfaces in-flight `index(build)`
        // progress and the completion summary).
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

        // Per-library indexing states (agent-tracked, non-idle only).
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
