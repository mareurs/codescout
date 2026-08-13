//! SemanticSearch tool — vector + BM25 hybrid search.

use super::super::format::format_overflow;
use super::super::{optional_u64_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

/// Map a `RetrievalClient::from_env` error into what `SemanticSearch::call`
/// returns to its caller.
///
/// A `RecoverableError` (e.g. `build_embedder`'s `guard_sparse` conflict — a
/// local backend with the hybrid sparse leg still enabled) already carries
/// its own accurate message and hint; passed through unchanged here, rather
/// than relabelled below, is what keeps a config conflict from being
/// reported as a down service. Anything else (a genuine connect/build
/// failure) gets the "retrieval stack offline" framing, tailored to the
/// active backend — the lite stack has no local daemon to start; its
/// failure mode is an unreachable remote embedding endpoint, not a down
/// Qdrant/sparse/reranker service.
pub(crate) fn map_from_env_error(
    e: anyhow::Error,
    backend: crate::retrieval::code_store::VectorBackend,
) -> anyhow::Error {
    if e.downcast_ref::<crate::tools::RecoverableError>().is_some() {
        return e;
    }
    let hint = match backend {
        crate::retrieval::code_store::VectorBackend::SqliteVec => {
            "Lite stack: verify CODESCOUT_EMBEDDER_URL and EMBED_API_KEY — \
             the remote embedding endpoint is unreachable (no local daemon to start)."
        }
        crate::retrieval::code_store::VectorBackend::Qdrant => {
            "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack."
        }
    };
    crate::tools::RecoverableError::with_hint(format!("retrieval stack offline: {e}"), hint).into()
}

/// Map a qdrant/search error string to an actionable recovery hint.
///
/// Patterns are checked in order of specificity: collection-missing first
/// (most common after first-time setup), then dim-mismatch (model/index
/// drift), then TEI errors (dense embedding service unhealthy), then
/// embedder-connect (client-side: dense/sparse endpoint unreachable via
/// `EmbedderHttp`), then the resolver-path embedder bucket (client-side:
/// `ollama:`/`openai:`/etc. via `RemoteEmbedder`, reached with no
/// `embedder_url` configured), then transport (stack went away), then a
/// generic fallback.
///
/// The embedder-connect bucket must precede transport: a client-side connect
/// failure's message carries reqwest's "Connection refused", which would
/// otherwise be misrouted to the qdrant-oriented transport hint
/// (docs/issues/2026-07-13-semantic-search-misleading-stack-error-on-missing-env.md).
/// The resolver-path bucket exists for the same reason: `RemoteEmbedder`'s
/// failure shapes ("embedding server unavailable after N attempts", "HTTP
/// {status} from embedding server", `crates/codescout-embed/src/remote.rs`)
/// match neither the TEI bucket's "tei status" wording nor the
/// `EmbedderHttp`-specific bucket's "embed connect failed"/"openai send"/
/// "embed sparse" wording, and would otherwise fall all the way to the
/// generic qdrant-oriented fallback — the identical misrouting class.
pub(crate) fn classify_search_error(err_str: &str, project_id: &str) -> String {
    if err_str.contains("doesn't exist")
        || err_str.contains("not found")
        || err_str.contains("Collection")
    {
        format!(
            "Qdrant collection is missing for project `{project_id}`. \
             Populate it: `cargo run --release --bin sync_project -- . {project_id}`"
        )
    } else if err_str.contains("Vector dimension") || err_str.contains("expected dim") {
        "Embedding dim mismatch between index and configured model. \
         Drop the collection and re-index: \
         `curl -X DELETE $CODESCOUT_QDRANT_URL/../collections/code_chunks` \
         then `cargo run --release --bin sync_project -- . <project-id>`"
            .to_string()
    } else if err_str.contains("dense tei")
        || err_str.contains("sparse tei")
        || err_str.contains("tei status")
    {
        "Embedding service (TEI) is unhealthy. Most common cause: dense \
         or sparse TEI container OOM'd or returned non-200. \
         Check: `docker logs codescout-tei-dense` and \
         `docker logs codescout-tei-sparse`. \
         Restart: `./scripts/retrieval-stack.sh up`. \
         If persistent, inspect container memory limits + model file. \
         Workaround: fall back to `grep` / `symbols` for this query while TEI recovers."
            .to_string()
    } else if err_str.contains("embed connect failed")
        || err_str.contains("openai send")
        || err_str.contains("embed sparse")
    {
        // Client-side: the query could not be embedded because the embedder
        // endpoint is unreachable/erroring. Distinct from a Qdrant fault —
        // must NOT send the operator to qdrant logs (the 2026-07-13 bug).
        "Query could not be embedded — the dense/sparse embedder is unreachable \
         or returned an error. This is an embedder problem, NOT a Qdrant issue \
         (don't check qdrant logs). Verify CODESCOUT_EMBEDDER_URL / \
         CODESCOUT_SPARSE_EMBEDDER_URL point at the running embedder, then \
         `./scripts/retrieval-stack.sh ps`."
            .to_string()
    } else if err_str.contains("embedding server") {
        // The no-url resolver path (`ollama:`/`openai:`, wired in
        // src/retrieval/client.rs::build_embedder) constructs a
        // `RemoteEmbedder` whose failures carry this wording, not
        // "embed connect failed" — same client-side class as the bucket
        // above, distinct wording, so it needs its own match.
        "The configured embedding model's server is unreachable or returned an \
         error. This is an embedder problem, NOT a Qdrant issue (don't check \
         qdrant logs). Verify [embeddings].model (or CODESCOUT_EMBEDDER_MODEL) \
         names a reachable server — e.g. `ollama list` / `OLLAMA_HOST` for an \
         `ollama:` model, or your OpenAI-compatible endpoint's status for \
         `openai:`."
            .to_string()
    } else if err_str.contains("Connection refused")
        || err_str.contains("transport error")
        || err_str.contains("tonic")
    {
        "Stack went offline mid-query. \
         Restart with `./scripts/retrieval-stack.sh up` and retry."
            .to_string()
    } else {
        "Stack reachable but query failed. \
         Check `./scripts/retrieval-stack.sh ps` and qdrant logs \
         (`docker logs codescout-qdrant`)."
            .to_string()
    }
}

#[allow(dead_code)] // re-wire when the stack search gains file-diversity capping (tracker L-15)
/// Apply a per-file cap to a score-sorted list of search results. Iterates in
/// order and keeps at most `max_per_file` entries sharing the same `file_path`;
/// later duplicates are dropped. Passing 0 disables the cap (returns input).
pub(crate) fn apply_file_diversity_cap(
    results: Vec<crate::embed::schema::SearchResult>,
    max_per_file: usize,
) -> Vec<crate::embed::schema::SearchResult> {
    if max_per_file == 0 {
        return results;
    }
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    results
        .into_iter()
        .filter(|r| {
            let count = seen.entry(r.file_path.clone()).or_insert(0);
            if *count < max_per_file {
                *count += 1;
                true
            } else {
                false
            }
        })
        .collect()
}

/// Classification of a linked worktree's index state, used by `SemanticSearch::call`'s
/// worktree branch to decide whether to query main+delta or return the
/// not-yet-indexed hint. See `classify_worktree_index_state` for the decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorktreeIndexState {
    /// No sidecar, or an empty dirty set with an empty delta: the worktree sync
    /// has simply never run. `symbols`/`grep`/`references` are still correct;
    /// only the vector index is missing.
    NotIndexed,
    /// A non-empty dirty set: the normal, healthy in-progress state. Main
    /// excludes exactly these paths; the delta supplies them.
    Healthy,
    /// HUMAN RULING: an empty dirty set but a non-empty delta. Impossible in a
    /// healthy state -- a delta only exists because something was dirty. Arises
    /// when a plain `sync_project` rewrites the sidecar with `dirty_paths: []`
    /// after a worktree sync had recorded a real set (`write_index_state`
    /// delegates to `write_index_state_with_dirty(root, &[])`). Must NOT be read
    /// as "nothing is dirty": that would query main with `exclude_paths` empty
    /// and let it silently serve every stale chunk.
    Suspect,
}

/// Pure decision at the core of the worktree branch -- see [`WorktreeIndexState`]
/// for what each variant means and why. Kept free of any store/filesystem access
/// so all meaningful combinations are testable without a live index.
/// `delta_has_chunks` only matters when `dirty_paths_empty`; callers may skip
/// that remote check otherwise (the common, non-empty case).
pub(crate) fn classify_worktree_index_state(
    sidecar_present: bool,
    dirty_paths_empty: bool,
    delta_has_chunks: bool,
) -> WorktreeIndexState {
    if !sidecar_present {
        return WorktreeIndexState::NotIndexed;
    }
    if !dirty_paths_empty {
        return WorktreeIndexState::Healthy;
    }
    if delta_has_chunks {
        WorktreeIndexState::Suspect
    } else {
        WorktreeIndexState::NotIndexed
    }
}

/// Hint returned when a linked worktree has no usable index yet
/// (`WorktreeIndexState::NotIndexed`). Names the resolved delta project id and
/// both exits available while the vector index catches up: `index(action="build")`
/// to fix it, or the filesystem-backed tools that are already correct here.
pub(crate) fn worktree_no_index_hint(delta_id: &str) -> String {
    format!(
        "No index for worktree project `{delta_id}`. A worktree's files differ from the main \
         checkout, so main's vectors are not served for changed files. Run index(action=\"build\") \
         here to index them — it only embeds what differs. `symbols`, `grep` and `references` \
         are computed from the filesystem and are already correct in this worktree."
    )
}

/// `Some(hint)` when the delta is not usable yet, `None` once it is. Split from
/// [`worktree_no_index_hint`] so both directions are testable without a live
/// store: a hint that fires unconditionally would pass a test that only checks
/// the positive direction while telling the caller nothing.
pub(crate) fn worktree_hint_for(delta_present: bool, delta_id: &str) -> Option<String> {
    if delta_present {
        None
    } else {
        Some(worktree_no_index_hint(delta_id))
    }
}

/// State-condition note for `WorktreeIndexState::Suspect` (see the human ruling on
/// that variant above). Surfaced in the response payload, never as a
/// `RecoverableError` -- this is evidence about the index's own bookkeeping, not
/// something the caller did wrong.
pub(crate) fn worktree_suspect_note(delta_id: &str) -> String {
    format!(
        "Worktree dirty-path tracking looks inconsistent for `{delta_id}`: the delta index has \
         chunks but no dirty paths are recorded, which should never happen in a healthy state (a \
         delta only exists because something was dirty). Main was queried with no path exclusions \
         below and may be serving stale chunks for files this worktree has changed. Re-run \
         index(action=\"build\") here to repair the dirty-path record."
    )
}

/// `true` when `main_last_indexed_at` parses to a later instant than
/// `worktree_last_indexed_at`. `false` on any parse failure or absence --
/// silence rather than a false claim of freshness, matching the plan's rule that
/// undetectable drift is reported as silence, not reassurance.
pub(crate) fn main_reindexed_after_worktree(
    main_last_indexed_at: &str,
    worktree_last_indexed_at: &str,
) -> bool {
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).ok();
    match (parse(main_last_indexed_at), parse(worktree_last_indexed_at)) {
        (Some(m), Some(w)) => m > w,
        _ => false,
    }
}

/// Note attached when [`main_reindexed_after_worktree`] fires. Also mentions --
/// cheaply, since this string is already being constructed -- the residual gap
/// Task 6 left open: `sync_worktree` writes its sidecar fail-soft *after*
/// upserting delta chunks, so a disk error mid-write can leave a non-empty but
/// stale `dirty_paths`. That combination isn't detectable without scrolling the
/// delta's full chunk list on every query (too expensive for a read path), so it
/// is named here rather than checked.
pub(crate) fn worktree_main_ahead_note() -> &'static str {
    "Note: the main checkout was re-indexed after this worktree's delta was built, so results \
     for unchanged files may reflect main's newer content. Re-run index(action=\"build\") here \
     to refresh. (The delta's own dirty-path record can also lag behind a partially-failed \
     sync — a non-empty dirty set is evidence of freshness, not a guarantee of it.)"
}

/// `(main_exclude_paths, delta_exclude_paths)` for the two-source worktree query.
/// Main must exclude the worktree's dirty paths -- it would otherwise serve stale
/// content for files the worktree changed; the delta must exclude nothing -- it
/// holds exactly those dirty files and nothing else. Extracted so the wiring is
/// mutation-verifiable without a live store: swapping the two would starve the
/// delta query of the very paths `sync_worktree` upserted, while main keeps
/// serving what it should have excluded.
pub(crate) fn worktree_query_exclusions(dirty_paths: &[String]) -> (Vec<String>, Vec<String>) {
    (dirty_paths.to_vec(), Vec::new())
}

pub struct SemanticSearch;

#[async_trait::async_trait]
impl Tool for SemanticSearch {
    fn name(&self) -> &str {
        "semantic_search"
    }
    fn description(&self) -> &str {
        "Find code by natural language description or code snippet. \
         Returns ranked chunks with file path, line range, and similarity score."
    }

    fn relevant_guide_topic(&self) -> Option<&str> {
        Some("progressive-disclosure")
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## When to use\n\
             \n\
             Use `semantic_search` when you know the *concept* but not the symbol name.\n\
             Examples: \"retry logic\", \"parse JWT token\", \"database connection pool\".\n\
             For known symbol names, prefer `symbols` (faster, exact).\n\
             \n\
             ## Prerequisites\n\
             \n\
             The project index must be built: run `index(action='build')` first.\n\
             Check status with `index(action='status')`.\n\
             \n\
             ## Key parameters\n\
             \n\
             - `query`: natural language or a code snippet.\n\
             - `limit`: number of results (default 10). Raise to 20-30 for broad concepts.\n\
             - `scope`: `\"project\"` (default), `\"libraries\"`, `\"all\"`, or `\"lib:<name>\"`.\n\
             - `include_memories=true`: also search semantic memories.\n\
             - `project_id`: filter to a specific workspace sub-project.\n\
             - `mode`: `\"code\"` (default) excludes markdown chunks — best for finding implementations.\n\
                       `\"full\"` includes all indexed content.\n\
             \n\
             ## Output\n\
             \n\
             Each result has `file`, `start_line`, `end_line`, and `score` (0.0–1.0).\n\
             Use `symbols` or `read_file(start_line=N, end_line=M)` to read the chunk body.\n\
             \n\
             ## Tips\n\
             \n\
             - Short, specific queries beat long prose.\n\
             - Scores below 0.3 are usually noise; re-query with a different angle.",
        )
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": { "type": "string", "description": "Natural language or code snippet to search for" },
                "limit": { "type": "integer", "default": 10 },
                "detail_level": { "type": "string", "description": "'full' for complete chunks (default: compact)" },
                "offset": { "type": "integer", "description": "Pagination offset" },
                "scope": { "type": "string", "description": "'project' (default), 'libraries', 'all', or 'lib:<name>'" },
                "include_memories": { "type": "boolean", "default": false, "description": "Also search semantic memories." },
                "project_id": { "type": "string", "description": "Filter to a workspace project ID." },
                "mode": { "type": "string", "enum": ["code", "full"], "default": "code", "description": "'code' (default) excludes markdown chunks — best for finding implementations. 'full' includes all indexed content (code + docs)." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::tools::output::OutputGuard;

        let query = crate::tools::require_str_param(&input, "query")?;
        if query.trim().is_empty() {
            return Err(crate::tools::RecoverableError::with_hint(
                    "'query' must be a non-empty string",
                    "Provide a natural-language phrase or code snippet describing what to search for (e.g. query=\"how does the embedding cache evict entries\").",
                )
                .into());
        }
        let limit = optional_u64_param(&input, "limit").unwrap_or(10) as usize;
        let _guard = OutputGuard::from_input(&input);

        // Phase 7 (narrow): stack is the only retrieval backend for code search.
        // The legacy sqlite-vec + tantivy path is gone. Memory storage and recall
        // still live on the legacy index — see
        // docs/trackers/2026-05-07-legacy-retrieval-removal.md (L-01..L-11).
        if parse_bool_param(&input["include_memories"]) {
            return Err(crate::tools::RecoverableError::with_hint(
                "include_memories is not supported by the Qdrant retrieval stack",
                "Use `memory(action=\"recall\", query=...)` for semantic memory search.",
            )
            .into());
        }
        if input
            .get("scope")
            .and_then(|v| v.as_str())
            .map(|s| s.starts_with("lib:"))
            .unwrap_or(false)
        {
            return Err(crate::tools::RecoverableError::with_hint(
                "library scope is not yet supported by the Qdrant retrieval stack",
                "Track L-12 in docs/trackers/2026-05-07-legacy-retrieval-removal.md; \
             use `symbols(name=...)` against the library project as a workaround.",
            )
            .into());
        }

        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("loading embedding model").await;
        }
        let project_id = if let Some(pid) = input.get("project_id").and_then(|v| v.as_str()) {
            pid.to_string()
        } else {
            ctx.agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.name.clone())
                    })
                    .await
                    .map_err(|_| {
                        crate::tools::RecoverableError::with_hint(
                            "No active project. Use workspace(action='activate') first.",
                            "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                        )
                    })?
        };
        let root = ctx
            .agent
            .project_root_for(ctx.workspace_override.as_deref())
            .await;
        let client = crate::retrieval::client::RetrievalClient::from_env(root.as_deref())
            .await
            .map_err(|e| {
                map_from_env_error(e, crate::retrieval::code_store::VectorBackend::resolve())
            })?;
        let opts = crate::retrieval::search::SearchOpts {
            limit,
            overfetch: limit * 2,
            rerank: true,
            exclude_languages: match input.get("mode").and_then(|v| v.as_str()).unwrap_or("code") {
                "full" => Vec::new(),
                _ => vec!["markdown".to_string()],
            },
            exclude_paths: Vec::new(),
        };
        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("searching").await;
        }

        // A linked worktree's own project id never gets a `sync_project` collection
        // -- `index(action="build")` routes a worktree root to `sync_worktree`
        // instead (Task 6), which populates `<main>@<worktree-basename>` and
        // records the worktree's dirty paths in its own
        // `.codescout/index-state.json`. So when `root` is a worktree, query main
        // (with the dirty paths excluded) plus that delta project and merge --
        // never the worktree's own `project_id`, which has nothing indexed under
        // it at all.
        //
        // Skipped entirely when the caller passed an explicit `project_id`: that
        // names a specific collection to search, and redirecting it to main+delta
        // would silently ignore the caller's choice.
        let explicit_project_id = input.get("project_id").and_then(|v| v.as_str()).is_some();
        if !explicit_project_id {
            let worktree_main_repo = root
                .as_deref()
                .and_then(crate::prompts::detect_worktree_info)
                .and_then(|info| info.main_repo);
            if let Some(main_repo) = worktree_main_repo {
                let worktree_root = root
                    .clone()
                    .expect("main_repo only resolves from a Some(root)");
                // Same derivation Task 6's sync_worktree call site uses
                // (src/tools/semantic/index.rs) -- must match exactly, or the
                // delta id computed here names a project nothing was ever
                // written under.
                let main_project_id =
                    crate::config::project::ProjectConfig::load_or_default(&main_repo)
                        .map(|c| c.project.name)
                        .unwrap_or_else(|_| {
                            main_repo
                                .file_name()
                                .map(|s| s.to_string_lossy().into_owned())
                                .unwrap_or_else(|| "main".to_string())
                        });
                let wt_dir = worktree_root
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "worktree".to_string());
                let delta_id = crate::retrieval::sync::delta_project_id(&main_project_id, &wt_dir);
                let collection = client.config.collection("code_chunks");

                let state = crate::retrieval::index_state::read_index_state(&worktree_root);
                let dirty_paths_empty = state
                    .as_ref()
                    .map(|s| s.dirty_paths.is_empty())
                    .unwrap_or(true);
                // Only worth the remote check when dirty_paths is empty -- see
                // classify_worktree_index_state: it's irrelevant otherwise.
                let delta_has_chunks = if state.is_some() && dirty_paths_empty {
                    client
                        .project_has_chunks(&collection, &delta_id)
                        .await
                        .map_err(|e| {
                            let hint = classify_search_error(&e.to_string(), &delta_id);
                            crate::tools::RecoverableError::with_hint(
                                format!("stack search failed: {e}"),
                                hint,
                            )
                        })?
                } else {
                    false
                };
                let classification = classify_worktree_index_state(
                    state.is_some(),
                    dirty_paths_empty,
                    delta_has_chunks,
                );

                if let Some(hint) =
                    worktree_hint_for(classification != WorktreeIndexState::NotIndexed, &delta_id)
                {
                    return Ok(serde_json::json!({
                        "results": [], "total": 0, "truncated": false, "hint": hint
                    }));
                }

                let state =
                    state.expect("classification other than NotIndexed implies a parsed sidecar");
                let (main_exclude, delta_exclude) = worktree_query_exclusions(&state.dirty_paths);

                let mut main_opts = opts.clone();
                main_opts.exclude_paths = main_exclude;
                let main_hits = client
                    .search_code(&main_project_id, query, main_opts)
                    .await
                    .map_err(|e| {
                        let hint = classify_search_error(&e.to_string(), &main_project_id);
                        crate::tools::RecoverableError::with_hint(
                            format!("stack search failed: {e}"),
                            hint,
                        )
                    })?;

                let mut delta_opts = opts.clone();
                delta_opts.exclude_paths = delta_exclude;
                let delta_hits = client
                    .search_code(&delta_id, query, delta_opts)
                    .await
                    .map_err(|e| {
                        let hint = classify_search_error(&e.to_string(), &delta_id);
                        crate::tools::RecoverableError::with_hint(
                            format!("stack search failed: {e}"),
                            hint,
                        )
                    })?;

                let merged = crate::retrieval::search::merge_hits(main_hits, delta_hits, limit);
                let result_items: Vec<serde_json::Value> = merged
                    .iter()
                    .map(|h| {
                        format_search_result_item(
                            &h.file_path,
                            h.start_line as usize,
                            h.end_line as usize,
                            "stack",
                            h.content.clone(),
                        )
                    })
                    .collect();
                let count = result_items.len();
                let truncated = count >= limit;
                let mut out = serde_json::json!({
                    "results": result_items, "total": count, "truncated": truncated
                });
                if truncated {
                    out["truncated_hint"] = serde_json::json!(
                            "results capped at `limit`; raise `limit` for more (ranked by relevance, so later results matter less)"
                        );
                }
                if classification == WorktreeIndexState::Suspect {
                    out["worktree_state_warning"] =
                        serde_json::json!(worktree_suspect_note(&delta_id));
                }
                if let Some(main_state) =
                    crate::retrieval::index_state::read_index_state(&main_repo)
                {
                    if main_reindexed_after_worktree(
                        &main_state.last_indexed_at,
                        &state.last_indexed_at,
                    ) {
                        out["drift_note"] = serde_json::json!(worktree_main_ahead_note());
                    }
                }
                return Ok(out);
            }
        }

        let hits = client
            .search_code(&project_id, query, opts)
            .await
            .map_err(|e| {
                let hint = classify_search_error(&e.to_string(), &project_id);
                crate::tools::RecoverableError::with_hint(format!("stack search failed: {e}"), hint)
            })?;
        let result_items: Vec<serde_json::Value> = hits
            .iter()
            .map(|h| {
                format_search_result_item(
                    &h.file_path,
                    h.start_line as usize,
                    h.end_line as usize,
                    "stack",
                    h.content.clone(),
                )
            })
            .collect();
        let count = result_items.len();
        // KNN over a large index: a full page almost always means more relevant
        // chunks exist. Signal it rather than let `total` read as complete.
        let truncated = count >= limit;
        let mut out =
            serde_json::json!({ "results": result_items, "total": count, "truncated": truncated });
        if truncated {
            out["truncated_hint"] = serde_json::json!(
                    "results capped at `limit`; raise `limit` for more (ranked by relevance, so later results matter less)"
                );
        }
        Ok(out)
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_semantic_search(result))
    }

    fn availability(&self, _caps: &crate::tools::ToolCapabilities) -> crate::tools::Availability {
        crate::tools::Availability::RequiresEmbeddings
    }
}

pub(crate) fn format_search_result_item(
    file_path: &str,
    start_line: usize,
    end_line: usize,
    source: &str,
    content: String,
) -> Value {
    // Field order: identity → location → metadata → content (bulk payload last)
    let mut map = serde_json::Map::new();
    map.insert("file_path".into(), json!(file_path));
    map.insert("start_line".into(), json!(start_line));
    map.insert("end_line".into(), json!(end_line));
    if source != "project" {
        map.insert("source".into(), json!(source));
    }
    map.insert("content".into(), json!(content));
    Value::Object(map)
}
pub(crate) fn format_semantic_search(val: &Value) -> String {
    let results = match val["results"].as_array() {
        Some(arr) => arr,
        None => return String::new(),
    };
    let total = val["total"].as_u64().unwrap_or(results.len() as u64);

    if results.is_empty() {
        return "0 results".to_string();
    }

    let result_word = if total == 1 { "result" } else { "results" };
    let mut out = format!("{total} {result_word}\n");

    // Build rows: (location, preview)
    let rows: Vec<(String, String)> = results
        .iter()
        .map(|r| {
            let file = r["file_path"].as_str().unwrap_or("?");
            let start = r["start_line"].as_u64().unwrap_or(0);
            let end = r["end_line"].as_u64().unwrap_or(0);
            let location = if start > 0 && end > 0 && start != end {
                format!("{file}:{start}-{end}")
            } else if start > 0 {
                format!("{file}:{start}")
            } else {
                file.to_string()
            };

            // Content preview: first line, truncated to ~50 chars
            let content = r["content"].as_str().unwrap_or("");
            let first_line = content.lines().next().unwrap_or("").trim();
            let preview = if first_line.chars().count() > 50 {
                let mut end = 47.min(first_line.len());
                while !first_line.is_char_boundary(end) {
                    end -= 1;
                }
                format!("{}...", &first_line[..end])
            } else {
                first_line.to_string()
            };

            (location, preview)
        })
        .collect();

    let max_loc_len = rows.iter().map(|(l, _)| l.len()).max().unwrap_or(0);

    for (location, preview) in &rows {
        out.push('\n');
        out.push_str("  ");
        out.push_str(location);
        if !preview.is_empty() {
            let loc_pad = max_loc_len - location.len();
            for _ in 0..loc_pad {
                out.push(' ');
            }
            out.push_str("  ");
            out.push_str(preview);
        }
    }

    // Git sync info (informational only — does not affect result quality)
    if val["git_sync"]["status"].as_str() == Some("behind") {
        out.push('\n');
        if let Some(n) = val["git_sync"]["behind_commits"].as_u64() {
            out.push_str(&format!(
                "\n  {n} commits not yet indexed (results still valid — run index(action='build') to include new code)"
            ));
        }
    }

    // Overflow
    if let Some(overflow) = val.get("overflow").filter(|o| o.is_object()) {
        out.push('\n');
        out.push_str(&format_overflow(overflow));
    }

    out
}

#[cfg(test)]
mod classify_search_error_tests {
    use super::classify_search_error;

    #[test]
    fn missing_collection_routes_to_sync_project_hint() {
        let err = "hybrid_query: Collection `code_chunks` doesn't exist!";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("sync_project"), "hint: {hint}");
        assert!(hint.contains("codescout"), "hint must name project: {hint}");
    }

    #[test]
    fn dim_mismatch_routes_to_drop_and_reindex_hint() {
        let err = "upsert_points: Vector dimension error: expected dim: 512, got 768";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("dim mismatch"), "hint: {hint}");
        assert!(
            hint.contains("DELETE"),
            "hint must give drop command: {hint}"
        );
        assert!(
            hint.contains("sync_project"),
            "hint must follow with reindex: {hint}"
        );
    }

    #[test]
    fn transport_error_routes_to_restart_hint() {
        let err = "tonic::transport::Error: Connection refused (os error 111)";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("offline"), "hint: {hint}");
        assert!(
            hint.contains("retrieval-stack.sh up"),
            "hint must restart: {hint}"
        );
    }

    #[test]
    fn unknown_error_routes_to_diagnostic_hint() {
        let err = "some weird unrelated failure";
        let hint = classify_search_error(err, "codescout");
        assert!(hint.contains("ps"), "fallback must check stack: {hint}");
        assert!(
            hint.contains("docker logs"),
            "fallback must point at logs: {hint}"
        );
    }

    #[test]
    fn collection_missing_takes_priority_over_transport() {
        // If both signals present, collection-missing wins (more actionable).
        let err = "Collection `code_chunks` not found via tonic transport";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("sync_project"),
            "specificity ordering: {hint}"
        );
    }

    #[test]
    fn tei_status_routes_to_embedding_service_hint() {
        // I-7: 45 of 53 'dense tei status' errors in the 2026-05-27 usage
        // analysis fell into the generic bucket because TEI didn't have its
        // own classification. New TEI bucket gives a concrete recovery path.
        let err = "stack search failed: dense tei status: HTTP 503";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("TEI") || hint.contains("tei"),
            "hint must name TEI explicitly: {hint}"
        );
        assert!(
            hint.contains("docker logs"),
            "hint must point at container logs: {hint}"
        );
        assert!(
            hint.contains("retrieval-stack.sh up"),
            "hint must give restart command: {hint}"
        );
    }

    #[test]
    fn tei_bucket_takes_priority_over_generic_fallback() {
        // A bare TEI error string that doesn't also match collection/dim/
        // transport should resolve to the new TEI hint, not the generic one.
        let err = "search_code: dense tei status (HTTP 504, upstream timeout)";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "must not hit generic fallback: {hint}"
        );
    }

    #[test]
    fn dense_connect_failure_routes_to_embedder_hint_not_qdrant() {
        // Regression: a client-side connect failure (wrong CODESCOUT_EMBEDDER_URL
        // or embedder down) used to fall through to the generic "check qdrant
        // logs" fallback, sending operators to debug a healthy stack. See
        // docs/issues/2026-07-13-semantic-search-misleading-stack-error-on-missing-env.md.
        let err = "stack search failed: dense embed connect failed: \
                   http://127.0.0.1:8081/v1/embeddings — the dense embedder is unreachable";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("CODESCOUT_EMBEDDER_URL"),
            "hint must point at the embedder URL env var: {hint}"
        );
        // The regression marker is the generic fallback's qdrant directive; our
        // hint may *mention* qdrant (to steer away from it), so match the precise
        // fallback text rather than the bare substring "qdrant logs".
        assert!(
            !hint.contains("docker logs codescout-qdrant")
                && !hint.contains("Stack reachable but query failed"),
            "must NOT route a client-side connect failure to the qdrant-logs fallback: {hint}"
        );
    }

    #[test]
    fn openai_send_does_not_hit_generic_qdrant_fallback() {
        let err = "stack search failed: dense openai send";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "a dense send failure must route to the embedder bucket, not the generic fallback: {hint}"
        );
    }

    #[test]
    fn resolver_path_retry_exhaustion_routes_to_embedder_hint_not_qdrant() {
        // RemoteEmbedder's retry-exhaustion wording (reached via the no-url
        // resolver path an `ollama:`/`openai:` model takes, `remote.rs`'s
        // `anyhow!("embedding server unavailable after {MAX_RETRIES}
        // attempts")`) is distinct from `EmbedderHttp`'s "embed connect
        // failed" and must not fall through to the generic qdrant-oriented
        // fallback — same misrouting class as the 2026-07-13 bug.
        let err = "could not build the 'ollama:nomic-embed-text' embedder: \
                   embedding server unavailable after 3 attempts";
        let hint = classify_search_error(err, "codescout");
        assert!(
            hint.contains("[embeddings].model") || hint.contains("CODESCOUT_EMBEDDER_MODEL"),
            "hint must point at the model config: {hint}"
        );
        assert!(
            !hint.contains("docker logs codescout-qdrant")
                && !hint.contains("Stack reachable but query failed"),
            "must NOT route a resolver-path failure to the qdrant-logs fallback: {hint}"
        );
    }

    #[test]
    fn resolver_path_http_status_failure_does_not_hit_generic_fallback() {
        let err = "could not build the 'openai:text-embedding-3-small' embedder: \
                   HTTP 401 from embedding server: invalid api key";
        let hint = classify_search_error(err, "codescout");
        assert!(
            !hint.contains("Stack reachable but query failed"),
            "an embedding-server HTTP failure must route to the resolver-path \
             embedder bucket, not the generic fallback: {hint}"
        );
    }
}

#[cfg(test)]
mod map_from_env_error_tests {
    use super::map_from_env_error;
    use crate::retrieval::code_store::VectorBackend;
    use crate::tools::RecoverableError;

    #[test]
    fn a_recoverable_error_passes_through_without_the_stack_offline_headline() {
        // I3(b): a `RecoverableError` from `from_env` (e.g. `build_embedder`'s
        // `guard_sparse` sparse-conflict guard) must reach the caller with its
        // own message intact, not relabelled "retrieval stack offline" — that
        // headline points at a restart script that cannot fix a config
        // conflict.
        let original = RecoverableError::with_hint(
            "the local embedding backend produces no sparse vector, but the \
             hybrid sparse leg is enabled.",
            "Either set CODESCOUT_DISABLE_SPARSE=1 to run dense-only, or \
             configure an embedder url that serves both dense and sparse.",
        );
        let original_msg = original.to_string();
        let mapped = map_from_env_error(original.into(), VectorBackend::Qdrant);
        assert!(
            mapped.downcast_ref::<RecoverableError>().is_some(),
            "must remain a RecoverableError: {mapped}"
        );
        assert_eq!(
            mapped.to_string(),
            original_msg,
            "must pass through unchanged, not acquire a new headline"
        );
        assert!(
            !mapped.to_string().contains("retrieval stack offline"),
            "a config conflict must not be reported as a down service: {mapped}"
        );
    }

    #[test]
    fn a_non_recoverable_error_gets_the_stack_offline_headline() {
        let original = anyhow::anyhow!("connection refused");
        let mapped = map_from_env_error(original, VectorBackend::Qdrant);
        assert!(
            mapped.to_string().contains("retrieval stack offline"),
            "a genuine connect failure must still get the stack-offline headline: {mapped}"
        );
        assert!(
            mapped.downcast_ref::<RecoverableError>().is_some(),
            "must still be RecoverableError so sibling calls survive: {mapped}"
        );
    }
}

#[cfg(test)]
mod worktree_search_tests {
    use super::*;

    #[test]
    fn worktree_hint_names_the_delta_project_and_both_exits() {
        let h = worktree_no_index_hint("codescout@wt");
        assert!(
            h.contains("codescout@wt"),
            "hint must name the resolved project id"
        );
        assert!(h.contains("index(action=\"build\")"));
        assert!(
            h.contains("grep"),
            "hint must offer the tools that ARE correct here"
        );
    }

    #[test]
    fn no_hint_when_the_delta_is_indexed() {
        // The negative direction. A hint that fires unconditionally passes the
        // positive test while telling the user nothing.
        assert!(worktree_hint_for(/* delta present */ true, "codescout@wt").is_none());
    }

    #[test]
    fn hint_fires_when_the_delta_is_not_present() {
        let hint = worktree_hint_for(/* delta present */ false, "codescout@wt");
        assert_eq!(hint, Some(worktree_no_index_hint("codescout@wt")));
    }

    /// Exhaustive over the meaningful combinations (sidecar-absent collapses the
    /// other two dimensions). HUMAN RULING: `(sidecar=true, dirty_empty=true,
    /// delta_has_chunks=true)` is `Suspect`, never read as "nothing is dirty" --
    /// that combination is impossible in a healthy state (a delta only exists
    /// because something was dirty).
    #[test]
    fn classify_worktree_index_state_covers_every_combination() {
        use WorktreeIndexState::*;
        assert_eq!(
            classify_worktree_index_state(false, true, true),
            NotIndexed,
            "no sidecar at all, regardless of the delta"
        );
        assert_eq!(
            classify_worktree_index_state(false, true, false),
            NotIndexed
        );
        assert_eq!(
            classify_worktree_index_state(false, false, true),
            NotIndexed,
            "sidecar-absent short-circuits before dirty_paths is even consulted"
        );
        assert_eq!(
            classify_worktree_index_state(true, false, false),
            Healthy,
            "a non-empty dirty set is the normal, healthy in-progress state"
        );
        assert_eq!(
            classify_worktree_index_state(true, false, true),
            Healthy,
            "non-empty dirty set stays healthy even when the delta also has chunks"
        );
        assert_eq!(
            classify_worktree_index_state(true, true, false),
            NotIndexed,
            "empty dirty set + empty delta = the worktree sync has simply not run yet"
        );
        assert_eq!(
            classify_worktree_index_state(true, true, true),
            Suspect,
            "HUMAN RULING: empty dirty set + non-empty delta is impossible in a healthy state"
        );
    }

    #[test]
    fn main_reindexed_after_worktree_flags_when_main_is_strictly_newer() {
        assert!(main_reindexed_after_worktree(
            "2026-08-13T10:00:00+00:00",
            "2026-08-13T09:00:00+00:00",
        ));
        assert!(!main_reindexed_after_worktree(
            "2026-08-13T09:00:00+00:00",
            "2026-08-13T10:00:00+00:00",
        ));
        assert!(
            !main_reindexed_after_worktree(
                "2026-08-13T09:00:00+00:00",
                "2026-08-13T09:00:00+00:00",
            ),
            "equal timestamps are not 'after' -- must be strictly greater"
        );
    }

    #[test]
    fn main_reindexed_after_worktree_is_false_on_unparseable_timestamps() {
        // Undetectable drift is reported as silence, never as a false claim of
        // freshness.
        assert!(!main_reindexed_after_worktree(
            "not-a-timestamp",
            "2026-08-13T09:00:00+00:00",
        ));
        assert!(!main_reindexed_after_worktree(
            "2026-08-13T09:00:00+00:00",
            "",
        ));
    }

    #[test]
    fn worktree_query_exclusions_puts_dirty_paths_on_main_only() {
        let dirty = vec!["src/a.rs".to_string(), "src/b.rs".to_string()];
        let (main_exclude, delta_exclude) = worktree_query_exclusions(&dirty);
        assert_eq!(
            main_exclude, dirty,
            "main must exclude the worktree's dirty paths"
        );
        assert!(
            delta_exclude.is_empty(),
            "delta must exclude nothing -- it holds exactly the dirty files"
        );
    }
}
