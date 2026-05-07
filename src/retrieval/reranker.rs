use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub struct RerankerHttp {
    base: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct RerankReq<'a> {
    query: &'a str,
    texts: &'a [String],
    raw_scores: bool,
}

#[derive(Deserialize)]
struct RerankItem {
    index: usize,
    score: f32,
}

impl RerankerHttp {
    pub fn new(base: impl Into<String>) -> Self {
        Self {
            base: base.into(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn rerank(&self, query: &str, texts: &[String]) -> Result<Vec<f32>> {
        let url = format!("{}/rerank", self.base);
        let body = RerankReq {
            query,
            texts,
            raw_scores: false,
        };
        let items: Vec<RerankItem> = self
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
        let mut scores = vec![0.0_f32; texts.len()];
        for it in items {
            if it.index < scores.len() {
                scores[it.index] = it.score;
            }
        }
        Ok(scores)
    }
}
