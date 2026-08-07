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
    pub embedder_url: String,
    pub sparse_embedder_url: String,
    pub reranker_url: String,
    pub model_dim: usize,
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

    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant_url: std::env::var("CODESCOUT_QDRANT_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:6334".into()),
            embedder_url: std::env::var("CODESCOUT_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            sparse_embedder_url: std::env::var("CODESCOUT_SPARSE_EMBEDDER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8084".into()),
            reranker_url: std::env::var("CODESCOUT_RERANKER_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8083".into()),
            model_dim: std::env::var("CODESCOUT_MODEL_DIM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(768),
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
