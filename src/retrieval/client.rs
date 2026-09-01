use crate::retrieval::code_store::{CodeVectorStore, VectorBackend};
use crate::retrieval::config::RetrievalConfig;
use crate::retrieval::embedder::CodeEmbedder;
#[cfg(feature = "remote-embed")]
use crate::retrieval::embedder::EmbedderHttp;
#[cfg(feature = "server-stack")]
use crate::retrieval::qdrant::QdrantWrap;
#[cfg(feature = "remote-embed")]
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

/// Model prefixes that name a **network** backend — arms 3 and 4 of
/// `create_embedder_with_config`, both gated on the `remote-embed` feature.
///
/// `pub(crate)` because `ProjectStatus` needs this same list to tell whether a
/// config names a remote backend the binary cannot build. One definition, two
/// callers asking different questions of it.
pub(crate) const REMOTE_MODEL_PREFIXES: [&str; 2] = ["ollama:", "openai:"];

/// Arm 5's prefix: a hard error carrying a migration hint, neither local nor
/// remote. Named so it cannot fall through the bare-name branch and be mistaken
/// for a local model name.
const HARD_ERROR_MODEL_PREFIX: &str = "custom:";

/// Does this model string name a network backend?
pub(crate) fn model_names_remote_backend(model: &str) -> bool {
    REMOTE_MODEL_PREFIXES.iter().any(|p| model.starts_with(p))
}

/// Does this model string select the in-process local backend?
///
/// Mirrors `create_embedder_with_config`'s resolution ladder
/// (`crates/codescout-embed/src/lib.rs`) on the no-url path. Two ways to select
/// local, and the second one is the fix for
/// `docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md`:
///
/// 1. An explicit `local:` / `local-dir:` prefix — arm 2.
/// 2. A **bare** name, when a local backend is compiled in — arm 6 resolves it
///    as a local ONNX model. Nothing else can be built for such a string, so
///    "local or nothing" collapses to local for every consumer's purposes.
///
/// Case 2 used to be missing, and the cost was not cosmetic. `guard_sparse` and
/// `dense_only` both read this, so a bare-name local config had its hybrid
/// sparse leg left enabled against an embedder that emits no sparse vector —
/// exactly the silent recall degradation `guard_sparse` exists to turn into a
/// loud, operator-fixable error. `resolve_model_dim` likewise sized fresh
/// collections from the default dimension instead of asking the real model.
///
/// # What this deliberately does NOT do
///
/// It does not consult compiled features for the *remote* prefixes. On a
/// `--no-default-features` build an `ollama:` model has no constructible arm at
/// all, and `create_embedder_with_config` bails with "Unknown model" — the right
/// outcome. Reporting such a config as local would raise `guard_sparse`'s
/// "local backend produces no sparse vector" error, which is a false
/// explanation for a config that cannot build anything. That residual is
/// status-string-only and stays recorded in the bug file.
///
/// # Accepted cost
///
/// A bare name that is *not* a real local model (a typo) now trips
/// `guard_sparse` before construction reports "Unknown model", so diagnosing a
/// typo can take two steps instead of one. That is worth it: a typo fails either
/// way, whereas the config this fixes was working and silently degraded.
fn model_names_local_backend(model: &str) -> bool {
    if model.starts_with("local:") || model.starts_with("local-dir:") {
        return true;
    }
    // Arm 6 only exists when a local backend is compiled in. These are this
    // crate's features, which forward to codescout-embed's own — the switch an
    // operator actually flips.
    if !cfg!(any(
        feature = "local-embed",
        feature = "local-embed-dynamic"
    )) {
        return false;
    }
    !model_names_remote_backend(model) && !model.starts_with(HARD_ERROR_MODEL_PREFIX)
}

pub struct RetrievalClient {
    /// Code-chunk vector store behind the `CodeVectorStore` seam. Qdrant today;
    /// in-process sqlite-vec in the lite stack (Phase 2). `pub(crate)` so the
    /// sibling `search`/`sync` modules can reach it without exposing it outside
    /// the crate. See `docs/plans/archive/2026-06-16-two-stack-retrieval-lite.md`.
    pub(crate) code_store: Arc<dyn CodeVectorStore>,
    pub embedder: Arc<dyn CodeEmbedder>,
    #[cfg(feature = "remote-embed")]
    pub reranker: RerankerHttp,
    pub config: RetrievalConfig,
    /// True for the daemon-free lite stack (sqlite-vec backend): dense-only, no
    /// reranker server. Gates the rerank step in `search_in`.
    pub(crate) lite: bool,
}

impl RetrievalClient {
    /// A local backend is selected when no url is configured and the model
    /// names one. Keep this the single source of truth — `dense_only`, the
    /// sparse guard, `resolve_model_dim`, and `ProjectStatus` all read it.
    ///
    /// Note the `is_none()`: this answers "is the local backend SELECTED", so it
    /// is false precisely when a url also exists. `guard_local_model_with_url`
    /// therefore cannot reuse it — it must see the model's intent through the url
    /// that overrides it.
    pub(crate) fn backend_is_local(config: &RetrievalConfig) -> bool {
        config.embedder_url.is_none() && model_names_local_backend(&config.model)
    }

    /// A url and a `local-dir:` model are contradictory: the url selects a network
    /// client, the prefix forces in-process ONNX against on-disk weights.
    /// `codescout-embed` already treats the combination as fatal
    /// (`create_embedder_with_config`, crates/codescout-embed/src/lib.rs), but
    /// `build_embedder` short-circuits on the url before ever reaching that
    /// resolver — so without this guard the model is discarded in silence and
    /// embedding goes over the network.
    ///
    /// That silence is the whole defect: `local-dir:` exists so a restricted host
    /// embeds WITHOUT touching the network, and a stale url (this repo's own
    /// `~/.config/codescout/.env` supplies one) turned that guarantee into its
    /// opposite with exit 0 and no warning. Fail closed, not open.
    ///
    /// **`local:` is deliberately NOT covered.** `default_embed_model()` is
    /// `"local:AllMiniLML6V2Q"` (src/config/project.rs), so "url set, model unset"
    /// — an ordinary remote deployment — resolves to a `local:` model that the
    /// operator never chose. Rejecting that would break every such deployment, and
    /// the config carries no provenance that would distinguish it from a deliberate
    /// `local:`. `local-dir:` has no default, so it is unambiguously intent. Covering
    /// `local:` too would require threading "was this the default?" through
    /// `merge_embed_config` into `RetrievalConfig`; until then a url still silently
    /// wins over an explicitly-chosen `local:` model.
    /// (Learned the hard way: the wider guard failed
    /// `agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`, which
    /// built from a root-less config and so got exactly that default. That test now
    /// supplies its own project config — it had to, because the root-less path made it
    /// read the developer's environment and held CI red for a week — so it would no
    /// longer be the thing that catches a widening. The reasoning above stands on its
    /// own: the default is indistinguishable from a chosen `local:` without provenance.)
    ///
    /// `RecoverableError` for the same reason as `guard_sparse`: an operator-fixable
    /// config conflict, not a bug — isError: false, so sibling parallel calls survive.
    pub(crate) fn guard_local_model_with_url(config: &RetrievalConfig) -> Result<()> {
        if let Some(url) = config.embedder_url.as_deref() {
            if config.model.starts_with("local-dir:") {
                return Err(crate::tools::RecoverableError::with_hint(
                    format!(
                        "embedder url '{url}' is configured alongside the offline model \
                         '{}', but a url selects a network client while local-dir: \
                         forces in-process ONNX against on-disk weights.",
                        config.model
                    ),
                    "Drop the url to use the local weights (CODESCOUT_EMBEDDER_URL= , or \
                     remove [embeddings].url), or drop the local-dir: prefix to use the \
                     url. Note that a url may arrive from the startup dotenv at \
                     ~/.config/codescout/.env even when it is absent from your shell — \
                     set CODESCOUT_ENV_FILE to a nonexistent path to rule that out.",
                )
                .into());
            }
        }
        Ok(())
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
    /// (`codescout_embed::remote::is_https_or_loopback` — both callers share the
    /// crate's copy since T7 deleted root's). Without this, a key arriving from
    /// project.toml would leak over cleartext HTTP where the env-var path does not.
    #[cfg(feature = "remote-embed")]
    pub(crate) fn guarded_api_key(url: &str, api_key: Option<&str>) -> Option<String> {
        let key = api_key?;
        if key.is_empty() {
            return None;
        }
        if codescout_embed::remote::is_https_or_loopback(url) {
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
    #[cfg(feature = "remote-embed")]
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

    /// The `[embeddings].url` arm of [`Self::build_embedder`], split out so a
    /// build with no HTTP embed transport refuses the configuration with an
    /// actionable message instead of failing to compile. Mirrors the lean arm of
    /// [`Self::qdrant_code_store`], which names its missing feature the same way.
    #[cfg(feature = "remote-embed")]
    fn build_embedder_for_url(
        url: &str,
        config: &RetrievalConfig,
        dense_only: bool,
    ) -> Result<Arc<dyn CodeEmbedder>> {
        Ok(Arc::new(Self::build_http_embedder(url, config, dense_only)))
    }

    #[cfg(not(feature = "remote-embed"))]
    fn build_embedder_for_url(
        _url: &str,
        _config: &RetrievalConfig,
        _dense_only: bool,
    ) -> Result<Arc<dyn CodeEmbedder>> {
        anyhow::bail!(
            "an embedder url is configured, but this build has no HTTP embed \
             transport. Rebuild with --features remote-embed, or unset \
             [embeddings].url (CODESCOUT_EMBEDDER_URL) so the model name selects \
             an in-process backend."
        )
    }

    /// Select and build the query-side embedder from `config`: a configured
    /// url selects the HTTP backend for any model that does not name a local
    /// backend — `create_embedder_with_config` would resolve `RemoteEmbedder`
    /// for a url too, but `EmbedderHttp` is the **hybrid** path, and the sparse
    /// leg has no crate equivalent.
    ///
    /// That is the whole of the reason now. This comment used to add "routing
    /// through `EmbedderHttp` here keeps the connect-error marker
    /// `src/tools/semantic/semantic_search.rs` matches on", and that constraint is
    /// gone: `EmbedError::Connect`/`::Status` render published markers the
    /// classifier imports (T4, T6 step A/B), and since T6 step D `EmbedderHttp`'s
    /// dense leg *is* `RemoteEmbedder` — so the two producers cannot diverge in
    /// the way the sentence was guarding against. It is recorded rather than
    /// merely deleted because it is what deferred the swap for a month.
    /// No url means the model names the backend: the codescout-embed resolver
    /// picks `local-dir:` / `local:` / `ollama:` / `openai:`.
    ///
    /// A url combined with a `local-dir:` model is neither of those cases but a
    /// contradiction, and is rejected by `guard_local_model_with_url` before the
    /// url branch is reached. It used to be silently resolved in the url's favour,
    /// which defeated the offline guarantee the prefix exists to provide.
    ///
    /// `local:` is deliberately NOT rejected — it is what "url set, model unset"
    /// resolves to, so rejecting it would break every ordinary remote deployment.
    /// This line said `local:` / `local-dir:` until 2026-08-26 and misled a reader
    /// into predicting a rejection that cannot happen; the authoritative account of
    /// the asymmetry, and of what covering `local:` would cost, is on
    /// [`Self::guard_local_model_with_url`] itself.
    ///
    /// Calls `guard_sparse` itself (rather than leaving that to each caller)
    /// so every caller of this function — not just `from_env` — gets the
    /// sparse conflict check for free; `from_env` no longer needs to call it
    /// separately.
    pub(crate) async fn build_embedder(
        config: &RetrievalConfig,
        lite: bool,
    ) -> Result<Arc<dyn CodeEmbedder>> {
        // Must precede the url branch below: that branch takes the HTTP backend
        // regardless of model, which is exactly what silently discards a local
        // model. Checking after it would be unreachable.
        Self::guard_local_model_with_url(config)?;
        Self::guard_sparse(config, lite)?;
        let dense_only = Self::dense_only(config, lite);
        if let Some(url) = config.embedder_url.as_deref() {
            Self::build_embedder_for_url(url, config, dense_only)
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
                Arc::new(crate::retrieval::sqlite_code_store::SqliteVecCodeStore::at(
                    config.sqlite_dir.clone(),
                ))
            }
            VectorBackend::Qdrant => Self::qdrant_code_store(&config).await?,
        };
        let embedder = Self::build_embedder(&config, lite).await?;
        #[cfg(feature = "remote-embed")]
        let reranker = RerankerHttp::new(&config.reranker_url);
        Ok(Self {
            code_store,
            embedder,
            #[cfg(feature = "remote-embed")]
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
        let model_dim = self.effective_model_dim(index_dim as usize);
        if model_dim == index_dim {
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

    /// Force-aware front end to [`RetrievalClient::guard_index_dim`]. Returns
    /// `Some((from, to))` when it migrated, `None` when there was nothing to do.
    ///
    /// - `force == false` — the guard, unchanged. A dimension mismatch is an error.
    /// - `force == true` — the mismatch is the *reason* for the rebuild, so discard
    ///   this project's index and let the caller recreate it at the configured
    ///   width.
    ///
    /// A separate method rather than a `force: bool` parameter on the guard,
    /// because the guard's other caller is `search_in`
    /// (`src/retrieval/search.rs`), where "force" is meaningless and the flag would
    /// be threaded through as a permanent `false`.
    ///
    /// Before this existed, `sync_project` called the guard unconditionally *ahead*
    /// of the force-capable indexing work, so `force=true` — which advertises a
    /// full reindex — could not perform the one rebuild that actually requires
    /// one:
    /// `docs/issues/archive/2026-08-26-force-reindex-cannot-migrate-embedding-dimensions.md`.
    pub(crate) async fn migrate_or_guard_index_dim(
        &self,
        collection: &str,
        project_id: &str,
        force: bool,
    ) -> Result<Option<(u64, u64)>> {
        if !force {
            self.guard_index_dim(collection, project_id).await?;
            return Ok(None);
        }
        // Nothing indexed yet — no dimension to disagree with, so a forced build is
        // an ordinary first build.
        let Some(index_dim) = self
            .code_store
            .collection_dim(collection, project_id)
            .await?
        else {
            return Ok(None);
        };
        let model_dim = self.effective_model_dim(index_dim as usize);
        if model_dim == index_dim {
            return Ok(None);
        }
        // Loud on purpose: this discards a real index. `force=true` authorises the
        // rebuild, but the operator asked for a reindex, not necessarily for a model
        // migration, and the two are only distinguishable from here.
        tracing::warn!(
            project_id,
            collection,
            from = index_dim,
            to = model_dim,
            "force reindex is migrating the vector table to a new embedding \
             dimension — the existing index is discarded and rebuilt"
        );
        self.code_store
            .reset_project_index(collection, project_id)
            .await?;
        Ok(Some((index_dim, model_dim)))
    }

    /// The dimension to validate or size an index against, for the embedder
    /// this client actually holds (`self.embedder`) — not a config value that
    /// can default to whatever it's being compared against.
    ///
    /// Priority: the embedder's own report (`CodeEmbedder::known_dim`) when it
    /// has one — true for a `local:`/`local-dir:` backend (self-describes at
    /// construction), false for `EmbedderHttp` and false for a
    /// `CodeEmbedderAdapter` wrapping `RemoteEmbedder` (`ollama:`/`openai:`
    /// with no url) until its first successful embed populates its cache —
    /// then the operator's `CODESCOUT_MODEL_DIM` pin, then `fallback`.
    ///
    /// This is the fix for a real defect found in review: the previous
    /// `guard_index_dim` compared against `self.config.model_dim.unwrap_or(index_dim)`
    /// directly — when unpinned (the common case; `RetrievalConfig.model_dim`'s
    /// own doc calls `None` "the model is the authority"), `model_dim` *became*
    /// `index_dim` by construction and the comparison could never fail. That is
    /// silently inert in exactly the scenario this plan exists to enable: an
    /// index built at 768 by a remote model, switched to an unpinned
    /// `local:AllMiniLML6V2Q` (384) — the guard passed, and the mismatch surfaced
    /// only later, mid-operation. Reading the embedder's own `known_dim()` first
    /// fixes this for local backends without paying a second model load: unlike
    /// `resolve_model_dim` (which callers with no already-built embedder use, at
    /// the cost of constructing a throwaway one), this reads the *live*
    /// `self.embedder` this client already holds.
    ///
    /// For a remote backend (`known_dim()` is `None`, genuinely unknowable
    /// without a network round trip), this keeps the historical pin-or-`fallback`
    /// behaviour — callers pass `index_dim` as `fallback` (trust the index when
    /// nothing else is known, avoiding a false positive against a correctly
    /// configured but unpinned, non-`DEFAULT_MODEL_DIM` remote model) or
    /// `DEFAULT_MODEL_DIM` (the compatibility constant, for sizing a fresh
    /// collection where there is no index to trust yet).
    ///
    /// `pub(crate)`, not private: `sync_project`'s body lives in the sibling
    /// `sync` module, and Rust's method privacy is module-scoped, not
    /// type-scoped — a bare `fn` here would not compile from there.
    pub(crate) fn effective_model_dim(&self, fallback: usize) -> u64 {
        self.embedder
            .known_dim()
            .or(self.config.model_dim)
            .unwrap_or(fallback) as u64
    }

    // `server-stack` is not a default feature (Cargo.toml `default = [...]` omits
    // it), so a bare `cargo clippy -- -D warnings` compiles this crate WITHOUT it —
    // and this function's only production caller (`Agent::semantic_memory_store`'s
    // Qdrant branch) is itself `#[cfg(feature = "server-stack")]`. Without the
    // `test` arm here, that default-features build reports `resolve_model_dim` as
    // dead code and the mandated gate (CLAUDE.md; `.github/workflows/ci.yml:50`)
    // fails on every commit, not just an unusual feature combination. The `test`
    // arm keeps the function AND its tests compiling — and, critically, *executing*
    // — under `cargo test --workspace --features local-embed` (no `server-stack`),
    // which is the actual test-running gate command.
    #[cfg(any(feature = "server-stack", test))]
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
    ///
    /// Policy note (minor, review round-2): unlike `CodeEmbedderAdapter::new`,
    /// this does NOT treat a local model's real dimension disagreeing with
    /// `config.model_dim` as a hard error — it just reports the model's own
    /// value and moves on. That's deliberate, not an oversight: this function
    /// only sizes a *fresh* collection before any real embedder has been
    /// constructed. The same disagreement is still caught, as a hard
    /// `RecoverableError`, the moment a real embedder IS built —
    /// `Agent::memory_embedder()` → `RetrievalClient::build_embedder` →
    /// `CodeEmbedderAdapter::new` — which happens on the very next
    /// remember/recall call (`src/tools/memory/mod.rs` calls
    /// `memory_embedder()` before `semantic_memory_store()`). So a wrong pin
    /// is never silently accepted; it's validated a moment later, on the path
    /// that actually enforces it.
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

    /// Regression: a **bare** model name with no url selects the local backend,
    /// because `create_embedder_with_config`'s arm 6 resolves it as local ONNX
    /// and nothing else can be built for such a string.
    ///
    /// The assertions past the predicate are the point. `backend_is_local` is
    /// read by `guard_sparse` and `dense_only`, so before this fix a bare-name
    /// local config kept the hybrid sparse leg enabled against an embedder that
    /// emits no sparse vector — the silent recall loss `guard_sparse` exists to
    /// make loud. Asserting only the predicate would not catch a future caller
    /// that stops consulting it.
    /// docs/issues/archive/2026-08-11-project-status-backend-misreports-bare-model-and-lean-build.md
    #[cfg(any(feature = "local-embed", feature = "local-embed-dynamic"))]
    #[test]
    fn no_url_with_a_bare_model_name_selects_the_local_backend_when_compiled_in() {
        let mut c = cfg_with(None, "AllMiniLML6V2Q");
        c.disable_sparse = false;
        assert!(
            RetrievalClient::backend_is_local(&c),
            "arm 6 resolves a bare name locally, so the classifier must agree"
        );
        assert!(
            RetrievalClient::dense_only(&c, /* lite */ false),
            "a local embedder emits no sparse vector"
        );
        assert!(
            RetrievalClient::guard_sparse(&c, /* lite */ false).is_err(),
            "the sparse conflict must be raised, not silently degraded"
        );
    }

    /// The other side of the same cfg: with no local backend compiled, arm 6
    /// does not exist, so a bare name selects nothing local — construction bails
    /// with "Unknown model" instead, and the sparse guard must stay quiet rather
    /// than blame a local backend that cannot exist.
    #[cfg(not(any(feature = "local-embed", feature = "local-embed-dynamic")))]
    #[test]
    fn no_url_with_a_bare_model_name_selects_nothing_local_without_a_local_backend() {
        let mut c = cfg_with(None, "AllMiniLML6V2Q");
        c.disable_sparse = false;
        assert!(!RetrievalClient::backend_is_local(&c));
        assert!(RetrievalClient::guard_sparse(&c, /* lite */ false).is_ok());
    }

    /// Pins the deliberate non-fix: a remote-prefixed model with no url is never
    /// classified local, whatever is compiled. On a `--no-default-features` build
    /// nothing can be built for it and `create_embedder_with_config` bails with
    /// "Unknown model" — the right error. Calling it local would instead raise
    /// `guard_sparse`'s "local backend produces no sparse vector", a false
    /// explanation. That residual is status-string-only and stays open.
    #[test]
    fn a_remote_prefixed_model_is_never_local_regardless_of_compiled_features() {
        for model in ["ollama:nomic-embed-text", "openai:text-embedding-3-small"] {
            let c = cfg_with(None, model);
            assert!(
                !RetrievalClient::backend_is_local(&c),
                "{model} must not be classified local"
            );
        }
    }

    /// `custom:` is a hard-error prefix (arm 5), not a local one — it must not
    /// fall through the bare-name branch into a local classification.
    #[test]
    fn the_custom_prefix_is_not_classified_local() {
        let c = cfg_with(None, "custom:whatever");
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

    /// The lean build's load-bearing invariant, first half:
    /// `!cfg(server-stack)` ⟹ every constructible client has `lite == true`.
    ///
    /// Two constructors exist. `from_config_only` is itself
    /// `#[cfg(feature = "server-stack")]`, so in a lean build it does not exist at
    /// all — a fact this file's compilation asserts more strongly than any runtime
    /// check could. That leaves `from_env`, which derives `lite` from the resolved
    /// backend and reaches `qdrant_code_store` for the only non-lite one. This test
    /// pins that call refusing, which is what closes the last door.
    ///
    /// Why it matters beyond tidiness: the consolidation in
    /// `docs/plans/2026-07-25-embedding-transport-consolidation.md` gates the sparse
    /// leg and the reranker behind `server-stack`, and argues the gate is a runtime
    /// no-op *because* a lean build can never take either path. That argument IS this
    /// invariant. Until now three files had to agree for it to hold and nothing made
    /// them fail together. See `resume-embedding-transport-stages-1-3:ET-1`.
    #[cfg(not(feature = "server-stack"))]
    #[tokio::test]
    async fn a_lean_build_cannot_construct_a_non_lite_client() {
        let c = cfg_with(Some("http://127.0.0.1:8081"), "text-embedding-3-small");
        // `map(|_| ())` discards the Ok side purely so `expect_err` has a `Debug`
        // bound to satisfy — `Arc<dyn CodeVectorStore>` has none. The panic message
        // is the point; the value never survives to be read.
        let err = RetrievalClient::qdrant_code_store(&c)
            .await
            .map(|_| ())
            .expect_err("a lean build must refuse the qdrant backend, not build one");
        assert!(
            err.to_string().contains("server-stack"),
            "the refusal has to name the missing feature for an operator to act on it; got: {err}"
        );
    }

    /// The same invariant's second half: given `lite`, the sparse leg and the
    /// reranker are both unreachable regardless of configuration.
    ///
    /// Asserted against a config that is otherwise fully hybrid — explicit HTTP
    /// embedder url, sparse enabled, reranking wanted by caller *and* operator — and
    /// each half is preceded by its own negative guard. Without those guards the test
    /// would pass just as happily against a config that was never hybrid to begin
    /// with, which is the vacuous shape this repo keeps finding.
    #[test]
    fn lite_alone_forces_dense_only_and_vetoes_the_reranker() {
        use crate::retrieval::search::should_rerank;

        let mut c = cfg_with(Some("http://127.0.0.1:8081"), "text-embedding-3-small");
        c.disable_sparse = false;

        assert!(
            !RetrievalClient::dense_only(&c, /* lite */ false),
            "guard: without lite this config must be hybrid, or the next assertion proves nothing"
        );
        assert!(
            RetrievalClient::dense_only(&c, /* lite */ true),
            "lite alone must force dense-only"
        );

        assert!(
            should_rerank(true, true, /* lite */ false, 10),
            "guard: without lite this call must rerank, or the next assertion proves nothing"
        );
        assert!(
            !should_rerank(true, true, /* lite */ true, 10),
            "lite alone must veto the reranker even when caller and operator both want it"
        );
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn guarded_api_key_sends_the_key_over_https() {
        assert_eq!(
            RetrievalClient::guarded_api_key("https://embed.example.com", Some("secret")),
            Some("secret".to_string())
        );
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn guarded_api_key_sends_the_key_over_loopback_http() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://127.0.0.1:8081", Some("secret")),
            Some("secret".to_string())
        );
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn guarded_api_key_drops_the_key_over_plaintext_non_loopback_http() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://embed.example.com", Some("secret")),
            None
        );
    }

    /// The spoofed-host cases, asserted at **root's own layer**.
    ///
    /// Every other guard test here uses `http://embed.example.com` — a plainly
    /// non-loopback host that even an unanchored `contains("localhost")` check
    /// would reject. These are the inputs that separate a correct guard from a
    /// merely plausible one, and until T7 they were asserted only against root's
    /// private copy of the predicate, one layer below where a key actually leaks.
    ///
    /// Kept at root rather than left to `codescout-embed`'s own predicate test
    /// because the claim is different: not "the predicate is right" but "root
    /// drops the key", which is the thing an operator is exposed to. That
    /// distinction is why deleting root's duplicated predicate test in T7 costs
    /// nothing — this covers the same inputs against the behaviour that matters.
    #[cfg(feature = "remote-embed")]
    #[test]
    fn guarded_api_key_drops_the_key_for_a_host_that_only_looks_like_loopback() {
        for spoof in [
            "http://127.evil.com/v1",
            "http://localhost.evil.com/v1",
            "http://127.0.0.1.evil.com/v1",
            "http://example.com/127.0.0.1",
            // Userinfo form: the host is `evil.com` and `127.0.0.1` is a
            // username. A guard that searches the url rather than parsing the
            // authority accepts this and sends the key to evil.com in cleartext.
            "http://127.0.0.1@evil.com/v1",
        ] {
            assert_eq!(
                RetrievalClient::guarded_api_key(spoof, Some("secret")),
                None,
                "{spoof} does not target loopback — forwarding the key here sends \
                 it in cleartext to a host the operator did not intend"
            );
        }
    }

    #[cfg(feature = "remote-embed")]
    #[test]
    fn guarded_api_key_is_none_when_no_key_is_configured() {
        assert_eq!(
            RetrievalClient::guarded_api_key("http://embed.example.com", None),
            None
        );
    }

    #[cfg(feature = "remote-embed")]
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

    /// Binds `guard_local_model_with_url` to its call site inside `build_embedder`.
    ///
    /// This is the regression test for the defect itself, so it has to discriminate
    /// against the exact behaviour that shipped: deleting the
    /// `Self::guard_local_model_with_url(config)?;` line does NOT make this an
    /// `is_err()` failure — execution falls through to the url branch, which builds
    /// an `EmbedderHttp` successfully and returns `Ok`. So the mutation kills this
    /// test at the `Ok(_) => panic!` arm, which is the whole point: asserting
    /// `is_err()` on a config that "looks wrong" would have passed against the buggy
    /// code too, because nothing errored — it silently embedded over the network.
    #[tokio::test]
    async fn build_embedder_rejects_a_url_combined_with_a_local_dir_model() {
        let c = cfg_with(Some("http://127.0.0.1:8081/v1"), "local-dir:/weights");
        let err = match RetrievalClient::build_embedder(&c, /* lite */ true).await {
            Ok(_) => panic!(
                "a url alongside a local-dir: model must be rejected, not silently \
                 resolved in the url's favour — that is the offline guarantee failing open"
            ),
            Err(e) => e,
        };
        // Class, not just message: a config conflict must stay retryable
        // (isError: false) so sibling parallel tool calls are not aborted.
        assert!(
            err.downcast_ref::<crate::tools::RecoverableError>()
                .is_some(),
            "must be RecoverableError (isError: false), not anyhow::bail!; got: {err}"
        );
        let msg = err.to_string();
        // Both operands must appear, or the operator cannot tell WHICH url is
        // fighting WHICH model — the url in particular may have come from the
        // startup dotenv rather than anything they typed.
        assert!(
            msg.contains("local-dir:/weights"),
            "must name the offending model, got: {msg}"
        );
        assert!(
            msg.contains("http://127.0.0.1:8081/v1"),
            "must name the url it conflicts with, got: {msg}"
        );
    }

    /// The complement of the guard, and a regression test for a real break this
    /// fix caused before it was narrowed: `default_embed_model()` is
    /// `"local:AllMiniLML6V2Q"`, so "url configured, model left unset" — an
    /// ordinary remote deployment — arrives here as a `local:` model the operator
    /// never chose. An earlier draft of `guard_local_model_with_url` covered
    /// `local:` too and took down
    /// `agent::tests::memory_embedder_is_built_from_the_shared_code_embedder`,
    /// which builds from a root-less config and so gets exactly that default.
    ///
    /// Widening the guard to `local:` must therefore fail this test until the
    /// config can distinguish a defaulted model from a chosen one.
    #[tokio::test]
    async fn build_embedder_accepts_a_url_with_the_defaulted_local_model() {
        assert_eq!(
            crate::config::project::default_embed_model(),
            "local:AllMiniLML6V2Q",
            "this test's premise is that the default model carries a local: prefix; \
             if the default changes, re-derive whether the guard can widen"
        );
        let c = cfg_with(
            Some("http://127.0.0.1:8081/v1"),
            &crate::config::project::default_embed_model(),
        );
        let got = RetrievalClient::build_embedder(&c, /* lite */ true).await;

        #[cfg(feature = "remote-embed")]
        assert!(
            got.is_ok(),
            "a url with the DEFAULT local: model is the ordinary remote deployment — \
             rejecting it would break every setup that configures a url and no model"
        );

        // A lean build has no HTTP transport, so this configuration cannot be BUILT.
        // The claim under test survives anyway, and it is asserted rather than
        // skipped: the guard must not be WHAT refuses it. Gating this test out (the
        // cheaper fix) would delete the guard's non-over-firing proof from exactly
        // the configuration where a mis-widened guard could hide behind a
        // legitimate refusal and look identical to today.
        #[cfg(not(feature = "remote-embed"))]
        {
            // `match`, not `expect_err`: the Ok variant is `Arc<dyn CodeEmbedder>`,
            // which is not `Debug`, so `expect_err`'s bound does not hold.
            let msg = match got {
                Ok(_) => panic!("a lean build cannot construct an HTTP embedder"),
                Err(e) => e.to_string(),
            };
            assert!(
                msg.contains("no HTTP embed transport"),
                "the refusal must be the transport bail. got: {msg}"
            );
            assert!(
                !msg.contains("local-dir"),
                "the guard must NOT reject a url + DEFAULTED local: model, in any \
                 build configuration. got: {msg}"
            );
        }
    }

    /// The guard must not over-fire: a url with an ordinary remote model is the
    /// normal production configuration and has to keep working. Without this,
    /// widening the predicate to "any url + any model" would pass the two tests
    /// above while breaking every remote deployment.
    #[tokio::test]
    async fn build_embedder_still_accepts_a_url_with_an_ordinary_model() {
        let c = cfg_with(Some("http://127.0.0.1:8081/v1"), "CodeRankEmbed");
        let got = RetrievalClient::build_embedder(&c, /* lite */ true).await;

        #[cfg(feature = "remote-embed")]
        assert!(
            got.is_ok(),
            "a url with a non-local model is the ordinary remote setup and must build"
        );

        // See the sibling above for why this asserts rather than gates.
        #[cfg(not(feature = "remote-embed"))]
        {
            let msg = match got {
                Ok(_) => panic!("a lean build cannot construct an HTTP embedder"),
                Err(e) => e.to_string(),
            };
            assert!(
                msg.contains("no HTTP embed transport"),
                "the refusal must be the transport bail. got: {msg}"
            );
            assert!(
                !msg.contains("local-dir"),
                "the guard must NOT reject a url + ordinary remote model, in any \
                 build configuration. got: {msg}"
            );
        }
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
