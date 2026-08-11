use anyhow::Result;

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
    /// Precedence, highest first: `CODESCOUT_EMBEDDER_MODEL` (read directly
    /// below) > `[embeddings].model` in project.toml > the built-in default.
    /// **Surprise**: a *different* env var, `CODESCOUT_EMBED_MODEL`, is
    /// applied even earlier, inside `ProjectConfig::load_or_default` itself —
    /// so a project's `[embeddings].model` can already have been silently
    /// overwritten before it ever reaches the project-config side of this
    /// merge. Two independently-named env vars reach the same effective
    /// setting at two different layers; `CODESCOUT_EMBEDDER_MODEL` (this
    /// field) is the one Task 6 introduced, `CODESCOUT_EMBED_MODEL` predates
    /// it and lives in `config/project.rs`.
    pub model: String,
    /// Embedding API key, used only when `embedder_url` is set.
    pub api_key: Option<String>,
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
}

impl RetrievalConfig {
    /// Compose a per-instance collection name. With empty prefix this returns
    /// the canonical names (`code_chunks` etc.) preserving backwards compatibility.
    pub fn collection(&self, kind: &str) -> String {
        format!("{}{}", self.collection_prefix, kind)
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
            resolve_embed_fields_with(EmbedEnv::from_real_env(), root);
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url,
            model,
            api_key,
            model_dim,
            sparse_embedder_url: std::env::var("CODESCOUT_SPARSE_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8084".into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8083".into()),
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
        })
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
}

impl EmbedEnv {
    fn from_real_env() -> Self {
        Self {
            url: std::env::var("CODESCOUT_EMBEDDER_URL").ok(),
            model: std::env::var("CODESCOUT_EMBEDDER_MODEL").ok(),
            api_key: std::env::var("EMBED_API_KEY").ok(),
            dim: parse_model_dim(std::env::var("CODESCOUT_MODEL_DIM").ok()),
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
fn normalize_embedder_url(url: &str) -> String {
    let trimmed = url.trim_end_matches('/');
    if let Some(base) = trimmed.strip_suffix("/v1/embeddings") {
        base.to_string()
    } else if let Some(base) = trimmed.strip_suffix("/v1") {
        base.to_string()
    } else {
        trimmed.to_string()
    }
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
            non_empty(Some(e.model)),
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

/// Resolve the four embed-related `RetrievalConfig` fields for a project
/// root, given an already-resolved `EmbedEnv`. Thin edge wrapper around
/// `resolve_embed_fields_from` — the ONLY thing it adds is the real
/// `ProjectConfig::load_or_default(root)` call.
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
) -> (Option<String>, String, Option<String>, Option<usize>) {
    let project_config =
        root.and_then(|r| crate::config::project::ProjectConfig::load_or_default(r).ok());
    resolve_embed_fields_from(env, project_config)
}

/// Same composition as `resolve_embed_fields_with`, but takes an
/// already-loaded `ProjectConfig` instead of a root — the seam tests use
/// (via `ProjectConfig::load_with_global_base` with an empty global
/// layer, exactly like the sibling `load_or_default_*` tests in
/// `config/project.rs`) so real project.toml file I/O is exercised
/// end-to-end without inheriting `load_or_default`'s own env overlay.
fn resolve_embed_fields_from(
    env: EmbedEnv,
    project_config: Option<crate::config::project::ProjectConfig>,
) -> (Option<String>, String, Option<String>, Option<usize>) {
    merge_embed_config(env, project_config.map(|c| c.embeddings))
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
        };
        let project = EmbeddingsSection {
            model: "local-dir:/weights".to_string(),
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
        };
        let project = EmbeddingsSection {
            model: "local-dir:/weights".to_string(),
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
            model: "local-dir:/weights".to_string(),
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
            model: default_embed_model(),
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
    fn from_env_and_project_none_root_has_no_embeddings_section() {
        // No root -> no project.toml to load -> model falls back to the
        // built-in default, url/api_key/dim stay unset (absent a matching env
        // var). This is the real end-to-end path, not just the pure helpers.
        let cfg = RetrievalConfig::from_env_and_project(None).unwrap();
        assert_eq!(cfg.model, crate::config::project::default_embed_model());
    }
}
