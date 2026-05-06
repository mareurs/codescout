use anyhow::Result;

#[derive(Debug, Clone)]
pub struct RetrievalConfig {
    pub qdrant_url: String,
    pub embedder_url: String,
    pub reranker_url: String,
    pub model_dim: usize,
    pub profile: String,
}

impl RetrievalConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant_url:   std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6333".into()),
            embedder_url: std::env::var("CODESCOUT_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080".into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            model_dim:    std::env::var("CODESCOUT_MODEL_DIM")
                .ok().and_then(|s| s.parse().ok()).unwrap_or(1024),
            profile:      std::env::var("CODESCOUT_RETRIEVAL_PROFILE")
                .unwrap_or_else(|_| "cpu".into()),
        })
    }
}
