//! Remote embedding via OpenAI-compatible HTTP API.
//!
//! Works with OpenAI, Ollama, LM Studio, and any other server that
//! implements the `/v1/embeddings` endpoint.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use anyhow::{bail, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{Embedder, Embedding};

pub struct RemoteEmbedder {
    client: Client,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    /// Cached embedding dimensionality. Zero until the first successful `embed()` call,
    /// after which it is set to the length of the returned vectors. Using `Arc<AtomicUsize>`
    /// so clones of this embedder share the cached value.
    cached_dims: Arc<AtomicUsize>,
    /// Query prefix for asymmetric models (e.g. CodeRankEmbed). `None` for most models.
    query_prefix: Option<String>,
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

/// `true` when `url` is `https://…` or targets a loopback host (`localhost`,
/// `127.0.0.1`, `[::1]`). Used by `from_url` / `custom` to keep local Ollama
/// setups working while rejecting API keys over plaintext HTTP on the network.
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
    // leak the API key over cleartext HTTP.
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

impl RemoteEmbedder {
    /// Install rustls' ring crypto provider as the default. Idempotent — safe
    /// to call from multiple entry points. Required because reqwest uses
    /// `rustls-no-provider`: callers must install a provider before the first
    /// TLS handshake.
    fn install_default_crypto_provider() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            let _ = rustls::crypto::ring::default_provider().install_default();
        });
    }

    /// Build a reqwest client with a per-request timeout so that a hung
    /// embedding server (e.g. Ollama during GPU discovery failure) doesn't
    /// block `index_project` forever.
    fn http_client() -> Client {
        Self::install_default_crypto_provider();
        Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client")
    }

    /// Returns the query prefix for models that require asymmetric embedding.
    /// Currently only CodeRankEmbed models need a prefix on query side.
    fn query_prefix_for(model: &str) -> Option<String> {
        if model.to_lowercase().contains("coderank") {
            Some("Represent this query for searching relevant code: ".into())
        } else {
            None
        }
    }

    pub fn openai(model: &str, api_key: Option<String>) -> Result<Self> {
        let api_key = api_key
            .or_else(|| std::env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "OpenAI API key not found. Set api_key in [embeddings] or OPENAI_API_KEY env var"
                )
            })?;
        Ok(Self {
            client: Self::http_client(),
            endpoint: "https://api.openai.com/v1/embeddings".into(),
            model: model.to_string(),
            api_key: Some(api_key),
            cached_dims: Arc::new(AtomicUsize::new(0)),
            query_prefix: Self::query_prefix_for(model),
        })
    }

    pub fn ollama(model: &str) -> Result<Self> {
        let host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".into());
        Ok(Self {
            client: Self::http_client(),
            endpoint: format!("{}/v1/embeddings", host.trim_end_matches('/')),
            model: model.to_string(),
            api_key: None,
            cached_dims: Arc::new(AtomicUsize::new(0)),
            query_prefix: Self::query_prefix_for(model),
        })
    }

    pub fn custom(base_url: &str, model: &str) -> Result<Self> {
        let endpoint = format!("{}/v1/embeddings", base_url.trim_end_matches('/'));
        let api_key = std::env::var("EMBED_API_KEY").ok();
        if api_key.is_some() && !base_url.starts_with("https://") {
            bail!(
                "HTTPS required when EMBED_API_KEY is set — \
                 refusing to send API key over plaintext HTTP to {}",
                base_url
            );
        }
        Ok(Self {
            client: Self::http_client(),
            endpoint,
            model: model.to_string(),
            api_key,
            cached_dims: Arc::new(AtomicUsize::new(0)),
            query_prefix: Self::query_prefix_for(model),
        })
    }

    /// Create an embedder from an explicit URL.
    ///
    /// Normalizes the URL to always end with `/v1/embeddings`:
    /// - `http://host:port`               → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1`            → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1/embeddings` → `http://host:port/v1/embeddings`
    ///
    /// Rejects plaintext HTTP when an `api_key` is supplied (from argument or
    /// the `EMBED_API_KEY` env var). Loopback hosts (`localhost`, `127.0.0.1`,
    /// `[::1]`) are permitted to support local Ollama / llama.cpp setups where
    /// the key is only meaningful as a request-shape parameter.
    pub fn from_url(url: &str, model: &str, api_key: Option<String>) -> Result<Self> {
        let base = url.trim_end_matches('/');
        let endpoint = if base.ends_with("/v1/embeddings") {
            base.to_string()
        } else if base.ends_with("/v1") {
            format!("{}/embeddings", base)
        } else {
            format!("{}/v1/embeddings", base)
        };

        let api_key = api_key.or_else(|| std::env::var("EMBED_API_KEY").ok());

        if api_key.is_some() && !is_https_or_loopback(url) {
            bail!(
                "HTTPS required when api_key is set — \
                 refusing to send API key over plaintext HTTP to {}",
                url
            );
        }

        Ok(Self {
            client: Self::http_client(),
            endpoint,
            model: model.to_string(),
            api_key,
            cached_dims: Arc::new(AtomicUsize::new(0)),
            query_prefix: Self::query_prefix_for(model),
        })
    }
}

#[async_trait::async_trait]
impl Embedder for RemoteEmbedder {
    fn dimensions(&self) -> usize {
        // Returns 0 until the first successful `embed()` call populates the cache.
        // Callers that need a guaranteed non-zero value should embed a sample text first,
        // or test for 0 and treat it as "unknown" (see index.rs force-rebuild path).
        self.cached_dims.load(Ordering::Relaxed)
    }

    async fn embed(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        // Filter empty/whitespace-only strings — embedding servers reject them with 400.
        let non_empty: Vec<(usize, &str)> = texts
            .iter()
            .enumerate()
            .filter(|(_, t)| !t.trim().is_empty())
            .map(|(i, t)| (i, *t))
            .collect();
        if non_empty.is_empty() {
            // Pre-fix path returned `vec![vec![0.0; 1]; texts.len()]` here — a
            // 1-element sentinel vector that did not match the model's real
            // dim and silently corrupted the vec0 INSERT downstream (see
            // 2026-05-17-reindex-embedding-dim-mismatch.md). Surface the
            // condition instead so callers filter empties before calling.
            bail!(
                "cannot embed batch — all {} text(s) are empty/whitespace; \
                 filter empty inputs before calling embed()",
                texts.len()
            );
        }
        let filtered: Vec<&str> = non_empty.iter().map(|(_, t)| *t).collect();

        const BATCH_SIZE: usize = 32;
        const MAX_RETRIES: usize = 3;
        const INITIAL_BACKOFF_MS: u64 = 500;

        let mut embedded = Vec::with_capacity(filtered.len());
        for batch in filtered.chunks(BATCH_SIZE) {
            let mut last_err: Option<anyhow::Error> = None;
            let mut backoff_ms = INITIAL_BACKOFF_MS;
            let resp_data = 'retry: {
                for attempt in 0..MAX_RETRIES {
                    if attempt > 0 {
                        tokio::time::sleep(tokio::time::Duration::from_millis(backoff_ms)).await;
                        backoff_ms *= 2;
                    }
                    let mut req = self
                        .client
                        .post(&self.endpoint)
                        .header("Content-Type", "application/json")
                        .json(&EmbedRequest {
                            model: &self.model,
                            input: batch,
                        });
                    if let Some(key) = &self.api_key {
                        req = req.bearer_auth(key);
                    }
                    let resp = match req.send().await {
                        Ok(r) => r,
                        Err(e) => {
                            last_err = Some(anyhow::anyhow!(e));
                            continue;
                        }
                    };
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        if status.is_server_error() {
                            last_err = Some(anyhow::anyhow!(
                                "HTTP {status} from embedding server: {body}"
                            ));
                            continue;
                        }
                        // 4xx — bad request, wrong model, etc. — don't retry.
                        bail!("HTTP {status} from embedding server: {body}");
                    }
                    // Cap the response body at 32 MiB before json-decode. A
                    // hostile or misconfigured endpoint can otherwise stream
                    // gigabytes into memory — the 300s per-request timeout
                    // bounds duration, not bytes.
                    const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
                    let body_bytes = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            last_err = Some(anyhow::anyhow!(e));
                            continue;
                        }
                    };
                    if body_bytes.len() > MAX_RESPONSE_BYTES {
                        bail!(
                            "embedding response {} bytes exceeds {}-byte cap",
                            body_bytes.len(),
                            MAX_RESPONSE_BYTES
                        );
                    }
                    break 'retry serde_json::from_slice::<EmbedResponse>(&body_bytes)?;
                }
                return Err(last_err.unwrap_or_else(|| {
                    anyhow::anyhow!("embedding server unavailable after {MAX_RETRIES} attempts")
                }));
            };
            let mut data = resp_data.data;
            data.sort_by_key(|d| d.index);
            embedded.extend(data.into_iter().map(|d| d.embedding));
        }

        // Reconstruct: filtered embeddings in original positions, zeros for empty inputs.
        // If `embedded` is empty here, the server returned 200 with no data — refuse
        // rather than fall back to a 1-element dim sentinel that would corrupt the
        // vec0 INSERT downstream (2026-05-17-reindex-embedding-dim-mismatch).
        let dim = match embedded.first() {
            Some(first) => first.len(),
            None => {
                let cached = self.cached_dims.load(Ordering::Relaxed);
                if cached == 0 {
                    bail!(
                        "embedding server returned no data and no cached dimensions \
                         are available — cannot determine vector size"
                    );
                }
                cached
            }
        };

        // Cache dimensions on first successful embed so dimensions() returns a real value.
        if self.cached_dims.load(Ordering::Relaxed) == 0 && dim > 0 {
            self.cached_dims.store(dim, Ordering::Relaxed);
        }

        let mut all = vec![vec![0.0; dim]; texts.len()];
        for (slot, (orig_idx, _)) in non_empty.iter().enumerate() {
            all[*orig_idx] = std::mem::take(&mut embedded[slot]);
        }
        Ok(all)
    }

    async fn embed_query(&self, text: &str) -> Result<Embedding> {
        let prefixed;
        let input: &str = if let Some(prefix) = &self.query_prefix {
            prefixed = format!("{}{}", prefix, text);
            &prefixed
        } else {
            text
        };
        let mut batch = self.embed(&[input]).await?;
        batch
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Embedder returned empty batch"))
    }
}

/// Probe whether the Ollama daemon is reachable at the given host URL.
///
/// Issues a GET to the Ollama root with a 2-second timeout. Used by
/// `create_embedder` to detect when Ollama is absent and fall back to a
/// local CPU model. Returns `Ok(())` on any HTTP response (even 4xx/5xx —
/// the daemon is at least up), or an error if the connection is refused or
/// times out.
pub async fn probe_ollama(host: &str) -> anyhow::Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()?;
    client
        .get(host.trim_end_matches('/'))
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Ollama not reachable at {}: {}", host, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "nomic-embed-text";

    fn make_embedder() -> RemoteEmbedder {
        RemoteEmbedder::ollama(MODEL).unwrap()
    }

    async fn embed_one(text: &str) -> Vec<f32> {
        let mut results = make_embedder().embed(&[text]).await.expect("embed failed");
        results.pop().expect("empty response")
    }

    #[tokio::test]
    #[ignore = "requires running Ollama"]
    async fn ollama_returns_nonzero_dimensions() {
        let vec = embed_one("fn main() {}").await;
        assert!(!vec.is_empty(), "embedding should be non-empty");
        assert!(
            vec.iter().any(|&v| v != 0.0),
            "embedding should be non-zero"
        );
    }

    #[tokio::test]
    #[ignore = "requires running Ollama"]
    async fn ollama_batch_consistent_dimensions() {
        let embedder = make_embedder();
        let texts = &["fn main() {}", "struct Config {}", "impl Foo for Bar {}"];
        let results = embedder.embed(texts).await.expect("embed failed");
        assert_eq!(results.len(), texts.len(), "one vector per input");
        let dims = results[0].len();
        assert!(dims > 0);
        assert!(
            results.iter().all(|v| v.len() == dims),
            "all vectors same dims"
        );
    }

    #[tokio::test]
    #[ignore = "requires running Ollama"]
    async fn ollama_different_texts_produce_different_vectors() {
        let a = embed_one("fn authenticate_user(password: &str) -> bool").await;
        let b = embed_one("SELECT * FROM orders WHERE status = 'pending'").await;
        let l1_diff: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y).abs()).sum();
        assert!(
            l1_diff > 1.0,
            "distinct texts should produce distinct embeddings (diff={l1_diff:.3})"
        );
    }

    #[tokio::test]
    #[ignore = "requires running Ollama"]
    async fn ollama_similar_texts_score_higher_than_unrelated() {
        fn cosine(a: &[f32], b: &[f32]) -> f32 {
            let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
            let na: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
            let nb: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
            if na == 0.0 || nb == 0.0 {
                return 0.0;
            }
            (dot / (na * nb)).clamp(-1.0, 1.0)
        }

        let auth1 = embed_one("fn check_password(hash: &str, input: &str) -> bool").await;
        let auth2 = embed_one("fn verify_credentials(username: &str, pwd: &str) -> bool").await;
        let unrelated = embed_one("CREATE TABLE products (id INT, price DECIMAL)").await;

        let sim_related = cosine(&auth1, &auth2);
        let sim_unrelated = cosine(&auth1, &unrelated);
        assert!(
            sim_related > sim_unrelated,
            "semantically similar code should score higher: {sim_related:.3} vs {sim_unrelated:.3}"
        );
    }

    #[tokio::test]
    #[ignore = "requires running Ollama"]
    async fn ollama_large_batch_exceeding_batch_size() {
        // BATCH_SIZE is 8; send 20 texts to exercise the chunking logic
        let embedder = make_embedder();
        let texts: Vec<String> = (0..20)
            .map(|i| format!("fn function_{i}() -> i32 {{ {i} }}"))
            .collect();
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let results = embedder.embed(&refs).await.expect("large batch failed");
        assert_eq!(results.len(), 20);
        let dims = results[0].len();
        assert!(
            results.iter().all(|v| v.len() == dims),
            "all vectors same dims"
        );
    }

    #[tokio::test]
    async fn probe_ollama_errors_when_unreachable() {
        // Port 1 is a reserved system port that is never listening in practice,
        // so the connection is refused immediately without waiting for the timeout.
        let result = super::probe_ollama("http://127.0.0.1:1").await;
        assert!(result.is_err());
        assert!(
            result.unwrap_err().to_string().contains("not reachable"),
            "error message should mention 'not reachable'"
        );
    }

    #[test]
    #[serial_test::serial]
    fn custom_rejects_http_with_api_key() {
        unsafe { std::env::set_var("EMBED_API_KEY", "sk-test-key") };
        let result = RemoteEmbedder::custom("http://example.com", "model");
        unsafe { std::env::remove_var("EMBED_API_KEY") };
        let err = result.err().expect("should be Err");
        assert!(err.to_string().contains("HTTPS"));
    }

    #[test]
    #[serial_test::serial]
    fn custom_allows_http_without_api_key() {
        unsafe { std::env::remove_var("EMBED_API_KEY") };
        let result = RemoteEmbedder::custom("http://localhost:11434", "model");
        assert!(result.is_ok());
    }

    #[test]
    #[serial_test::serial]
    fn custom_allows_https_with_api_key() {
        unsafe { std::env::set_var("EMBED_API_KEY", "sk-test-key") };
        let result = RemoteEmbedder::custom("https://api.example.com", "model");
        unsafe { std::env::remove_var("EMBED_API_KEY") };
        assert!(result.is_ok());
    }

    #[test]
    #[serial_test::serial]
    fn from_url_normalizes_bare_host() {
        let e = RemoteEmbedder::from_url("http://127.0.0.1:43300", "nomic", None).unwrap();
        assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
        assert_eq!(e.model, "nomic");
        assert!(e.api_key.is_none());
    }

    #[test]
    fn from_url_normalizes_v1_suffix() {
        let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1", "nomic", None).unwrap();
        assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
    }

    #[test]
    fn from_url_normalizes_v1_embeddings_suffix() {
        let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1/embeddings", "nomic", None)
            .unwrap();
        assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
    }

    #[test]
    fn from_url_normalizes_trailing_slash() {
        let e = RemoteEmbedder::from_url("http://127.0.0.1:43300/v1/", "nomic", None).unwrap();
        assert_eq!(e.endpoint, "http://127.0.0.1:43300/v1/embeddings");
    }

    #[test]
    fn from_url_passes_api_key() {
        let e =
            RemoteEmbedder::from_url("https://host:8080", "model", Some("sk-123".into())).unwrap();
        assert_eq!(e.api_key.as_deref(), Some("sk-123"));
    }

    #[test]
    fn is_https_or_loopback_matches_host_exactly() {
        // Genuine https / loopback — allowed (no key leak).
        assert!(is_https_or_loopback("https://embed.corp.example/v1"));
        assert!(is_https_or_loopback("http://localhost:48081/v1"));
        assert!(is_https_or_loopback("http://127.0.0.1:48081"));
        assert!(is_https_or_loopback("http://127.0.0.5/v1")); // 127.0.0.0/8
        assert!(is_https_or_loopback("http://[::1]:48081/v1"));
        assert!(is_https_or_loopback("http://user:pass@localhost:8080"));
        // Spoofed hosts an unanchored prefix check would wrongly accept — these
        // must NOT count as loopback, or the API key leaks over cleartext HTTP.
        assert!(!is_https_or_loopback("http://127.evil.com/v1"));
        assert!(!is_https_or_loopback("http://localhost.evil.com/v1"));
        assert!(!is_https_or_loopback("http://127.0.0.1.evil.com/v1"));
        assert!(!is_https_or_loopback("http://example.com/127.0.0.1"));
    }

    #[test]
    #[serial_test::serial]
    fn from_url_falls_back_to_env_api_key() {
        // When api_key param is None, from_url checks EMBED_API_KEY env var.
        // We don't set it here, so it should be None.
        unsafe { std::env::remove_var("EMBED_API_KEY") };
        let e = RemoteEmbedder::from_url("http://host:8080", "model", None).unwrap();
        assert!(e.api_key.is_none());
    }

    #[test]
    fn openai_uses_explicit_api_key_over_env() {
        let e = RemoteEmbedder::openai("text-embedding-3-small", Some("sk-from-config".into()))
            .unwrap();
        assert_eq!(e.api_key.as_deref(), Some("sk-from-config"));
    }

    /// Regression pin for 2026-05-17-reindex-embedding-dim-mismatch.
    ///
    /// The pre-fix code path silently returned `vec![vec![0.0; 1]; texts.len()]`
    /// when every input was empty/whitespace — 1-element sentinel vectors that
    /// did not match the model's real dim, corrupting the downstream vec0
    /// INSERT with a misleading mid-pipeline error. The fix bails before any
    /// vector construction so callers see the cause directly.
    #[tokio::test]
    async fn embed_returns_err_when_all_inputs_empty() {
        let emb = make_embedder();
        let err = emb.embed(&["", "  ", "\t\n"]).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("all 3 text(s) are empty"),
            "expected error message naming the empty count, got: {msg}"
        );
    }
}
