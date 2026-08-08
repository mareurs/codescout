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
}

impl SearchOpts {
    pub fn new(limit: usize) -> Self {
        Self {
            limit,
            overfetch: limit * 2,
            rerank: true,
            exclude_languages: Vec::new(),
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
    async fn search_in(
        &self,
        collection: &str,
        project_id: &str,
        query: &str,
        opts: SearchOpts,
    ) -> Result<Vec<Hit>> {
        let mut timer = crate::perf::PhaseTimer::start("semantic_search");
        let q = self.embedder.embed_one(query).await?;
        timer.lap("embed");
        let candidates = self
            .code_store
            .query(
                collection,
                project_id,
                &q.dense,
                &q.sparse,
                opts.overfetch,
                self.config.bm25_boost,
                self.config.disable_sparse,
                &opts.exclude_languages,
            )
            .await?;
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
        self.search_in(&self.config.collection("memories"), project_id, query, opts)
            .await
    }

    /// Search across all library chunks regardless of project.
    pub async fn search_libraries(&self, query: &str, opts: SearchOpts) -> Result<Vec<Hit>> {
        self.search_in(&self.config.collection("library_chunks"), "*", query, opts)
            .await
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
