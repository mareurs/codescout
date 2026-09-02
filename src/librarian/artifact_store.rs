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
#[cfg(feature = "server-stack")]
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
    /// 3. Default: Qdrant on the server build; the daemon-free sqlite-vec store
    ///    on a lean build (no `server-stack` feature — Qdrant isn't compiled in).
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
        #[cfg(feature = "server-stack")]
        {
            ArtifactBackend::Qdrant
        }
        #[cfg(not(feature = "server-stack"))]
        {
            ArtifactBackend::SqliteVec
        }
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

    /// Dense KNN → ranked `(artifact_id, distance)` pairs, closest first.
    /// `project_id = Some` narrows to one project (single-project scope); `None`
    /// searches all (the catalog's scoped filter narrows after hydration either
    /// way).
    ///
    /// **`distance` is LOWER-IS-CLOSER on every backend, by contract.** The two
    /// implementations disagree natively — Qdrant's cosine `score` is a similarity
    /// (higher is better), sqlite-vec's `distance` column is L2 (lower is better)
    /// — so each converts to this polarity before returning. Returning the raw
    /// value would make the number's meaning depend on which backend answered,
    /// with Qdrant's default status hiding it from almost everyone.
    ///
    /// The SCALE remains backend-defined (cosine distance vs L2). Values are
    /// comparable **within one response**, which is what ranking and
    /// starvation-detection need; they are NOT comparable across backends, so no
    /// caller may threshold on an absolute number.
    async fn knn(
        &self,
        project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>>;
}

// ---------------------------------------------------------------------------
// Qdrant backend (default)
// ---------------------------------------------------------------------------

#[cfg(feature = "server-stack")]
pub struct QdrantArtifactStore {
    qdrant: QdrantWrap,
    collection: String,
    ensured: tokio::sync::OnceCell<()>,
}

#[cfg(feature = "server-stack")]
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

#[cfg(feature = "server-stack")]
#[async_trait]
impl ArtifactVectorStore for QdrantArtifactStore {
    async fn upsert(&self, project_id: &str, id: &str, vector: &[f32]) -> Result<()> {
        if vector.is_empty() {
            anyhow::bail!("artifact embedding dim is 0 (embedder returned an empty vector)");
        }
        // Chunk-grain Qdrant is DEFERRED, and a deferral is only honest if the
        // deferred path refuses the input it cannot represent. `upsert` carries
        // exactly one id, so a chunk id arriving here is written as BOTH the
        // point id and the payload's claimed `artifact_id` (retrieval/artifact.rs),
        // indistinguishable downstream from a real one — no error, no log line,
        // no observer.
        //
        // The two id spaces are shape-distinguishable, so this is exact rather
        // than a heuristic. Artifact ids are `sha256(abs_path)` hex[..16] —
        // 16 chars (librarian/ids.rs). Chunk ids are UUID v4 — 36 chars with
        // four dashes (catalog/chunk.rs) — and cannot satisfy the predicate.
        //
        // The observer is real: `index_repo`'s `s.upsert(project_id, id, vec)`
        // reaches here whenever `ArtifactBackend::resolve` returns Qdrant, which
        // is the default on the server build, and the error surfaces through
        // `index_repo`'s `?` into the reindex report.
        if id.len() != 16 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            anyhow::bail!(
                "QdrantArtifactStore is artifact-grain and was handed a non-artifact id {id:?}. \
                 Chunk-grain retrieval is implemented on the sqlite-vec backend only; \
                 set the artifact backend to sqlite-vec, or implement chunk-grain Qdrant."
            );
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

    async fn knn(
        &self,
        project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        if !self.qdrant.collection_exists(&self.collection).await? {
            return Ok(vec![]);
        }
        // `artifact_knn_scored` performs the similarity→distance flip; see its doc
        // comment for why that conversion lives there and not here.
        self.qdrant
            .artifact_knn_scored(&self.collection, project_id, query.to_vec(), k)
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
        // Chunk-grain: the embed queue is keyed by `chunk_id` (see
        // `indexer::embed_queue_items`), so vectors belong in the chunk-keyed
        // `artifact_vec_v2`, never the artifact-keyed v1 table.
        crate::librarian::indexer::write_embeddings_v2(&cat, &[(id.to_string(), vector.to_vec())])
    }

    async fn delete(&self, id: &str) -> Result<()> {
        let cat = self.catalog.lock();
        // `id` here is an ARTIFACT id — the trait's delete is artifact-grain —
        // so the chunk vectors must be reached via `artifact_chunk` rather than
        // deleted by id. The `artifact_vec_v2_cascade_delete` trigger is NOT
        // available on this path: it fires on `artifact_chunk` deletion, and no
        // artifact row is removed here. Missing this reds nothing; the cost is
        // an accumulating orphan set that no sweep collects.
        crate::librarian::indexer::delete_chunk_vectors(&cat, id)?;
        cat.conn.execute(
            "DELETE FROM artifact_vec WHERE id = ?1",
            rusqlite::params![id],
        )?;
        Ok(())
    }

    async fn knn(
        &self,
        _project_id: Option<&str>,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<(String, f32)>> {
        // sqlite-vec has no project_id column; the catalog's scoped filter does
        // the project narrowing after hydration (results match the Qdrant path).
        //
        // `artifact_vec` is declared `vec0(id, embedding FLOAT[768])` in schema.sql
        // with no distance metric, so sqlite-vec's default L2 applies and the
        // `distance` column is already lower-is-closer — the polarity the trait
        // requires. No conversion here, unlike the Qdrant path.
        let blob: Vec<u8> = query.iter().flat_map(|f| f.to_le_bytes()).collect();
        let cat = self.catalog.lock();
        let mut stmt = cat.conn.prepare(
            "SELECT id, distance FROM artifact_vec_v2 WHERE embedding MATCH vec_f32(?1) ORDER BY distance LIMIT ?2",
        )?;
        let hits = stmt
            .query_map(rusqlite::params![blob, k as i64], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)? as f32))
            })?
            .collect::<rusqlite::Result<Vec<(String, f32)>>>()?;
        Ok(hits)
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
        ) -> Result<Vec<(String, f32)>> {
            let pts = self.points.lock();
            // Cosine SIMILARITY internally, converted to the trait's
            // lower-is-closer distance on the way out — the same flip the
            // Qdrant backend does, so a test fixture cannot accidentally
            // encode the opposite polarity from production.
            let mut scored: Vec<(String, f32)> = pts
                .iter()
                .filter(|(_, (pid, _))| project_id.is_none_or(|p| p == pid))
                .map(|(id, (_, v))| (id.clone(), 1.0 - cosine(query, v)))
                .collect();
            scored.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored.into_iter().take(k).collect())
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
        let scoped: Vec<String> = store
            .knn(Some("p1"), &[1.0, 0.0], 10)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(scoped, vec!["a".to_string()]);
        // Unscoped → both.
        let mut all: Vec<String> = store
            .knn(None, &[1.0, 0.0], 10)
            .await
            .unwrap()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        all.sort();
        assert_eq!(all, vec!["a".to_string(), "b".to_string()]);
    }

    /// The trait's polarity contract: `knn` returns a DISTANCE, lower-is-closer,
    /// on every backend. The in-memory store computes cosine similarity natively
    /// and must flip it, exactly as the Qdrant backend flips `ScoredPoint.score`.
    ///
    /// Without this, a fixture encoding the OPPOSITE polarity from production
    /// would let every downstream ranking test pass while the two real backends
    /// disagreed about which direction is good — and Qdrant's default status
    /// would keep that hidden from all but escape-hatch users.
    ///
    /// BUG docs/issues/archive/2026-08-27-semantic-find-fills-the-page-past-relevance-with-no-score.md
    #[tokio::test]
    async fn knn_returns_lower_is_closer_distance() {
        let store = InMemoryArtifactStore::default();
        store.upsert("p", "near", &[1.0, 0.0]).await.unwrap();
        store.upsert("p", "far", &[0.0, 1.0]).await.unwrap();

        let hits = store.knn(Some("p"), &[1.0, 0.0], 10).await.unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].0, "near", "closest must sort first");
        assert!(
            hits[0].1 < hits[1].1,
            "distance must ASCEND with dissimilarity, got {hits:?}"
        );
        assert!(
            hits[0].1.abs() < 1e-6,
            "an identical vector is distance ~0, got {}",
            hits[0].1
        );
    }

    #[tokio::test]
    async fn delete_is_idempotent() {
        let store = InMemoryArtifactStore::default();
        store.upsert("p", "a", &[1.0]).await.unwrap();
        store.delete("a").await.unwrap();
        store.delete("a").await.unwrap(); // missing id → no-op
        assert!(store.knn(None, &[1.0], 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_sqlite_store_writes_a_chunk_id_into_v2_and_never_into_v1() {
        // LOAD-BEARING: this drives `SqliteVecArtifactStore::upsert` — the
        // PRODUCTION writer — and NOT `write_embeddings_v2` directly. A test
        // that calls the new function proves the function works and says
        // nothing about whether anything calls it, which is exactly how this
        // task would otherwise have shipped `write_embeddings_v2` as dead code.
        //
        // The ids MUST come from `embed_queue_items`, never a literal. Every
        // existing `semantic_find` test hand-feeds an ARTIFACT id, which is
        // precisely why none of them noticed when the queue was re-keyed to
        // chunk ids. Replace the queue with literals here and this test
        // silently rejoins that population.
        let cat = Catalog::open_in_memory().unwrap();
        crate::librarian::catalog::artifact::upsert(
            &cat,
            &crate::librarian::catalog::artifact::TestArtifactRowBuilder::new("a")
                .with_kind("tracker")
                .with_status("active")
                .build(),
        )
        .unwrap();
        let queue = crate::librarian::indexer::embed_queue_items(
            &cat,
            "a",
            Some("T".into()),
            "# T\n\n## W-1 — x\n\nalpha\n\n## W-2 — y\n\nbeta\n",
        )
        .unwrap();
        // Without >1 chunk the grain bug is UNREPRESENTABLE by this fixture: a
        // single-chunk artifact's chunk id and artifact id fail the same way,
        // so the test would pass under both the broken and the fixed writer.
        assert!(
            queue.len() > 1,
            "fixture must yield >1 chunk, got {}",
            queue.len()
        );

        let cat = std::sync::Arc::new(parking_lot::Mutex::new(cat));
        let store = SqliteVecArtifactStore::new(cat.clone());
        for (id, _, _) in &queue {
            store.upsert("proj", id, &vec![0.5f32; 768]).await.unwrap();
        }

        let guard = cat.lock();
        for (id, _, _) in &queue {
            let n: i64 = guard
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM artifact_vec_v2 WHERE id = ?1",
                    [id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 1, "chunk {id} must reach artifact_vec_v2");
        }
        // BOTH halves are required. The first is monotone under a writer that
        // writes v2 AND v1 — which is exactly what a half-finished re-point
        // looks like, and is a state this task passes through.
        let v1: i64 = guard
            .conn
            .query_row("SELECT COUNT(*) FROM artifact_vec", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v1, 0, "no chunk id may reach the artifact-keyed v1 table");
    }

    #[cfg(feature = "server-stack")]
    #[tokio::test]
    async fn qdrant_store_refuses_a_chunk_id_and_lets_an_artifact_id_through() {
        // Both fixtures are REAL shapes taken from their mint sites, never
        // hand-typed look-alikes: change either constructor and this test must
        // be re-derived rather than quietly keeping its old fixture.
        let chunk_like = uuid::Uuid::new_v4().to_string();
        let artifact_like =
            crate::librarian::ids::artifact_id_from_abs(std::path::Path::new("/test/a.md"));
        assert_eq!(
            artifact_like.len(),
            16,
            "artifact id shape moved — re-derive the guard in QdrantArtifactStore::upsert"
        );
        assert_ne!(
            chunk_like.len(),
            16,
            "chunk id shape moved — the guard's length discriminator no longer separates them"
        );

        // Points at a port nothing listens on. The guard runs BEFORE `ensure`,
        // so the REFUSAL direction never reaches the network and is
        // deterministic; the ACCEPT direction is expected to get past the guard
        // and fail at connect, which is this test's pass condition.
        let store = QdrantArtifactStore::new(
            crate::retrieval::qdrant::QdrantWrap {
                client: qdrant_client::Qdrant::from_url("http://127.0.0.1:1")
                    .skip_compatibility_check()
                    .build()
                    .expect("building a client must not require a server"),
            },
            "artifacts_test",
        );

        let refused = store
            .upsert("proj", &chunk_like, &[0.1f32; 8])
            .await
            .expect_err("a chunk id must be refused");
        assert!(
            refused.to_string().contains("artifact-grain"),
            "the refusal must name the GRAIN, not fail incidentally at the network: {refused}"
        );

        // LOAD-BEARING, and the half that makes the pair non-vacuous: without
        // it the accept direction is satisfied by ANY error — including the
        // guard wrongly firing on a valid artifact id — so the test would be
        // monotone in exactly the direction it exists to check.
        let passed_guard = store.upsert("proj", &artifact_like, &[0.1f32; 8]).await;
        if let Err(e) = passed_guard {
            assert!(
                !e.to_string().contains("artifact-grain"),
                "an artifact id must reach the network, not be refused by the guard: {e}"
            );
        }
    }
}
