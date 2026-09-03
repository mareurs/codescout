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
    Distance, FieldType, Filter, PointId, PointStruct, Query, QueryPointsBuilder,
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

        // Needed by `artifact_delete`, which matches on this key rather than on
        // a derived point id. Without the index Qdrant full-scans the whole
        // collection per delete; `code_chunks` has no payload index and paid
        // 37.29s vs 0.57s for exactly that (see qdrant.rs:46).
        self.client
            .create_field_index(CreateFieldIndexCollectionBuilder::new(
                name,
                "artifact_id",
                FieldType::Keyword,
            ))
            .await
            .context("create_field_index(artifact_id)")?;

        Ok(())
    }

    /// Insert or update a single CHUNK's embedding.
    ///
    /// The point id is derived from `chunk_id`, so a second call with the same
    /// chunk overwrites in place. The payload carries **both** ids:
    /// `chunk_id` (what `artifact_knn_scored` returns, and what the caller
    /// resolves through `artifact_chunk`) and `artifact_id` (what
    /// `artifact_delete` matches on, and the catalog key the hit belongs to).
    ///
    /// Until 2026-09-03 this took ONE id and spent it twice — as the point id
    /// and as `payload["artifact_id"]` — because the trait had only one slot.
    /// That is what made a chunk id indistinguishable from an artifact id
    /// downstream. See
    /// `docs/issues/2026-09-03-editing-an-artifact-removes-it-from-qdrant-backed-semantic-search.md`.
    pub async fn artifact_upsert(
        &self,
        collection: &str,
        project_id: &str,
        chunk_id: &str,
        artifact_id: &str,
        dense: Vec<f32>,
    ) -> Result<()> {
        let mut payload: HashMap<String, Value> = HashMap::new();
        payload.insert("project_id".into(), Value::from(project_id.to_string()));
        payload.insert("artifact_id".into(), Value::from(artifact_id.to_string()));
        payload.insert("chunk_id".into(), Value::from(chunk_id.to_string()));

        let mut named: HashMap<String, qdrant_client::qdrant::Vector> = HashMap::new();
        named.insert("dense".to_string(), dense.into());

        // Keyed on the CHUNK id: one point per chunk, N per artifact.
        let point = PointStruct::new(artifact_point_id(chunk_id), named, payload);

        self.client
            .upsert_points(UpsertPointsBuilder::new(collection, vec![point]).wait(true))
            .await
            .context("upsert_points(artifact)")?;
        Ok(())
    }

    /// Delete EVERY point belonging to `artifact_id`.
    ///
    /// By payload FILTER, not by derived point id. Points are keyed on
    /// `chunk_id` now, so one artifact owns N of them and
    /// `artifact_point_id(artifact_id)` names none of them — the old form would
    /// have been a silent no-op that left the whole artifact's vectors behind.
    /// Requires the `artifact_id` keyword index created in
    /// [`Self::ensure_artifacts_collection`]; without it Qdrant full-scans.
    pub async fn artifact_delete(&self, collection: &str, artifact_id: &str) -> Result<()> {
        self.client
            .delete_points(
                DeletePointsBuilder::new(collection)
                    .points(Filter::must(vec![Condition::matches(
                        "artifact_id",
                        artifact_id.to_string(),
                    )]))
                    .wait(true),
            )
            .await
            .context("delete_points(artifact)")?;
        Ok(())
    }

    /// Every artifact-chunk collection currently in Qdrant, by name prefix.
    ///
    /// Artifact vectors live one collection per project, so the cross-project
    /// scopes (`repo` / `umbrella` / `all`) need the list to fan out over.
    /// Enumerating by PREFIX rather than from the workspace registry is
    /// deliberate: a project indexed earlier and since de-registered still has
    /// vectors, and a store that could not see them would return a short page
    /// with no indication anything was missing.
    ///
    /// Returns them sorted, so a fan-out's collection order is deterministic and
    /// two runs of the same query cannot differ by tie-break.
    pub async fn artifact_collections(&self, prefix: &str) -> Result<Vec<String>> {
        let resp = self
            .client
            .list_collections()
            .await
            .context("list_collections(artifact)")?;
        let mut names: Vec<String> = resp
            .collections
            .into_iter()
            .map(|c| c.name)
            .filter(|n| n.starts_with(prefix))
            .collect();
        names.sort();
        Ok(names)
    }

    /// Dense KNN → ranked `(artifact_id, distance)` pairs, closest first.
    /// `project_id` filters to a single project when the query is project-scoped;
    /// `None` searches all projects (the catalog's scoped filter still narrows
    /// after).
    ///
    /// **Returns a DISTANCE — lower is closer — not Qdrant's raw score.** The
    /// artifacts collection is built with `Distance::Cosine`
    /// ([`Self::ensure_artifacts_collection`]), so `ScoredPoint.score` is cosine
    /// *similarity*: higher is better, the opposite polarity from the sqlite-vec
    /// backend's L2 `distance` column. Two implementations of one trait returning
    /// numbers that disagree about which direction is good is a silent,
    /// backend-dependent wrong answer — and Qdrant being the default would have
    /// kept it hidden until someone used the escape hatch. `1 - similarity` is the
    /// standard cosine-distance identity and puts both backends on one polarity.
    ///
    /// The SCALE is still backend-defined (cosine distance here, L2 there), so
    /// these values are comparable **within** one response and not across
    /// backends. Callers must not threshold on an absolute number.
    pub async fn artifact_knn_scored(
        &self,
        collection: &str,
        project_id: Option<&str>,
        dense: Vec<f32>,
        top_n: usize,
    ) -> Result<Vec<(String, f32)>> {
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
            .context("artifact_knn_scored")?;

        Ok(resp
            .result
            .into_iter()
            .filter_map(|pt| {
                let distance = 1.0 - pt.score;
                // Read the CHUNK id: it is the vector's identity and what
                // `semantic_find` resolves through `artifact_chunk`. Reading
                // `artifact_id` here — which this did until 2026-09-03 — returns
                // a key the caller cannot resolve, and the caller's response to
                // an unresolvable id is to skip it, so the whole page silently
                // shrinks. A point written before that date has no `chunk_id`
                // key and is dropped here; `SemanticPage::unresolved` is where
                // the caller-side equivalent is now counted.
                pt.payload
                    .get("chunk_id")
                    .and_then(|v| v.as_str().map(|s| (s.as_str().to_owned(), distance)))
            })
            .collect())
    }
}
