//! Memory tools: persistent per-project knowledge store.

use std::collections::{HashMap, HashSet};

use super::{parse_bool_param, OutputForm, RecoverableError, Tool, ToolContext};
use serde_json::{json, Value};

/// Render a `memory(action="read")` result.
///
/// Normally the memory text verbatim. When the topic resolved from the project's
/// OTHER memory store rather than the write target, the note goes FIRST: this
/// function's output is the memory body, so a trailing note is what any
/// downstream truncation cuts, and a caller pasting the result would carry it
/// into the memory itself.
///
/// These fields exist only in the split-store case, so this branch is the only
/// thing that renders them — a field the formatter drops reaches nobody, which is
/// how `docs/issues/archive/2026-08-26-read-file-truncation-flag-never-rendered.md`
/// shipped inert.
fn format_read_memory(result: &Value) -> String {
    let content = result["content"].as_str().unwrap_or("");
    match (
        result["resolved_from"].as_str(),
        result["write_target"].as_str(),
    ) {
        (Some(from), Some(target)) => format!(
            "⚠ read from {from} — this project's other memory store. \
             memory(action='write') on this topic targets {target} instead, which \
             would leave the text below untouched and shadowed by a second copy.\n\n\
             {content}"
        ),
        _ => content.to_string(),
    }
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
/// Epoch-seconds as a zero-padded 10-digit string. Lexicographic compare ==
/// numeric compare until year 2286. Used as `created_at` / `updated_at` for
/// new semantic memories so `MemoryOrder::UpdatedAtDesc` sorts correctly.
fn now_epoch_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs:010}")
}

/// Resolve the topic parameter, accepting `topic` (canonical) or the
/// `name` / `key` aliases. LLMs frequently call `memory(action="read",
/// name="...")` — the strict missing-`topic` error makes the tool feel
/// brittle for what is effectively a synonym. Error message still
/// references the canonical `topic` so the schema stays the source of
/// truth.
fn require_topic_param(input: &Value) -> anyhow::Result<&str> {
    // Delegates rather than carrying its own message: this used to hold a private
    // copy of the generic "Add the required 'topic' parameter" text, which meant the
    // shared hint table was silently bypassed on the tool's most-used path. Live
    // verification caught it after the table shipped — `query` taught the call and
    // `topic`, on the same tool, did not. BL-3 Class B.
    crate::tools::require_str_param_or(input, "topic", &["name", "key"])
}

/// Rank `available` topics by shared kebab/slash/underscore token overlap with
/// `query`, returning up to 3 (best first, alpha-tie-broken). Topic names are
/// structured (`iel-solver-debug`, `research/agent-memory`), so token overlap
/// surfaces siblings of a misremembered name far better than raw edit distance.
fn closest_topics(query: &str, available: &[String]) -> Vec<String> {
    fn tokens(s: &str) -> Vec<&str> {
        s.split(['-', '/', '_']).filter(|t| !t.is_empty()).collect()
    }
    let q = tokens(query);
    let mut scored: Vec<(usize, &String)> = available
        .iter()
        .map(|t| {
            (
                tokens(t).into_iter().filter(|tok| q.contains(tok)).count(),
                t,
            )
        })
        .filter(|(shared, _)| *shared > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored.into_iter().take(3).map(|(_, t)| t.clone()).collect()
}

/// The refusal a would-be destructive `memory(write)` gets.
///
/// Mirrors the artifact body-shrink guard in `librarian/tools/update.rs` — and
/// since 2026-08-29 shares its predicate outright via
/// `crate::util::shrink_guard`. Name the guard, show both dimensions, and name
/// BOTH ways forward. The hint leads with *why* rather than *what*, because the
/// failure mode is a wrong mental model — callers reach for `write` believing it
/// appends. It replaces.
///
/// Motivating incident: two new sections written to a 17-section memory
/// deleted the other fifteen and returned `{"status":"ok"}`.
/// See `docs/issues/archive/2026-08-28-memory-write-has-no-shrink-guard.md`.
fn shrink_guard_error(topic: &str, r: &crate::memory::ShrinkReport) -> RecoverableError {
    RecoverableError::with_hint(
        format!("memory-shrink guard: write to '{topic}' {}", r.describe()),
        "memory(action=\"write\") REPLACES the topic wholesale — it does not append. \
         To add or change one section, read the topic first and write the whole document \
         back with your edit folded in, or edit the file under .codescout/memories/ with \
         edit_markdown. If the shrinkage is intentional (a deliberate rewrite or prune), \
         re-call with force=true.",
    )
    .with_extra(
        "shrink",
        json!({
            "old_bytes": r.old_bytes,
            "new_bytes": r.new_bytes,
            "byte_pct": r.byte_pct,
            "old_lines": r.old_lines,
            "new_lines": r.new_lines,
            "line_pct": r.line_pct,
            "tripped_on": r.dimension.as_str(),
        }),
    )
}

/// Build the "topic not found" error for memory reads. Rather than a bare
/// warning telling the caller to go run `list`, the response embeds a *preview*
/// of the store — the full `available_topics` list plus best-effort
/// `did_you_mean` suggestions — so the caller self-corrects in one round-trip.
/// Stays a `RecoverableError` (`ok:false`) so genuine misconfiguration (wrong
/// `project_id`, empty store) still surfaces rather than reading as success.
fn topic_not_found_error(topic: &str, available: Vec<String>) -> RecoverableError {
    let suggestions = closest_topics(topic, &available);
    let hint = if available.is_empty() {
        "no memory topics exist yet — create one with \
         memory(action='write', topic=…, content=…)"
            .to_string()
    } else if suggestions.is_empty() {
        format!(
            "{} topic(s) available — see `available_topics` in this response",
            available.len()
        )
    } else {
        format!(
            "did you mean: {}? Full list in `available_topics`",
            suggestions.join(", ")
        )
    };
    let mut err = RecoverableError::with_hint(format!("topic '{topic}' not found"), hint)
        .with_extra("available_topics", json!(available));
    if !suggestions.is_empty() {
        err = err.with_extra("did_you_mean", json!(suggestions));
    }
    err
}

/// Best-effort cross-embed a markdown memory into the semantic store.
/// Called on `write` so that structured memories are also discoverable via `recall`.
async fn cross_embed_memory(ctx: &ToolContext, topic: &str, content: &str) -> anyhow::Result<()> {
    let (project_id, model_spec) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok((
                p.config.project.name.clone(),
                p.config.embeddings.model.clone(),
            ))
        })
        .await?;

    // `embed_document`, not `embed`: this is a document being stored. `embed` is
    // the query side and carries an asymmetric model's query prefix.
    // docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md
    //
    // Budget from the CONFIGURED MODEL, not a constant. `chunk_size_for_model` was
    // kept alive precisely for consumers like this one — the decision in
    // docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md
    // rejected it for sizing CODE CHUNKS (where 1200 chars is benchmark-backed and
    // model-independent), while keeping the function because it is correct for the
    // per-model arms. Memory content has no such benchmarked size; its only real
    // constraint is the ceiling, which is what this returns.
    let budget_chars = crate::embed::chunk_size_for_model(&model_spec);
    let embedder = ctx.agent.memory_embedder().await?;
    let dense =
        crate::embed::document::embed_document_pooled(embedder.as_ref(), content, budget_chars)
            .await?;

    let now = now_epoch_string();
    let memory = crate::retrieval::memory_payload::SemanticMemory {
        project_id,
        bucket: "structured".into(),
        title: topic.to_string(),
        content: content.to_string(),
        anchors: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    let store = ctx.agent.semantic_memory_store().await?;
    store.upsert(&memory, &dense).await?;
    Ok(())
}

/// Create semantic anchors for a markdown memory by embedding it, asking the
/// retrieval stack for similar code chunks, and re-upserting the memory with
/// `anchors` populated. Excludes files already covered by path anchors.
///
/// The re-upsert overwrites `cross_embed_memory`'s prior point (same
/// deterministic id) but preserves the content the caller passed in here, so
/// callers don't need to coordinate the two writes.
///
/// Best-effort: failures are returned to the caller (which logs and ignores).
async fn create_semantic_anchors(
    ctx: &ToolContext,
    topic: &str,
    content: &str,
    path_anchor_files: &HashSet<String>,
) -> anyhow::Result<()> {
    let (project_id, min_sim, top_n, model_spec) = ctx
        .agent
        .with_project_at(ctx.workspace_override.as_deref(), |p| {
            Ok((
                p.config.project.name.clone(),
                p.config.memory.semantic_anchor_min_similarity,
                p.config.memory.semantic_anchor_top_n,
                p.config.embeddings.model.clone(),
            ))
        })
        .await?;

    // `embed_document` for the stored vector — see `cross_embed_memory`. The
    // anchor *search* below is a query and stays on the query side, which is the
    // asymmetry this seam exists to express.
    //
    // Segmented on the same budget as `cross_embed_memory`, and for the same reason:
    // this call re-upserts the SAME point id, so leaving it unsegmented would let the
    // anchor pass overwrite a correctly-pooled vector with a truncated or missing one
    // — the second write silently undoing the first.
    let budget_chars = crate::embed::chunk_size_for_model(&model_spec);
    let embedder = ctx.agent.memory_embedder().await?;
    let dense =
        crate::embed::document::embed_document_pooled(embedder.as_ref(), content, budget_chars)
            .await?;

    // Code chunk search goes through its own seam. The embedder seam above only
    // covers the dense vector path, so until 2026-08-30 this line built a real
    // client from ambient config even in tests that had stubbed the embedder —
    // which is why the memory suite's runtime tracked the developer's local stack
    // (1.16s up, 20.65s wedged, 76 passing either way). When the retrieval stack
    // is offline, search_code errors and the anchor-creation step is skipped by
    // the caller (logged at warn); that is unchanged.
    let root = ctx
        .agent
        .require_project_root_for(ctx.workspace_override.as_deref())
        .await?;
    let client = ctx.agent.code_search(Some(&root)).await?;

    // Code chunk search via the retrieval stack. Overfetch so dedupe-by-file
    // has room to pick the best chunk per file.
    let opts = crate::retrieval::search::SearchOpts {
        limit: top_n,
        overfetch: top_n * 4,
        // Cross-encoder reranking gives higher-quality anchor selection than
        // raw RRF. Falls back to RRF score automatically if the reranker is
        // unavailable.
        rerank: true,
        exclude_languages: vec!["markdown".to_string()],
        exclude_paths: Vec::new(),
    };
    let hits = client.search_code(&project_id, content, opts).await?;

    // Dedupe by file path, keep highest score, apply min_sim + path-anchor exclusions.
    // Prefer rerank_score when the reranker ran; fall back to RRF score otherwise.
    let mut best_per_file: HashMap<String, f32> = HashMap::new();
    for h in &hits {
        let score = h.rerank_score.unwrap_or(h.score);
        if score < min_sim {
            continue;
        }
        if path_anchor_files.contains(&h.file_path) {
            continue;
        }
        best_per_file
            .entry(h.file_path.clone())
            .and_modify(|s| {
                if score > *s {
                    *s = score;
                }
            })
            .or_insert(score);
    }

    let mut anchors: Vec<crate::retrieval::memory_payload::MemoryAnchor> = best_per_file
        .into_keys()
        .map(|path| crate::retrieval::memory_payload::MemoryAnchor { path })
        .collect();
    anchors.sort_by(|a, b| a.path.cmp(&b.path)); // deterministic ordering

    let now = now_epoch_string();
    let memory = crate::retrieval::memory_payload::SemanticMemory {
        project_id,
        bucket: "structured".into(),
        title: topic.to_string(),
        content: content.to_string(),
        anchors,
        created_at: now.clone(),
        updated_at: now,
    };

    let store = ctx.agent.semantic_memory_store().await?;
    store.upsert(&memory, &dense).await?;
    Ok(())
}

/// The directories a `memory` READ should consult, in precedence order.
///
/// A sub-project has two memory stores and always has had, because the two
/// layouts are keyed on different things:
///
/// | resolved by | rooted at | reached when |
/// |---|---|---|
/// | `Workspace::memory_dir_for_project` | `<ws>/.codescout/projects/<id>/memories` | the project is addressed as a member of a workspace |
/// | `MemoryStore::open(project.root)` | `<project>/.codescout/memories` | the project is addressed by absolute path, so it IS the root project |
///
/// The first is a function of the WORKSPACE, the second of the PROJECT — so the
/// same project reached two ways gets two different stores, and each surface saw
/// only one of them. Measured on this machine 2026-08-27: one workspace held 53
/// memories in the project-local layout and none in the workspace layout, another
/// held 42 in the workspace layout and none project-local. Neither is debris, so
/// neither can simply be declared the loser.
///
/// Reads therefore union both; **writes are unchanged** and still target
/// [`Self::primary`] alone. That keeps this a pure read-visibility fix with no
/// migration: no file moves, none is untracked, and no write lands anywhere it
/// would not have landed before.
///
/// For the ROOT project the two paths coincide, `secondary` is `None`, and every
/// method here reduces to the single-directory behaviour byte for byte. That is
/// why a single-project repo never saw this bug and never changes behaviour now.
///
/// See `docs/issues/archive/2026-07-07-memory-tool-hides-project-memories-after-workspace-activate.md`
/// for the id-route half, and
/// `docs/issues/archive/2026-08-27-activate-by-path-bypasses-workspace-memory-resolution.md`
/// for the by-path half this union deliberately does NOT close: a by-path
/// activation builds a STANDALONE workspace rooted at the target, so the parent's
/// per-project tree is not merely unread, it is not loaded at all — there is no
/// second layout there to union.
pub(crate) struct MemoryReadDirs {
    /// The write target — exactly what `resolve_memory_dir` returns.
    pub(crate) primary: std::path::PathBuf,
    /// The other layout's directory for the same project, when it differs.
    pub(crate) secondary: Option<std::path::PathBuf>,
}

impl MemoryReadDirs {
    /// Every directory a read consults, write target first.
    pub(crate) fn all(&self) -> impl Iterator<Item = &std::path::PathBuf> {
        std::iter::once(&self.primary).chain(self.secondary.iter())
    }

    /// Union of topics across both layouts, deduped and sorted.
    ///
    /// Uses `from_dir_readonly` deliberately: `from_dir` would create the
    /// secondary directory on every list, which is the litter this fix must not
    /// introduce while removing an invisibility.
    pub(crate) fn list_union(&self) -> anyhow::Result<Vec<String>> {
        let mut topics = Vec::new();
        for dir in self.all() {
            topics.extend(crate::memory::MemoryStore::from_dir_readonly(dir.clone()).list()?);
        }
        topics.sort();
        topics.dedup();
        Ok(topics)
    }

    /// Read `topic`, write target first. Returns the content and the directory it
    /// came from, so the caller can say so when it was not the write target.
    pub(crate) fn read_first(
        &self,
        topic: &str,
    ) -> anyhow::Result<Option<(String, std::path::PathBuf)>> {
        for dir in self.all() {
            if let Some(content) =
                crate::memory::MemoryStore::from_dir_readonly(dir.clone()).read(topic)?
            {
                return Ok(Some((content, dir.clone())));
            }
        }
        Ok(None)
    }
}

/// Resolve the directories a `memory` tool call reads from.
///
/// If `project_id` is provided, route to the per-project directory via
/// `Workspace::memory_dir_for_project`. Otherwise use the focused project's memory
/// dir. Falls back gracefully when no workspace is loaded.
///
/// **`project` was an undocumented alias and is now a refusal.** It was accepted
/// from 2026-06-09 to 2026-09-02 because the onboarding prompt taught that
/// spelling; the prompt surfaces now say `project_id` everywhere, so the alias had
/// no caller left. Deleting it outright would have re-created the original defect
/// in its harder direction — a scoping param silently dropped, yielding an
/// unscoped result that reads as scoped — so the key is rejected with the correct
/// spelling instead of ignored. The schema advertises one name and the runtime
/// honours exactly that one.
///
/// docs/issues/archive/2026-09-02-activation-banner-names-a-project-param-symbols-does-not-have.md
///
/// `.primary` is the write target; `.secondary` is the other layout's directory
/// for the same project when the two differ. See [`MemoryReadDirs`].
async fn resolve_memory_dirs(input: &Value, ctx: &ToolContext) -> anyhow::Result<MemoryReadDirs> {
    if input.get("project_id").is_none() {
        if let Some(v) = input.get("project").and_then(Value::as_str) {
            return Err(super::RecoverableError::with_hint(
                "`project` is not a parameter of `memory` — the key is `project_id`",
                format!("Re-send with project_id: {v:?}"),
            )
            .into());
        }
    }
    let project_param = input
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    // Pin the memory dir to the workspace named by ctx.workspace_override
    // (resident-on-demand), else the session default (regime-3).
    if let Some(root) = ctx.workspace_override.as_deref() {
        let _ = ctx.agent.ensure_resident(root.to_path_buf(), None).await;
    }
    let inner = ctx.agent.inner.read().await;
    let ws = match ctx.workspace_override.as_deref() {
        Some(root) => {
            let key = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
            inner.workspaces.get(&key)
        }
        None => inner.default_workspace(),
    };
    if let Some(ws) = ws {
        // Validate the CALLER-SUPPLIED id, and only that one. `ws.focused` is
        // seeded from `ws.projects` by `Workspace::new` and re-checked by
        // `set_focused`, so it is always a real id; the `ROOT_PROJECT_ID` last
        // resort is deliberately left untouched so the no-argument path stays
        // byte-identical.
        //
        // Without this check an unknown id reached `memory_dir_for_project`,
        // whose lookup miss is indistinguishable from a known non-root project.
        // A typo therefore got its own `projects/<id>/memories` tree, and `read`
        // answered "no memory topics exist yet" with `available_topics: []` for a
        // project that never existed — an empty answer a caller acts on.
        if let Some(id) = project_param.as_deref() {
            if !ws.has_project(id) {
                let mut ids = ws.project_ids();
                ids.sort();
                let hint = if ids.is_empty() {
                    "This workspace has no projects — omit project_id.".to_string()
                } else {
                    format!(
                        "Valid project ids: {}. Omit project_id to use the focused project.",
                        ids.join(", ")
                    )
                };
                return Err(super::RecoverableError::with_hint(
                    format!("No project '{id}'."),
                    hint,
                )
                .into());
            }
        }
        let project_id = project_param
            .or_else(|| ws.focused.clone())
            .unwrap_or_else(|| crate::workspace::ROOT_PROJECT_ID.to_string());
        let primary = ws.memory_dir_for_project(&project_id);
        // The SAME project's other store. `project_root_by_id` is the existing
        // by-id resolver used by `resolve_root`; joining `.codescout/memories`
        // onto it reproduces `MemoryStore::open`'s layout without opening (and
        // therefore without creating) anything.
        //
        // The `filter` is what makes the root project a no-op: its
        // `project_root_by_id` IS `ws.root`, so the computed path equals
        // `primary` and drops out.
        let secondary = ws
            .project_root_by_id(&project_id)
            .ok()
            .map(|root| root.join(".codescout").join("memories"))
            .filter(|dir| dir != &primary);
        Ok(MemoryReadDirs { primary, secondary })
    } else {
        // No workspace — fall back to the active project's memory dir. That store
        // is already `MemoryStore::open(p.root)`, i.e. the project-local layout,
        // so there is no second directory to union in.
        let p = inner.active_project().ok_or_else(|| {
            super::RecoverableError::with_hint(
                "No active project.",
                "Call workspace(action='activate') first.",
            )
        })?;
        Ok(MemoryReadDirs {
            primary: p.memory.dir().to_path_buf(),
            secondary: None,
        })
    }
}

/// The single directory a `memory` WRITE targets.
///
/// Thin wrapper over [`resolve_memory_dirs`] — `.primary` is byte-identical to
/// what this function returned before the read-union fix, so every write, delete
/// and anchor call site keeps its exact previous behaviour.
async fn resolve_memory_dir(
    input: &Value,
    ctx: &ToolContext,
) -> anyhow::Result<std::path::PathBuf> {
    Ok(resolve_memory_dirs(input, ctx).await?.primary)
}

/// Union `project_local` with the workspace-layout memory topics for the focused
/// project, for `workspace(action="activate")`'s `memories` array.
///
/// `build_activation_response` reports `p.memory.list()` — the PROJECT-local
/// layout — while `memory(action="list")` resolves through [`MemoryReadDirs`].
/// For a sub-project those are different directories, so the activation response
/// advertised memories that `memory(action="read")` then answered "not found"
/// for, and the caller's next move was to write a second copy. Reporting the
/// union makes the two surfaces agree by construction — it is the same set
/// [`MemoryReadDirs::list_union`] returns for the same project.
///
/// Falls back to `project_local` unchanged when no workspace resolves, so a
/// single-project activation is untouched.
pub(crate) async fn union_with_workspace_memories(
    ctx: &ToolContext,
    project_local: Vec<String>,
) -> Vec<String> {
    let mut topics = project_local;
    // No `project_id`: resolves through `ws.focused`, which activation has
    // already pointed at the project being activated.
    if let Ok(dirs) = resolve_memory_dirs(&json!({}), ctx).await {
        topics.extend(dirs.list_union().unwrap_or_default());
    }
    topics.sort();
    topics.dedup();
    topics
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
                "this memory has no headings below the title to filter on \
                 (searched levels ##..######) — read it without `sections`"
                    .to_string()
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
        "Persistent project memory. Topic-based: read/write/list/delete/refresh_anchors with \
         path-like keys. \
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
             - `action=\"refresh_anchors\"`: re-hash a topic's code anchors, clearing staleness.\n\
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
                    "description": "For read. Return only the listed sections (case-insensitive). Sections are the memory's shallowest heading level below the title — `##` in most memories, `###` in some; deeper headings come along as body. E.g. [\"Rust\", \"MCP Binary Symlink\"]. Omit to return full content."
                },
                "content": { "type": "string", "description": "For write or remember." },
                "private": { "type": "boolean", "default": false, "description": "Use gitignored private store." },
                "force": { "type": "boolean", "default": false, "description": "For write: bypass the shrink guard. Required when the write would replace an existing topic with less than half its bytes. `write` REPLACES wholesale — it never appends — so a partial document is data loss, not an update." },
                "include_private": { "type": "boolean", "default": false, "description": "For list: include private topics." },
                "title": { "type": "string", "description": "For remember. Short label (auto-extracted if omitted)." },
                "bucket": {
                    "type": "string",
                    "enum": ["code", "system", "preferences", "unstructured"],
                    "description": "For remember (always specify) or recall (optional filter)."
                },
                "query": { "type": "string", "description": "For recall. Search query." },
                "limit": { "type": "integer", "description": "For recall. Max results (default 5)." },
                "id": { "type": "string", "description": "For forget. UUID string from a recall result." },
                "project_id": { "type": "string", "description": "Scope to a workspace project ID. Default: focused project." }
            }
        })
    }

    async fn call(&self, input: Value, ctx: &ToolContext) -> anyhow::Result<Value> {
        // `action` indexes a different enum in every tool that has one, so the shared
        // table has no entry — name this tool's own set. BL-3 Class B.
        let action = super::require_str_param_or_hint(
            &input,
            "action",
            &[],
            "Pass the operation, e.g. action=\"read\". One of: read, write, list, delete, \
             remember, recall, forget, refresh_anchors. Topic-based actions take `topic`; \
             semantic ones take `query` or `content`.",
        )?;
        match action {
            "write" => {
                let topic = require_topic_param(&input)?;
                let content = super::require_str_param(&input, "content")?;
                let private = parse_bool_param(&input["private"]);
                let force = parse_bool_param(&input["force"]);

                // Write markdown file — route to per-project dir when `project` param given.
                //
                // The shrink check runs against the SAME store the write lands
                // in, inside each branch rather than hoisted above them. The
                // private and project stores are different directories, so one
                // check up top would read the wrong file and could clear a
                // destructive write or block a harmless one.
                if private {
                    ctx.agent
                        .with_project_at(ctx.workspace_override.as_deref(), |p| {
                            if !force {
                                if let Some(r) = p.private_memory.shrink_check(topic, content) {
                                    return Err(shrink_guard_error(topic, &r).into());
                                }
                            }
                            p.private_memory.write(topic, content)?;
                            Ok(())
                        })
                        .await?;
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    let store = crate::memory::MemoryStore::from_dir(memories_dir)?;
                    if !force {
                        if let Some(r) = store.shrink_check(topic, content) {
                            return Err(shrink_guard_error(topic, &r).into());
                        }
                    }
                    store.write(topic, content)?;
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
                    if let Ok(root) = ctx.agent.require_project_root_for(ctx.workspace_override.as_deref()).await {
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
                        if let Ok(root) = ctx.agent.require_project_root_for(ctx.workspace_override.as_deref()).await {
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
                let topic = require_topic_param(&input)?;
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
                        .with_project_at(ctx.workspace_override.as_deref(), |p| {
                            match p.private_memory.read(topic)? {
                                Some(content) => {
                                    apply_sections_filter(content, topic, &sections, &buf)
                                }
                                None => Err(topic_not_found_error(
                                    topic,
                                    p.private_memory.list().unwrap_or_default(),
                                )
                                .into()),
                            }
                        })
                        .await
                } else {
                    let dirs = resolve_memory_dirs(&input, ctx).await?;
                    match dirs.read_first(topic)? {
                        Some((content, from)) => {
                            let mut value =
                                apply_sections_filter(content, topic, &sections, &ctx.output_buffer)?;
                            // Resolved from the OTHER layout, not the write target.
                            // Say so: a later `memory(write)` on this topic goes to
                            // `primary`, leaving the file just read untouched and
                            // shadowed by a second copy. That is the one foot-gun
                            // the read-union introduces, so it is reported every
                            // time it is live — and rendered by `format_read_memory`,
                            // which returns `$.content` alone and would otherwise
                            // drop these fields silently.
                            if from != dirs.primary {
                                if let Some(obj) = value.as_object_mut() {
                                    obj.insert(
                                        "resolved_from".to_string(),
                                        json!(crate::util::fs::to_forward_slash(&from)),
                                    );
                                    obj.insert(
                                        "write_target".to_string(),
                                        json!(crate::util::fs::to_forward_slash(&dirs.primary)),
                                    );
                                }
                            }
                            Ok(value)
                        }
                        None => Err(topic_not_found_error(
                            topic,
                            dirs.list_union().unwrap_or_default(),
                        )
                        .into()),
                    }
                }
            }
            "list" => {
                let include_private = parse_bool_param(&input["include_private"]);
                let dirs = resolve_memory_dirs(&input, ctx).await?;
                let shared = dirs.list_union()?;
                if include_private {
                    // include_private needs the private store from ActiveProject — use with_project.
                    let private = ctx.agent.with_project_at(ctx.workspace_override.as_deref(), |p| p.private_memory.list()).await?;
                    Ok(json!({ "shared": shared, "private": private }))
                } else {
                    Ok(json!({ "topics": shared }))
                }
            }
            "delete" => {
                let topic = require_topic_param(&input)?;
                let private = parse_bool_param(&input["private"]);

                // Delete markdown file — route to per-project dir when `project` param given.
                if private {
                    ctx.agent
                        .with_project_at(ctx.workspace_override.as_deref(), |p| {
                            p.private_memory.delete(topic)?;
                            Ok(())
                        })
                        .await?;
                } else {
                    let memories_dir = resolve_memory_dir(&input, ctx).await?;
                    crate::memory::MemoryStore::from_dir(memories_dir.clone())?.delete(topic)?;

                    // Remove the path-anchor sidecar so it does not orphan and
                    // continue surfacing in staleness scans (review I4,
                    // docs/reviews/2026-04-24/phase-5-embed-memory-library.md).
                    let sidecar =
                        crate::memory::anchors::anchor_path_for_topic(&memories_dir, topic);
                    match std::fs::remove_file(&sidecar) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            tracing::warn!(
                                "failed to remove anchor sidecar {}: {e}",
                                sidecar.display()
                            );
                        }
                    }
                }

                // Remove cross-embedded entry (best-effort, non-fatal).
                // The point id is derived from (project_id, "structured", topic),
                // so we can delete without looking it up first.
                if !private {
                    let project_id = ctx
                        .agent
                        .with_project_at(ctx.workspace_override.as_deref(), |p| {
                            Ok(p.config.project.name.clone())
                        })
                        .await
                        .ok();
                    if let Some(project_id) = project_id {
                        let id = crate::retrieval::memory_payload::point_id_for(
                            &project_id,
                            "structured",
                            topic,
                        );
                        if let Ok(store) = ctx.agent.semantic_memory_store().await {
                            let _ = store.delete(&project_id, id).await;
                        }
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

                let project_id = ctx
                    .agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.name.clone())
                    })
                    .await?;

                let dense = ctx.agent.memory_embedder().await.map_err(|e| {
                    super::RecoverableError::with_hint(
                        format!("embedder unavailable: {e}"),
                        "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack.",
                    )
                })?.embed(content).await?;

                let now = now_epoch_string();
                let memory = crate::retrieval::memory_payload::SemanticMemory {
                    project_id,
                    bucket: bucket.clone(),
                    title: title.clone(),
                    content: content.to_string(),
                    anchors: Vec::new(),
                    created_at: now.clone(),
                    updated_at: now,
                };
                let store = ctx.agent.semantic_memory_store().await?;
                store.upsert(&memory, &dense).await?;

                Ok(json!("ok"))
            }
            "recall" => {
                let query = super::require_str_param(&input, "query")?;
                let limit = super::optional_u64_param(&input, "limit").unwrap_or(5) as usize;
                let bucket_filter = input["bucket"].as_str();

                let project_id = ctx
                    .agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.name.clone())
                    })
                    .await?;

                // Embed via the shared dense-embedder seam so the query vector
                // lives in the same space as the memories collection's stored
                // vectors. Tests can swap the embedder via
                // `Agent::set_memory_embedder_for_test`.
                let query_vec = ctx.agent.memory_embedder().await.map_err(|e| {
                    super::RecoverableError::with_hint(
                        format!("embedder unavailable: {e}"),
                        "Run `./scripts/retrieval-stack.sh up` to start the retrieval stack.",
                    )
                })?.embed(query).await?;

                let store = ctx.agent.semantic_memory_store().await?;
                // Overfetch limit+1 to detect that more memories match than the
                // page shows (silent-cap family — see
                // docs/issues/archive/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
                let mut hits = store
                    .search(&project_id, &query_vec, limit + 1, bucket_filter)
                    .await?;
                let has_more = hits.len() > limit;
                hits.truncate(limit);

                let guard = super::output::OutputGuard::from_input(&input);
                let items: Vec<serde_json::Value> = hits
                    .iter()
                    .map(|h| {
                        let content = if guard.should_include_body() {
                            h.memory.content.clone()
                        } else {
                            let first_line = h.memory.content.lines().next().unwrap_or("").trim();
                            if first_line.chars().count() > 50 {
                                let mut end = 47.min(first_line.len());
                                while !first_line.is_char_boundary(end) {
                                    end -= 1;
                                }
                                format!("{}...", &first_line[..end])
                            } else {
                                first_line.to_string()
                            }
                        };
                        json!({
                            "id": h.id.to_string(),
                            "bucket": h.memory.bucket,
                            "title": h.memory.title,
                            "content": content,
                            "similarity": h.score.map(|s| format!("{s:.2}")).unwrap_or_default(),
                            "created_at": h.memory.created_at,
                        })
                    })
                    .collect();

                let count = items.len();
                let mut out = json!({
                    "results": items,
                    "count": count,
                    "has_more": has_more,
                });
                if has_more {
                    out["more_hint"] =
                        json!("more memories match; raise `limit` or refine `query`");
                }
                Ok(out)
            }
            "forget" => {
                let id_str = input["id"].as_str().ok_or_else(|| {
                    super::RecoverableError::with_hint(
                        "Missing required parameter 'id'",
                        "Pass the UUID string id from a recall result",
                    )
                })?;
                let id = uuid::Uuid::parse_str(id_str).map_err(|_| {
                    super::RecoverableError::with_hint(
                        format!("invalid memory id '{id_str}': not a UUID"),
                        "Pass the UUID string id from a recall result, e.g. \"3f2a...\"",
                    )
                })?;

                let project_id = ctx
                    .agent
                    .with_project_at(ctx.workspace_override.as_deref(), |p| {
                        Ok(p.config.project.name.clone())
                    })
                    .await?;

                let store = ctx.agent.semantic_memory_store().await?;
                store.delete(&project_id, id).await?;

                Ok(json!("ok"))
            }
            "refresh_anchors" => {
                let topic = require_topic_param(&input)?;
                let root = ctx.agent.require_project_root_for(ctx.workspace_override.as_deref()).await?;
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

                let dropped =
                    crate::memory::anchors::refresh_hashes(&root, &memories_dir, topic)?;
                if dropped.is_empty() {
                    return Ok(json!("ok"));
                }
                // Silent when there is nothing to say; when the anchor set shrank,
                // nothing else would tell the caller why.
                Ok(json!({
                    "status": "ok",
                    "dropped_machine_local": dropped,
                    "hint": "These paths are gitignored, so their hashes could not travel \
                             with this sidecar and made the memory stale by construction. \
                             They are no longer anchored.",
                }))
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

    fn output_form(&self) -> OutputForm {
        OutputForm::Text
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
