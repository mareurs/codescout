use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, Scope, ScopeApplied};
use super::{Tool, ToolContext};
use crate::catalog::augmentation;
use crate::catalog::find::{count_matching, find, FindOpts};
use crate::filter::FilterNode;

pub struct ArtifactFind;

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;
const HIDDEN_STATUSES: &[&str] = &["archived", "superseded"];

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    filter: Option<FilterNode>,
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

#[async_trait]
impl Tool for ArtifactFind {
    fn name(&self) -> &'static str {
        "artifact_find"
    }

    fn description(&self) -> &'static str {
        "Search artifacts by filter AST (kind/status/tags/updated_at etc). \
         Composition: and/or/not. Leaf ops: eq/ne/in/nin/gt/lt/gte/lte/contains/prefix. \
         Defaults: scope=project (current sub-project only), archived/superseded hidden \
         when the filter does not constrain status. Pass scope=repo|umbrella|all to widen, \
         include_archived=true to surface archived rows."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter": {"type": "object"},
                "limit": {"type": "integer", "default": 50, "maximum": 500},
                "offset": {"type": "integer", "default": 0, "maximum": 100000},
                "semantic": {"type": "string", "description": "Natural-language query for semantic search (requires embedder)"},
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project"
                },
                "include_archived": {
                    "type": "boolean",
                    "default": false
                },
                "augmented": {
                    "type": "boolean",
                    "description": "Filter to augmented (true) or non-augmented (false) artifacts. Omit to return all."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
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
                Some(match a.filter {
                    Some(f) => FilterNode::And {
                        and: vec![f, in_node],
                    },
                    None => in_node,
                })
            } else if ids.is_empty() {
                // Nothing is augmented → "non-augmented" = everything; user filter unchanged.
                a.filter
            } else {
                let id_values: Vec<Value> = ids.into_iter().map(|id| json!(id)).collect();
                let nin_node = FilterNode::Leaf(
                    [("id".to_string(), json!({"nin": id_values}))]
                        .into_iter()
                        .collect(),
                );
                Some(match a.filter {
                    Some(f) => FilterNode::And {
                        and: vec![f, nin_node],
                    },
                    None => nin_node,
                })
            }
        } else {
            a.filter
        };

        let user_constrains_status = user_filter
            .as_ref()
            .map(filter_mentions_status)
            .unwrap_or(false);
        let base = combine_user_with_archived_hide(
            user_filter,
            a.include_archived,
            user_constrains_status,
        );

        let requested_scope = a.scope.unwrap_or_default();
        let (effective_scope, scope_fallback) =
            match (requested_scope, ctx.current_project.is_some()) {
                (Scope::Project | Scope::Repo, false) => (Scope::All, true),
                (s, _) => (s, false),
            };

        let current = ctx.current_project.as_deref();
        let (scoped_filter, applied) =
            apply_scope(base.clone(), effective_scope, &ctx.workspace, current)?;

        let cat = ctx.catalog.lock();
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
                    "repo": r.repo,
                    "rel_path": r.rel_path,
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

        Ok(json!({
            "count": items.len(),
            "items": items,
            "scope": applied.to_json(),
            "hints": hints,
        }))
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
    cat: &crate::catalog::Catalog,
    base: Option<&FilterNode>,
    applied: &ScopeApplied,
    ws: &crate::workspace::WorkspaceConfig,
    current: Option<&crate::current_project::CurrentProject>,
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

    if !matches!(applied.scope, Scope::All) {
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
        expand.push("scope=\"umbrella\"");
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
    cat: &crate::catalog::Catalog,
    base: Option<&FilterNode>,
    ws: &crate::workspace::WorkspaceConfig,
    current: Option<&crate::current_project::CurrentProject>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use crate::catalog::Catalog;
    use crate::current_project::CurrentProject;
    use crate::embedding::EmbeddingService;
    use crate::workspace::{Root, WorkspaceConfig};
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![Root {
                    name: "claude".into(),
                    path: "/tmp/claude".into(),
                }],
                ignore: vec![],
                rules: vec![],
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: Some(Arc::new(CurrentProject {
                root: "claude".into(),
                subdir: "code-explorer".into(),
                umbrella: None,
                ..Default::default()
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
            repo: "claude".into(),
            rel_path: format!("code-explorer/{id}.md"),
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
        let v = ArtifactFind
            .call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
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
        let v = ArtifactFind
            .call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
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
        let v = ArtifactFind
            .call(&ctx, json!({"filter": {"status": {"eq": "archived"}}}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn scope_all_widens_to_workspace() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "in-project")).unwrap();
        let mut elsewhere = sample_row("b", "elsewhere");
        elsewhere.repo = "agents".into();
        elsewhere.rel_path = "x/y.md".into();
        artifact::upsert(&cat, &elsewhere).unwrap();

        let ctx = mk_ctx(cat);
        let v_default = ArtifactFind
            .call(&ctx, json!({"filter": {"kind": {"eq": "spec"}}}))
            .await
            .unwrap();
        assert_eq!(v_default["count"].as_u64(), Some(1));
        assert_eq!(v_default["hints"]["more_in_workspace"].as_u64(), Some(1));

        let v_all = ArtifactFind
            .call(
                &ctx,
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
        let v = ArtifactFind
            .call(&ctx, json!({"limit": 10_000_000}))
            .await
            .unwrap();
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

        let v = ArtifactFind
            .call(
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
        use crate::catalog::augmentation::{self, AugmentationRow};
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
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactFind
            .call(&ctx, json!({"augmented": true, "scope": "all"}))
            .await
            .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "aug");
    }

    #[tokio::test]
    async fn augmented_false_returns_only_non_augmented() {
        use crate::catalog::augmentation::{self, AugmentationRow};
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
            },
        )
        .unwrap();
        let ctx = mk_ctx(cat);
        let result = ArtifactFind
            .call(&ctx, json!({"augmented": false, "scope": "all"}))
            .await
            .unwrap();
        let items = result["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["id"], "plain");
    }
}
