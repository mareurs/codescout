use crate::retrieval::code_store::{CodeVectorStore, VectorBackend};
use crate::retrieval::config::RetrievalConfig;
use crate::retrieval::embedder::{is_https_or_loopback, CodeEmbedder, EmbedderHttp};
#[cfg(feature = "server-stack")]
use crate::retrieval::qdrant::QdrantWrap;
use crate::retrieval::reranker::RerankerHttp;
use anyhow::Result;
use std::sync::Arc;

#[cfg(feature = "server-stack")]
/// Pre-Option `EmbedderHttp` construction default, preserved exactly.
/// `from_config_only` (below, itself `server-stack`-only) is the last caller —
/// Task 7 replaced the `from_env` arm that used to fall back to this
/// unconditionally with real url-vs-model backend selection, so a build
/// without `server-stack` never references this constant at all. Cfg-gated
/// to match, rather than left as always-compiled dead code on that build.
const FALLBACK_EMBEDDER_URL: &str = "http://127.0.0.1:8081";

pub struct RetrievalClient {
    /// Code-chunk vector store behind the `CodeVectorStore` seam. Qdrant today;
    /// in-process sqlite-vec in the lite stack (Phase 2). `pub(crate)` so the
    /// sibling `search`/`sync` modules can reach it without exposing it outside
    /// the crate. See `docs/plans/2026-06-16-two-stack-retrieval-lite.md`.
    pub(crate) code_store: Arc<dyn CodeVectorStore>,
    pub embedder: Arc<dyn CodeEmbedder>,
    pub reranker: RerankerHttp,
    pub config: RetrievalConfig,
    /// True for the daemon-free lite stack (sqlite-vec backend): dense-only, no
    /// reranker server. Gates the rerank step in `search_in`.
    pub(crate) lite: bool,
}

impl RetrievalClient {
    /// A local backend is selected when no url is configured and the model
    /// names one. Keep this the single source of truth — `dense_only` and the
    /// sparse guard both read it.
    pub(crate) fn backend_is_local(config: &RetrievalConfig) -> bool {
        config.embedder_url.is_none()
            && (config.model.starts_with("local:") || config.model.starts_with("local-dir:"))
    }

    /// A local backend emits no sparse vector. Silently dropping to dense would
    /// show up as degraded recall and never as a failure, so it is an error —
    /// but an expected, operator-fixable one (a config conflict, not a bug),
    /// so `RecoverableError` (isError: false, sibling parallel calls survive)
    /// rather than `anyhow::bail!` (isError: true, aborts siblings).
    pub(crate) fn guard_sparse(config: &RetrievalConfig, lite: bool) -> Result<()> {
        if Self::backend_is_local(config) && !lite && !config.disable_sparse {
            return Err(crate::tools::RecoverableError::with_hint(
                "the local embedding backend produces no sparse vector, but the hybrid \
                 sparse leg is enabled."
                    .to_string(),
                "Either set CODESCOUT_DISABLE_SPARSE=1 to run dense-only, or configure \
                 an embedder url that serves both dense and sparse.",
            )
            .into());
        }
        Ok(())
    }

    /// A local backend never produces a sparse vector, so it always runs
    /// dense-only — so does the lite stack (no sparse server) and an explicit
    /// sparse opt-out. Kept as its own term rather than inferred from
    /// `guard_sparse` having already run first, so a future caller that
    /// consults `dense_only` without going through the sparse guard still
    /// gets the right answer.
    pub(crate) fn dense_only(config: &RetrievalConfig, lite: bool) -> bool {
        lite || config.disable_sparse || Self::backend_is_local(config)
    }

    /// Whether `[embeddings].api_key` should be forwarded to the dense
    /// endpoint — only over HTTPS or loopback, exactly like `EmbedderHttp::new`
    /// already guards its own `EMBED_API_KEY` env read
    /// (`crate::retrieval::embedder::is_https_or_loopback`). Without this, a
    /// key arriving from project.toml would leak over cleartext HTTP where the
    /// env-var path does not.
    pub(crate) fn guarded_api_key(url: &str, api_key: Option<&str>) -> Option<String> {
        let key = api_key?;
        if key.is_empty() {
            return None;
        }
        if is_https_or_loopback(url) {
            Some(key.to_string())
        } else {
            tracing::warn!(
                "[embeddings].api_key is set but the embedder url is not HTTPS or loopback; \
                 dropping the key so it is not sent in cleartext. Use an https:// endpoint."
            );
            None
        }
    }

    /// Build the HTTP-backed embedder for a configured `embedder_url`. Split
    /// out of `build_embedder` (which erases to `Arc<dyn CodeEmbedder>`) so a
    /// test can inspect the constructed `EmbedderHttp`'s resolved `api_key`
    /// directly — on the exact code path `build_embedder`/`from_env` run, not
    /// a copy of it.
    fn build_http_embedder(url: &str, config: &RetrievalConfig, dense_only: bool) -> EmbedderHttp {
        let http = EmbedderHttp::new(
            url,
            &config.sparse_embedder_url,
            config
                .model_dim
                .unwrap_or(crate::retrieval::config::DEFAULT_MODEL_DIM),
        )
        .dense_only(dense_only);
        // `[embeddings].api_key` is a separate, project-aware key from the
        // legacy `EMBED_API_KEY` env var that `EmbedderHttp::new` already
        // reads. When set (and not dropped by the cleartext-HTTP guard), it
        // wins; otherwise `new()`'s own env-derived key, if any, stands
        // untouched — `guarded_api_key` returning `None` must never be read
        // as "clear the key".
        match Self::guarded_api_key(url, config.api_key.as_deref()) {
            Some(key) => http.api_key(Some(key)),
            None => http,
        }
    }

    /// Select and build the query-side embedder from `config`: a configured
    /// url always selects the HTTP backend regardless of model —
    /// `create_embedder_with_config` would resolve `RemoteEmbedder` for a url
    /// too, but routing through `EmbedderHttp` here keeps the connect-error
    /// marker `src/tools/semantic/semantic_search.rs` matches on. No url
    /// means the model names the backend: the codescout-embed resolver picks
    /// `local-dir:` / `local:` / `ollama:` / `openai:`.
    ///
    /// Calls `guard_sparse` itself (rather than leaving that to each caller)
    /// so every caller of this function — not just `from_env` — gets the
    /// sparse conflict check for free; `from_env` no longer needs to call it
    /// separately.
    pub(crate) async fn build_embedder(
        config: &RetrievalConfig,
        lite: bool,
    ) -> Result<Arc<dyn CodeEmbedder>> {
        Self::guard_sparse(config, lite)?;
        let dense_only = Self::dense_only(config, lite);
        if let Some(url) = config.embedder_url.as_deref() {
            Ok(Arc::new(Self::build_http_embedder(url, config, dense_only)))
        } else {
            let inner = codescout_embed::create_embedder_with_config(
                &config.model,
                None,
                config.api_key.clone(),
            )
            .await
            .map_err(|e| {
                crate::tools::RecoverableError::with_hint(
                    format!("could not build the '{}' embedder: {e}", config.model),
                    "Set [embeddings].url (or CODESCOUT_EMBEDDER_URL) to an \
                     OpenAI-compatible endpoint, or rebuild with --features local-embed \
                     for in-process ONNX. For a host with no network, point \
                     [embeddings].model at local-dir:/path/to/weights. \
                     If this is a dylib load error, set ORT_DYLIB_PATH to onnxruntime.dll \
                     — and note that an `os error 5` here is application control (e.g. \
                     CyberArk EPM) denying the load, not a missing file: the DLL is \
                     present but not permitted to execute.",
                )
            })?;
            Ok(Arc::new(
                crate::retrieval::embedder::CodeEmbedderAdapter::new(inner, config.model_dim)?,
            ))
        }
    }

    pub async fn from_env(root: Option<&std::path::Path>) -> Result<Self> {
        let config = RetrievalConfig::from_env_and_project(root)?;
        // Backend selection (server Qdrant vs daemon-free sqlite-vec lite stack).
        // sqlite-vec never touches the network — no Qdrant connect probe.
        let backend = VectorBackend::resolve();
        let lite = matches!(backend, VectorBackend::SqliteVec);
        let code_store: Arc<dyn CodeVectorStore> = match backend {
            VectorBackend::SqliteVec => {
                Arc::new(crate::retrieval::sqlite_code_store::SqliteVecCodeStore::from_env()?)
            }
            VectorBackend::Qdrant => Self::qdrant_code_store(&config).await?,
        };
        let embedder = Self::build_embedder(&config, lite).await?;
        let reranker = RerankerHttp::new(&config.reranker_url);
        Ok(Self {
            code_store,
            embedder,
            reranker,
            config,
            lite,
        })
    }

    /// Build the Qdrant-backed code store (server stack).
    #[cfg(feature = "server-stack")]
    async fn qdrant_code_store(config: &RetrievalConfig) -> Result<Arc<dyn CodeVectorStore>> {
        Ok(Arc::new(QdrantWrap::connect(&config.qdrant_url).await?))
    }

    /// Lean build: Qdrant isn't compiled in, so a `qdrant` backend request is a
    /// configuration error pointing at the fix.
    #[cfg(not(feature = "server-stack"))]
    async fn qdrant_code_store(_config: &RetrievalConfig) -> Result<Arc<dyn CodeVectorStore>> {
        anyhow::bail!(
            "CODESCOUT_VECTOR_BACKEND=qdrant requires the `server-stack` build feature. \
             Rebuild with `--features server-stack`, or run the lean lite stack with \
             CODESCOUT_VECTOR_BACKEND=sqlite-vec."
        )
    }

    #[cfg(feature = "server-stack")]
    /// Constructs without connecting to Qdrant — for tests and config validation.
    /// Always the Qdrant (hybrid) shape; the lite stack is constructed via
    /// `from_env` with `CODESCOUT_VECTOR_BACKEND=sqlite-vec`.
    ///
    /// The no-connection claim holds only because the compatibility probe is
    /// disarmed below — under qdrant-client's default it would block on a health
    /// check inside `build()`. Drop that call and this doc comment becomes false.
    pub fn from_config_only(config: RetrievalConfig) -> Self {
        // Keep this constructor's old behaviour explicit rather than letting it
        // inherit new url-vs-model selection semantics by accident — it is
        // always the Qdrant/HTTP-embedder shape, unconditionally.
        let embedder: Arc<dyn CodeEmbedder> = Arc::new(EmbedderHttp::new(
            config
                .embedder_url
                .as_deref()
                .unwrap_or(FALLBACK_EMBEDDER_URL),
            &config.sparse_embedder_url,
            config
                .model_dim
                .unwrap_or(crate::retrieval::config::DEFAULT_MODEL_DIM),
        ));
        let reranker = RerankerHttp::new(&config.reranker_url);
        let client = qdrant_client::Qdrant::from_url(&config.qdrant_url)
            .timeout(std::time::Duration::from_secs(120))
            // See `QdrantWrap::connect` for why this is disarmed at every
            // construction site: the probe `println!`s onto stdout and blocks.
            .skip_compatibility_check()
            .build()
            .expect("invalid qdrant url");
        let code_store: Arc<dyn CodeVectorStore> = Arc::new(QdrantWrap { client });
        Self {
            code_store,
            embedder,
            reranker,
            config,
            lite: false,
        }
    }

    /// `(chunk_count, file_count)` for a project's code index. Delegates to the
    /// code store so external callers (index status, dashboard) don't reach into
    /// the concrete backend.
    pub async fn project_index_stats(
        &self,
        collection: &str,
        project_id: &str,
    ) -> Result<(usize, usize)> {
        self.code_store
            .project_index_stats(collection, project_id)
            .await
    }

    /// Does this project have any indexed chunks? Constant-cost existence check —
    /// use this instead of `project_index_stats(..).0 > 0`, which enumerates the
    /// project to produce counts the caller then throws away.
    pub async fn project_has_chunks(&self, collection: &str, project_id: &str) -> Result<bool> {
        self.code_store
            .project_has_chunks(collection, project_id)
            .await
    }

    /// Fail legibly when the configured embedder disagrees with what the index
    /// already holds. Called at the entry to indexing and to search, which is
    /// the first point `project_id` is known — client construction does not
    /// have one (the sqlite store is per-project).
    pub(crate) async fn guard_index_dim(&self, collection: &str, project_id: &str) -> Result<()> {
        let Some(index_dim) = self
            .code_store
            .collection_dim(collection, project_id)
            .await?
        else {
            return Ok(());
        };
        let model_dim = self.config.model_dim.unwrap_or(index_dim as usize);
        if model_dim as u64 == index_dim {
            return Ok(());
        }
        Err(crate::tools::RecoverableError::with_hint(
            format!(
                "code index was built at {index_dim} dimensions; the configured \
                 embedder produces {model_dim}"
            ),
            "Delete the code index and reindex — the vector table bakes the dimension \
             in at creation and cannot migrate in place. Or set [embeddings].model back \
             to the model the index was built with.",
        )
        .into())
    }

    /// The dimension to size a *fresh* Qdrant collection with when no
    /// `CODESCOUT_MODEL_DIM` pin exists.
    ///
    /// For a local embedding backend, `config.model_dim` is an optional
    /// operator pin — leaving it unset (the common case; local model dims are
    /// self-describing) previously meant [`crate::retrieval::config::DEFAULT_MODEL_DIM`]
    /// (a bare compatibility constant, 768) was baked into a fresh collection
    /// regardless of the model actually configured. `local:AllMiniLML6V2Q` is
    /// 384-dimensional, so that combination created a `memories`/`code_chunks`
    /// collection that rejected every upsert. Resolving the model's own report
    /// is the only value that cannot be stale — the same resolution
    /// `build_embedder`/`CodeEmbedderAdapter::new` perform, paid once more here
    /// because callers of this function (e.g. `Agent::semantic_memory_store`)
    /// have no shared construction path with `Agent::memory_embedder` / a
    /// `RetrievalClient`: both are independently-lazy caches, so there's no
    /// already-built embedder instance to read a dimension off of.
    ///
    /// A remote (HTTP) backend cannot self-report a dimension without a network
    /// round trip, so it keeps the existing pin-or-default fallback.
    pub(crate) async fn resolve_model_dim(config: &RetrievalConfig) -> Result<usize> {
        if !Self::backend_is_local(config) {
            return Ok(config
                .model_dim
                .unwrap_or(crate::retrieval::config::DEFAULT_MODEL_DIM));
        }
        let inner = codescout_embed::create_embedder_with_config(
            &config.model,
            None,
            config.api_key.clone(),
        )
        .await
        .map_err(|e| {
            crate::tools::RecoverableError::with_hint(
                format!(
                    "could not build the '{}' embedder to size its collection: {e}",
                    config.model
                ),
                "Rebuild with --features local-embed for in-process ONNX, or point \
                         [embeddings].model at local-dir:/path/to/weights for an offline host.",
            )
        })?;
        Ok(inner.dimensions())
    }
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    /// Build a `RetrievalConfig` from ambient env (read, never mutated) and then
    /// override exactly the fields under test. This is what makes these tests
    /// immune to whatever `CODESCOUT_*` / `EMBED_API_KEY` happens to be set in
    /// the ambient environment without `EnvGuard`/`#[serial]` — both banned
    /// crate-wide by `docs/conventions/test-env-isolation.md`. Every field the
    /// assertions below depend on is set explicitly after construction, so the
    /// ambient values that survive are ones none of these tests read.
    fn cfg_with(url: Option<&str>, model: &str) -> RetrievalConfig {
        let mut c = RetrievalConfig::from_env_and_project(None).unwrap();
        c.embedder_url = url.map(|s| s.to_string());
        c.model = model.to_string();
        c
    }

    #[test]
    fn explicit_url_selects_the_http_backend_regardless_of_model() {
        let c = cfg_with(Some("http://127.0.0.1:8081/v1"), "local:AllMiniLML6V2Q");
        assert!(!RetrievalClient::backend_is_local(&c));
    }

    #[test]
    fn no_url_with_a_local_model_selects_the_local_backend() {
        let c = cfg_with(None, "local-dir:/weights");
        assert!(RetrievalClient::backend_is_local(&c));
    }

    #[test]
    fn no_url_with_a_remote_style_model_does_not_select_the_local_backend() {
        let c = cfg_with(None, "openai:text-embedding-3-small");
        assert!(!RetrievalClient::backend_is_local(&c));
    }

    #[test]
    fn local_backend_with_sparse_expected_is_an_error() {
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = false; // explicit field override — ambient env cannot leak in
        let err = RetrievalClient::guard_sparse(&c, /* lite */ false).unwrap_err();
        // Class, not just message: `anyhow::bail!` with equivalent wording
        // would pass a `.to_string().contains("sparse")` check too (measured:
        // the full suite stays green under that revert) but produces
        // `isError: true`, aborting sibling parallel tool calls for what is
        // an expected, operator-fixable config conflict.
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false), not anyhow::bail!; got: {err}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("sparse"),
            "must name sparse as the conflict, got: {msg}"
        );
    }

    #[test]
    fn local_backend_with_disable_sparse_is_not_an_error() {
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = true;
        assert!(RetrievalClient::guard_sparse(&c, /* lite */ false).is_ok());
    }

    #[test]
    fn local_backend_in_the_lite_stack_is_not_an_error() {
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = false;
        assert!(RetrievalClient::guard_sparse(&c, /* lite */ true).is_ok());
    }

    #[test]
    fn http_backend_with_sparse_expected_is_not_an_error() {
        let mut c = cfg_with(Some("http://127.0.0.1:8081"), "local-dir:/weights");
        c.disable_sparse = false;
        assert!(RetrievalClient::guard_sparse(&c, false).is_ok());
    }

    #[test]
    fn dense_only_is_true_for_a_local_backend_even_without_lite_or_disable_sparse() {
        // `from_env` only ever calls `dense_only` after `guard_sparse` has
        // already required `lite || disable_sparse` to hold whenever the
        // backend is local — so this exact state never reaches `dense_only`
        // through that caller. Testing `dense_only` directly, independent of
        // that caller invariant, is what catches a future caller (or a
        // mutation) that drops the `backend_is_local` term: nothing else
        // here would ever observe the drop.
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = false;
        assert!(RetrievalClient::dense_only(&c, /* lite */ false));
    }

    #[test]
    fn dense_only_is_false_for_a_hybrid_http_backend() {
        let mut c = cfg_with(Some("http://127.0.0.1:8081"), "local-dir:/weights");
        c.disable_sparse = false;
        assert!(!RetrievalClient::dense_only(&c, /* lite */ false));
    }

    #[test]
    fn guarded_api_key_sends_the_key_over_https() {
        assert_eq!(
            RetrievalClient::guarded_api_key("https://embed.example.com", Some("secret")),
            Some("secret".to_string())
        );
    }

    #[test]
    fn guarded_api_key_sends_the_key_over_loopback_http() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://127.0.0.1:8081", Some("secret")),
            Some("secret".to_string())
        );
    }

    #[test]
    fn guarded_api_key_drops_the_key_over_plaintext_non_loopback_http() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://embed.example.com", Some("secret")),
            None
        );
    }

    #[test]
    fn guarded_api_key_is_none_when_no_key_is_configured() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://embed.example.com", None),
            None
        );
    }

    #[test]
    fn build_http_embedder_never_sends_a_configured_key_over_plaintext_http() {
        // Binds `guarded_api_key` to the call site inside `build_http_embedder`
        // (the code `build_embedder`/`from_env` actually run), not just to the
        // pure function in isolation. Mutating the call site to
        // `config.api_key.clone()` (skipping the guard) makes this assert
        // `Some("secret")` instead of `None` — dies.
        let mut c = cfg_with(Some("http://embed.example.com"), "local-dir:/weights");
        c.api_key = Some("secret".to_string());
        let http = RetrievalClient::build_http_embedder("http://embed.example.com", &c, false);
        assert_eq!(
            http.api_key_for_test(),
            None,
            "a configured api_key must never reach a non-loopback plaintext HTTP embedder"
        );
    }

    #[tokio::test]
    async fn build_embedder_errors_for_a_local_backend_with_sparse_still_enabled() {
        // Binds `guard_sparse` to the call site inside `build_embedder` (what
        // `from_env` actually runs), not just to the pure function in
        // isolation. Deleting the `Self::guard_sparse(config, lite)?;` call
        // lets execution fall through to the resolver, which fails for an
        // unrelated reason (no weights at "/weights") — an error that does
        // NOT contain "sparse". Asserting the specific cause, not just
        // `is_err()`, is what makes this discriminate between the two.
        //
        // `Arc<dyn CodeEmbedder>` (the Ok type) isn't `Debug`, so
        // `unwrap_err()`/`expect_err()` don't compile here — match instead.
        let mut c = cfg_with(None, "local-dir:/weights");
        c.disable_sparse = false;
        let err = match RetrievalClient::build_embedder(&c, /* lite */ false).await {
            Ok(_) => panic!("expected an error for a local backend with sparse enabled"),
            Err(e) => e,
        };
        // Class, not just message — see the sibling pure-function test's
        // comment: an `anyhow::bail!` with equivalent wording passes the
        // message check too, but the wrong error class is a bug on its own
        // (isError: true aborts sibling parallel tool calls for a config
        // conflict that should be retryable).
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false), not anyhow::bail!; got: {err}"
        );
        let msg = err.to_string();
        assert!(
            msg.contains("sparse"),
            "must name sparse as the conflict, got: {msg}"
        );
    }

    #[tokio::test]
    async fn resolve_model_dim_uses_the_pin_or_default_for_a_remote_backend() {
        // An HTTP backend cannot self-report a dimension without a network
        // round trip, so `resolve_model_dim` must not attempt to build a local
        // embedder for it — it should fall straight back to the
        // pin-or-default path `ensure_collection` callers used before this
        // function existed.
        let mut pinned = cfg_with(Some("http://unused.invalid"), "some-remote-model");
        pinned.model_dim = Some(42);
        let dim = RetrievalClient::resolve_model_dim(&pinned).await.unwrap();
        assert_eq!(
            dim, 42,
            "a remote backend's explicit pin must be honoured verbatim"
        );

        let mut unpinned = cfg_with(Some("http://unused.invalid"), "some-remote-model");
        unpinned.model_dim = None;
        let dim = RetrievalClient::resolve_model_dim(&unpinned).await.unwrap();
        assert_eq!(
            dim,
            crate::retrieval::config::DEFAULT_MODEL_DIM,
            "an unpinned remote backend keeps the compatibility default"
        );
    }

    /// Regression for the memories-collection bug named in the task 8 brief:
    /// with `CODESCOUT_MODEL_DIM` unset (the plan's own headline
    /// configuration), the old fallback was `DEFAULT_MODEL_DIM` (768)
    /// regardless of which model was actually configured — wrong for
    /// `local:AllMiniLML6V2Q`, which is 384-dimensional, so a fresh
    /// server-stack `memories` collection was created at the wrong size and
    /// rejected every upsert. `resolve_model_dim` must report the model's own
    /// value instead.
    #[cfg(feature = "local-embed")]
    #[tokio::test]
    async fn resolve_model_dim_reports_the_local_models_own_dimension() {
        if std::env::var("CODESCOUT_SKIP_ONNX_TESTS").is_ok() {
            eprintln!(
                "SKIP resolve_model_dim_reports_the_local_models_own_dimension: opt-out is set"
            );
            return;
        }
        let mut c = cfg_with(None, "local:AllMiniLML6V2Q");
        c.model_dim = None;
        let dim = RetrievalClient::resolve_model_dim(&c).await.unwrap();
        assert_eq!(
            dim, 384,
            "must report the model's own dimension, not the 768 compatibility default"
        );
    }
}
