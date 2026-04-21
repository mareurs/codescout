use anyhow::Result;

/// Embedding vector — dimensions depend on the configured model
/// (e.g. 768 for jina-embeddings-v2-base-code, 384 for bge-small).
pub type Embedding = Vec<f32>;

/// Trait implemented by all embedding backends.
#[async_trait::async_trait]
pub trait Embedder: Send + Sync {
    /// Return the dimensionality of the produced vectors.
    fn dimensions(&self) -> usize;

    /// Embed a batch of texts, returning one vector per text.
    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>>;

    /// Embed a single query text.
    ///
    /// Override to apply model-specific query prefixes (e.g. CodeRankEmbed).
    /// Default implementation delegates to `embed` with no prefix.
    async fn embed_query(&self, text: &str) -> Result<Embedding> {
        let mut batch = self.embed(&[text]).await?;
        batch
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Embedder returned empty batch"))
    }
}
