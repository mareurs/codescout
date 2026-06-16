//! Vector store for **code chunks** — the backend behind `semantic_search`
//! (query) and `sync_project` (index).
//!
//! Phase 1 of the two-stack split (see
//! `docs/plans/2026-06-16-two-stack-retrieval-lite.md`): this trait is the seam
//! that lets the code-search backend be **Qdrant** (server / hybrid stack) or, in
//! a later phase, **in-process sqlite-vec** (the daemon-free lite stack). Today
//! the only production impl is [`QdrantWrap`]; the trait introduces no behavior
//! change — it just routes the existing calls through an interface.
//!
//! Mirrors the librarian's `ArtifactVectorStore` and memory's
//! `SemanticMemoryStore`: a small, store-agnostic surface with no Qdrant types in
//! the signatures (the `payload_to_map` conversion is pushed into the Qdrant impl).

use crate::retrieval::drift::ChunkRef;
use crate::retrieval::embedder::{EmbedOutput, SparseVector};
use crate::retrieval::payload::CodePayload;
use crate::retrieval::search::Hit;
use anyhow::Result;
use async_trait::async_trait;

/// Operations the code-search index + query paths perform against a vector store.
///
/// `collection` names the logical index (e.g. `code_chunks`); the Qdrant impl maps
/// it to a Qdrant collection, a future sqlite-vec impl to a table/namespace.
#[async_trait]
pub trait CodeVectorStore: Send + Sync {
    /// Ensure the backing collection exists with a `dim`-dimensional dense vector
    /// (+ a sparse vector on hybrid backends). Idempotent.
    async fn ensure_collection(&self, collection: &str, dim: u64) -> Result<()>;

    /// `(chunk_id, content_hash)` for every chunk already stored for `project_id`.
    /// Drives incremental drift detection in `sync_project`.
    async fn chunk_refs(&self, collection: &str, project_id: &str) -> Result<Vec<ChunkRef>>;

    /// Upsert code chunks with their dense (+ optional sparse) embeddings. The
    /// store derives point ids and payloads from the [`CodePayload`] internally.
    async fn upsert_chunks(
        &self,
        collection: &str,
        chunks: &[(CodePayload, EmbedOutput)],
    ) -> Result<()>;

    /// Delete chunks by id.
    async fn delete_chunks(&self, collection: &str, project_id: &str, ids: &[String])
        -> Result<()>;

    /// Query: hybrid dense+sparse RRF, or pure-dense ANN when `disable_sparse`.
    /// `exclude_languages` drops hits whose payload `language` is in the list.
    #[allow(clippy::too_many_arguments)]
    async fn query(
        &self,
        collection: &str,
        project_id: &str,
        dense: &[f32],
        sparse: &SparseVector,
        limit: usize,
        bm25_boost: f32,
        disable_sparse: bool,
        exclude_languages: &[String],
    ) -> Result<Vec<Hit>>;

    /// `(chunk_count, file_count)` for `project_id`.
    async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)>;
}
/// Which code-vector backend the retrieval client uses.
///
/// - `Qdrant` (default) — the server / hybrid stack.
/// - `SqliteVec` — the daemon-free lite stack (in-process `vec0`, dense-only).
///
/// Resolved from `CODESCOUT_VECTOR_BACKEND` (`qdrant` | `sqlite-vec` | `lite`).
/// Mirrors the librarian's `ArtifactBackend` selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorBackend {
    Qdrant,
    SqliteVec,
}

impl VectorBackend {
    pub fn resolve() -> Self {
        match std::env::var("CODESCOUT_VECTOR_BACKEND")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "sqlite-vec" | "sqlite_vec" | "sqlite" | "local" | "lite" => Self::SqliteVec,
            "qdrant" | "server" => Self::Qdrant,
            // Default depends on what's compiled in: the server build prefers the
            // Qdrant hybrid stack; a lean build has only the in-process sqlite-vec
            // backend, so default to it (never bail by surprise on a fresh setup).
            _ => {
                #[cfg(feature = "server-stack")]
                {
                    Self::Qdrant
                }
                #[cfg(not(feature = "server-stack"))]
                {
                    Self::SqliteVec
                }
            }
        }
    }
}

#[cfg(feature = "server-stack")]

/// The Qdrant (server / hybrid stack) implementation — a thin adapter over the
/// existing inherent `QdrantWrap` methods. UFCS (`QdrantWrap::method`) is used
/// where a trait method shares a name with an inherent method.
#[async_trait]
impl CodeVectorStore for crate::retrieval::qdrant::QdrantWrap {
    async fn ensure_collection(&self, collection: &str, dim: u64) -> Result<()> {
        crate::retrieval::qdrant::QdrantWrap::ensure_collection(self, collection, dim).await
    }

    async fn chunk_refs(&self, collection: &str, project_id: &str) -> Result<Vec<ChunkRef>> {
        self.scroll_chunk_refs(collection, project_id).await
    }

    async fn upsert_chunks(
        &self,
        collection: &str,
        chunks: &[(CodePayload, EmbedOutput)],
    ) -> Result<()> {
        let points: Vec<(
            String,
            std::collections::HashMap<String, qdrant_client::qdrant::Value>,
            EmbedOutput,
        )> = chunks
            .iter()
            .map(|(p, e)| {
                (
                    p.chunk_id.clone(),
                    crate::retrieval::payload::payload_to_map(p),
                    e.clone(),
                )
            })
            .collect();
        self.upsert_points(collection, &points).await
    }

    async fn delete_chunks(
        &self,
        collection: &str,
        _project_id: &str,
        ids: &[String],
    ) -> Result<()> {
        self.delete_points(collection, ids).await
    }

    async fn query(
        &self,
        collection: &str,
        project_id: &str,
        dense: &[f32],
        sparse: &SparseVector,
        limit: usize,
        bm25_boost: f32,
        disable_sparse: bool,
        exclude_languages: &[String],
    ) -> Result<Vec<Hit>> {
        self.hybrid_query(
            collection,
            project_id,
            dense,
            sparse,
            limit,
            bm25_boost,
            disable_sparse,
            exclude_languages,
        )
        .await
    }

    async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)> {
        crate::retrieval::qdrant::QdrantWrap::project_index_stats(self, collection, project_id)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::embedder::SparseVector;
    use crate::retrieval::payload::CodePayload;
    use parking_lot::Mutex;

    /// Pure-Rust, dependency-free reference impl: brute-force cosine over chunks
    /// held in memory. It exists to pin the `CodeVectorStore` contract — the
    /// sqlite-vec impl (a later phase) must satisfy the same tests. Dense-only:
    /// the `sparse` arg is ignored, matching the lite stack's behavior.
    #[derive(Default)]
    struct InMemoryCodeStore {
        // (payload, dense)
        chunks: Mutex<Vec<(CodePayload, Vec<f32>)>>,
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
    impl CodeVectorStore for InMemoryCodeStore {
        async fn ensure_collection(&self, _collection: &str, _dim: u64) -> Result<()> {
            Ok(())
        }

        async fn chunk_refs(&self, _collection: &str, project_id: &str) -> Result<Vec<ChunkRef>> {
            Ok(self
                .chunks
                .lock()
                .iter()
                .filter(|(p, _)| p.project_id == project_id)
                .map(|(p, _)| ChunkRef {
                    chunk_id: p.chunk_id.clone(),
                    content_hash: p.content_hash.clone(),
                })
                .collect())
        }

        async fn upsert_chunks(
            &self,
            _collection: &str,
            chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            let mut store = self.chunks.lock();
            for (p, e) in chunks {
                store.retain(|(existing, _)| existing.chunk_id != p.chunk_id);
                store.push((p.clone(), e.dense.clone()));
            }
            Ok(())
        }

        async fn delete_chunks(
            &self,
            _collection: &str,
            _project_id: &str,
            ids: &[String],
        ) -> Result<()> {
            let drop: std::collections::HashSet<&String> = ids.iter().collect();
            self.chunks
                .lock()
                .retain(|(p, _)| !drop.contains(&p.chunk_id));
            Ok(())
        }

        async fn query(
            &self,
            _collection: &str,
            project_id: &str,
            dense: &[f32],
            _sparse: &SparseVector,
            limit: usize,
            _bm25_boost: f32,
            _disable_sparse: bool,
            exclude_languages: &[String],
        ) -> Result<Vec<Hit>> {
            let mut scored: Vec<(f32, CodePayload)> = self
                .chunks
                .lock()
                .iter()
                .filter(|(p, _)| p.project_id == project_id)
                .filter(|(p, _)| !exclude_languages.contains(&p.language))
                .map(|(p, v)| (cosine(dense, v), p.clone()))
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            Ok(scored
                .into_iter()
                .take(limit)
                .map(|(score, p)| Hit {
                    chunk_id: p.chunk_id,
                    file_path: p.file_path,
                    start_line: p.start_line,
                    end_line: p.end_line,
                    content: p.content,
                    score,
                    rerank_score: None,
                })
                .collect())
        }

        async fn project_index_stats(
            &self,
            _collection: &str,
            project_id: &str,
        ) -> Result<(usize, usize)> {
            let store = self.chunks.lock();
            let chunks = store
                .iter()
                .filter(|(p, _)| p.project_id == project_id)
                .count();
            let files: std::collections::HashSet<&str> = store
                .iter()
                .filter(|(p, _)| p.project_id == project_id)
                .map(|(p, _)| p.file_path.as_str())
                .collect();
            Ok((chunks, files.len()))
        }
    }

    fn payload(id: &str, project: &str, file: &str, lang: &str, hash: &str) -> CodePayload {
        CodePayload {
            project_id: project.into(),
            file_path: file.into(),
            language: lang.into(),
            start_line: 1,
            end_line: 2,
            ast_kind: String::new(),
            ast_header: String::new(),
            content: format!("content of {id}"),
            content_hash: hash.into(),
            last_indexed_commit: String::new(),
            chunk_id: id.into(),
        }
    }

    fn embed(dense: Vec<f32>) -> EmbedOutput {
        EmbedOutput {
            dense,
            sparse: SparseVector {
                indices: vec![],
                values: vec![],
            },
        }
    }

    #[tokio::test]
    async fn contract_upsert_query_orders_by_cosine() {
        let store = InMemoryCodeStore::default();
        store
            .upsert_chunks(
                "code_chunks",
                &[
                    (
                        payload("a", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("b", "proj", "b.rs", "rust", "h2"),
                        embed(vec![0.0, 1.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        let hits = store
            .query(
                "code_chunks",
                "proj",
                &[1.0, 0.1],
                &SparseVector {
                    indices: vec![],
                    values: vec![],
                },
                10,
                3.0,
                true,
                &[],
            )
            .await
            .unwrap();
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].chunk_id, "a", "nearest by cosine should rank first");
    }

    #[tokio::test]
    async fn contract_delete_and_stats_and_refs() {
        let store = InMemoryCodeStore::default();
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("a", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("b", "proj", "a.rs", "rust", "h2"),
                        embed(vec![0.0, 1.0]),
                    ),
                    (
                        payload("c", "other", "z.rs", "rust", "h3"),
                        embed(vec![1.0, 1.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        // stats scoped by project: 2 chunks across 1 file for "proj"
        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (2, 1)
        );

        // refs reflect stored state, scoped by project
        let mut refs = store.chunk_refs("c", "proj").await.unwrap();
        refs.sort_by(|a, b| a.chunk_id.cmp(&b.chunk_id));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].chunk_id, "a");
        assert_eq!(refs[0].content_hash, "h1");

        // delete removes only the named id
        store
            .delete_chunks("c", "proj", &["a".to_string()])
            .await
            .unwrap();
        assert_eq!(
            store.project_index_stats("c", "proj").await.unwrap(),
            (1, 1)
        );
    }

    #[tokio::test]
    async fn contract_query_excludes_languages_and_scopes_project() {
        let store = InMemoryCodeStore::default();
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("a", "proj", "a.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("m", "proj", "m.md", "markdown", "h2"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("x", "other", "x.rs", "rust", "h3"),
                        embed(vec![1.0, 0.0]),
                    ),
                ],
            )
            .await
            .unwrap();

        let hits = store
            .query(
                "c",
                "proj",
                &[1.0, 0.0],
                &SparseVector {
                    indices: vec![],
                    values: vec![],
                },
                10,
                3.0,
                true,
                &["markdown".to_string()],
            )
            .await
            .unwrap();
        // "m" excluded by language, "x" excluded by project → only "a"
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "a");
    }
}
