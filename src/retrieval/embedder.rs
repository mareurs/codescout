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

pub struct EmbedderHttp {
    dense_base: String,
    sparse_base: String,
    expected_dim: usize,
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
        let dense_model_name = std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").unwrap_or_default();
        let query_prefix = std::env::var("CODESCOUT_QUERY_PREFIX").unwrap_or_default();
        Self::with_config(
            dense_base,
            sparse_base,
            expected_dim,
            dense_model_name,
            query_prefix,
        )
    }

    /// Construct without reading process env vars.
    ///
    /// Use this from tests and any caller that wants explicit control over the
    /// dense model name and query prefix. `new()` is the env-reading convenience
    /// for production callers. Dense embedding is always OpenAI-compatible
    /// (`POST {base}/v1/embeddings`).
    pub fn with_config(
        dense_base: impl Into<String>,
        sparse_base: impl Into<String>,
        expected_dim: usize,
        dense_model_name: impl Into<String>,
        query_prefix: impl Into<String>,
    ) -> Self {
        crate::install_default_crypto_provider();
        Self {
            dense_base: dense_base.into(),
            sparse_base: sparse_base.into(),
            expected_dim,
            dense_model_name: dense_model_name.into(),
            query_prefix: query_prefix.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Send a dense-embedding batch to the OpenAI-compatible endpoint
    /// (`POST {base}/v1/embeddings`). Returns one vector per input, in input
    /// order. Works against any OpenAI-shape server — llama-server, vLLM, Ollama,
    /// OpenAI proper, or a corporate embedding gateway.
    async fn dense_batch(&self, inputs: &[&str]) -> Result<Vec<Vec<f32>>> {
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
    /// Dense-only query embedding: applies the configured `query_prefix` (if any)
    /// and hits ONLY the dense endpoint — no sparse leg. This is the path for
    /// dense-only retrieval (memory recall today; the sqlite-vec "lite" stack
    /// tomorrow), which never needs sparse terms. Distinct from [`Self::embed`],
    /// which also fetches the sparse vector for hybrid code search.
    pub async fn dense_query(&self, text: &str) -> Result<Vec<f32>> {
        let dense_text = if self.query_prefix.is_empty() {
            text.to_string()
        } else {
            format!("{}{}", self.query_prefix, text)
        };
        let dense = self
            .dense_batch(&[dense_text.as_str()])
            .await?
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
        Ok(dense)
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
                // Empty input is rejected by the sparse server (HTTP 400);
                // an empty chunk simply has no sparse terms.
                if text.is_empty() {
                    return Ok(Vec::<Vec<SparseEntry>>::new());
                }
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
        // The sparse (SPLADE/TEI) server caps client batches at 8
        // (HTTP 422 "batch size N > maximum allowed batch size 8"), so keep
        // both the dense and sparse legs at or below that limit.
        const BATCH: usize = 8;
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(BATCH) {
            let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
            // The sparse (SPLADE/TEI) server rejects empty strings with HTTP 400,
            // which would abort the whole batch. An empty chunk has no terms, so
            // omit it from the sparse request and re-expand to an empty vector at
            // its original position to stay aligned with the dense response.
            let nonempty: Vec<&str> = inputs.iter().copied().filter(|s| !s.is_empty()).collect();
            let sparse_body = serde_json::json!({ "inputs": &nonempty });

            let (dense_batch, sparse_nonempty) =
                tokio::try_join!(self.dense_batch(&inputs), async {
                    if nonempty.is_empty() {
                        return Ok(Vec::<Vec<SparseEntry>>::new());
                    }
                    let mut attempt: u32 = 0;
                    loop {
                        let resp = self
                            .client
                            .post(&sparse_url)
                            .json(&sparse_body)
                            .send()
                            .await
                            .context("embed_batch sparse send")?;
                        let status = resp.status();
                        if status.is_success() {
                            return resp
                                .json::<Vec<Vec<SparseEntry>>>()
                                .await
                                .context("embed_batch sparse json");
                        }
                        // The shared sparse server returns 424/429/5xx when momentarily
                        // overloaded by concurrent callers; retry those with backoff
                        // before surfacing a detailed error.
                        let code = status.as_u16();
                        let retryable = code == 424 || code == 429 || status.is_server_error();
                        attempt += 1;
                        if !retryable || attempt >= 8 {
                            let body = resp.text().await.unwrap_or_default();
                            return Err(anyhow!(
                                "embed_batch sparse status {} (inputs={}): {}",
                                status,
                                nonempty.len(),
                                body.chars().take(200).collect::<String>()
                            ));
                        }
                        let backoff =
                            std::time::Duration::from_millis(100u64 * (1u64 << attempt.min(6)));
                        tokio::time::sleep(backoff).await;
                    }
                })?;

            let mut sparse_nonempty = sparse_nonempty.into_iter();
            let sparse_resp: Vec<Vec<SparseEntry>> = inputs
                .iter()
                .map(|s| {
                    if s.is_empty() {
                        Vec::new()
                    } else {
                        sparse_nonempty.next().unwrap_or_default()
                    }
                })
                .collect();

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
        // Dense-only: no sparse leg. Memory recall (and the lite stack) rank on
        // the dense vector alone, so skip the sparse HTTP round-trip entirely.
        self.inner.dense_query(text).await
    }
}
