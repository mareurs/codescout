//! Remote embedding via OpenAI-compatible HTTP API.
//!
//! Works with OpenAI, Ollama, LM Studio, and any other server that
//! implements the `/v1/embeddings` endpoint.

use std::borrow::Cow;
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
    /// Query-prefix policy for asymmetric models. See [`QueryPrefix`] for why
    /// this is three-state rather than `Option<String>`.
    query_prefix: QueryPrefix,
    /// Total send attempts per sub-batch, including the first — **not** the
    /// number of retries after it. `1` means fail-fast. See
    /// [`Self::with_max_attempts`].
    max_attempts: usize,
}

/// Send attempts per sub-batch when a caller does not choose. One initial send
/// plus two retries.
///
/// Deliberately *not* referenced by the test that pins it: a test asserting
/// `default == DEFAULT_MAX_ATTEMPTS` passes for every value of the constant and
/// so pins nothing. That test spells `3` as a literal.
const DEFAULT_MAX_ATTEMPTS: usize = 3;

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
///
/// **`pub` as of 2026-08-30 so root can delete its copy rather than keep a second
/// one** (`resume-embedding-transport-stages-1-3:ET-9` T5, the precondition for
/// T7). The two copies had already drifted, and in only one direction: root's had
/// no test at all until `28bb6e8a` added one. That is the shape `ET-4` records —
/// a duplicated *security* predicate whose two halves are unequally guarded. This
/// is the surviving copy; root's goes in Phase D and its test re-points here.
pub fn is_https_or_loopback(url: &str) -> bool {
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

/// How a [`RemoteEmbedder`] decides the query-side prefix for asymmetric models.
///
/// Three states rather than `Option<String>`, because that type cannot
/// distinguish *"nothing configured, work it out from the model"* from
/// *"definitely no prefix"* — and on this project's default model those want
/// **opposite** behaviour.
///
/// `docs/manual/src/concepts/retrieval-stack.md` benchmarks CodeRankEmbed
/// **Q4_K_M with no prefix at 37 (champion)** against **f16 with the required
/// prefix at 34**: *"Q4 loses asymmetric subspace if a prefix is forced."* So a
/// model name containing `coderank` is NOT sufficient evidence that a prefix is
/// wanted — the quantization decides, and [`QueryPrefix::derive_for`]'s
/// name-matching cannot see it. `CodeRankEmbed-Q4_K_M.gguf` carries the answer
/// in the same string it matches on, and matches the wrong half.
///
/// Decided 2026-08-30 (`resume-embedding-transport-stages-1-3:ET-9` D1),
/// upholding `docs/adrs/2026-07-25-embedding-transport-boundary.md`
/// § *The three contracts*: an unset `CODESCOUT_QUERY_PREFIX` maps to
/// [`QueryPrefix::Suppressed`], never to [`QueryPrefix::Derive`]. Deriving there
/// would silently cost ~3 benchmark points on the default deployment.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QueryPrefix {
    /// Infer from the model name. The crate's historical behaviour, kept as the
    /// default for its own constructors so standalone consumers are unaffected
    /// by the addition of the other two states.
    #[default]
    Derive,
    /// Prepend exactly this string. Wins over anything the model name implies.
    Explicit(String),
    /// Never prefix, whatever the model name suggests. This is the state
    /// `Option<String>` could not express, and the one the default deployment
    /// needs.
    Suppressed,
}

impl QueryPrefix {
    /// The prefix to prepend to query text, or `None` for none.
    ///
    /// Borrowed for [`Self::Explicit`] — the common configured path — and owned
    /// only when a prefix is actually derived.
    pub fn resolve<'a>(&'a self, model: &str) -> Option<Cow<'a, str>> {
        match self {
            Self::Suppressed => None,
            Self::Explicit(prefix) => Some(Cow::Borrowed(prefix.as_str())),
            Self::Derive => Self::derive_for(model).map(Cow::Owned),
        }
    }

    /// The prefix a model name implies, if any. Only the CodeRank family is
    /// known to need one.
    ///
    /// Deliberately NOT quantization-aware. Making it so would be a second
    /// fragile substring match layered on the first; the type's doc comment
    /// explains why that is the wrong direction and why `Derive` is therefore
    /// the wrong default for this project.
    pub fn derive_for(model: &str) -> Option<String> {
        if model.to_lowercase().contains("coderank") {
            Some("Represent this query for searching relevant code: ".into())
        } else {
            None
        }
    }
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

    /// The single place the HTTP client is configured. Both entry points below
    /// route through it.
    ///
    /// That sharing is deliberate and load-bearing for the tests, not just tidy:
    /// a builder duplicated between the production path and the injectable one
    /// lets a test pass while the shipped path has lost its bound entirely —
    /// certifying a path the fix does not run on. One builder means removing
    /// `.read_timeout()` here breaks the test that guards it.
    ///
    /// **Two bounds, deliberately. They catch opposite failures and neither
    /// subsumes the other.**
    ///
    /// - `read_timeout` bounds the gap *between* bytes and resets after every
    ///   successful read. This catches a peer that completes the TCP handshake
    ///   and then goes silent: a wedged llama-server, an NVIDIA driver left in an
    ///   invalid state by a failed suspend/resume. Measured 2026-08-29 — exactly
    ///   that, listening and never answering for 15 hours, while its `/health`
    ///   endpoint (a static string that never touches the model) stayed green.
    /// - `timeout` bounds the *whole* request, catching the opposite shape: a
    ///   peer that dribbles bytes indefinitely, never idle long enough to trip
    ///   the read bound.
    ///
    /// The total bound alone is not sufficient, and cannot be tightened into
    /// sufficiency: legitimate 32-input batches measure 23-33s end to end on GPU
    /// and roughly 4x that on CPU, so a total bound tight enough to catch a wedge
    /// promptly would cut off real work.
    fn build_client(read_timeout: std::time::Duration) -> Client {
        Self::install_default_crypto_provider();
        Client::builder()
            .read_timeout(read_timeout)
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build HTTP client")
    }

    /// Production entry point — [`Self::build_client`] with the operator's
    /// override or the default.
    ///
    /// Reads `CODESCOUT_HTTP_READ_TIMEOUT_SECS` here in the constructor path,
    /// mirroring how `EMBED_API_KEY` / `OPENAI_API_KEY` / `OLLAMA_HOST` are
    /// already resolved. Like the `api_key` exemplar in
    /// `docs/conventions/test-env-isolation.md`, that env read is paired with an
    /// injection seam ([`Self::with_read_timeout`]) so no test has to touch
    /// process-global state to exercise the bound.
    ///
    /// A zero or unparseable override falls back to the default rather than
    /// erroring: an operator typo must not be able to restore the unbounded-wait
    /// behaviour this exists to remove.
    fn http_client() -> Client {
        /// Gap-between-bytes allowance. Generous on purpose — a cold GGUF load can
        /// accept a connection well before it can answer, and killing that would
        /// trade a hang for a spurious failure. The goal is a *bounded* wait, not
        /// a short one.
        const DEFAULT_READ_TIMEOUT_SECS: u64 = 120;

        let secs = std::env::var("CODESCOUT_HTTP_READ_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_READ_TIMEOUT_SECS);
        Self::build_client(std::time::Duration::from_secs(secs))
    }

    /// Rebuild the HTTP client with an explicit read timeout. Builder-style.
    ///
    /// The injection seam for [`Self::http_client`]'s env read. Tests use it to
    /// make the silent-peer case observable in milliseconds rather than two
    /// minutes without mutating process-global state — which
    /// `docs/conventions/test-env-isolation.md` rules out as option B ("NOT
    /// VIABLE": `#[serial]` coordinates only among annotated tests, so any
    /// untagged test reading the same var still races).
    ///
    /// Routes through [`Self::build_client`], so it configures the client
    /// identically to the production path apart from the duration. That is what
    /// makes a test written against this method a real guard on the shipped
    /// behaviour rather than on a parallel copy of it.
    pub fn with_read_timeout(mut self, read_timeout: std::time::Duration) -> Self {
        self.client = Self::build_client(read_timeout);
        self
    }

    /// Set the query-prefix policy, replacing the constructor default of
    /// [`QueryPrefix::Derive`].
    ///
    /// This is how a caller expresses *explicitly no prefix* — the state that did
    /// not exist before 2026-08-30 and the one this project's default deployment
    /// needs. See [`QueryPrefix`] for the benchmark that makes it load-bearing.
    pub fn with_query_prefix(mut self, query_prefix: QueryPrefix) -> Self {
        self.query_prefix = query_prefix;
        self
    }

    /// Set the total send attempts per sub-batch, replacing the default of
    /// [`DEFAULT_MAX_ATTEMPTS`]. **`1` is fail-fast**: one send, no backoff, the
    /// first error surfaces.
    ///
    /// *Attempts*, not retries, and the distinction is why this is not
    /// `with_max_retries`. The bound it feeds has always been
    /// `for attempt in 0..n`, and the operator-facing error has always read
    /// `"unavailable after {n} attempts"` — only the constant behind it was
    /// misnamed `MAX_RETRIES`. Naming the knob for retries would have required an
    /// off-by-one at every call site to mean the same thing.
    ///
    /// **Why a caller would want `1`.** codescout's root crate is swapping its
    /// dense embedding leg onto this type
    /// (`resume-embedding-transport-stages-1-3:ET-9` T6). That leg issues exactly
    /// one request today and fails on the first connect error, and it runs while
    /// the per-project index lock is held. Retrying under that lock is the
    /// failure mode this project already capped on its *sparse* leg, whose own
    /// comment names the cost: unbounded retry there "means the lock never
    /// releases, wedging every subsequent index for that project". So the swap
    /// opts out of retry to stay behaviour-preserving, rather than silently
    /// inheriting 1.5s of backoff per sub-batch on a path that has never had any.
    ///
    /// `0` is clamped to `1`. Zero attempts would send nothing and then report
    /// the server "unavailable", blaming a peer that was never contacted — the
    /// same reasoning as [`Self::http_client`], where a zero override falls back
    /// rather than erroring.
    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
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
            query_prefix: QueryPrefix::Derive,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
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
            query_prefix: QueryPrefix::Derive,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
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
            query_prefix: QueryPrefix::Derive,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
        })
    }

    /// Create an embedder from an explicit URL.
    ///
    /// Normalizes the URL to always end with `/v1/embeddings`:
    /// - `http://host:port`               → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1`            → `http://host:port/v1/embeddings`
    /// - `http://host:port/v1/embeddings` → `http://host:port/v1/embeddings`
    ///
    /// The shape recognition (which of the three forms `url` is) is
    /// `crate::normalize_embeddings_base` — shared with the root crate's
    /// `RetrievalConfig::normalize_embedder_url` rather than duplicated, since
    /// the two used to be independent copies of the same three branches.
    ///
    /// `api_key` is used exactly as given — no ambient-env fallback. The one
    /// production caller (`create_embedder_with_config`) always receives an
    /// already-resolved value from its own caller's `RetrievalConfig`/
    /// `LibrarianEnv`, so resolving `EMBED_API_KEY` again here was a second,
    /// redundant read of the same var — and the one that let tests calling
    /// `from_url` directly race the tests that mutate that var under
    /// `#[serial]`. See docs/conventions/test-env-isolation.md.
    ///
    /// Rejects plaintext HTTP when an `api_key` is supplied. Loopback hosts
    /// (`localhost`, `127.0.0.1`, `[::1]`) are permitted to support local
    /// Ollama / llama.cpp setups where the key is only meaningful as a
    /// request-shape parameter.
    pub fn from_url(url: &str, model: &str, api_key: Option<String>) -> Result<Self> {
        let endpoint = format!("{}/v1/embeddings", crate::normalize_embeddings_base(url));

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
            query_prefix: QueryPrefix::Derive,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
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
        const INITIAL_BACKOFF_MS: u64 = 500;
        // Total sends per sub-batch INCLUDING the first, so `1` is fail-fast.
        // Was `const MAX_RETRIES: usize = 3` — a misnomer, since the loop below
        // has always been `0..n` and the error below has always said "attempts".
        // Now per-instance so a caller can opt out of retry entirely; see
        // `with_max_attempts` for why the root crate's dense leg does.
        let max_attempts = self.max_attempts;

        let mut embedded = Vec::with_capacity(filtered.len());
        for batch in filtered.chunks(BATCH_SIZE) {
            let mut last_err: Option<anyhow::Error> = None;
            let mut backoff_ms = INITIAL_BACKOFF_MS;
            let resp_data = 'retry: {
                for attempt in 0..max_attempts {
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
                            // Typed for the unreachable cases, so a consumer can
                            // classify without matching on reqwest's own wording.
                            // Untyped, this surfaces as "error sending request for
                            // url (...)", which matches no bucket in root's
                            // classifier and falls through to the generic
                            // Qdrant-oriented fallback — sending operators to debug
                            // a healthy store. ET-5.
                            //
                            // The `is_connect() || is_timeout()` split mirrors the
                            // one root's own EmbedderHttp already applies: a 4xx
                            // body or a builder error is NOT a reachability problem
                            // and must not claim to be one.
                            last_err = Some(if e.is_connect() || e.is_timeout() {
                                anyhow::Error::new(crate::EmbedError::Connect {
                                    url: self.endpoint.clone(),
                                    detail: e.to_string(),
                                })
                            } else {
                                anyhow::Error::new(e)
                            });
                            continue;
                        }
                    };
                    let status = resp.status();
                    if !status.is_success() {
                        let body = resp.text().await.unwrap_or_default();
                        // Typed for the same reason the connect arm above is: the
                        // consumer classifies on wording across a crate boundary,
                        // where nothing makes a literal and its test fail together.
                        // This one additionally carries the server's own response
                        // body, so a consumer must be able to match it BEFORE
                        // testing for anything a body might coincidentally contain
                        // — untyped, an embedder 404 reading `model not found` was
                        // classified as a missing Qdrant collection. ET-5.
                        let typed = crate::EmbedError::Status {
                            url: self.endpoint.clone(),
                            status: status.as_u16(),
                            body,
                        };
                        if status.is_server_error() {
                            last_err = Some(anyhow::Error::new(typed));
                            continue;
                        }
                        // 4xx — bad request, wrong model, etc. — don't retry.
                        return Err(anyhow::Error::new(typed));
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
                    anyhow::anyhow!("embedding server unavailable after {max_attempts} attempts")
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

        // The server must return exactly one vector per non-empty input. A gateway
        // that silently truncates an oversize request returns fewer — and the
        // reconstruction below indexes `embedded[slot]` once per non-empty slot, so
        // without this check a short response is an index-out-of-bounds **panic**,
        // not an error. The `dim` block above anticipated the *no-data* case and
        // not the *short* one; and its cached-dims fallback made the gap worse,
        // since it let a zero-vector reconstruction proceed into the same panic.
        //
        // Reachable from any deployment whose endpoint truncates, which is exactly
        // what the consumer-side check in root's `embed_one_batch` was written for.
        if embedded.len() != filtered.len() {
            bail!(
                "embedding server returned {} vectors for {} non-empty inputs — the \
                 server may be silently truncating an oversize request instead of \
                 refusing it. Send a smaller batch.",
                embedded.len(),
                filtered.len()
            );
        }

        let mut all = vec![vec![0.0; dim]; texts.len()];
        for (slot, (orig_idx, _)) in non_empty.iter().enumerate() {
            all[*orig_idx] = std::mem::take(&mut embedded[slot]);
        }
        Ok(all)
    }

    /// Embed a single **query**, applying the configured [`QueryPrefix`].
    ///
    /// The document side ([`Embedder::embed`]) never prefixes — that asymmetry is
    /// the entire point of an asymmetric model, and getting it backwards strands
    /// stored vectors in query-space. See
    /// `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`.
    async fn embed_query(&self, text: &str) -> Result<Embedding> {
        let prefixed;
        let input: &str = match self.query_prefix.resolve(&self.model) {
            Some(prefix) => {
                prefixed = format!("{prefix}{text}");
                &prefixed
            }
            None => text,
        };
        let mut batch = self.embed(&[input]).await?;
        batch
            .pop()
            .ok_or_else(|| anyhow::anyhow!("embed_query: empty response"))
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
    /// A peer that completes the TCP handshake and then goes silent is the shape
    /// of a wedged model server, and it is the failure a *total*-request timeout
    /// handles worst: the 300s bound eventually fires, but only after 300s.
    ///
    /// Measured 2026-08-29 on a developer host — an NVIDIA driver failed a
    /// suspend/resume, `llama-server` became an unreapable zombie, and the
    /// container kept its port open and answered `/health` (a static string that
    /// never touches the model) for 15 hours while every inference request hung.
    /// This crate's `http_client` doc comment had named that exact class
    /// ("a hung embedding server, e.g. Ollama during GPU discovery failure")
    /// since it was written; the bound it carried was simply the coarse one.
    ///
    /// Three details are load-bearing and each would silently defeat the test:
    ///
    /// - **Not a closed port.** That fails on connect, which already worked and
    ///   already produced a clear error. The defect lives entirely in the gap
    ///   between "connected" and "answered".
    /// - **The accepted streams are held, not dropped.** A dropped stream closes
    ///   the socket, which the client reports promptly as a clean EOF — the test
    ///   would then pass with no read bound configured at all.
    /// - **`with_read_timeout`, not `CODESCOUT_HTTP_READ_TIMEOUT_SECS`.**
    ///   Mutating process env to reach this is option B of
    ///   `docs/conventions/test-env-isolation.md`, marked NOT VIABLE: `#[serial]`
    ///   coordinates only among annotated tests, so any untagged test reading the
    ///   same var still races. `from_url` is used because it is the one
    ///   constructor with no ambient-env fallback.
    ///
    /// The outer `tokio::time::timeout` is what lets this fail rather than wedge
    /// the binary: remove `.read_timeout()` from `http_client` and the inner call
    /// sits until the 300s total bound, well past this 30s ceiling.
    #[tokio::test]
    async fn a_peer_that_accepts_and_never_answers_errors_instead_of_waiting_forever() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _wedged = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                held.push(stream);
            }
        });

        let embedder = RemoteEmbedder::from_url(&format!("http://{addr}"), MODEL, None)
            .expect("plaintext loopback without an api_key is permitted")
            .with_read_timeout(std::time::Duration::from_millis(250));

        let result =
            tokio::time::timeout(std::time::Duration::from_secs(30), embedder.embed(&["x"]))
                .await
                .expect(
                    "a silent peer must produce an error within the read timeout, not hang. \
             If this fires, http_client has lost its read_timeout and every embed \
             call has fallen back to the 300s total bound",
                );

        assert!(
            result.is_err(),
            "a peer that never answers cannot produce an embedding"
        );
    }

    /// Spawn a loopback server that answers **500** to everything and counts the
    /// requests it actually served. Returns its base url and the counter.
    ///
    /// `500` is load-bearing, not arbitrary. `embed`'s loop retries
    /// `status.is_server_error()` and `bail!`s on 4xx, so a 4xx server would
    /// measure the terminal path and report `1` for every attempt setting —
    /// passing the fail-fast test while proving nothing.
    ///
    /// Counted **after** reading the request rather than on `accept`, so the
    /// number is requests served, not connections opened. `Connection: close`
    /// keeps the two identical anyway; the ordering is what makes that true
    /// rather than assumed.
    async fn spawn_counting_500_server() -> (String, std::sync::Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let hits = std::sync::Arc::new(AtomicUsize::new(0));
        let hits_bg = std::sync::Arc::clone(&hits);
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let hits = std::sync::Arc::clone(&hits_bg);
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    // One read suffices: the callers below embed a single short
                    // text, so the whole request fits well inside this buffer and
                    // the body is never inspected.
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).await;
                    hits.fetch_add(1, Ordering::SeqCst);
                    let _ = stream
                        .write_all(
                            b"HTTP/1.1 500 Internal Server Error\r\n\
                              Content-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        (format!("http://{addr}"), hits)
    }

    /// Drive `embed` against the counting server and report how many requests it
    /// actually sent. The call is expected to fail — every response is a 500.
    async fn attempts_made(configure: impl FnOnce(RemoteEmbedder) -> RemoteEmbedder) -> usize {
        let (url, hits) = spawn_counting_500_server().await;
        let embedder = configure(RemoteEmbedder::from_url(&url, MODEL, None).unwrap());
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(30), embedder.embed(&["x"]))
                .await
                .expect("a server that always answers 500 must terminate, not hang");
        assert!(
            result.is_err(),
            "a persistently 500-ing server cannot produce an embedding"
        );
        hits.load(Ordering::SeqCst)
    }

    /// The default is three sends per sub-batch — one initial plus two retries.
    ///
    /// `3` is spelled as a **literal**, deliberately. Asserting against
    /// `DEFAULT_MAX_ATTEMPTS` would hold for every value of that constant and so
    /// pin nothing; this fails if the default is changed, which is the point.
    #[tokio::test]
    async fn the_default_is_three_attempts() {
        assert_eq!(
            attempts_made(|e| e).await,
            3,
            "default must be one send plus two retries"
        );
    }

    /// The fail-fast setting `codescout`'s dense leg needs (ET-9 T6): exactly one
    /// send, no backoff, the first error surfaces.
    ///
    /// This is the assertion the whole knob exists for. A `with_max_attempts`
    /// that stored the value but never reached the loop — the four-link gap where
    /// a resolver is correct and nothing calls it — passes a field-equality test
    /// and fails this one, because this counts requests on the wire.
    #[tokio::test]
    async fn max_attempts_of_one_sends_exactly_one_request() {
        assert_eq!(
            attempts_made(|e| e.with_max_attempts(1)).await,
            1,
            "1 attempt must mean one send and no retry — this is the \
             behaviour-preserving setting for a caller whose current \
             implementation does not retry at all"
        );
    }

    /// Distinguishes "reads the field" from "special-cases 1". Without this, an
    /// `if max_attempts == 1 { once } else { 3 }` implementation passes both
    /// tests above.
    #[tokio::test]
    async fn an_intermediate_max_attempts_is_honoured_exactly() {
        assert_eq!(
            attempts_made(|e| e.with_max_attempts(2)).await,
            2,
            "the loop bound must be the configured value, not a two-state choice \
             between fail-fast and the default"
        );
    }

    /// `0` clamps to `1` rather than sending nothing.
    ///
    /// Unclamped, `0..0` never executes, `last_err` stays `None`, and the caller
    /// is told the server was "unavailable after 0 attempts" — blaming a peer
    /// that was never contacted. Asserting the request count rather than the
    /// error text is what makes this about behaviour: the count separates
    /// "clamped and tried once" from "sent nothing and guessed".
    #[tokio::test]
    async fn zero_max_attempts_is_clamped_to_one_send() {
        assert_eq!(
            attempts_made(|e| e.with_max_attempts(0)).await,
            1,
            "0 must clamp to a single real send, never to zero sends"
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
        // `from_url` used to fall back to EMBED_API_KEY when the argument was
        // None — that made it read ambient config, which is exactly the shape
        // docs/conventions/test-env-isolation.md rules out (an untagged test
        // elsewhere reading the same var can race a tagged one that sets it).
        // Loopback host so a leaked key doesn't also trip the HTTPS guard —
        // that guard is orthogonal to this test and is covered elsewhere.
        unsafe { std::env::set_var("EMBED_API_KEY", "sk-should-be-ignored") };
        let e = RemoteEmbedder::from_url("http://127.0.0.1:43300", "model", None).unwrap();
        unsafe { std::env::remove_var("EMBED_API_KEY") };
        assert!(
            e.api_key.is_none(),
            "from_url must not consult EMBED_API_KEY at all — the caller is the only source"
        );
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

    /// One-shot HTTP server that captures the request body and answers with a
    /// valid embeddings response.
    ///
    /// The wire is the only place the prefix decision is observable: `resolve()`
    /// can be unit-tested in isolation, but that proves nothing about whether
    /// `embed_query` still calls it. Without a wire assertion, deleting the
    /// `resolve` call from `embed_query` leaves every prefix test green.
    async fn capture_one_request() -> (String, tokio::task::JoinHandle<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = Vec::new();
            loop {
                let mut chunk = [0u8; 4096];
                let n = stream.read(&mut chunk).await.unwrap();
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&chunk[..n]);
                let text = String::from_utf8_lossy(&buf).into_owned();
                let Some(head_end) = text.find("\r\n\r\n") else {
                    continue;
                };
                let declared = text[..head_end]
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                if buf.len() - (head_end + 4) >= declared {
                    break;
                }
            }
            let text = String::from_utf8_lossy(&buf).into_owned();
            let body = text
                .split_once("\r\n\r\n")
                .map(|(_, b)| b.to_string())
                .unwrap_or_default();

            let payload = r#"{"data":[{"embedding":[0.1,0.2],"index":0}]}"#;
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                payload.len(),
                payload
            );
            stream.write_all(resp.as_bytes()).await.unwrap();
            stream.flush().await.unwrap();
            body
        });
        (format!("http://{addr}"), handle)
    }

    const CODERANK: &str = "CodeRankEmbed-Q4_K_M.gguf";
    const DERIVED: &str = "Represent this query for searching relevant code: ";

    /// The state that did not exist before 2026-08-30, and the reason the whole
    /// three-state change happened: a model name that WOULD derive a prefix, told
    /// not to.
    ///
    /// `CODERANK` is deliberately the real default filename. It contains both
    /// `CodeRank` (which `derive_for` matches) and `Q4_K_M` (the quantization the
    /// benchmark says wants NO prefix, and which `derive_for` cannot see). If
    /// `Suppressed` ever collapses back into `Derive`, this is the configuration
    /// that silently drops from 37 to 34.
    ///
    /// Mutation that must kill it: change `Self::Suppressed => None` in
    /// `QueryPrefix::resolve` to fall through to `derive_for`.
    #[tokio::test]
    async fn suppressed_sends_no_prefix_even_for_a_model_that_would_derive_one() {
        let (url, server) = capture_one_request().await;
        let embedder = RemoteEmbedder::from_url(&url, CODERANK, None)
            .unwrap()
            .with_query_prefix(QueryPrefix::Suppressed);

        embedder.embed_query("fn main").await.unwrap();
        let body = server.await.unwrap();

        assert!(
            !body.contains("Represent this query"),
            "Suppressed must beat the model name; the wire carried a derived \
             prefix, which is the 34-point configuration. body: {body}"
        );
        assert!(
            body.contains("fn main"),
            "the query text itself must still be sent. body: {body}"
        );
    }

    /// An explicit prefix wins over whatever the model name implies — the Nomic
    /// case (`search_query: `) pointed at a CodeRank-named endpoint.
    ///
    /// Mutation that must kill it: reorder `resolve` so `Derive` is consulted
    /// before `Explicit`.
    #[tokio::test]
    async fn an_explicit_prefix_overrides_the_one_the_model_name_implies() {
        let (url, server) = capture_one_request().await;
        let embedder = RemoteEmbedder::from_url(&url, CODERANK, None)
            .unwrap()
            .with_query_prefix(QueryPrefix::Explicit("search_query: ".into()));

        embedder.embed_query("fn main").await.unwrap();
        let body = server.await.unwrap();

        assert!(
            body.contains("search_query: fn main"),
            "the explicit prefix must reach the wire. body: {body}"
        );
        assert!(
            !body.contains("Represent this query"),
            "the model-derived prefix must not also be applied. body: {body}"
        );
    }

    /// `Derive` is unchanged and remains the constructor default, so standalone
    /// consumers of this crate see no behaviour change from the other two states
    /// being added.
    ///
    /// Mutation that must kill it: make `QueryPrefix::default()` return
    /// `Suppressed`, or drop the `resolve` call from `embed_query`.
    #[tokio::test]
    async fn derive_is_still_the_constructor_default_and_still_prefixes_coderank() {
        let (url, server) = capture_one_request().await;
        let embedder = RemoteEmbedder::from_url(&url, CODERANK, None).unwrap();
        assert_eq!(
            embedder.query_prefix,
            QueryPrefix::Derive,
            "constructors must keep deriving, or every standalone consumer changes behaviour"
        );

        embedder.embed_query("fn main").await.unwrap();
        let body = server.await.unwrap();

        assert!(
            body.contains(&format!("{DERIVED}fn main")),
            "Derive must still apply the model-derived prefix. body: {body}"
        );
    }

    /// The document side never prefixes, under any policy. Getting this backwards
    /// strands stored vectors in query-space —
    /// docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md.
    #[tokio::test]
    async fn the_document_path_never_prefixes_even_under_derive() {
        let (url, server) = capture_one_request().await;
        let embedder = RemoteEmbedder::from_url(&url, CODERANK, None).unwrap();

        embedder.embed(&["fn main"]).await.unwrap();
        let body = server.await.unwrap();

        assert!(
            !body.contains("Represent this query"),
            "embed() is the document path and must never carry a query prefix. body: {body}"
        );
    }

    /// `derive_for` recognises only the CodeRank family. Pinned separately from
    /// the wire tests because it is the half a future model would extend.
    #[test]
    fn derive_for_matches_the_coderank_family_and_nothing_else() {
        assert_eq!(
            QueryPrefix::derive_for("CodeRankEmbed"),
            Some(DERIVED.into())
        );
        assert_eq!(
            QueryPrefix::derive_for("coderankembed-q4_k_m.gguf"),
            Some(DERIVED.into()),
            "matching is case-insensitive"
        );
        assert_eq!(QueryPrefix::derive_for("nomic-embed-text"), None);
        assert_eq!(QueryPrefix::derive_for(""), None);

        // Suppressed and Explicit do not consult the model at all.
        assert_eq!(QueryPrefix::Suppressed.resolve("CodeRankEmbed"), None);
        assert_eq!(
            QueryPrefix::Explicit("x: ".into())
                .resolve("CodeRankEmbed")
                .as_deref(),
            Some("x: ")
        );
    }

    /// A connect failure must render the crate's **published** marker, so a
    /// consumer can route it to the embedder hint instead of "check qdrant logs".
    ///
    /// Port 1 is reserved and never listening, so this is connect-refused rather
    /// than a timeout — the fast half of the same class the wedged-peer test
    /// covers from the slow side.
    ///
    /// `resume-embedding-transport-stages-1-3:ET-5`. Before this, the retry loop
    /// stored the raw `reqwest::Error`, whose Display is "error sending request
    /// for url (...)" — containing neither this marker nor the "embedding server"
    /// wording a consumer might match instead. A dead embedder on the resolver
    /// path therefore fell through to the generic Qdrant-oriented fallback,
    /// sending operators to debug a healthy store.
    #[tokio::test]
    async fn a_connect_failure_renders_the_published_marker() {
        let embedder = RemoteEmbedder::from_url("http://127.0.0.1:1", MODEL, None).unwrap();
        let err = embedder
            .embed(&["x"])
            .await
            .expect_err("a port that refuses connections cannot produce an embedding");
        let msg = err.to_string();

        assert!(
            msg.contains(crate::CONNECT_FAILED_MARKER),
            "a connect failure must carry `{}` so a consumer can classify it \
             without matching on reqwest's own wording. got: {msg}",
            crate::CONNECT_FAILED_MARKER
        );
        assert!(
            msg.contains("127.0.0.1:1"),
            "the marker is useless without the url it failed against. got: {msg}"
        );
    }

    /// Spawn a loopback server answering every request with `status_line` and `body`.
    ///
    /// Distinct from [`spawn_counting_500_server`] in purpose: that one measures how
    /// many times `embed` retries, so it must be a 5xx. These callers want the error
    /// the caller finally *receives*, so they use a 4xx — `embed` returns on 4xx
    /// without retrying, keeping the test to one round trip.
    async fn spawn_status_server(status_line: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};
                    let mut buf = [0u8; 4096];
                    let _ = stream.read(&mut buf).await;
                    let resp = format!(
                        "HTTP/1.1 {status_line}\r\nContent-Length: {}\r\n\
                         Connection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(resp.as_bytes()).await;
                    let _ = stream.shutdown().await;
                });
            }
        });
        format!("http://{addr}")
    }

    /// A status failure must render the crate's **published** marker — and must do
    /// so even when the server's body contains text that another classifier arm
    /// matches.
    ///
    /// The body here is the real case, not a contrivance. Untyped, this rendered as
    /// `HTTP 404 from embedding server: model 'coderank' not found`, and root's
    /// classifier tests `not found` (its Qdrant-collection arm) *before* it tests
    /// `embedding server` — so a healthy collection was reported missing and the
    /// operator was told to re-index it. See
    /// `docs/issues/archive/2026-08-30-crate-status-errors-hijack-the-qdrant-collection-bucket.md`.
    /// A marker the body cannot impersonate is what lets a consumer match on
    /// specificity first.
    #[tokio::test]
    async fn a_status_failure_renders_the_published_marker() {
        let url = spawn_status_server("404 Not Found", "model 'coderank' not found").await;
        let embedder = RemoteEmbedder::from_url(&url, MODEL, None).unwrap();
        let err = embedder
            .embed(&["x"])
            .await
            .expect_err("a server answering 404 cannot produce an embedding");
        let msg = err.to_string();

        assert!(
            msg.contains(crate::STATUS_FAILED_MARKER),
            "a status failure must carry `{}` so a consumer can classify it without \
             matching on this crate's prose. got: {msg}",
            crate::STATUS_FAILED_MARKER
        );
        assert!(
            msg.contains("404"),
            "the status code is half the diagnosis — 4xx means the request is wrong, \
             5xx means the server is. got: {msg}"
        );
        assert!(
            msg.contains("model 'coderank' not found"),
            "the server's body is usually the whole diagnosis and must survive into \
             the message. got: {msg}"
        );
        // The hazard, as an assertion rather than a comment: this body DOES carry
        // the text that hijacked the collection bucket, and the marker is present
        // regardless. A consumer testing the marker first is therefore always able
        // to win, whatever the server chose to say.
        assert!(
            msg.contains("not found") && msg.contains(crate::STATUS_FAILED_MARKER),
            "the marker must coexist with a body impersonating another arm. got: {msg}"
        );
    }

    /// The typed form is reachable, and carries the status as a **number**.
    ///
    /// Mirrors `a_connect_failure_is_downcastable_to_the_typed_error`. The numeric
    /// field is the point: a consumer branching on 4xx-vs-5xx must not have to
    /// re-parse the code back out of the prose it just rendered.
    #[tokio::test]
    async fn a_status_failure_is_downcastable_to_the_typed_error() {
        let url = spawn_status_server("422 Unprocessable Entity", "input too long").await;
        let embedder = RemoteEmbedder::from_url(&url, MODEL, None).unwrap();
        let err = embedder
            .embed(&["x"])
            .await
            .expect_err("a server answering 422 cannot produce an embedding");

        match err.downcast_ref::<crate::EmbedError>() {
            Some(crate::EmbedError::Status { status, body, .. }) => {
                assert_eq!(*status, 422, "the numeric status must survive typing");
                assert_eq!(body, "input too long", "the body must survive verbatim");
            }
            other => panic!("expected EmbedError::Status, got {other:?}"),
        }
    }

    /// Display is bounded at 400 characters; the typed field is not.
    ///
    /// Both halves matter and they pull in opposite directions. The bound exists
    /// because an HTML error page would otherwise flood every surface that renders
    /// this — `SyncReport.skipped` holds one entry per skipped chunk. Keeping the
    /// field whole exists because a consumer that downcasts wants the real body, and
    /// truncating at construction would have destroyed it for everyone.
    #[test]
    fn a_status_body_is_bounded_in_display_but_kept_whole_in_the_type() {
        let e = crate::EmbedError::Status {
            url: "http://127.0.0.1:9/v1/embeddings".into(),
            status: 500,
            body: "z".repeat(1000),
        };

        let msg = e.to_string();
        assert_eq!(
            msg.matches('z').count(),
            400,
            "Display must show exactly the 400-character bound, not the whole body"
        );

        let crate::EmbedError::Status { body, .. } = &e else {
            panic!("constructed a Status, must still be one");
        };
        assert_eq!(
            body.len(),
            1000,
            "the field keeps the whole body — truncation is a rendering concern"
        );
    }

    /// An empty body must not render as a dangling colon.
    ///
    /// llama.cpp answers some refusals with a status and no body at all; `HTTP 400
    /// from embedding server: ` reads as a truncated message rather than as the
    /// complete information it actually is.
    #[test]
    fn a_status_with_no_body_says_so() {
        let msg = crate::EmbedError::Status {
            url: "http://127.0.0.1:9/v1/embeddings".into(),
            status: 400,
            body: "   ".into(),
        }
        .to_string();

        assert!(
            msg.contains("<empty response body>"),
            "a whitespace-only body must be named as empty, not rendered as nothing. \
             got: {msg}"
        );
    }

    /// A server returning fewer vectors than inputs must **error, not panic**.
    ///
    /// The reconstruction indexes `embedded[slot]` once per non-empty input, so a
    /// short response was an index-out-of-bounds panic — a library aborting the
    /// process on remote input it does not control, reachable from any endpoint
    /// that truncates an oversize request instead of refusing it.
    ///
    /// Found 2026-08-30 when root's dense leg began delegating here: root's own
    /// arity check had been catching this cleanly, and once the crate ran first the
    /// panic surfaced. `embed_one_batch_errors_on_dense_arity_mismatch` in root is
    /// the same assertion one layer up, and both are worth keeping — they fail for
    /// different reasons now.
    #[tokio::test]
    async fn a_short_response_errors_instead_of_panicking() {
        let url = spawn_status_server(
            "200 OK",
            r#"{"data":[{"embedding":[1.0,2.0,3.0],"index":0}]}"#,
        )
        .await;
        let embedder = RemoteEmbedder::from_url(&url, MODEL, None).unwrap();

        let err = embedder
            .embed(&["a", "b", "c"])
            .await
            .expect_err("one vector for three inputs cannot be a successful embed");

        let msg = err.to_string();
        assert!(
            msg.contains('1') && msg.contains('3'),
            "the error must name both the returned and the expected count, so an \
             operator can tell truncation from an outage. got: {msg}"
        );
    }

    /// The typed error survives as a type, not only as text — so a consumer that
    /// wants to branch structurally can downcast instead of substring-matching.
    /// The marker exists for consumers (like root's `classify_search_error`) that
    /// only ever see a rendered string.
    #[tokio::test]
    async fn a_connect_failure_is_downcastable_to_the_typed_error() {
        let embedder = RemoteEmbedder::from_url("http://127.0.0.1:1", MODEL, None).unwrap();
        let err = embedder.embed(&["x"]).await.unwrap_err();

        let typed = err.downcast_ref::<crate::EmbedError>().expect(
            "a connect failure must reach the caller as EmbedError, not as an opaque anyhow",
        );
        match typed {
            crate::EmbedError::Connect { url, .. } => {
                assert!(
                    url.contains("127.0.0.1:1"),
                    "url should name the endpoint, got {url}"
                )
            }
            // Deliberately an arm rather than a wildcard: the two variants have
            // opposite remedies, so a refused connection surfacing as `Status`
            // would send an operator to read a response body that was never
            // received. Keeping it exhaustive also means the next variant added
            // to `EmbedError` fails this file rather than passing silently.
            other => panic!("a refused connection must be Connect, got {other:?}"),
        }
    }
}
