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

/// Sort order for list results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MemoryOrder {
    /// Qdrant's internal scroll order — no specific guarantee.
    #[default]
    Unordered,
    /// Most-recently-updated first. Requires `updated_at` in payload.
    UpdatedAtDesc,
}

/// Filter + ordering for `SemanticMemoryStore::list`. All fields default to
/// "no filter / no sort / no limit" so callers can supply only what they need.
#[derive(Debug, Clone, Default)]
pub struct MemoryFilter {
    /// Restrict to a single bucket (e.g. `"preferences"`, `"structured"`).
    pub bucket: Option<String>,
    /// Restrict to memories whose `anchors[].path` includes this path.
    pub anchor_path: Option<String>,
    /// Sort order applied client-side after scroll.
    pub order_by: MemoryOrder,
    /// Cap on results returned. `None` = unlimited.
    pub limit: Option<usize>,
}

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

    /// List memories with optional bucket/anchor-path filters and ordering.
    ///
    /// Pass `MemoryFilter::default()` for "all memories in this project,
    /// unordered, unlimited" — equivalent to the old `list(project_id)`.
    /// Set `anchor_path` to filter to memories anchored to a file.
    async fn list(&self, project_id: &str, filter: MemoryFilter) -> Result<Vec<MemoryHit>>;
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

    async fn list(&self, project_id: &str, filter: MemoryFilter) -> Result<Vec<MemoryHit>> {
        let mut hits = self
            .qdrant
            .memory_list_filtered(
                &self.collection,
                project_id,
                filter.bucket.as_deref(),
                filter.anchor_path.as_deref(),
            )
            .await?;
        if filter.order_by == MemoryOrder::UpdatedAtDesc {
            hits.sort_by(|a, b| b.memory.updated_at.cmp(&a.memory.updated_at));
        }
        if let Some(n) = filter.limit {
            hits.truncate(n);
        }
        Ok(hits)
    }
}

/// Test-only in-memory implementation of [`SemanticMemoryStore`].
///
/// Mirrors [`QdrantSemanticMemoryStore`] behaviour without requiring a
/// running Qdrant — useful as a test seam for code that uses
/// [`crate::agent::Agent::set_semantic_memory_store_for_test`].
///
/// Implementation notes:
/// - `search` returns cosine similarity (matches Qdrant's COSINE distance).
/// - `delete` is idempotent (missing ids are no-ops, like Qdrant).
/// - `list` honours `bucket`, `anchor_path`, `order_by`, and `limit` exactly
///   as the Qdrant impl does, so trait-level tests cover both implementations
///   with the same assertions.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    pub struct InMemorySemanticMemoryStore {
        inner: Mutex<HashMap<Uuid, (SemanticMemory, Vec<f32>)>>,
    }

    impl Default for InMemorySemanticMemoryStore {
        fn default() -> Self {
            Self {
                inner: Mutex::new(HashMap::new()),
            }
        }
    }

    impl InMemorySemanticMemoryStore {
        pub fn new() -> Self {
            Self::default()
        }
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }

    #[async_trait]
    impl SemanticMemoryStore for InMemorySemanticMemoryStore {
        async fn upsert(&self, m: &SemanticMemory, dense: &[f32]) -> Result<()> {
            self.inner
                .lock()
                .unwrap()
                .insert(m.point_id(), (m.clone(), dense.to_vec()));
            Ok(())
        }

        async fn search(
            &self,
            project_id: &str,
            query: &[f32],
            top_n: usize,
            bucket: Option<&str>,
        ) -> Result<Vec<MemoryHit>> {
            let guard = self.inner.lock().unwrap();
            let mut scored: Vec<(Uuid, &SemanticMemory, f32)> = guard
                .iter()
                .filter(|(_, (m, _))| m.project_id == project_id)
                .filter(|(_, (m, _))| bucket.is_none_or(|b| m.bucket == b))
                .map(|(id, (m, v))| (*id, m, cosine(query, v)))
                .collect();
            scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored
                .into_iter()
                .take(top_n)
                .map(|(id, m, s)| MemoryHit {
                    id,
                    memory: m.clone(),
                    score: Some(s),
                })
                .collect())
        }

        async fn delete(&self, _project_id: &str, id: Uuid) -> Result<()> {
            self.inner.lock().unwrap().remove(&id);
            Ok(())
        }

        async fn list(&self, project_id: &str, filter: MemoryFilter) -> Result<Vec<MemoryHit>> {
            let guard = self.inner.lock().unwrap();
            let mut hits: Vec<MemoryHit> = guard
                .iter()
                .filter(|(_, (m, _))| m.project_id == project_id)
                .filter(|(_, (m, _))| filter.bucket.as_deref().is_none_or(|b| m.bucket == b))
                .filter(|(_, (m, _))| {
                    filter
                        .anchor_path
                        .as_deref()
                        .is_none_or(|p| m.anchors.iter().any(|a| a.path == p))
                })
                .map(|(id, (m, _))| MemoryHit {
                    id: *id,
                    memory: m.clone(),
                    score: None,
                })
                .collect();
            if filter.order_by == MemoryOrder::UpdatedAtDesc {
                hits.sort_by(|a, b| b.memory.updated_at.cmp(&a.memory.updated_at));
            }
            if let Some(n) = filter.limit {
                hits.truncate(n);
            }
            Ok(hits)
        }
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

        // Bucket filter on search
        let hits = store
            .search("test-proj", &q, 5, Some("preferences"))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.title, "beta-pref");

        // List default (no filter) returns both
        assert_eq!(
            store
                .list("test-proj", MemoryFilter::default())
                .await
                .unwrap()
                .len(),
            2
        );

        // List filtered by anchor_path
        let by = store
            .list(
                "test-proj",
                MemoryFilter {
                    anchor_path: Some("src/a.rs".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(by.len(), 1);
        assert_eq!(by[0].memory.title, "alpha-system");

        // List filtered by bucket + limit
        let prefs = store
            .list(
                "test-proj",
                MemoryFilter {
                    bucket: Some("preferences".into()),
                    limit: Some(10),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].memory.title, "beta-pref");

        // Delete alpha
        store.delete("test-proj", alpha.point_id()).await.unwrap();
        let remaining = store
            .list("test-proj", MemoryFilter::default())
            .await
            .unwrap();
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

    /// Mirror of `semantic_memory_store_trait_roundtrip` against the test stub.
    /// Runs on every test invocation (no Qdrant needed) so trait-shape
    /// regressions get caught immediately even before the Qdrant E2E runs.
    #[tokio::test]
    async fn in_memory_store_trait_roundtrip() {
        use test_support::InMemorySemanticMemoryStore;

        let store = InMemorySemanticMemoryStore::new();

        let alpha = sample("alpha-system", "src/a.rs");
        let beta = sample("beta-pref", "src/b.rs");
        let v_alpha = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let v_beta = vec![0.7, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        store.upsert(&alpha, &v_alpha).await.unwrap();
        store.upsert(&beta, &v_beta).await.unwrap();

        // Search — alpha wins on cosine
        let q = vec![1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let hits = store.search("test-proj", &q, 2, None).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].memory.title, "alpha-system");

        // Bucket filter on search
        let hits = store
            .search("test-proj", &q, 5, Some("preferences"))
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.title, "beta-pref");

        // List default returns both
        assert_eq!(
            store
                .list("test-proj", MemoryFilter::default())
                .await
                .unwrap()
                .len(),
            2
        );

        // anchor_path filter
        let by = store
            .list(
                "test-proj",
                MemoryFilter {
                    anchor_path: Some("src/a.rs".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(by.len(), 1);
        assert_eq!(by[0].memory.title, "alpha-system");

        // bucket + limit
        let prefs = store
            .list(
                "test-proj",
                MemoryFilter {
                    bucket: Some("preferences".into()),
                    limit: Some(10),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(prefs.len(), 1);
        assert_eq!(prefs[0].memory.title, "beta-pref");

        // Project-scoping: a memory in a different project must not leak
        let other = SemanticMemory {
            project_id: "other-proj".into(),
            ..sample("orphan", "src/x.rs")
        };
        store.upsert(&other, &v_alpha).await.unwrap();
        assert_eq!(
            store
                .list("test-proj", MemoryFilter::default())
                .await
                .unwrap()
                .len(),
            2,
            "other-proj memory must not appear under test-proj"
        );

        // Delete + idempotency
        store.delete("test-proj", alpha.point_id()).await.unwrap();
        let remaining = store
            .list("test-proj", MemoryFilter::default())
            .await
            .unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].memory.title, "beta-pref");
        store.delete("test-proj", alpha.point_id()).await.unwrap();
    }

    /// Trait object compiles — proves the trait is dyn-compatible and that
    /// callers can store `Box<dyn SemanticMemoryStore>`.
    #[test]
    fn trait_is_dyn_compatible() {
        fn _accepts_dyn(_: &dyn SemanticMemoryStore) {}
        // Compiles iff the trait is dyn-compatible.
    }
}
