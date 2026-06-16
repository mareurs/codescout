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

/// `true` when `url` is `https://…` or targets a loopback host. Mirrors the
/// codescout-embed `RemoteEmbedder` guard: keep local Ollama / llama.cpp working
/// while never sending `EMBED_API_KEY` over plaintext HTTP on the network.
fn is_https_or_loopback(url: &str) -> bool {
    if url.starts_with("https://") {
        return true;
    }
    let rest = match url.strip_prefix("http://") {
        Some(r) => r,
        None => return false,
    };
    // Parse the HOST out of `[userinfo@]host[:port][/path…]` and match it exactly.
    // An unanchored prefix check (`starts_with("127.")`/`starts_with("localhost")`)
    // would treat http://127.evil.com or http://localhost.evil.com as loopback and
    // leak EMBED_API_KEY over cleartext HTTP.
    let host_port = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(rest)
        .rsplit('@')
        .next()
        .unwrap_or(rest);
    let host = if let Some(v6) = host_port.strip_prefix('[') {
        v6.split(']').next().unwrap_or(v6) // IPv6 literal: [::1]:port
    } else {
        host_port.split(':').next().unwrap_or(host_port)
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .map(|ip| ip.is_loopback())
            .unwrap_or(false)
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
    /// Dense-only mode: skip the sparse HTTP leg entirely in `embed()` /
    /// `embed_batch()` and return an empty sparse vector. Set by the lite stack
    /// (sqlite-vec backend) and whenever sparse is disabled — no sparse server is
    /// required, and the wasted round-trip is avoided.
    dense_only: bool,
    /// Optional bearer token for the dense endpoint (`EMBED_API_KEY`). Needed for
    /// authenticated corporate / OpenAI gateways — the lite stack's typical
    /// remote embedder. Sent only on the dense (`/v1/embeddings`) leg.
    api_key: Option<String>,
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
        let dense_base = dense_base.into();
        let dense_model_name = std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").unwrap_or_default();
        let query_prefix = std::env::var("CODESCOUT_QUERY_PREFIX").unwrap_or_default();
        // Never transmit EMBED_API_KEY over plaintext HTTP (loopback exempt for
        // local llama.cpp / Ollama) — mirrors RemoteEmbedder's HTTPS guard.
        let api_key = std::env::var("EMBED_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|key| {
                if is_https_or_loopback(&dense_base) {
                    Some(key)
                } else {
                    tracing::warn!(
                        "EMBED_API_KEY is set but CODESCOUT_EMBEDDER_URL is not HTTPS or loopback; \
                         dropping the key so it is not sent in cleartext. Use an https:// endpoint."
                    );
                    None
                }
            });
        Self::with_config(
            dense_base,
            sparse_base,
            expected_dim,
            dense_model_name,
            query_prefix,
        )
        .api_key(api_key)
    }

    /// Construct without reading process env vars.
    ///
    /// Use this from tests and any caller that wants explicit control over the
    /// dense model name and query prefix. `new()` is the env-reading convenience
    /// for production callers. Dense embedding is always OpenAI-compatible
    /// (`POST {base}/v1/embeddings`). Sparse is enabled by default — chain
    /// [`Self::dense_only`] to disable it.
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
            dense_only: false,
            api_key: None,
            client: reqwest::Client::new(),
        }
    }
    /// Set the bearer token for the dense endpoint. Builder-style; `new()` reads
    /// it from `EMBED_API_KEY`. `None` sends no Authorization header.
    pub fn api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Enable/disable dense-only mode (no sparse leg). Builder-style; the lite
    /// stack sets this true so `embed()` / `embed_batch()` never call a sparse
    /// server. Default is false (hybrid).
    pub fn dense_only(mut self, dense_only: bool) -> Self {
        self.dense_only = dense_only;
        self
    }

    /// Send a dense-embedding batch to the OpenAI-compatible endpoint
    /// (`POST {base}/v1/embeddings`). Returns one vector per input, in input
    /// order. Works against any OpenAI-shape server — llama-server, vLLM, Ollama,
    /// OpenAI proper, or a corporate embedding gateway. Sends `Authorization:
    /// Bearer <key>` when an `api_key` is configured.
    async fn dense_batch(&self, inputs: &[&str]) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/v1/embeddings", self.dense_base);
        let body = OpenAiEmbedReq {
            input: inputs.to_vec(),
            model: &self.dense_model_name,
        };
        let mut req = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        let resp: OpenAiEmbedResp = req
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
        if self.dense_only {
            // Lite stack: dense vector only — no sparse server contacted.
            return Ok(EmbedOutput {
                dense: self.dense_query(text).await?,
                sparse: SparseVector {
                    indices: vec![],
                    values: vec![],
                },
            });
        }
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
        if self.dense_only {
            // Lite stack: dense vectors only — no sparse server contacted.
            const DENSE_BATCH: usize = 8;
            let mut out = Vec::with_capacity(texts.len());
            for chunk in texts.chunks(DENSE_BATCH) {
                let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
                for dense in self.dense_batch(&inputs).await? {
                    if dense.len() != self.expected_dim {
                        return Err(anyhow!(
                            "embed dim mismatch: got {}, expected {}",
                            dense.len(),
                            self.expected_dim
                        ));
                    }
                    out.push(EmbedOutput {
                        dense,
                        sparse: SparseVector {
                            indices: vec![],
                            values: vec![],
                        },
                    });
                }
            }
            return Ok(out);
        }
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
