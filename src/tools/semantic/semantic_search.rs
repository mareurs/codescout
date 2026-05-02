//! SemanticSearch tool — vector + BM25 hybrid search.

use super::super::format::format_overflow;
use super::super::{optional_u64_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};
/// Default cap on how many chunks from the same file may appear in semantic
/// search results, to preserve file-level diversity in the top-K. Set to 0 to
/// disable. See docs/manual/src/experimental/file-diversity-rerank.md.
const MAX_CHUNKS_PER_FILE: usize = 3;

use std::collections::HashMap;

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
                "project_id": { "type": "string", "description": "Filter to a workspace project ID." }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        use crate::tools::output::OutputGuard;

        let query = crate::tools::require_str_param(&input, "query")?;
        let limit = optional_u64_param(&input, "limit").unwrap_or(10) as usize;
        // Overfetch so the file-diversity cap can drop same-file duplicates
        // without starving the requested limit.
        let search_limit = limit.saturating_mul(MAX_CHUNKS_PER_FILE.max(1)).max(limit);
        let include_memories = parse_bool_param(&input["include_memories"]);
        let project_filter = input
            .get("project_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let guard = OutputGuard::from_input(&input);

        let (root, model, library_registry) = {
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
                p.library_registry.clone(),
            )
        };

        let scope = crate::library::scope::Scope::parse(input["scope"].as_str());

        // Gate: check source_available for explicit library scope
        if let crate::library::scope::Scope::Library(ref lib_name) = scope {
            if let Some(entry) = library_registry.lookup(lib_name) {
                if !entry.source_available {
                    return Err(crate::tools::RecoverableError::with_hint(
                        format!(
                            "Library '{}' source code is not available locally — cannot search.",
                            lib_name
                        ),
                        format!(
                            "Download sources, then index(action='build', scope='lib:{}') before searching.",
                            lib_name
                        ),
                    )
                    .into());
                }
            }
        }

        // Async: cached embedder + HTTP embed
        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("loading embedding model").await;
        }
        let embedder = ctx.agent.get_or_create_embedder(&model).await?;
        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("searching").await;
        }
        let query_embedding = codescout_embed::embed_one(embedder.as_ref(), query).await?;

        // Drain files written by tools in this session and re-embed them before searching.
        let dirty_paths = ctx.agent.drain_dirty_files().await;
        if !dirty_paths.is_empty() {
            match crate::embed::index::reindex_files(&root, &dirty_paths, &embedder).await {
                Ok(n) => {
                    if n > 0 {
                        tracing::info!("auto-reindexed {} file(s) before semantic_search", n);
                    }
                }
                Err(e) => {
                    tracing::warn!("auto-reindex failed, search proceeds on stale data: {e}");
                    // Re-insert so next search can retry
                    for path in dirty_paths {
                        ctx.agent.mark_file_dirty(path).await;
                    }
                }
            }
        }

        // Sync SQLite off async runtime
        let root2 = root.clone();
        let model2 = model.clone();
        let scope2 = scope.clone();
        let library_registry2 = library_registry.clone();
        let query2 = query.to_string();
        let (results, memory_results, staleness) = tokio::task::spawn_blocking(move || {
            // Guard: catch model/dimension mismatch before sqlite-vec sees the
            // wrong-dimensioned query vector and emits a cryptic error.
            // Only applicable for project-scoped searches (library DBs may use
            // a different model and have their own dimension validation).
            if matches!(scope2, crate::library::scope::Scope::Project) {
                let conn = crate::embed::index::open_db(&root2)?;
                crate::embed::index::check_model_mismatch(&conn, &model2).map_err(|e| {
                    crate::tools::RecoverableError::with_hint(
                        e.to_string(),
                        "Run index(action='build', force=true) to rebuild the index with the current model.",
                    )
                })?;
            }
            let vector_results = crate::embed::index::search_multi_db(
                &root2,
                &query_embedding,
                search_limit,
                &scope2,
                &library_registry2,
                project_filter.as_deref(),
            )?;

            // BM25 leg — project scope only; other scopes fall back to pure vector
            let bm25_results = if matches!(scope2, crate::library::scope::Scope::Project) {
                match crate::embed::bm25::BM25Index::open(&root2)? {
                    Some(idx) => idx.search(&query2, search_limit).unwrap_or_else(|e| {
                        tracing::warn!(
                            "BM25 search failed, falling back to pure vector: {e}"
                        );
                        vec![]
                    }),
                    None => vec![],
                }
            } else {
                vec![]
            };

            // RRF fusion: re-rank when BM25 has results, else preserve vector order
            let results = if bm25_results.is_empty() {
                vector_results
            } else {
                let fused_ids =
                    crate::embed::fusion::rrf_fuse(&vector_results, &bm25_results, 60.0);

                // Compute RRF score per chunk_id so BM25-only hits get a meaningful
                // score instead of 0.0 (which would push them to the bottom of the
                // post-spawn_blocking sort-by-score step).
                let rrf_scores: HashMap<u64, f32> = fused_ids
                    .iter()
                    .enumerate()
                    .map(|(i, &id)| (id, 1.0 / (60.0 + (i + 1) as f32)))
                    .collect();

                // Build lookup from vector results (already have full data)
                let mut sr_map: HashMap<u64, crate::embed::schema::SearchResult> =
                    vector_results.into_iter().map(|r| (r.id, r)).collect();

                // Fetch BM25-only hits that vector search didn't return
                let bm25_only: Vec<u64> = fused_ids
                    .iter()
                    .filter(|id| !sr_map.contains_key(id))
                    .copied()
                    .collect();
                if !bm25_only.is_empty() {
                    let conn2 = crate::embed::index::open_db(&root2)?;
                    let placeholders = bm25_only
                        .iter()
                        .enumerate()
                        .map(|(i, _)| format!("?{}", i + 1))
                        .collect::<Vec<_>>()
                        .join(",");
                    let sql = format!(
                        "SELECT id, file_path, language, content, start_line, \
                         end_line, source, project_id FROM chunks WHERE id IN ({})",
                        placeholders
                    );
                    let mut stmt = conn2.prepare(&sql)?;
                    let params =
                        rusqlite::params_from_iter(bm25_only.iter().map(|id| *id as i64));
                    let rows = stmt.query_map(params, |row| {
                        let chunk_id = row.get::<_, i64>(0)? as u64;
                        Ok(crate::embed::schema::SearchResult {
                            id: chunk_id,
                            file_path: row.get(1)?,
                            language: row.get(2)?,
                            content: row.get(3)?,
                            start_line: row.get::<_, i64>(4)? as usize,
                            end_line: row.get::<_, i64>(5)? as usize,
                            source: row.get(6)?,
                            score: rrf_scores.get(&chunk_id).copied().unwrap_or(0.0),
                            project_id: row.get(7)?,
                        })
                    })?;
                    for row in rows.flatten() {
                        sr_map.insert(row.id, row);
                    }
                }

                // Reconstruct Vec<SearchResult> in fused order
                fused_ids
                    .into_iter()
                    .filter_map(|id| sr_map.remove(&id))
                    .collect()
            };

            let results = apply_file_diversity_cap(results, MAX_CHUNKS_PER_FILE);
            let results: Vec<_> = results.into_iter().take(limit).collect();
            let memory_results = if include_memories {
                let conn = crate::embed::index::open_db(&root2)?;
                crate::embed::index::ensure_vec_memories(&conn)?;
                crate::embed::index::search_memories(&conn, &query_embedding, None, limit)?
            } else {
                vec![]
            };
            let staleness = {
                let conn = crate::embed::index::open_db(&root2)?;
                crate::embed::index::check_index_staleness(&conn, &root2).ok()
            };
            anyhow::Ok((results, memory_results, staleness))
        })
        .await??;

        // Transform code results based on mode; keep score alongside for sorting
        let mut scored_items: Vec<(f32, Value)> = results
            .iter()
            .map(|r| {
                let content_field = if guard.should_include_body() {
                    // Focused mode: full content
                    r.content.clone()
                } else {
                    // Exploring mode: first line only, max 50 chars (matches text compact format)
                    let first_line = r.content.lines().next().unwrap_or("").trim();
                    let char_count = first_line.chars().count();
                    if char_count > 50 {
                        let truncated: String = first_line.chars().take(47).collect();
                        format!("{}...", truncated)
                    } else {
                        first_line.to_string()
                    }
                };
                (
                    r.score,
                    format_search_result_item(
                        &r.file_path,
                        r.start_line,
                        r.end_line,
                        &r.source,
                        content_field,
                    ),
                )
            })
            .collect();

        // Merge memory results if requested
        for mr in &memory_results {
            let content_field = if guard.should_include_body() {
                mr.content.clone()
            } else {
                let first_line = mr.content.lines().next().unwrap_or("").trim();
                if first_line.len() > 50 {
                    format!("{}...", &first_line[..47])
                } else {
                    first_line.to_string()
                }
            };
            scored_items.push((
                mr.similarity,
                format_search_result_item(
                    &format!("[memory:{}]", mr.title),
                    0,
                    0,
                    "memory",
                    content_field,
                ),
            ));
        }

        // Sort by score descending (high first), then discard scores — order is the signal
        scored_items
            .sort_by(|(sa, _), (sb, _)| sb.partial_cmp(sa).unwrap_or(std::cmp::Ordering::Equal));
        let result_items: Vec<Value> = scored_items.into_iter().map(|(_, item)| item).collect();

        // Apply pagination/capping
        let (result_items, overflow) = guard.cap_items(
            result_items,
            "Use detail_level='full' with offset for pagination",
        );
        let total = overflow.as_ref().map_or(result_items.len(), |o| o.total);
        let mut result = json!({ "results": result_items, "total": total });
        if let Some(ov) = overflow {
            result["overflow"] = OutputGuard::overflow_json(&ov);
        }
        // Check index freshness — framed as informational, not a quality signal
        if let Some(staleness) = staleness {
            if staleness.stale {
                result["git_sync"] = json!({
                    "status": "behind",
                    "behind_commits": staleness.behind_commits,
                    "note": "Recent commits not yet indexed — run index(action='build') to include new code. Existing results are unaffected."
                });
            }
        }
        Ok(result)
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
            let preview = if first_line.len() > 50 {
                format!("{}...", &first_line[..47])
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
