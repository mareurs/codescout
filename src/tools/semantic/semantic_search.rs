//! SemanticSearch tool — vector + BM25 hybrid search.

use super::super::format::format_overflow;
use super::super::{optional_u64_param, parse_bool_param, Tool, ToolContext};
use serde_json::{json, Value};

/// Map a qdrant/search error string to an actionable recovery hint.
///
/// Patterns are checked in order of specificity: collection-missing first
/// (most common after first-time setup), then dim-mismatch (model/index
/// drift), then TEI errors (dense embedding service unhealthy), then
/// transport (stack went away), then a generic fallback.
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
            let inner = ctx.agent.inner.read().await;
            let p = inner.active_project().ok_or_else(|| {
                crate::tools::RecoverableError::with_hint(
                    "No active project. Use workspace(action='activate') first.",
                    "Call workspace(action='activate', path=\"/path/to/project\") to set the active project.",
                )
            })?;
            p.config.project.name.clone()
        };
        let client = crate::retrieval::client::RetrievalClient::from_env()
            .await
            .map_err(|e| {
                crate::tools::RecoverableError::with_hint(
                    format!("retrieval stack offline: {e}"),
                    "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack.",
                )
            })?;
        let opts = crate::retrieval::search::SearchOpts {
            limit,
            overfetch: limit * 2,
            rerank: true,
            exclude_languages: match input.get("mode").and_then(|v| v.as_str()).unwrap_or("code") {
                "full" => Vec::new(),
                _ => vec!["markdown".to_string()],
            },
        };
        if let Some(p) = ctx.progress.as_ref() {
            p.report_text("searching").await;
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
        let total = result_items.len();
        Ok(serde_json::json!({ "results": result_items, "total": total }))
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
}
