use crate::retrieval::code_store::{CodeVectorStore, VectorBackend};
use crate::retrieval::config::RetrievalConfig;
use crate::retrieval::embedder::EmbedderHttp;
#[cfg(feature = "server-stack")]
use crate::retrieval::qdrant::QdrantWrap;
use crate::retrieval::reranker::RerankerHttp;
use anyhow::Result;
use std::sync::Arc;

pub struct RetrievalClient {
    /// Code-chunk vector store behind the `CodeVectorStore` seam. Qdrant today;
    /// in-process sqlite-vec in the lite stack (Phase 2). `pub(crate)` so the
    /// sibling `search`/`sync` modules can reach it without exposing it outside
    /// the crate. See `docs/plans/2026-06-16-two-stack-retrieval-lite.md`.
    pub(crate) code_store: Arc<dyn CodeVectorStore>,
    pub embedder: EmbedderHttp,
    pub reranker: RerankerHttp,
    pub config: RetrievalConfig,
    /// True for the daemon-free lite stack (sqlite-vec backend): dense-only, no
    /// reranker server. Gates the rerank step in `search_in`.
    pub(crate) lite: bool,
}

impl RetrievalClient {
    pub async fn from_env() -> Result<Self> {
        let config = RetrievalConfig::from_env()?;
        // Backend selection (server Qdrant vs daemon-free sqlite-vec lite stack).
        // sqlite-vec never touches the network — no Qdrant connect probe.
        let backend = VectorBackend::resolve();
        let lite = matches!(backend, VectorBackend::SqliteVec);
        let code_store: Arc<dyn CodeVectorStore> = match backend {
            VectorBackend::SqliteVec => {
                Arc::new(crate::retrieval::sqlite_code_store::SqliteVecCodeStore::from_env()?)
            }
            VectorBackend::Qdrant => Self::qdrant_code_store(&config).await?,
        };
        // The lite stack has no sparse server; also skip the sparse leg whenever
        // sparse is disabled (the vector isn't used → don't pay for it).
        let dense_only = lite || config.disable_sparse;
        let embedder = EmbedderHttp::new(
            &config.embedder_url,
            &config.sparse_embedder_url,
            config.model_dim,
        )
        .dense_only(dense_only);
        let reranker = RerankerHttp::new(&config.reranker_url);
        Ok(Self {
            code_store,
            embedder,
            reranker,
            config,
            lite,
        })
    }

    /// Build the Qdrant-backed code store (server stack).
    #[cfg(feature = "server-stack")]
    async fn qdrant_code_store(config: &RetrievalConfig) -> Result<Arc<dyn CodeVectorStore>> {
        Ok(Arc::new(QdrantWrap::connect(&config.qdrant_url).await?))
    }

    /// Lean build: Qdrant isn't compiled in, so a `qdrant` backend request is a
    /// configuration error pointing at the fix.
    #[cfg(not(feature = "server-stack"))]
    async fn qdrant_code_store(_config: &RetrievalConfig) -> Result<Arc<dyn CodeVectorStore>> {
        anyhow::bail!(
            "CODESCOUT_VECTOR_BACKEND=qdrant requires the `server-stack` build feature. \
             Rebuild with `--features server-stack`, or run the lean lite stack with \
             CODESCOUT_VECTOR_BACKEND=sqlite-vec."
        )
    }

    #[cfg(feature = "server-stack")]
    /// Constructs without connecting to Qdrant — for tests and config validation.
    /// Always the Qdrant (hybrid) shape; the lite stack is constructed via
    /// `from_env` with `CODESCOUT_VECTOR_BACKEND=sqlite-vec`.
    pub fn from_config_only(config: RetrievalConfig) -> Self {
        let embedder = EmbedderHttp::new(
            &config.embedder_url,
            &config.sparse_embedder_url,
            config.model_dim,
        );
        let reranker = RerankerHttp::new(&config.reranker_url);
        let client = qdrant_client::Qdrant::from_url(&config.qdrant_url)
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("invalid qdrant url");
        let code_store: Arc<dyn CodeVectorStore> = Arc::new(QdrantWrap { client });
        Self {
            code_store,
            embedder,
            reranker,
            config,
            lite: false,
        }
    }

    /// `(chunk_count, file_count)` for a project's code index. Delegates to the
    /// code store so external callers (index status, dashboard) don't reach into
    /// the concrete backend.
    pub async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)> {
        self.code_store
            .project_index_stats(collection, project_id)
            .await
    }
}
