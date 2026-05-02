//! Memory tools: persistent per-project knowledge store.

use std::collections::{HashMap, HashSet};

use super::{parse_bool_param, RecoverableError, Tool, ToolContext};
use serde_json::{json, Value};

#[cfg(test)]
pub(crate) struct WriteMemory;
#[cfg(test)]
pub(crate) struct ReadMemory;
#[cfg(test)]
pub(crate) struct ListMemories;
#[cfg(test)]
pub(crate) struct DeleteMemory;

#[cfg(test)]
#[async_trait::async_trait]
impl Tool for WriteMemory {
    fn name(&self) -> &str {
        "write_memory"
    }
    fn description(&self) -> &str {
        "Persist a piece of knowledge about the project. \
         Topic is a path-like string, e.g. 'debugging/async-patterns'."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["topic", "content"],
            "properties": {
                "topic": { "type": "string" },
                "content": { "type": "string" },
                "private": {
                    "type": "boolean",
                    "description": "If true, write to the gitignored private store (personal/machine-specific notes, not shared with teammates)."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let topic = super::require_str_param(&input, "topic")?;
        let content = super::require_str_param(&input, "content")?;
        let private = parse_bool_param(&input["private"]);
        ctx.agent
            .with_project(|p| {
                if private {
                    p.private_memory.write(topic, content)?;
                } else {
                    p.memory.write(topic, content)?;
                }
                Ok(json!("ok"))
            })
            .await
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl Tool for ReadMemory {
    fn name(&self) -> &str {
        "read_memory"
    }
    fn description(&self) -> &str {
        "Read a stored memory entry by topic."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["topic"],
            "properties": {
                "topic": { "type": "string" },
                "private": {
                    "type": "boolean",
                    "description": "If true, read from the private memory store."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let topic = super::require_str_param(&input, "topic")?;
        let private = parse_bool_param(&input["private"]);
        ctx.agent
            .with_project(|p| {
                let store = if private {
                    &p.private_memory
                } else {
                    &p.memory
                };
                match store.read(topic)? {
                    Some(content) => Ok(json!({ "content": content })),
                    None => Err(RecoverableError::with_hint(
                        format!("topic '{}' not found", topic),
                        "Use list_memories to see available topics",
                    )
                    .into()),
                }
            })
            .await
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_read_memory(result))
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl Tool for ListMemories {
    fn name(&self) -> &str {
        "list_memories"
    }
    fn description(&self) -> &str {
        "List all stored memory topics for the active project. \
         Pass include_private: true to also see private topics."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "include_private": {
                    "type": "boolean",
                    "description": "If true, also list private memory topics. Returns { shared, private } instead of { topics }."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let include_private = parse_bool_param(&input["include_private"]);
        ctx.agent
            .with_project(|p| {
                if include_private {
                    let shared = p.memory.list()?;
                    let private = p.private_memory.list()?;
                    Ok(json!({ "shared": shared, "private": private }))
                } else {
                    let topics = p.memory.list()?;
                    Ok(json!({ "topics": topics }))
                }
            })
            .await
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        Some(format_list_memories(result))
    }
}

fn format_read_memory(result: &Value) -> String {
    result["content"].as_str().unwrap_or("").to_string()
}

fn format_list_memories(result: &Value) -> String {
    // include_private=true path: { shared: [...], private: [...] }
    if let (Some(shared), Some(private)) =
        (result["shared"].as_array(), result["private"].as_array())
    {
        let mut out = format!("{} shared, {} private", shared.len(), private.len());
        for t in shared {
            if let Some(name) = t.as_str() {
                out.push_str(&format!("\n  {name}"));
            }
        }
        if !private.is_empty() {
            out.push_str("\n  -- private --");
            for t in private {
                if let Some(name) = t.as_str() {
                    out.push_str(&format!("\n  {name}"));
                }
            }
        }
        return out;
    }
    // Default path: { topics: [...] }
    let topics = match result["topics"].as_array() {
        Some(t) if !t.is_empty() => t,
        _ => return "0 topics".to_string(),
    };
    let mut out = format!("{} topics", topics.len());
    for topic in topics.iter() {
        if let Some(name) = topic.as_str() {
            out.push_str(&format!("\n  {name}"));
        }
    }
    out
}

#[cfg(test)]
#[async_trait::async_trait]
impl Tool for DeleteMemory {
    fn name(&self) -> &str {
        "delete_memory"
    }
    fn description(&self) -> &str {
        "Delete a memory entry by topic."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["topic"],
            "properties": {
                "topic": { "type": "string" },
                "private": {
                    "type": "boolean",
                    "description": "If true, delete from the private memory store."
                }
            }
        })
    }
    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let topic = super::require_str_param(&input, "topic")?;
        let private = parse_bool_param(&input["private"]);
        ctx.agent
            .with_project(|p| {
                let memories_dir = if private {
                    p.private_memory.delete(topic)?;
                    p.private_memory.dir().to_path_buf()
                } else {
                    p.memory.delete(topic)?;
                    p.memory.dir().to_path_buf()
                };
                // Also clean up the anchor sidecar — without this, stale
                // sidecars accumulate and continue to show up in anchor
                // enumeration (check_all_memories).
                let sidecar = crate::memory::anchors::anchor_path_for_topic(&memories_dir, topic);
                if sidecar.exists() {
                    if let Err(e) = std::fs::remove_file(&sidecar) {
                        tracing::warn!(
                            "failed to remove anchor sidecar {}: {}",
                            sidecar.display(),
                            e,
                        );
                    }
                }
                Ok(json!("ok"))
            })
            .await
    }
}

pub struct Memory;

fn extract_title(content: &str) -> String {
    let first_sentence_end = content
        .find(". ")
        .or_else(|| content.find(".\n"))
        .map(|i| i + 1)
        .unwrap_or(content.len());
    let end = first_sentence_end.min(80).min(content.len());
    // Use safe_truncate to avoid panicking on multi-byte char boundaries
    let truncated = crate::tools::safe_truncate(content, end);
    let mut title = truncated.to_string();
    if end < content.len() && !title.ends_with('.') {
        title.push_str("...");
    }
    title
}

/// Best-effort cross-embed a markdown memory into the semantic store.
/// Called on `write` so that structured memories are also discoverable via `recall`.
async fn cross_embed_memory(ctx: &ToolContext, topic: &str, content: &str) -> anyhow::Result<()> {
    let (root, model) = {
        let inner = ctx.agent.inner.read().await;
        let p = inner
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("no project"))?;
        (p.root.clone(), p.config.embeddings.model.clone())
    };

    let embedder = ctx.agent.get_or_create_embedder(&model).await?;
    let embedding = codescout_embed::embed_one(embedder.as_ref(), content).await?;

    let topic_owned = topic.to_string();
    let content_owned = content.to_string();
    tokio::task::spawn_blocking(move || {
        let conn = crate::embed::index::open_db(&root)?;
        crate::embed::index::ensure_vec_memories(&conn)?;
        crate::embed::index::upsert_memory_by_title(
            &conn,
            "structured",
            &topic_owned,
            &content_owned,
            &embedding,
        )?;
        anyhow::Ok(())
    })
    .await??;
    Ok(())
}

/// Create semantic anchors for a markdown memory by embedding it and finding
/// similar code chunks. Excludes files already covered by path anchors.
async fn create_semantic_anchors(
    ctx: &ToolContext,
    topic: &str,
    content: &str,
    path_anchor_files: &HashSet<String>,
) -> anyhow::Result<()> {
    let (root, model, min_sim, top_n) = {
        let inner = ctx.agent.inner.read().await;
        let p = inner
            .active_project()
            .ok_or_else(|| anyhow::anyhow!("no project"))?;
        (
            p.root.clone(),
            p.config.embeddings.model.clone(),
            p.config.memory.semantic_anchor_min_similarity,
            p.config.memory.semantic_anchor_top_n,
        )
    };

    let embedder = ctx.agent.get_or_create_embedder(&model).await?;
    let embedding = codescout_embed::embed_one(embedder.as_ref(), content).await?;

    let path_anchors = path_anchor_files.clone();
    let topic_owned = topic.to_string();
    tokio::task::spawn_blocking(move || {
        let conn = crate::embed::index::open_db(&root)?;
        crate::embed::index::ensure_memory_anchors(&conn)?;

        // Delete old semantic anchors for this memory
        crate::embed::index::delete_semantic_anchors(&conn, "markdown", &topic_owned)?;

        // Search for similar code chunks
        let results = crate::embed::index::search(&conn, &embedding, top_n)?;

        // Deduplicate by file, keep highest similarity
        let mut best_per_file: HashMap<String, (f32, String)> = HashMap::new();
        for r in &results {
            if r.score < min_sim {
                continue;
            }
            if path_anchors.contains(&r.file_path) {
                continue;
            }
            let hash = crate::embed::index::get_file_hash(&conn, &r.file_path)?.unwrap_or_default();
            best_per_file
                .entry(r.file_path.clone())
                .and_modify(|(old_sim, _)| {
                    if r.score > *old_sim {
                        *old_sim = r.score;
                    }
                })
                .or_insert((r.score, hash));
        }

        for (file_path, (sim, hash)) in &best_per_file {
            crate::embed::index::insert_semantic_anchor(
                &conn,
                "markdown",
                &topic_owned,
                file_path,
                hash,
                *sim,
            )?;
        }
        anyhow::Ok(())
    })
    .await??;
    Ok(())
}

/// Resolve the memory directory for a `memory` tool call.
///
/// If the `project` parameter is provided, route to the per-project directory via
/// `Workspace::memory_dir_for_project`. Otherwise use the focused project's memory dir.
/// Falls back gracefully when no workspace is loaded.
async fn resolve_memory_dir(
    input: &Value,
    ctx: &ToolContext,
) -> anyhow::Result<std::path::PathBuf> {
    let project_param = input.get("project_id").and_then(|v| v.as_str());
    let inner = ctx.agent.inner.read().await;
    if let Some(ws) = inner.workspace.as_ref() {
        let project_id = project_param
            .map(|s| s.to_string())
            .or_else(|| ws.focused.clone())
            .unwrap_or_else(|| crate::workspace::ROOT_PROJECT_ID.to_string());
        Ok(ws.memory_dir_for_project(&project_id))
    } else {
        // No workspace — fall back to the active project's memory dir.
        let p = inner.active_project().ok_or_else(|| {
            super::RecoverableError::with_hint(
                "No active project.",
                "Call workspace(action='activate') first.",
            )
        })?;
        Ok(p.memory.dir().to_path_buf())
    }
}

/// Apply `sections` filtering to memory content and produce the JSON response value.
///
/// - If `sections` is empty, returns `content` unchanged (no filtering).
/// - If filtering is active and nothing matched, returns a `RecoverableError`.
/// - Handles the inline-vs-buffer threshold; uses a `@`-prefixed synthetic path
///   when buffering filtered content so `store_file` does not stat a missing file
///   and evict the entry immediately.
fn apply_sections_filter(
    content: String,
    topic: &str,
    sections: &[String],
    output_buffer: &std::sync::Arc<crate::tools::output_buffer::OutputBuffer>,
) -> anyhow::Result<serde_json::Value> {
    let (content, missing) = if sections.is_empty() {
        (content, vec![])
    } else {
        let section_refs: Vec<&str> = sections.iter().map(String::as_str).collect();
        let result = crate::memory::filter::filter_sections(&content, &section_refs);
        if !result.matched {
            let hint = if result.available.is_empty() {
                "this memory has no ### sections to filter".to_string()
            } else {
                format!("available sections: {}", result.available.join(", "))
            };
            return Err(RecoverableError::with_hint("no sections matched", hint).into());
        }
        (result.content, result.missing)
    };

    let value = if crate::tools::exceeds_inline_limit(&content) {
        let total_lines = content.lines().count();
        // Use a `@`-prefixed synthetic path: store_file sets source_path=None for
        // paths starting with '@', preventing get_with_refresh_flag from stat-ing
        // a non-existent file and immediately evicting the entry.
        let synthetic_path = format!("@memory:{topic}:filtered");
        let file_id = output_buffer.store_file(synthetic_path, content);
        if missing.is_empty() {
            json!({ "file_id": file_id, "total_lines": total_lines })
        } else {
            json!({ "file_id": file_id, "total_lines": total_lines, "missing": missing })
        }
    } else if missing.is_empty() {
        json!({ "content": content })
    } else {
        json!({ "content": content, "missing": missing })
    };

    Ok(value)
}

#[async_trait::async_trait]
impl Tool for Memory {
    fn name(&self) -> &str {
        "memory"
    }

    fn is_write(&self, input: &Value) -> bool {
        // Dispatched by the `action` field. These mutate the memory store;
        // read|list|recall|dump bypass the write lock.
        input
            .get("action")
            .and_then(|v| v.as_str())
            .map(|a| {
                matches!(
                    a,
                    "write" | "remember" | "forget" | "delete" | "refresh_anchors"
                )
            })
            .unwrap_or(false)
    }

    fn description(&self) -> &str {
        "Persistent project memory. Topic-based: read/write/list/delete with path-like keys. \
         Semantic: remember/recall/forget with bucket classification and meaning-based search."
    }

    fn long_docs(&self) -> Option<&str> {
        Some(
            "## Two memory systems\n\
             \n\
             **Topic-based** (structured, Markdown files on disk):\n\
             - `action=\"write\"`: save knowledge with a path-like topic key.\n\
             - `action=\"read\"`: retrieve by exact topic.\n\
             - `action=\"list\"`: list all topics.\n\
             - `action=\"delete\"`: remove a topic.\n\
             \n\
             **Semantic** (embedded, meaning-based search):\n\
             - `action=\"remember\"`: embed and store a free-text fact.\n\
             - `action=\"recall\"`: search by meaning (natural language query).\n\
             - `action=\"forget\"`: remove a semantic memory entry.\n\
             \n\
             ## Topic naming\n\
             \n\
             Topics are path-like strings: `\"architecture/overview\"`, `\"debugging/async-patterns\"`.\n\
             Nested topics appear as sections in the memory resource.\n\
             \n\
             ## Sections filter\n\
             \n\
             Pass `sections=[\"### Heading\"]` when reading to get only matching `###` blocks.\n\
             \n\
             ## Private memories\n\
             \n\
             `private=true` routes to a gitignored store for machine-specific notes.\n\
             \n\
             ## Cross-project\n\
             \n\
             Pass `project_id` in a workspace to target a specific sub-project's memory.",
        )
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["action"],
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["read", "write", "list", "delete", "remember", "recall", "forget", "refresh_anchors"]
                },
                "topic": {
                    "type": "string",
                    "description": "For read/write/delete/refresh_anchors. Path-like key, e.g. 'architecture'."
                },
                "sections": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "For read. Return only the listed ### headings (case-insensitive). E.g. [\"Rust\", \"TypeScript\"]. Omit to return full content."
                },
                "content": { "type": "string", "description": "For write or remember." },
                "private": { "type": "boolean", "default": false, "description": "Use gitignored private store." },
                "include_private": { "type": "boolean", "default": false, "description": "For list: include private topics." },
                "title": { "type": "string", "description": "For remember. Short label (auto-extracted if omitted)." },
                "bucket": {
                    "type": "string",
                    "enum": ["code", "system", "preferences", "unstructured"],
                    "description": "For remember (always specify) or recall (optional filter)."
                },
                "query": { "type": "string", "description": "For recall. Search query." },
                "limit": { "type": "integer", "description": "For recall. Max results (default 5)." },
                "id": { "type": "integer", "description": "For forget. Memory ID to delete." },
                "project_id": { "type": "string", "description": "Scope to a workspace project ID. Default: focused project." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        let action = super::require_str_param(&input, "action")?;
        match action {
            "write" => {
                let topic = super::require_str_param(&input, "topic")?;
                let content = super::require_str_param(&input, "content")?;
                let private = parse_bool_param(&input["private"]);

                // Write markdown file — route to per-project dir when `project` param given.
                if private {
                    ctx.agent
                        .with_project(|p| {
                            p.private_memory.write(topic, content)?;
                            Ok(())
                        })
                        .await?;
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    crate::memory::MemoryStore::from_dir(memories_dir)?.write(topic, content)?;
                }

                // Collect non-fatal side-effect failures so the caller has a
                // chance to see them. Cross-embed / anchor indexing are
                // best-effort but the user explicitly asked for "memory write"
                // — silent degradation there is data loss from their POV.
                let mut warnings: Vec<String> = Vec::new();

                // Cross-embed into semantic store (best-effort, non-fatal)
                if !private {
                    if let Err(e) = cross_embed_memory(ctx, topic, content).await {
                        tracing::warn!("cross-embed memory failed (non-fatal): {e}");
                        warnings.push(format!("cross-embed failed: {e}"));
                    }
                }

                // Seed/merge path anchors (best-effort, non-fatal)
                if !private {
                    if let Ok(root) = ctx.agent.require_project_root().await {
                        let memories_dir = resolve_memory_dir(&input, ctx).await.unwrap_or_else(
                            |_| root.join(".codescout").join("memories"),
                        );
                        if let Err(e) = crate::memory::anchors::update_anchors_on_write(
                            &root, &memories_dir, topic, content,
                        ) {
                            tracing::warn!("anchor update failed (non-fatal): {e}");
                            warnings.push(format!("anchor update failed: {e}"));
                        }
                    }
                }

                // Create semantic anchors (best-effort, non-fatal)
                if !private {
                    let path_files: HashSet<String> = {
                        if let Ok(root) = ctx.agent.require_project_root().await {
                            let memories_dir =
                                resolve_memory_dir(&input, ctx).await.unwrap_or_else(|_| {
                                    root.join(".codescout").join("memories")
                                });
                            let sidecar_path =
                                memories_dir.join(format!("{}.anchors.toml", topic));
                            crate::memory::anchors::read_anchor_file(&sidecar_path)
                                .map(|af| af.anchors.into_iter().map(|a| a.path).collect())
                                .unwrap_or_default()
                        } else {
                            HashSet::new()
                        }
                    };
                    if let Err(e) =
                        create_semantic_anchors(ctx, topic, content, &path_files).await
                    {
                        tracing::warn!("semantic anchor creation failed (non-fatal): {e}");
                        warnings.push(format!("semantic anchor creation failed: {e}"));
                    }
                }

                if warnings.is_empty() {
                    Ok(json!("ok"))
                } else {
                    // Legitimate exception to the `json!("ok")` rule for writes:
                    // the caller cannot otherwise know that a best-effort side
                    // effect (semantic indexing, anchor update) silently failed.
                    Ok(json!({
                        "status": "ok",
                        "warnings": warnings,
                    }))
                }
            }
            "read" => {
                let topic = super::require_str_param(&input, "topic")?;
                let private = parse_bool_param(&input["private"]);
                let sections: Vec<String> = super::optional_array_param(&input, "sections")
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if private {
                    let buf = std::sync::Arc::clone(&ctx.output_buffer);
                    ctx.agent
                        .with_project(|p| {
                            match p.private_memory.read(topic)? {
                                Some(content) => {
                                    apply_sections_filter(content, topic, &sections, &buf)
                                }
                                None => Err(RecoverableError::with_hint(
                                    format!("topic '{}' not found", topic),
                                    "Use memory(action='list') to see available topics",
                                )
                                .into()),
                            }
                        })
                        .await
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    let store = crate::memory::MemoryStore::from_dir(memories_dir)?;
                    match store.read(topic)? {
                        Some(content) => {
                            apply_sections_filter(content, topic, &sections, &ctx.output_buffer)
                        }
                        None => Err(RecoverableError::with_hint(
                            format!("topic '{}' not found", topic),
                            "Use memory(action='list') to see available topics",
                        )
                        .into()),
                    }
                }
            }
            "list" => {
                let include_private = parse_bool_param(&input["include_private"]);
                if include_private {
                    // include_private needs the private store from ActiveProject — use with_project.
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    let shared_store = crate::memory::MemoryStore::from_dir(memories_dir)?;
                    let shared = shared_store.list()?;
                    let private = ctx.agent.with_project(|p| p.private_memory.list()).await?;
                    Ok(json!({ "shared": shared, "private": private }))
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    let topics = crate::memory::MemoryStore::from_dir(memories_dir)?.list()?;
                    Ok(json!({ "topics": topics }))
                }
            }
            "delete" => {
                let topic = super::require_str_param(&input, "topic")?;
                let private = parse_bool_param(&input["private"]);

                // Delete markdown file — route to per-project dir when `project` param given.
                if private {
                    ctx.agent
                        .with_project(|p| {
                            p.private_memory.delete(topic)?;
                            Ok(())
                        })
                        .await?;
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    crate::memory::MemoryStore::from_dir(memories_dir)?.delete(topic)?;
                }

                // Remove cross-embedded entry (best-effort, non-fatal)
                if !private {
                    let root = {
                        let inner = ctx.agent.inner.read().await;
                        inner.active_project().map(|p| p.root.clone())
                    };
                    if let Some(root) = root {
                        let topic_owned = topic.to_string();
                        let _ = tokio::task::spawn_blocking(move || {
                            use rusqlite::OptionalExtension;
                            let conn = crate::embed::index::open_db(&root)?;
                            let id: Option<i64> = conn
                                .query_row(
                                    "SELECT id FROM memories WHERE title = ?1 AND bucket = 'structured'",
                                    rusqlite::params![topic_owned],
                                    |r| r.get(0),
                                )
                                .optional()?;
                            if let Some(id) = id {
                                crate::embed::index::delete_memory(&conn, id)?;
                            }
                            anyhow::Ok(())
                        })
                        .await;
                    }
                }

                Ok(json!("ok"))
            }
            "remember" => {
                let content = super::require_str_param(&input, "content")?;
                let title = input["title"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| extract_title(content));
                let bucket = input["bucket"]
                    .as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unstructured".to_string());

                let (root, model) = {
                    let inner = ctx.agent.inner.read().await;
                    let p = inner.active_project().ok_or_else(|| {
                        super::RecoverableError::with_hint(
                            "No active project.",
                            "Call workspace(action='activate') first.",
                        )
                    })?;
                    (p.root.clone(), p.config.embeddings.model.clone())
                };

                let embedder = ctx.agent.get_or_create_embedder(&model).await?;
                let embedding = codescout_embed::embed_one(embedder.as_ref(), content).await?;

                let bucket2 = bucket.clone();
                let title2 = title.clone();
                let content2 = content.to_string();
                tokio::task::spawn_blocking(move || {
                    let conn = crate::embed::index::open_db(&root)?;
                    crate::embed::index::ensure_vec_memories(&conn)?;
                    crate::embed::index::insert_memory(
                        &conn, &bucket2, &title2, &content2, &embedding,
                    )?;
                    anyhow::Ok(())
                })
                .await??;

                Ok(json!("ok"))
            }
            "recall" => {
                let query = super::require_str_param(&input, "query")?;
                let limit = super::optional_u64_param(&input, "limit").unwrap_or(5) as usize;
                let bucket_filter = input["bucket"].as_str();

                let (root, model) = {
                    let inner = ctx.agent.inner.read().await;
                    let p = inner.active_project().ok_or_else(|| {
                        super::RecoverableError::with_hint(
                            "No active project.",
                            "Call workspace(action='activate') first.",
                        )
                    })?;
                    (p.root.clone(), p.config.embeddings.model.clone())
                };

                let embedder = ctx.agent.get_or_create_embedder(&model).await?;
                let query_embedding =
                    codescout_embed::embed_one(embedder.as_ref(), query).await?;

                let bucket = bucket_filter.map(|s| s.to_string());
                let results = tokio::task::spawn_blocking(move || {
                    let conn = crate::embed::index::open_db(&root)?;
                    crate::embed::index::ensure_vec_memories(&conn)?;
                    crate::embed::index::search_memories(
                        &conn,
                        &query_embedding,
                        bucket.as_deref(),
                        limit,
                    )
                })
                .await??;

                let guard = super::output::OutputGuard::from_input(&input);
                let items: Vec<serde_json::Value> = results
                    .iter()
                    .map(|r| {
                        let content = if guard.should_include_body() {
                            r.content.clone()
                        } else {
                            // Exploring mode: first line only, max 50 chars
                            let first_line = r.content.lines().next().unwrap_or("").trim();
                            if first_line.len() > 50 {
                                format!("{}...", &first_line[..47])
                            } else {
                                first_line.to_string()
                            }
                        };
                        json!({
                            "id": r.id,
                            "bucket": r.bucket,
                            "title": r.title,
                            "content": content,
                            "similarity": format!("{:.2}", r.similarity),
                            "created_at": r.created_at,
                        })
                    })
                    .collect();

                Ok(json!({ "results": items }))
            }
            "forget" => {
                let id = super::optional_i64_param(&input, "id").ok_or_else(|| {
                    super::RecoverableError::with_hint(
                        "Missing required parameter 'id'",
                        "Pass the numeric id from a recall result",
                    )
                })?;

                let root = {
                    let inner = ctx.agent.inner.read().await;
                    let p = inner.active_project().ok_or_else(|| {
                        super::RecoverableError::with_hint(
                            "No active project.",
                            "Call workspace(action='activate') first.",
                        )
                    })?;
                    p.root.clone()
                };

                tokio::task::spawn_blocking(move || {
                    let conn = crate::embed::index::open_db(&root)?;
                    crate::embed::index::delete_memory(&conn, id)?;
                    anyhow::Ok(())
                })
                .await??;

                Ok(json!("ok"))
            }
            "refresh_anchors" => {
                let topic = super::require_str_param(&input, "topic")?;
                let root = ctx.agent.require_project_root().await?;
                let memories_dir = resolve_memory_dir(&input, ctx).await.unwrap_or_else(|_| {
                    root.join(".codescout").join("memories")
                });

                // Check that the memory topic exists
                let topic_path = memories_dir.join(format!("{}.md", topic));
                if !topic_path.exists() {
                    return Err(RecoverableError::with_hint(
                        format!("topic '{}' not found", topic),
                        "Use memory(action='list') to see available topics",
                    )
                    .into());
                }

                crate::memory::anchors::refresh_hashes(&root, &memories_dir, topic)?;
                Ok(json!("ok"))
            }
            _ => Err(RecoverableError::with_hint(
                format!(
                    "unknown action '{}'. Must be one of: read, write, list, delete, remember, recall, forget, refresh_anchors",
                    action
                ),
                "Pass action: 'read', 'write', 'list', 'delete', 'remember', 'recall', 'forget', or 'refresh_anchors'",
            )
            .into()),
        }
    }

    fn format_compact(&self, result: &Value) -> Option<String> {
        if result["topics"].is_array() || result["shared"].is_array() {
            Some(format_list_memories(result))
        } else if result["content"].is_string() {
            Some(format_read_memory(result))
        } else {
            None
        }
    }

    fn json_path_hint(&self, val: &Value) -> String {
        if val["content"].is_string() {
            "$.content".to_string()
        } else {
            "$.field".to_string()
        }
    }
}

#[cfg(test)]
mod tests;
