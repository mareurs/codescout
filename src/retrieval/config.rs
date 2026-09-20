use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Parse `CODESCOUT_RERANK` into the opt-in flag. **Absent, blank, or unrecognised is
/// `false`** — the reranker stays off unless someone asks for it explicitly.
///
/// A pure fn over `Option<&str>` rather than an inline env read, so it is testable
/// without `std::env::set_var` — which is UB against the suite's concurrent `getenv`
/// readers. Same shape as `server::parse_idle_shutdown` for the same reason.
///
/// Unrecognised values resolve to `false` rather than erroring: this gates an
/// optimisation, so a typo costing you a disabled reranker is strictly better than a
/// typo costing you a failed search.
pub(crate) fn parse_rerank_opt_in(raw: Option<&str>) -> bool {
    matches!(
        raw.map(str::trim)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

/// Resolve the sqlite-vec store directory, from an **already-read** env value
/// and the project root.
///
/// Precedence:
///
/// 1. `CODESCOUT_SQLITE_DIR`, when set and non-empty — an operator override, and
///    the only way to put the stores somewhere else entirely.
/// 2. `<root>/.codescout/embeddings` — the default. Per-project, beside the
///    `.codescout/` directory that already holds the project's memories and
///    workspace config.
/// 3. `$HOME/.codescout/embeddings` — only when there is no project root at all
///    (`RetrievalConfig::from_env()`). Rootless callers have no project-local
///    place to put anything.
///
/// Pure over its inputs for the same reason as [`parse_rerank_opt_in`]: the
/// precedence must be testable without `std::env::set_var`, which is UB against
/// the suite's concurrent `getenv` readers and is banned crate-wide by
/// `docs/conventions/test-env-isolation.md`.
///
/// **Why per-project rather than per-user.** Case 2 replaced a `$HOME` default
/// that had three problems, only the first of which was visible:
///
/// - Tests build projects in tempdirs, so every run minted a fresh
///   `<random>.db` in the developer's home that nothing ever removed — ~148 per
///   full `cargo test`, 8,000+ files and 2.7 GB accumulated on one machine.
///   Under a project root the store is inside the tempdir and dies with it.
/// - Nothing bounded growth in production either: deleting a project left its
///   store behind forever, at 3.2 MB preallocated per `vec0` table.
/// - `project_id` is the root's **directory basename** when a project has no
///   config of its own (`src/config/project.rs`), so `$HOME` made a single
///   global namespace keyed by basename — two projects named `api` shared one
///   database file, and the `project_id` column inside it could not tell their
///   rows apart. Per-root paths cannot collide.
///
/// The cost is that stores written under the old default are orphaned: an
/// existing project re-indexes once. That was the accepted trade —
/// `docs/issues/archive/2026-08-13-tests-leak-sqlite-vec-dbs-into-real-home.md`.
pub(crate) fn resolve_sqlite_dir(raw: Option<String>, root: Option<&Path>) -> Result<PathBuf> {
    if let Some(d) = raw.filter(|s| !s.is_empty()) {
        return Ok(PathBuf::from(d));
    }
    let base = match root {
        Some(r) => r.to_path_buf(),
        None => crate::platform::home_dir().context(
            "cannot resolve home dir for the sqlite-vec store, and no project root was \
             supplied; set CODESCOUT_SQLITE_DIR",
        )?,
    };
    Ok(base.join(".codescout").join("embeddings"))
}

/// Sparse-embedder fallback: the **host** port `docker-compose.yml` publishes for
/// the sparse service (`127.0.0.1:48084:80`).
///
/// The container-internal port (`80`) is not reachable from the host, and neither
/// is `8084` — which is what this fallback used to be. A wrong fallback here does
/// not fail loudly at startup; it degrades `semantic_search`, semantic-memory
/// cross-embedding, and anchor creation with a connect error while the surrounding
/// operation still reports success, which is why the drift survived unnoticed.
/// `retrieval_default_ports_match_published_compose_ports` keeps this honest.
pub(crate) const DEFAULT_SPARSE_EMBEDDER_URL: &str = "http://127.0.0.1:48084";

/// Reranker fallback: the **host** port `docker-compose.yml` publishes for the
/// reranker service (`127.0.0.1:48083:8080`). See
/// [`DEFAULT_SPARSE_EMBEDDER_URL`] for why the container-internal port is wrong
/// and how the mismatch stays caught.
pub(crate) const DEFAULT_RERANKER_URL: &str = "http://127.0.0.1:48083";

// no Debug derive: `api_key` holds a plaintext key, and Debug is the only
// thing that would let a stray `tracing::debug!(?config)` leak it. If a
// future change adds a derive here, redact `api_key` explicitly first.
pub struct RetrievalConfig {
    pub qdrant_url: String,
    /// `None` means "no url configured" — resolve the backend from `model`.
    /// Previously defaulted to `http://127.0.0.1:8081`, which fabricated a
    /// server that may never have existed. An explicit env value is untouched.
    /// Normalized (a trailing `/v1` or `/v1/embeddings` stripped) so
    /// `EmbedderHttp`'s unconditional `/v1/embeddings` suffix never doubles up
    /// — see `normalize_embedder_url`.
    pub embedder_url: Option<String>,
    pub sparse_embedder_url: String,
    pub reranker_url: String,
    /// `None` means "the model is the authority". `Some(n)` is an operator pin.
    pub model_dim: Option<usize>,
    /// Model identifier in codescout-embed's grammar (`local:`, `local-dir:`,
    /// `ollama:`, `openai:`, or a bare name sent to `embedder_url`).
    ///
    /// Precedence, highest first: `CODESCOUT_EMBEDDING_MODEL` (or its deprecated
    /// aliases `CODESCOUT_EMBEDDER_MODEL`/`CODESCOUT_EMBED_MODEL`, in that
    /// order — see [`crate::config::embedding_env::MODEL`]) > `[embeddings].model`
    /// in project.toml > the built-in default (`local:AllMiniLML6V2Q`).
    ///
    /// Resolved in ONE place, [`resolve_embed_fields_with`], through
    /// [`crate::config::embedding_env::read`] — not at two independent layers.
    /// It used to be two: `CODESCOUT_EMBED_MODEL` applied inside
    /// `ProjectConfig::load_or_default` while `CODESCOUT_EMBEDDER_MODEL` applied
    /// here, so which one won depended on where you looked. Fixed 2026-09-17,
    /// `docs/plans/2026-09-17-embedding-config-consolidation.md` Task 5b
    /// (`70ec79d5`) — this comment used to describe the old two-layer bug as
    /// current behaviour, which was itself doc-vs-code drift nobody was
    /// reading closely enough to catch.
    pub model: String,
    /// Embedding API key, used only when `embedder_url` is set.
    pub api_key: Option<String>,
    /// Operator override for the model name sent in the `/v1/embeddings`
    /// request body, from `CODESCOUT_EMBEDDER_MODEL_NAME`. `None` means "no
    /// override" — the name is then derived from `model`.
    ///
    /// Read here, at the config edge, rather than inside `EmbedderHttp::new`
    /// where it used to live. Two reasons, and the second is the one that
    /// matters: every other `CODESCOUT_*` read already happens in this file, and
    /// an env read buried in a constructor is untestable without `set_var`,
    /// which is UB against the suite's concurrent `getenv` readers and banned
    /// crate-wide by `docs/conventions/test-env-isolation.md`. With the value on
    /// the struct, a test sets the field and asserts the resolution both ways —
    /// the same shape as `EmbedEnv::from_real_env` feeding the pure
    /// `merge_embed_config`.
    ///
    /// That untestability was not incidental: it is why `[embeddings].model`
    /// could be discarded on the url path with a green suite. See
    /// `docs/issues/archive/2026-09-17-the-configured-embedding-model-is-discarded-whenever-a-url-is-set.md`.
    pub dense_model_name_override: Option<String>,
    pub profile: String,
    /// Multiplier for the sparse (BM25) prefetch candidate pool relative to dense.
    /// 1.0 = equal weight (default), 2.0 = BM25 gets 2× more candidates in RRF.
    pub bm25_boost: f32,
    /// Skip the sparse leg entirely. Search becomes pure dense ANN.
    /// Set via CODESCOUT_DISABLE_SPARSE=1 — used in matrix control cells.
    pub disable_sparse: bool,
    /// Apply the cross-encoder reranker. **Opt-in, default OFF** — set
    /// `CODESCOUT_RERANK=1`.
    ///
    /// Note the polarity: this is a positive flag, unlike its `disable_sparse`
    /// neighbour. Measured 2026-08-07 on the rebuilt index, both arms differing only
    /// in this one dimension: reranking scored **23/75 at a 1559 ms warm median**
    /// against **26/75 at 990 ms** without it — about **569 ms per query** for a
    /// result that got no better (it helped 4 of 25 test cases and hurt 5). A
    /// component that costs half a second and does not measurably improve retrieval
    /// has no business being on by default, and memory `conventions`
    /// § Environment-Agnostic Tuning says the honest shape for it is inert with the
    /// active value opt-in.
    ///
    /// Kept configurable rather than deleted because the cost is entirely
    /// model-and-hardware dependent — the same weights served over TEI rather than
    /// llama-server measured ~80 ms, and a different cross-encoder may well earn its
    /// keep. What is not defensible is choosing for the user silently. Full data:
    /// `docs/issues/archive/2026-07-28-reranker-costs-42x-latency-and-lowers-score.md`.
    pub rerank: bool,
    /// Prefix prepended to qdrant collection names. Default empty (live collections
    /// `code_chunks`, `memories`, etc.). Set via
    /// CODESCOUT_QDRANT_COLLECTION_PREFIX to isolate benchmark runs (e.g.
    /// `bench_jinav2_` → `bench_jinav2_code_chunks`).
    pub collection_prefix: String,
    /// Directory holding the sqlite-vec stores, one `<project_id>.db` per project.
    /// Resolved once at the edge by [`resolve_sqlite_dir`] rather than read from
    /// the environment inside the store constructor.
    ///
    /// Inert on the Qdrant backend — `VectorBackend::resolve` picks the store, and
    /// only `SqliteVec` reads this.
    pub sqlite_dir: PathBuf,
}

impl RetrievalConfig {
    /// Compose a per-instance collection name. With empty prefix this returns
    /// the canonical names (`code_chunks` etc.) preserving backwards compatibility.
    pub fn collection(&self, kind: &str) -> String {
        format!("{}{}", self.collection_prefix, kind)
    }

    /// The model name to put in the `/v1/embeddings` request body.
    ///
    /// Pure over the struct — no env access — so both directions are testable
    /// without `set_var`. Precedence matches the project-wide ladder: the
    /// operator's `CODESCOUT_EMBEDDER_MODEL_NAME` override wins, otherwise the
    /// configured `model` with its routing prefix stripped.
    ///
    /// The strip uses [`codescout_embed::bare_model_name`], shared with
    /// `create_embedder_with_config`'s url arm rather than re-derived, so a
    /// `local:`/`ollama:`/`openai:` model resolves to the same wire name on both
    /// paths. Re-deriving it here is precisely how the two drifted before.
    ///
    /// Note the override is kept on top by the 2026-09-17 ruling: every stack
    /// deployment in existence sets it while leaving `[embeddings].model` at the
    /// built-in default, so letting `model` win would silently repoint them at
    /// the wrong model. The shadow WARNING that makes this visible needs
    /// provenance `RetrievalConfig` does not yet carry — it cannot distinguish a
    /// chosen `model` from a defaulted one — and lands with Task 5 of
    /// `docs/plans/2026-09-17-embedding-config-consolidation.md`.
    // Its only caller, `build_http_embedder`, is `remote-embed`-gated; the lean
    // lane compiles this and reaches it from nowhere.
    #[cfg_attr(not(feature = "remote-embed"), allow(dead_code))]
    pub(crate) fn dense_model_name(&self) -> String {
        self.dense_model_name_override
            .clone()
            .unwrap_or_else(|| codescout_embed::bare_model_name(&self.model).to_string())
    }

    /// Env-only construction. Equivalent to `from_env_and_project(None)`.
    pub fn from_env() -> Result<Self> {
        Self::from_env_and_project(None)
    }

    /// `[embeddings]` in the project's config is the base; `CODESCOUT_*` env
    /// vars override it. Benchmark matrix cells set env, so they are unaffected.
    ///
    /// The four embed-related fields (`embedder_url`, `model`, `api_key`,
    /// `model_dim`) are resolved by `resolve_embed_fields_with`, split out
    /// specifically so the composition (root -> project.toml load -> merge)
    /// is testable end-to-end without mutating real process env — see that
    /// function's doc comment and `merge_tests` below.
    pub fn from_env_and_project(root: Option<&std::path::Path>) -> Result<Self> {
        let (embedder_url, model, api_key, model_dim) =
            resolve_embed_fields_with(EmbedEnv::from_real_env(), root)?;
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url,
            model,
            api_key,
            model_dim,
            // Blank-is-absent, matching `non_empty`'s policy for the sibling
            // embed fields: an exported-but-empty `FOO=` must not win over a
            // configured model, which is the same silent-loss class this
            // override was implicated in.
            dense_model_name_override: non_empty(
                std::env::var("CODESCOUT_EMBEDDER_MODEL_NAME").ok(),
            ),
            sparse_embedder_url: std::env::var("CODESCOUT_SPARSE_EMBEDDER_URL")
                .unwrap_or_else(|_| DEFAULT_SPARSE_EMBEDDER_URL.into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| DEFAULT_RERANKER_URL.into()),
            profile: std::env::var("CODESCOUT_RETRIEVAL_PROFILE").unwrap_or_else(|_| "cpu".into()),
            // Dense-vs-sparse fusion weight — corpus- and model-dependent by
            // construction, so 3.0 is a value that worked on OUR corpus and dense
            // model, not a calibration anyone else inherits (memory `conventions`
            // § Environment-Agnostic Tuning). Our own sweep peaked at 5.0 (35/75)
            // while 3.0 stayed the default; both are observations, and users
            // re-derive theirs with scripts/sweep-bm25-boost.sh. Inert while
            // CODESCOUT_DISABLE_SPARSE is set.
            bm25_boost: std::env::var("CODESCOUT_BM25_BOOST")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3.0),
            disable_sparse: std::env::var("CODESCOUT_DISABLE_SPARSE")
                .ok()
                .map(|v| matches!(v.as_str(), "1" | "true" | "yes"))
                .unwrap_or(false),
            rerank: parse_rerank_opt_in(std::env::var("CODESCOUT_RERANK").ok().as_deref()),
            collection_prefix: std::env::var("CODESCOUT_QDRANT_COLLECTION_PREFIX")
                .unwrap_or_default(),
            sqlite_dir: resolve_sqlite_dir(std::env::var("CODESCOUT_SQLITE_DIR").ok(), root)?,
        })
    }
}

/// Where an effective embedding setting's value actually came from —
/// [`effective_embedding_settings`]'s per-field answer to Task 8's "which one
/// won?" question (`docs/plans/2026-09-17-embedding-config-consolidation.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    /// A `CODESCOUT_EMBEDDING_*` env var (canonical or deprecated alias) set it.
    Env,
    /// `[embeddings]` in project.toml or the global `config.toml` set it. The
    /// two are deep-merged into one `EmbeddingsSection` before this struct
    /// exists (`merge_toml`, before deserialisation), so they are not
    /// distinguished further here — telling them apart needs loading each
    /// layer alone, which this reporting pass does not do. Noted in the plan
    /// as a scoping choice, not an oversight.
    Config,
    /// Neither env nor config set it; this is the built-in literal.
    Default,
    /// `dim` only: no `CODESCOUT_EMBEDDING_DIM` override, so the model's own
    /// dimension applies. There is no project.toml `dim` field to check —
    /// `EmbeddingsSection` has no such field, by design (see its own doc).
    Model,
}

/// One resolved setting, paired with where it came from.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResolvedSetting<T> {
    pub value: T,
    pub source: SettingSource,
}

/// Every effective `[embeddings]` setting, each paired with its source —
/// Task 8's "show the resolved value and its source", consumed by
/// `workspace(action="status")` and `librarian(action="doctor")` so the two
/// can never disagree with each other, or with what the embedder was
/// actually built from.
///
/// Reads exactly the same layers `RetrievalConfig::from_env_and_project`
/// reads (env, then project.toml/global config.toml, then the built-in
/// default) — this is a REPORTING PASS over that same resolution, not a
/// second one, which is what keeps it from drifting: the exact defect named
/// in `src/main.rs`'s comment about `p.config.embeddings.model` diverging
/// from `client.config.model`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EffectiveEmbeddingSettings {
    pub model: ResolvedSetting<String>,
    pub url: ResolvedSetting<Option<String>>,
    /// Never the key itself — only whether one is set, and where from.
    pub api_key_set: ResolvedSetting<bool>,
    pub dim: ResolvedSetting<Option<usize>>,
}

/// Build [`EffectiveEmbeddingSettings`] for `root` (or the process env alone
/// when `None`, matching [`RetrievalConfig::from_env`]'s own convention).
pub fn effective_embedding_settings(
    root: Option<&std::path::Path>,
) -> anyhow::Result<EffectiveEmbeddingSettings> {
    let resolved = RetrievalConfig::from_env_and_project(root)?;
    Ok(effective_embedding_settings_for(&resolved, root))
}

/// Same as [`effective_embedding_settings`], but takes an ALREADY-RESOLVED
/// config instead of resolving one. For a caller (`ProjectStatus`, `doctor`)
/// that constructed a `RetrievalConfig` moments earlier for its own purposes
/// (backend classification, an index client, ...) and must not build a
/// *second*, separately-resolved one just to report its provenance — two
/// resolutions in one response is exactly the "second copy that can diverge"
/// shape this whole task exists to end, even if both happen to agree today.
/// Only re-reads `root`'s project.toml, for provenance classification; the
/// reported VALUES are always `resolved`'s own, never re-derived here.
pub(crate) fn effective_embedding_settings_for(
    resolved: &RetrievalConfig,
    root: Option<&std::path::Path>,
) -> EffectiveEmbeddingSettings {
    let env = EmbedEnv::from_real_env();
    let project_embeddings = root
        .and_then(|r| crate::config::project::ProjectConfig::load_or_default(r).ok())
        .map(|c| c.embeddings);
    effective_embedding_settings_from(resolved, &env, project_embeddings.as_ref())
}

/// The pure core of [`effective_embedding_settings`] — genuinely no env or file
/// I/O: `env` is [`EmbedEnv`], already resolved by the caller, not re-read here.
/// An earlier version called [`crate::config::embedding_env::read`] directly
/// inside this function — which LOOKED pure (no explicit env param) but silently
/// read real process env on every call, exactly the untestable-without-`set_var`
/// shape `docs/conventions/test-env-isolation.md` exists to prevent. Caught before
/// commit by trying to write a test for it. Takes the ALREADY-RESOLVED config, the
/// ALREADY-RESOLVED env layer, and the raw (pre-default) project `[embeddings]`
/// section — all three of which the real entry point below has to load anyway.
fn effective_embedding_settings_from(
    resolved: &RetrievalConfig,
    env: &EmbedEnv,
    project: Option<&crate::config::project::EmbeddingsSection>,
) -> EffectiveEmbeddingSettings {
    let source_for = |env_val: Option<&String>, cfg_val: Option<&String>| {
        if non_empty(env_val.cloned()).is_some() {
            SettingSource::Env
        } else if non_empty(cfg_val.cloned()).is_some() {
            SettingSource::Config
        } else {
            SettingSource::Default
        }
    };

    let model_source = source_for(env.model.as_ref(), project.and_then(|p| p.model.as_ref()));
    let url_source = source_for(env.url.as_ref(), project.and_then(|p| p.url.as_ref()));
    let api_key_cfg = project
        .and_then(|p| p.api_key.as_ref())
        .map(|k| k.as_str().to_string());
    let api_key_source = source_for(env.api_key.as_ref(), api_key_cfg.as_ref());
    let dim_source = if env.dim.is_some() {
        SettingSource::Env
    } else {
        SettingSource::Model
    };

    EffectiveEmbeddingSettings {
        model: ResolvedSetting {
            value: resolved.model.clone(),
            source: model_source,
        },
        url: ResolvedSetting {
            value: resolved.embedder_url.clone(),
            source: url_source,
        },
        api_key_set: ResolvedSetting {
            value: resolved.api_key.is_some(),
            source: api_key_source,
        },
        dim: ResolvedSetting {
            value: resolved.model_dim,
            source: dim_source,
        },
    }
}

/// Compatibility default for an unpinned `model_dim` at the few call sites that
/// still need a concrete `usize` today (constructing `EmbedderHttp`, sizing a
/// Qdrant collection) — Task 6 threads the `Option` through without yet
/// selecting a backend from it, so those sites fall back to the same 768 that
/// used to live inside `from_env` itself. A wrong value here means broken, not
/// degraded (memory `conventions` § Environment-Agnostic Tuning classifies this
/// as a compatibility constant, out of scope for that rule).
pub(crate) const DEFAULT_MODEL_DIM: usize = 768;

/// Parse `CODESCOUT_MODEL_DIM`. Absent or unparsable is `None` — "the model is
/// the authority", never a fabricated 768. Pure fn for the same testability
/// reason as `merge_env_over_project`.
fn parse_model_dim(env_val: Option<String>) -> Option<usize> {
    env_val.and_then(|s| s.parse().ok())
}

/// Env-side embedding config, resolved once at the edge from real process
/// env. `EmbedEnv::from_real_env` is the ONLY thing in this module that reads
/// `CODESCOUT_EMBEDDER_URL`/`CODESCOUT_EMBEDDER_MODEL`/`EMBED_API_KEY`/
/// `CODESCOUT_MODEL_DIM` — everything downstream (`merge_embed_config`,
/// `resolve_embed_fields_with`) takes this struct as a plain value, so the
/// FULL composition (not just an isolated one-line precedence rule) is
/// testable without ever mutating process env. This is
/// `docs/conventions/test-env-isolation.md` option A, applied to the
/// composition rather than to the leaf decisions only — mirrors
/// `LibrarianEnv::from_env`/`ServerEnv::from_env`'s shape.
// no Debug derive: `api_key` holds a plaintext key (see RetrievalConfig's
// same note).
#[derive(Clone, Default)]
struct EmbedEnv {
    url: Option<String>,
    model: Option<String>,
    api_key: Option<String>,
    dim: Option<usize>,
    /// Which of the fields above arrived via the **startup dotenv** rather than an
    /// operator export.
    ///
    /// Env wins over both config layers, and that is deliberate — it is the
    /// documented escape hatch for benchmark cells and CI. But
    /// `~/.config/codescout/.env` is read into the environment on every start, so a
    /// machine *default* written there becomes the highest-precedence *override* and
    /// silently outranks every project's own `[embeddings]`. By the time this struct
    /// is built the two are indistinguishable: both are just process env.
    ///
    /// Carrying the provenance is what lets the resolver tell them apart — warning on
    /// the second case while staying silent on the first. Held as plain data on the
    /// struct rather than read from a global inside the merge, so
    /// `dotenv_shadowed_fields` stays pure and both directions are ordinary unit
    /// tests — the same discipline that keeps `merge_embed_config` testable without
    /// `set_var`.
    from_dotenv: DotenvProvenance,
}

/// Per-field provenance for [`EmbedEnv`]. `true` means "this value came from the
/// startup dotenv", which is the only case that warrants a shadowing warning — an
/// operator export winning is the escape hatch working as designed.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct DotenvProvenance {
    url: bool,
    model: bool,
    api_key: bool,
}

/// Pure: which `[embeddings]` fields a **dotenv-injected** env value silently
/// overrode.
///
/// Returns field names, not a message, so the caller owns the wording and this stays
/// a value function with an ordinary equality assertion.
///
/// Three conditions must hold for a field to be named, and dropping any one of them
/// turns this into noise:
///
/// 1. the env value came from the dotenv (an export is the sanctioned override);
/// 2. the env value is actually present and non-blank (a blank never wins anyway,
///    per `non_empty`);
/// 3. the config layers actually set that field — if the project and global are both
///    silent there is nothing being shadowed, which is the ordinary case on a machine
///    configured entirely through `.env`.
///
/// `project` is the already-**merged** section (global beneath project), so a global
/// `config.toml` value being shadowed is reported exactly like a project one. That is
/// correct: both are files the user edited and expected to take effect.
fn dotenv_shadowed_fields(
    env: &EmbedEnv,
    project: Option<&crate::config::project::EmbeddingsSection>,
) -> Vec<&'static str> {
    let Some(p) = project else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let shadows = |from_dotenv: bool, env_val: Option<&String>, cfg_val: Option<&String>| {
        from_dotenv
            && env_val.is_some_and(|v| !v.trim().is_empty())
            && cfg_val.is_some_and(|v| !v.trim().is_empty())
    };
    if shadows(env.from_dotenv.url, env.url.as_ref(), p.url.as_ref()) {
        out.push("url");
    }
    if shadows(env.from_dotenv.model, env.model.as_ref(), p.model.as_ref()) {
        out.push("model");
    }
    let cfg_key = p.api_key.as_ref().map(|k| k.as_str().to_string());
    if shadows(
        env.from_dotenv.api_key,
        env.api_key.as_ref(),
        cfg_key.as_ref(),
    ) {
        out.push("api_key");
    }
    out
}

impl EmbedEnv {
    fn from_real_env() -> Self {
        // Read the dotenv-injected key set ONCE here at the edge, then carry the
        // answer as data. Consulting the global inside the merge would make the
        // shadowing rule untestable without `set_var` — the same trap that hid the
        // model-discard defect for a release.
        let injected = crate::config::global::dotenv_injected_keys();
        use crate::config::embedding_env as envs;
        // Provenance is per CANONICAL setting, so it must be true when ANY of that
        // setting's names was dotenv-injected — a value that arrived under a
        // deprecated alias is no less a dotenv default than one under the new name.
        let any_injected = |n: &envs::EnvName| {
            injected.contains(n.canonical) || n.deprecated.iter().any(|d| injected.contains(*d))
        };
        Self {
            url: envs::read(&envs::URL),
            model: envs::read(&envs::MODEL),
            api_key: envs::read(&envs::API_KEY),
            dim: parse_model_dim(envs::read(&envs::DIM)),
            from_dotenv: DotenvProvenance {
                url: any_injected(&envs::URL),
                model: any_injected(&envs::MODEL),
                api_key: any_injected(&envs::API_KEY),
            },
        }
    }
}

/// Treat an exported-but-blank value as absent, on either side of the merge —
/// an empty `CODESCOUT_EMBEDDER_URL=` must not "win" over a real project.toml
/// value, and an explicit `url = ""` in project.toml must not read as "a url
/// is configured". `EmbedderHttp::new` already guards its own `EMBED_API_KEY`
/// read the same way; this mirrors it here so the merge can't reintroduce the
/// gap for the other three fields.
fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty())
}

/// Normalize an embedder base url so `EmbedderHttp`'s unconditional
/// `format!("{base}/v1/embeddings")` produces the right endpoint regardless of
/// which convention the value came from: a bare host (the
/// `CODESCOUT_EMBEDDER_URL` convention, e.g. `.env.example`'s
/// `http://127.0.0.1:48081`) or an already-`/v1`-suffixed API base (the
/// `[embeddings].url` convention documented on `EmbeddingsSection::url`, e.g.
/// `http://127.0.0.1:43300/v1`). Without this, a project.toml `url` ending in
/// `/v1` reached `EmbedderHttp` unnormalized and produced
/// `.../v1/v1/embeddings` -> 404 instead of the intended endpoint.
///
/// The shape recognition itself lives in `codescout_embed::normalize_embeddings_base`
/// — shared with `RemoteEmbedder::from_url`'s identical three-branch logic
/// rather than duplicated here, so the two conventions cannot drift apart.
fn normalize_embedder_url(url: &str) -> String {
    codescout_embed::normalize_embeddings_base(url).to_string()
}

/// `[embeddings]` in the project's config is the base; the resolved
/// `EmbedEnv` overrides it, field by field. Pure — no env access, no file
/// I/O — which is what makes the PRECEDENCE (not just each field's shape)
/// directly testable: a test constructs both arguments and asserts which one
/// won.
///
/// `model_dim` has no project.toml counterpart (see `RetrievalConfig::model_dim`'s
/// doc) — it passes through from `env` unconditionally.
fn merge_embed_config(
    env: EmbedEnv,
    project: Option<crate::config::project::EmbeddingsSection>,
) -> (Option<String>, String, Option<String>, Option<usize>) {
    let (proj_model, proj_url, proj_key) = match project {
        Some(e) => (
            non_empty(e.model),
            non_empty(e.url),
            non_empty(e.api_key.map(|k| k.as_str().to_string())),
        ),
        None => (None, None, None),
    };

    let url = non_empty(env.url)
        .or(proj_url)
        .map(|u| normalize_embedder_url(&u));
    let model = non_empty(env.model)
        .or(proj_model)
        .unwrap_or_else(crate::config::project::default_embed_model);
    let api_key = non_empty(env.api_key).or(proj_key);
    let dim = env.dim;

    (url, model, api_key, dim)
}

/// The four embed-related `RetrievalConfig` fields, in the order
/// `(embedder_url, model, api_key, model_dim)` — the shape `merge_embed_config` returns
/// and both wrappers below forward.
///
/// Named rather than written out because wrapping the tuple in `Result` (so a config
/// load failure can refuse instead of defaulting) puts it over clippy's
/// `type_complexity` threshold. The tuple itself did not grow.
type ResolvedEmbedFields = (Option<String>, String, Option<String>, Option<usize>);

/// Resolve the four embed-related `RetrievalConfig` fields for a project
/// root, given an already-resolved `EmbedEnv`. Thin edge wrapper around
/// `resolve_embed_fields_from` — the ONLY thing it adds is the real
/// `ProjectConfig::load_or_default(root)` call.
///
/// **A load failure REFUSES; it does not fall back.** This used to be
/// `root.and_then(|r| ProjectConfig::load_or_default(r).ok())`, and that `.ok()`
/// turned a parse error into `None`, which fell through
/// `merge_embed_config`'s `unwrap_or_else(default_embed_model)` to the built-in
/// model — so a project whose config cannot be parsed was indistinguishable from
/// one that never configured a model, and nothing named the substitution
/// (`docs/issues/archive/2026-09-18-a-malformed-project-toml-silently-resolves-the-default-embedding-model.md`,
/// the same shape as the archived `f73130523241a666` one subsystem over).
/// `Agent::with_project_at` already failed closed on the identical config; this is
/// the other half catching up.
///
/// `root == None` (no project at all) and an **absent** project.toml both still
/// resolve silently: `load_or_default` synthesises a default table for a missing
/// file rather than erroring, so only a genuine load failure reaches the `?`.
///
/// Deliberately NOT the seam most tests exercise: `load_or_default`
/// itself applies its own `CODESCOUT_EMBED_MODEL`/`CODESCOUT_EMBED_URL`
/// overlay (a DIFFERENT, pre-existing env-var family from the
/// `CODESCOUT_EMBEDDER_*` ones `EmbedEnv` reads — see
/// `RetrievalConfig::model`'s doc) — a layer this module does not own and
/// cannot edge-resolve away. `merge_tests` below tests
/// `resolve_embed_fields_from` instead (same composition, an
/// already-loaded `ProjectConfig` in place of a root), which is fully
/// ambient-env-immune. `tests/retrieval_unit.rs` covers this exact
/// function end-to-end, using `temp_env` (this repo's established pattern
/// for `RetrievalConfig::from_env` integration tests) to neutralize both
/// env families for the duration.
fn resolve_embed_fields_with(
    env: EmbedEnv,
    root: Option<&std::path::Path>,
) -> Result<ResolvedEmbedFields> {
    let project_config = match root {
        Some(r) => Some(
            crate::config::project::ProjectConfig::load_or_default(r).map_err(|e| {
                // Input-driven: the user's own config file is malformed, and the
                // caller can fix it or drop it. `RecoverableError` keeps this
                // `isError: false`, so a sibling parallel tool call in the same turn
                // is not aborted by someone else's typo (`get_guide("error-handling")`).
                // The hint names two actions the reader can actually perform — a
                // guard that says only what is wrong sends nobody anywhere.
                crate::tools::RecoverableError::with_hint(
                    format!(
                        "the project config at {} could not be loaded: {e}",
                        r.join(".codescout").join("project.toml").display()
                    ),
                    "Fix the file named in the error — a [project] table with a `name` \
                         is required — or delete it to fall back to the built-in embedding \
                         defaults. It is no longer defaulted silently: a config that cannot \
                         be parsed would otherwise resolve to a model the project never \
                         asked for.",
                )
            })?,
        ),
        None => None,
    };
    Ok(resolve_embed_fields_from(env, project_config))
}

/// Same composition as `resolve_embed_fields_with`, but takes an
/// already-loaded `ProjectConfig` instead of a root — the seam tests use
/// (via `ProjectConfig::load_with_global_base` with an empty global
/// layer, exactly like the sibling `load_or_default_*` tests in
/// `src/config/project.rs`) so real project.toml file I/O is exercised
/// end-to-end without inheriting `load_or_default`'s own env overlay.
///
/// This is also where the dotenv-shadowing warning is emitted, because it is the
/// one place holding both sides of the comparison. The decision itself lives in
/// the pure [`dotenv_shadowed_fields`]; only the wording is here.
fn resolve_embed_fields_from(
    env: EmbedEnv,
    project_config: Option<crate::config::project::ProjectConfig>,
) -> ResolvedEmbedFields {
    let section = project_config.map(|c| c.embeddings);
    for field in dotenv_shadowed_fields(&env, section.as_ref()) {
        tracing::warn!(
            "[embeddings].{field} is set in your config but was overridden by the \
                 startup dotenv (CODESCOUT_ENV_FILE, else ~/.config/codescout/.env). \
                 Environment wins over both config layers by design — but a dotenv is \
                 read on every start, so a machine-wide DEFAULT written there silently \
                 outranks every project. Move it to ~/.config/codescout/config.toml to \
                 make it a default the project can override, or unset it there if the \
                 override was intended for one run."
        );
    }
    merge_embed_config(env, section)
}

#[cfg(test)]
mod rerank_opt_in_tests {
    use super::parse_rerank_opt_in;

    /// The default is the load-bearing case: absent means OFF. Every input here is
    /// something a real `.env` produces — commented out, set empty, set to a word.
    #[test]
    fn rerank_is_off_unless_explicitly_requested() {
        for raw in [
            None,
            Some(""),
            Some("  "),
            Some("0"),
            Some("false"),
            Some("no"),
            Some("off"),
            Some("maybe"),
            Some("2"),
        ] {
            assert!(
                !parse_rerank_opt_in(raw),
                "{raw:?} must NOT enable the reranker — off is the default, and an \
                 unrecognised value must not silently cost ~569 ms/query"
            );
        }
    }

    #[test]
    fn rerank_accepts_the_documented_truthy_forms_case_and_space_insensitively() {
        for raw in [
            "1", "true", "TRUE", "True", "yes", "YES", "on", "ON", " 1 ", "\ttrue\n",
        ] {
            assert!(
                parse_rerank_opt_in(Some(raw)),
                "{raw:?} should enable the reranker"
            );
        }
    }
}

#[cfg(test)]
mod sqlite_dir_tests {
    use super::resolve_sqlite_dir;
    use std::path::PathBuf;

    // Every case below drives the resolver by ARGUMENT. That is the whole point
    // of the split: before it, the same precedence lived inside
    // `SqliteVecCodeStore::from_env` and could only be exercised by mutating
    // process env, which `docs/conventions/test-env-isolation.md` bans and which
    // is UB against the suite's concurrent readers.

    #[test]
    fn an_explicit_value_wins() {
        // Over the project root, too — an operator override is the only way to
        // put the stores somewhere else entirely.
        let got = resolve_sqlite_dir(
            Some("/tmp/somewhere/else".to_string()),
            Some(std::path::Path::new("/w/proj")),
        )
        .unwrap();
        assert_eq!(got, PathBuf::from("/tmp/somewhere/else"));
    }

    #[test]
    fn an_empty_value_is_treated_as_unset() {
        // `CODESCOUT_SQLITE_DIR=` is a shell idiom for clearing a variable, and
        // taking it literally would resolve the store to the process's current
        // directory — silently, and differently per invocation. Pinned because
        // the `.filter(|s| !s.is_empty())` that prevents it is one call long and
        // reads like a redundant guard.
        let got =
            resolve_sqlite_dir(Some(String::new()), Some(std::path::Path::new("/w/proj"))).unwrap();
        assert_eq!(got, PathBuf::from("/w/proj/.codescout/embeddings"));
    }

    #[test]
    fn the_fallback_is_the_users_home_directory() {
        // Only when there is no project root at all — `RetrievalConfig::from_env()`.
        // A rootless caller has no project-local place to put anything. This is
        // now the ONLY surviving path to `$HOME`; it used to be the default, and
        // that default is what produced the leak, the unbounded production
        // growth, and the basename collision.
        // docs/issues/archive/2026-08-13-tests-leak-sqlite-vec-dbs-into-real-home.md
        let got = resolve_sqlite_dir(None, None).unwrap();
        let home = crate::platform::home_dir().unwrap();
        assert_eq!(got, home.join(".codescout").join("embeddings"));
    }

    #[test]
    fn the_default_is_under_the_project_root() {
        let got = resolve_sqlite_dir(None, Some(std::path::Path::new("/w/proj"))).unwrap();
        assert_eq!(got, PathBuf::from("/w/proj/.codescout/embeddings"));
    }

    #[test]
    fn two_projects_with_the_same_basename_get_different_stores() {
        // The regression guard for the collision this change exists to close.
        // `project_id` is the root's directory basename when a project has no
        // config of its own (`src/config/project.rs`), so under the old `$HOME`
        // default these two resolved to ONE database file — and the `project_id`
        // column inside it, being that same basename, could not tell their rows
        // apart either.
        let one = std::path::Path::new("/w/one/api");
        let two = std::path::Path::new("/w/two/api");
        assert_eq!(
            one.file_name(),
            two.file_name(),
            "precondition: the roots must share a basename, or this proves nothing"
        );
        let a = resolve_sqlite_dir(None, Some(one)).unwrap();
        let b = resolve_sqlite_dir(None, Some(two)).unwrap();
        assert_ne!(a, b, "same-basename projects must not share a store dir");
    }

    /// DRY gate: `CODESCOUT_SQLITE_DIR` must be READ in exactly one place.
    ///
    /// It was read in two: `SqliteVecCodeStore::from_env` and
    /// `SqliteVecSemanticMemoryStore::from_env`, verbatim twins differing only in
    /// the db filename suffix. Routing the code store through `RetrievalConfig`
    /// left the memory store still writing `<id>.memories.db` into `$HOME`, and
    /// nothing failed — the suite was green and the leak was 60% smaller, which
    /// is exactly the shape of a fix that looks done. It was found by insisting
    /// the measured per-run delta reach zero rather than "much better".
    ///
    /// The needle is assembled character-wise so this test's own source, and the
    /// prose above it, do not match.
    #[test]
    fn the_sqlite_dir_env_var_is_read_in_exactly_one_place() {
        let needle: String = ["env::var(\"", "CODESCOUT", "_SQLITE_DIR"].concat();
        let root = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
        let mut hits: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("rs") {
                continue;
            }
            let Ok(content) = std::fs::read_to_string(path) else {
                continue;
            };
            let count = content.matches(needle.as_str()).count();
            if count > 0 {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                hits.push(format!(
                    "{} ({count})",
                    rel.display().to_string().replace('\\', "/")
                ));
            }
        }
        assert_eq!(
            hits,
            vec!["retrieval/config.rs (1)".to_string()],
            "the sqlite-vec dir must be resolved once, at the config edge; a new \
             store should take a directory from RetrievalConfig::sqlite_dir rather \
             than reading the environment itself — found: {hits:?}"
        );
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use crate::config::project::{default_embed_model, EmbeddingsSection};
    use crate::config::sensitive::SensitiveString;

    // `RetrievalConfig::from_env_and_project` reads real process env
    // (CODESCOUT_EMBEDDER_URL/_MODEL/_DIM, EMBED_API_KEY) at its edge, via
    // `EmbedEnv::from_real_env`. Per docs/conventions/test-env-isolation.md,
    // EnvGuard + #[serial] is NOT VIABLE for new tests: it does not
    // coordinate with non-serial tests elsewhere in the suite that read the
    // same vars, and `a656f8cec220d347` removed the pattern crate-wide for
    // exactly that reason. So every test below constructs an `EmbedEnv`
    // directly instead of calling `from_real_env` -- the same shape
    // `parse_rerank_opt_in` above already uses, applied to the composition
    // (`resolve_embed_fields_with`/`merge_embed_config`), not just to each
    // field's precedence in isolation. Verified real: this dev machine
    // genuinely exports `CODESCOUT_EMBEDDER_URL`/`CODESCOUT_EMBED_MODEL`/
    // `CODESCOUT_EMBED_URL`, so a naive `Some(tempdir)` test that relied on
    // real env being unset would be silently machine-dependent.

    fn write_project_toml(dir: &std::path::Path, embeddings_toml: &str) {
        std::fs::create_dir_all(dir.join(".codescout")).unwrap();
        std::fs::write(
            dir.join(".codescout/project.toml"),
            format!("[project]\nname = \"proj\"\n\n{embeddings_toml}"),
        )
        .unwrap();
    }

    /// A dotenv-injected value that overrides a configured one is NAMED.
    ///
    /// This is the defect the provenance channel exists for: the operator's
    /// `~/.config/codescout/.env` is a machine-wide default, but it reaches the
    /// resolver as process env — the highest-precedence layer — so it silently beats
    /// every project's own `[embeddings]`. `codescout index` then reports success,
    /// exit 0, against an endpoint the project did not configure.
    /// `docs/issues/archive/2026-09-17-the-machine-default-dotenv-outranks-every-per-project-override.md`
    #[test]
    fn a_dotenv_value_that_overrides_a_configured_one_is_named() {
        let env = EmbedEnv {
            url: Some("http://from-dotenv:1".into()),
            from_dotenv: DotenvProvenance {
                url: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let project = EmbeddingsSection {
            url: Some("http://from-project:2".into()),
            ..Default::default()
        };
        assert_eq!(dotenv_shadowed_fields(&env, Some(&project)), vec!["url"]);
    }

    /// An EXPORTED value that overrides a configured one is silent.
    ///
    /// The half that makes the warning worth having. Env-beats-config is the
    /// documented escape hatch — benchmark cells and CI depend on it — so a warning
    /// keyed on "env won" rather than on provenance would fire on every legitimate
    /// override and be tuned out. The inputs here are byte-identical to the test
    /// above except `from_dotenv`, which is the entire claim.
    #[test]
    fn an_exported_value_that_overrides_a_configured_one_is_silent() {
        let env = EmbedEnv {
            url: Some("http://from-export:1".into()),
            from_dotenv: DotenvProvenance::default(),
            ..Default::default()
        };
        let project = EmbeddingsSection {
            url: Some("http://from-project:2".into()),
            ..Default::default()
        };
        assert!(dotenv_shadowed_fields(&env, Some(&project)).is_empty());
    }

    /// A dotenv value with nothing to shadow is silent.
    ///
    /// The ordinary case on a machine configured entirely through `.env` — which is
    /// this repo's own setup. Without this condition the warning would fire on every
    /// resolution for every such user, which is the shape that gets a warning
    /// deleted rather than heeded.
    #[test]
    fn a_dotenv_value_with_no_configured_counterpart_is_silent() {
        let env = EmbedEnv {
            url: Some("http://from-dotenv:1".into()),
            from_dotenv: DotenvProvenance {
                url: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let project = EmbeddingsSection::default();
        assert!(dotenv_shadowed_fields(&env, Some(&project)).is_empty());
        // and with no project section at all
        assert!(dotenv_shadowed_fields(&env, None).is_empty());
    }

    /// Each field is judged on its OWN provenance, not the struct's.
    ///
    /// `EmbedEnv` carries three independently-sourced values: a user can export one
    /// for a single run while the dotenv supplies the others. A per-struct flag would
    /// report all three or none, and be wrong in both directions on the same call.
    #[test]
    fn provenance_is_per_field_not_per_struct() {
        let env = EmbedEnv {
            url: Some("http://from-dotenv:1".into()),
            model: Some("model-from-export".into()),
            api_key: Some("key-from-dotenv".into()),
            from_dotenv: DotenvProvenance {
                url: true,
                model: false,
                api_key: true,
            },
            ..Default::default()
        };
        let project = EmbeddingsSection {
            url: Some("http://from-project:2".into()),
            model: Some("model-from-project".into()),
            api_key: Some(SensitiveString::new("key-from-project")),
            ..Default::default()
        };
        assert_eq!(
            dotenv_shadowed_fields(&env, Some(&project)),
            vec!["url", "api_key"],
            "the exported `model` must not be named even though it also won"
        );
    }

    /// A blank dotenv value is not a shadow.
    ///
    /// `merge_embed_config` treats an exported-but-empty value as absent via
    /// `non_empty`, so the configured value wins and nothing was overridden. Warning
    /// here would describe an override that did not happen.
    #[test]
    fn a_blank_dotenv_value_shadows_nothing() {
        let env = EmbedEnv {
            url: Some("   ".into()),
            from_dotenv: DotenvProvenance {
                url: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let project = EmbeddingsSection {
            url: Some("http://from-project:2".into()),
            ..Default::default()
        };
        assert!(dotenv_shadowed_fields(&env, Some(&project)).is_empty());
    }

    /// The global layer supplies `url` and `api_key` when the project sets
    /// neither, and the project's own `model` still wins.
    ///
    /// **Routed through `GlobalConfig::load_from_dir` + `to_toml_value`
    /// deliberately, not through a hand-built `toml::Value`.** The defect this
    /// pins lived in the *type*: the global `[embeddings]` was a separate struct
    /// carrying only `model`, so serde discarded `url` and `api_key` at parse
    /// time and `to_toml_value` re-serialised the struct — meaning the dropped
    /// keys could not reach the merge even in principle. A test that constructs
    /// the merge base directly never touches that struct and therefore passes
    /// both before and after the fix: it would assert about `merge_toml`, which
    /// was never broken. The parse-and-re-serialise round trip is the whole
    /// subject.
    ///
    /// `docs/issues/archive/2026-09-17-the-global-embeddings-section-holds-one-field-and-drops-the-rest.md`
    #[test]
    fn a_global_url_and_key_survive_the_round_trip_into_the_resolved_config() {
        let global_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            global_dir.path().join("config.toml"),
            "[embeddings]\nurl = \"https://global.example.com/v1\"\napi_key = \"sk-global\"\n",
        )
        .unwrap();
        let global = crate::config::global::GlobalConfig::load_from_dir(global_dir.path())
            .unwrap()
            .expect("the global config file exists, so this must parse to Some");

        let proj_dir = tempfile::tempdir().unwrap();
        write_project_toml(proj_dir.path(), "[embeddings]\nmodel = \"project-model\"\n");
        let cfg = crate::config::project::ProjectConfig::load_with_global_base(
            proj_dir.path(),
            global.to_toml_value(),
        )
        .unwrap();

        // `EmbedEnv::default()`, never `from_real_env()` — this machine exports
        // the CODESCOUT_EMBEDDER_* family, so a real-env read would decide the
        // verdict instead of the merge under test.
        let (url, model, api_key, _dim) = resolve_embed_fields_from(EmbedEnv::default(), Some(cfg));

        assert_eq!(
            url.as_deref(),
            Some("https://global.example.com"),
            "a global url must reach the resolved config; the trailing /v1 is \
             stripped by normalize_embedder_url, which is why this is not the \
             literal configured string"
        );
        assert_eq!(
            api_key.as_deref(),
            Some("sk-global"),
            "a global api_key must survive too — it rides the same round trip, \
             and SensitiveString is serde(transparent) so the value passes \
             through rather than being redacted into the merge base"
        );
        assert_eq!(
            model, "project-model",
            "the project layer must still win the field it DOES set — the point \
             is global-default-then-project-override, not global replacing the \
             project"
        );
    }

    /// The other half of the same merge, and the one a per-field fix would miss:
    /// the global layer fills a gap *within* `[embeddings]` while the project
    /// sets a sibling field in the same table.
    ///
    /// This is what `merge_toml`'s recursion buys, and it is the behaviour
    /// `docs/manual/src/configuration/global-config.md` § Merge semantics
    /// currently denies in writing — it claims a project `[embeddings]` replaces
    /// the global table wholesale. `merge_toml_base_fills_missing_key` already
    /// pins the recursion at the `toml::Value` level; this pins that the shared
    /// struct does not undo it one layer up, which is the layer that was broken.
    #[test]
    fn the_global_layer_fills_a_gap_inside_the_project_embeddings_table() {
        let global_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            global_dir.path().join("config.toml"),
            "[embeddings]\nmodel = \"global-model\"\nurl = \"https://global.example.com/v1\"\n",
        )
        .unwrap();
        let global = crate::config::global::GlobalConfig::load_from_dir(global_dir.path())
            .unwrap()
            .expect("the global config file exists, so this must parse to Some");

        let proj_dir = tempfile::tempdir().unwrap();
        write_project_toml(proj_dir.path(), "[embeddings]\nmodel = \"project-model\"\n");
        let cfg = crate::config::project::ProjectConfig::load_with_global_base(
            proj_dir.path(),
            global.to_toml_value(),
        )
        .unwrap();

        let (url, model, _key, _dim) = resolve_embed_fields_from(EmbedEnv::default(), Some(cfg));

        assert_eq!(model, "project-model", "the project overrides what it sets");
        assert_eq!(
            url.as_deref(),
            Some("https://global.example.com"),
            "and inherits what it does not — a project that names only `model` \
             must not lose the global `url` sitting beside it"
        );
    }

    #[test]
    fn unset_everything_no_longer_fabricates_anything() {
        let (url, model, api_key, dim) = merge_embed_config(EmbedEnv::default(), None);
        assert_eq!(
            url, None,
            "an unset url must mean 'resolve from the model', not 'assume 8081'"
        );
        assert_eq!(model, default_embed_model());
        assert_eq!(api_key, None);
        assert_eq!(dim, None, "an unpinned dim must let the model decide");
    }

    #[test]
    fn env_wins_over_project_on_all_three_string_fields() {
        // Reproduces review round-1 mutation β (invert precedence at all
        // three wiring sites at once): each assertion below fails on its own
        // if only that ONE field's `.or()` gets inverted, and all three fail
        // if the review's exact all-at-once mutation is reapplied.
        let env = EmbedEnv {
            url: Some("http://from-env:8".to_string()),
            model: Some("local:BGESmallENV15".to_string()),
            api_key: Some("sk-env".to_string()),
            dim: None,
            ..Default::default()
        };
        let project = EmbeddingsSection {
            model: Some("local-dir:/weights".to_string()),
            url: Some("http://from-toml:9".to_string()),
            api_key: Some(SensitiveString::new("sk-toml")),
            ..Default::default()
        };
        let (url, model, api_key, _) = merge_embed_config(env, Some(project));
        assert_eq!(
            url.as_deref(),
            Some("http://from-env:8"),
            "env url must win"
        );
        assert_eq!(model, "local:BGESmallENV15", "env model must win");
        assert_eq!(api_key.as_deref(), Some("sk-env"), "env api_key must win");
    }

    #[test]
    fn project_config_reaches_through_the_full_composition_when_env_is_silent() {
        // Reproduces review round-1 mutation α (replace the
        // `ProjectConfig::load_or_default` chain with `let embeddings =
        // None;`). Drives the REAL file-loading path via
        // `ProjectConfig::load_with_global_base` + `resolve_embed_fields_from`
        // -- not a hand-built `EmbeddingsSection` fed straight to
        // `merge_embed_config` -- so it is the one that would catch that
        // specific deletion (applied to `resolve_embed_fields_from`'s
        // `project_config.map(|c| c.embeddings)` line; the thin
        // `resolve_embed_fields_with` wrapper's OWN `load_or_default` call is
        // covered separately in `tests/retrieval_unit.rs`, which can
        // neutralize `load_or_default`'s own env overlay via `temp_env` —
        // not available to a `#[cfg(test)]` module in the same binary as
        // 3000+ other parallel unit tests).
        let dir = tempfile::tempdir().unwrap();
        write_project_toml(
            dir.path(),
            "[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
        );
        let empty_global = toml::Value::Table(toml::map::Map::new());
        let project_config =
            crate::config::project::ProjectConfig::load_with_global_base(dir.path(), empty_global)
                .unwrap();
        let (url, model, _, _) =
            resolve_embed_fields_from(EmbedEnv::default(), Some(project_config));
        assert_eq!(model, "local-dir:/weights");
        assert_eq!(url.as_deref(), Some("http://from-toml:9"));
    }

    #[test]
    fn env_still_wins_through_the_full_composition() {
        // Same composition seam as above, but with env populated too — a
        // precedence inversion that only manifests once real project.toml
        // loading is wired in (rather than in the pure `merge_embed_config`
        // call alone) would slip past `env_wins_over_project_on_all_three_...`
        // but not this one.
        let dir = tempfile::tempdir().unwrap();
        write_project_toml(
            dir.path(),
            "[embeddings]\nmodel = \"local-dir:/weights\"\nurl = \"http://from-toml:9\"\n",
        );
        let empty_global = toml::Value::Table(toml::map::Map::new());
        let project_config =
            crate::config::project::ProjectConfig::load_with_global_base(dir.path(), empty_global)
                .unwrap();
        let env = EmbedEnv {
            url: Some("http://from-env:8".to_string()),
            model: Some("local:BGESmallENV15".to_string()),
            ..Default::default()
        };
        let (url, model, _, _) = resolve_embed_fields_from(env, Some(project_config));
        assert_eq!(url.as_deref(), Some("http://from-env:8"));
        assert_eq!(model, "local:BGESmallENV15");
    }

    #[test]
    fn empty_string_env_is_treated_as_absent() {
        // M-2: an exported-but-empty/whitespace-only env var must not "win"
        // over a real project.toml value. `std::env::var(X).ok()` yields
        // `Some("")` for `X=`, which is exactly the shape `EmbedEnv` carries
        // here (constructed directly rather than via `from_real_env`, so this
        // exercises the merge's own filtering, not env's).
        let env = EmbedEnv {
            url: Some(String::new()),
            model: Some("   ".to_string()),
            api_key: None,
            dim: None,
            ..Default::default()
        };
        let project = EmbeddingsSection {
            model: Some("local-dir:/weights".to_string()),
            url: Some("http://from-toml:9".to_string()),
            ..Default::default()
        };
        let (url, model, _, _) = merge_embed_config(env, Some(project));
        assert_eq!(url.as_deref(), Some("http://from-toml:9"));
        assert_eq!(model, "local-dir:/weights");
    }

    #[test]
    fn empty_string_project_url_is_treated_as_absent() {
        let project = EmbeddingsSection {
            model: Some("local-dir:/weights".to_string()),
            url: Some(String::new()),
            ..Default::default()
        };
        let (url, _, _, _) = merge_embed_config(EmbedEnv::default(), Some(project));
        assert_eq!(url, None);
    }

    #[test]
    fn url_normalization_strips_v1_suffix_variants() {
        for (input, expected) in [
            ("http://host:9", "http://host:9"),
            ("http://host:9/v1", "http://host:9"),
            ("http://host:9/v1/embeddings", "http://host:9"),
            ("http://host:9/v1/", "http://host:9"),
            ("http://host:9/", "http://host:9"),
        ] {
            assert_eq!(
                normalize_embedder_url(input),
                expected,
                "input {input:?} must normalize to {expected:?}"
            );
        }
    }

    #[test]
    fn project_url_with_v1_suffix_is_normalized_through_the_merge() {
        // I-1: `[embeddings].url` is documented (EmbeddingsSection::url) as a
        // `/v1`-suffixed API base, e.g. "http://127.0.0.1:43300/v1". Feeding
        // that straight into `EmbedderHttp` (which appends `/v1/embeddings`
        // unconditionally) produced `.../v1/v1/embeddings` -> 404.
        let project = EmbeddingsSection {
            model: Some(default_embed_model()),
            url: Some("http://127.0.0.1:43300/v1".to_string()),
            ..Default::default()
        };
        let (url, _, _, _) = merge_embed_config(EmbedEnv::default(), Some(project));
        assert_eq!(url.as_deref(), Some("http://127.0.0.1:43300"));
    }

    #[test]
    fn unset_model_dim_is_none_not_768() {
        assert_eq!(
            parse_model_dim(None),
            None,
            "an unpinned dim must let the model decide"
        );
    }

    #[test]
    fn model_dim_parses_a_set_value() {
        assert_eq!(parse_model_dim(Some("4096".to_string())), Some(4096));
    }

    #[test]
    fn model_dim_has_no_project_toml_counterpart() {
        // model_dim passes straight through from env regardless of project
        // config -- there's no `[embeddings].dim` to merge against.
        let env = EmbedEnv {
            dim: Some(4096),
            ..Default::default()
        };
        let (_, _, _, dim) = merge_embed_config(env, Some(EmbeddingsSection::default()));
        assert_eq!(dim, Some(4096));
    }

    #[test]
    fn resolve_embed_fields_with_none_root_has_no_embeddings_section() {
        // Round-2 review R2: the previous version of this test called the REAL
        // `RetrievalConfig::from_env_and_project(None)`, which reads real
        // process env via `EmbedEnv::from_real_env()` -- so it only passed on
        // machines that happen not to export `CODESCOUT_EMBEDDER_MODEL`
        // (true here, not true everywhere). Chose to express it against the
        // pure seam rather than drop it: `merge_tests` already covers the
        // merge-with-nothing-set assertion
        // (`unset_everything_no_longer_fabricates_anything`), and
        // `tests/retrieval_unit.rs::config_from_env_uses_defaults_when_unset`
        // already covers the full `RetrievalConfig::from_env()` public path
        // with real env properly neutralized via `temp_env` -- but neither
        // one drives `resolve_embed_fields_with`'s OWN `root == None`
        // short-circuit specifically. This does, with an explicit
        // `EmbedEnv::default()` (never `from_real_env()`) as input: no
        // ambient variable can decide the verdict, because no real env is
        // read at all.
        let (url, model, api_key, dim) = resolve_embed_fields_with(EmbedEnv::default(), None)
            .expect("root == None loads no config, so it cannot fail");
        assert_eq!(model, default_embed_model());
        assert_eq!(url, None);
        assert_eq!(api_key, None);
        assert_eq!(dim, None);
    }
}

#[cfg(test)]
mod effective_settings_tests {
    use super::{effective_embedding_settings_from, EmbedEnv, RetrievalConfig, SettingSource};
    use crate::config::project::EmbeddingsSection;

    /// Same discipline `client.rs::cfg_with` uses: build off the real
    /// `from_env_and_project(None)` base, then set every field these tests
    /// actually depend on explicitly, so no ambient env can decide a verdict.
    fn resolved(
        model: &str,
        url: Option<&str>,
        api_key: Option<&str>,
        dim: Option<usize>,
    ) -> RetrievalConfig {
        let mut c = RetrievalConfig::from_env_and_project(None).unwrap();
        c.model = model.to_string();
        c.embedder_url = url.map(str::to_string);
        c.api_key = api_key.map(str::to_string);
        c.model_dim = dim;
        c
    }

    fn env(
        model: Option<&str>,
        url: Option<&str>,
        api_key: Option<&str>,
        dim: Option<usize>,
    ) -> EmbedEnv {
        EmbedEnv {
            model: model.map(str::to_string),
            url: url.map(str::to_string),
            api_key: api_key.map(str::to_string),
            dim,
            ..Default::default()
        }
    }

    fn project_with_model(model: &str) -> EmbeddingsSection {
        EmbeddingsSection {
            model: Some(model.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn env_set_reports_env_regardless_of_config() {
        let r = resolved("ollama:nomic-embed-text", None, None, None);
        let e = env(Some("ollama:nomic-embed-text"), None, None, None);
        let p = project_with_model("local:AllMiniLML6V2Q");
        let out = effective_embedding_settings_from(&r, &e, Some(&p));
        assert_eq!(out.model.source, SettingSource::Env);
    }

    #[test]
    fn env_absent_config_set_reports_config() {
        let r = resolved("local:JinaEmbeddingsV2BaseCode", None, None, None);
        let e = env(None, None, None, None);
        let p = project_with_model("local:JinaEmbeddingsV2BaseCode");
        let out = effective_embedding_settings_from(&r, &e, Some(&p));
        assert_eq!(out.model.source, SettingSource::Config);
    }

    #[test]
    fn both_absent_reports_default() {
        let r = resolved("local:AllMiniLML6V2Q", None, None, None);
        let e = env(None, None, None, None);
        let out = effective_embedding_settings_from(&r, &e, None);
        assert_eq!(out.model.source, SettingSource::Default);
    }

    /// The precedence order matters here, not just the two endpoints: with
    /// BOTH env and config set (to different values), the source must still
    /// read "env" — that's the case an either/or pair of tests cannot catch,
    /// since each of the two above only clears one side.
    #[test]
    fn env_wins_the_source_label_even_when_config_also_set_a_different_value() {
        let r = resolved("openai:text-embedding-3-small", None, None, None);
        let e = env(Some("openai:text-embedding-3-small"), None, None, None);
        let p = project_with_model("local:AllMiniLML6V2Q");
        let out = effective_embedding_settings_from(&r, &e, Some(&p));
        assert_eq!(out.model.source, SettingSource::Env);
    }

    #[test]
    fn a_blank_env_value_does_not_report_env() {
        // Blank-is-absent, matching `non_empty`'s policy everywhere else in this
        // consolidation — an exported-but-empty var must not be reported as the
        // winning source.
        let r = resolved("local:AllMiniLML6V2Q", None, None, None);
        let e = env(Some("  "), None, None, None);
        let out = effective_embedding_settings_from(&r, &e, None);
        assert_eq!(out.model.source, SettingSource::Default);
    }

    #[test]
    fn url_and_api_key_resolve_independently_of_model() {
        let r = resolved(
            "CodeRankEmbed",
            Some("http://127.0.0.1:48081"),
            Some("secret"),
            None,
        );
        let e = env(None, Some("http://127.0.0.1:48081"), None, None);
        let p = EmbeddingsSection {
            api_key: Some(crate::config::sensitive::SensitiveString::from("secret")),
            ..Default::default()
        };
        let out = effective_embedding_settings_from(&r, &e, Some(&p));
        assert_eq!(out.url.source, SettingSource::Env);
        assert_eq!(out.api_key_set.source, SettingSource::Config);
        assert!(out.api_key_set.value);
    }

    #[test]
    fn no_api_key_anywhere_reports_default_and_unset() {
        let r = resolved("local:AllMiniLML6V2Q", None, None, None);
        let e = env(None, None, None, None);
        let out = effective_embedding_settings_from(&r, &e, None);
        assert_eq!(out.api_key_set.source, SettingSource::Default);
        assert!(!out.api_key_set.value);
    }

    #[test]
    fn dim_source_is_env_or_model_never_config() {
        // There is no project.toml `dim` field — EmbeddingsSection has none by
        // design — so dim's only two sources are env and "derived from model".
        let with_env = effective_embedding_settings_from(
            &resolved("local:AllMiniLML6V2Q", None, None, Some(768)),
            &env(None, None, None, Some(768)),
            None,
        );
        assert_eq!(with_env.dim.source, SettingSource::Env);
        assert_eq!(with_env.dim.value, Some(768));

        let without_env = effective_embedding_settings_from(
            &resolved("local:AllMiniLML6V2Q", None, None, None),
            &env(None, None, None, None),
            None,
        );
        assert_eq!(without_env.dim.source, SettingSource::Model);
        assert_eq!(without_env.dim.value, None);
    }

    /// The value reported must be `resolved`'s, not re-derived from `env`/
    /// `project` — this function REPORTS the resolution `RetrievalConfig`
    /// already performed, it does not perform a second one. A mutation that
    /// swapped in the config value here would still pass every source-only
    /// assertion above.
    #[test]
    fn the_reported_value_is_the_resolved_configs_own_value() {
        let r = resolved("openai:text-embedding-3-small", None, None, None);
        let e = env(Some("openai:text-embedding-3-small"), None, None, None);
        let p = project_with_model("local:AllMiniLML6V2Q");
        let out = effective_embedding_settings_from(&r, &e, Some(&p));
        assert_eq!(out.model.value, "openai:text-embedding-3-small");
    }
}

#[cfg(test)]
mod default_port_tests {
    use super::{DEFAULT_RERANKER_URL, DEFAULT_SPARSE_EMBEDDER_URL};

    /// Host ports `docker-compose.yml` publishes, read out of its
    /// `- "127.0.0.1:<host>:<container>"` mappings.
    ///
    /// Deliberately parsed from the real compose file rather than restated here: a
    /// constant duplicated into the test is a constant the test can no longer
    /// disagree with.
    fn published_host_ports() -> Vec<u16> {
        let compose = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docker-compose.yml"),
        )
        .expect("docker-compose.yml must exist at the crate root");

        compose
            .lines()
            .filter_map(|line| {
                let mapping = line
                    .trim()
                    .strip_prefix("- ")?
                    .trim()
                    .trim_matches('"')
                    .strip_prefix("127.0.0.1:")?;
                mapping.split(':').next()?.parse::<u16>().ok()
            })
            .collect()
    }

    fn port_of(url: &str) -> u16 {
        url.rsplit(':')
            .next()
            .and_then(|tail| tail.split('/').next())
            .and_then(|p| p.parse().ok())
            .unwrap_or_else(|| panic!("no parseable port in {url}"))
    }

    /// The bug this guards: the fallbacks named `8084`/`8083`, which are
    /// container-internal ports. Nothing listens on them from the host, so
    /// `semantic_search`, semantic-memory cross-embedding, and anchor creation all
    /// failed to connect — each non-fatally, which is why it went unnoticed for
    /// weeks. See `docs/issues/archive/2026-08-06-retrieval-stack-default-endpoints-doc-drift.md`.
    #[test]
    fn retrieval_default_ports_match_published_compose_ports() {
        let published = published_host_ports();

        // Without this the whole test silently degrades to a no-op the moment the
        // compose port syntax changes shape.
        assert!(
            !published.is_empty(),
            "parsed zero published ports from docker-compose.yml — the port-mapping \
             syntax changed and this guard has stopped guarding"
        );

        for (service, url) in [
            ("sparse embedder", DEFAULT_SPARSE_EMBEDDER_URL),
            ("reranker", DEFAULT_RERANKER_URL),
        ] {
            let port = port_of(url);
            assert!(
                published.contains(&port),
                "{service} default {url} uses port {port}, which docker-compose.yml \
                 does not publish to the host (published: {published:?}). A \
                 container-internal port here degrades retrieval silently."
            );
        }
    }

    #[test]
    fn sparse_and_reranker_defaults_are_distinct() {
        assert_ne!(
            port_of(DEFAULT_SPARSE_EMBEDDER_URL),
            port_of(DEFAULT_RERANKER_URL),
            "sparse and reranker share a port — one of the two constants was \
             copy-pasted without changing the port"
        );
    }
}
