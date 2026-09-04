//! Indexing tools: IndexProject, IndexStatus, Index.

use super::super::{parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

pub struct IndexProject;
pub struct IndexStatus;
pub struct IndexVerify;

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

            let (root, lib_path) = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |project| {
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
                    Ok((project.root.clone(), entry.path.clone()))
                })
                .await?;

            let source = format!("lib:{}", lib_name);
            let lib_project_id = source.clone();

            // Sync the library directory into Qdrant under its own
            // project_id namespace (`lib:<name>`). The retrieval stack
            // handles chunking, embedding, and incremental upsert/delete.
            let client = crate::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;
            let opts = crate::retrieval::sync::SyncOpts {
                force_reindex: force,
                ignore_patterns: crate::config::project::ProjectConfig::load_or_default(&lib_path)
                    .map(|c| c.ignored_paths.patterns)
                    .unwrap_or_default(),
                ..Default::default()
            };
            client
                .sync_project(&lib_project_id, &lib_path, opts)
                .await?;

            // Read current version from lockfile and update the registry.
            let versions = crate::library::versions::resolve_dependency_versions(&root);
            let current_version = crate::library::versions::find_version(&versions, lib_name);
            if current_version.is_none() {
                tracing::debug!(
                    "version tracking not available for library '{}' — unsupported lockfile ecosystem",
                    lib_name
                );
            }

            ctx.agent
                .with_project_at_mut(ctx.workspace_override.as_deref(), |project| {
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
                    Ok(())
                })
                .await?;

            // Read counts back from Qdrant for the response. We re-scroll
            // here rather than threading them through sync_project's return
            // type so the same shape works for force / non-force reruns.
            let collection = client.config.collection("code_chunks");
            let (chunk_count, file_count) = client
                .project_index_stats(&collection, &lib_project_id)
                .await
                .unwrap_or((0, 0));

            return Ok(json!({
                "status": "ok",
                "library": lib_name,
                "source": source,
                "files_indexed": file_count,
                "chunks": chunk_count,
            }));
        }

        let force = parse_bool_param(&input["force"]);
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;

        // ── Preflight scope check ───────────────────────────────────────
        // Stat-walk the root to estimate size + detect broad roots (home, system).
        // Requires user confirmation via elicitation if either trigger fires.
        // Runs BEFORE the concurrent-run guard so that a declined or unavailable
        // elicitation never leaves IndexingState stuck in Running.
        {
            use crate::embed::preflight::{check_index_scope, PreflightVerdict};

            let (max_bytes, pf_patterns) = ctx
                .agent
                .with_project_at(ctx.workspace_override.as_deref(), |p| {
                    Ok((
                        p.config.security.max_index_bytes,
                        p.config.ignored_paths.patterns.clone(),
                    ))
                })
                .await
                .unwrap_or((500 * 1024 * 1024, Vec::new()));
            let preflight_root = root.clone();
            let verdict = tokio::task::spawn_blocking(move || {
                check_index_scope(&preflight_root, max_bytes, &pf_patterns)
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

        // Resolve project_id up front — sync_project needs it as the
        // multi-tenant namespace inside the shared Qdrant collection, and the
        // cross-process lock peek just below needs it too.
        let project_id = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok(p.project_id().to_string())
            })
            .await?;

        // Guard against concurrent runs — same agent process.
        {
            let mut state = ctx.agent.indexing.lock().unwrap_or_else(|e| e.into_inner());
            if matches!(*state, IndexingState::Running { .. }) {
                return Ok(json!({
                    "status": "already_running",
                    "hint": "Use index(action='status') to check progress."
                }));
            }

            // Guard against concurrent runs — a DIFFERENT process. Checked
            // synchronously, before committing to `Running`, so a doomed spawn
            // never corrupts `ctx.agent.indexing` with a stale `Failed(...)` that
            // has nothing to do with this agent's own request. See
            // docs/issues/archive/2026-08-24-index-status-lock-contention-reads-as-failed.md.
            if let Some(holder_pid) = crate::retrieval::index_lock::peek(&project_id) {
                return Ok(already_running_elsewhere_response(holder_pid));
            }

            *state = IndexingState::Running {
                done: 0,
                total: 0,
                eta_secs: None,
            };
        }

        let state_arc = ctx.agent.indexing.clone();
        let progress = ctx.progress.clone();
        // Progress is opt-in: `ctx.progress` is `Some` only when the client sent
        // `_meta.progressToken` (see server.rs::call_tool), so emitting here is
        // safe — never an unsolicited notification. BUG-038 was the old
        // unconditional case that crashed Claude Code 2.x.
        if let Some(p) = &progress {
            p.report(0, None).await;
            p.report_text("indexing project").await;
        }

        // Patterns for the index walk (defaults if no config). Fetched here so the
        // spawned sync task can capture them; the indexer prunes these dirs.
        let ignore_patterns = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok(p.config.ignored_paths.patterns.clone())
            })
            .await
            .unwrap_or_default();

        // Capture the dirty-files Arc before spawning so the task can clear it on success.
        let dirty_files_arc = ctx
            .agent
            .dirty_files_arc_for(ctx.workspace_override.as_deref())
            .await;

        tracing::info!(force, "spawning sync task for project");
        let sync_abort_for_task = ctx.agent.active_sync_abort.clone();
        let sync_abort_for_store = ctx.agent.active_sync_abort.clone();
        let task = tokio::spawn(async move {
            // sync_project does not yet stream incremental progress; that deeper
            // wiring is tracked separately. `progress` (opt-in via progressToken)
            // is moved in so future increments can report safely. IndexingState
            // stays at Running{done:0, total:0} until completion sets Done/Failed.
            let _progress = progress;

            tracing::info!("sync task entered");
            crate::heartbeat::note_background_op(&format!("index:{project_id}"));
            let sync_result: anyhow::Result<(crate::retrieval::sync::SyncReport, String)> = async {
                tracing::info!("constructing RetrievalClient::from_env");
                let client =
                    crate::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;

                // A linked worktree gets its own delta sync: reuse main's vectors
                // for byte-identical files, embed only what differs under
                // `<main>@<worktree>`, and record the paths main must not be asked
                // for. `sync_worktree` is reachable ONLY from this path -- never
                // from `semantic_search`, which stays a pure read with no intent
                // gate for an embedder failure to surface under.
                if let Some(main_repo) =
                    crate::prompts::detect_worktree_info(&root).and_then(|info| info.main_repo)
                {
                    // Both ids derived together, once -- see `worktree_ids`'s doc
                    // comment. `semantic_search`'s query-side branch must land on
                    // the exact same `delta_id`, so this is the ONE call both
                    // sides go through.
                    let (main_project_id, delta_id) =
                        crate::retrieval::sync::worktree_ids(&main_repo, &root);
                    let collection = client.config.collection("code_chunks");
                    tracing::info!(
                        main_repo = ?main_repo,
                        main_project_id = %main_project_id,
                        "worktree detected -- syncing delta against main instead of sync_project"
                    );
                    let report = crate::retrieval::sync::sync_worktree(
                        client.code_store.as_ref(),
                        &root,
                        &main_project_id,
                        &collection,
                        &*client.embedder,
                        force,
                        &ignore_patterns,
                        None,
                        // Production: snapshot the live process inside sync_worktree.
                        None,
                    )
                    .await?;

                    // Report what the operator can't see from added/deleted alone:
                    // the delta's total chunk count and how many paths main is now
                    // excluded from serving.
                    let (delta_chunks, _) = client
                        .project_index_stats(&collection, &delta_id)
                        .await
                        .unwrap_or((0, 0));
                    let dirty_count = crate::retrieval::index_state::read_index_state(&root)
                        .map(|s| s.dirty_paths.len())
                        .unwrap_or(0);
                    Ok((
                        report,
                        format!(" delta_chunks={delta_chunks} dirty_paths={dirty_count}"),
                    ))
                } else {
                    tracing::info!("RetrievalClient ready, calling sync_project");
                    let opts = crate::retrieval::sync::SyncOpts {
                        force_reindex: force,
                        record_index_state: true,
                        ignore_patterns: ignore_patterns.clone(),
                        ..Default::default()
                    };
                    let report = client.sync_project(&project_id, &root, opts).await?;
                    Ok((report, String::new()))
                }
            }
            .await;

            // Drop the MutexGuard before any `.await` — MutexGuard is !Send.
            {
                let mut state = state_arc.lock().unwrap_or_else(|e| e.into_inner());
                *state = match sync_result {
                    Ok((report, extra_detail)) => {
                        tracing::info!(
                            added = report.added,
                            deleted = report.deleted,
                            skipped = report.skipped.len(),
                            elapsed_ms = report.elapsed_ms,
                            extra_detail,
                            "sync task succeeded",
                        );
                        // A sync that skipped chunks produced an INCOMPLETE index, and
                        // saying so is the whole point of threading `skipped` up here:
                        // nothing marks a skipped chunk dirty, so a later no-op sync
                        // will never reconcile it. Silence here is what let an index
                        // missing a whole directory read as healthy
                        // (docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md).
                        if !report.skipped.is_empty() {
                            tracing::warn!(
                                skipped = report.skipped.len(),
                                sample = ?report.skipped.iter().take(10).collect::<Vec<_>>(),
                                "index is INCOMPLETE: the embedder refused these chunks",
                            );
                        }
                        let skipped_detail = if report.skipped.is_empty() {
                            String::new()
                        } else {
                            format!(
                                " skipped={} INDEX INCOMPLETE (first: {})",
                                report.skipped.len(),
                                report
                                    .skipped
                                    .iter()
                                    .take(3)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            )
                        };
                        // Indexing succeeded — files are now fresh, clear the dirty set.
                        if let Some(ref arc) = dirty_files_arc {
                            arc.lock().unwrap_or_else(|e| e.into_inner()).clear();
                        }
                        IndexingState::Done {
                            files_indexed: report.added + report.updated,
                            files_deleted: report.deleted,
                            detail: format!(
                                "elapsed_ms={}{extra_detail}{skipped_detail}",
                                report.elapsed_ms
                            ),
                            // Total counts now live in Qdrant — IndexStatus
                            // re-route (task #91) will scroll the collection
                            // for these. For now leave 0 to avoid a sqlite
                            // round-trip that step 8 will delete anyway.
                            total_files: 0,
                            total_chunks: 0,
                        }
                    }
                    Err(e)
                        if e.downcast_ref::<crate::retrieval::index_lock::LockHeldError>()
                            .is_some() =>
                    {
                        // Lost the lock race in the narrow window between this
                        // task's pre-spawn peek (IndexProject::call) and its own
                        // acquire attempt inside sync_project. Benign — someone
                        // else is genuinely indexing — so step back to Idle
                        // rather than freezing at Failed(...) for a request this
                        // agent effectively withdrew.
                        tracing::info!(
                            "sync task deferred: lost the index lock race after \
                             passing the pre-spawn peek"
                        );
                        IndexingState::Idle
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "sync task failed");
                        IndexingState::Failed(e.to_string())
                    }
                };
            }

            // Clear the abort handle slot — the task is done, nothing to cancel.
            *sync_abort_for_task
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = None;
        });
        *sync_abort_for_store
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(task.abort_handle());

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
        "Show index stats: file count, chunk count, model, last update."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let project_id = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok(p.project_id().to_string())
            })
            .await?;

        // Try the Qdrant-backed status. If the retrieval stack is offline or
        // the project has no chunks indexed, return a "not indexed" envelope
        // that callers can branch on the same way they did against the
        // legacy sqlite "no db" path.
        let root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        // Captured inside the arm below, because `client` is dropped with it and the
        // model-mismatch check further down needs the CONFIGURED model to compare the
        // stored one against. Taken from `client.config` rather than the project config,
        // for the same reason the migration budget is: two copies of this setting exist,
        // and only this one describes the embedder the store was actually built through.
        let mut configured_model: Option<String> = None;
        // Does the model NAME determine the vectors, or is it only a label?
        //
        // `embedder_url: None` means the backend is resolved FROM the model spec
        // (`local:`, `local-dir:`, ...), so two names are two models and a difference is a
        // real embedding-space difference. With a url set, the name is a field in an
        // OpenAI-compatible request the server may ignore — measured 2026-08-26 against
        // llama-server on this host: `CodeRankEmbed`, `all-minilm` and
        // `total-nonsense-model` all returned the byte-identical 768-d vector, because it
        // serves whichever gguf is loaded. Reporting a name difference as an
        // embedding-space mismatch there is an overclaim, and this repo's own sidecar hit
        // it on the first live call of this feature.
        let mut model_name_authoritative = false;
        let mut result = match crate::retrieval::client::RetrievalClient::from_env(root.as_deref())
            .await
        {
            Ok(client) => {
                let collection = client.config.collection("code_chunks");
                configured_model = Some(client.config.model.clone());
                model_name_authoritative = client.config.embedder_url.is_none();
                match client.project_index_stats(&collection, &project_id).await {
                    Ok((0, 0)) => json!({
                        "indexed": false,
                        "project_id": project_id,
                        "message": format!(
                            "No chunks indexed for project '{project_id}' in collection '{collection}'. Run index(action='build')."
                        ),
                    }),
                    Ok((chunk_count, file_count)) => {
                        // The cheap half of `index(action="verify")`, folded in here: one
                        // indexed COUNT, no walk. Safe on this path because `status`
                        // ALREADY pays `project_index_stats`, which enumerates the project
                        // — and unsafe on the activation path, which is why that one asks
                        // `project_has_chunks` instead (see `check_has_index` in
                        // src/tools/config/mod.rs and the bug it cites).
                        //
                        // Orphan detection is deliberately NOT folded in: it needs
                        // `chunk_refs` over every chunk plus a stat per stored file, which
                        // is the enumeration class that broke the probe. It stays in
                        // `verify`.
                        let holes = client
                            .code_store
                            .count_chunks_without_vectors(&collection, &project_id)
                            .await
                            .unwrap_or(0);
                        json!({
                            "indexed": true,
                            // Still true, and deliberately not downgraded: an index with a
                            // hole IS queryable, it just cannot return the holed chunks.
                            // Lying here would break every caller that branches on it.
                            "queryable": true,
                            "project_id": project_id,
                            "collection": collection,
                            "file_count": file_count,
                            "chunk_count": chunk_count,
                            "chunks_without_vectors": holes,
                            "integrity": if holes > 0 { "degraded" } else { "ok" },
                            // The point of this whole change. `indexed: true, queryable:
                            // true` READ as "complete" — that is how an index holding 486
                            // of 1606 files, with a whole top-level directory at zero, was
                            // reported healthy. Coverage cannot be answered cheaply (it
                            // needs the indexer's walk), so say so explicitly instead of
                            // letting silence imply the opposite.
                            // docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md
                            "coverage": "unchecked",
                            "coverage_hint": "file_count/chunk_count are what the store HOLDS, not proof it holds everything eligible. Run index(action='verify') for coverage against the indexer's own walk.",
                        })
                    }
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
                IndexingState::Idle => {
                    // This agent isn't indexing, but a different process might
                    // be — surface that instead of silence, the same blind spot
                    // this bug fixed for the build side. See
                    // docs/issues/archive/2026-08-24-index-status-lock-contention-reads-as-failed.md.
                    if let Some(holder_pid) = crate::retrieval::index_lock::peek(&project_id) {
                        result["indexing"] = running_elsewhere_indexing_block(holder_pid);
                    }
                }
                IndexingState::Running {
                    done,
                    total,
                    eta_secs,
                } => {
                    let mut indexing = json!({
                        "status": "running",
                        "done": done,
                        "total": total,
                        "eta_secs": eta_secs,
                    });
                    // Project-scope builds (sync_project) don't stream per-file
                    // progress, so done/total stay at 0/0 for the whole build —
                    // see IndexProject::call. Without a note this reads as a
                    // stall. chunk_count climbs from 0 on an initial build; on a
                    // force re-embed it stays ~stable (chunks upserted in place),
                    // so the note is scenario-honest rather than promising movement.
                    if *done == 0 && *total == 0 {
                        indexing["note"] = json!(
                            "0/0 is the healthy in-progress shape for project-scope \
                             builds (per-file progress isn't streamed), not a stall. \
                             chunk_count climbs from 0 on an initial build; on a \
                             force re-embed it stays ~stable (chunks upserted in place)."
                        );
                        indexing["chunks_so_far"] = result
                            .get("chunk_count")
                            .and_then(|v| v.as_u64())
                            .map_or_else(|| json!(0), |cc| json!(cc));
                    }
                    result["indexing"] = indexing;
                }
                IndexingState::Done {
                    files_indexed,
                    files_deleted,
                    detail,
                    total_files,
                    total_chunks,
                } => {
                    // The producer leaves total_files/total_chunks at 0 (see
                    // IndexProject::call, task #91) — the authoritative totals
                    // are the top-level Qdrant file_count/chunk_count above.
                    let total_files = resolve_done_total(&result, "file_count", *total_files);
                    let total_chunks = resolve_done_total(&result, "chunk_count", *total_chunks);
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

        // Index freshness vs git HEAD — surfaces external checkout/pull/HEAD
        // moves that the on-edit reindex never observes. Resilient: any failure
        // (no project root, no sidecar, non-git root) simply omits git_sync.
        if result["indexed"].as_bool() == Some(true) {
            if let Ok(root) = ctx
                .agent
                .require_project_root_for(ctx.workspace_override.as_deref())
                .await
            {
                // The two fields docs/manual/src/troubleshooting.md tells users to compare.
                // They were dropped in 79e0e4f2 and their readers in `format_index_status`
                // outlived them by three and a half months, so the manual's recipe for
                // diagnosing a model mismatch named two keys nothing emitted.
                // docs/issues/archive/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
                //
                // `indexed_with_model` is the STORED model — what actually produced the
                // vectors — and `configured_model` is what is set now. Reporting the
                // configured value under both names is the shortcut that would make a
                // mismatch invisible by construction, which is why the sidecar records the
                // model at sync time rather than this path deriving it.
                if let Some(st) = crate::retrieval::index_state::read_index_state(&root) {
                    result["indexed_at"] = json!(st.last_indexed_at);
                    // Who wrote this sidecar, reported only when it was NOT this build —
                    // presence-means-a-problem, same convention as `model_mismatch` and
                    // `last_sync_skipped` below and above.
                    //
                    // Facts, not a verdict. A peer's rebuild of unrelated code makes every
                    // other running server "stale" while its answers stay correct, so a
                    // flat "stale writer" warning would cry wolf continuously in exactly
                    // the multi-session case this serves. What a reader needs instead is
                    // enough to adjudicate: which build, whether that build was dirty,
                    // which pid, and — the load-bearing one — whether the writer's own
                    // executable had already been unlinked, which is the difference between
                    // "an older peer wrote this" and "code that no longer exists wrote
                    // this".
                    //
                    // `None` is silent by design: a pre-field sidecar is "not recorded",
                    // never "written by the current build".
                    // docs/issues/archive/2026-08-26-zombie-servers-on-deleted-binaries-stamp-stale-config-into-shared-state.md
                    if let Some(w) = st.written_by.as_ref() {
                        if w.git_sha != env!("CODESCOUT_GIT_SHA") {
                            result["written_by"] = json!({
                                "git_sha": w.git_sha,
                                "git_dirty": w.git_dirty,
                                "pid": w.pid,
                                "exe_deleted": w.exe_deleted,
                                "reading_binary_sha": env!("CODESCOUT_GIT_SHA"),
                            });
                        }
                    }
                    // The durable half of
                    // docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md:
                    // a caller who never runs index(action="verify") still sees whether the
                    // sync that produced THIS store's vectors skipped anything, because the
                    // fact outlives the call that discovered it. Absent (not `count: 0`) when
                    // clean, matching `model_mismatch`'s presence-means-a-problem convention.
                    if st.last_sync_skipped_count > 0 {
                        result["last_sync_skipped"] = json!({
                            "count": st.last_sync_skipped_count,
                            "sample": st.last_sync_skipped_sample,
                        });
                        if result["integrity"] == json!("ok") {
                            result["integrity"] = json!("degraded");
                        }
                    }
                    if let Some(m) = st.indexed_with_model.as_deref() {
                        result["indexed_with_model"] = json!(m);
                        // A dimension check already catches a dimension change; the common
                        // swaps share one (AllMiniLM/BGESmall both 384d, CodeRankEmbed/Jina
                        // both 768d), so only a stored identity sees them.
                        if configured_model.as_deref().is_some_and(|c| c != m) {
                            // Two claims of different strength, because the name means
                            // different things on the two backends. Saying "two embedding
                            // spaces" when the endpoint ignores the name is the overclaim
                            // this whole field exists to avoid making.
                            let hint = if model_name_authoritative {
                                "The stored vectors were built by a different model than \
                                 the one configured now. The backend is resolved FROM the \
                                 model spec, so these are genuinely different models and \
                                 queries are being scored across two embedding spaces. Run \
                                 index(action='build', force=true) to re-embed."
                            } else {
                                "The stored and configured model NAMES differ. The backend \
                                 is a configured endpoint, which may ignore the requested \
                                 model and serve whatever it has loaded — so this is not \
                                 proof of two embedding spaces, only that two writers \
                                 disagreed about what they were using. Check the endpoint \
                                 (GET /v1/models) before rebuilding; if it serves one \
                                 model, the labels are wrong and the vectors are fine."
                            };
                            result["model_mismatch"] = json!({
                                "indexed_with": m,
                                "configured": configured_model.clone(),
                                // Names the strength of the claim, so a caller can branch
                                // instead of re-deriving it from the prose.
                                "name_is_authoritative": model_name_authoritative,
                                "hint": hint,
                            });
                        }
                    }
                }
                // Always present, even with no sidecar: it describes config, not the store,
                // so it is knowable regardless — and absence of `indexed_with_model` is
                // "never recorded", never a mismatch.
                if let Some(c) = configured_model.as_deref() {
                    result["configured_model"] = json!(c);
                }

                if let Some(gs) = crate::retrieval::index_state::git_sync_status(&root) {
                    result["git_sync"] = gs;
                }
            }
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

    /// Mixed: `status`/`verify`/`cancel` read, `build` writes. Additive — a build
    /// refreshes and never destroys user data — and incremental builds skip unchanged
    /// hashes, so a repeat is a no-op. `openWorld` stays `true`: `RequiresEmbeddings`.
    fn annotations(&self) -> Option<rmcp::model::ToolAnnotations> {
        crate::tools::annot::additive_open()
    }

    fn is_write(&self, input: &Value) -> bool {
        input.get("action").and_then(Value::as_str) == Some("build")
    }

    fn description(&self) -> &str {
        "Semantic index operations. Actions: \
             `build` (build/update the index; `scope='lib:<name>'` for a library), \
             `status` (show index stats), \
             `verify` (check coverage against the filesystem; read-only), \
             `cancel` (abort an in-flight reindex — no-op if nothing is running)."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["build", "status", "cancel", "verify"],
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
            "verify" => IndexVerify.call(input, ctx).await,
            "cancel" => {
                let handle = ctx
                    .agent
                    .active_sync_abort
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .take();
                match handle {
                    Some(h) => {
                        h.abort();
                        // Aborted future won't reach its terminal-state arm —
                        // reset IndexingState here so status reflects reality.
                        *ctx.agent.indexing.lock().unwrap_or_else(|e| e.into_inner()) =
                            crate::agent::IndexingState::Failed("cancelled by user".into());
                        tracing::info!("sync task cancelled by user");
                        Ok(json!({"status": "cancelled"}))
                    }
                    None => Ok(json!({"status": "no_active_sync"})),
                }
            }
            other => Err(crate::tools::RecoverableError::with_hint(
                format!("unknown index action: {}", other),
                "Valid actions: 'build', 'status', 'cancel', 'verify'.",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        // `verdict` FIRST: a verify envelope carries neither `indexed` nor
        // `file_count`, so without this arm it fell through to IndexProject's
        // formatter and rendered an integrity report as a build summary.
        if result.get("verdict").is_some() {
            IndexVerify.format_compact(result)
        } else if result.get("indexed").is_some() || result.get("file_count").is_some() {
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
#[async_trait::async_trait]
impl crate::tools::Tool for IndexVerify {
    fn name(&self) -> &str {
        "index_verify"
    }

    fn description(&self) -> &str {
        "Verify the semantic index against the filesystem: coverage, orphaned rows, \
         and chunks missing vectors. Read-only."
    }

    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}})
    }

    /// Read-only by construction, and that is the design rather than a limitation. A
    /// negative result must never authorise a deletion: a bad walk reporting every
    /// file as an orphan would, if this pruned, destroy a live index. Repair stays
    /// with `index(action="build")`, whose prune runs against a walk it performed
    /// itself.
    async fn call(&self, _input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let (project_id, ignore_patterns) = ctx
            .agent
            .with_project_at(ctx.workspace_override.as_deref(), |p| {
                Ok((
                    p.project_id().to_string(),
                    p.config.ignored_paths.patterns.clone(),
                ))
            })
            .await?;
        let root = ctx
            .agent
            .require_project_root_for(ctx.workspace_override.as_deref())
            .await?;

        let client = crate::retrieval::client::RetrievalClient::from_env(Some(&root)).await?;
        let collection = client.config.collection("code_chunks");

        let integrity = crate::retrieval::sync::verify_index_coverage(
            &root,
            &project_id,
            &collection,
            client.code_store.as_ref(),
            &ignore_patterns,
        )
        .await?;

        let git = crate::retrieval::index_state::git_sync_status(&root);
        let behind = git
            .as_ref()
            .and_then(|v| v.get("behind_commits"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);

        let empty_dirs = integrity.empty_eligible_dirs.len();
        let verdict = integrity_verdict(
            behind,
            integrity.missing_count,
            empty_dirs,
            integrity.chunks_without_vectors,
        );

        // The hint names the ONE next action for this verdict. A report that lists
        // problems without saying which command addresses them gets read as noise.
        let hint = if verdict == "complete" {
            complete_hint(&collection)
        } else if verdict == "stale" {
            format!(
                "Index is {behind} commit(s) behind HEAD; the {} missing file(s) are \
                 explained by that. Run index(action='build') to catch up.",
                integrity.missing_count
            )
        } else if integrity.chunks_without_vectors > 0 {
            format!(
                "{} chunk(s) have no vector — they answer chunk_refs and count toward \
                 status, but can never match a query. Run index(action='build', \
                 force=true).",
                integrity.chunks_without_vectors
            )
        } else if empty_dirs > 0 {
            format!(
                "These eligible top-level directories have files on disk and NONE in \
                 the index: {}. Run index(action='build', force=true).",
                integrity.empty_eligible_dirs.join(", ")
            )
        } else {
            format!(
                "Index is level with HEAD but missing {} eligible file(s). Run \
                 index(action='build').",
                integrity.missing_count
            )
        };

        // Memories are the OTHER collection, and they leak differently: a failed
        // cross-embed is non-fatal, so the markdown lands and no point is ever
        // written. That damage is invisible to every store-side instrument, which is
        // why it is checked against disk here rather than left to
        // `reembed_memories_in_place`, whose own enumeration cannot see it.
        //
        // Folded into this verdict-bearing report rather than given a `memory`
        // action of its own, for two reasons: a new action costs advertised
        // tool-surface bytes against a budget that is already at its cap, and a
        // SECOND health surface is the thing
        // docs/issues/archive/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
        // asks us to stop creating.
        //
        // Degrades rather than propagates: the code index is this tool's primary
        // subject, and a memory-store outage must not turn its report into an error.
        let memories = match ctx.agent.semantic_memory_store().await {
            Ok(store) => {
                // SHARED topics only — `cross_embed_memory` runs under `if !private`,
                // so private memories have no point BY DESIGN and would otherwise
                // report as missing, every one of them, forever.
                let topics = ctx
                    .agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| p.memory.list())
                    .await?;
                match crate::memory::verify_memory_coverage(&topics, store.as_ref(), &project_id)
                    .await
                {
                    Ok(m) => json!({
                        "on_disk": m.on_disk,
                        "in_store": m.in_store,
                        "missing_count": m.missing_count,
                        "missing_sample": m.missing_sample,
                        "orphan_count": m.orphan_count,
                        "orphan_sample": m.orphan_sample,
                        "hint": memory_coverage_hint(&m),
                    }),
                    Err(e) => json!({"status": "unchecked", "reason": format!("{e:#}")}),
                }
            }
            Err(e) => json!({"status": "unchecked", "reason": format!("{e:#}")}),
        };

        Ok(json!({
            "verdict": verdict,
            "project_id": project_id,
            "collection": collection,
            "memories": memories,
            "expected_files": integrity.expected_files,
            "stored_files": integrity.stored_files,
            "missing_count": integrity.missing_count,
            "missing_sample": integrity.missing_sample,
            "orphan_count": integrity.orphan_count,
            "orphan_sample": integrity.orphan_sample,
            "empty_eligible_dirs": integrity.empty_eligible_dirs,
            "chunks_without_vectors": integrity.chunks_without_vectors,
            "git_sync": git,
            "hint": hint,
        }))
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        let verdict = result["verdict"].as_str()?;
        Some(format!(
            "{verdict} · {}/{} files · missing {} · orphans {} · no-vector {}",
            result["stored_files"].as_u64().unwrap_or(0),
            result["expected_files"].as_u64().unwrap_or(0),
            result["missing_count"].as_u64().unwrap_or(0),
            result["orphan_count"].as_u64().unwrap_or(0),
            result["chunks_without_vectors"].as_u64().unwrap_or(0),
        ))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::Always
    }
}

/// Derive ONE verdict from the integrity axes.
///
/// Separate booleans are what the originating report asked to eliminate: *"status
/// surfaces can disagree… these should derive from one authoritative state model that
/// distinguishes catalog freshness, embedding freshness, and queryability."* Six
/// independent fields make the reader do that reconciliation, and readers do it
/// inconsistently.
///
/// Order matters, and the `stale` arm before `incomplete` is the whole point. An index
/// legitimately behind HEAD is *expected* to be missing the files those commits added
/// — calling that "incomplete" would cry wolf on every project that has not re-synced,
/// which is most of them most of the time. Only an index that is level with HEAD and
/// still missing files is actually broken.
fn integrity_verdict(
    behind_commits: u64,
    missing: usize,
    empty_dirs: usize,
    holes: usize,
) -> &'static str {
    if holes > 0 || empty_dirs > 0 {
        // A vector hole or a wholly-absent eligible directory is broken regardless of
        // freshness: no number of pending commits explains either.
        "incomplete"
    } else if behind_commits > 0 {
        "stale"
    } else if missing > 0 {
        "incomplete"
    } else {
        "complete"
    }
}
/// The `complete` verdict's hint.
///
/// Extracted so its CLAIM can be pinned by a test, for the same reason
/// [`integrity_verdict`] was: the sentence is what readers act on, and it was
/// wrong in a way no field in the report was.
///
/// **What this tool actually verifies is two lanes, not the index.** It opens
/// `code_chunks` for the project and the shared memory store, and nothing else.
/// The librarian's artifact chunk vectors live in a *separate* per-project
/// store (`artifact_chunks_<base>_<hash>` on Qdrant, or the sqlite-vec
/// `artifact_vec_v2` escape hatch) which this tool never constructs — the core
/// `ToolContext` carries no catalog and no artifact store, so it cannot.
///
/// Measured 2026-09-04: this surface returned `verdict: "complete"`,
/// `memories: 23/23`, `chunks_without_vectors: 0` and *"covers every eligible
/// file"* on a host where `doc(action="find", semantic=…)` returned **zero
/// results for every query**, because the per-project artifact collection had
/// never been built there. Every field was accurate. The sentence was the
/// defect, and it was quoted back as evidence the catalog was fully restored.
fn complete_hint(collection: &str) -> String {
    format!("Index is level with HEAD and covers every eligible file in `{collection}`, plus the shared memory store. NOT covered: the librarian's artifact vectors, which live in a separate per-project store this tool never opens and can be wholly absent while every field above reads clean. Check that lane separately — `doc(action=\"find\", semantic=\"…\")` returning results is the cheapest positive test; `librarian(action=\"reindex\", reembed=true)` builds it.")
}

/// The one next action for a memory-coverage result.
///
/// Names `codescout migrate-memories --in-place`, which reads the missing memories from
/// disk **server-side**. The first cut of this hint said to re-run `memory(action='write')`
/// with each memory's current content — correct for a human with the file open, wrong for
/// an agent. That routes the bytes through the caller's context, making the caller a lossy
/// channel with no checksum until after the write has already overwritten the file, and at
/// `eval-design`'s 31 KB it is not possible at all: a result that large comes back as a
/// buffer handle that cannot be re-emitted as a tool argument.
///
/// `--in-place` could not fix this either until `embed_missing_memories` was added
/// alongside it, because the store-driven pass cannot see a point that was never written.
/// Both passes now run under that one flag, so the hint names one command rather than
/// ruling one out.
fn memory_coverage_hint(m: &crate::memory::MemoryIntegrity) -> String {
    if m.missing_count == 0 && m.orphan_count == 0 {
        return "Every memory on disk has a point, and every point has a file.".to_string();
    }
    let mut parts = Vec::new();
    if m.missing_count > 0 {
        parts.push(format!(
            "{} memory/memories are on disk with NO point — invisible to recall. Repair \
             with `codescout migrate-memories --in-place`, which reads them from disk \
             server-side: {}.",
            m.missing_count,
            m.missing_sample.join(", ")
        ));
    }
    if m.orphan_count > 0 {
        parts.push(format!(
            "{} point(s) have no file on disk — recall can still return them. Drop with \
             memory(action='forget'): {}.",
            m.orphan_count,
            m.orphan_sample.join(", ")
        ));
    }
    parts.join(" ")
}

pub(crate) fn format_index_status(result: &Value) -> String {
    let indexed = result["indexed"].as_bool().unwrap_or(false);
    if !indexed {
        return "not indexed".to_string();
    }
    let files = result["file_count"].as_u64().unwrap_or(0);
    let chunks = result["chunk_count"].as_u64().unwrap_or(0);

    // NOT "good". That word was derived from nothing but a non-zero chunk count, and
    // it is the overclaim at the heart of
    // docs/issues/archive/2026-08-26-index-status-claims-complete-without-checking-coverage.md:
    // an index holding 486 of 1606 files with a whole top-level directory at zero
    // rendered as "good · queryable". `indexed` states what is actually known here;
    // coverage is not checked on this path, and `coverage_hint` in the JSON says so.
    //
    // Three conditions ARE knowable cheaply, so when any holds it leads the line.
    // A model mismatch outranks everything: every score is being compared across two
    // embedding spaces, not merely some. A skipped-chunk sync outranks a vector hole:
    // skipped chunks never reached the store at all, where a hole means the chunk
    // exists but lacks a vector -- the more severe absence leads.
    let holes = result["chunks_without_vectors"].as_u64().unwrap_or(0);
    let skipped = result["last_sync_skipped"]["count"].as_u64().unwrap_or(0);
    let mismatch = result["model_mismatch"].is_object();
    let mut out = if mismatch {
        let indexed_with = result["model_mismatch"]["indexed_with"]
            .as_str()
            .unwrap_or("?");
        let configured = result["model_mismatch"]["configured"]
            .as_str()
            .unwrap_or("?");
        // Two strengths, matching the JSON's `name_is_authoritative`. On a configured
        // endpoint the model name is a request field the server may ignore (measured: this
        // host's llama-server returns the byte-identical vector for any name), so a name
        // difference is a flag to check, not a verdict to announce. Shouting MISMATCH
        // there is a false alarm on a line people see constantly — and a line that cries
        // wolf is one nobody reads.
        if result["model_mismatch"]["name_is_authoritative"]
            .as_bool()
            .unwrap_or(false)
        {
            format!(
                "MODEL MISMATCH · indexed with {indexed_with}, configured {configured} — \
                     different models, so every score crosses two embedding spaces · \
                     {files} files · {chunks} chunks"
            )
        } else {
            format!(
                "model label differs · indexed as {indexed_with}, configured {configured} \
                     — the endpoint may ignore the name; check before rebuilding · {files} \
                     files · {chunks} chunks"
            )
        }
    } else if skipped > 0 {
        format!(
            "DEGRADED · last sync skipped {skipped} chunk(s) that could not be embedded · \
                 queryable · {files} files · {chunks} chunks"
        )
    } else if holes > 0 {
        format!(
            "DEGRADED · {holes} chunk(s) have no vector · queryable · {files} files · {chunks} chunks"
        )
    } else {
        format!("indexed · queryable · {files} files · {chunks} chunks")
    };

    // Live again. These two arms read keys that `79e0e4f2` stopped emitting, and they
    // sat unreachable for three and a half months while a test that hand-built the
    // envelope kept them green — so the model is appended only when it is NOT already
    // in the mismatch banner above, or the line names it twice.
    // docs/issues/archive/2026-08-26-index-status-model-fields-dropped-but-still-documented.md
    if !mismatch {
        if let Some(model) = result["indexed_with_model"].as_str() {
            out.push_str(&format!(" · {model}"));
        }
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

/// Resolve a `done`-state total for `IndexStatus` output.
///
/// The sync-task completion path leaves `IndexingState::Done.total_files` /
/// `.total_chunks` at 0 (see `IndexProject::call`, task #91) — the authoritative
/// totals live in Qdrant and are already surfaced as the top-level `file_count`
/// / `chunk_count`. Prefer those so the `done` summary matches reality; fall
/// back to the placeholder variant value only when Qdrant didn't supply a count
/// (offline / not yet indexed).
pub(crate) fn resolve_done_total(result: &Value, key: &str, fallback: usize) -> u64 {
    result
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(fallback as u64)
}

/// The response for a `build` request that never started because a different
/// process already holds this project's index lock. Mirrors the neighboring
/// same-agent-process `{"status": "already_running"}` shape used when this
/// agent's own `ctx.agent.indexing` is already `Running`.
pub(crate) fn already_running_elsewhere_response(holder_pid: Option<u32>) -> Value {
    json!({
        "status": "already_running_elsewhere",
        "holder_pid": holder_pid,
        "hint": "Another process already holds this project's index lock and is \
                 indexing it. Use index(action='status') — chunk_count/file_count \
                 reflect its live progress even though this request did not start \
                 a new job.",
    })
}

/// The `indexing` block `IndexStatus` reports when this agent is `Idle` but a
/// different process currently holds the project's index lock — the truthful
/// alternative to reporting nothing, which is what an `Idle` agent saw before.
pub(crate) fn running_elsewhere_indexing_block(holder_pid: Option<u32>) -> Value {
    json!({
        "status": "running_elsewhere",
        "holder_pid": holder_pid,
    })
}

#[cfg(test)]
mod integrity_verdict_tests {
    use super::integrity_verdict;

    fn mem_integrity(missing: &[&str], orphans: &[&str]) -> crate::memory::MemoryIntegrity {
        crate::memory::MemoryIntegrity {
            on_disk: 23,
            in_store: 18,
            missing_count: missing.len(),
            missing_sample: missing.iter().map(|s| (*s).to_string()).collect(),
            orphan_count: orphans.len(),
            orphan_sample: orphans.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    /// The hint must name a repair the CALLER CAN ACTUALLY PERFORM, and must not name the
    /// one it cannot.
    ///
    /// It first said to re-run `memory(action='write')` with each memory's current
    /// content. That routes the bytes through the caller's context — a lossy channel with
    /// no checksum until after the write has already overwritten the file — and for the
    /// 31 KB `eval-design` it is impossible outright, since a result that size comes back
    /// as a buffer handle that cannot be re-emitted as a tool argument.
    #[test]
    fn missing_hint_names_the_repair_and_rules_out_the_wrong_one() {
        let h = super::memory_coverage_hint(&mem_integrity(&["eval-design"], &[]));
        assert!(h.contains("eval-design"), "must name the memory: {h}");
        assert!(
            h.contains("migrate-memories --in-place"),
            "must name the server-side repair: {h}"
        );
        assert!(
            !h.contains("memory(action='write')"),
            "must not route the bytes through the caller's own context: {h}"
        );
    }

    /// Both kinds present must both be reported — an early return on the first would hide
    /// the second, and they need different commands: the missing ones are recovered from
    /// disk server-side, the stale points are dropped through the tool.
    #[test]
    fn hint_reports_missing_and_orphans_together() {
        let h = super::memory_coverage_hint(&mem_integrity(&["lost"], &["stale-point"]));
        assert!(h.contains("lost") && h.contains("stale-point"), "{h}");
        assert!(
            h.contains("migrate-memories --in-place") && h.contains("memory(action='forget')"),
            "each kind needs its own command: {h}"
        );
    }

    /// The two clauses must not run together.
    ///
    /// `parts.join(" ")` put the missing list's last entry flush against the orphan
    /// count, rendering `... system-prompt 3 point(s) have no file` — which reads as a
    /// list item with a number stuck on it. The three `contains` assertions above all
    /// passed on that output; only reading the live bytes showed it. So this pins the
    /// SEAM rather than the substrings.
    #[test]
    fn hint_clauses_are_separated_sentences() {
        let h = super::memory_coverage_hint(&mem_integrity(&["lost"], &["stale-point"]));
        assert!(
            !h.contains("lost 1 point"),
            "clauses ran together — a name flush against a count reads as one token: {h}"
        );
        assert!(
            h.contains("lost."),
            "the missing clause must close before the next one opens: {h}"
        );
    }

    /// A clean result must not read as a warning. This surface is checked routinely, so a
    /// permanently alarming hint is how the whole report gets learned around.
    #[test]
    fn clean_memory_hint_is_not_a_warning() {
        let h = super::memory_coverage_hint(&mem_integrity(&[], &[]));
        assert!(!h.contains("memory(action="), "no repair to name: {h}");
        assert!(!h.to_lowercase().contains("missing"), "{h}");
    }

    /// The arm ordering is the whole design, so it gets pinned directly.
    ///
    /// An index legitimately behind HEAD is EXPECTED to be missing the files those
    /// commits added. Reporting that as `incomplete` would fire on almost every
    /// project almost all the time, and a check that always complains gets switched
    /// off — which is the failure mode this verdict exists to avoid.
    #[test]
    fn behind_head_with_missing_files_is_stale_not_incomplete() {
        assert_eq!(integrity_verdict(10, 42, 0, 0), "stale");
    }

    /// The converse, and the actual defect: level with HEAD and still short.
    #[test]
    fn level_with_head_but_missing_files_is_incomplete() {
        assert_eq!(integrity_verdict(0, 42, 0, 0), "incomplete");
    }

    /// Two conditions no amount of pending commits can explain, so they outrank
    /// staleness rather than hiding behind it. A vector hole and a wholly-absent
    /// eligible directory are both broken at any freshness.
    #[test]
    fn holes_and_empty_directories_outrank_staleness() {
        assert_eq!(
            integrity_verdict(10, 0, 0, 3),
            "incomplete",
            "a chunk with no vector can never match a query, however fresh the index"
        );
        assert_eq!(
            integrity_verdict(10, 0, 1, 0),
            "incomplete",
            "an eligible directory holding nothing is not explained by lag"
        );
    }

    #[test]
    fn a_level_fully_covered_index_is_complete() {
        assert_eq!(integrity_verdict(0, 0, 0, 0), "complete");
    }
    /// `complete` is a claim about TWO lanes, and the hint must say which.
    ///
    /// This pins the CLAIM, not the prose. Both directions matter: a hint that
    /// merely stopped saying "every eligible file" would satisfy a `!contains`
    /// check while still telling the reader nothing about where the uncovered
    /// lane lives — and that silence is the half that cost a day.
    ///
    /// The negative assertion is the one that fails against the superseded
    /// wording (`"Index is level with HEAD and covers every eligible file."`),
    /// observed RED before this shipped.
    #[test]
    fn the_complete_hint_names_the_lanes_it_checked_and_the_one_it_did_not() {
        let h = super::complete_hint("code_chunks");
        assert!(
            h.contains("code_chunks"),
            "must name the collection it actually opened: {h}"
        );
        assert!(
            h.contains("artifact"),
            "must name the lane it does NOT cover — silence about it is what made a \
         broken artifact search read as a clean index: {h}"
        );
        assert!(
            !h.contains("covers every eligible file."),
            "must not assert whole-index coverage: this tool opens code_chunks and the \
         memory store, never the artifact vector store: {h}"
        );
        assert!(
            h.contains("semantic=") || h.contains("reembed=true"),
            "a scope disclaimer with no positive test to run is not actionable: {h}"
        );
    }
}
