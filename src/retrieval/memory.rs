//! Qdrant operations for the `memories` collection.
//!
//! Memories live in a single shared collection (filter by `project_id`) with
//! a dense-only vector config — no sparse, no rerank by default. The schema
//! is defined in [`crate::retrieval::memory_payload`]; this module supplies
//! the collection bootstrap and CRUD operations against Qdrant.

use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, PointId, PointStruct, PointsIdsList, Query, QueryPointsBuilder,
    ScrollPointsBuilder, UpsertPointsBuilder, VectorInput, VectorParamsBuilder,
    VectorsConfigBuilder,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::retrieval::memory_payload::{memory_to_payload, payload_to_memory, SemanticMemory};
use crate::retrieval::qdrant::QdrantWrap;

/// One memory result from a search or scroll — payload decoded, point id
/// extracted, and (for search) the dense similarity score.
#[derive(Debug, Clone)]
pub struct MemoryHit {
    pub id: Uuid,
    pub memory: SemanticMemory,
    /// Cosine similarity for KNN results; `None` for plain scrolls.
    pub score: Option<f32>,
}

/// Qdrant point ID for a memory — UUIDv5 over (project_id, bucket, title).
fn memory_point_id(uuid: Uuid) -> PointId {
    PointId::from(uuid.to_string())
}

fn parse_uuid(id: &PointId) -> Option<Uuid> {
    let kind = id.point_id_options.as_ref()?;
    use qdrant_client::qdrant::point_id::PointIdOptions;
    match kind {
        PointIdOptions::Uuid(s) => Uuid::parse_str(s).ok(),
        PointIdOptions::Num(_) => None,
    }
}

impl QdrantWrap {
    /// Ensure the memories collection exists with a single dense vector and
    /// payload indexes on the filter fields we use most often. Idempotent —
    /// safe to call on every startup.
    ///
    /// Schema:
    /// - dense vector "dense", `dim` dimensions, Cosine distance
    /// - no sparse vector (memories are short prose; reranker is sufficient)
    /// - keyword payload indexes on `project_id`, `bucket`, `anchors[].path`
    pub async fn ensure_memories_collection(&self, name: &str, dim: u64) -> Result<()> {
        if self.collection_exists(name).await? {
            return Ok(());
        }

        let mut vectors = VectorsConfigBuilder::default();
        vectors.add_named_vector_params("dense", VectorParamsBuilder::new(dim, Distance::Cosine));

        self.client
            .create_collection(CreateCollectionBuilder::new(name).vectors_config(vectors))
            .await
            .context("create_collection(memories)")?;

        for (field, kind) in [
            ("project_id", FieldType::Keyword),
            ("bucket", FieldType::Keyword),
            ("anchors[].path", FieldType::Keyword),
        ] {
            self.client
                .create_field_index(CreateFieldIndexCollectionBuilder::new(name, field, kind))
                .await
                .with_context(|| format!("create_field_index({field})"))?;
        }

        Ok(())
    }

    /// Insert or update a single memory. The point ID is derived from the
    /// memory's (project_id, bucket, title), so a second call with the same
    /// title overwrites content, anchors, and timestamps.
    pub async fn memory_upsert(
        &self,
        collection: &str,
        m: &SemanticMemory,
        dense: Vec<f32>,
    ) -> Result<()> {
        let payload = memory_to_payload(m);
        let mut named: HashMap<String, qdrant_client::qdrant::Vector> = HashMap::new();
        named.insert("dense".to_string(), dense.into());

        let point = PointStruct::new(memory_point_id(m.point_id()), named, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(collection, vec![point]).wait(true))
            .await
            .context("upsert_points(memory)")?;
        Ok(())
    }

    /// Delete a single memory by its UUIDv5 point id. No-op if it doesn't
    /// exist.
    pub async fn memory_delete(&self, collection: &str, id: Uuid) -> Result<()> {
        self.client
            .delete_points(
                DeletePointsBuilder::new(collection)
                    .points(PointsIdsList {
                        ids: vec![memory_point_id(id)],
                    })
                    .wait(true),
            )
            .await
            .context("delete_points(memory)")?;
        Ok(())
    }

    /// Dense KNN search over memories for a project. Optional `bucket`
    /// narrows to a single bucket; `None` searches across all buckets.
    /// No reranker — memory results are short and the dense leg is enough.
    pub async fn memory_search_dense(
        &self,
        collection: &str,
        project_id: &str,
        dense: Vec<f32>,
        top_n: usize,
        bucket: Option<&str>,
    ) -> Result<Vec<MemoryHit>> {
        let mut conds = vec![Condition::matches("project_id", project_id.to_string())];
        if let Some(b) = bucket {
            conds.push(Condition::matches("bucket", b.to_string()));
        }
        let filter = Filter::must(conds);

        let req = QueryPointsBuilder::new(collection)
            .query(Query::new_nearest(VectorInput::new_dense(dense)))
            .using("dense")
            .filter(filter)
            .limit(top_n as u64)
            .with_payload(true)
            .build();

        let resp = self
            .client
            .query(req)
            .await
            .context("memory_search_dense")?;

        Ok(resp
            .result
            .into_iter()
            .filter_map(|pt| {
                let id = pt.id.as_ref().and_then(parse_uuid)?;
                let memory = payload_to_memory(&pt.payload).ok()?;
                Some(MemoryHit {
                    id,
                    memory,
                    score: Some(pt.score),
                })
            })
            .collect())
    }

    /// Scroll memories for a project with optional bucket / anchor-path
    /// filters. Results are returned in Qdrant's internal order — caller
    /// applies ordering and limit (kept out of this layer to avoid pulling
    /// `MemoryFilter` into the retrieval module and creating a dep cycle
    /// with `crate::memory::semantic_store`).
    pub async fn memory_list_filtered(
        &self,
        collection: &str,
        project_id: &str,
        bucket: Option<&str>,
        anchor_path: Option<&str>,
    ) -> Result<Vec<MemoryHit>> {
        let mut conds = vec![Condition::matches("project_id", project_id.to_string())];
        if let Some(b) = bucket {
            conds.push(Condition::matches("bucket", b.to_string()));
        }
        if let Some(p) = anchor_path {
            conds.push(Condition::matches("anchors[].path", p.to_string()));
        }
        self.scroll_memories(collection, Filter::must(conds)).await
    }

    /// Shared scroll body — paginates until exhausted, decodes payload.
    /// Skips points whose payload doesn't parse (defensive, shouldn't happen).
    async fn scroll_memories(&self, collection: &str, filter: Filter) -> Result<Vec<MemoryHit>> {
        let mut out = Vec::new();
        let mut offset: Option<PointId> = None;
        loop {
            let mut builder = ScrollPointsBuilder::new(collection)
                .filter(filter.clone())
                .with_payload(true)
                .with_vectors(false)
                .limit(1000u32);
            if let Some(off) = offset.take() {
                builder = builder.offset(off);
            }
            let resp = self
                .client
                .scroll(builder)
                .await
                .context("scroll_memories")?;
            for pt in resp.result {
                let Some(id) = pt.id.as_ref().and_then(parse_uuid) else {
                    continue;
                };
                let Ok(memory) = payload_to_memory(&pt.payload) else {
                    continue;
                };
                out.push(MemoryHit {
                    id,
                    memory,
                    score: None,
                });
            }
            match resp.next_page_offset {
                None => break,
                Some(next) => offset = Some(next),
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::memory_payload::{MemoryAnchor, SemanticMemory};

    fn sample_memory(title: &str) -> SemanticMemory {
        SemanticMemory {
            project_id: "test-proj".into(),
            bucket: "system".into(),
            title: title.into(),
            content: "test content".into(),
            anchors: vec![MemoryAnchor {
                path: "src/foo.rs".into(),
                hash: "deadbeef".into(),
            }],
            created_at: "2026-05-13T00:00:00Z".into(),
            updated_at: "2026-05-13T00:00:00Z".into(),
        }
    }

    /// E2E test — requires a running Qdrant at localhost:6334.
    /// Run with: cargo test -- --ignored memory_collection_bootstrap_and_upsert
    #[tokio::test]
    #[ignore]
    async fn memory_collection_bootstrap_and_upsert() {
        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_memories_bootstrap";
        let _ = wrap.client.delete_collection(coll).await;

        wrap.ensure_memories_collection(coll, 384)
            .await
            .expect("ensure");

        assert!(wrap.collection_exists(coll).await.unwrap());

        // Idempotent
        wrap.ensure_memories_collection(coll, 384)
            .await
            .expect("idempotent");

        // Upsert one memory with a dummy 384-dim vector
        let m = sample_memory("first memory");
        let dummy: Vec<f32> = (0..384).map(|i| (i as f32) * 0.001).collect();
        wrap.memory_upsert(coll, &m, dummy.clone())
            .await
            .expect("upsert");

        // Re-upsert (same title => same point id => overwrite)
        let mut m2 = m.clone();
        m2.content = "updated content".into();
        wrap.memory_upsert(coll, &m2, dummy.clone())
            .await
            .expect("re-upsert");

        // Delete
        wrap.memory_delete(coll, m.point_id())
            .await
            .expect("delete");

        wrap.client.delete_collection(coll).await.unwrap();
    }

    /// E2E search/list/by_anchor — requires a running Qdrant at localhost:6334.
    /// Run with: cargo test -- --ignored memory_search_list_by_anchor
    #[tokio::test]
    #[ignore]
    async fn memory_search_list_by_anchor() {
        let wrap = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");

        let coll = "test_memories_search";
        let _ = wrap.client.delete_collection(coll).await;
        wrap.ensure_memories_collection(coll, 8)
            .await
            .expect("ensure");

        // Three memories with hand-crafted vectors so KNN is predictable.
        // alpha closest to query [1,0,...]; beta mid; gamma far.
        let mk = |title: &str, vec: Vec<f32>, anchor: &str| {
            let m = SemanticMemory {
                project_id: "test-proj".into(),
                bucket: if title.contains("pref") {
                    "preferences"
                } else {
                    "system"
                }
                .into(),
                title: title.into(),
                content: format!("content for {title}"),
                anchors: vec![MemoryAnchor {
                    path: anchor.into(),
                    hash: "h".into(),
                }],
                created_at: "2026-05-13T00:00:00Z".into(),
                updated_at: "2026-05-13T00:00:00Z".into(),
            };
            (m, vec)
        };
        let (m_a, v_a) = mk(
            "alpha-system",
            vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "src/a.rs",
        );
        let (m_b, v_b) = mk(
            "beta-pref",
            vec![0.7, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "src/b.rs",
        );
        let (m_c, v_c) = mk(
            "gamma-system",
            vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "src/a.rs",
        );
        wrap.memory_upsert(coll, &m_a, v_a).await.unwrap();
        wrap.memory_upsert(coll, &m_b, v_b).await.unwrap();
        wrap.memory_upsert(coll, &m_c, v_c).await.unwrap();

        // KNN: query close to alpha
        let q = vec![1.0_f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let hits = wrap
            .memory_search_dense(coll, "test-proj", q.clone(), 3, None)
            .await
            .expect("search");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].memory.title, "alpha-system");
        assert!(hits[0].score.unwrap() > hits[2].score.unwrap());

        // Bucket filter
        let hits = wrap
            .memory_search_dense(coll, "test-proj", q, 5, Some("preferences"))
            .await
            .expect("search bucket");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.title, "beta-pref");

        // List (no filter)
        let all = wrap
            .memory_list_filtered(coll, "test-proj", None, None)
            .await
            .expect("list");
        assert_eq!(all.len(), 3);

        // By anchor (anchor_path filter)
        let by_a = wrap
            .memory_list_filtered(coll, "test-proj", None, Some("src/a.rs"))
            .await
            .expect("by_anchor");
        assert_eq!(by_a.len(), 2);
        let titles: Vec<_> = by_a.iter().map(|h| h.memory.title.as_str()).collect();
        assert!(titles.contains(&"alpha-system"));
        assert!(titles.contains(&"gamma-system"));

        // Bucket filter via list
        let prefs = wrap
            .memory_list_filtered(coll, "test-proj", Some("preferences"), None)
            .await
            .expect("list bucket");
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].memory.title, "beta-pref");

        wrap.client.delete_collection(coll).await.unwrap();
    }
}
