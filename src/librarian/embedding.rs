use crate::librarian::catalog::chunk::embed_input_hash;
use crate::librarian::vector_cache::{
    CacheLookup, CacheStatus, FlushOutcome, ModelKey, VectorCache,
};
use anyhow::Result;
use codescout_embed::Embedder;
use std::sync::Arc;

/// What [`EmbeddingService::embed_artifact_cached`] returns.
#[must_use = "the vector and the cache report are lost when this is dropped"]
pub struct CachedEmbed {
    /// The vector, or the embedder's error. A cache problem is never an error here.
    pub vector: Result<Vec<f32>>,
    /// The vector came from the cache and the embedder was not called.
    pub from_cache: bool,
    /// The cache failed on this item (lookup, or the vector could not be staged).
    pub cache_failed: bool,
}

/// A cache and the key a service reads and writes it under.
struct CacheSeam {
    cache: Arc<VectorCache>,
    model: ModelKey,
}

/// Writes the vectors a run staged in the vector cache when it drops, however the run ends.
///
/// A run that embeds through [`EmbeddingService::embed_artifact_cached`] leaves through a `?`,
/// a panic or a dropped future at places that each follow some embeds, so a flush written at the
/// end of the function is skipped by exactly the runs that need it. The explicit flushes beside
/// each store write give one cache transaction per page or batch; this guard is the backstop for
/// every other way out. A run that reaches its report has flushed after its last write, so the
/// flush here finds nothing staged and does nothing, as long as one run at a time uses the cache.
/// Today the CLI calls the backfill once per process and `index_repo` has no production caller
/// (its only caller is test code), so the other user of a shared cache would be a concurrent
/// reindex call.
/// The flush outcome is not reported.
///
/// The cache is one per process, so the guard also writes the rows another call staged and had
/// not flushed yet. They are valid vectors, and a later flush would write them anyway.
#[must_use = "the flush runs when the guard drops: bind it to a name, not `_`"]
pub struct FlushCacheOnDrop<'a>(&'a EmbeddingService);

impl Drop for FlushCacheOnDrop<'_> {
    fn drop(&mut self) {
        let _ = self.0.flush_cache();
    }
}

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
    /// How many `embed_artifact` calls a reindex keeps in flight at once. `1` — the default —
    /// is the sequential behaviour every constructor starts with; only
    /// [`Self::with_concurrency`] raises it, from `LIBRARIAN_EMBED_CONCURRENCY`.
    ///
    /// It is a setting the OPERATOR makes, not one derived from the backend or the host name:
    /// how well an embedding server scales with parallel requests depends on its hardware and
    /// its own slot count, which the client cannot see. Measured 2026-11-07 against a local
    /// llama-server (CodeRankEmbed, ROCm GPU, `--parallel 16`): 12.2 ms per text at 1 request in
    /// flight, 16.6 ms at 8, 19.0 ms at 16. A server on other hardware may scale the other way.
    concurrency: usize,
    /// The vector cache and the key this service uses it under. `None` in `new` and
    /// `with_budget`, so the cache seam passes through for every service that never asked for it.
    /// Set only by [`Self::with_vector_cache`], which sets both together.
    seam: Option<CacheSeam>,
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
            concurrency: 1,
            seam: None,
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
            concurrency: 1,
            seam: None,
        }
    }

    /// Keep up to `n` artifact embeds in flight during a reindex. `0` is taken as `1`: there is
    /// no "zero requests at a time", and a stalled reindex would be the only reading of it.
    ///
    /// Every constructor starts at 1, so a caller that never asks keeps the sequential
    /// behaviour. Whether more helps depends on the embedding server, which the client cannot
    /// see — see the field doc for one measurement where 8 was slower than 1.
    pub fn with_concurrency(mut self, n: usize) -> Self {
        self.concurrency = n.max(1);
        self
    }

    /// How many artifact embeds a reindex keeps in flight at once; always at least 1.
    pub fn concurrency(&self) -> usize {
        self.concurrency
    }

    /// Sets the cache and the model key together (C3-R10). Chain it after `with_budget`,
    /// because the key carries the budget. `model` is the configured model string, never a
    /// literal.
    pub fn with_vector_cache(mut self, model: &str, cache: Arc<VectorCache>) -> Self {
        let model = ModelKey::new(model, self.budget_chars);
        self.seam = Some(CacheSeam { cache, model });
        self
    }

    /// The key this service reads and writes cache rows under. `None` without a cache.
    pub fn model_key(&self) -> Option<&ModelKey> {
        self.seam.as_ref().map(|s| &s.model)
    }

    /// For inspection only (status, path, tests). A caller must not stage rows around the
    /// seam: `embed_artifact_cached` is the one writer.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn vector_cache(&self) -> Option<&VectorCache> {
        self.seam.as_ref().map(|s| s.cache.as_ref())
    }

    /// `Off` without a cache.
    pub fn cache_status(&self) -> CacheStatus {
        self.seam
            .as_ref()
            .map_or(CacheStatus::Off, |s| s.cache.status())
    }

    /// Lets the next call that needs the cache file open it again after a failed open.
    /// Nothing without a cache.
    pub fn rearm_cache(&self) {
        if let Some(seam) = &self.seam {
            seam.cache.rearm();
        }
    }

    /// Embed an artifact through the vector cache.
    ///
    /// Without a cache this is `embed_artifact` and nothing else. With one:
    /// - `read_cache` true: look the vector up first, under the live dimension. A hit skips
    ///   the embedder. A dimension of 0 (a remote embedder before its first embed) is a miss.
    /// - The embedder's vector is staged for the next `flush_cache`, whatever `read_cache` was,
    ///   so `read_cache = false` still refreshes the row. A vector whose width differs from the
    ///   live dimension is returned to the caller but never staged.
    ///
    /// A cache problem never fails the call. It sets `cache_failed`. No lock is held across the
    /// embed: the cache takes its lock inside each synchronous call.
    pub async fn embed_artifact_cached(
        &self,
        title: Option<&str>,
        body: &str,
        read_cache: bool,
    ) -> CachedEmbed {
        let Some(seam) = &self.seam else {
            return CachedEmbed {
                vector: self.embed_artifact(title, body).await,
                from_cache: false,
                cache_failed: false,
            };
        };
        let hash = embed_input_hash(title, body);
        let mut cache_failed = false;
        if read_cache {
            match seam
                .cache
                .get(&seam.model, self.embedder.dimensions(), &hash)
            {
                CacheLookup::Hit(vector) => {
                    return CachedEmbed {
                        vector: Ok(vector),
                        from_cache: true,
                        cache_failed: false,
                    };
                }
                CacheLookup::Failed => cache_failed = true,
                CacheLookup::Miss => {}
            }
        }
        let vector = self.embed_artifact(title, body).await;
        if let Ok(v) = &vector {
            let live = self.embedder.dimensions();
            let width_agrees = live == 0 || live == v.len();
            if !(width_agrees && seam.cache.stage(&seam.model, &hash, v)) {
                cache_failed = true;
            }
        }
        CachedEmbed {
            vector,
            from_cache: false,
            cache_failed,
        }
    }

    /// Write the staged vectors to the cache in one transaction. The default outcome
    /// (nothing stored, nothing failed) without a cache.
    pub fn flush_cache(&self) -> FlushOutcome {
        self.seam
            .as_ref()
            .map_or_else(FlushOutcome::default, |s| s.cache.flush())
    }

    /// A guard that writes the staged vectors when it drops, however the caller's run ends.
    /// Take it right after [`Self::rearm_cache`]; it does nothing without a cache.
    pub fn flush_on_drop(&self) -> FlushCacheOnDrop<'_> {
        FlushCacheOnDrop(self)
    }

    /// Embed one text on the **document** side.
    ///
    /// `Embedder::embed` is the document seam and never prefixes; `embed_query` is the
    /// query seam and applies an asymmetric model's query instruction. Artifact bodies are
    /// stored content, so they belong here — using the query seam stranded every stored
    /// vector in query-space, which is what this replaced.
    async fn embed_document_one(&self, text: &str) -> Result<Vec<f32>> {
        let mut batch = self.embedder.embed(&[text]).await?;
        batch
            .pop()
            .ok_or_else(|| anyhow::anyhow!("embedder returned an empty batch for one input"))
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
    /// Goes through the **document** seam ([`Self::embed_document_one`]). It used to use
    /// `embed_query`, which prefixes on an asymmetric model — so on a CodeRankEmbed
    /// deployment every stored artifact vector carried a query instruction, and the
    /// librarian reached `QueryPrefix::Derive` (the state ET-9 D1 rules out) by calling the
    /// crate constructor directly rather than `EmbedderHttp::remote_dense`. Two independent
    /// wrongs were needed, which is why neither alone was caught.
    /// `docs/issues/archive/2026-09-04-librarian-embeds-stored-artifacts-through-the-query-seam.md`
    ///
    /// One deliberate consequence: `Embedder::embed` refuses an all-whitespace batch, and
    /// the old prefix made an empty body non-empty. An artifact whose composed text is
    /// blank now surfaces as a named error in the reindex's `embed_errors` instead of
    /// quietly embedding a prefix that carried none of its content.
    pub async fn embed_artifact(&self, title: Option<&str>, body: &str) -> Result<Vec<f32>> {
        let text = format!("{}\n\n{}", title.unwrap_or(""), body);
        if text.chars().count() <= self.budget_chars {
            return self.embed_document_one(&text).await;
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
            vectors.push(self.embed_document_one(seg).await?);
        }
        crate::embed::document::mean_pool_normalized(&vectors)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use parking_lot::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};

    enum Shape {
        /// `[1, 0, 0, ...]`
        Unit,
        /// `v[0]` is the chars of the text, `v[1]` is `1.0`, the rest are `0`.
        Lengths,
    }

    /// Counts the texts it embeds. It can declare one width and return another, learn its
    /// width late (as `RemoteEmbedder` does), and fail.
    pub(crate) struct CountingEmbedder {
        declared: usize,
        returned: usize,
        shape: Shape,
        learns_late: bool,
        learned: AtomicBool,
        failing: AtomicBool,
        seen: Mutex<Vec<String>>,
    }

    impl CountingEmbedder {
        fn build(declared: usize, returned: usize, shape: Shape, learns_late: bool) -> Arc<Self> {
            Arc::new(Self {
                declared,
                returned,
                shape,
                learns_late,
                learned: AtomicBool::new(false),
                failing: AtomicBool::new(false),
                seen: Mutex::new(Vec::new()),
            })
        }

        /// Declares and returns `dim`.
        pub(crate) fn new(dim: usize) -> Arc<Self> {
            Self::build(dim, dim, Shape::Unit, false)
        }

        /// Declares `declared` and returns vectors `returned` wide.
        pub(crate) fn declaring(declared: usize, returned: usize) -> Arc<Self> {
            Self::build(declared, returned, Shape::Unit, false)
        }

        /// `dimensions()` is 0 until the first embed has returned.
        pub(crate) fn learning(dim: usize) -> Arc<Self> {
            Self::build(dim, dim, Shape::Unit, true)
        }

        /// The vector tells the length of the text it was made from.
        pub(crate) fn lengths(dim: usize) -> Arc<Self> {
            Self::build(dim, dim, Shape::Lengths, false)
        }

        pub(crate) fn set_failing(&self, on: bool) {
            self.failing.store(on, Ordering::SeqCst);
        }

        /// Texts embedded. A failed call records nothing.
        pub(crate) fn calls(&self) -> usize {
            self.seen.lock().len()
        }

        /// The exact texts, in order.
        pub(crate) fn seen(&self) -> Vec<String> {
            self.seen.lock().clone()
        }
    }

    #[async_trait::async_trait]
    impl Embedder for CountingEmbedder {
        fn dimensions(&self) -> usize {
            if self.learns_late && !self.learned.load(Ordering::SeqCst) {
                0
            } else {
                self.declared
            }
        }

        async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
            anyhow::ensure!(
                !self.failing.load(Ordering::SeqCst),
                "the counting embedder is set to fail"
            );
            let mut out = Vec::with_capacity(texts.len());
            for t in texts {
                self.seen.lock().push((*t).to_owned());
                let mut v = vec![0.0_f32; self.returned];
                match self.shape {
                    Shape::Unit => v[0] = 1.0,
                    Shape::Lengths => {
                        v[0] = t.chars().count() as f32;
                        v[1] = 1.0;
                    }
                }
                out.push(v);
            }
            self.learned.store(true, Ordering::SeqCst);
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::CountingEmbedder;
    use super::*;
    use crate::librarian::vector_cache::COMPOSITION_VERSION;
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
        /// Also refuse whitespace-only input, with the real `RemoteEmbedder::embed` message.
        refuse_blank: bool,
    }

    impl CeilingEmbedder {
        fn new(max_chars: usize) -> Self {
            Self {
                max_chars,
                calls: AtomicUsize::new(0),
                refuse_blank: false,
            }
        }

        /// As [`Self::new`], but mirrors `RemoteEmbedder::embed`'s refusal of an
        /// all-whitespace batch (`crates/codescout-embed/src/remote.rs`), which
        /// `embed_artifact` propagates, leaving the artifact with no vector.
        fn refusing_blank(max_chars: usize) -> Self {
            Self {
                refuse_blank: true,
                ..Self::new(max_chars)
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
                anyhow::ensure!(
                    !(self.refuse_blank && t.trim().is_empty()),
                    "cannot embed batch — all 1 text(s) are empty/whitespace; \
                     filter empty inputs before calling embed()"
                );
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

    // cap-class: NOT_A_CAP — test-only fixture ceiling handed to CeilingEmbedder::new and EmbeddingService::with_budget; it parameterises the test's own embedder and bounds no runtime path
    const BUDGET: usize = 500;

    /// The failure text of the three pins on how `embed_artifact` builds a vector.
    const BUMP_COMPOSITION_VERSION: &str = "the join, the segmentation or the pooling in \
         embed_artifact changed: bump COMPOSITION_VERSION in vector_cache.rs, or cached vectors \
         made the old way are served for the new text";

    fn cached(emb: &Arc<CountingEmbedder>, cache: &Arc<VectorCache>) -> EmbeddingService {
        EmbeddingService::new(emb.clone()).with_vector_cache("test-model", cache.clone())
    }

    /// The key `cached` builds: model `test-model`, no budget.
    fn test_key() -> ModelKey {
        ModelKey::new("test-model", usize::MAX)
    }

    /// Put one row in the cache, as an earlier process would have.
    fn seed(cache: &VectorCache, title: &str, body: &str, vector: &[f32]) {
        let hash = embed_input_hash(Some(title), body);
        assert!(cache.stage(&test_key(), &hash, vector));
        assert_eq!(cache.flush().stored, 1);
    }

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

    /// A whitespace run between two over-budget tokens used to be emitted as a segment of
    /// its own; the backend refuses a blank input and `embed_artifact` propagates it, so the
    /// artifact got no vector at all. Untitled, as production's empty-title case, so the
    /// composed text also begins with a blank `"\n\n"` group.
    #[tokio::test]
    async fn a_blank_run_inside_a_long_artifact_does_not_cost_it_its_vector() {
        let emb = Arc::new(CeilingEmbedder::refusing_blank(100));
        let svc = EmbeddingService::with_budget(emb, 100);
        let body = format!("{}{}{}", "x".repeat(100), " ".repeat(60), "y".repeat(100));
        let v = svc
            .embed_artifact(None, &body)
            .await
            .expect("a blank segment reached the embedder and was refused");
        assert_eq!(v.len(), 3);
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
    #[test]
    fn every_constructor_starts_at_one_embed_at_a_time() {
        // The default is what keeps every existing caller sequential. Both constructors are checked
        // because a future third one that forgot the field would not compile, but one that set it to
        // something else would.
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        assert_eq!(EmbeddingService::new(emb.clone()).concurrency(), 1);
        assert_eq!(EmbeddingService::with_budget(emb, BUDGET).concurrency(), 1);
    }

    #[test]
    fn with_concurrency_sets_the_value_and_zero_is_one() {
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::with_budget(emb.clone(), BUDGET).with_concurrency(8);
        assert_eq!(svc.concurrency(), 8);
        // Zero must not reach `buffer_unordered(0)`, which never polls a future.
        let svc = EmbeddingService::with_budget(emb, BUDGET).with_concurrency(0);
        assert_eq!(svc.concurrency(), 1);
    }

    #[test]
    fn with_concurrency_keeps_the_segmentation_budget() {
        // The builder consumes and returns the service: a bug that rebuilt it through `new` would
        // silently reset the budget to `usize::MAX` and send oversized artifacts whole.
        let emb = Arc::new(CeilingEmbedder::new(BUDGET));
        let svc = EmbeddingService::with_budget(emb, BUDGET).with_concurrency(4);
        assert_eq!(svc.budget_chars, BUDGET);
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
    /// Same device as the budget test above, for the operator's concurrency setting. The behavioural
    /// test in `librarian::tests` reaches the `build_tool_context_with` site only; `reindex_cli`
    /// builds its own service and no test can observe how many embeds it runs at once, so a site that
    /// forgot `with_concurrency` would pass everything and ignore `LIBRARIAN_EMBED_CONCURRENCY`
    /// there. Counting both forms ties them together: a new construction site that omits the setting
    /// makes the two counts differ.
    #[test]
    fn every_production_construction_site_passes_the_concurrency_setting() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/librarian/mod.rs"))
                .expect("src/librarian/mod.rs must be readable");

        let sites = src.matches("EmbeddingService::with_budget(").count();
        let passing = src
            .matches(".with_concurrency(env.embed_concurrency.unwrap_or(1))")
            .count();

        assert!(
            sites >= 2,
            "the librarian builds the service at two production sites (the tool context and the \
         reindex CLI); found {sites}"
        );
        assert_eq!(
            passing, sites,
            "every `EmbeddingService::with_budget(` in src/librarian/mod.rs must be followed by \
         `.with_concurrency(env.embed_concurrency.unwrap_or(1))`, or \
         LIBRARIAN_EMBED_CONCURRENCY is silently ignored at that site"
        );
    }

    /// Stored content must not go through the QUERY seam.
    ///
    /// `embed_query` is where `RemoteEmbedder` applies an asymmetric model's query
    /// instruction, so an artifact embedded through it is stored in query-space. The bug
    /// file called this observation owed: the chain had been verified link by link by
    /// reading, and nothing had ever watched a call take the right seam.
    ///
    /// The double records inside its **override of `embed_query`**, and that is the whole
    /// discriminator. `Embedder::embed_query` has a default implementation that delegates
    /// to `embed`, so a recorder counting only `embed` calls cannot tell the two seams
    /// apart — both arrive there. Overriding the query side is what makes the wrong seam
    /// observable at all.
    #[tokio::test]
    async fn an_artifact_is_embedded_on_the_document_seam_never_the_query_seam() {
        #[derive(Default)]
        struct SeamRecorder {
            via_query: std::sync::atomic::AtomicBool,
        }

        #[async_trait::async_trait]
        impl Embedder for SeamRecorder {
            fn dimensions(&self) -> usize {
                3
            }
            async fn embed(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
                Ok(texts.iter().map(|_| vec![1.0, 0.0, 0.0]).collect())
            }
            async fn embed_query(&self, text: &str) -> Result<Vec<f32>> {
                self.via_query.store(true, Ordering::SeqCst);
                let mut b = self.embed(&[text]).await?;
                Ok(b.pop().expect("non-empty batch"))
            }
        }

        // The single-call path.
        let emb = Arc::new(SeamRecorder::default());
        EmbeddingService::new(emb.clone())
            .embed_artifact(Some("IC-18"), "a stored body")
            .await
            .unwrap();
        assert!(
            !emb.via_query.load(Ordering::SeqCst),
            "an artifact body reached the query seam — on an asymmetric model that stores \
             it in query-space, which is the defect this replaced"
        );

        // And the SEGMENTED path, which is a second call site of the same helper: a change
        // that restored `embed_query` in only one of the two would leave the other green.
        let emb2 = Arc::new(SeamRecorder::default());
        EmbeddingService::with_budget(emb2.clone(), 50)
            .embed_artifact(None, &"x".repeat(500))
            .await
            .unwrap();
        assert!(
            !emb2.via_query.load(Ordering::SeqCst),
            "a segmented artifact reached the query seam"
        );
    }

    #[tokio::test]
    async fn a_service_without_a_cache_embeds_every_time() {
        let emb = CountingEmbedder::new(4);
        let svc = EmbeddingService::new(emb.clone());
        let a = svc.embed_artifact_cached(Some("t"), "b", true).await;
        let b = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(!a.from_cache && !b.from_cache);
        assert!(!a.cache_failed && !b.cache_failed);
        assert_eq!(emb.calls(), 2);
        assert_eq!(svc.cache_status(), CacheStatus::Off);
        assert!(svc.model_key().is_none());
        assert!(svc.vector_cache().is_none());
        assert_eq!(svc.flush_cache(), FlushOutcome::default());
    }

    #[tokio::test]
    async fn a_second_identical_input_is_served_from_the_cache_after_a_flush() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);

        let first = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(!first.from_cache);
        assert!(!first.cache_failed);
        assert_eq!(
            svc.flush_cache(),
            FlushOutcome {
                stored: 1,
                failed: 0
            }
        );

        let key = svc.model_key().expect("a cache brings a key").clone();
        let hash = embed_input_hash(Some("t"), "b");
        assert_eq!(
            cache.get(&key, 4, &hash),
            CacheLookup::Hit(vec![1.0, 0.0, 0.0, 0.0])
        );

        let second = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(second.from_cache);
        assert_eq!(emb.calls(), 1);
        assert_eq!(second.vector.unwrap(), first.vector.unwrap());
    }

    #[tokio::test]
    async fn an_unflushed_vector_is_not_served() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);

        let _staged = svc.embed_artifact_cached(Some("t"), "b", true).await;
        let second = svc.embed_artifact_cached(Some("t"), "b", true).await;

        assert!(!second.from_cache);
        assert_eq!(emb.calls(), 2);
    }

    #[tokio::test]
    async fn the_model_string_and_the_budget_each_change_the_key() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        let first = cached(&emb, &cache);
        let _stored = first.embed_artifact_cached(Some("t"), "b", true).await;
        assert_eq!(first.flush_cache().stored, 1);

        let other_model = EmbeddingService::new(emb.clone()).with_vector_cache("m2", cache.clone());
        let got = other_model
            .embed_artifact_cached(Some("t"), "b", true)
            .await;
        assert!(!got.from_cache, "another model string must miss");

        let other_budget = EmbeddingService::with_budget(emb.clone(), 999)
            .with_vector_cache("test-model", cache.clone());
        let got = other_budget
            .embed_artifact_cached(Some("t"), "b", true)
            .await;
        assert!(!got.from_cache, "another budget must miss");

        let same = cached(&emb, &cache);
        let got = same.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(got.from_cache, "the same model and budget must hit");
    }

    #[tokio::test]
    async fn the_same_text_under_another_title_is_a_miss() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);
        let _stored = svc.embed_artifact_cached(Some("A"), "body", true).await;
        assert_eq!(svc.flush_cache().stored, 1);

        let b = svc.embed_artifact_cached(Some("B"), "body", true).await;
        assert!(!b.from_cache);
        let none = svc.embed_artifact_cached(None, "body", true).await;
        assert!(!none.from_cache);
        let a = svc.embed_artifact_cached(Some("A"), "body", true).await;
        assert!(a.from_cache, "the stored title still hits");
    }

    #[tokio::test]
    async fn an_unknown_live_dimension_makes_the_first_lookup_a_miss() {
        // The `RemoteEmbedder` shape: `dimensions()` is 0 until its first embed.
        let emb = CountingEmbedder::learning(4);
        let cache = Arc::new(VectorCache::in_memory());
        seed(&cache, "t", "b", &[9.0; 4]);
        seed(&cache, "t", "c", &[7.0; 4]);
        let svc = cached(&emb, &cache);
        assert_eq!(emb.dimensions(), 0, "the double must start without a width");

        let first = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(!first.from_cache, "a lookup at dimension 0 is a miss");
        assert_eq!(first.vector.unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
        assert_eq!(emb.calls(), 1);
        assert_eq!(emb.dimensions(), 4);

        let second = svc.embed_artifact_cached(Some("t"), "c", true).await;
        assert!(second.from_cache);
        assert_eq!(second.vector.unwrap(), vec![7.0; 4]);
        assert_eq!(emb.calls(), 1);
    }

    #[tokio::test]
    async fn a_vector_of_the_wrong_width_is_not_cached() {
        let emb = CountingEmbedder::declaring(4, 3);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);

        let got = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert_eq!(got.vector.unwrap().len(), 3, "the caller still gets it");
        assert!(got.cache_failed);
        assert_eq!(
            svc.flush_cache(),
            FlushOutcome {
                stored: 0,
                failed: 0
            }
        );
    }

    #[tokio::test]
    async fn the_stored_vector_is_made_from_the_title_and_body_it_is_keyed_by() {
        // The other cached tests use a double whose vector ignores the text. This one does not:
        // a seam that hashed (title, body) but embedded something else would store a vector
        // under a key it does not belong to.
        let emb = CountingEmbedder::lengths(4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);
        let expected = EmbeddingService::new(CountingEmbedder::lengths(4))
            .embed_artifact(Some("t"), "b")
            .await
            .unwrap();

        let first = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert_eq!(emb.seen(), vec!["t\n\nb".to_owned()]);
        assert_eq!(svc.flush_cache().stored, 1);
        assert_eq!(first.vector.unwrap(), expected);

        let hit = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(hit.from_cache);
        assert_eq!(
            hit.vector.unwrap(),
            expected,
            "the hit is the vector of the text"
        );

        // The title is in the text, so another title gives another vector, and its own row.
        let other = svc
            .embed_artifact_cached(Some("longer title"), "b", true)
            .await;
        assert!(!other.from_cache);
        let other_vector = other.vector.unwrap();
        assert_ne!(other_vector, expected);
        assert_eq!(svc.flush_cache().stored, 1);
        let other_hit = svc
            .embed_artifact_cached(Some("longer title"), "b", true)
            .await;
        assert!(other_hit.from_cache);
        assert_eq!(other_hit.vector.unwrap(), other_vector);
    }

    #[tokio::test]
    async fn a_corrupt_row_is_a_failed_lookup_and_the_next_flush_repairs_it() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vector-cache.db");
        let cache = Arc::new(VectorCache::open(&path));
        seed(&cache, "t", "b", &[9.0; 4]);
        let raw = rusqlite::Connection::open(&path).unwrap();
        raw.execute_batch("UPDATE vec SET vector = zeroblob(5)")
            .unwrap();
        let emb = CountingEmbedder::new(4);
        let svc = cached(&emb, &cache);

        let got = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(
            got.cache_failed,
            "a row of the wrong size is a cache failure"
        );
        assert!(!got.from_cache);
        assert_eq!(got.vector.unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
        assert_eq!(svc.flush_cache().stored, 1);

        let again = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(again.from_cache, "the flush replaced the corrupt row");
        assert!(!again.cache_failed);
        assert_eq!(again.vector.unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
        assert_eq!(emb.calls(), 1);
    }

    #[tokio::test]
    async fn a_vector_is_staged_when_the_live_dimension_is_unknown() {
        // Declares 0 and returns 4 wide: the width check lets it through, as it must for an
        // embedder that has not learned its width.
        let emb = CountingEmbedder::declaring(0, 4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);

        let got = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(!got.cache_failed);
        assert_eq!(got.vector.unwrap().len(), 4);
        assert_eq!(
            svc.flush_cache(),
            FlushOutcome {
                stored: 1,
                failed: 0
            }
        );
    }

    #[tokio::test]
    async fn reading_the_cache_can_be_skipped_and_the_write_still_refreshes_it() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        seed(&cache, "t", "b", &[9.0; 4]);
        let svc = cached(&emb, &cache);

        let skipped = svc.embed_artifact_cached(Some("t"), "b", false).await;
        assert!(!skipped.from_cache);
        assert_eq!(skipped.vector.unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
        assert_eq!(svc.flush_cache().stored, 1);

        let after = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(after.from_cache);
        assert_eq!(
            after.vector.unwrap(),
            vec![1.0, 0.0, 0.0, 0.0],
            "the refresh replaced the seeded row"
        );
        assert_eq!(emb.calls(), 1);
    }

    #[tokio::test]
    async fn a_failing_cache_degrades_to_the_embedder() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("vector-cache.db");
        let garbage: Vec<u8> = (0..200u32).map(|i| (i * 7 + 3) as u8).collect();
        std::fs::write(&path, &garbage).unwrap();
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::open(&path));
        let svc = cached(&emb, &cache);

        let got = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(!got.from_cache);
        assert!(got.cache_failed);
        assert_eq!(got.vector.unwrap(), vec![1.0, 0.0, 0.0, 0.0]);
        assert_eq!(svc.cache_status(), CacheStatus::Unavailable);
    }

    #[tokio::test]
    async fn an_embedder_error_is_returned_and_nothing_is_cached() {
        let emb = CountingEmbedder::new(4);
        let cache = Arc::new(VectorCache::in_memory());
        let svc = cached(&emb, &cache);
        emb.set_failing(true);

        let got = svc.embed_artifact_cached(Some("t"), "b", true).await;
        assert!(got.vector.is_err());
        assert!(!got.from_cache);
        assert!(!got.cache_failed, "the cache did nothing wrong");
        assert_eq!(
            svc.flush_cache(),
            FlushOutcome {
                stored: 0,
                failed: 0
            }
        );
    }

    #[tokio::test]
    async fn the_text_sent_to_the_embedder_is_pinned_to_the_composition_version() {
        // Whitespace in the inputs: a trim or a normalisation of the join would change them.
        let emb = CountingEmbedder::new(4);
        let svc = EmbeddingService::new(emb.clone());
        svc.embed_artifact(Some(" T "), " B\n").await.unwrap();
        svc.embed_artifact(None, " B\n").await.unwrap();
        assert_eq!(
            emb.seen(),
            vec![" T \n\n B\n".to_owned(), "\n\n B\n".to_owned()],
            "{BUMP_COMPOSITION_VERSION}"
        );
        assert_eq!(COMPOSITION_VERSION, 1, "{BUMP_COMPOSITION_VERSION}");
    }

    #[tokio::test]
    async fn the_segmentation_and_pooling_of_a_long_input_are_pinned_to_the_composition_version() {
        // Measured on the code before the cache seam existed. The unit-length test above
        // checks the norm only, so a changed segment size would pass it. This one would not.
        let emb = CountingEmbedder::lengths(4);
        let svc = EmbeddingService::with_budget(emb.clone(), 50);
        let v = svc.embed_artifact(None, &"x".repeat(500)).await.unwrap();

        let lengths: Vec<usize> = emb.seen().iter().map(|s| s.chars().count()).collect();
        assert_eq!(
            lengths,
            vec![3, 49, 50, 50, 50, 50, 50, 50, 50, 50, 50],
            "{BUMP_COMPOSITION_VERSION}"
        );
        let rounded: Vec<String> = v.iter().map(|x| format!("{x:.6}")).collect();
        assert_eq!(
            rounded,
            vec!["0.999760", "0.021907", "0.000000", "0.000000"],
            "{BUMP_COMPOSITION_VERSION}"
        );
        assert_eq!(COMPOSITION_VERSION, 1, "{BUMP_COMPOSITION_VERSION}");
    }
    #[tokio::test]
    async fn the_segmentation_of_spaced_multi_line_text_is_pinned_to_the_composition_version() {
        // The test above cuts one line with no whitespace, so it reaches only the hard cut. This
        // one has lines, spaces and one long line with spaces, so it reaches the line packing and
        // the cut between words. The first text is exactly the budget, so it goes whole.
        let emb = CountingEmbedder::lengths(4);
        let svc = EmbeddingService::with_budget(emb.clone(), 40);
        let at_budget = svc
            .embed_artifact(Some("T"), "the quick brown fox jumps over a lazy")
            .await
            .unwrap();
        let body = "first line of the notes\nsecond line, a bit more\n\
                    a long line with many words that is cut between words and never inside one\n\
                    tail";
        let pooled = svc.embed_artifact(Some("Notes"), body).await.unwrap();

        assert_eq!(
            emb.seen(),
            vec![
                // Exactly the budget: one call, whole.
                "T\n\nthe quick brown fox jumps over a lazy".to_owned(),
                // Whole lines pack up to the budget.
                "Notes\n\nfirst line of the notes\n".to_owned(),
                "second line, a bit more\n".to_owned(),
                // The long line is cut after the last space inside the budget window.
                "a long line with many words that is cut ".to_owned(),
                "between words and never inside one\n".to_owned(),
                "tail".to_owned(),
            ],
            "{BUMP_COMPOSITION_VERSION}"
        );
        let rounded = |v: &[f32]| -> Vec<String> { v.iter().map(|x| format!("{x:.6}")).collect() };
        assert_eq!(
            rounded(&at_budget),
            vec!["40.000000", "1.000000", "0.000000", "0.000000"],
            "a text at the budget is not pooled: {BUMP_COMPOSITION_VERSION}"
        );
        assert_eq!(
            rounded(&pooled),
            vec!["0.999305", "0.037287", "0.000000", "0.000000"],
            "{BUMP_COMPOSITION_VERSION}"
        );
        assert_eq!(COMPOSITION_VERSION, 1, "{BUMP_COMPOSITION_VERSION}");
    }

    /// Same device as the budget and concurrency guards. Each chained call is one short line
    /// that rustfmt leaves alone, so plain substrings keep matching after formatting.
    #[test]
    fn every_production_construction_site_attaches_the_vector_cache_with_its_model() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/librarian/mod.rs"))
                .expect("src/librarian/mod.rs must be readable");

        let sites = src.matches("EmbeddingService::with_budget(").count();
        let attached = src.matches(".with_vector_cache(model, cache)").count();
        let bound = src
            .matches("VectorCache::open(&paths.vector_cache)")
            .count();

        assert!(sites >= 1, "found no construction site; the scan is stale");
        assert!(
            attached >= 1,
            "no site attaches the cache; the paired check against a removal"
        );
        assert_eq!(
            attached, sites,
            "every `EmbeddingService::with_budget(` in src/librarian/mod.rs must be followed by \
             `.with_vector_cache(model, cache)`, or that site embeds without the vector cache"
        );
        assert_eq!(
            bound, sites,
            "every site opens the machine's cache (`paths.vector_cache`), never a path made from \
             a catalog it opens"
        );
        assert_eq!(
            src.matches(".with_vector_cache(\"").count(),
            0,
            "the model passed to `with_vector_cache` is the configured one, never a literal"
        );
    }
    /// The production region of a source file: the lines before the first one that trims to
    /// `mod tests {`, without the comment lines.
    fn production_lines(text: &str) -> Vec<&str> {
        text.lines()
            .take_while(|l| l.trim() != "mod tests {")
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect()
    }

    /// No production code embeds a chunk around the vector cache. `embed_artifact` is the
    /// uncached embed; the one caller outside the seam is the dimension probe of the backfill,
    /// which is compatibility information and never a vector worth keeping. Every other site
    /// calls `embed_artifact_cached`.
    ///
    /// The bare calls are exactly the seam's two (the pass-through of a service without a cache,
    /// and the miss path) and the probe. The cached calls are the reindex tool's drain and the
    /// two sites of the indexer (the backfill and `index_repo`), so a site that loses its call
    /// cannot pass: the absence check on bare calls alone is monotone under removal.
    ///
    /// Named blind spots:
    /// - Production code that sits after an inline `mod tests {` is not scanned. The behavioural
    ///   test of each site covers it.
    /// - Only `embed_artifact` is counted. A direct `embedder.embed(` call on the service's
    ///   embedder also bypasses the cache, and this scan does not see it.
    /// - A file that is test-only and has no `mod tests {` line (for example
    ///   `src/librarian/tools/read_door_tests.rs`) is scanned as production, so an
    ///   `embed_artifact` call in it would be counted.
    /// - The two bare calls counted in `embedding.rs` are not checked to sit inside
    ///   `embed_artifact_cached`: a third bare call in another function there would fail the
    ///   count, but moving one of the two out of the seam would pass.
    #[test]
    fn every_embed_call_outside_the_seam_is_the_dimension_probe() {
        use std::collections::BTreeMap;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut bare: BTreeMap<String, usize> = BTreeMap::new();
        let mut cached: BTreeMap<String, usize> = BTreeMap::new();
        let mut indexer_bare_lines: Vec<String> = Vec::new();
        for entry in walkdir::WalkDir::new(root.join("src"))
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.extension().is_some_and(|x| x == "rs") {
                continue;
            }
            let text = std::fs::read_to_string(path).unwrap();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for line in production_lines(&text) {
                let n_bare = line.matches(".embed_artifact(").count()
                    + line.matches("::embed_artifact(").count();
                let n_cached = line.matches(".embed_artifact_cached(").count()
                    + line.matches("::embed_artifact_cached(").count();
                if n_bare > 0 {
                    *bare.entry(rel.clone()).or_default() += n_bare;
                    if rel == "src/librarian/indexer.rs" {
                        indexer_bare_lines.push(line.to_string());
                    }
                }
                if n_cached > 0 {
                    *cached.entry(rel.clone()).or_default() += n_cached;
                }
            }
        }

        let want_bare: BTreeMap<String, usize> = [
            ("src/librarian/embedding.rs".to_string(), 2),
            ("src/librarian/indexer.rs".to_string(), 1),
        ]
        .into();
        assert_eq!(
            bare, want_bare,
            "a production `embed_artifact` call outside the seam skips the vector cache: use \
             `embed_artifact_cached` and flush with `flush_cache`. Only the seam's two calls and \
             the backfill's dimension probe may stay bare"
        );
        assert_eq!(
            indexer_bare_lines.len(),
            1,
            "{indexer_bare_lines:?}: the one bare call in the indexer"
        );
        assert!(
            indexer_bare_lines[0].contains("\"codescout backfill dimension probe\""),
            "the one bare call in the indexer is the dimension probe, not: {}",
            indexer_bare_lines[0]
        );

        let want_cached: BTreeMap<String, usize> = [
            ("src/librarian/indexer.rs".to_string(), 2),
            ("src/librarian/tools/reindex.rs".to_string(), 1),
        ]
        .into();
        assert_eq!(
            cached, want_cached,
            "the three embed sites that go through the vector cache: the reindex drain, the \
             backfill and `index_repo`. A site that loses its call embeds around the cache"
        );
    }
}
