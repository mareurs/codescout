use anyhow::Result;

/// Embedding vector — dimensions depend on the configured model
/// (e.g. 768 for jina-embeddings-v2-base-code, 384 for bge-small).
pub type Embedding = Vec<f32>;

/// The stable substring every [`EmbedError::Connect`] renders.
///
/// **Match on this constant, never on a literal of your own.** The producer used
/// to live in the consumer's own crate, where one `grep` found both sides; moving
/// it here promoted a substring agreement into a cross-crate contract with a test
/// on each side and nothing making the two fail together
/// (`resume-embedding-transport-stages-1-3:ET-5`). Importing the constant is what
/// removes the drift: change the wording here and every consumer follows in the
/// same compile.
///
/// Deliberately identical to the wording root's `EmbedderHttp` already emitted,
/// so operator-facing text and its regression tests keep working across the swap.
pub const CONNECT_FAILED_MARKER: &str = "embed connect failed";

/// Errors this crate publishes as a **contract**, not merely as prose.
///
/// Lives in this ungated module rather than in `remote`, because the consumer
/// that classifies these strings is itself ungated — a lean build with no HTTP
/// transport must still be able to name the marker it is looking for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmbedError {
    /// The embedding server could not be reached: connection refused, DNS
    /// failure, or a read timeout (the wedged-peer case, where the socket is
    /// accepted and nothing ever comes back).
    Connect {
        /// The endpoint that could not be reached. Operators need this — the
        /// commonest cause is a stale URL pointing at a port nothing serves.
        url: String,
        /// The underlying transport error, preserved verbatim.
        detail: String,
    },
}

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect { url, detail } => write!(
                f,
                "{CONNECT_FAILED_MARKER}: {url} — the embedding server is \
                 unreachable (connect/timeout). Check the configured embedder \
                 URL and that the server is running. ({detail})"
            ),
        }
    }
}

impl std::error::Error for EmbedError {}

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
