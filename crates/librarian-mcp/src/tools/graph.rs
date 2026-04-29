use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};

use super::{Tool, ToolContext};
use crate::catalog::links;

use rusqlite::OptionalExtension;

pub struct ArtifactGraph;

#[derive(Deserialize)]
struct Args {
    id: String,
    depth: usize,
    #[serde(default)]
    rels: Option<Vec<String>>,
    #[serde(default)]
    include_events: bool,
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
                },
                "include_events": {
                    "type": "boolean",
                    "default": false,
                    "description": "When true, BFS also walks event and source nodes via event_edges"
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

        // Collect artifact ids in BFS order for event pass.
        let mut artifact_ids_ordered: Vec<String> = vec![a.id.clone()];

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
                    artifact_ids_ordered.push(link.dst_id.clone());
                    queue.push_back((link.dst_id, current_depth + 1));
                }
            }
        }

        // Build artifact nodes with discriminator.
        let mut nodes: Vec<Value> = visited
            .iter()
            .map(|id| json!({"node_type": "artifact", "id": id}))
            .collect();

        // Parallel event pass: only when requested.
        if a.include_events {
            let mut visited_events: HashSet<String> = HashSet::new();
            let mut visited_sources: HashSet<String> = HashSet::new();

            for artifact_id in &artifact_ids_ordered {
                let events = crate::catalog::events::timeline_for_artifact(
                    &cat,
                    artifact_id,
                    None,
                    usize::MAX,
                )?;
                for ev in &events {
                    if visited_events.contains(&ev.id) {
                        continue;
                    }
                    visited_events.insert(ev.id.clone());
                    nodes.push(json!({
                        "node_type": "event",
                        "id": ev.id,
                        "artifact_id": ev.artifact_id,
                        "kind": ev.kind,
                    }));

                    // Walk event_edges outgoing from this event.
                    let event_outgoing = crate::catalog::event_edges::outgoing(&cat, &ev.id)?;
                    for ee in event_outgoing {
                        if let Some(ref dst_event_id) = ee.dst_event_id {
                            edges.push(json!({
                                "src": ee.src_event_id,
                                "dst": dst_event_id,
                                "rel": ee.rel,
                            }));
                            // dst event will be visited when its artifact's timeline is processed,
                            // or we add it here if not yet seen.
                            if !visited_events.contains(dst_event_id) {
                                // Leave for the artifact's own timeline sweep; no extra BFS needed.
                            }
                        } else if let Some(ref dst_artifact_id) = ee.dst_artifact_id {
                            edges.push(json!({
                                "src": ee.src_event_id,
                                "dst": dst_artifact_id,
                                "rel": ee.rel,
                            }));
                        } else if let Some(ref dst_source_id) = ee.dst_source_id {
                            edges.push(json!({
                                "src": ee.src_event_id,
                                "dst": dst_source_id,
                                "rel": ee.rel,
                            }));
                            if !visited_sources.contains(dst_source_id) {
                                visited_sources.insert(dst_source_id.clone());
                                // Fetch source payload from catalog.
                                let src_row: Option<(String, String, Option<String>)> = cat
                                    .conn
                                    .query_row(
                                        "SELECT id, kind, uri FROM sources WHERE id=?1",
                                        rusqlite::params![dst_source_id],
                                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                                    )
                                    .optional()?;
                                if let Some((sid, skind, suri)) = src_row {
                                    nodes.push(json!({
                                        "node_type": "source",
                                        "id": sid,
                                        "kind": skind,
                                        "uri": suri,
                                    }));
                                }
                            }
                        }
                    }
                }
            }
        }

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

    #[tokio::test]
    async fn graph_includes_event_nodes_when_requested() {
        use crate::catalog::artifact::upsert as art_upsert;
        use crate::tools::event_create::ArtifactEventCreate;
        use crate::tools::observe::tests::mk_ctx as mk_ctx_with_root;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx_with_root(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_upsert(&cat, &mk_row("a")).unwrap();
        }

        let intent_res = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a",
                    "kind": "intent",
                    "payload": {"hypothesis": "h"}
                }),
            )
            .await
            .unwrap();
        let intent_id = intent_res["event_id"].as_str().unwrap().to_string();

        let verdict_res = ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a",
                    "kind": "verdict",
                    "payload": {"outcome": "confirmed", "summary": "ok"},
                    "resolves_intent_event_id": intent_id.clone()
                }),
            )
            .await
            .unwrap();
        let verdict_id = verdict_res["event_id"].as_str().unwrap().to_string();

        let res = ArtifactGraph
            .call(&ctx, json!({"id": "a", "depth": 2, "include_events": true}))
            .await
            .unwrap();

        let nodes = res["nodes"].as_array().unwrap();
        let node_ids: Vec<String> = nodes
            .iter()
            .filter_map(|n| n["id"].as_str().map(String::from))
            .collect();
        assert!(
            node_ids.iter().any(|n| n == &intent_id),
            "intent node missing: {:?}",
            node_ids
        );
        assert!(
            node_ids.iter().any(|n| n == &verdict_id),
            "verdict node missing: {:?}",
            node_ids
        );

        let edges = res["edges"].as_array().unwrap();
        assert!(
            edges.iter().any(|e| e["rel"] == "resolves"),
            "resolves edge missing: {:?}",
            edges
        );
    }

    #[tokio::test]
    async fn graph_excludes_events_by_default() {
        use crate::catalog::artifact::upsert as art_upsert;
        use crate::tools::event_create::ArtifactEventCreate;
        use crate::tools::observe::tests::mk_ctx as mk_ctx_with_root;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let ctx = mk_ctx_with_root(tmp.path().to_path_buf());
        {
            let cat = ctx.catalog.lock();
            art_upsert(&cat, &mk_row("a")).unwrap();
        }

        ArtifactEventCreate
            .call(
                &ctx,
                json!({
                    "artifact_id": "a",
                    "kind": "intent",
                    "payload": {"hypothesis": "h"}
                }),
            )
            .await
            .unwrap();

        let res = ArtifactGraph
            .call(&ctx, json!({"id": "a", "depth": 2}))
            .await
            .unwrap();

        let nodes = res["nodes"].as_array().unwrap();
        for n in nodes {
            if let Some(t) = n.get("node_type") {
                assert_ne!(t, "event", "unexpected event node: {:?}", n);
                assert_ne!(t, "source", "unexpected source node: {:?}", n);
            }
        }
    }
}
