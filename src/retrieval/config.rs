use anyhow::Result;

pub struct RetrievalConfig {
    pub qdrant_url: String,
    pub embedder_url: String,
    pub sparse_embedder_url: String,
    pub reranker_url: String,
    pub model_dim: usize,
    pub profile: String,
    /// Multiplier for the sparse (BM25) prefetch candidate pool relative to dense.
    /// 1.0 = equal weight (default), 2.0 = BM25 gets 2× more candidates in RRF.
    pub bm25_boost: f32,
    /// Skip the sparse leg entirely. Search becomes pure dense ANN.
    /// Set via CODESCOUT_DISABLE_SPARSE=1 — used in matrix control cells.
    pub disable_sparse: bool,
    /// Prefix prepended to qdrant collection names. Default empty (live collections
    /// `code_chunks`, `memories`, etc.). Set via
    /// CODESCOUT_QDRANT_COLLECTION_PREFIX to isolate benchmark runs (e.g.
    /// `bench_jinav2_` → `bench_jinav2_code_chunks`).
    pub collection_prefix: String,
}

impl RetrievalConfig {
    /// Compose a per-instance collection name. With empty prefix this returns
    /// the canonical names (`code_chunks` etc.) preserving backwards compatibility.
    pub fn collection(&self, kind: &str) -> String {
        format!("{}{}", self.collection_prefix, kind)
    }

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url: std::env::var("CODESCOUT_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            sparse_embedder_url: std::env::var("CODESCOUT_SPARSE_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8084".into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8083".into()),
            model_dim: std::env::var("CODESCOUT_MODEL_DIM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(768),
            profile: std::env::var("CODESCOUT_RETRIEVAL_PROFILE").unwrap_or_else(|_| "cpu".into()),
            bm25_boost: std::env::var("CODESCOUT_BM25_BOOST")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3.0),
            disable_sparse: std::env::var("CODESCOUT_DISABLE_SPARSE")
                .ok()
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
            collection_prefix: std::env::var("CODESCOUT_QDRANT_COLLECTION_PREFIX")
                .unwrap_or_default(),
        })
    }
}
