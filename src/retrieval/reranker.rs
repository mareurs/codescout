use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct RerankerHttp {
    base: String,
    client: reqwest::Client,
    protocol: Protocol,
    model_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Protocol {
    Tei,
    Infinity,
}

impl Protocol {
    fn from_env() -> Self {
        match std::env::var("CODESCOUT_RERANKER_PROTOCOL")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "infinity" | "cohere" | "llama-server" | "llama_server" | "llamacpp" => Self::Infinity,
            _ => Self::Tei,
        }
    }
}

#[derive(Serialize)]
struct TeiRerankReq<'a> {
    query: &'a str,
    texts: &'a [String],
    raw_scores: bool,
}

#[derive(Deserialize)]
struct TeiRerankItem {
    index: usize,
    score: f32,
}

#[derive(Serialize)]
struct InfinityRerankReq<'a> {
    query: &'a str,
    documents: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
}

#[derive(Deserialize)]
struct InfinityRerankResp {
    results: Vec<InfinityRerankItem>,
}

#[derive(Deserialize)]
struct InfinityRerankItem {
    index: usize,
    relevance_score: f32,
}

impl RerankerHttp {
    pub fn new(base: impl Into<String>) -> Self {
        let protocol = Protocol::from_env();
        let model_id = std::env::var("CODESCOUT_RERANKER_MODEL").ok();
        Self::with_protocol(base, protocol, model_id)
            .with_read_timeout(crate::retrieval::transport::read_timeout_from_env())
    }

    /// Construct without reading process env vars.
    ///
    /// Use this from tests and any caller that wants explicit control over
    /// the reranker protocol and model id. `new()` is the env-reading
    /// convenience for production callers.
    pub fn with_protocol(
        base: impl Into<String>,
        protocol: Protocol,
        model_id: Option<String>,
    ) -> Self {
        crate::install_default_crypto_provider();
        Self {
            base: base.into(),
            client: crate::retrieval::transport::client(std::time::Duration::from_secs(
                crate::retrieval::transport::DEFAULT_READ_TIMEOUT_SECS,
            )),
            protocol,
            model_id,
        }
    }
    /// Rebuild the HTTP client with a different read timeout. Builder-style, and
    /// the exact counterpart of `EmbedderHttp::with_read_timeout` — the reranker
    /// leg had the same unbounded-wait defect, since `reqwest::Client::new()`
    /// sets no timeout of any kind.
    ///
    /// `new()` supplies the operator's `CODESCOUT_HTTP_READ_TIMEOUT_SECS` or the
    /// default; `with_protocol` stays free of ambient config so tests are not a
    /// function of the developer's shell.
    pub fn with_read_timeout(mut self, read_timeout: std::time::Duration) -> Self {
        self.client = crate::retrieval::transport::client(read_timeout);
        self
    }

    pub async fn rerank(&self, query: &str, texts: &[String]) -> Result<Vec<f32>> {
        let url = format!("{}/rerank", self.base);
        let mut scores = vec![0.0_f32; texts.len()];
        match self.protocol {
            Protocol::Tei => {
                let body = TeiRerankReq {
                    query,
                    texts,
                    raw_scores: false,
                };
                let items: Vec<TeiRerankItem> = self
                    .client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .context("rerank send")?
                    .error_for_status()
                    .context("rerank status")?
                    .json()
                    .await
                    .context("rerank json")?;
                for it in items {
                    if it.index < scores.len() {
                        scores[it.index] = it.score;
                    }
                }
            }
            Protocol::Infinity => {
                let body = InfinityRerankReq {
                    query,
                    documents: texts,
                    model: self.model_id.as_deref(),
                };
                let resp: InfinityRerankResp = self
                    .client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .context("rerank send")?
                    .error_for_status()
                    .context("rerank status")?
                    .json()
                    .await
                    .context("rerank json")?;
                for it in resp.results {
                    if it.index < scores.len() {
                        scores[it.index] = it.relevance_score;
                    }
                }
            }
        }
        Ok(scores)
    }
}
