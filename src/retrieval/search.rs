use anyhow::Result;

use crate::retrieval::client::RetrievalClient;

/// Options controlling search behaviour.
#[derive(Debug, Clone)]
pub struct SearchOpts {
    /// Number of final hits to return after reranking.
    pub limit: usize,
    /// Number of candidates fetched from Qdrant before reranking.
    pub overfetch: usize,
    /// Whether to apply the cross-encoder reranker. Degrades gracefully on
    /// reranker failure.
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
        let q = self.embedder.embed(query).await?;
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

        // Lite stack has no reranker server — skip the rerank step entirely.
        if !opts.rerank || self.lite || candidates.is_empty() {
            return Ok(candidates.into_iter().take(opts.limit).collect());
        }

        let texts: Vec<String> = candidates.iter().map(|h| h.content.clone()).collect();
        match self.reranker.rerank(query, &texts).await {
            Ok(scores) => {
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
