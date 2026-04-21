use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::links;

pub struct ArtifactLinks;

#[derive(Deserialize)]
struct Args {
    id: String,
    #[serde(default)]
    rel: Option<String>,
    #[serde(default = "default_direction")]
    direction: String,
}

fn default_direction() -> String {
    "both".to_string()
}

#[async_trait]
impl Tool for ArtifactLinks {
    fn name(&self) -> &'static str {
        "artifact_links"
    }

    fn description(&self) -> &'static str {
        "Fetch links for an artifact. direction: out|in|both (default both). \
         Optionally filter by rel type."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id"],
            "properties": {
                "id": {"type": "string"},
                "rel": {"type": "string"},
                "direction": {
                    "type": "string",
                    "enum": ["out", "in", "both"],
                    "default": "both"
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        let cat = ctx.catalog.lock();

        let mut items: Vec<Value> = Vec::new();

        if a.direction == "out" || a.direction == "both" {
            let rows = links::outgoing(&cat, &a.id)?;
            for l in rows {
                if let Some(rel) = &a.rel {
                    if &l.rel != rel {
                        continue;
                    }
                }
                items.push(json!({
                    "direction": "out",
                    "src_id": l.src_id,
                    "dst_id": l.dst_id,
                    "rel": l.rel,
                }));
            }
        }

        if a.direction == "in" || a.direction == "both" {
            let rows = links::incoming(&cat, &a.id)?;
            for l in rows {
                if let Some(rel) = &a.rel {
                    if &l.rel != rel {
                        continue;
                    }
                }
                items.push(json!({
                    "direction": "in",
                    "src_id": l.src_id,
                    "dst_id": l.dst_id,
                    "rel": l.rel,
                }));
            }
        }

        Ok(json!({"count": items.len(), "items": items}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use crate::catalog::links::{self, LinkRow};
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

    fn mk_row(id: &str) -> ArtifactRow {
        ArtifactRow {
            id: id.into(),
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
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

    fn mk_link(src: &str, dst: &str) -> LinkRow {
        LinkRow {
            src_id: src.into(),
            dst_id: dst.into(),
            rel: "implements".into(),
            created_at: 0,
        }
    }

    #[tokio::test]
    async fn both_directions() {
        // A→B and B→C; query B direction=both → 2 items
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &mk_row("a")).unwrap();
        artifact::upsert(&cat, &mk_row("b")).unwrap();
        artifact::upsert(&cat, &mk_row("c")).unwrap();
        links::insert(&cat, &mk_link("a", "b")).unwrap();
        links::insert(&cat, &mk_link("b", "c")).unwrap();

        let ctx = mk_ctx(cat);
        let v = ArtifactLinks
            .call(&ctx, json!({"id": "b", "direction": "both"}))
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(2));
    }
}
