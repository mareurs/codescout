//! Remote embedding via OpenAI-compatible HTTP API.
//!
//! Works with OpenAI, Ollama, LM Studio, and any other server that
//! implements the `/v1/embeddings` endpoint.

use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::{Embedder, Embedding};

pub struct RemoteEmbedder {
    client: Client,
    endpoint: String,
    model: String,
    api_key: Option<String>,
}

#[derive(Serialize)]
struct EmbedRequest<'a> {
    model: &'a str,
    input: &'a [&'a str],
}

#[derive(Deserialize)]
struct EmbedResponse {
    data: Vec<EmbedData>,
}

#[derive(Deserialize)]
struct EmbedData {
    embedding: Vec<f32>,
    index: usize,
}

impl RemoteEmbedder {
    pub fn openai(model: &str) -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENAI_API_KEY env var not set"))?;
        Ok(Self {
            client: Client::new(),
            endpoint: "https://api.openai.com/v1/embeddings".into(),
            model: model.to_string(),
            api_key: Some(api_key),
        })
    }

    pub fn ollama(model: &str) -> Result<Self> {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        Ok(Self {
            client: Client::new(),
            endpoint: format!("{}/v1/embeddings", host.trim_end_matches('/')),
            model: model.to_string(),
            api_key: None,
        })
    }

    pub fn custom(base_url: &str, model: &str) -> Result<Self> {
        let endpoint = format!("{}/v1/embeddings", base_url.trim_end_matches('/'));
        Ok(Self {
            client: Client::new(),
            endpoint,
            model: model.to_string(),
            api_key: std::env::var("EMBED_API_KEY").ok(),
        })
    }
}

#[async_trait::async_trait]
impl Embedder for RemoteEmbedder {
    fn dimensions(&self) -> usize {
        // Unknown at construction time for remote models; callers should
        // derive dimensions from the first response.
        0
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let mut req = self
            .client
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .json(&EmbedRequest {
                model: &self.model,
                input: texts,
            });

        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        let resp: EmbedResponse = req.send().await?.error_for_status()?.json().await?;

        // Sort by index to match input order
        let mut data = resp.data;
        data.sort_by_key(|d| d.index);
        Ok(data.into_iter().map(|d| d.embedding).collect())
    }
}
