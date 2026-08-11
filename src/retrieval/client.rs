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
    /// show up as degraded recall and never as a failure, so it is an error.
    pub(crate) fn guard_sparse(config: &RetrievalConfig, lite: bool) -> Result<()> {
        if Self::backend_is_local(config) && !lite && !config.disable_sparse {
            anyhow::bail!(
                "the local embedding backend produces no sparse vector, but the hybrid \
                 sparse leg is enabled.\n\
                 Either set CODESCOUT_DISABLE_SPARSE=1 to run dense-only, or configure \
                 an embedder url that serves both dense and sparse."
            );
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
        Self::guard_sparse(&config, lite)?;
        let dense_only = Self::dense_only(&config, lite);
        // A configured url always selects the HTTP backend, regardless of
        // model — `create_embedder_with_config` would resolve `RemoteEmbedder`
        // for a url too, but routing through `EmbedderHttp` here keeps the
        // connect-error marker `src/tools/semantic/semantic_search.rs` matches
        // on. No url means the model names the backend: the codescout-embed
        // resolver picks `local-dir:` / `local:` / `ollama:` / `openai:`.
        let embedder: Arc<dyn CodeEmbedder> = if let Some(url) = config.embedder_url.as_deref() {
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
            // reads. When set (and not dropped by the cleartext-HTTP guard),
            // it wins; otherwise `new()`'s own env-derived key, if any, stands
            // untouched — `guarded_api_key` returning `None` must never be
            // read as "clear the key".
            let http = match Self::guarded_api_key(url, config.api_key.as_deref()) {
                Some(key) => http.api_key(Some(key)),
                None => http,
            };
            Arc::new(http)
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
            Arc::new(crate::retrieval::embedder::CodeEmbedderAdapter::new(
                inner,
                config.model_dim,
            )?)
        };
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
        let err = RetrievalClient::guard_sparse(&c, /* lite */ false)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("sparse"),
            "must name sparse as the conflict, got: {err}"
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
}
