//! Qdrant operations for the `artifacts` collection.
//!
//! The librarian artifact vector index, used when `vector_backend = "qdrant"`
//! (the default). Artifacts are **project-scoped**: each point carries a
//! `project_id` (the workspace root / repo the artifact's file lives under),
//! and KNN filters by it when a single project is in scope. The point id is
//! keyed on the globally-unique artifact id (`sha256(abs_path)`), so
//! re-indexing the same artifact under a changed root just updates its
//! `project_id` payload in place — no orphaned point.
//!
//! KNN returns ranked `artifact_id`s only; the catalog then hydrates
//! (`WHERE id IN …`) and applies the caller's scoped filter AST — that filter
//! is the authoritative scope backstop, so an unfiltered or foreign candidate
//! id is simply dropped, never mis-returned. The vectors are produced by the
//! librarian's own `EmbeddingService`; this module only stores them. Mirrors
//! [`crate::retrieval::memory`]; the sqlite-vec backend (the daemon-free escape
//! hatch) lives in `crate::librarian::artifact_store`.

use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, PointId, PointStruct, PointsIdsList, Query, QueryPointsBuilder,
    UpsertPointsBuilder, Value, VectorInput, VectorParamsBuilder, VectorsConfigBuilder,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::retrieval::qdrant::QdrantWrap;

/// Namespace for UUIDv5 derivation of artifact point IDs. Stable across
/// versions — never change. ("cs-artifact-v5" + 0x00 0x01.)
const ARTIFACT_NS: Uuid = Uuid::from_bytes([
    0x63, 0x73, 0x2d, 0x61, 0x72, 0x74, 0x69, 0x66, 0x61, 0x63, 0x74, 0x2d, 0x76, 0x35, 0x00, 0x01,
]);

/// Deterministic Qdrant point ID for an artifact — UUIDv5 over the artifact id
/// (`sha256(abs_path)`, globally unique). Keyed on the id alone (not the
/// project) so re-indexing under a different root updates the same point's
/// `project_id` payload rather than orphaning the old point.
pub fn artifact_point_id(artifact_id: &str) -> PointId {
    PointId::from(Uuid::new_v5(&ARTIFACT_NS, artifact_id.as_bytes()).to_string())
}

impl QdrantWrap {
    /// Ensure the artifacts collection exists with a single dense vector and a
    /// keyword index on `project_id` (the KNN scope filter). Idempotent — safe
    /// on every startup.
    pub async fn ensure_artifacts_collection(&self, name: &str, dim: u64) -> Result<()> {
        if self.collection_exists(name).await? {
            return Ok(());
        }

        let mut vectors = VectorsConfigBuilder::default();
        vectors.add_named_vector_params("dense", VectorParamsBuilder::new(dim, Distance::Cosine));

        self.client
            .create_collection(CreateCollectionBuilder::new(name).vectors_config(vectors))
            .await
            .context("create_collection(artifacts)")?;

        self.client
            .create_field_index(CreateFieldIndexCollectionBuilder::new(
                name,
                "project_id",
                FieldType::Keyword,
            ))
            .await
            .context("create_field_index(project_id)")?;

        Ok(())
    }

    /// Insert or update a single artifact's embedding. The point ID is derived
    /// from the artifact id, so a second call overwrites in place (and updates
    /// `project_id` if the artifact moved roots). Payload carries `project_id`
    /// (KNN scope) and `artifact_id` (the catalog key the KNN returns).
    pub async fn artifact_upsert(
        &self,
        collection: &str,
        project_id: &str,
        id: &str,
        dense: Vec<f32>,
    ) -> Result<()> {
        let mut payload: HashMap<String, Value> = HashMap::new();
        payload.insert("project_id".into(), Value::from(project_id.to_string()));
        payload.insert("artifact_id".into(), Value::from(id.to_string()));

        let mut named: HashMap<String, qdrant_client::qdrant::Vector> = HashMap::new();
        named.insert("dense".to_string(), dense.into());

        let point = PointStruct::new(artifact_point_id(id), named, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(collection, vec![point]).wait(true))
            .await
            .context("upsert_points(artifact)")?;
        Ok(())
    }

    /// Delete a single artifact's embedding by id. No-op if it doesn't exist.
    pub async fn artifact_delete(&self, collection: &str, id: &str) -> Result<()> {
        self.client
            .delete_points(
                DeletePointsBuilder::new(collection)
                    .points(PointsIdsList {
                        ids: vec![artifact_point_id(id)],
                    })
                    .wait(true),
            )
            .await
            .context("delete_points(artifact)")?;
        Ok(())
    }

    /// Dense KNN → ranked `artifact_id`s (closest first). `project_id` filters
    /// to a single project when the query is project-scoped; `None` searches
    /// all projects (the catalog's scoped filter still narrows after).
    pub async fn artifact_knn_ids(
        &self,
        collection: &str,
        project_id: Option<&str>,
        dense: Vec<f32>,
        top_n: usize,
    ) -> Result<Vec<String>> {
        let mut req = QueryPointsBuilder::new(collection)
            .query(Query::new_nearest(VectorInput::new_dense(dense)))
            .using("dense")
            .limit(top_n as u64)
            .with_payload(true);
        if let Some(pid) = project_id {
            req = req.filter(Filter::must(vec![Condition::matches(
                "project_id",
                pid.to_string(),
            )]));
        }

        let resp = self
            .client
            .query(req.build())
            .await
            .context("artifact_knn_ids")?;

        Ok(resp
            .result
            .into_iter()
            .filter_map(|pt| {
                pt.payload
                    .get("artifact_id")
                    .and_then(|v| v.as_str().map(|s| s.as_str().to_owned()))
            })
            .collect())
    }
}
