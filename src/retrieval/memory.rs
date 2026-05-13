//! Qdrant operations for the `memories` collection.
//!
//! Memories live in a single shared collection (filter by `project_id`) with
//! a dense-only vector config — no sparse, no rerank by default. The schema
//! is defined in [`crate::retrieval::memory_payload`]; this module supplies
//! the collection bootstrap and CRUD operations against Qdrant.

use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, PointId, PointStruct, PointsIdsList, UpsertPointsBuilder,
    VectorParamsBuilder, VectorsConfigBuilder,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::retrieval::memory_payload::{memory_to_payload, SemanticMemory};
use crate::retrieval::qdrant::QdrantWrap;

/// Qdrant point ID for a memory — UUIDv5 over (project_id, bucket, title).
fn memory_point_id(uuid: Uuid) -> PointId {
    PointId::from(uuid.to_string())
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
            .upsert_points(
                UpsertPointsBuilder::new(collection, vec![point]).wait(true),
            )
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

    /// Convenience: a project-id filter used by every memory read.
    /// Used in step 2c by search/list/by_anchor; kept here so callers can
    /// reuse the exact filter when iterating themselves.
    #[allow(dead_code)]
    pub(crate) fn memory_project_filter(project_id: &str) -> Filter {
        Filter::must([Condition::matches(
            "project_id",
            project_id.to_string(),
        )])
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
        wrap.memory_delete(coll, m.point_id()).await.expect("delete");

        wrap.client.delete_collection(coll).await.unwrap();
    }
}
