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
const HIDDEN_STATUSES: &[&str] = &["archived", "superseded"];

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
    /// Include archived/superseded rows. Ignored when the user filter
    /// already constrains `status`.
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
) -> Result<Value> {
    let mut hints = serde_json::Map::new();

    if scope_fallback {
        hints.insert(
            "scope_fallback".into(),
            json!("cwd is outside all workspace roots; defaulted to scope=all"),
        );
    }

    let here = count_for_scope(cat, base, ws, current, applied.scope)?;

    if !matches!(applied.scope, Scope::Repo | Scope::All) && current.is_some() {
        let in_repo = count_for_scope(cat, base, ws, current, Scope::Repo)?;
        let extra = in_repo.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_repo".into(), json!(extra));
        }
    }

    if !matches!(applied.scope, Scope::Umbrella | Scope::All)
        && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        let in_umbrella = count_for_scope(cat, base, ws, current, Scope::Umbrella)?;
        let extra = in_umbrella.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_umbrella".into(), json!(extra));
        }
    }

    // Only hint scope="all" when an umbrella exists — without one, workspace projects
    // are unrelated and crossing into them would be misleading.
    if !matches!(applied.scope, Scope::All) && current.and_then(|c| c.umbrella.as_deref()).is_some()
    {
        let in_workspace = count_for_scope(cat, base, ws, current, Scope::All)?;
        let extra = in_workspace.saturating_sub(here);
        if extra > 0 {
            hints.insert("more_in_workspace".into(), json!(extra));
        }
    }

    if !user_constrains_status && !include_archived {
        let stripped = base.cloned().map(strip_status_clause);
        let with_archived = count_for_scope(cat, stripped.as_ref(), ws, current, applied.scope)?;
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

fn count_for_scope(
    cat: &crate::librarian::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::librarian::workspace::WorkspaceConfig,
    current: Option<&crate::librarian::current_project::CurrentProject>,
    scope: Scope,
) -> Result<usize> {
    if matches!(scope, Scope::Project | Scope::Repo) && current.is_none() {
        return Ok(0);
    }
    if matches!(scope, Scope::Umbrella) && current.and_then(|c| c.umbrella.as_deref()).is_none() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(base.cloned(), scope, ws, current)?;
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
    let a: Args = serde_json::from_value(args)?;
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
            None => anyhow::bail!("semantic search requires an embedding service"),
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
    let (scoped_filter, applied) =
        apply_scope(base.clone(), effective_scope, &ctx.workspace, current)?;

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

        let rows = find(
            &cat,
            &FindOpts {
                filter: scoped_filter,
                limit,
                offset,
                semantic: semantic_vec,
            },
        )?;
        let items: Vec<Value> = rows
            .into_iter()
            .map(|r| {
                json!({
                    "id": r.id,
                    "kind": r.kind,
                    "status": r.status,
                    "title": r.title,
                    "abs_path": r.abs_path.display().to_string(),
                    "updated_at": r.updated_at,
                })
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
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::librarian::catalog::artifact::{self, ArtifactRow};
    use crate::librarian::catalog::Catalog;
    use crate::librarian::current_project::CurrentProject;
    use crate::librarian::embedding::EmbeddingService;
    use crate::librarian::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "code-explorer".into(),
                    path: "/tmp/code-explorer".into(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(CurrentProject {
                abs_path: std::path::PathBuf::from("/test/code-explorer"),
                git_root: std::path::PathBuf::from("/test/code-explorer"),
                umbrella: None,
            })),
        }
    }

    fn mk_ctx_with_embedder(cat: Catalog, svc: Arc<EmbeddingService>) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: Some(svc),
            current_project: None,
        }
    }

    fn sample_row(id: &str, title: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            abs_path: std::path::PathBuf::from(format!("/test/code-explorer/{id}.md")),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(title.into()),
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 1,
            file_mtime: 0,
            file_sha256: String::new(),
            confidence: 1.0,
        }
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
        let ctx_umbrella = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(make_cat())),
            workspace: Arc::new(crate::librarian::workspace::WorkspaceConfig {
                roots: vec![crate::librarian::workspace::Root {
                    name: "code-explorer".into(),
                    path: "/tmp/code-explorer".into(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![crate::librarian::workspace::Umbrella {
                    name: "main".into(),
                    members: vec![
                        std::path::PathBuf::from("/test/code-explorer"),
                        std::path::PathBuf::from("/test/agents"),
                    ],
                }],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    umbrella: Some("main".into()),
                },
            )),
        };
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
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "alpha")).unwrap();
        let ctx = mk_ctx(cat);
        let v = call(&ctx, json!({"limit": 10_000_000})).await.unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
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
            ArtifactRow {
                id: id.into(),
                abs_path: std::path::PathBuf::from(format!("/test/code-explorer/{id}.md")),
                kind: kind.into(),
                status: "active".into(),
                title: Some(id.into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: "".into(),
                confidence: 1.0,
            }
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
            ArtifactRow {
                id: id.into(),
                abs_path: std::path::PathBuf::from(format!("/test/code-explorer/{id}.md")),
                kind: kind.into(),
                status: status.into(),
                title: Some(id.into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: "".into(),
                confidence: 1.0,
            }
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
            ArtifactRow {
                id: id.into(),
                abs_path: std::path::PathBuf::from(format!("/test/code-explorer/{id}.md")),
                kind: "spec".into(),
                status: status.into(),
                title: Some(id.into()),
                owners: vec![],
                tags: vec![],
                topic: None,
                time_scope: None,
                source: None,
                created_at: 0,
                updated_at: 0,
                file_mtime: 0,
                file_sha256: "".into(),
                confidence: 1.0,
            }
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
        let ctx = ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(crate::librarian::workspace::WorkspaceConfig {
                roots: vec![crate::librarian::workspace::Root {
                    name: "code-explorer".into(),
                    path: "/tmp/code-explorer".into(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![crate::librarian::workspace::Umbrella {
                    name: "main".into(),
                    members: vec![std::path::PathBuf::from("/test/code-explorer")],
                }],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(
                crate::librarian::current_project::CurrentProject {
                    abs_path: std::path::PathBuf::from("/test/code-explorer"),
                    git_root: std::path::PathBuf::from("/test/code-explorer"),
                    umbrella: Some("main".into()),
                },
            )),
        };
        let result = call(&ctx, json!({"scope": "all"})).await.unwrap();
        assert_eq!(result["count"].as_u64(), Some(1));
    }
}
