use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::scope::{apply_scope, Scope, ScopeApplied};
use super::{Tool, ToolContext};
use crate::catalog::find::{count_matching, find, FindOpts};
use crate::filter::FilterNode;

pub struct ArtifactListByKind;

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;
const HIDDEN_STATUSES: &[&str] = &["archived", "superseded"];

#[derive(Deserialize)]
struct Args {
    kind: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    scope: Option<Scope>,
    /// Include `archived` and `superseded` rows. Default false. Has no
    /// effect when `status` is set explicitly — the explicit value wins.
    #[serde(default)]
    include_archived: bool,
}

#[async_trait]
impl Tool for ArtifactListByKind {
    fn name(&self) -> &'static str {
        "artifact_list_by_kind"
    }

    fn description(&self) -> &'static str {
        "List artifacts of a given kind. Defaults: scope=project (current sub-project only), \
         archived/superseded hidden. Pass scope=repo|umbrella|all to widen, include_archived=true \
         to surface archived rows. Response includes a `scope` block and `hints` for widening."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["kind"],
            "properties": {
                "kind": {"type": "string"},
                "status": {"type": "string"},
                "limit": {"type": "integer", "default": 50, "maximum": 500},
                "offset": {"type": "integer", "default": 0, "maximum": 100000},
                "scope": {
                    "type": "string",
                    "enum": ["project", "repo", "umbrella", "all"],
                    "default": "project",
                    "description": "project = current sub-project (default); repo = whole current root; umbrella = declared umbrella members; all = workspace-wide."
                },
                "include_archived": {
                    "type": "boolean",
                    "default": false,
                    "description": "Include archived/superseded rows. Ignored when `status` is set."
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let limit = a.limit.unwrap_or(50).min(MAX_LIMIT);
        let offset = a.offset.unwrap_or(0).min(MAX_OFFSET);

        // No current project resolved → silently widen to All so the tool
        // still works (e.g. CI, cwd outside roots) and surface the fact in hints.
        let requested_scope = a.scope.unwrap_or_default();
        let (effective_scope, scope_fallback) =
            match (requested_scope, ctx.current_project.is_some()) {
                (Scope::Project | Scope::Repo, false) => (Scope::All, true),
                (s, _) => (s, false),
            };

        let base = build_base_filter(&a.kind, a.status.as_deref(), a.include_archived);
        let current = ctx.current_project.as_deref();
        let (scoped_filter, applied) =
            apply_scope(Some(base.clone()), effective_scope, &ctx.workspace, current)?;

        let cat = ctx.catalog.lock();
        let rows = find(
            &cat,
            &FindOpts {
                filter: scoped_filter.clone(),
                limit,
                offset,
                semantic: None,
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

        let hints = build_hints(
            &cat,
            &base,
            &applied,
            &ctx.workspace,
            current,
            scope_fallback,
            a.status.is_some(),
            a.include_archived,
        )?;

        Ok(json!({
            "count": items.len(),
            "items": items,
            "scope": applied.to_json(),
            "hints": hints,
        }))
    }
}

/// Filter that captures kind + (optional) status + (default) hide-archived.
/// The scope clause is layered on top of this in `apply_scope`.
fn build_base_filter(kind: &str, status: Option<&str>, include_archived: bool) -> FilterNode {
    let kind_node = FilterNode::Leaf(
        [("kind".to_string(), json!({"eq": kind}))]
            .into_iter()
            .collect(),
    );

    let status_node = match (status, include_archived) {
        (Some(s), _) => Some(FilterNode::Leaf(
            [("status".to_string(), json!({"eq": s}))]
                .into_iter()
                .collect(),
        )),
        (None, false) => Some(FilterNode::Leaf(
            [("status".to_string(), json!({"nin": HIDDEN_STATUSES}))]
                .into_iter()
                .collect(),
        )),
        (None, true) => None,
    };

    match status_node {
        Some(s) => FilterNode::And {
            and: vec![kind_node, s],
        },
        None => kind_node,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_hints(
    cat: &crate::catalog::Catalog,
    base: &FilterNode,
    applied: &ScopeApplied,
    ws: &crate::workspace::WorkspaceConfig,
    current: Option<&crate::current_project::CurrentProject>,
    scope_fallback: bool,
    user_set_status: bool,
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

    if !user_set_status && !include_archived {
        // Same scope but counting WITH archived → diff = hidden archived rows.
        let with_archived =
            count_for_scope_unfiltered_status(cat, base, ws, current, applied.scope)?;
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
    base: &FilterNode,
    ws: &crate::workspace::WorkspaceConfig,
    current: Option<&crate::current_project::CurrentProject>,
    scope: Scope,
) -> Result<usize> {
    // Project/Repo/Umbrella scoping requires a current project — skip silently.
    if matches!(scope, Scope::Project | Scope::Repo) && current.is_none() {
        return Ok(0);
    }
    if matches!(scope, Scope::Umbrella) && current.and_then(|c| c.umbrella.as_deref()).is_none() {
        return Ok(0);
    }
    let (filter, _) = apply_scope(Some(base.clone()), scope, ws, current)?;
    count_matching(cat, filter.as_ref())
}

/// Count at `scope` with the archived/superseded exclusion stripped from `base`.
/// Used to compute `hidden_archived`.
fn count_for_scope_unfiltered_status(
    cat: &crate::catalog::Catalog,
    base: &FilterNode,
    ws: &crate::workspace::WorkspaceConfig,
    current: Option<&crate::current_project::CurrentProject>,
    scope: Scope,
) -> Result<usize> {
    let stripped = strip_status_clause(base);
    count_for_scope(cat, &stripped, ws, current, scope)
}

/// Remove the top-level `status nin [...]` clause that `build_base_filter`
/// inserts when neither user-status nor include_archived is set. Idempotent.
fn strip_status_clause(node: &FilterNode) -> FilterNode {
    if let FilterNode::And { and } = node {
        let kept: Vec<FilterNode> = and
            .iter()
            .filter(|n| !is_status_nin_clause(n))
            .cloned()
            .collect();
        if kept.len() == 1 {
            return kept.into_iter().next().unwrap();
        }
        return FilterNode::And { and: kept };
    }
    node.clone()
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
    use crate::workspace::{Root, Umbrella, WorkspaceConfig};
    use std::sync::Arc;

    fn mk_ctx_with(
        cat: Catalog,
        ws: WorkspaceConfig,
        current: Option<CurrentProject>,
    ) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(ws),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: current.map(Arc::new),
        }
    }

    fn mk_row(id: &str, kind: &str, status: &str, repo: &str, rel_path: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: repo.into(),
            rel_path: rel_path.into(),
            kind: kind.into(),
            status: status.into(),
            title: None,
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

    fn ws_with_root() -> WorkspaceConfig {
        WorkspaceConfig {
            roots: vec![Root {
                name: "claude".into(),
                path: "/tmp/claude".into(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![],
        }
    }

    #[tokio::test]
    async fn defaults_to_project_scope_and_hides_archived() {
        let cat = Catalog::open_in_memory().unwrap();
        // Three trackers in same repo: one in current project, one elsewhere in repo, one archived in current project.
        artifact::upsert(
            &cat,
            &mk_row(
                "here_a",
                "tracker",
                "active",
                "claude",
                "code-explorer/a.md",
            ),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row(
                "here_b",
                "tracker",
                "archived",
                "claude",
                "code-explorer/b.md",
            ),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row("there", "tracker", "active", "claude", "other-proj/c.md"),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row("far", "tracker", "active", "agents", "x/y.md"),
        )
        .unwrap();

        let cp = CurrentProject {
            root: "claude".into(),
            subdir: "code-explorer".into(),
            umbrella: None,
        };
        let ctx = mk_ctx_with(cat, ws_with_root(), Some(cp));

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker"}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(1));
        assert_eq!(v["items"][0]["id"], "here_a");

        let hints = &v["hints"];
        assert_eq!(hints["hidden_archived"].as_u64(), Some(1));
        assert_eq!(hints["more_in_repo"].as_u64(), Some(1));
        assert_eq!(hints["more_in_workspace"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn include_archived_surfaces_them() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "active", "claude", "code-explorer/a.md"),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row("b", "tracker", "archived", "claude", "code-explorer/b.md"),
        )
        .unwrap();

        let cp = CurrentProject {
            root: "claude".into(),
            subdir: "code-explorer".into(),
            umbrella: None,
        };
        let ctx = mk_ctx_with(cat, ws_with_root(), Some(cp));

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker", "include_archived": true}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(2));
        assert!(v["hints"].get("hidden_archived").is_none());
    }

    #[tokio::test]
    async fn scope_all_returns_workspace_wide() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "active", "claude", "code-explorer/a.md"),
        )
        .unwrap();
        artifact::upsert(&cat, &mk_row("b", "tracker", "active", "agents", "x/y.md")).unwrap();

        let cp = CurrentProject {
            root: "claude".into(),
            subdir: "code-explorer".into(),
            umbrella: None,
        };
        let ctx = mk_ctx_with(cat, ws_with_root(), Some(cp));

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker", "scope": "all"}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(2));
        assert!(v["hints"].get("more_in_workspace").is_none());
    }

    #[tokio::test]
    async fn no_current_project_falls_back_to_all() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "active", "claude", "code-explorer/a.md"),
        )
        .unwrap();
        let ctx = mk_ctx_with(cat, ws_with_root(), None);

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker"}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(1));
        assert!(v["hints"]["scope_fallback"].is_string());
    }

    #[tokio::test]
    async fn umbrella_scope_aggregates_members() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "active", "infra", "svc-a/a.md"),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row("b", "tracker", "active", "infra", "svc-b/b.md"),
        )
        .unwrap();
        artifact::upsert(
            &cat,
            &mk_row("c", "tracker", "active", "infra", "svc-c/c.md"),
        )
        .unwrap();

        let ws = WorkspaceConfig {
            roots: vec![Root {
                name: "infra".into(),
                path: "/tmp/infra".into(),
            }],
            ignore: vec![],
            rules: vec![],
            umbrellas: vec![Umbrella {
                name: "platform".into(),
                members: vec!["infra/svc-a".into(), "infra/svc-b".into()],
            }],
        };
        let cp = CurrentProject {
            root: "infra".into(),
            subdir: "svc-a".into(),
            umbrella: Some("platform".into()),
        };
        let ctx = mk_ctx_with(cat, ws, Some(cp));

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker", "scope": "umbrella"}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn explicit_status_overrides_archived_default() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "archived", "claude", "code-explorer/a.md"),
        )
        .unwrap();
        let cp = CurrentProject {
            root: "claude".into(),
            subdir: "code-explorer".into(),
            umbrella: None,
        };
        let ctx = mk_ctx_with(cat, ws_with_root(), Some(cp));

        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker", "status": "archived"}))
            .await
            .unwrap();

        assert_eq!(v["count"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(
            &cat,
            &mk_row("a", "tracker", "active", "claude", "code-explorer/a.md"),
        )
        .unwrap();
        let cp = CurrentProject {
            root: "claude".into(),
            subdir: "code-explorer".into(),
            umbrella: None,
        };
        let ctx = mk_ctx_with(cat, ws_with_root(), Some(cp));
        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "tracker", "limit": 10_000_000}))
            .await
            .unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
    }
}
