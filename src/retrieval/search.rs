use anyhow::Result;

use crate::retrieval::client::RetrievalClient;

/// Should `search_in` run the cross-encoder rerank step?
///
/// All four conditions must hold, and each vetoes for a different reason:
///
/// - `caller_wants` — the per-call [`SearchOpts::rerank`]. A caller may suppress.
/// - `operator_enabled` — [`RetrievalConfig::rerank`], from `CODESCOUT_RERANK`, **default
///   off**. This is the outer bound: a caller cannot enable reranking against the
///   operator's configuration. Off by default because it measured ~569 ms per query for
///   no gain in score (2026-08-07; see that field's docs).
/// - `lite` — the daemon-free sqlite-vec stack runs no reranker server at all.
/// - `n_candidates` — nothing to reorder.
///
/// Extracted as a pure fn so the precedence is exhaustively testable without an async
/// client, a mock server, or process-env mutation. The composition is the part worth
/// pinning: reading `caller_wants && operator_enabled` as either one alone is the
/// mistake that would either ignore the operator or ignore the caller.
pub(crate) fn should_rerank(
    caller_wants: bool,
    operator_enabled: bool,
    lite: bool,
    n_candidates: usize,
) -> bool {
    caller_wants && operator_enabled && !lite && n_candidates > 0
}

/// Options controlling search behaviour.
#[derive(Debug, Clone)]
pub struct SearchOpts {
    /// Number of final hits to return after reranking.
    pub limit: usize,
    /// Number of candidates fetched from Qdrant before reranking.
    pub overfetch: usize,
    /// Whether this CALLER objects to reranking. Degrades gracefully on reranker
    /// failure.
    ///
    /// `true` means "no objection", **not** "will rerank": the operator's
    /// `CODESCOUT_RERANK` opt-in (`RetrievalConfig::rerank`, default off) is the outer
    /// bound, so a caller can suppress reranking but cannot enable it against the
    /// operator's configuration. See `should_rerank`.
    pub rerank: bool,
    /// Payload `language` values to exclude (Qdrant `must_not` clause). Used by
    /// `semantic_search(mode="code")` to drop markdown noise. Empty = no filter.
    pub exclude_languages: Vec<String>,
    /// Payload `file_path` values to exclude (Qdrant `must_not`; post-filtered in
    /// the lite store). Set by worktree search to paths the worktree changed;
    /// empty = no filter.
    pub exclude_paths: Vec<String>,
}

impl SearchOpts {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            overfetch: limit * 2,
            rerank: true,
            exclude_languages: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }
}

impl Default for SearchOpts {
    fn default() -> Self {
        Self {
            limit: 10,
            overfetch: 20,
            rerank: true,
            exclude_languages: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }
}

/// A single search result returned by any `search_*` method.
#[derive(Debug, Clone)]
pub struct Hit {
    pub chunk_id: String,
    pub file_path: String,
    pub start_line: i64,
    pub end_line: i64,
    pub content: String,
    /// RRF score from Qdrant (before reranking).
    pub score: f32,
    /// Cross-encoder score, populated when reranking succeeds.
    pub rerank_score: Option<f32>,
}

impl RetrievalClient {
    /// Core helper: embed → query (hybrid or dense-only) → optional rerank.
    ///
    /// `overlay_project_id`, when present, unions a second project into the
    /// same ranking with `opts.exclude_paths` applied to `project_id` only —
    /// the worktree main+delta query. It is deliberately handed to the store
    /// as one call rather than composed here from two: see
    /// [`crate::retrieval::code_store::CodeVectorStore::query_overlay`] for why
    /// a caller cannot merge two lists correctly without knowing the backend.
    async fn search_in(
        &self,
        collection: &str,
        project_id: &str,
        overlay_project_id: Option<&str>,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        self.guard_index_dim(collection, project_id).await?;
        let mut timer = crate::perf::PhaseTimer::start("semantic_search");
        let q = self.embedder.embed_one(query).await?;
        timer.lap("embed");
        let candidates = match overlay_project_id {
            Some(overlay) => {
                self.code_store
                    .query_overlay(
                        collection,
                        project_id,
                        overlay,
                        &q.dense,
                        &q.sparse,
                        opts.overfetch,
                        self.config.bm25_boost,
                        self.config.disable_sparse,
                        &opts.exclude_languages,
                        &opts.exclude_paths,
                    )
                    .await?
            }
            None => {
                self.code_store
                    .query(
                        collection,
                        project_id,
                        &q.dense,
                        &q.sparse,
                        opts.overfetch,
                        self.config.bm25_boost,
                        self.config.disable_sparse,
                        &opts.exclude_languages,
                        &opts.exclude_paths,
                    )
                    .await?
            }
        };
        timer.lap("vector_query");

        if !should_rerank(opts.rerank, self.config.rerank, self.lite, candidates.len()) {
            timer.finish();
            return Ok(candidates.into_iter().take(opts.limit).collect());
        }

        let texts: Vec<String> = candidates.iter().map(|h| h.content.clone()).collect();
        match self.reranker.rerank(query, &texts).await {
            Ok(scores) => {
                timer.lap("rerank");
                timer.finish();
                let mut zipped: Vec<(Hit, f32)> = candidates.into_iter().zip(scores).collect();
                zipped.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
                Ok(zipped
                    .into_iter()
                    .take(opts.limit)
                    .map(|(mut h, s)| {
                        h.rerank_score = Some(s);
                        h
                    })
                    .collect())
            }
            Err(e) => {
                timer.lap("rerank_degraded");
                timer.finish();
                tracing::warn!("reranker degraded: {e}");
                Ok(candidates.into_iter().take(opts.limit).collect())
            }
        }
    }

    pub async fn search_code(
        &self,
        project_id: &str,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        self.search_in(
            &self.config.collection("code_chunks"),
            project_id,
            None,
            query,
            opts,
        )
        .await
    }

    /// The worktree main+delta search: `project_id` (main) with
    /// `opts.exclude_paths` applied, unioned with `overlay_project_id` (the
    /// worktree's delta project) which excludes nothing, returned as ONE
    /// relevance ranking.
    ///
    /// Callers stay backend-agnostic on purpose — whether this costs one
    /// round trip or two is the store's business. See
    /// [`crate::retrieval::code_store::CodeVectorStore::query_overlay`].
    pub async fn search_code_overlay(
        &self,
        project_id: &str,
        overlay_project_id: &str,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        self.search_in(
            &self.config.collection("code_chunks"),
            project_id,
            Some(overlay_project_id),
            query,
            opts,
        )
        .await
    }

    pub async fn search_memories(
        &self,
        project_id: &str,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        self.search_in(
            &self.config.collection("memories"),
            project_id,
            None,
            query,
            opts,
        )
        .await
    }

    /// Search across all library chunks regardless of project.
    pub async fn search_libraries(&self, query: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        self.search_in(
            &self.config.collection("library_chunks"),
            "*",
            None,
            query,
            opts,
        )
        .await
    }
}

/// Merge two score-ordered hit lists and truncate to `limit`.
///
/// Exact for top-`limit` when each source was queried at `limit`: a hit in the
/// global top-`limit` is necessarily within its own source's top-`limit`.
///
/// # This is only valid when the two sources' scores are comparable
///
/// **It is not universally.** This docstring used to assert "Scores are cosine
/// from the same model, so they are comparable across sources", and that
/// sentence was false on the default backend — it is what made C1 look safe.
///
/// - `InMemoryCodeStore` returns cosine and `SqliteVecCodeStore` returns
///   `1 / (1 + distance)`. Both are absolute functions of content, so merging
///   two of their lists is meaningful. This function is correct there.
/// - `QdrantWrap` with `disable_sparse == false` — the **default**
///   configuration ([`crate::retrieval::config::RetrievalConfig`]) — returns
///   the **RRF fusion score**, which depends on *rank position only*.
///   Measured on the live collection: 0.5, 0.333, 0.25, 0.2, 0.167 … =
///   `1 / (1 + rank)`, identical whether the queried project holds three
///   chunks or half a million. Feeding two such lists in here sorts a
///   3-chunk delta's top-3 level with a 500k-chunk index's top-3; the sort
///   is stable, so the result is literally `a[0], b[0], a[1], b[1], …` and
///   the smaller source takes half of every page regardless of relevance.
///
/// So: only call this on lists whose scores mean the same thing. Cross-project
/// code search goes through
/// [`crate::retrieval::code_store::CodeVectorStore::query_overlay`] instead,
/// which lets a rank-fusion backend answer with a single ranking; that trait
/// method's *default implementation* is this function, which is exactly the
/// right default for the score-comparable stores and is overridden by Qdrant.
pub fn merge_hits(a: Vec<Hit>, b: Vec<Hit>, limit: usize) -> Vec<Hit> {
    let mut all: Vec<Hit> = a.into_iter().chain(b).collect();
    all.sort_by(|x, y| {
        y.score
            .partial_cmp(&x.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    all.truncate(limit);
    all
}

#[cfg(test)]
mod merge_tests {
    use super::*;

    fn hit(id: &str, score: f32) -> Hit {
        Hit {
            chunk_id: id.to_string(),
            file_path: "unused.rs".to_string(),
            start_line: 1,
            end_line: 1,
            content: String::new(),
            score,
            rerank_score: None,
        }
    }

    #[test]
    fn merge_at_limit_equals_the_true_global_top_k() {
        // Each source is queried at `limit`, and the merge is exact: a hit in the
        // global top-k is necessarily in its own source's top-k. So no over-fetch
        // is needed for the merge (the lite store's internal k-widening is
        // separate).
        let a = vec![hit("a1", 0.90), hit("a2", 0.50)];
        let b = vec![hit("b1", 0.70), hit("b2", 0.60)];
        let merged = merge_hits(a, b, 3);
        let ids: Vec<&str> = merged.iter().map(|h| h.chunk_id.as_str()).collect();
        assert_eq!(ids, vec!["a1", "b1", "b2"]);
    }

    #[test]
    fn merge_returns_fewer_than_limit_when_sources_are_short() {
        let merged = merge_hits(vec![hit("a1", 0.9)], vec![], 5);
        assert_eq!(
            merged.len(),
            1,
            "must not pad or panic when sources are short"
        );
    }
}

#[cfg(test)]
mod rerank_gate_tests {
    use super::should_rerank;

    /// The whole truth table, all 16 combinations. Exhaustive rather than representative
    /// because the function is four booleans wide and cheap to call, and because every
    /// plausible mutation of it — dropping a conjunct, flipping a polarity, swapping
    /// `&&` for `||` — changes at least one row. A three-case test lets several through.
    #[test]
    fn should_rerank_requires_all_four_conditions() {
        for caller in [false, true] {
            for operator in [false, true] {
                for lite in [false, true] {
                    for n in [0usize, 5] {
                        let expected = caller && operator && !lite && n > 0;
                        assert_eq!(
                            should_rerank(caller, operator, lite, n),
                            expected,
                            "caller={caller} operator={operator} lite={lite} n={n}"
                        );
                    }
                }
            }
        }
    }

    /// The row that encodes the 2026-08-07 decision, called out separately so a change to
    /// the DEFAULT is a visibly failing test rather than a silent behaviour flip.
    #[test]
    fn caller_cannot_enable_reranking_against_the_operator() {
        assert!(
            !should_rerank(true, false, false, 20),
            "SearchOpts::rerank defaults to true, so if this ever passes the reranker is \
             back on by default and ~569 ms/query is being paid silently"
        );
        assert!(
            should_rerank(true, true, false, 20),
            "and with CODESCOUT_RERANK=1 it must actually run"
        );
    }
}

#[cfg(test)]
mod dim_guard_tests {
    use super::*;
    use crate::retrieval::client::RetrievalClient;
    use crate::retrieval::code_store::CodeVectorStore;
    use crate::retrieval::config::RetrievalConfig;
    use crate::retrieval::drift::ChunkRef;
    use crate::retrieval::embedder::{
        BatchEmbedder, CodeEmbedder, EmbedOutput, EmbedderHttp, SparseVector,
    };
    use crate::retrieval::payload::CodePayload;
    use crate::retrieval::reranker::RerankerHttp;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Reports a fixed stored dim and records whether `query` was ever reached,
    /// so a test can prove `guard_index_dim` short-circuits `search_in` BEFORE
    /// the store runs — not just that some error eventually surfaces.
    #[derive(Default)]
    struct DimReportingStore {
        dim: Option<u64>,
        queried: AtomicBool,
    }

    #[async_trait]
    impl CodeVectorStore for DimReportingStore {
        async fn ensure_collection(&self, _c: &str, _d: u64) -> Result<()> {
            Ok(())
        }
        async fn chunk_refs(&self, _c: &str, _p: &str) -> Result<Vec<ChunkRef>> {
            Ok(vec![])
        }
        async fn upsert_chunks(
            &self,
            _c: &str,
            _chunks: &[(CodePayload, EmbedOutput)],
        ) -> Result<()> {
            Ok(())
        }
        async fn delete_chunks(&self, _c: &str, _p: &str, _ids: &[String]) -> Result<()> {
            Ok(())
        }
        #[allow(clippy::too_many_arguments)]
        async fn query(
            &self,
            _c: &str,
            _p: &str,
            _dense: &[f32],
            _sparse: &SparseVector,
            _limit: usize,
            _bm25: f32,
            _disable_sparse: bool,
            _excl: &[String],
            _paths: &[String],
        ) -> Result<Vec<Hit>> {
            self.queried.store(true, Ordering::SeqCst);
            Ok(vec![])
        }
        async fn project_index_stats(&self, _c: &str, _p: &str) -> Result<(usize, usize)> {
            Ok((0, 0))
        }
        async fn project_has_chunks(&self, _c: &str, _p: &str) -> Result<bool> {
            Ok(false)
        }
        async fn collection_dim(&self, _c: &str, _p: &str) -> Result<Option<u64>> {
            Ok(self.dim)
        }
    }

    /// A `CodeEmbedder` fake standing in for `CodeEmbedderAdapter` (a local
    /// backend that self-describes its dimension) without a real ONNX load.
    /// Every method but `known_dim` is unreachable — these tests only exercise
    /// `guard_index_dim`/`RetrievalClient::effective_model_dim`, which never
    /// call embed.
    struct FixedDimEmbedder(usize);

    #[async_trait]
    impl BatchEmbedder for FixedDimEmbedder {
        async fn embed_batch_dyn(&self, _texts: &[String]) -> Result<Vec<EmbedOutput>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
    }

    #[async_trait]
    impl CodeEmbedder for FixedDimEmbedder {
        async fn embed_one(&self, _text: &str) -> Result<EmbedOutput> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_dense_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }
        async fn embed_document_one(&self, _text: &str) -> Result<Vec<f32>> {
            unreachable!("FixedDimEmbedder is only used to answer known_dim()")
        }

        fn known_dim(&self) -> Option<usize> {
            Some(self.0)
        }
    }

    fn client_with_store_and_embedder(
        store: Arc<dyn CodeVectorStore>,
        embedder: Arc<dyn CodeEmbedder>,
        model_dim: Option<usize>,
    ) -> RetrievalClient {
        RetrievalClient {
            code_store: store,
            embedder,
            reranker: RerankerHttp::new("http://unused.invalid"),
            config: RetrievalConfig {
                qdrant_url: "http://unused.invalid".into(),
                embedder_url: Some("http://unused.invalid".into()),
                sparse_embedder_url: "http://unused.invalid".into(),
                reranker_url: "http://unused.invalid".into(),
                model_dim,
                model: "local:AllMiniLML6V2Q".into(),
                api_key: None,
                profile: "cpu".into(),
                bm25_boost: 1.0,
                disable_sparse: false,
                rerank: false,
                collection_prefix: String::new(),
            },
            lite: false,
        }
    }

    fn client_with_store(store: Arc<dyn CodeVectorStore>, model_dim: usize) -> RetrievalClient {
        client_with_store_and_embedder(
            store,
            Arc::new(EmbedderHttp::new(
                "http://unused.invalid",
                "http://unused.invalid",
                3,
            )),
            Some(model_dim),
        )
    }

    /// Asserts the error is `RecoverableError` (isError: false — sibling
    /// parallel tool calls survive) carrying the reindex remedy, not merely
    /// some error with the right numbers in its `Display`. Review round-2 I2:
    /// `RecoverableError`'s `Display` appends the hint text, so a prior
    /// version of these tests that asserted only on `format!("{err:#}")`
    /// stayed green even if the guard's `RecoverableError::with_hint(...)`
    /// were replaced wholesale with a bare `anyhow::anyhow!(...)` — dropping
    /// the hint AND flipping the MCP contract from `isError: false` to `true`.
    fn assert_dim_guard_error(err: &anyhow::Error) {
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false) so sibling parallel tool calls \
             survive a dimension mismatch; got: {err:#}"
        );
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Delete the code index"),
            "must carry the reindex remedy, got: {msg}"
        );
    }

    /// Call-site mutation target for `guard_index_dim`'s wiring into
    /// `search_in`. The store reports an existing index at dim 999 against a
    /// configured `model_dim` of 3; absent the
    /// `self.guard_index_dim(collection, project_id).await?;` line at the top
    /// of `search_in`, this call would instead proceed to embed the query
    /// (against `http://unused.invalid`, which errors for an unrelated reason)
    /// and then query the store. Asserting BOTH the specific error AND that
    /// the store was never queried distinguishes "the guard fired first" from
    /// "something else failed downstream".
    #[tokio::test]
    async fn search_in_fails_fast_on_a_dim_mismatch_without_querying_the_store() {
        let store = Arc::new(DimReportingStore {
            dim: Some(999),
            ..Default::default()
        });
        let client = client_with_store(store.clone(), 3);
        let err = client
            .search_code("proj", "query text", SearchOpts::new(5))
            .await
            .expect_err("a stored dim of 999 must fail against the configured model_dim of 3");
        assert_dim_guard_error(&err);
        let msg = format!("{err:#}");
        assert!(
            msg.contains("999") && msg.contains('3'),
            "error should name both the stored and configured dims, got: {msg}"
        );
        assert!(
            !store.queried.load(Ordering::SeqCst),
            "guard must short-circuit search_in before the store is ever queried"
        );
    }

    /// Pins `==` as `guard_index_dim`'s comparison, not `>=` or `<=`. My first
    /// draft of the call-site tests above only exercised the
    /// pinned-dim-smaller-than-stored direction (3 vs 999), which an `>=`
    /// mutation of the guard survives undetected (`3 >= 999` is false either
    /// way — the mutation and the original agree by accident). Both
    /// directions must error so a comparison-operator mutation in either
    /// direction is caught.
    #[tokio::test]
    async fn guard_index_dim_errors_in_both_mismatch_directions() {
        let bigger_index = Arc::new(DimReportingStore {
            dim: Some(999),
            ..Default::default()
        });
        let client = client_with_store(bigger_index, 3);
        let err = client
            .guard_index_dim("code_chunks", "proj")
            .await
            .expect_err("configured 3 vs stored 999 must error");
        assert_dim_guard_error(&err);
        assert!(format!("{err:#}").contains("999"));

        let smaller_index = Arc::new(DimReportingStore {
            dim: Some(3),
            ..Default::default()
        });
        let client = client_with_store(smaller_index, 999);
        let err = client
            .guard_index_dim("code_chunks", "proj")
            .await
            .expect_err("configured 999 vs stored 3 must error");
        assert_dim_guard_error(&err);
        assert!(format!("{err:#}").contains('3'));
    }

    /// Review round-2 I1: the guard used to compare against
    /// `self.config.model_dim.unwrap_or(index_dim)` — when `CODESCOUT_MODEL_DIM`
    /// is unset (the common, documented case: "the model is the authority"),
    /// `model_dim` *became* `index_dim` by construction and the comparison
    /// could never fail. That's exactly the plan's headline migration: an
    /// index built at 768 by a remote model, switched to an unpinned local
    /// model that is actually 384-dimensional. Here `model_dim: None` and the
    /// embedder reports 384 via `known_dim()` — the guard must use THAT, not
    /// silently agree with whatever the store says.
    #[tokio::test]
    async fn guard_index_dim_catches_an_unpinned_local_model_switch() {
        let store = Arc::new(DimReportingStore {
            dim: Some(768),
            ..Default::default()
        });
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(FixedDimEmbedder(384));
        let client = client_with_store_and_embedder(store, embedder, /* model_dim */ None);
        let err = client
            .guard_index_dim("code_chunks", "proj")
            .await
            .expect_err(
                "an unpinned local embedder reporting 384 must fail against a 768-d index — \
                 unwrap_or(index_dim) would wrongly treat this as a match",
            );
        assert_dim_guard_error(&err);
        let msg = format!("{err:#}");
        assert!(
            msg.contains("768") && msg.contains("384"),
            "error should name both the stored (768) and the embedder's real (384) dims, got: {msg}"
        );
    }

    /// Review round-2 C2: closes the coverage hole that let a real regression
    /// through. Every existing test of `effective_model_dim` either had the
    /// embedder answer `known_dim()` with `Some(_)`, or had `config.model_dim`
    /// pinned to `Some(_)` — so `.or(self.config.model_dim).unwrap_or(fallback)`
    /// never actually reached its `unwrap_or` arm, and a
    /// `.unwrap_or(fallback) -> .unwrap_or(0)` mutation survived undetected.
    /// This is the case where NEITHER the embedder nor the config pin can
    /// answer — a remote/HTTP backend (`known_dim()` is always `None`) with no
    /// `CODESCOUT_MODEL_DIM` set — so the caller's `fallback` must be the
    /// value that comes out.
    #[tokio::test]
    async fn effective_model_dim_falls_back_when_nothing_is_known() {
        let store = Arc::new(DimReportingStore::default());
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(EmbedderHttp::new(
            "http://unused.invalid",
            "http://unused.invalid",
            3,
        ));
        let client = client_with_store_and_embedder(store, embedder, /* model_dim */ None);
        assert_eq!(
            client.effective_model_dim(999),
            999,
            "with neither the embedder nor a config pin able to answer, the caller's \
             fallback must come through unchanged"
        );
    }
}
