use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values:  Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct EmbedOutput {
    pub dense:  Vec<f32>,
    pub sparse: SparseVector,
}

pub struct EmbedderHttp {
    dense_base: String,
    sparse_base: String,
    expected_dim: usize,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbedReq<'a> { inputs: Vec<&'a str> }

#[derive(Deserialize)]
struct SparseEntry { index: u32, value: f32 }

impl EmbedderHttp {
    pub fn new(
        dense_base: impl Into<String>,
        sparse_base: impl Into<String>,
        expected_dim: usize,
    ) -> Self {
        Self {
            dense_base: dense_base.into(),
            sparse_base: sparse_base.into(),
            expected_dim,
            client: reqwest::Client::new(),
        }
    }

    pub async fn embed(&self, text: &str) -> Result<EmbedOutput> {
        let dense_url  = format!("{}/embed", self.dense_base);
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let body = EmbedReq { inputs: vec![text] };

        let dense: Vec<Vec<f32>> = self.client.post(&dense_url).json(&body)
            .send().await.context("embed dense")?
            .error_for_status().context("embed dense status")?
            .json().await.context("embed dense json")?;
        let dense = dense.into_iter().next()
            .ok_or_else(|| anyhow!("empty dense response"))?;
        if dense.len() != self.expected_dim {
            return Err(anyhow!("embed dim mismatch: got {}, expected {}",
                dense.len(), self.expected_dim));
        }

        let sparse: Vec<Vec<SparseEntry>> = self.client.post(&sparse_url).json(&body)
            .send().await.context("embed sparse")?
            .error_for_status().context("embed sparse status")?
            .json().await.context("embed sparse json")?;
        let sparse_vec = sparse.into_iter().next().unwrap_or_default();
        let (indices, values): (Vec<u32>, Vec<f32>) = sparse_vec.into_iter()
            .map(|e| (e.index, e.value))
            .unzip();

        Ok(EmbedOutput { dense, sparse: SparseVector { indices, values } })
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
        const BATCH: usize = 32;
        let dense_url  = format!("{}/embed", self.dense_base);
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
            let body = serde_json::json!({ "inputs": inputs });

            let dense_fut = self.client
                .post(&dense_url).json(&body)
                .send();
            let sparse_fut = self.client
                .post(&sparse_url).json(&body)
                .send();

            let (dense_resp, sparse_resp) = tokio::try_join!(dense_fut, sparse_fut)
                .context("embed_batch send")?;

            let dense_batch: Vec<Vec<f32>> = dense_resp
                .error_for_status().context("embed_batch dense status")?
                .json().await.context("embed_batch dense json")?;
            let sparse_batch: Vec<Vec<SparseEntry>> = sparse_resp
                .error_for_status().context("embed_batch sparse status")?
                .json().await.context("embed_batch sparse json")?;

            for (dense, sparse_vec) in dense_batch.into_iter().zip(sparse_batch) {
                if dense.len() != self.expected_dim {
                    return Err(anyhow!(
                        "embed dim mismatch: got {}, expected {}",
                        dense.len(), self.expected_dim
                    ));
                }
                let (indices, values): (Vec<u32>, Vec<f32>) = sparse_vec
                    .into_iter()
                    .map(|e| (e.index, e.value))
                    .unzip();
                out.push(EmbedOutput { dense, sparse: SparseVector { indices, values } });
            }
        }
        Ok(out)
    }
}
