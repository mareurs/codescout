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

/// Batch (dense + sparse) embedding seam for the indexing path.
///
/// Mirrors [`DenseEmbedder`] but yields the full [`EmbedOutput`] that the code
/// indexer upserts. It exists so the streaming indexer
/// (`crate::retrieval::sync::stream_index`) can be unit-tested with a
/// deterministic fake instead of standing up the HTTP embed servers. Production
/// uses the impl on [`EmbedderHttp`], which forwards to its inherent
/// `embed_batch`.
#[async_trait::async_trait]
pub trait BatchEmbedder: Send + Sync {
    async fn embed_batch_dyn(&self, texts: &[String]) -> anyhow::Result<Vec<EmbedOutput>>;
}

#[async_trait::async_trait]
impl BatchEmbedder for EmbedderHttp {
    async fn embed_batch_dyn(&self, texts: &[String]) -> anyhow::Result<Vec<EmbedOutput>> {
        self.embed_batch(texts).await
    }
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
    /// Memoised sparse-server per-request cap. Resolved on first use because
    /// `EmbedderHttp::new` is synchronous and the probe is async.
    sparse_batch_cap: tokio::sync::OnceCell<usize>,
    /// Escape-hatch override for the discovered batch size (`CODESCOUT_EMBED_BATCH`).
    /// `new()` reads it from process env, exactly like `api_key`/`EMBED_API_KEY`;
    /// `with_config` defaults to `None`. Injectable via `with_batch_override` so
    /// tests never need to mutate real process env to exercise the override path.
    batch_override: Option<String>,
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
        // CODESCOUT_EMBED_BATCH: escape hatch for resolve_batch_size, read here
        // (not inside resolve_batch_size) so it's injectable data on the struct —
        // mirrors api_key/EMBED_API_KEY exactly. See with_batch_override.
        let batch_override = std::env::var("CODESCOUT_EMBED_BATCH").ok();
        Self::with_config(
            dense_base,
            sparse_base,
            expected_dim,
            dense_model_name,
            query_prefix,
        )
        .api_key(api_key)
        .with_batch_override(batch_override)
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
            sparse_batch_cap: tokio::sync::OnceCell::new(),
            batch_override: None,
        }
    }
    /// Set the bearer token for the dense endpoint. Builder-style; `new()` reads
    /// it from `EMBED_API_KEY`. `None` sends no Authorization header.
    pub fn api_key(mut self, api_key: Option<String>) -> Self {
        self.api_key = api_key;
        self
    }

    /// Inject the `CODESCOUT_EMBED_BATCH` override directly. Builder-style;
    /// `new()` reads it from process env, mirroring `api_key`. Tests use this to
    /// set (`Some("4".into())`) or explicitly clear (`None`) the override without
    /// ever mutating real process env — see `resolve_batch_size` and
    /// docs/issues/2026-07-27-embedder-batch-env-test-race-reintroduces-fixed-ub.md
    /// for why that matters (a prior thread-local-based version of this fix was
    /// superseded by this injected-field version on review).
    pub fn with_batch_override(mut self, batch_override: Option<String>) -> Self {
        self.batch_override = batch_override;
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
            .map_err(|e| {
                // A connect/timeout failure is client-side: the embedder URL is
                // wrong or the service is down. Lift the URL + a "connect" marker
                // to the top-level message so `e.to_string()` (what the search
                // layer's classifier sees) routes this to the embedder hint
                // instead of the misleading "check qdrant logs" fallback.
                if e.is_connect() || e.is_timeout() {
                    anyhow!(
                        "dense embed connect failed: {url} — the dense embedder is \
                         unreachable (connect/timeout). Check CODESCOUT_EMBEDDER_URL and \
                         that the embedder is running (`./scripts/retrieval-stack.sh ps`). ({e})"
                    )
                } else {
                    anyhow::Error::new(e).context("dense openai send")
                }
            })?
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

    /// Embed exactly one sub-batch: dense and sparse legs concurrently.
    ///
    /// Split out of `embed_batch` so the sub-batches can be pipelined (see the
    /// `buffered` driver there) and so this unit is testable on its own.
    async fn embed_one_batch(&self, chunk: Vec<String>) -> Result<Vec<EmbedOutput>> {
        let inputs: Vec<&str> = chunk.iter().map(String::as_str).collect();
        let sparse_url = format!("{}/embed_sparse", self.sparse_base);
        let mut out = Vec::with_capacity(inputs.len());
        // The sparse (SPLADE/TEI) server rejects empty strings with HTTP 400,
        // which would abort the whole batch. An empty chunk has no terms, so
        // omit it from the sparse request and re-expand to an empty vector at
        // its original position to stay aligned with the dense response.
        let nonempty: Vec<&str> = inputs.iter().copied().filter(|s| !s.is_empty()).collect();
        let sparse_body = serde_json::json!({ "inputs": &nonempty });

        let (dense_batch, sparse_nonempty) = tokio::try_join!(self.dense_batch(&inputs), async {
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
                let backoff = std::time::Duration::from_millis(100u64 * (1u64 << attempt.min(6)));
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
        Ok(out)
    }
    /// Per-request input count for both legs.
    ///
    /// `CODESCOUT_EMBED_BATCH` → the sparse server's advertised
    /// `max_client_batch_size` → 8. The 8 preserves the historical value for any
    /// server that does not answer `/info`.
    ///
    /// Discovered rather than hardcoded on purpose: the previous `const BATCH = 8`
    /// was justified by a comment citing a cap that only `sparse-amd` ever
    /// imposed, and it silently survived that service's removal.
    async fn resolve_batch_size(&self) -> usize {
        const FALLBACK: usize = 8;
        *self
            .sparse_batch_cap
            .get_or_init(|| async {
                if let Some(n) = self
                    .batch_override
                    .as_deref()
                    .and_then(|v| v.parse::<usize>().ok())
                    .filter(|&n| n > 0)
                {
                    tracing::info!(batch = n, source = "env", "embed batch size");
                    return n;
                }
                let url = format!("{}/info", self.sparse_base);
                let discovered = async {
                    let resp = self.client.get(&url).send().await.ok()?;
                    if !resp.status().is_success() {
                        return None;
                    }
                    let v: serde_json::Value = resp.json().await.ok()?;
                    v.get("max_client_batch_size")?.as_u64().map(|n| n as usize)
                }
                .await
                .filter(|&n| n > 0);

                match discovered {
                    Some(n) => {
                        tracing::info!(batch = n, source = "info", "embed batch size");
                        n
                    }
                    None => {
                        tracing::info!(batch = FALLBACK, source = "fallback", "embed batch size");
                        FALLBACK
                    }
                }
            })
            .await
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
        // The sparse (SPLADE/TEI) server caps client batches at a
        // server-advertised limit (HTTP 422 "batch size N > maximum allowed
        // batch size M" otherwise); discover it instead of hardcoding, so
        // keep both the dense and sparse legs at or below that limit.
        let batch = self.resolve_batch_size().await;
        let mut out = Vec::with_capacity(texts.len());
        for chunk in texts.chunks(batch) {
            out.extend(self.embed_one_batch(chunk.to_vec()).await?);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A dense connect failure must name the target URL and flag "connect" so
    /// the search-layer classifier routes it to the embedder hint (not qdrant).
    /// Port 1 refuses connections instantly, keeping the test hermetic.
    #[tokio::test]
    async fn dense_connect_failure_names_url_and_flags_connect() {
        let e = EmbedderHttp::with_config("http://127.0.0.1:1", "http://127.0.0.1:1", 768, "m", "");
        let err = e.dense_query("hello").await.unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("http://127.0.0.1:1"),
            "connect error must name the target URL; got: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("connect"),
            "must flag a connect failure so the classifier can route it; got: {msg}"
        );
    }
    /// Mid-chunk empties must not shift sparse vectors onto the wrong dense
    /// position — the re-expansion iterator has to skip exactly the empty
    /// slots and nothing else. Each non-empty input gets a uniquely
    /// identifiable sparse vector so a shifted alignment cannot be mistaken
    /// for the right answer; a length-only assertion would miss this.
    #[tokio::test]
    async fn mid_chunk_empty_strings_keep_sparse_alignment() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;
        let dense_mock = dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_body(
                r#"{"data":[
                    {"embedding":[0.0,0.0,0.0],"index":0},
                    {"embedding":[1.0,0.0,0.0],"index":1},
                    {"embedding":[2.0,0.0,0.0],"index":2},
                    {"embedding":[3.0,0.0,0.0],"index":3},
                    {"embedding":[4.0,0.0,0.0],"index":4}
                ]}"#,
            )
            .create_async()
            .await;
        // Three non-empty inputs ("a","b","c" at positions 0,2,4) each get a
        // distinct index/value pair.
        let sparse_mock = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(200)
            .with_body(
                r#"[[{"index":10,"value":0.1}],[{"index":20,"value":0.2}],[{"index":30,"value":0.3}]]"#,
            )
            .create_async()
            .await;

        let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
        let chunk = vec![
            "a".to_string(),
            "".to_string(),
            "b".to_string(),
            "".to_string(),
            "c".to_string(),
        ];
        let out = e.embed_one_batch(chunk).await.expect("embed_one_batch");

        assert_eq!(out.len(), 5);
        assert_eq!(out[0].sparse.indices, vec![10u32]);
        assert!((out[0].sparse.values[0] - 0.1_f32).abs() < 1e-6);
        assert!(
            out[1].sparse.indices.is_empty(),
            "position 1 was an empty input"
        );
        assert!(out[1].sparse.values.is_empty());
        assert_eq!(out[2].sparse.indices, vec![20u32]);
        assert!((out[2].sparse.values[0] - 0.2_f32).abs() < 1e-6);
        assert!(
            out[3].sparse.indices.is_empty(),
            "position 3 was an empty input"
        );
        assert!(out[3].sparse.values.is_empty());
        assert_eq!(out[4].sparse.indices, vec![30u32]);
        assert!((out[4].sparse.values[0] - 0.3_f32).abs() < 1e-6);
        // Dense stays aligned to the original position too, not just sparse.
        assert_eq!(out[0].dense, vec![0.0_f32, 0.0, 0.0]);
        assert_eq!(out[2].dense, vec![2.0_f32, 0.0, 0.0]);
        assert_eq!(out[4].dense, vec![4.0_f32, 0.0, 0.0]);

        dense_mock.assert_async().await;
        sparse_mock.assert_async().await;
    }

    /// An all-empty chunk must not send an empty-array sparse request — the
    /// real SPLADE/TEI server answers HTTP 400 to `{"inputs": []}`, which
    /// would abort the whole batch. This asserts zero sparse invocations,
    /// which is stronger than merely accepting a `[]` sparse response.
    #[tokio::test]
    async fn all_empty_chunk_sends_zero_sparse_requests() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;
        let dense_mock = dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_body(
                r#"{"data":[
                    {"embedding":[0.0,0.0,0.0],"index":0},
                    {"embedding":[0.0,0.0,0.0],"index":1},
                    {"embedding":[0.0,0.0,0.0],"index":2}
                ]}"#,
            )
            .expect(1)
            .create_async()
            .await;
        let sparse_mock = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(200)
            .with_body("[]")
            .expect(0)
            .create_async()
            .await;

        let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
        let chunk = vec!["".to_string(), "".to_string(), "".to_string()];
        let out = e.embed_one_batch(chunk).await.expect("embed_one_batch");

        assert_eq!(out.len(), 3);
        for o in &out {
            assert!(o.sparse.indices.is_empty());
            assert!(o.sparse.values.is_empty());
        }
        dense_mock.assert_async().await;
        sparse_mock.assert_async().await;
    }

    /// The sparse retry loop must actually retry a retryable status (429) and
    /// converge on success — not just "eventually errors". Kept fast by
    /// stubbing only two failures before success instead of exhausting the
    /// real 8-attempt cap (which would sleep through the full backoff ladder,
    /// ~19s). mockito serves same-route mocks in creation order, advancing to
    /// the next once the current one's `.expect(n)` hit count is satisfied,
    /// so this stubs exactly "429, 429, success" as the first three requests.
    ///
    /// NOTE: this does NOT exercise the exact `attempt >= 8` cap boundary —
    /// see the task report for why a fast unit test can't reach it without a
    /// production change.
    #[tokio::test]
    async fn sparse_429_retries_then_succeeds_within_a_few_attempts() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;
        let dense_mock = dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_body(r#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#)
            .create_async()
            .await;
        let fail_1 = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(429)
            .expect(1)
            .create_async()
            .await;
        let fail_2 = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(429)
            .expect(1)
            .create_async()
            .await;
        let success = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(200)
            .with_body(r#"[[{"index":1,"value":0.5}]]"#)
            .expect(1)
            .create_async()
            .await;

        let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
        let out = e
            .embed_one_batch(vec!["x".to_string()])
            .await
            .expect("embed_one_batch should succeed after retries");

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].sparse.indices, vec![1u32]);
        dense_mock.assert_async().await;
        fail_1.assert_async().await;
        fail_2.assert_async().await;
        success.assert_async().await;
    }

    /// The `retryable` boolean must gate correctly in both directions: a
    /// retryable 5xx retries and can still succeed, while a non-retryable
    /// 400 must fail on the very first attempt with no retry at all. Uses 500
    /// (the `is_server_error()` arm) rather than 429 (already covered by the
    /// retry-cap test above) to spread branch coverage across the `||`.
    #[tokio::test]
    async fn sparse_retryable_and_non_retryable_status_both_exercised() {
        // Retryable: 500 then success.
        {
            let mut dense_server = mockito::Server::new_async().await;
            let mut sparse_server = mockito::Server::new_async().await;
            dense_server
                .mock("POST", "/v1/embeddings")
                .with_status(200)
                .with_body(r#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#)
                .create_async()
                .await;
            let fail = sparse_server
                .mock("POST", "/embed_sparse")
                .with_status(500)
                .expect(1)
                .create_async()
                .await;
            let success = sparse_server
                .mock("POST", "/embed_sparse")
                .with_status(200)
                .with_body(r#"[[{"index":2,"value":0.7}]]"#)
                .expect(1)
                .create_async()
                .await;

            let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
            let out = e
                .embed_one_batch(vec!["x".to_string()])
                .await
                .expect("500 should retry then succeed");
            assert_eq!(out[0].sparse.indices, vec![2u32]);
            fail.assert_async().await;
            success.assert_async().await;
        }

        // Non-retryable: 400 must fail on the first attempt, no retry.
        {
            let mut dense_server = mockito::Server::new_async().await;
            let mut sparse_server = mockito::Server::new_async().await;
            dense_server
                .mock("POST", "/v1/embeddings")
                .with_status(200)
                .with_body(r#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#)
                .create_async()
                .await;
            let bad_request = sparse_server
                .mock("POST", "/embed_sparse")
                .with_status(400)
                .with_body("bad input")
                .expect(1)
                .create_async()
                .await;

            let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
            let err = e
                .embed_one_batch(vec!["y".to_string()])
                .await
                .expect_err("400 must not be retried into success");
            assert!(
                err.to_string().contains("400"),
                "error should surface the status: {err}"
            );
            bad_request.assert_async().await;
        }
    }

    /// `tokio::try_join!` must run the dense and sparse legs concurrently — a
    /// regression to sequential `.await`s produces byte-identical output
    /// values, so only wall-clock timing catches it.
    ///
    /// Equal delays are deliberate, not arbitrary: the separation between the
    /// concurrent and sequential cases is `sum` vs `max`, and that ratio
    /// (`sum/max`) is maximized (2×) when both delays are equal. Unequal
    /// delays throw headroom away — an earlier version used 300ms/600ms
    /// (only a 1.5× ratio) and measured just 4-6ms of margin on the
    /// sequential side (903.8-905.7ms against a 900ms ceiling) — a false
    /// negative waiting to happen on any machine a few ms faster. With both
    /// legs at 500ms and an 800ms ceiling, measured locally (5 runs each):
    ///   concurrent ≈ 502.8-503.6ms  → ~296-297ms below the ceiling
    ///   sequential ≈ 1004.8-1005.5ms → ~205ms above the ceiling
    /// Do not "optimize" these back down to unequal/smaller delays — that
    /// silently reintroduces the tight margin this comment exists to prevent.
    #[tokio::test]
    async fn dense_and_sparse_legs_run_concurrently() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;
        let dense_mock = dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_chunked_body(|w| {
                std::thread::sleep(std::time::Duration::from_millis(500));
                w.write_all(br#"{"data":[{"embedding":[0.1,0.2,0.3],"index":0}]}"#)
            })
            .create_async()
            .await;
        let sparse_mock = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(200)
            .with_chunked_body(|w| {
                std::thread::sleep(std::time::Duration::from_millis(500));
                w.write_all(br#"[[{"index":1,"value":0.5}]]"#)
            })
            .create_async()
            .await;

        let e = EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3);
        let start = std::time::Instant::now();
        let out = e
            .embed_one_batch(vec!["x".to_string()])
            .await
            .expect("embed_one_batch");
        let elapsed = start.elapsed();

        assert_eq!(out[0].dense, vec![0.1_f32, 0.2, 0.3]);
        assert_eq!(out[0].sparse.indices, vec![1u32]);
        assert!(
            elapsed < std::time::Duration::from_millis(800),
            "legs should run concurrently (~max(500,500)=500ms), took {elapsed:?} — \
                 a sequential-await regression would take ~1000ms"
        );
        assert!(
            elapsed >= std::time::Duration::from_millis(400),
            "delays should have actually elapsed (~500ms), took {elapsed:?} — a \
                 near-zero time means the mock delay did not run and this test is not \
                 exercising anything"
        );
        dense_mock.assert_async().await;
        sparse_mock.assert_async().await;
    }

    #[tokio::test]
    async fn batch_size_discovered_from_info() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"max_client_batch_size":32,"max_input_length":512}"#)
            .create_async()
            .await;

        let e =
            EmbedderHttp::new("http://unused.invalid", server.url(), 768).with_batch_override(None);
        assert_eq!(e.resolve_batch_size().await, 32);
    }

    /// The 404 body is a JSON envelope (not empty) so this test actually pins
    /// the `if !resp.status().is_success() { return None; }` guard in
    /// `resolve_batch_size` — deleting that guard would still parse
    /// `max_client_batch_size` successfully and yield 32, not 8, so the test
    /// would then fail. An empty 404 body would let a deleted guard hide
    /// behind a JSON-parse failure instead, which does not discriminate it.
    #[tokio::test]
    async fn batch_size_falls_back_to_8_when_info_missing() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/info")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(r#"{"max_client_batch_size":32}"#)
            .create_async()
            .await;

        let e =
            EmbedderHttp::new("http://unused.invalid", server.url(), 768).with_batch_override(None);
        assert_eq!(
            e.resolve_batch_size().await,
            8,
            "a non-TEI sparse server must keep today's behaviour"
        );
    }

    #[tokio::test]
    async fn env_override_wins_over_info() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_body(r#"{"max_client_batch_size":32}"#)
            .create_async()
            .await;

        let e = EmbedderHttp::new("http://unused.invalid", server.url(), 768)
            .with_batch_override(Some("4".to_string()));
        assert_eq!(e.resolve_batch_size().await, 4);
    }

    #[tokio::test]
    async fn batch_size_is_memoised() {
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/info")
            .with_status(200)
            .with_body(r#"{"max_client_batch_size":32}"#)
            .expect(1)
            .create_async()
            .await;

        let e =
            EmbedderHttp::new("http://unused.invalid", server.url(), 768).with_batch_override(None);
        assert_eq!(e.resolve_batch_size().await, 32);
        assert_eq!(e.resolve_batch_size().await, 32);
        m.assert_async().await; // exactly one /info request
    }

    /// Discovery *failure* must be memoised too, not just success — otherwise a
    /// `OnceCell` misuse that re-probes on every `embed_batch` call (e.g. storing
    /// `Option<usize>` and re-running on `None`) would pass every other test here
    /// while adding an HTTP round-trip per sub-batch in production against any
    /// sparse server that never answers `/info`.
    #[tokio::test]
    async fn batch_size_failure_is_memoised() {
        let mut server = mockito::Server::new_async().await;
        let m = server
            .mock("GET", "/info")
            .with_status(404)
            .expect(1)
            .create_async()
            .await;

        let e =
            EmbedderHttp::new("http://unused.invalid", server.url(), 768).with_batch_override(None);
        assert_eq!(e.resolve_batch_size().await, 8);
        assert_eq!(e.resolve_batch_size().await, 8);
        m.assert_async().await; // exactly one /info request despite two calls
    }

    /// End-to-end regression through `embed_batch` itself (not
    /// `resolve_batch_size` directly). The Task 3b hybrid tests above never
    /// mock `/info`, so they resolve to the `8` fallback either way and cannot
    /// catch a regression where `embed_batch` stops consulting the discovered
    /// value (e.g. a future edit reverting `let batch = self.resolve_batch_size().await;`
    /// back to a hardcoded `let batch = 8;`, or dropping it while rewriting
    /// this into a pipelined call in Task 5). 12 texts at the discovered batch
    /// (32) is one `/embed_sparse` request; at a hardcoded 8 it would be two —
    /// so the `.expect(1)` below fails loudly on that regression instead of
    /// silently staying green.
    #[tokio::test]
    async fn embed_batch_uses_discovered_batch_size_end_to_end() {
        let mut dense_server = mockito::Server::new_async().await;
        let mut sparse_server = mockito::Server::new_async().await;

        let _info_mock = sparse_server
            .mock("GET", "/info")
            .with_status(200)
            .with_body(r#"{"max_client_batch_size":32}"#)
            .create_async()
            .await;

        let dense_body = serde_json::json!({
            "data": (0..12u32)
                .map(|i| serde_json::json!({"embedding": [i as f32, 0.0, 0.0], "index": i}))
                .collect::<Vec<_>>()
        })
        .to_string();
        let dense_mock = dense_server
            .mock("POST", "/v1/embeddings")
            .with_status(200)
            .with_body(dense_body)
            .expect(1)
            .create_async()
            .await;

        let sparse_body = format!("[{}]", ["[]"; 12].join(","));
        let sparse_mock = sparse_server
            .mock("POST", "/embed_sparse")
            .with_status(200)
            .with_body(sparse_body)
            .expect(1)
            .create_async()
            .await;

        let e =
            EmbedderHttp::new(dense_server.url(), sparse_server.url(), 3).with_batch_override(None);
        let texts: Vec<String> = (0..12).map(|i| format!("text-{i}")).collect();
        let out = e.embed_batch(&texts).await.expect("embed_batch");

        assert_eq!(out.len(), 12);
        dense_mock.assert_async().await;
        sparse_mock.assert_async().await; // exactly one /embed_sparse request — batch 32, not 8
    }
}
