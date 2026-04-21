use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::find::{find, FindOpts};
use crate::filter::FilterNode;

pub struct ArtifactListByKind;

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;

#[derive(Deserialize)]
struct Args {
    kind: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    offset: Option<usize>,
}

#[async_trait]
impl Tool for ArtifactListByKind {
    fn name(&self) -> &'static str {
        "artifact_list_by_kind"
    }

    fn description(&self) -> &'static str {
        "List artifacts of a given kind, optionally filtered by status."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["kind"],
            "properties": {
                "kind": {"type": "string"},
                "status": {"type": "string"},
                "limit": {"type": "integer", "default": 50, "maximum": 500},
                "offset": {"type": "integer", "default": 0, "maximum": 100000}
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let limit = a.limit.unwrap_or(50).min(MAX_LIMIT);
        let offset = a.offset.unwrap_or(0).min(MAX_OFFSET);
        let filter = build_filter(&a.kind, a.status.as_deref());
        let cat = ctx.catalog.lock();
        let rows = find(
            &cat,
            &FindOpts {
                filter: Some(filter),
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
        Ok(json!({"count": items.len(), "items": items}))
    }
}

fn build_filter(kind: &str, status: Option<&str>) -> FilterNode {
    let kind_node = FilterNode::Leaf(
        [("kind".to_string(), json!({"eq": kind}))]
            .into_iter()
            .collect(),
    );
    match status {
        Some(s) => {
            let status_node = FilterNode::Leaf(
                [("status".to_string(), json!({"eq": s}))]
                    .into_iter()
                    .collect(),
            );
            FilterNode::And {
                and: vec![kind_node, status_node],
            }
        }
        None => kind_node,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use crate::catalog::Catalog;
    use crate::workspace::WorkspaceConfig;
    use std::sync::Arc;

    fn mk_ctx(cat: Catalog) -> ToolContext {
        ToolContext {
            catalog: Arc::new(parking_lot::Mutex::new(cat)),
            workspace: Arc::new(WorkspaceConfig {
                roots: vec![],
                ignore: vec![],
                rules: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
        }
    }

    fn mk_row(id: &str, kind: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: kind.into(),
            status: "active".into(),
            title: None,
            owners: vec![],
            tags: vec![],
            topic: None,
            time_scope: None,
            source: None,
            created_at: 0,
            updated_at: 1,
            file_mtime: 0,
            file_sha256: "".into(),
            confidence: 1.0,
        }
    }

    #[tokio::test]
    async fn filters_by_kind() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("s1", "spec")).unwrap();
        artifact::upsert(&cat, &mk_row("s2", "spec")).unwrap();
        artifact::upsert(&cat, &mk_row("p1", "plan")).unwrap();

        let ctx = mk_ctx(cat);
        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "spec"}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(2));
    }

    #[tokio::test]
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("s1", "spec")).unwrap();
        let ctx = mk_ctx(cat);
        // Should not error even with ridiculous limit.
        let v = ArtifactListByKind
            .call(&ctx, json!({"kind": "spec", "limit": 10_000_000}))
            .await
            .unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
    }
}
