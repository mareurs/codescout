use anyhow::Result;
use codescout_embed::Embedder;
use std::sync::Arc;

pub struct EmbeddingService {
    pub embedder: Arc<dyn Embedder>,
    /// Character budget above which an artifact is segmented and mean-pooled rather than
    /// sent whole. Should come from `chunk_size_for_model(&model_spec)` for the configured
    /// model — the same source the migration and tool-layer write paths use, so all three
    /// segment identically instead of each inventing a ceiling.
    ///
    /// `usize::MAX` opts out, matching `HttpMigrationEmbedder::new`'s convention: the
    /// length comparison in `embed_artifact` is then always true, so opting out needs no
    /// special-case branch that could drift from the real one.
    budget_chars: usize,
}

impl EmbeddingService {
    /// Build a service that never segments — `budget_chars` is `usize::MAX`.
    ///
    /// Right for a caller with no model spec to hand, which in practice means tests using
    /// a ceiling-less mock embedder. Production should use [`Self::with_budget`], because
    /// a real backend **rejects** oversized input rather than truncating it, and the
    /// rejection is absorbing: the artifact lands vectorless and is invisible to
    /// `doc(action="find", semantic=…)` with no error anywhere downstream.
    pub fn new(e: Arc<dyn Embedder>) -> Self {
        Self {
            embedder: e,
            budget_chars: usize::MAX,
        }
    }

    /// Build a service that segments above `budget_chars`.
    ///
    /// Pass `chunk_size_for_model(&model_spec)` for the model this embedder is configured
    /// with — never a literal. That function encodes what each backend *actually* accepts
    /// rather than what its model card advertises, including a CodeRankEmbed arm measured
    /// against a live llama-server by binary search on input length.
    pub fn with_budget(e: Arc<dyn Embedder>, budget_chars: usize) -> Self {
        Self {
            embedder: e,
            budget_chars,
        }
    }

    /// Embed an artifact, segmenting and mean-pooling when it exceeds the budget.
    ///
    /// Below the budget this is exactly one call — the common case is untouched. Above it,
    /// the unsegmented code sent the whole thing and llama-server answered HTTP 400
    /// `exceed_context_size_error` (or HTTP 500 `too large to process` above the batch
    /// limit), leaving the artifact with **no vector at all**.
    ///
    /// Measured 2026-09-04: 7 artifacts, every one the `**Members:**` line of a
    /// `docs/trackers/issue-clusters/IC-*.md` file — a single line, so the chunker's
    /// character budget cannot split it and never could. The binding limit is the model's
    /// `n_ctx` (2048 tokens here), not the 4096-token physical batch: the three chunks
    /// above 4096 merely trip the batch check first, which is why raising `--ubatch-size`
    /// would have fixed 3 of 7 and only changed the other 4 from HTTP 500 to HTTP 400.
    ///
    /// **Deliberately still `embed_query`, which is the wrong seam for stored content.**
    /// Correcting it invalidates every vector already in the store — they are all
    /// query-prefixed and new ones would not be — so the seam change and a full
    /// `reindex(reembed=true)` are one operation, and shipping the seam alone would leave
    /// the collection split across two incompatible spaces. Filed separately as
    /// `docs/issues/2026-09-04-librarian-embeds-stored-artifacts-through-the-query-seam.md`.
    /// Segmentation is separable and safe alone, so it ships alone.
    pub async fn embed_artifact(&self, title: Option<&str>, body: &str) -> Result<Vec<f32>> {
        let text = format!("{}\n\n{}", title.unwrap_or(""), body);
        if text.chars().count() <= self.budget_chars {
            return self.embedder.embed_query(&text).await;
        }

        let segments = crate::embed::document::segment_for_budget(&text, self.budget_chars);
        tracing::debug!(
            segments = segments.len(),
            budget_chars = self.budget_chars,
            text_chars = text.chars().count(),
            "artifact exceeds the embedding model's budget — segmenting and mean-pooling"
        );

        let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(segments.len());
        for seg in &segments {
            vectors.push(self.embedder.embed_query(seg).await?);
        }
        crate::embed::document::mean_pool_normalized(&vectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// An embedder that **refuses** any input longer than `max_chars`, standing in for the
    /// backend's per-request ceiling, and counts requests.
    ///
    /// Refusing rather than truncating is load-bearing: llama-server answers HTTP 400 /
    /// 500 and stores nothing, so a double that silently truncated would let every test
    /// below pass against the unsegmented code.
    struct CeilingEmbedder {
        max_chars: usize,
        calls: AtomicUsize,
    }

    impl CeilingEmbedder {
        fn new(max_chars: usize) -> Self {
            Self {
                max_chars,
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait::async_trait]
    impl Embedder for CeilingEmbedder {
        fn dimensions(&self) -> usize {
            3
        }

        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            let mut out = Vec::with_capacity(texts.len());
            for t in texts {
                let n = t.chars().count();
                // Counted BEFORE the ceiling check, so this tallies requests SENT,
                // including refused ones. The opt-out test's whole claim is that the text
                // reached the embedder whole; a counter that only tallied *accepted*
                // requests reads 0 there and proves nothing about what was sent.
                let i = self.calls.fetch_add(1, Ordering::SeqCst);
                anyhow::ensure!(
                    n <= self.max_chars,
                    "input ({n} chars) exceeds the ceiling ({})",
                    self.max_chars
                );
                // Deliberately DIVERGENT per call, and that is what makes the unit-norm
                // assertion able to fail. A mean of near-identical unit vectors is already
                // ~unit-norm, so a fixture returning one constant direction would pass
                // whether or not the pooler renormalises. These three are orthogonal, so
                // their raw mean has norm 0.577.
                out.push(match i % 3 {
                    0 => vec![1.0, 0.0, 0.0],
                    1 => vec![0.0, 1.0, 0.0],
                    _ => vec![0.0, 0.0, 1.0],
                });
            }
            Ok(out)
        }
    }

    const BUDGET: usize = 500;

    #[tokio::test]
    async fn an_artifact_whose_single_line_exceeds_the_budget_is_segmented_under_the_ceiling() {
        // The real fixture shape, and one well-formed markdown cannot express: ONE line,
        // longer than the budget. All 7 artifacts this fixes are the `**Members:**` line
        // of a docs/trackers/issue-clusters/IC-*.md file, and a line has no interior
        // structure to split on — which is why a budget the chunker honours everywhere
        // else does not bound this input. Shortening the line, or letting a newline into
        // it, silently stops this test discriminating.
        let one_long_line = format!("**Members:** {}", "x".repeat(5_000));
        assert!(
            !one_long_line.contains('\n'),
            "fixture must be a single line — a newline gives the splitter a boundary and \
             the test stops covering the case it exists for"
        );

        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::with_budget(emb.clone(), BUDGET);

        let v = svc
            .embed_artifact(Some("IC-18"), &one_long_line)
            .await
            .expect("segmenting must keep every request under the backend ceiling");

        assert_eq!(
            v.len(),
            3,
            "a pooled vector keeps the embedder's dimensions"
        );
        assert!(
            emb.calls.load(Ordering::SeqCst) > 1,
            "must have segmented: one raw call would have exceeded the ceiling"
        );
    }

    #[tokio::test]
    async fn an_artifact_under_the_budget_stays_a_single_call() {
        // The common case must not become N calls — segmentation is a fallback, not a
        // reshaping of every write.
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::with_budget(emb.clone(), BUDGET);

        svc.embed_artifact(Some("t"), "a short body").await.unwrap();

        assert_eq!(
            emb.calls.load(Ordering::SeqCst),
            1,
            "under budget must be one un-pooled call"
        );
    }

    #[tokio::test]
    async fn the_budgetless_constructor_sends_raw_and_therefore_hits_the_ceiling() {
        // `new` opts out with usize::MAX. Asserted as an observed ERROR rather than by
        // reading the field back: the opt-out only means anything if the text actually
        // reaches the embedder whole, and this is the behaviour every existing caller
        // (all of them tests, with ceiling-less mocks) still gets.
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::new(emb.clone());

        svc.embed_artifact(None, &"x".repeat(5_000))
            .await
            .expect_err("opting out must send it raw, and raw hits the ceiling");

        assert_eq!(
            emb.calls.load(Ordering::SeqCst),
            1,
            "raw means exactly one request, not a segmented retry"
        );
    }

    #[tokio::test]
    async fn a_pooled_artifact_vector_is_renormalised_to_unit_length() {
        // Guards that this site pools through `mean_pool_normalized` rather than
        // hand-rolling a mean. An unnormalised mean of k unit vectors has norm < 1 and
        // shrinks as they diverge, which would push every SEGMENTED artifact
        // systematically further from every query than a short one — a ranking defect that
        // no "did it embed" assertion can see, and the reason that function's own doc
        // comment calls renormalising "not cosmetic".
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::with_budget(emb, BUDGET);

        let v = svc.embed_artifact(None, &"x".repeat(5_000)).await.unwrap();

        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "pooled vector must be unit-norm, got {norm} — an un-renormalised mean of \
             divergent unit vectors lands near 0.577 here"
        );
    }
    /// The budget only exists if the production wiring passes one, and no other test here
    /// can see that — every test above constructs the service directly, so all four stay
    /// green with the real callers reverted to `new`.
    ///
    /// That is the `declared-not-wired` shape, which this repo has already shipped: two
    /// `Tool` impls that satisfied the trait, were registered nowhere, and carried a
    /// passing suite for months. Pinned at the source level, the same device as
    /// `claude_md_gate_lists_its_four_commands_in_the_load_bearing_order`. If a
    /// construction site ever legitimately wants no budget, this test is where to say so.
    #[test]
    fn every_production_construction_site_passes_a_budget() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/librarian/mod.rs"))
                .expect("src/librarian/mod.rs must be readable");

        assert_eq!(
            src.matches("EmbeddingService::new(").count(),
            0,
            "src/librarian/mod.rs must build the service with `with_budget`, never `new` — \
             `new` opts out of segmentation via usize::MAX, so a bare `new` here silently \
             restores the defect this fixed: an oversized artifact rejected by the backend \
             and left permanently vectorless, with no error downstream"
        );

        // Paired deliberately. The assertion above is an ABSENCE, so it is monotone under
        // removal: deleting both construction sites would satisfy it perfectly while
        // embedding nothing at all. This is the direction it cannot see.
        assert!(
            src.contains("EmbeddingService::with_budget("),
            "and the service must still be constructed — a file with neither form passes \
             the absence check above while the librarian embeds nothing"
        );
    }
}
