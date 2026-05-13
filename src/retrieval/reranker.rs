use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct RerankerHttp {
    base: String,
    client: reqwest::Client,
    protocol: Protocol,
    model_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Protocol {
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
            "infinity" | "cohere" => Self::Infinity,
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
        crate::install_default_crypto_provider();
        let model_id = std::env::var("CODESCOUT_RERANKER_MODEL").ok();
        Self {
            base: base.into(),
            client: reqwest::Client::new(),
            protocol,
            model_id,
        }
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
