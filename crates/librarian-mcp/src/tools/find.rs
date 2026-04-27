use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};

use super::{Tool, ToolContext};
use crate::catalog::find::{find, FindOpts};
use crate::filter::FilterNode;

pub struct ArtifactFind;

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
}

const MAX_LIMIT: usize = 500;
const MAX_OFFSET: usize = 100_000;

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
         Composition: and/or/not. Leaf ops: eq/ne/in/nin/gt/lt/gte/lte/contains."
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filter": {"type": "object"},
                "limit": {"type": "integer", "default": 50, "maximum": 500},
                "offset": {"type": "integer", "default": 0, "maximum": 100000},
                "semantic": {"type": "string", "description": "Natural-language query for semantic search (requires embedder)"}
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

        let cat = ctx.catalog.lock();
        let rows = find(
            &cat,
            &FindOpts {
                filter: a.filter,
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
        Ok(json!({"count": items.len(), "items": items}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::artifact::{self, ArtifactRow};
    use crate::catalog::Catalog;
    use crate::embedding::EmbeddingService;
    use crate::workspace::WorkspaceConfig;
    use async_trait::async_trait;
    use codescout_embed::{Embedder, Embedding};
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
            repo: "r".into(),
            rel_path: format!("{id}.md"),
            kind: "spec".into(),
            status: "active".into(),
            title: Some(title.into()),
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

    #[tokio::test]
    async fn returns_rows_matching_filter() {
        let cat = Catalog::open_in_memory().unwrap();
        artifact::upsert(&cat, &sample_row("a", "A")).unwrap();
        let ctx = mk_ctx(cat);
        let v = ArtifactFind
            .call(
                &ctx,
                json!({
                    "filter": {"kind": {"eq": "spec"}},
                    "limit": 10
                }),
            )
            .await
            .unwrap();
        assert_eq!(v["count"].as_u64(), Some(1));
    }

    #[tokio::test]
    async fn clamps_oversized_limit() {
        let cat = Catalog::open_in_memory().unwrap();
        let ctx = mk_ctx(cat);
        // Should not error even with ridiculous limit.
        let v = ArtifactFind
            .call(&ctx, json!({"limit": 10_000_000}))
            .await
            .unwrap();
        assert!(v["count"].as_u64().unwrap() <= 500);
    }

    /// MockEmbedder: text containing "auth" → [1.0, 0.0, 0.0, ...] (768 dims)
    ///               otherwise         → [0.0, 1.0, 0.0, ...] (768 dims)
    /// This makes "auth" artifacts closest to an "auth" query by cosine distance.
    struct MockEmbedder;

    #[async_trait]
    impl Embedder for MockEmbedder {
        fn dimensions(&self) -> usize {
            768
        }
        async fn embed(&self, texts: &[&str]) -> anyhow::Result<Vec<Embedding>> {
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

        // Insert two artifacts
        artifact::upsert(&cat, &sample_row("auth-doc", "Authentication Guide")).unwrap();
        artifact::upsert(&cat, &sample_row("deploy-doc", "Deployment Runbook")).unwrap();

        // Write embeddings directly to artifact_vec — mirroring what index_repo_sync produces.
        // auth-doc → [1, 0, 0, ...] (auth vector)
        // deploy-doc → [0, 1, 0, ...] (non-auth vector)
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
                json!({
                    "semantic": "auth login flow",
                    "limit": 10
                }),
            )
            .await
            .unwrap();

        let items = v["items"].as_array().unwrap();
        assert_eq!(items.len(), 2, "both artifacts should be returned");
        // auth-doc must be first (closest to auth query vector)
        assert_eq!(items[0]["id"].as_str(), Some("auth-doc"));
    }
}
