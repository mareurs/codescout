use anyhow::Result;
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, Scope, ScopeApplied};
use super::{RecoverableError, ToolContext};
use crate::librarian::catalog::augmentation;
use crate::librarian::catalog::find::{catalog_summary, count_matching, find, FindOpts};
use crate::librarian::filter::FilterNode;

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;
use super::HIDDEN_STATUSES;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    filter: Option<FilterNode>,
    /// Shortcut: equivalent to filter `{kind: {eq: value}}`. Combined with `filter` via AND.
    #[serde(default)]
    kind: Option<String>,
    /// Shortcut: equivalent to filter `{status: {eq: value}}`. Disables archived-hide default.
    #[serde(default)]
    status: Option<String>,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    offset: usize,
    /// Natural-language query for semantic search. Requires embedding service.
    #[serde(default)]
    semantic: Option<String>,
    #[serde(default)]
    scope: Option<Scope>,
    /// Include archived/superseded/retired rows. Ignored when the user filter
    /// already constrains `status`.
    /// `retired` covers the in-place redirect pattern (MRV `2-lane-strategy.md`):
    /// the file stays at its original path with `status: retired` and a body
    /// that forwards to the canonical successor, so incoming links keep
    /// resolving while the tracker stops showing in active listings.
    #[serde(default)]
    include_archived: bool,
    /// Filter to augmented (true) or non-augmented (false) artifacts. Omit to return all.
    #[serde(default)]
    augmented: Option<bool>,
}

fn default_limit() -> usize {
    50
}

fn merge_kind_status(
    filter: Option<FilterNode>,
    kind: Option<&str>,
    status: Option<&str>,
) -> Option<FilterNode> {
    let mut parts: Vec<FilterNode> = Vec::new();
    if let Some(k) = kind {
        parts.push(FilterNode::Leaf(
            [("kind".to_string(), json!({"eq": k}))]
                .into_iter()
                .collect(),
        ));
    }
    if let Some(s) = status {
        parts.push(FilterNode::Leaf(
            [("status".to_string(), json!({"eq": s}))]
                .into_iter()
                .collect(),
        ));
    }
    if let Some(f) = filter {
        parts.push(f);
    }
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => Some(FilterNode::And { and: parts }),
    }
}

fn combine_user_with_archived_hide(
    user: Option<FilterNode>,
    include_archived: bool,
    user_constrains_status: bool,
) -> Option<FilterNode> {
    if include_archived || user_constrains_status {
        return user;
    }
    let hide = FilterNode::Leaf(
        [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
            .into_iter()
            .collect(),
    );
    Some(match user {
        Some(u) => FilterNode::And { and: vec![u, hide] },
        None => hide,
    })
}

/// Recursively check whether any leaf in `node` constrains the `status` field.
fn filter_mentions_status(node: &FilterNode) -> bool {
    match node {
        FilterNode::And { and } => and.iter().any(filter_mentions_status),
        FilterNode::Or { or } => or.iter().any(filter_mentions_status),
        FilterNode::Not { not } => filter_mentions_status(not),
        FilterNode::Leaf(map) => map.contains_key("status"),
    }
}

#[allow(clippy::too_many_arguments)]
fn build_hints(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    applied: &ScopeApplied,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope_fallback: bool,
    user_constrains_status: bool,
    include_archived: bool,
    returned_count: usize,
    offset: usize,
    exclude_worktrees: &[String],
    deduped_main_ids: &[String],
) -> Result<Value> {
    let mut hints = serde_json::Map::new();

    if scope_fallback {
        hints.insert(
            "scope_fallback".into(),
            json!("cwd is outside all workspace roots; defaulted to scope=all"),
        );
    }

    let here = count_for_scope(cat, base, ws, current, applied.scope, exclude_worktrees)?;
    // Overlay dedup (see `call`) drops main twins whose shadow lives under
    // this worktree session's own root before `items` is built, but `here` is
    // a raw scope COUNT that still counts both the shadow and its main twin.
    // Subtract however many of the deduped main ids are themselves counted in
    // `here` so `more_in_scope` reflects the post-dedup page, not the
    // pre-dedup union. A worktree session's own root is never excluded from
    // scope (see `exclude_worktrees`), so a deduped main id counted here
    // always has its shadow counted here too — the adjustment never
    // over-subtracts.
    let deduped_here = count_deduped_main_ids(
        cat,
        base,
        ws,
        current,
        applied.scope,
        exclude_worktrees,
        deduped_main_ids,
    )?;
    let here = here.saturating_sub(deduped_here);

    // More matched in THIS scope than were returned on this page? The result
    // set is capped by `limit`/`offset`; without this signal an agent reads the
    // returned page as the complete set (silent-cap bug — see
    // docs/issues/2026-07-10-silent-cap-missing-overflow-signals-audit.md).
    let shown_through = offset.saturating_add(returned_count);
    let more_in_scope = here.saturating_sub(shown_through);
    if more_in_scope > 0 {
        hints.insert("more_in_scope".into(), json!(more_in_scope));
        hints.insert(
            "more_in_scope_hint".into(),
            json!(
                "more artifacts match in this scope than were returned; \
                 raise `limit`, page with `offset`, or narrow the filter"
            ),
        );
    }

    if !matches!(applied.scope, Scope::Repo | Scope::All) && current.is_some() {
        let in_repo = count_for_scope(cat, base, ws, current, Scope::Repo, exclude_worktrees)?;
        let extra = in_repo.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_repo".into(), json!(extra));
        }
    }

    if !matches!(applied.scope, Scope::Umbrella | Scope::All)
        && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        let in_umbrella =
            count_for_scope(cat, base, ws, current, Scope::Umbrella, exclude_worktrees)?;
        let extra = in_umbrella.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_umbrella".into(), json!(extra));
        }
    }

    // Hint that more rows exist beyond the current scope only when there is a
    // BROADER reachable scope to widen to. `scope="all"` aliases to umbrella
    // whenever the project has one (see the alias in `call`), so at umbrella
    // scope the user is already as wide as the scope param can reach —
    // suggesting scope="all" there just re-aliases to umbrella (self-referential,
    // and it counts extra-umbrella catalog rows the alias can never reach).
    // Excluding Umbrella keeps this hint reachable and non-self-referential.
    // See docs/issues/2026-07-17-artifact-find-ignores-workspace-pin.md (sub-finding #2).
    if !matches!(applied.scope, Scope::All | Scope::Umbrella)
        && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        let in_workspace = count_for_scope(cat, base, ws, current, Scope::All, exclude_worktrees)?;
        let extra = in_workspace.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_workspace".into(), json!(extra));
        }
    }

    if !user_constrains_status && !include_archived {
        let stripped = base.cloned().map(strip_status_clause);
        let with_archived = count_for_scope(
            cat,
            stripped.as_ref(),
            ws,
            current,
            applied.scope,
            exclude_worktrees,
        )?;
        let hidden = with_archived.saturating_sub(here);
        if hidden > 0 {
            hints.insert("hidden_archived".into(), json!(hidden));
            hints.insert(
                "include_archived_hint".into(),
                json!("pass include_archived=true to surface archived/superseded rows"),
            );
        }
    }

    let mut expand = Vec::new();
    if hints.contains_key("more_in_repo") {
        expand.push("scope=\"repo\"");
    }
    if hints.contains_key("more_in_umbrella") {
        expand.push("scope=\"all\"");
    }
    if hints.contains_key("more_in_workspace") {
        expand.push("scope=\"all\"");
    }
    if !expand.is_empty() {
        hints.insert("expand".into(), json!(expand));
    }

    Ok(Value::Object(hints))
}

/// How many of `deduped_main_ids` are themselves counted by `count_for_scope`
/// under (`base`, `scope`). Used to correct `more_in_scope`: `find`'s overlay
/// dedup drops these ids from `items` before returning, but the raw scope
/// COUNT (`here`) still counts them — without this adjustment `more_in_scope`
/// overcounts by exactly the number of deduped main twins.
#[allow(clippy::too_many_arguments)]
fn count_deduped_main_ids(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope: Scope,
    exclude_worktrees: &[String],
    deduped_main_ids: &[String],
) -> Result<usize> {
    if deduped_main_ids.is_empty() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(base.cloned(), scope, ws, current, exclude_worktrees)?;
    let id_values: Vec<Value> = deduped_main_ids.iter().map(|id| json!(id)).collect();
    let id_filter = FilterNode::Leaf(
        [("id".to_string(), json!({"in": id_values}))]
            .into_iter()
            .collect(),
    );
    let combined = match filter {
        Some(f) => FilterNode::And {
            and: vec![f, id_filter],
        },
        None => id_filter,
    };
    count_matching(cat, Some(&combined))
}

fn count_for_scope(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope: Scope,
    exclude_worktrees: &[String],
) -> Result<usize> {
    if matches!(scope, Scope::Project | Scope::Repo) && current.is_none() {
        return Ok(0);
    }
    if matches!(scope, Scope::Umbrella) && current.and_then(|c| c.umbrella.as_deref()).is_none() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(base.cloned(), scope, ws, current, exclude_worktrees)?;
    count_matching(cat, filter.as_ref())
}

fn strip_status_clause(node: FilterNode) -> FilterNode {
    if let FilterNode::And { and } = node {
        let kept: Vec<FilterNode> = and
            .into_iter()
            .filter(|n| !is_status_nin_clause(n))
            .collect();
        if kept.len() == 1 {
            return kept.into_iter().next().unwrap();
        }
        return FilterNode::And { and: kept };
    }
    node
}

fn is_status_nin_clause(n: &FilterNode) -> bool {
    if let FilterNode::Leaf(map) = n {
        if let Some(ops) = map.get("status").and_then(|v| v.as_object()) {
            return ops.contains_key("nin");
        }
    }
    false
}
/// Extract the first `rel_path` contains/prefix value from a filter tree.
fn rel_path_hint(node: &FilterNode) -> Option<String> {
    match node {
        FilterNode::And { and } => and.iter().find_map(rel_path_hint),
        FilterNode::Or { or } => or.iter().find_map(rel_path_hint),
        FilterNode::Not { not } => rel_path_hint(not),
        FilterNode::Leaf(map) => map
            .get("rel_path")?
            .as_object()?
            .iter()
            .find_map(|(op, v)| {
                if matches!(op.as_str(), "contains" | "prefix") {
                    v.as_str().map(str::to_owned)
                } else {
                    None
                }
            }),
    }
}

/// Walk the current project directory for `.md` files whose repo-relative path
/// contains `hint`. Returns relative paths (relative to the repo root).
fn scan_unindexed_md(
    roots: &[crate::librarian::workspace::Root],
    cp: &crate::librarian::current_project::CurrentProject,
    hint: &str,
    ignore_patterns: &[String],
) -> Vec<String> {
    // Transitional bridge: derive legacy root/subdir from cp.git_root.
    let cp_root: String = cp
        .git_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let Some(root) = roots.iter().find(|r| r.name == cp_root) else {
        return vec![];
    };
    let base = root.path.clone();
    let ignore =
        crate::librarian::workspace::compile_ignore(ignore_patterns).unwrap_or_else(|_| {
            globset::GlobSetBuilder::new()
                .build()
                .expect("empty globset")
        });
    let mut found = Vec::new();
    let walker = ignore::WalkBuilder::new(&base)
        .standard_filters(true)
        .build();
    for entry in walker.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let rel = match path.strip_prefix(&root.path) {
            Ok(r) => crate::librarian::util::normalize_rel_path(&r.to_string_lossy()),
            Err(_) => continue,
        };
        if !ignore.is_match(&rel) && rel.contains(hint) {
            found.push(rel);
        }
    }
    found
}

pub async fn call(ctx: &ToolContext, args: Value) -> Result<Value> {
    let mut a: Args = serde_json::from_value(args)?;
    // Repair-and-continue: fix the deterministic inverted-leaf filter mistake
    // ({op:{field,value}} -> {field:{op:value}}) in place rather than erroring,
    // so a malformed filter costs zero retry round-trips. The corrections ride
    // back in the response so the agent still learns the canonical shape.
    let filter_corrections = a
        .filter
        .as_mut()
        .map(crate::librarian::filter::repair_inverted_leaves)
        .unwrap_or_default();
    let is_cold_call = a.filter.is_none()
        && a.semantic.is_none()
        && a.kind.is_none()
        && a.status.is_none()
        && a.augmented.is_none();
    let limit = a.limit.min(MAX_LIMIT);
    let offset = a.offset.min(MAX_OFFSET);

    // Resolve semantic query → embedding vector (if requested and available).
    let semantic_vec: Option<Vec<f32>> = if let Some(ref query) = a.semantic {
        match ctx.embedding.as_ref() {
            Some(svc) => Some(svc.embedder.embed_query(query).await?),
            // Correctable by the caller — drop `semantic` and the same query runs
            // as a plain filter. Recoverable, not a hard failure that aborts
            // sibling calls (omnibus 49ee6a03, F11).
            None => {
                return Err(RecoverableError::with_hint(
                    "semantic search requires an embedding service, which is not configured",
                    "Retry without the `semantic` param (use `filter` with a `contains` op on \
                     title/rel_path for a literal match), or configure an embedder.",
                ))
            }
        }
    } else {
        None
    };

    // Merge kind/status shortcut params into the base filter.
    let status_shortcut_set = a.status.is_some();
    let rel_path_filter_hint = a.filter.as_ref().and_then(rel_path_hint);
    let base_filter = merge_kind_status(a.filter, a.kind.as_deref(), a.status.as_deref());

    // Build augmented pre-filter if requested, then merge with user filter.
    let user_filter: Option<FilterNode> = if let Some(want_augmented) = a.augmented {
        let ids = {
            let cat = ctx.catalog.lock();
            augmentation::list_all_ids(&cat)?
        };
        if want_augmented {
            if ids.is_empty() {
                return Ok(json!({"count": 0, "items": [], "scope": Value::Null, "hints": {}}));
            }
            let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
            let in_node = FilterNode::Leaf(
                [("id".to_string(), json!({"in": id_values}))]
                    .into_iter()
                    .collect(),
            );
            Some(match base_filter {
                Some(f) => FilterNode::And {
                    and: vec![f, in_node],
                },
                None => in_node,
            })
        } else if ids.is_empty() {
            // Nothing is augmented → "non-augmented" = everything; base filter unchanged.
            base_filter
        } else {
            let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
            let nin_node = FilterNode::Leaf(
                [("id".to_string(), json!({"nin": id_values}))]
                    .into_iter()
                    .collect(),
            );
            Some(match base_filter {
                Some(f) => FilterNode::And {
                    and: vec![f, nin_node],
                },
                None => nin_node,
            })
        }
    } else {
        base_filter
    };

    let user_constrains_status = status_shortcut_set
        || user_filter
            .as_ref()
            .map(filter_mentions_status)
            .unwrap_or(false);
    let base =
        combine_user_with_archived_hide(user_filter, a.include_archived, user_constrains_status);

    let requested_scope = a.scope.unwrap_or_default();
    if a.scope == Some(Scope::All) {
        if let Some(cp) = ctx.current_project.as_deref() {
            if cp.umbrella.is_none() {
                return Err(RecoverableError::new(
                    "scope=\"all\" requires a configured umbrella — without one it crosses into \
                     unrelated workspace projects. Use scope=\"repo\" to widen to your repo, or \
                     configure [[umbrella]] in workspace.toml to group related projects.",
                ));
            }
        }
    }
    // scope=all is an alias for umbrella when the current project has one;
    // without a current project or umbrella, All passes through (no-cwd fallback path).
    let requested_scope = if requested_scope == Scope::All
        && ctx
            .current_project
            .as_deref()
            .and_then(|c| c.umbrella.as_deref())
            .is_some()
    {
        Scope::Umbrella
    } else {
        requested_scope
    };
    let (effective_scope, scope_fallback) = match (requested_scope, ctx.current_project.is_some()) {
        (Scope::Project | Scope::Repo, false) => (Scope::All, true),
        (s, _) => (s, false),
    };

    let current = ctx.current_project.as_deref();
    let exclude_worktrees: Vec<String> = {
        let cat = ctx.catalog.lock();
        let own = current
            .filter(|c| c.main_root.is_some())
            .map(|c| crate::util::fs::RepoPath::from(c.git_root.as_path()).into_string());
        crate::librarian::catalog::worktree::active_roots(&cat)?
            .into_iter()
            .filter(|r| own.as_deref() != Some(r.as_str()))
            .collect()
    };
    let (scoped_filter, applied) = apply_scope(
        base.clone(),
        effective_scope,
        &ctx.workspace,
        current,
        &exclude_worktrees,
    )?;

    // Semantic path runs the async store-backed coordinator (it manages its own
    // catalog locking); the sync `find` below handles the non-semantic case.
    let semantic_rows = if let Some(vec) = semantic_vec {
        let store = ctx.artifact_store.as_ref().ok_or_else(|| {
            RecoverableError::new(
                "artifact semantic search backend unavailable — the configured Qdrant is \
                 unreachable. Set `[librarian] vector_backend = \"sqlite-vec\"` (or \
                 CODESCOUT_ARTIFACT_BACKEND=sqlite-vec) for the offline backend.",
            )
        })?;
        // Project scope → stamp the parent workspace root (superset-safe for the
        // catalog scoped filter); other scopes search all projects.
        let project_id = if effective_scope == Scope::Project {
            current.and_then(|cp| {
                let roots: Vec<std::path::PathBuf> =
                    ctx.workspace.roots.iter().map(|r| r.path.clone()).collect();
                crate::librarian::tools::containing_root(&roots, &cp.abs_path)
                    .map(|p| p.to_string_lossy().into_owned())
            })
        } else {
            None
        };
        Some(
            crate::librarian::catalog::find::semantic_find(
                store.as_ref(),
                &ctx.catalog,
                project_id.as_deref(),
                &vec,
                scoped_filter.as_ref(),
                limit,
                offset,
            )
            .await?,
        )
    } else {
        None
    };

    let (items, hints, catalog_value) = {
        let cat = ctx.catalog.lock();

        let catalog_value: Option<serde_json::Value> = if is_cold_call {
            let summary = catalog_summary(&cat, scoped_filter.as_ref())?;
            Some(serde_json::json!({
                "total": summary.total,
                "by_kind": summary.by_kind,
                "augmented": summary.augmented,
            }))
        } else {
            None
        };

        let rows = match semantic_rows {
            Some(r) => r,
            None => find(
                &cat,
                &FindOpts {
                    filter: scoped_filter,
                    limit,
                    offset,
                },
            )?,
        };

        // Overlay dedup: a worktree session sees its shadow INSTEAD of the main
        // twin. `worktree_of` pairs (main_id, shadow_id) whose shadow lives
        // under THIS session's own worktree root identify the main ids to drop;
        // survivors carrying a shadow are flagged `overlay: true` below. Gated
        // on `main_root.is_some()` — a plain (non-worktree) session never runs
        // this query and its find results are unaffected. `shadow_main_pairs`
        // (shared with get.rs) wildcard-escapes the worktree-root LIKE pattern.
        let mut rows = rows;
        let mut overlay_ids: std::collections::HashSet<String> = Default::default();
        let mut deduped_main_ids: Vec<String> = Vec::new();
        if let Some(cp) = current.filter(|c| c.main_root.is_some()) {
            let wt = crate::util::fs::RepoPath::from(cp.git_root.as_path()).into_string();
            let pairs = crate::librarian::tools::worktree::shadow_main_pairs(&cat, &wt)?;
            let shadowed: std::collections::HashSet<&str> =
                pairs.iter().map(|(main_id, _)| main_id.as_str()).collect();
            rows.retain(|r| !shadowed.contains(r.id.as_str()));
            overlay_ids = pairs
                .iter()
                .map(|(_, shadow_id)| shadow_id.clone())
                .collect();
            deduped_main_ids = pairs.into_iter().map(|(main_id, _)| main_id).collect();
        }

        let items: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                let mut item = json!({
                    "id": r.id,
                    "kind": r.kind,
                    "status": r.status,
                    "title": r.title,
                    "abs_path": r.abs_path.display().to_string(),
                    "updated_at": r.updated_at,
                });
                if overlay_ids.contains(&r.id) {
                    item["overlay"] = json!(true);
                }
                item
            })
            .collect();

        // Hints only meaningful for non-semantic queries — semantic results are
        // KNN-bounded and a count comparison would be misleading.
        let hints = if a.semantic.is_some() {
            json!({})
        } else {
            build_hints(
                &cat,
                base.as_ref(),
                &applied,
                &ctx.workspace,
                current,
                scope_fallback,
                user_constrains_status,
                a.include_archived,
                items.len(),
                offset,
                &exclude_worktrees,
                &deduped_main_ids,
            )?
        };

        (items, hints, catalog_value)
    };

    // When a rel_path filter returns nothing, scan the filesystem for unindexed
    // matching files so the caller gets an actionable error instead of silent empty.
    if items.is_empty() && a.semantic.is_none() {
        if let Some(ref hint) = rel_path_filter_hint {
            if let Some(ref cp) = ctx.current_project {
                let unindexed =
                    scan_unindexed_md(&ctx.workspace.roots, cp, hint, &ctx.workspace.ignore);
                if !unindexed.is_empty() {
                    let sample = unindexed
                        .iter()
                        .take(5)
                        .cloned()
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(RecoverableError::new(format!(
                        "No indexed artifacts match rel_path ~ {hint:?}. \
                         Found {} unindexed file(s): {sample}. \
                         Run librarian(action=\"reindex\", scope=\"project\") to index them, then retry.",
                        unindexed.len()
                    )));
                }
            }
        }
    }

    let mut response = serde_json::json!({
        "count": items.len(),
        "items": items,
        "scope": applied.to_json(),
        "hints": hints,
    });
    if let Some(cat_val) = catalog_value {
        response["catalog"] = cat_val;
    }
    if !filter_corrections.is_empty() {
        response["corrections"] = serde_json::json!({
            "filter": filter_corrections,
            "hint": "Filter leaf shape is {field: {op: value}}, not {op: {field, value}}. \
                     Your filter was auto-corrected and the query ran; use the canonical shape next time.",
        });
    }
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow, TestArtifactRowBuilder};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::embedding::EmbeddingService;
    use crate::librarian::tools::TestToolContextBuilder;
    use crate::librarian::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: None,
                },
            ))
            .build()
    }

    fn mk_ctx_with_embedder(cat: Catalog, svc: Arc<EmbeddingService>) -> ToolContext {
        let catalog = Arc::new(parking_lot::Mutex::new(cat));
        ToolContext {
            lsp: crate::lsp::MockLspProvider::with_client(crate::lsp::MockLspClient::default()),
            catalog: Arc::clone(&catalog),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: Some(svc),
            // Exercise the semantic path against the in-memory artifact_vec via
            // the sqlite-vec backend (no Qdrant daemon in tests).
            artifact_store: Some(Arc::new(
                crate::librarian::artifact_store::SqliteVecArtifactStore::new(Arc::clone(&catalog)),
            )),
            current_project: None,
        }
    }

    fn sample_row(id: &str, title: &str) -> ArtifactRow {
        TestArtifactRowBuilder::new(id)
            .with_abs_path(format!("/test/code-explorer/{id}.md"))
            .with_title(title)
            .with_updated_at(1)
            .build()
    }

    #[tokio::test]
    async fn semantic_without_embedder_is_recoverable() {
        // BUG (omnibus 49ee6a03, F11): find() hard-failed via anyhow::bail! when
        // `semantic=` was passed with no embedding service configured. That is a
        // user-CORRECTABLE input error — the agent can simply retry without the
        // `semantic` param — so it must be a RecoverableError (isError:false at
        // the MCP boundary), not a hard failure that aborts sibling calls.
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat); // no embedder

        let err = call(&ctx, json!({"semantic": "some query"}))
            .await
            .expect_err("semantic without an embedder must error");

        assert!(
            err.downcast_ref::<RecoverableError>().is_some(),
            "must be RecoverableError so the agent can retry without `semantic`; got: {err}"
        );
    }

    #[tokio::test]
    async fn returns_rows_matching_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        artifact::upsert(&cat, &sample_row("b", "beta")).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn repairs_inverted_filter_and_notes_correction() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "or-tools guide")).unwrap();
        artifact::upsert(&cat, &sample_row("b", "unrelated")).unwrap();

        let ctx = mk_ctx(cat);
        // Inverted leaf {op:{field,value}} instead of {field:{op:value}} — the
        // handler repairs it and runs the query rather than erroring (no retry
        // round-trip), and rides a correction note back so the agent learns.
        let v = call(
            &ctx,
            json!({"filter": {"contains": {"field": "title", "value": "or-tools"}}}),
        )
        .await
        .expect("inverted filter should be repaired, not error");
        assert_eq!(v["count"].as_u64(), Some(1), "repaired query matches: {v}");
        assert!(
            v["corrections"]["filter"].is_array(),
            "correction note present: {v}"
        );
    }

    #[tokio::test]
    async fn defaults_hide_archived_when_filter_does_not_constrain_status() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut active = sample_row("a", "alpha");
        active.status = "active".into();
        let mut archived = sample_row("b", "beta");
        archived.status = "archived".into();
        artifact::upsert(&cat, &active).unwrap();
        artifact::upsert(&cat, &archived).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
        assert_eq!(v["hints"]["hidden_archived"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn status_in_filter_disables_archived_default() {
        let cat = Catalog::open_in_memory().unwrap();
        let mut archived = sample_row("a", "alpha");
        archived.status = "archived".into();
        artifact::upsert(&cat, &archived).unwrap();

        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"filter": {"status": {"eq": "archived"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn scope_all_widens_to_workspace() {
        let make_cat = || {
            let cat = Catalog::open_in_memory().unwrap();
            artifact::upsert(&cat, &sample_row("a", "in-project")).unwrap();
            let mut elsewhere = sample_row("b", "elsewhere");
            elsewhere.abs_path = std::path::PathBuf::from("/test/agents/x/y.md");
            artifact::upsert(&cat, &elsewhere).unwrap();
            cat
        };

        // Without umbrella: more_in_workspace hint must NOT appear — other repos are unrelated.
        let ctx = mk_ctx(make_cat());
        let v_default = call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v_default["count"].as_u64(), Some(1));
        assert!(
            v_default["hints"]["more_in_workspace"].is_null(),
            "no umbrella → more_in_workspace hint must be absent"
        );

        // With umbrella: more_in_workspace hint must appear.
        let ctx_umbrella = TestToolContextBuilder::new(make_cat())
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![
                    std::path::PathBuf::from("/test/code-explorer"),
                    std::path::PathBuf::from("/test/agents"),
                ],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();
        let v_umbrella = call(&ctx_umbrella, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v_umbrella["count"].as_u64(), Some(1));
        assert_eq!(
            v_umbrella["hints"]["more_in_workspace"].as_u64(),
            Some(1),
            "with umbrella → more_in_workspace hint must appear"
        );

        let v_all = call(
            &ctx_umbrella,
            json!({"filter": {"kind": {"eq": "spec"}}, "scope": "all"}),
        )
        .await
        .unwrap();
        assert_eq!(v_all["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn scope_all_does_not_self_reference_expand_hint() {
        // BUG (docs/issues/2026-07-17-artifact-find-ignores-workspace-pin.md,
        // sub-finding #2): passing scope="all" aliases to umbrella
        // (applied="umbrella"). build_hints counted rows OUTSIDE the umbrella —
        // unreachable, since "all" always re-aliases to umbrella — and emitted
        // expand:["scope=\"all\""], suggesting the exact param already passed.
        // At the broadest reachable scope there is nothing to widen to.
        let cat = Catalog::open_in_memory().unwrap();
        // One row INSIDE the umbrella, one OUTSIDE it (a foreign / ghost repo,
        // mirroring the deleted-repo + /tmp rows the real shared catalog holds).
        let mut inside = sample_row("a", "in-umbrella");
        inside.abs_path = std::path::PathBuf::from("/test/agents/x/y.md");
        artifact::upsert(&cat, &inside).unwrap();
        let mut outside = sample_row("b", "outside-umbrella");
        outside.abs_path = std::path::PathBuf::from("/other/ghost/z.md");
        artifact::upsert(&cat, &outside).unwrap();

        let ctx = TestToolContextBuilder::new(cat)
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![
                    std::path::PathBuf::from("/test/code-explorer"),
                    std::path::PathBuf::from("/test/agents"),
                ],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();

        let v = call(
            &ctx,
            json!({"filter": {"kind": {"eq": "spec"}}, "scope": "all"}),
        )
        .await
        .unwrap();
        // scope="all" aliases to umbrella → only the in-umbrella row is reachable.
        assert_eq!(v["scope"]["applied"], "umbrella");
        assert_eq!(v["count"].as_u64(), Some(1));
        // No widen hint at the broadest reachable scope; in particular no
        // count of the unreachable extra-umbrella row.
        assert!(
            v["hints"]["more_in_workspace"].is_null(),
            "at umbrella scope there is nothing broader to reach; got hints: {}",
            v["hints"]
        );
        // And crucially the expand list must never suggest scope="all" — the
        // very parameter that was passed (the self-referential bug).
        let suggests_all = v["hints"]["expand"]
            .as_array()
            .is_some_and(|e| e.iter().any(|s| s == "scope=\"all\""));
        assert!(
            !suggests_all,
            "expand must not suggest the already-passed scope=\"all\"; got hints: {}",
            v["hints"]
        );
    }

    #[tokio::test]
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"limit": 10_000_000})).await.unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
    }

    #[tokio::test]
    async fn more_in_scope_signals_capped_page() {
        // Regression for the silent-cap family: a limit-capped page must signal
        // that more match in scope. docs/issues/2026-07-10-silent-cap-missing-overflow-signals-audit.md
        let cat = Catalog::open_in_memory().unwrap();
        for i in 0..3 {
            artifact::upsert(&cat, &sample_row(&format!("id{i}"), &format!("t{i}"))).unwrap();
        }
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"limit": 2})).await.unwrap();
        assert_eq!(v["count"].as_u64(), Some(2), "page size stays items.len()");
        assert_eq!(
            v["hints"]["more_in_scope"].as_u64(),
            Some(1),
            "3 match, 2 returned → 1 more in scope"
        );
    }

    #[tokio::test]
    async fn no_more_in_scope_when_page_holds_everything() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({})).await.unwrap();
        assert!(
            v["hints"]["more_in_scope"].is_null(),
            "nothing capped → no more_in_scope signal"
        );
    }
    #[tokio::test]
    async fn worktree_find_shadows_main_twin_and_flags_overlay() {
        use crate::librarian::tools::worktree::test_support::{seed_main_tracker, wt_ctx};
        let ctx = wt_ctx(Catalog::open_in_memory().unwrap());
        let main_id = {
            let c = ctx.catalog.lock();
            seed_main_tracker(&c)
        };
        let shadow_id = {
            let mut c = ctx.catalog.lock();
            crate::librarian::tools::worktree::resolve_write_target(&mut c, &ctx, &main_id).unwrap()
        };
        let out = call(&ctx, serde_json::json!({"scope": "repo"}))
            .await
            .unwrap();
        let ids: Vec<&str> = out["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| i["id"].as_str().unwrap())
            .collect();
        assert!(ids.contains(&shadow_id.as_str()), "shadow visible: {ids:?}");
        assert!(
            !ids.contains(&main_id.as_str()),
            "main twin suppressed: {ids:?}"
        );
        let shadow_item = out["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["id"] == shadow_id.as_str())
            .unwrap();
        assert_eq!(shadow_item["overlay"], true);
        // Canonical Defect-2 regression: `here` (a raw scope COUNT) counts BOTH
        // the shadow and its deduped main twin, but `items.len()` passed to
        // `build_hints` is post-dedup — 1 tracker + 1 fork, scope=repo, default
        // paging must page-complete with NO more_in_scope hint (nothing is left
        // to page to).
        assert!(
            out["hints"].get("more_in_scope").is_none(),
            "spurious more_in_scope after dedup: {:?}",
            out["hints"]
        );
        assert!(
            out["hints"].get("more_in_scope_hint").is_none(),
            "spurious more_in_scope_hint after dedup: {:?}",
            out["hints"]
        );
    }

    #[tokio::test]
    async fn non_worktree_find_does_not_dedup_or_flag_despite_worktree_of_link() {
        // A worktree_of link existing in the catalog must never affect a plain
        // (non-worktree) session's find results — the dedup is gated on
        // `main_root.is_some()`, not merely on whether a matching link exists.
        use crate::librarian::catalog::links::{self, LinkRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("main-1", "Main")).unwrap();
        artifact::upsert(&cat, &sample_row("shadow-1", "Shadow")).unwrap();
        links::insert(
            &cat,
            &LinkRow {
                src_id: "shadow-1".into(),
                dst_id: "main-1".into(),
                rel: "worktree_of".into(),
                created_at: 0,
            },
        )
        .unwrap();

        let ctx = mk_ctx(cat);
        let out = call(
            &ctx,
            json!({"filter": {"id": {"in": ["main-1", "shadow-1"]}}}),
        )
        .await
        .unwrap();
        let items = out["items"].as_array().unwrap();
        let ids: Vec<&str> = items.iter().map(|i| i["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"main-1"), "main not suppressed: {ids:?}");
        assert!(ids.contains(&"shadow-1"), "shadow not suppressed: {ids:?}");
        for item in items {
            assert!(
                item.get("overlay").is_none(),
                "no overlay flag for non-worktree session: {item:?}"
            );
        }
    }

    struct MockEmbedder;

    #[async_trait::async_trait]
    impl codescout_embed::Embedder for MockEmbedder {
        fn dimensions(&self) -> usize {
            768
        }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<codescout_embed::Embedding>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0f32; 768];
                    if t.contains("auth") {
                        v[0] = 1.0;
                    } else {
                        v[1] = 1.0;
                    }
                    v
                })
                .collect())
        }
    }

    #[tokio::test]
    async fn semantic_search_returns_closest_artifact_first() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();

        let auth_blob: Vec<u8> = {
            let mut v = vec![0.0f32; 768];
            v[0] = 1.0;
            v.iter().flat_map(|f| f.to_le_bytes()).collect()
        };
        let deploy_blob: Vec<u8> = {
            let mut v = vec![0.0f32; 768];
            v[1] = 1.0;
            v.iter().flat_map(|f| f.to_le_bytes()).collect()
        };
        cat.conn
            .execute(
                "INSERT OR REPLACE INTO artifact_vec (id, embedding) VALUES (?1, ?2)",
                rusqlite::params!["auth-doc", auth_blob],
            )
            .unwrap();
        cat.conn
            .execute(
                "INSERT OR REPLACE INTO artifact_vec (id, embedding) VALUES (?1, ?2)",
                rusqlite::params!["deploy-doc", deploy_blob],
            )
            .unwrap();

        let svc = Arc::new(EmbeddingService::new(Arc::new(MockEmbedder)));
        let ctx = mk_ctx_with_embedder(cat, svc);

        let v = call(
            &ctx,
            json!({"semantic": "auth login flow", "limit": 10, "scope": "all"}),
        )
        .await
        .unwrap();

        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 2, "both artifacts should be returned");
        assert_eq!(items[0]["id"], "auth-doc");
    }

    #[tokio::test]
    async fn augmented_true_returns_only_augmented() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("aug", "Augmented")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"augmented": true})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "aug");
    }

    #[tokio::test]
    async fn augmented_false_returns_only_non_augmented() {
        use crate::librarian::catalog::augmentation::{self, AugmentationRow};
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("plain", "Plain")).unwrap();
        artifact::upsert(&cat, &sample_row("aug", "Augmented")).unwrap();
        augmentation::upsert(
            &cat,
            &AugmentationRow {
                artifact_id: "aug".to_string(),
                prompt: "p".to_string(),
                params: "{}".to_string(),
                last_refreshed_at: None,
                refresh_count: 0,
                created_at: "2026-01-01T00:00:00.000Z".to_string(),
                updated_at: "2026-01-01T00:00:00.000Z".to_string(),
                render_template: None,
                params_schema: None,
                append_mode: false,
                history_cap: None,
                entry_collection: None,
                refreshed_at_commit: None,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"augmented": false})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "plain");
    }

    #[tokio::test]
    async fn kind_shortcut_filters_by_kind() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, kind: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_kind(kind)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("spec-1", "spec")).unwrap();
        upsert(&cat, &row("plan-1", "plan")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"kind": "spec"})).await.unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "spec-1");
    }

    #[tokio::test]
    async fn kind_and_filter_combine_with_and() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, kind: &str, status: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_kind(kind)
                .with_status(status)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("spec-active", "spec", "active")).unwrap();
        upsert(&cat, &row("spec-draft", "spec", "draft")).unwrap();
        upsert(&cat, &row("plan-active", "plan", "active")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(
            &ctx,
            json!({
                "kind": "spec",
                "filter": {"status": {"eq": "active"}},
                "include_archived": true
            }),
        )
        .await
        .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "spec-active");
    }

    #[tokio::test]
    async fn status_shortcut_filters_by_status() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = Catalog::open_in_memory().unwrap();
        fn row(id: &str, status: &str) -> ArtifactRow {
            TestArtifactRowBuilder::new(id)
                .with_abs_path(format!("/test/code-explorer/{id}.md"))
                .with_status(status)
                .with_title(id)
                .build()
        }
        upsert(&cat, &row("a", "active")).unwrap();
        upsert(&cat, &row("d", "draft")).unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, json!({"status": "active", "include_archived": true}))
            .await
            .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "a");
    }

    #[tokio::test]
    async fn cold_call_returns_catalog_field() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a1".into(),
                abs_path: std::path::PathBuf::from("/test/code-explorer/docs/a1.md"),
                kind: "tracker".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({})).await.unwrap();
        assert!(
            result["catalog"].is_object(),
            "cold call must include catalog field"
        );
        assert_eq!(result["catalog"]["total"], 1);
        assert_eq!(result["catalog"]["by_kind"]["tracker"], 1);
        assert_eq!(result["catalog"]["augmented"], 0);
    }

    #[tokio::test]
    async fn find_with_kind_filter_omits_catalog_field() {
        use crate::librarian::catalog::artifact::{upsert, ArtifactRow};
        let cat = crate::librarian::catalog::Catalog::open_in_memory().unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        upsert(
            &cat,
            &ArtifactRow {
                id: "a1".into(),
                abs_path: std::path::PathBuf::from("/test/code-explorer/docs/a1.md"),
                kind: "tracker".into(),
                status: "draft".into(),
                title: None,
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: now,
                updated_at: now,
                file_mtime: now,
                file_sha256: "".into(),
                confidence: 1.0,
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = call(&ctx, serde_json::json!({"kind": "tracker"}))
            .await
            .unwrap();
        assert!(
            result.get("catalog").is_none() || result["catalog"].is_null(),
            "filtered find must not include catalog field"
        );
    }
    #[tokio::test]
    async fn scope_all_blocked_without_umbrella() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        let ctx = mk_ctx(cat);
        let err = call(&ctx, json!({"scope": "all"})).await.unwrap_err();
        assert!(
            err.downcast_ref::<crate::librarian::tools::RecoverableError>()
                .is_some(),
            "scope=all without umbrella must be RecoverableError, got: {err}"
        );
        assert!(
            err.to_string().contains("umbrella"),
            "error must mention umbrella"
        );
    }

    #[tokio::test]
    async fn scope_all_allowed_with_umbrella() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        let ctx = TestToolContextBuilder::new(cat)
            .with_root(crate::librarian::workspace::Root {
                name: "code-explorer".into(),
                path: "/tmp/code-explorer".into(),
            })
            .with_umbrellas(vec![crate::librarian::workspace::Umbrella {
                name: "main".into(),
                members: vec![std::path::PathBuf::from("/test/code-explorer")],
            }])
            .with_current_project(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    main_root: None,
                    umbrella: Some("main".into()),
                },
            ))
            .build();
        let result = call(&ctx, json!({"scope": "all"})).await.unwrap();
        assert_eq!(result["count"].as_u64(), Some(1));
    }
}
