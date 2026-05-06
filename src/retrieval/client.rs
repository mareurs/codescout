use anyhow::Result;
use crate::retrieval::config::RetrievalConfig;
use crate::retrieval::embedder::EmbedderHttp;
use crate::retrieval::reranker::RerankerHttp;
use crate::retrieval::qdrant::QdrantWrap;

pub struct RetrievalClient {
    pub qdrant:   QdrantWrap,
    pub embedder: EmbedderHttp,
    pub reranker: RerankerHttp,
    pub config:   RetrievalConfig,
}

impl RetrievalClient {
    pub async fn from_env() -> Result<Self> {
        let config = RetrievalConfig::from_env()?;
        let qdrant = QdrantWrap::connect(&config.qdrant_url).await?;
        let embedder = EmbedderHttp::new(&config.embedder_url, &config.sparse_embedder_url, config.model_dim);
        let reranker = RerankerHttp::new(&config.reranker_url);
        Ok(Self { qdrant, embedder, reranker, config })
    }

    /// Constructs without connecting to Qdrant — for tests and config validation.
    pub fn from_config_only(config: RetrievalConfig) -> Self {
        let embedder = EmbedderHttp::new(&config.embedder_url, &config.sparse_embedder_url, config.model_dim);
        let reranker = RerankerHttp::new(&config.reranker_url);
        let client = qdrant_client::Qdrant::from_url(&config.qdrant_url).build()
            .expect("invalid qdrant url");
        let qdrant = QdrantWrap { client };
        Self { qdrant, embedder, reranker, config }
    }
}
