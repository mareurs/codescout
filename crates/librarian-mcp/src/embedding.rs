use anyhow::Result;
use codescout_embed::Embedder;
use std::sync::Arc;

pub struct EmbeddingService {
    pub embedder: Arc<dyn Embedder>,
}

impl EmbeddingService {
    pub fn new(e: Arc<dyn Embedder>) -> Self {
        Self { embedder: e }
    }

    pub async fn embed_artifact(&self, title: Option<&str>, body: &str) -> Result<Vec<f32>> {
        let text = format!("{}\n\n{}", title.unwrap_or(""), body);
        self.embedder.embed_query(&text).await
    }
}
