//! Shared HTTP client construction for the retrieval stack's two remote legs
//! (the dense/sparse embedder and the reranker).
//!
//! Exists so the timeout policy is stated once. Both legs previously built their
//! client with `reqwest::Client::new()`, whose `timeout`, `read_timeout` and
//! `connect_timeout` all default to `None` — so a peer that completed the TCP
//! handshake and then never wrote a byte blocked the caller forever. Measured
//! 2026-08-29: a wedged local llama-server on `127.0.0.1:48081` turned
//! `cargo test` into an unbounded hang with no failure and no output. See
//! `docs/issues/archive/2026-08-29-wedged-embed-server-hangs-cargo-test-forever.md`.

use std::time::Duration;

/// Default gap-between-bytes allowance for the embed and rerank legs.
///
/// 120s matches the Qdrant client's timeout at `retrieval::client`'s
/// `from_config_only`, and is deliberately generous: a cold GGUF load can accept
/// the connection well before it can answer, and killing that would trade a hang
/// for a spurious failure. The point is to make the wait *bounded*, not short.
pub(crate) const DEFAULT_READ_TIMEOUT_SECS: u64 = 120;

/// A `reqwest::Client` that cannot wait forever on a silent peer.
///
/// Uses `read_timeout`, **not** `timeout`. The distinction is load-bearing:
/// `timeout` bounds the whole request, and this crate's own measurements
/// (see `DEFAULT_INFLIGHT` in `super::embedder`) record legitimate 32-input GPU
/// batches at 6.6-12.1s of inference and 23-33s end to end. A total-request
/// timeout tight enough to catch a wedged server would cut off real work.
/// `read_timeout` applies per read operation and **resets after every successful
/// read**, so a slow-but-progressing server is never affected while a server
/// producing no bytes at all fails promptly.
pub(crate) fn client(read_timeout: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .read_timeout(read_timeout)
        // The only documented failure is TLS backend initialisation, which
        // `crate::install_default_crypto_provider` has already performed at every
        // construction site. No caller input reaches this builder, so there is no
        // runtime-dependent way for it to fail.
        .build()
        .expect("static reqwest client configuration is always valid")
}

/// The operator's read-timeout override, or [`DEFAULT_READ_TIMEOUT_SECS`].
///
/// Read only from the env-reading constructors (`EmbedderHttp::new`,
/// `RerankerHttp::new`), never from their `with_*` siblings — those are the
/// explicit-control paths the tests use, and an ambient env var reaching them
/// would make test behaviour a function of the developer's shell. That is the
/// same coupling that let a wedged local service hang the suite in the first
/// place.
///
/// A zero or unparseable value falls back to the default rather than erroring:
/// an operator typo must not be able to restore the unbounded-wait behaviour
/// this module exists to remove.
pub(crate) fn read_timeout_from_env() -> Duration {
    Duration::from_secs(
        std::env::var("CODESCOUT_HTTP_READ_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|&n| n > 0)
            .unwrap_or(DEFAULT_READ_TIMEOUT_SECS),
    )
}
