use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct SparseVector {
    pub indices: Vec<u32>,
    pub values: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct EmbedOutput {
    pub dense: Vec<f32>,
    pub sparse: SparseVector,
}

/// Wire format for the dense leg.
///
/// * `Tei` — Hugging Face TEI native: `POST {base}/embed` with `{"inputs":[...]}`,
///   response `[[f32], ...]`.
/// * `OpenAi` — OpenAI-compatible (llama-server, vLLM, OpenAI proper):
///   `POST {base}/v1/embeddings` with `{"input":[...],"model":"..."}`,
///   response `{"data":[{"embedding":[...],"index":N},...]}`.
///
/// Selected via `CODESCOUT_EMBEDDER_PROTOCOL` env var (default `tei`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenseProtocol {
    Tei,
    OpenAi,
}

pub struct EmbedderHttp {
    dense_base: String,
    sparse_base: String,
    expected_dim: usize,
    dense_protocol: DenseProtocol,
    dense_model_name: String,
    /// Optional prefix prepended to the dense query text in `embed()` (search side).
    /// Doc-side `embed_batch()` is unaffected. Configure via `CODESCOUT_QUERY_PREFIX` —
    /// e.g. `Represent this query for searching relevant code: ` for CodeRankEmbed.
    query_prefix: String,
    client: reqwest::Client,
}

#[derive(Serialize)]
struct EmbedReq<'a> {
    inputs: Vec<&'a str>,
}

#[derive(Serialize)]
struct OpenAiEmbedReq<'a> {
    input: Vec<&'a str>,
    model: &'a str,
}

#[derive(Deserialize)]
struct OpenAiEmbedResp {
    data: Vec<OpenAiEmbedItem>,
}

#[derive(Deserialize)]
struct OpenAiEmbedItem {
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Deserialize)]
struct SparseEntry {
    index: u32,
    value: f32,
}

impl EmbedderHttp {
    pub fn new(
        dense_base: impl Into<String>,
        sparse_base: impl Into<String>,
        expected_dim: usize,
    ) -> Self {
        let dense_protocol = match std::env::var("CODESCOUT_EMBEDDER_PROTOCOL")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "openai" | "llama-server" | "llama_server" | "llamacpp" => DenseProtocol::OpenAi,
            _ => DenseProtocol::Tei,
        };
        let dense_model_name = std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").unwrap_or_default();
        let query_prefix = std::env::var("CODESCOUT_QUERY_PREFIX").unwrap_or_default();
        Self::with_protocol(
            dense_base,
            sparse_base,
            expected_dim,
            dense_protocol,
            dense_model_name,
            query_prefix,
        )
    }

    /// Construct without reading process env vars.
    ///
    /// Use this from tests and any caller that wants explicit control over
    /// the dense protocol, model name, and query prefix. `new()` is the env-
    /// reading convenience for production callers.
    pub fn with_protocol(
        dense_base: impl Into<String>,
        sparse_base: impl Into<String>,
        expected_dim: usize,
        dense_protocol: DenseProtocol,
        dense_model_name: impl Into<String>,
        query_prefix: impl Into<String>,
    ) -> Self {
        crate::install_default_crypto_provider();
        Self {
            dense_base: dense_base.into(),
            sparse_base: sparse_base.into(),
            expected_dim,
            dense_protocol,
            dense_model_name: dense_model_name.into(),
            query_prefix: query_prefix.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Send a dense-embedding batch using the configured protocol.
    /// Returns one vector per input, in the same order.
    async fn dense_batch(&self, inputs: &[&str]) -> Result<Vec<Vec<f32>>> {
        match self.dense_protocol {
            DenseProtocol::Tei => {
                let url = format!("{}/embed", self.dense_base);
                let body = serde_json::json!({ "inputs": inputs });
                let resp: Vec<Vec<f32>> = self
                    .client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .context("dense tei send")?
                    .error_for_status()
                    .context("dense tei status")?
                    .json()
                    .await
                    .context("dense tei json")?;
                Ok(resp)
            }
            DenseProtocol::OpenAi => {
                let url = format!("{}/v1/embeddings", self.dense_base);
                let body = OpenAiEmbedReq {
                    input: inputs.to_vec(),
                    model: &self.dense_model_name,
                };
                let resp: OpenAiEmbedResp = self
                    .client
                    .post(&url)
                    .json(&body)
                    .send()
                    .await
                    .context("dense openai send")?
                    .error_for_status()
                    .context("dense openai status")?
                    .json()
                    .await
                    .context("dense openai json")?;
                let mut items = resp.data;
                items.sort_by_key(|i| i.index);
                Ok(items.into_iter().map(|i| i.embedding).collect())
            }
        }
    }

    pub async fn embed(&self, text: &str) -> Result<EmbedOutput> {
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let sparse_body = EmbedReq { inputs: vec![text] };
        // Dense side may carry an asymmetric query prefix (e.g. CodeRankEmbed's
        // "Represent this query for searching relevant code: "). Sparse SPLADE
        // operates on raw tokens — leave it un-prefixed.
        let dense_text = if self.query_prefix.is_empty() {
            text.to_string()
        } else {
            format!("{}{}", self.query_prefix, text)
        };
        let dense_inputs = [dense_text.as_str()];

        let (dense_batch, sparse_resp) =
            tokio::try_join!(self.dense_batch(&dense_inputs), async {
                self.client
                    .post(&sparse_url)
                    .json(&sparse_body)
                    .send()
                    .await
                    .context("embed sparse")?
                    .error_for_status()
                    .context("embed sparse status")?
                    .json::<Vec<Vec<SparseEntry>>>()
                    .await
                    .context("embed sparse json")
            })?;

        let dense = dense_batch
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("empty dense response"))?;
        if dense.len() != self.expected_dim {
            return Err(anyhow!(
                "embed dim mismatch: got {}, expected {}",
                dense.len(),
                self.expected_dim
            ));
        }
        let sparse_vec = sparse_resp.into_iter().next().unwrap_or_default();
        let (indices, values): (Vec<u32>, Vec<f32>) =
            sparse_vec.into_iter().map(|e| (e.index, e.value)).unzip();
        Ok(EmbedOutput {
            dense,
            sparse: SparseVector { indices, values },
        })
    }

    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<EmbedOutput>> {
        const BATCH: usize = 32;
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
            let sparse_body = serde_json::json!({ "inputs": &inputs });

            let (dense_batch, sparse_resp) = tokio::try_join!(self.dense_batch(&inputs), async {
                self.client
                    .post(&sparse_url)
                    .json(&sparse_body)
                    .send()
                    .await
                    .context("embed_batch sparse send")?
                    .error_for_status()
                    .context("embed_batch sparse status")?
                    .json::<Vec<Vec<SparseEntry>>>()
                    .await
                    .context("embed_batch sparse json")
            })?;

            for (dense, sparse_vec) in dense_batch.into_iter().zip(sparse_resp) {
                if dense.len() != self.expected_dim {
                    return Err(anyhow!(
                        "embed dim mismatch: got {}, expected {}",
                        dense.len(),
                        self.expected_dim
                    ));
                }
                let (indices, values): (Vec<u32>, Vec<f32>) =
                    sparse_vec.into_iter().map(|e| (e.index, e.value)).unzip();
                out.push(EmbedOutput {
                    dense,
                    sparse: SparseVector { indices, values },
                });
            }
        }
        Ok(out)
    }
}

/// Storage-agnostic embedding contract.
///
/// Distinct from [`EmbedderHttp`] (which returns dense + sparse) — many
/// downstream paths (memory tool, migration, semantic-anchor creation)
/// only consume the dense vector. The trait isolates that subset so:
///
/// 1. Tests can swap in a deterministic fake without standing up the HTTP
///    retrieval stack (see [`Agent::set_memory_embedder_for_test`]).
/// 2. Production callers depend on a small, stable surface — the broader
///    `EmbedderHttp` API can grow without affecting them.
///
/// All implementations must be `Send + Sync` because the Agent stashes
/// them in a [`tokio::sync::OnceCell`] shared across tool calls.
#[async_trait::async_trait]
pub trait DenseEmbedder: Send + Sync {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>>;
}

/// Production [`DenseEmbedder`] backed by the HTTP retrieval stack.
/// Drops the sparse vector and surfaces only the dense one.
pub struct HttpDenseEmbedder {
    inner: EmbedderHttp,
}

impl HttpDenseEmbedder {
    pub fn new(inner: EmbedderHttp) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl DenseEmbedder for HttpDenseEmbedder {
    async fn embed(&self, text: &str) -> anyhow::Result<Vec<f32>> {
        Ok(self.inner.embed(text).await?.dense)
    }
}
