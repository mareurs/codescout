use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};

use super::{Tool, ToolContext};
use crate::catalog::links;

pub struct ArtifactGraph;

#[derive(Deserialize)]
struct Args {
    id: String,
    depth: usize,
    #[serde(default)]
    rels: Option<Vec<String>>,
}

#[async_trait]
impl Tool for ArtifactGraph {
    fn name(&self) -> &'static str {
        "artifact_graph"
    }

    fn description(&self) -> &'static str {
        "BFS graph expansion from a seed artifact. depth: 1–3. \
         Optionally filter by rel types. Returns {nodes, edges}."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["id", "depth"],
            "properties": {
                "id": {"type": "string"},
                "depth": {"type": "integer", "minimum": 1, "maximum": 3},
                "rels": {
                    "type": "array",
                    "items": {"type": "string"}
                }
            }
        })
    }

    async fn call(&self, ctx: &ToolContext, args: Value) -> Result<Value> {
        let a: Args = serde_json::from_value(args)?;
        if a.depth < 1 || a.depth > 3 {
            bail!("depth must be between 1 and 3");
        }

        let cat = ctx.catalog.lock();
        let rels_filter = a.rels.as_deref();

        let mut visited: HashSet<String> = HashSet::new();
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut edges: Vec<Value> = Vec::new();

        visited.insert(a.id.clone());
        queue.push_back((a.id.clone(), 0));

        while let Some((current_id, current_depth)) = queue.pop_front() {
            if current_depth >= a.depth {
                continue;
            }
            let outgoing = links::outgoing(&cat, &current_id)?;
            for link in outgoing {
                // apply rels filter
                if let Some(rels) = rels_filter {
                    if !rels.contains(&link.rel) {
                        continue;
                    }
                }
                edges.push(json!({
                    "src": link.src_id,
                    "dst": link.dst_id,
                    "rel": link.rel,
                }));
                if !visited.contains(&link.dst_id) {
                    visited.insert(link.dst_id.clone());
                    queue.push_back((link.dst_id, current_depth + 1));
                }
            }
        }

        let nodes: Vec<Value> = visited.into_iter().map(|id| json!({"id": id})).collect();
        Ok(json!({
            "nodes": nodes,
            "edges": edges,
        }))
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
                umbrellas: vec![],
            }),
            rules: Arc::new(vec![]),
            embedding: None,
            current_project: None,
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

    fn mk_link(src: &str, dst: &str, rel: &str) -> LinkRow {
        LinkRow {
            src_id: src.into(),
            dst_id: dst.into(),
            rel: rel.into(),
            created_at: 0,
        }
    }

    #[tokio::test]
    async fn linear_chain_depth_2() {
        // A→B→C→D via implements; seed=A depth=2 → nodes {A,B,C}, edges {A→B, B→C}
        let cat = Catalog::open_in_memory().unwrap();
        for id in ["a", "b", "c", "d"] {
            artifact::upsert(&cat, &mk_row(id)).unwrap();
        }
        links::insert(&cat, &mk_link("a", "b", "implements")).unwrap();
        links::insert(&cat, &mk_link("b", "c", "implements")).unwrap();
        links::insert(&cat, &mk_link("c", "d", "implements")).unwrap();

        let ctx = mk_ctx(cat);
        let v = ArtifactGraph
            .call(&ctx, json!({"id": "a", "depth": 2}))
            .await
            .unwrap();

        let nodes = v["nodes"].as_array().unwrap();
        let edges = v["edges"].as_array().unwrap();
        assert_eq!(nodes.len(), 3, "expected 3 nodes (A, B, C)");
        assert_eq!(edges.len(), 2, "expected 2 edges (A→B, B→C)");
    }

    #[tokio::test]
    async fn rejects_invalid_depth() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        let err = ArtifactGraph
            .call(&ctx, json!({"id": "a", "depth": 5}))
            .await;
        assert!(err.is_err());
    }
}
