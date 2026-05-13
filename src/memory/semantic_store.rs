//! `SemanticMemoryStore` trait + Qdrant implementation.
//!
//! Storage-only abstraction: callers handle the embedder. This lets the
//! memory tool stay backend-agnostic (the same code path works against the
//! Qdrant impl here and the legacy sqlite-vec impl in step 8's
//! `legacy-vec` feature). Embedding happens in the tool layer where the
//! active project's configured `Embedder` is already wired.
//!
//! Naming note: there is a separate `src/memory/MemoryStore` for
//! topic-based **file** memories (markdown in `.codescout/memories/`).
//! That subsystem is unrelated to semantic memory; `SemanticMemoryStore`
//! is the distinct name to avoid collision.

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::retrieval::memory::MemoryHit;
use crate::retrieval::memory_payload::SemanticMemory;
use crate::retrieval::qdrant::QdrantWrap;

/// Backend-agnostic semantic memory storage. Implementations:
/// - [`QdrantSemanticMemoryStore`] — production, this file.
/// - sqlite-vec impl behind `legacy-vec` feature (step 8 mechanical work).
#[async_trait]
pub trait SemanticMemoryStore: Send + Sync {
    /// Upsert by deterministic point id (UUIDv5 over
    /// `(project_id, bucket, title)`). Same title in same bucket overwrites.
    async fn upsert(&self, m: &SemanticMemory, dense: &[f32]) -> Result<()>;

    /// Dense KNN over a project's memories, optionally narrowed to a bucket.
    /// `query` is the already-embedded query vector — caller handles embedding.
    async fn search(
        &self,
        project_id: &str,
        query: &[f32],
        top_n: usize,
        bucket: Option<&str>,
    ) -> Result<Vec<MemoryHit>>;

    /// Delete by point id. Idempotent — missing ids are no-ops.
    async fn delete(&self, project_id: &str, id: Uuid) -> Result<()>;

    /// All memories for a project, in Qdrant's internal scroll order.
    async fn list(&self, project_id: &str) -> Result<Vec<MemoryHit>>;

    /// Memories anchored to a given file path.
    async fn by_anchor(&self, project_id: &str, path: &str) -> Result<Vec<MemoryHit>>;
}

/// Qdrant-backed implementation. Owns a [`QdrantWrap`] and the collection
/// name (configurable so a deployment can isolate test/prod or run multiple
/// memory namespaces side by side).
pub struct QdrantSemanticMemoryStore {
    qdrant: QdrantWrap,
    collection: String,
}

impl QdrantSemanticMemoryStore {
    /// Build the store and bootstrap the collection if missing. `dim` must
    /// match the active embedder's output dimension.
    pub async fn new(qdrant: QdrantWrap, collection: impl Into<String>, dim: u64) -> Result<Self> {
        let collection = collection.into();
        qdrant.ensure_memories_collection(&collection, dim).await?;
        Ok(Self { qdrant, collection })
    }

    /// Construct without bootstrapping the collection — useful when the
    /// caller has already ensured it (e.g. shared startup path that
    /// bootstraps code_chunks + memories together).
    pub fn from_parts(qdrant: QdrantWrap, collection: impl Into<String>) -> Self {
        Self {
            qdrant,
            collection: collection.into(),
        }
    }
}

#[async_trait]
impl SemanticMemoryStore for QdrantSemanticMemoryStore {
    async fn upsert(&self, m: &SemanticMemory, dense: &[f32]) -> Result<()> {
        // memory_upsert takes ownership of the vector to avoid an extra
        // clone inside qdrant_client's typed Vector conversion.
        self.qdrant
            .memory_upsert(&self.collection, m, dense.to_vec())
            .await
    }

    async fn search(
        &self,
        project_id: &str,
        query: &[f32],
        top_n: usize,
        bucket: Option<&str>,
    ) -> Result<Vec<MemoryHit>> {
        self.qdrant
            .memory_search_dense(&self.collection, project_id, query.to_vec(), top_n, bucket)
            .await
    }

    async fn delete(&self, _project_id: &str, id: Uuid) -> Result<()> {
        // project_id isn't required by Qdrant delete-by-id (the UUIDv5 is
        // already project-scoped), but the trait keeps it for the legacy
        // sqlite impl which scopes by project at the SQL level.
        self.qdrant.memory_delete(&self.collection, id).await
    }

    async fn list(&self, project_id: &str) -> Result<Vec<MemoryHit>> {
        self.qdrant.memory_list(&self.collection, project_id).await
    }

    async fn by_anchor(&self, project_id: &str, path: &str) -> Result<Vec<MemoryHit>> {
        self.qdrant
            .memory_by_anchor(&self.collection, project_id, path)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::memory_payload::{MemoryAnchor, SemanticMemory};

    fn sample(title: &str, anchor_path: &str) -> SemanticMemory {
        SemanticMemory {
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
                path: anchor_path.into(),
                hash: "h".into(),
            }],
            created_at: "2026-05-13T00:00:00Z".into(),
            updated_at: "2026-05-13T00:00:00Z".into(),
        }
    }

    /// E2E test exercising the full trait surface against a running Qdrant.
    /// Run with: cargo test -- --ignored semantic_memory_store_trait_roundtrip
    #[tokio::test]
    #[ignore]
    async fn semantic_memory_store_trait_roundtrip() {
        let qdrant = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("connect");
        let coll = "test_semantic_memory_store";
        let _ = qdrant.client.delete_collection(coll).await;

        let store = QdrantSemanticMemoryStore::new(qdrant, coll, 8)
            .await
            .expect("new");

        // Upsert two
        let alpha = sample("alpha-system", "src/a.rs");
        let beta = sample("beta-pref", "src/b.rs");
        let v_alpha = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v_beta = vec![0.7, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        store.upsert(&alpha, &v_alpha).await.unwrap();
        store.upsert(&beta, &v_beta).await.unwrap();

        // Search — alpha must win
        let q = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let hits = store.search("test-proj", &q, 2, None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory.title, "alpha-system");

        // Bucket filter
        let hits = store
            .search("test-proj", &q, 5, Some("preferences"))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.title, "beta-pref");

        // List
        assert_eq!(store.list("test-proj").await.unwrap().len(), 2);

        // by_anchor
        let by = store.by_anchor("test-proj", "src/a.rs").await.unwrap();
        assert_eq!(by.len(), 1);
        assert_eq!(by[0].memory.title, "alpha-system");

        // Delete alpha
        store.delete("test-proj", alpha.point_id()).await.unwrap();
        let remaining = store.list("test-proj").await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].memory.title, "beta-pref");

        // Idempotent delete
        store.delete("test-proj", alpha.point_id()).await.unwrap();

        // Cleanup
        let cleanup = QdrantWrap::connect("http://localhost:6334")
            .await
            .expect("reconnect");
        cleanup.client.delete_collection(coll).await.unwrap();
    }

    /// Trait object compiles — proves the trait is dyn-compatible and that
    /// callers can store `Box<dyn SemanticMemoryStore>`.
    #[test]
    fn trait_is_dyn_compatible() {
        fn _accepts_dyn(_: &dyn SemanticMemoryStore) {}
        // Compiles iff the trait is dyn-compatible.
    }
}
