//! Backend-agnostic artifact vector storage with a configurable backend:
//! **Qdrant** (default) or **sqlite-vec** (the daemon-free escape hatch for
//! low-end / locked-down machines that can't run a Qdrant daemon — e.g. the
//! `vdi-windows` worktree).
//!
//! Both backends return identical, **project-scoped** results: the
//! authoritative scope is the catalog's filter AST applied at hydration (see
//! [`crate::librarian::catalog::find::find_by_ids_filtered`]). The Qdrant
//! backend additionally pre-filters its KNN by `project_id` for efficiency; the
//! sqlite-vec KNN is unscoped (the catalog filter narrows it). Selection is via
//! [`ArtifactBackend::resolve`] — env `CODESCOUT_ARTIFACT_BACKEND`, then
//! `[librarian] vector_backend` in `project.toml`, else the default (Qdrant).

use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

use crate::librarian::catalog::Catalog;
use crate::retrieval::qdrant::QdrantWrap;

/// Which vector backend the librarian artifact index uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactBackend {
    /// Default — a shared Qdrant `artifacts` collection. Needs a reachable
    /// Qdrant daemon.
    Qdrant,
    /// Escape hatch — the in-process sqlite-vec `artifact_vec` table. No
    /// daemon; works fully offline on low-end / locked-down machines.
    SqliteVec,
}

impl ArtifactBackend {
    /// Resolve the backend. Layered, highest-priority first:
    /// 1. `CODESCOUT_ARTIFACT_BACKEND=qdrant|sqlite-vec` env var.
    /// 2. `[librarian] vector_backend = "qdrant"|"sqlite-vec"` in
    ///    `<project>/.codescout/project.toml`.
    /// 3. Default: Qdrant.
    ///
    /// Mirrors `crate::server::librarian_enabled_at_runtime`.
    pub fn resolve(project_path: Option<&str>) -> Self {
        if let Ok(v) = std::env::var("CODESCOUT_ARTIFACT_BACKEND") {
            if let Some(b) = Self::parse(&v) {
                return b;
            }
        }
        if let Some(root) = project_path {
            let cfg = std::path::Path::new(root)
                .join(".codescout")
                .join("project.toml");
            if let Ok(text) = std::fs::read_to_string(&cfg) {
                if let Ok(parsed) = toml::from_str::<toml::Value>(&text) {
                    if let Some(v) = parsed
                        .get("librarian")
                        .and_then(|t| t.get("vector_backend"))
                        .and_then(|v| v.as_str())
                    {
                        if let Some(b) = Self::parse(v) {
                            return b;
                        }
                    }
                }
            }
        }
        ArtifactBackend::Qdrant
    }

    fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "qdrant" => Some(Self::Qdrant),
            "sqlite-vec" | "sqlite_vec" | "sqlite" | "local" => Some(Self::SqliteVec),
            _ => None,
        }
    }
}

/// Backend-agnostic artifact vector store. Implementations:
/// - [`QdrantArtifactStore`] — default.
/// - [`SqliteVecArtifactStore`] — the daemon-free escape hatch.
#[async_trait]
pub trait ArtifactVectorStore: Send + Sync {
    /// Upsert an artifact's embedding tagged with its `project_id`. Idempotent
    /// on id — a second call with the same id overwrites the vector in place.
    async fn upsert(&self, project_id: &str, id: &str, vector: &[f32]) -> Result<()>;

    /// Delete an artifact's embedding by id. Idempotent — a missing id is a
    /// no-op.
    async fn delete(&self, id: &str) -> Result<()>;

    /// Dense KNN → ranked artifact ids (closest first). `project_id = Some`
    /// narrows to one project (single-project scope); `None` searches all (the
    /// catalog's scoped filter narrows after hydration either way).
    async fn knn(&self, project_id: Option<&str>, query: &[f32], k: usize) -> Result<Vec<String>>;
}

// ---------------------------------------------------------------------------
// Qdrant backend (default)
// ---------------------------------------------------------------------------

pub struct QdrantArtifactStore {
    qdrant: QdrantWrap,
    collection: String,
    ensured: tokio::sync::OnceCell<()>,
}

impl QdrantArtifactStore {
    /// Construct over a connected Qdrant. The collection is bootstrapped
    /// lazily on the first upsert (dim taken from the first vector), so a
    /// remote embedder whose dimension is only known after the first embed
    /// still works.
    pub fn new(qdrant: QdrantWrap, collection: impl Into<String>) -> Self {
        Self {
            qdrant,
            collection: collection.into(),
            ensured: tokio::sync::OnceCell::new(),
        }
    }

    async fn ensure(&self, dim: u64) -> Result<()> {
        self.ensured
            .get_or_try_init(|| {
                self.qdrant
                    .ensure_artifacts_collection(&self.collection, dim)
            })
            .await
            .map(|_| ())
    }
}

#[async_trait]
impl ArtifactVectorStore for QdrantArtifactStore {
    async fn upsert(&self, project_id: &str, id: &str, vector: &[f32]) -> Result<()> {
        if vector.is_empty() {
            anyhow::bail!("artifact embedding dim is 0 (embedder returned an empty vector)");
        }
        self.ensure(vector.len() as u64).await?;
        self.qdrant
            .artifact_upsert(&self.collection, project_id, id, vector.to_vec())
            .await
    }

    async fn delete(&self, id: &str) -> Result<()> {
        if !self.qdrant.collection_exists(&self.collection).await? {
            return Ok(());
        }
        self.qdrant.artifact_delete(&self.collection, id).await
    }

    async fn knn(&self, project_id: Option<&str>, query: &[f32], k: usize) -> Result<Vec<String>> {
        if !self.qdrant.collection_exists(&self.collection).await? {
            return Ok(vec![]);
        }
        self.qdrant
            .artifact_knn_ids(&self.collection, project_id, query.to_vec(), k)
            .await
    }
}

// ---------------------------------------------------------------------------
// sqlite-vec backend (escape hatch)
// ---------------------------------------------------------------------------

pub struct SqliteVecArtifactStore {
    catalog: Arc<parking_lot::Mutex<Catalog>>,
}

impl SqliteVecArtifactStore {
    pub fn new(catalog: Arc<parking_lot::Mutex<Catalog>>) -> Self {
        Self { catalog }
    }
}

#[async_trait]
impl ArtifactVectorStore for SqliteVecArtifactStore {
    async fn upsert(&self, _project_id: &str, id: &str, vector: &[f32]) -> Result<()> {
        // Delegate to the catalog's batch writer — reuses its dimension
        // validation and the BUG-045 DELETE-then-INSERT idempotency contract
        // verbatim (so the sqlite-vec backend behaves exactly as before).
        let cat = self.catalog.lock();
        crate::librarian::indexer::write_embeddings(&cat, &[(id.to_string(), vector.to_vec())])
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let cat = self.catalog.lock();
        cat.conn.execute(
            "DELETE FROM artifact_vec WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    async fn knn(&self, _project_id: Option<&str>, query: &[f32], k: usize) -> Result<Vec<String>> {
        // sqlite-vec has no project_id column; the catalog's scoped filter does
        // the project narrowing after hydration (results match the Qdrant path).
        let blob: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
        let cat = self.catalog.lock();
        let mut stmt = cat.conn.prepare(
            "SELECT id FROM artifact_vec WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2",
        )?;
        let ids = stmt
            .query_map(rusqlite::params![blob, k as i64], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(ids)
    }
}

#[cfg(test)]
pub mod test_support {
    //! In-memory artifact store for trait-level + coordinator tests.
    use super::*;
    use std::collections::HashMap;

    /// Brute-force cosine KNN over an in-memory map. Honors `project_id`
    /// filtering so coordinator tests exercise the same scoping as Qdrant.
    #[derive(Default)]
    pub struct InMemoryArtifactStore {
        // id -> (project_id, vector)
        points: parking_lot::Mutex<HashMap<String, (String, Vec<f32>)>>,
    }

    fn cosine(a: &[f32], b: &[f32]) -> f32 {
        let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
        let na = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let nb = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if na == 0.0 || nb == 0.0 {
            0.0
        } else {
            dot / (na * nb)
        }
    }

    #[async_trait]
    impl ArtifactVectorStore for InMemoryArtifactStore {
        async fn upsert(&self, project_id: &str, id: &str, vector: &[f32]) -> Result<()> {
            self.points
                .lock()
                .insert(id.to_string(), (project_id.to_string(), vector.to_vec()));
            Ok(())
        }

        async fn delete(&self, id: &str) -> Result<()> {
            self.points.lock().remove(id);
            Ok(())
        }

        async fn knn(
            &self,
            project_id: Option<&str>,
            query: &[f32],
            k: usize,
        ) -> Result<Vec<String>> {
            let pts = self.points.lock();
            let mut scored: Vec<(String, f32)> = pts
                .iter()
                .filter(|(_, (pid, _))| project_id.is_none_or(|p| p == pid))
                .map(|(id, (_, v))| (id.clone(), cosine(query, v)))
                .collect();
            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored.into_iter().take(k).map(|(id, _)| id).collect())
        }
    }
}
#[cfg(test)]
mod backend_tests {
    use super::test_support::InMemoryArtifactStore;
    use super::*;

    #[test]
    fn parse_recognizes_known_backends() {
        assert_eq!(
            ArtifactBackend::parse("qdrant"),
            Some(ArtifactBackend::Qdrant)
        );
        assert_eq!(
            ArtifactBackend::parse("sqlite-vec"),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(
            ArtifactBackend::parse("  SQLite  "),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(
            ArtifactBackend::parse("local"),
            Some(ArtifactBackend::SqliteVec)
        );
        assert_eq!(ArtifactBackend::parse("nonsense"), None);
    }

    #[tokio::test]
    async fn knn_filters_by_project_id() {
        let store = InMemoryArtifactStore::default();
        store.upsert("p1", "a", &[1.0, 0.0]).await.unwrap();
        store.upsert("p2", "b", &[1.0, 0.0]).await.unwrap();

        // Scoped to p1 → only "a".
        assert_eq!(
            store.knn(Some("p1"), &[1.0, 0.0], 10).await.unwrap(),
            vec!["a".to_string()]
        );
        // Unscoped → both.
        let mut all = store.knn(None, &[1.0, 0.0], 10).await.unwrap();
        all.sort();
        assert_eq!(all, vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let store = InMemoryArtifactStore::default();
        store.upsert("p", "a", &[1.0]).await.unwrap();
        store.delete("a").await.unwrap();
        store.delete("a").await.unwrap(); // missing id → no-op
        assert!(store.knn(None, &[1.0], 10).await.unwrap().is_empty());
    }
}
