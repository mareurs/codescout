//! Embedding a whole stored document, when it may exceed the model's per-request budget.
//!
//! Lives here rather than in the memory tool because it has **two** consumers with the
//! same hazard, and only one of them had it. `26feb1aa` added segmentation to the tool
//! layer (`cross_embed_memory`, `create_semantic_anchors`) and left
//! `HttpMigrationEmbedder` embedding raw — so `codescout migrate-memories`, whose entire
//! job is repairing what the unsegmented path lost, inherited the pre-fix code and could
//! not repair the largest memory in the corpus.
//!
//! Measured 2026-08-26: `migrate-memories --in-place` recovered 7 of 8 missing memories
//! and failed on `eval-design` (31 596 B) with llama.cpp's
//! `input is too large to process. increase the physical batch size`.
//! `docs/issues/2026-08-26-migration-embedder-lacks-the-segmentation-the-tool-path-has.md`
//!
//! The budget itself comes from [`super::chunk_size_for_model`], which is re-exported
//! from this module's parent — the reason this is the right home for the pooling that
//! depends on it.

/// Embed `content` as a document, segmenting and mean-pooling when it exceeds the
/// configured model's budget.
///
/// Below the budget this is exactly a single call — no pooling, no change in behaviour,
/// so the common case is untouched. Above it, the unsegmented code sent the whole thing in
/// one request and the two backends then diverged: llama-server returned HTTP 500 and the
/// caller stored the memory with NO vector (invisible to `recall`), while the local ONNX
/// path silently truncated at fastembed's 512-token default and stored a vector
/// representing only the opening fraction. The second is worse, because nothing anywhere
/// reports it.
///
/// Takes `&dyn DenseEmbedder` and calls `embed_document`, never `embed`. A migration
/// re-embeds *stored* content, so it must stay on the document side; `embed` is the query
/// seam and applies an asymmetric model's query prefix, which would re-create the defect
/// a re-embed exists to repair.
/// `docs/issues/archive/2026-08-11-memory-documents-stored-query-prefixed.md`
///
/// `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`
pub async fn embed_document_pooled(
    embedder: &dyn crate::retrieval::embedder::DenseEmbedder,
    content: &str,
    budget_chars: usize,
) -> anyhow::Result<Vec<f32>> {
    if content.chars().count() <= budget_chars {
        return embedder.embed_document(content).await;
    }

    let segments = segment_for_budget(content, budget_chars);
    tracing::debug!(
        segments = segments.len(),
        budget_chars,
        content_chars = content.chars().count(),
        "document exceeds the embedding model's budget — segmenting and mean-pooling"
    );

    let mut vectors: Vec<Vec<f32>> = Vec::with_capacity(segments.len());
    for seg in &segments {
        vectors.push(embedder.embed_document(seg).await?);
    }
    mean_pool_normalized(&vectors)
}

/// Split `content` into pieces no longer than `budget_chars`, preferring line boundaries.
///
/// Pure, so the boundary logic is testable without an embedder — which matters because
/// the interesting cases (a single over-long line, an exact-budget fit) are awkward to
/// provoke through a live backend.
///
/// A line longer than the budget on its own is hard-split by characters: no boundary can
/// help, and dropping it would be the silent loss this exists to stop.
pub fn segment_for_budget(content: &str, budget_chars: usize) -> Vec<String> {
    let budget = budget_chars.max(1);
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;

    for line in content.split_inclusive('\n') {
        let line_len = line.chars().count();

        if line_len > budget {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
                cur_len = 0;
            }
            let mut piece = String::new();
            let mut piece_len = 0usize;
            for ch in line.chars() {
                if piece_len == budget {
                    out.push(std::mem::take(&mut piece));
                    piece_len = 0;
                }
                piece.push(ch);
                piece_len += 1;
            }
            if !piece.is_empty() {
                out.push(piece);
            }
            continue;
        }

        if cur_len + line_len > budget && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
        cur.push_str(line);
        cur_len += line_len;
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Mean-pool `vectors` and re-normalise to unit length.
///
/// **Re-normalising is not cosmetic.** Every backend measured returns unit-norm vectors
/// (verified 2026-08-26: CodeRankEmbed L2 = 1.000000 for both a short and a long input),
/// and the sqlite-vec store queries with `embedding MATCH vec_f32(?)`, whose metric is L2
/// distance — where magnitude is emphatically not free. The mean of k unit vectors has
/// norm ≤ 1 and shrinks as they diverge, so an unnormalised pooled vector would put every
/// *segmented* document systematically further from every query than a short one, in
/// proportion to how varied its content is. That is a ranking bug that no test of "did it
/// embed" would catch.
pub fn mean_pool_normalized(vectors: &[Vec<f32>]) -> anyhow::Result<Vec<f32>> {
    let first = vectors
        .first()
        .ok_or_else(|| anyhow::anyhow!("nothing to pool: no segment produced a vector"))?;
    let dim = first.len();
    anyhow::ensure!(dim > 0, "embedder returned a zero-dimension vector");

    let mut acc = vec![0f64; dim];
    for v in vectors {
        anyhow::ensure!(
            v.len() == dim,
            "embedder returned inconsistent dimensions across segments: {} vs {dim}",
            v.len()
        );
        for (a, x) in acc.iter_mut().zip(v.iter()) {
            *a += f64::from(*x);
        }
    }

    let norm = acc.iter().map(|x| x * x).sum::<f64>().sqrt();
    if norm == 0.0 {
        // Degenerate (all-zero segments). Return it as-is rather than dividing by zero —
        // a zero vector is a legible "no signal", a NaN vector poisons the index silently.
        return Ok(vec![0f32; dim]);
    }
    Ok(acc.into_iter().map(|x| (x / norm) as f32).collect())
}
