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

    /// `(chunk_id, content_hash, file_path)` for every chunk already stored for
    /// `project_id`. Drives incremental drift detection in `sync_project`.
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
    /// `exclude_paths` drops hits whose payload `file_path` is in the list. Used by
    /// worktree search to suppress main's chunks for files the worktree changed;
    /// the worktree's delta project supplies those paths instead. Matching is exact
    /// string equality, so every entry must already be forward-slashed and
    /// project-relative -- the form the indexer writes via `to_forward_slash(rel_path)` --
    /// or the exclusion is a silent no-op.
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
        exclude_paths: &[String],
    ) -> Result<Vec<Hit>>;

    /// `(chunk_count, file_count)` for `project_id`.
    async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)>;

    /// Does this project have any indexed chunks at all?
    ///
    /// Deliberately separate from [`CodeVectorStore::project_index_stats`], which
    /// counts distinct files and therefore has to enumerate the project — O(corpus).
    /// Callers that only need existence must not pay that: the activation probe is
    /// bounded at two seconds and used to call `project_index_stats`, so on any real
    /// corpus it timed out, reported the project unindexed, and (by design) declined
    /// to cache the timeout — re-running the whole scan on every activation.
    ///
    /// Required rather than defaulted on purpose. A default delegating to
    /// `project_index_stats` would make that exact defect the behaviour a new backend
    /// inherits by not thinking about it.
    ///
    /// See `docs/issues/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`.
    async fn project_has_chunks(&self, collection: &str, project_id: &str) -> Result<bool>;

    /// Dense dimension this project's collection was created with, or `None`
    /// when it does not exist yet.
    ///
    /// Takes `project_id` as well as `collection` because the sqlite-vec store
    /// is per-project — `conn_for` keys on the project, not the collection.
    ///
    /// Deliberately has **no default implementation**: a backend that silently
    /// inherited `Ok(None)` would disable the dim guard with no diagnostic.
    /// Every implementor answers explicitly, so a new backend fails to compile
    /// rather than failing quietly.
    async fn collection_dim(&self, collection: &str, project_id: &str) -> Result<Option<u64>>;
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
        exclude_paths: &[String],
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
            exclude_paths,
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

    async fn project_has_chunks(&self, collection: &str, project_id: &str) -> Result<bool> {
        crate::retrieval::qdrant::QdrantWrap::project_has_chunks(self, collection, project_id).await
    }

    async fn collection_dim(&self, collection: &str, _project_id: &str) -> Result<Option<u64>> {
        // Qdrant collections are shared across projects, so project_id is unused
        // here — the dimension is a property of the collection itself.
        //
        // Fail-open by design, not just for the missing-collection case:
        // `collection_info` returning any `Err` (missing collection, a
        // transient network blip, an auth hiccup) maps to `Ok(None)`, which
        // disables `guard_index_dim` for this call rather than surfacing a
        // spurious failure on an unrelated transient error. This mirrors the
        // brief's own stance on this backend ("Qdrant rejects a wrong-dimension
        // upsert server-side anyway, so this backend loses less by abstaining
        // than sqlite does") — matching specifically on a gRPC NotFound status
        // would need `tonic` as a new direct dependency (it's currently only
        // transitive, via `qdrant-client`) to name the status code, for a
        // narrower guarantee this backend doesn't need: any error here already
        // has a legible server-side fallback at upsert time.
        match self.client.collection_info(collection).await {
            Ok(info) => Ok(dense_vector_size(&info)),
            Err(_) => Ok(None),
        }
    }
}

/// Walk a Qdrant collection-info response down to the size of its dense
/// ("dense") named vector. Confirmed against the vendored qdrant-client 1.17
/// proto types: `GetCollectionInfoResponse.result -> CollectionInfo.config ->
/// CollectionConfig.params -> CollectionParams.vectors_config ->
/// VectorsConfig.config`, a oneof of either a single unnamed `VectorParams`
/// (`Config::Params`) or a `VectorParamsMap` (`Config::ParamsMap`) — our
/// collections always use the latter (`ensure_collection` names the dense leg
/// "dense" alongside a sibling "sparse"), but `Config::Params` is handled too
/// so an unnamed-vector collection (never created by this code, but not
/// impossible to encounter) still resolves instead of silently returning
/// `None`.
#[cfg(feature = "server-stack")]
fn dense_vector_size(info: &qdrant_client::qdrant::GetCollectionInfoResponse) -> Option<u64> {
    use qdrant_client::qdrant::vectors_config::Config;
    let vectors_config = info
        .result
        .as_ref()?
        .config
        .as_ref()?
        .params
        .as_ref()?
        .vectors_config
        .as_ref()?
        .config
        .as_ref()?;
    match vectors_config {
        Config::Params(p) => Some(p.size),
        Config::ParamsMap(m) => m.map.get("dense").map(|p| p.size),
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
                    file_path: p.file_path.clone(),
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
            exclude_paths: &[String],
        ) -> Result<Vec<Hit>> {
            let mut scored: Vec<(f32, CodePayload)> = self
                .chunks
                .lock()
                .iter()
                .filter(|(p, _)| p.project_id == project_id)
                .filter(|(p, _)| !exclude_languages.contains(&p.language))
                .filter(|(p, _)| !exclude_paths.contains(&p.file_path))
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

        async fn project_has_chunks(&self, _collection: &str, project_id: &str) -> Result<bool> {
            let store = self.chunks.lock();
            Ok(store.iter().any(|(p, _)| p.project_id == project_id))
        }

        async fn collection_dim(
            &self,
            _collection: &str,
            _project_id: &str,
        ) -> Result<Option<u64>> {
            Ok(None)
        }
    }

    fn payload(id: &str, project: &str, file: &str, lang: &str, hash: &str) -> CodePayload {
        CodePayload {
            project_id: project.into(),
            file_path: file.into(),
            language: lang.into(),
            start_line: 1,
            end_line: 2,
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
    async fn contract_chunk_refs_carry_file_path() {
        // The dirty-set derivation needs main's PATH list to notice files deleted in
        // a worktree. chunk_id cannot be parsed for it (project_id may contain colons
        // -- see sqlite_code_store.rs:538), so ChunkRef must carry file_path directly.
        let store = InMemoryCodeStore::default();
        store
            .upsert_chunks(
                "c",
                &[(
                    payload("a", "proj", "src/a.rs", "rust", "h1"),
                    embed(vec![1.0, 0.0]),
                )],
            )
            .await
            .unwrap();

        let refs = store.chunk_refs("c", "proj").await.unwrap();
        assert_eq!(refs.len(), 1);
        assert_eq!(
            refs[0].file_path, "src/a.rs",
            "chunk_refs must expose file_path, not require parsing chunk_id"
        );
    }

    /// `project_has_chunks` must agree with `project_index_stats().0 > 0` in every
    /// state. It exists to answer that question WITHOUT enumerating the project, so
    /// the two can drift apart silently — this pins them together.
    ///
    /// The cost property itself is structural, not asserted here: the Qdrant impl
    /// scrolls `limit(1)` with no payload, sqlite uses `SELECT EXISTS`, and the trait
    /// method is required rather than defaulted so a new backend cannot inherit the
    /// enumerate-everything behaviour by omission.
    #[tokio::test]
    async fn contract_has_chunks_agrees_with_stats_and_costs_nothing_extra() {
        let store = InMemoryCodeStore::default();

        // Empty project: both say "nothing here".
        assert!(!store.project_has_chunks("c", "p1").await.unwrap());
        assert_eq!(store.project_index_stats("c", "p1").await.unwrap().0, 0);

        store
            .upsert_chunks(
                "c",
                &[(
                    payload("p1:a.rs:h1", "p1", "a.rs", "rust", "h1"),
                    embed(vec![1.0, 0.0]),
                )],
            )
            .await
            .unwrap();

        // Populated: both say "something here".
        assert!(store.project_has_chunks("c", "p1").await.unwrap());
        assert!(store.project_index_stats("c", "p1").await.unwrap().0 > 0);

        // Scoped to the project, not the collection — a different project's chunks
        // must not make this one look indexed. That confusion is exactly what the
        // activation probe would surface as a wrong `index.status`.
        assert!(!store.project_has_chunks("c", "p2").await.unwrap());
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
                &[],
            )
            .await
            .unwrap();
        // "m" excluded by language, "x" excluded by project → only "a"
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].chunk_id, "a");
    }

    #[tokio::test]
    async fn contract_query_excludes_paths() {
        // The worktree design serves main's vectors for every path EXCEPT the ones the
        // worktree changed. That exclusion is a store-level contract, so both backends
        // must honour it.
        let store = InMemoryCodeStore::default();
        store
            .upsert_chunks(
                "c",
                &[
                    (
                        payload("keep", "proj", "src/keep.rs", "rust", "h1"),
                        embed(vec![1.0, 0.0]),
                    ),
                    (
                        payload("drop", "proj", "src/drop.rs", "rust", "h2"),
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
                &[],
                &["src/drop.rs".to_string()],
            )
            .await
            .unwrap();

        assert!(
            hits.iter().all(|h| h.file_path != "src/drop.rs"),
            "excluded path must not appear in results"
        );
        assert!(
            hits.iter().any(|h| h.file_path == "src/keep.rs"),
            "exclusion must not empty the result set — the accepting case needs pinning too"
        );
    }

    /// Review round-2 I4: `dense_vector_size` is a pure function over plain
    /// prost structs, constructible without a real Qdrant — as shipped it had
    /// zero coverage, so a `"dense"` → other-key typo, or a
    /// `.get("dense")` → `.values().next()` mutation, was invisible. Four
    /// cases: the named-map shape our collections actually use, the unnamed
    /// single-vector shape handled for completeness, a map present but with
    /// no `"dense"` key, and a missing `result` (mirrors a not-found
    /// collection).
    #[cfg(feature = "server-stack")]
    mod dense_vector_size_tests {
        use super::super::dense_vector_size;
        use qdrant_client::qdrant::{
            vectors_config, CollectionConfig, CollectionInfo, CollectionParams,
            GetCollectionInfoResponse, VectorParams, VectorParamsMap, VectorsConfig,
        };
        use std::collections::HashMap;

        fn response_with(config: Option<vectors_config::Config>) -> GetCollectionInfoResponse {
            GetCollectionInfoResponse {
                result: Some(CollectionInfo {
                    config: Some(CollectionConfig {
                        params: Some(CollectionParams {
                            vectors_config: Some(VectorsConfig { config }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }
        }

        #[test]
        fn walks_the_named_dense_vector_out_of_a_params_map() {
            let mut map = HashMap::new();
            map.insert(
                "dense".to_string(),
                VectorParams {
                    size: 384,
                    ..Default::default()
                },
            );
            map.insert(
                "sparse".to_string(),
                VectorParams {
                    size: 999,
                    ..Default::default()
                },
            );
            let info = response_with(Some(vectors_config::Config::ParamsMap(VectorParamsMap {
                map,
            })));
            assert_eq!(
                dense_vector_size(&info),
                Some(384),
                "must read the \"dense\" key specifically, not any entry in the map \
                 (a .values().next() mutation would wrongly return 999 or 384 by luck \
                 of HashMap iteration order)"
            );
        }

        #[test]
        fn a_map_with_no_dense_key_is_none_not_some_other_entry() {
            let mut map = HashMap::new();
            map.insert(
                "sparse".to_string(),
                VectorParams {
                    size: 999,
                    ..Default::default()
                },
            );
            let info = response_with(Some(vectors_config::Config::ParamsMap(VectorParamsMap {
                map,
            })));
            assert_eq!(
                dense_vector_size(&info),
                None,
                "no \"dense\" key must be None — a .values().next() mutation would \
                 wrongly return 999 here"
            );
        }

        #[test]
        fn handles_the_unnamed_single_vector_shape() {
            let info = response_with(Some(vectors_config::Config::Params(VectorParams {
                size: 1536,
                ..Default::default()
            })));
            assert_eq!(dense_vector_size(&info), Some(1536));
        }

        #[test]
        fn a_missing_result_is_none() {
            // Mirrors what `collection_info` returns for a not-found collection.
            let info = GetCollectionInfoResponse::default();
            assert_eq!(dense_vector_size(&info), None);
        }
    }
}
