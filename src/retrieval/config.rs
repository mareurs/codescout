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

pub struct RetrievalConfig {
    pub qdrant_url: String,
    /// `None` means "no url configured" — resolve the backend from `model`.
    /// Previously defaulted to `http://127.0.0.1:8081`, which fabricated a
    /// server that may never have existed. An explicit env value is untouched.
    pub embedder_url: Option<String>,
    pub sparse_embedder_url: String,
    pub reranker_url: String,
    /// `None` means "the model is the authority". `Some(n)` is an operator pin.
    pub model_dim: Option<usize>,
    /// Model identifier in codescout-embed's grammar (`local:`, `local-dir:`,
    /// `ollama:`, `openai:`, or a bare name sent to `embedder_url`).
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
    pub fn from_env_and_project(root: Option<&std::path::Path>) -> Result<Self> {
        let embeddings = root
            .and_then(|r| crate::config::project::ProjectConfig::load_or_default(r).ok())
            .map(|c| c.embeddings);
        let (cfg_model, cfg_url, cfg_key) = match embeddings {
            Some(e) => (
                Some(e.model),
                e.url,
                e.api_key.map(|k| k.as_str().to_string()),
            ),
            None => (None, None, None),
        };
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url: merge_env_over_project(
                std::env::var("CODESCOUT_EMBEDDER_URL").ok(),
                cfg_url,
            ),
            model: merge_env_over_project(
                std::env::var("CODESCOUT_EMBEDDER_MODEL").ok(),
                cfg_model,
            )
            .unwrap_or_else(crate::config::project::default_embed_model),
            api_key: merge_env_over_project(std::env::var("EMBED_API_KEY").ok(), cfg_key),
            model_dim: parse_model_dim(std::env::var("CODESCOUT_MODEL_DIM").ok()),
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

/// Env value wins over project config; both absent is a genuine "unset", not a
/// fabricated default (see `RetrievalConfig::embedder_url`'s field doc). Pure
/// fn over already-resolved values, not an inline env read, so the precedence
/// is testable without `std::env::set_var` — same shape as `parse_rerank_opt_in`
/// above, for the same reason (see its doc comment).
fn merge_env_over_project(env_val: Option<String>, project_val: Option<String>) -> Option<String> {
    env_val.or(project_val)
}

/// Parse `CODESCOUT_MODEL_DIM`. Absent or unparsable is `None` — "the model is
/// the authority", never a fabricated 768. Pure fn for the same testability
/// reason as `merge_env_over_project`.
fn parse_model_dim(env_val: Option<String>) -> Option<usize> {
    env_val.and_then(|s| s.parse().ok())
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

    // `RetrievalConfig::from_env_and_project` reads real process env
    // (CODESCOUT_EMBEDDER_URL/_MODEL/_DIM, EMBED_API_KEY) at its edge. Per
    // docs/conventions/test-env-isolation.md, EnvGuard + #[serial] is NOT
    // VIABLE for new tests: it does not coordinate with non-serial tests
    // elsewhere in the suite that read the same vars, and `a656f8cec220d347`
    // removed the pattern crate-wide for exactly that reason (measured
    // 119 -> 0 `set_var`/`remove_var` occurrences in the default `cargo test`
    // build). So these tests exercise the merge precedence as pure functions
    // (`merge_env_over_project`, `parse_model_dim`) instead of mutating real
    // env -- the same shape `parse_rerank_opt_in` above already uses, and for
    // the same documented reason (see its doc comment).

    #[test]
    fn unset_url_no_longer_fabricates_8081() {
        assert_eq!(
            merge_env_over_project(None, None),
            None,
            "an unset url must mean 'resolve from the model', not 'assume 8081'"
        );
    }

    #[test]
    fn env_url_overrides_project_config() {
        assert_eq!(
            merge_env_over_project(
                Some("http://from-env:8/v1".to_string()),
                Some("http://from-toml:9/v1".to_string()),
            ),
            Some("http://from-env:8/v1".to_string())
        );
    }

    #[test]
    fn project_model_reaches_retrieval_when_env_is_silent() {
        // Exercise the real project.toml -> EmbeddingsSection load path (file
        // I/O only, no env mutation). Uses `load_with_global_base` with an empty
        // global layer rather than `load_or_default` -- the latter also applies
        // `CODESCOUT_EMBED_MODEL`/`CODESCOUT_EMBED_URL` env overrides (a
        // DIFFERENT var-name family than the CODESCOUT_EMBEDDER_* ones this
        // module reads), which would make this test's outcome depend on
        // whatever happens to be set in the ambient dev/CI environment -- same
        // no-guard convention as the sibling `load_or_default_*` tests in
        // config/project.rs, which use the same helper for the same reason.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".codescout")).unwrap();
        std::fs::write(
            dir.path().join(".codescout/project.toml"),
            "[project]\nname = \"proj\"\n\n[embeddings]\nmodel = \"local-dir:/weights\"\n",
        )
        .unwrap();
        let empty_global = toml::Value::Table(toml::map::Map::new());
        let cfg =
            crate::config::project::ProjectConfig::load_with_global_base(dir.path(), empty_global)
                .unwrap();
        assert_eq!(cfg.embeddings.model, "local-dir:/weights");
        assert_eq!(cfg.embeddings.url, None);

        // With env silent, the merge must carry the project value straight through.
        assert_eq!(
            merge_env_over_project(None, Some(cfg.embeddings.model.clone())),
            Some("local-dir:/weights".to_string())
        );
        assert_eq!(merge_env_over_project(None, cfg.embeddings.url), None);
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
    fn from_env_and_project_none_root_has_no_embeddings_section() {
        // No root -> no project.toml to load -> model falls back to the
        // built-in default, url/api_key/dim stay unset (absent a matching env
        // var). This is the real end-to-end path, not just the pure helpers.
        let cfg = RetrievalConfig::from_env_and_project(None).unwrap();
        assert_eq!(cfg.model, crate::config::project::default_embed_model());
    }
}
