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
//! `docs/issues/archive/2026-08-26-migration-embedder-lacks-the-segmentation-the-tool-path-has.md`
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
/// A line longer than the budget on its own cannot be split on a line boundary, and
/// dropping it would be the silent loss this exists to stop, so it is cut inside the line —
/// at the best boundary the window offers, see [`split_long_line`]. The cut used to fall
/// every `budget` chars regardless of content, which put a segment boundary in the middle of
/// a word and pooled vectors of half-sentences: the 26 KB `**Members:**` lines of the
/// issue-cluster ledger were embedded that way.
///
/// **Always lossless.** Concatenating the returned segments reproduces `content` byte for
/// byte: a cut never discards whitespace, it leaves it on the earlier segment.
///
/// **No whitespace-only segment** unless it cannot be avoided within the budget (see
/// [`absorb_blank_segments`]): the embedder refuses a blank input, and one refused segment
/// leaves the whole document without a vector.
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
            split_long_line(line, budget, &mut out);
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
    absorb_blank_segments(&mut out, budget);
    out
}

/// The separator between entries of an issue-cluster `**Members:**` line. A cut placed
/// before it starts the next segment on a whole entry.
const ENTRY_MARKER: &str = "  **+1:";

/// Cut one over-long `line` into pieces of at most `budget` chars, appended to `out`.
///
/// Each cut is chosen inside the next `budget`-char window, in this order:
///
/// 1. **Before the last [`ENTRY_MARKER`]** in the window, so a segment begins at an entry.
/// 2. **After the last whitespace char** in the window, so a word is never split. The
///    whitespace stays on the earlier segment — nothing is dropped.
/// 3. **At `budget` chars**, on a char boundary — for text with no whitespace at all (a
///    minified blob, a long path), where no boundary exists. This is the original behaviour.
///
/// A boundary only qualifies if it lies in the window's second half. Without that floor a
/// lone early space would emit a sliver segment (one wasted embed request), and requiring it
/// keeps every non-final segment at least half full, so the segment count stays within 2x of
/// the minimum. Cuts are made on byte offsets taken
/// from `char_indices`, so no cut can fall inside a UTF-8 character.
///
/// Terminates: every iteration emits a non-empty piece and shortens `rest` by it.
fn split_long_line(line: &str, budget: usize, out: &mut Vec<String>) {
    let mut rest = line;
    loop {
        // Byte offset just past the window's `budget` chars; `None` means `rest` fits.
        let Some((window_end, _)) = rest.char_indices().nth(budget) else {
            out.push(rest.to_string());
            return;
        };
        let min_fill = rest
            .char_indices()
            .nth(budget.div_ceil(2))
            .map_or(window_end, |(i, _)| i)
            .max(1);

        // 1. The last entry marker starting inside the qualifying part of the window. The
        //    haystack runs a marker's length past the window so one that straddles the
        //    window end is still seen (it is cut BEFORE, so only its start must be inside).
        //    The piece it leaves must hold content: a cut that leaves only whitespace
        //    would emit a segment the embedder refuses.
        let mut hay_end = (window_end + ENTRY_MARKER.len()).min(rest.len());
        while !rest.is_char_boundary(hay_end) {
            hay_end -= 1;
        }
        let at_marker = rest[..hay_end]
            .rmatch_indices(ENTRY_MARKER)
            .map(|(i, _)| i)
            .find(|&i| i <= window_end && i >= min_fill && has_content(&rest[..i]));

        // 2. Just after the last whitespace char of the window, again only if the piece
        //    before it holds content (a long whitespace run in the window's second half
        //    would otherwise be cut off as a whitespace-only segment).
        let after_space = || {
            rest[..window_end]
                .char_indices()
                .rev()
                .find(|&(i, c)| {
                    c.is_whitespace() && i + c.len_utf8() >= min_fill && has_content(&rest[..i])
                })
                .map(|(i, c)| i + c.len_utf8())
        };

        // 3. The hard cut.
        let cut = at_marker.or_else(after_space).unwrap_or(window_end);
        out.push(rest[..cut].to_string());
        rest = &rest[cut..];
    }
}

/// Whether `s` holds anything but whitespace — what `RemoteEmbedder::embed` tests
/// (`!t.trim().is_empty()`) before refusing an input.
fn has_content(s: &str) -> bool {
    s.chars().any(|c| !c.is_whitespace())
}

/// Byte offsets inside `s` at which the tail handed to a following blank segment could
/// start, best first: the last whole word, then just the last non-whitespace char. Only
/// offsets that leave `s[..at]` with content qualify, so `s` is never itself made blank.
fn tail_steal_points(s: &str) -> Vec<usize> {
    let Some((last, _)) = s.char_indices().rev().find(|(_, c)| !c.is_whitespace()) else {
        return Vec::new();
    };
    let word_start = s[..last]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    [word_start, last]
        .into_iter()
        .filter(|&at| at > 0 && has_content(&s[..at]))
        .collect()
}

/// The mirror of [`tail_steal_points`]: offsets just past the first whole word (leading
/// whitespace included), then just past the first non-whitespace char, that leave
/// `s[at..]` with content.
fn head_steal_points(s: &str) -> Vec<usize> {
    let Some((first, c)) = s.char_indices().find(|(_, c)| !c.is_whitespace()) else {
        return Vec::new();
    };
    let one_char = first + c.len_utf8();
    let word_end = s[first..]
        .char_indices()
        .find(|(_, c)| c.is_whitespace())
        .map_or(s.len(), |(i, _)| first + i);
    [word_end, one_char]
        .into_iter()
        .filter(|&at| at < s.len() && has_content(&s[at..]))
        .collect()
}

/// Make no segment whitespace-only, so none is refused by the embedder.
///
/// `RemoteEmbedder::embed` bails on an all-whitespace input, and `embed_artifact` /
/// `embed_document_pooled` propagate it: one blank segment leaves the whole document with
/// NO vector. Blank segments arise two ways: a cut in a long line that leaves a run of
/// whitespace on its own, and a group of blank lines flushed ahead of a long line (an
/// untitled artifact is composed as `"\n\n{body}"`).
///
/// Each blank segment is, in order: appended to the previous segment; prepended to the
/// next; or, when neither neighbour has room, given the last word (else last char) of the
/// previous segment, or the first word (else first char) of the next — whichever lets the
/// result stay within `budget`. Every step is a byte-exact move of text between neighbours
/// on char boundaries, so the concatenation is unchanged.
///
/// **Limits, stated rather than hidden.** A blank segment no neighbour can absorb within
/// the budget stays blank: a whitespace run longer than the budget, or a budget so small a
/// lone newline cannot join anything. Input that is blank throughout is left as is, since
/// there is nothing to merge a blank segment into. This pass never exceeds `budget`; a
/// blank segment is the lesser evil only where the alternative is an oversize request.
///
/// Terminates: each step shortens `segs` or turns a blank segment into one with content,
/// otherwise the index advances.
fn absorb_blank_segments(segs: &mut Vec<String>, budget: usize) {
    if !segs.iter().any(|s| has_content(s)) {
        return;
    }
    let chars = |s: &str| s.chars().count();
    let mut i = 0;
    while i < segs.len() {
        if has_content(&segs[i]) {
            i += 1;
            continue;
        }
        let n = chars(&segs[i]);

        if i > 0 && chars(&segs[i - 1]) + n <= budget {
            let blank = segs.remove(i);
            segs[i - 1].push_str(&blank);
            continue;
        }
        if i + 1 < segs.len() && n + chars(&segs[i + 1]) <= budget {
            let blank = segs.remove(i);
            segs[i].insert_str(0, &blank);
            continue;
        }
        if i > 0 {
            let prev = &segs[i - 1];
            let at = tail_steal_points(prev)
                .into_iter()
                .find(|&at| chars(&prev[at..]) + n <= budget);
            if let Some(at) = at {
                let tail = segs[i - 1].split_off(at);
                segs[i].insert_str(0, &tail);
                continue;
            }
        }
        if i + 1 < segs.len() {
            let next = &segs[i + 1];
            let at = head_steal_points(next)
                .into_iter()
                .find(|&at| n + chars(&next[..at]) <= budget);
            if let Some(at) = at {
                let head: String = segs[i + 1].drain(..at).collect();
                segs[i].push_str(&head);
                continue;
            }
        }
        i += 1;
    }
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

#[cfg(test)]
mod segment_tests {
    use super::segment_for_budget;

    /// The entry separator of an `IC-*.md` `**Members:**` line.
    const MARKER: &str = "  **+1:";

    /// Deterministic xorshift — the property tests need reproducible "random" inputs without
    /// a dev-dependency, and a failing seed must replay.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    /// Alphabet mixing ASCII, spaces, a tab, the 2/3/4-byte UTF-8 widths, the entry
    /// marker, and (rarely) a newline — every shape the boundary search has to survive.
    fn random_text(rng: &mut Rng, len: usize) -> String {
        const PIECES: &[&str] = &[
            "a", "b", "word", "x", " ", " ", " ", "\t", "é", "日", "本", "🦀", "  **+1:", "\n",
        ];
        (0..len).map(|_| PIECES[rng.below(PIECES.len())]).collect()
    }

    /// PROPERTY: for any text and any budget, segments (1) concatenate back to the input
    /// BYTE FOR BYTE, (2) each hold at most `budget` chars, (3) none is empty, and the call
    /// returns at all (termination). Slicing inside a UTF-8 character would panic, so
    /// reaching the asserts also proves no segment boundary fell inside one.
    #[test]
    fn any_text_at_any_budget_is_lossless_bounded_and_terminates() {
        for seed in 1..=300u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let len = 1 + rng.below(400);
            let text = random_text(&mut rng, len);
            for budget in [1usize, 2, 3, 5, 8, 13, 40, 97, 1000] {
                let segs = segment_for_budget(&text, budget);
                assert_eq!(
                    segs.concat(),
                    text,
                    "seed={seed} budget={budget}: not lossless"
                );
                for s in &segs {
                    assert!(!s.is_empty(), "seed={seed} budget={budget}: empty segment");
                    assert!(
                        s.chars().count() <= budget,
                        "seed={seed} budget={budget}: segment of {} chars over budget",
                        s.chars().count()
                    );
                }
            }
        }
    }

    /// A line with NO whitespace at all has no boundary to prefer: it must be hard-cut at
    /// exactly `budget` chars, on char boundaries, for every UTF-8 width.
    #[test]
    fn a_line_with_no_whitespace_is_hard_cut_at_exactly_the_budget() {
        for unit in ["x", "é", "日", "🦀"] {
            let line = unit.repeat(23);
            for budget in 1..=9usize {
                let segs = segment_for_budget(&line, budget);
                assert_eq!(segs.concat(), line, "unit={unit} budget={budget}");
                let (last, body) = segs.split_last().unwrap();
                for s in body {
                    assert_eq!(s.chars().count(), budget, "unit={unit} budget={budget}");
                }
                assert!(last.chars().count() <= budget);
                assert_eq!(
                    segs.len(),
                    23usize.div_ceil(budget),
                    "unit={unit} budget={budget}"
                );
            }
        }
    }

    /// The defect this fixes: a long prose line used to be cut MID-WORD every `budget`
    /// chars. Every non-final segment must now end on whitespace (kept on the earlier
    /// segment, which is what makes the split lossless).
    #[test]
    fn a_long_prose_line_is_split_between_words_not_inside_them() {
        let line = "alpha beta gamma delta epsilon zeta eta theta iota kappa ".repeat(40);
        let segs = segment_for_budget(&line, 100);
        assert!(segs.len() > 1, "fixture must exceed the budget");
        assert_eq!(segs.concat(), line);
        for s in &segs[..segs.len() - 1] {
            assert!(
                s.ends_with(char::is_whitespace),
                "a non-final segment ends inside a word: {s:?}"
            );
        }
    }

    /// The `**Members:**` shape: entries joined by two spaces + `**+1:`. A window that
    /// contains an entry boundary must be cut BEFORE it, so each entry starts a segment
    /// rather than a segment starting mid-sentence.
    #[test]
    fn a_members_style_line_is_cut_at_entry_boundaries() {
        let entry = format!(
            "{MARKER} `slug-name` (2026-10-14)**: needs a thing but proxies it with another thing."
        );
        let line = format!("**Members:**{}", entry.repeat(30));
        let budget = entry.chars().count() * 2 + 17; // two entries and a bit
        let segs = segment_for_budget(&line, budget);
        assert!(segs.len() > 1);
        assert_eq!(segs.concat(), line);
        for s in &segs[1..] {
            assert!(
                s.starts_with(MARKER),
                "every segment after the first must begin at an entry boundary: {s:?}"
            );
        }
    }

    /// A boundary right at the front of the window would leave a sliver segment. Cuts are
    /// confined to the window's second half; a lone early space does not qualify, and the
    /// segment is cut at the budget instead.
    #[test]
    fn an_early_boundary_does_not_produce_a_sliver() {
        let line = format!("ab {}", "z".repeat(200));
        let segs = segment_for_budget(&line, 100);
        assert_eq!(segs.concat(), line);
        assert_eq!(
            segs[0].chars().count(),
            100,
            "first segment was cut at the early space"
        );
    }

    /// Lines at or under budget, and the newline-joined path, are untouched: the new
    /// boundary logic applies only to a single line longer than the budget.
    #[test]
    fn lines_within_budget_keep_their_existing_segmentation() {
        let text = "one two three\nfour five six\nseven\n";
        assert_eq!(segment_for_budget(text, 1000), vec![text.to_string()]);
        assert_eq!(
            segment_for_budget(text, 14),
            vec![
                "one two three\n".to_string(),
                "four five six\n".to_string(),
                "seven\n".to_string()
            ]
        );
    }

    fn is_blank(s: &str) -> bool {
        s.chars().all(char::is_whitespace)
    }

    /// The reviewer's confirmed repro: a whitespace run between two over-budget tokens used
    /// to come out as its own all-whitespace segment, which `RemoteEmbedder::embed` refuses —
    /// and `embed_artifact` propagates that, so the whole document got NO vector.
    #[test]
    fn a_blank_run_between_two_long_tokens_is_not_its_own_segment() {
        let text = format!("{}{}{}", "x".repeat(100), " ".repeat(60), "y".repeat(100));
        let segs = segment_for_budget(&text, 100);
        assert_eq!(segs.concat(), text);
        assert!(
            segs.iter().all(|s| !is_blank(s)),
            "whitespace-only segment emitted: {:?}",
            segs.iter().map(|s| s.chars().count()).collect::<Vec<_>>()
        );
        assert!(segs.iter().all(|s| s.chars().count() <= 100));
    }

    /// A line ending in a blank run longer than the room left after a hard cut left the run
    /// as a lone final segment.
    #[test]
    fn a_trailing_blank_run_is_absorbed() {
        let text = format!("{}{}", "x".repeat(100), " ".repeat(5));
        let segs = segment_for_budget(&text, 100);
        assert_eq!(segs.concat(), text);
        assert!(segs.iter().all(|s| !is_blank(s)), "{segs:?}");
        assert!(segs.iter().all(|s| s.chars().count() <= 100));
    }

    /// `embed_artifact` composes `"{title}\n\n{body}"`, so an untitled artifact whose first
    /// line is over budget starts with a blank `"\n\n"` group that the long line then flushed
    /// as a segment of its own — the same defect, reachable from the old code too.
    #[test]
    fn a_leading_blank_group_before_a_long_line_is_absorbed() {
        let text = format!("\n\n{}", "x".repeat(250));
        let segs = segment_for_budget(&text, 100);
        assert_eq!(segs.concat(), text);
        assert!(segs.iter().all(|s| !is_blank(s)), "{segs:?}");
        assert!(segs.iter().all(|s| s.chars().count() <= 100));
    }

    /// Input that is blank all the way through has nothing to merge a blank segment INTO;
    /// it is returned as it always was, rather than concatenated into one oversize piece.
    #[test]
    fn input_that_is_blank_throughout_is_left_alone() {
        let text = " ".repeat(250);
        let segs = segment_for_budget(&text, 100);
        assert_eq!(segs.concat(), text);
        assert!(segs.iter().all(|s| s.chars().count() <= 100));
    }

    /// PROPERTY: whenever the input holds non-whitespace and its longest whitespace run is at
    /// most an eighth of the budget, NO segment is whitespace-only, and every segment stays
    /// within the budget. (A run longer than the budget cannot be absorbed within it, and at
    /// budget 1 a lone newline cannot join anything; those are the documented limits, which
    /// is why the property is gated on the run length.)
    #[test]
    fn no_segment_is_whitespace_only_when_it_can_be_avoided() {
        for seed in 1..=1500u64 {
            let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            let len = 1 + rng.below(400);
            let text = random_text(&mut rng, len);
            if is_blank(&text) {
                continue;
            }
            let mut max_run = 0usize;
            let mut run = 0usize;
            for c in text.chars() {
                run = if c.is_whitespace() { run + 1 } else { 0 };
                max_run = max_run.max(run);
            }
            for budget in [8usize, 13, 40, 97, 300] {
                if max_run * 8 > budget {
                    continue;
                }
                let segs = segment_for_budget(&text, budget);
                assert_eq!(segs.concat(), text, "seed={seed} budget={budget}");
                for s in &segs {
                    assert!(
                        !is_blank(s),
                        "seed={seed} budget={budget}: whitespace-only segment {s:?}"
                    );
                    assert!(
                        s.chars().count() <= budget,
                        "seed={seed} budget={budget}: segment over budget"
                    );
                }
            }
        }
    }

    /// The failure being prevented, with the REAL refusal as the oracle: `RemoteEmbedder::embed`
    /// bails on a whitespace-only input before any network call. Each segment of the repro is
    /// sent to a real embedder aimed at a dead port with a single attempt; the only outcome
    /// that matters is that none is refused as empty/whitespace (a connect error is expected
    /// and fine).
    #[cfg(feature = "remote-embed")]
    #[tokio::test]
    async fn no_segment_of_the_repro_meets_the_real_embedders_blank_refusal() {
        use codescout_embed::Embedder;
        let emb =
            codescout_embed::remote::RemoteEmbedder::from_url("http://127.0.0.1:1", "m", None)
                .unwrap()
                .with_max_attempts(1);
        // The oracle itself must fire on a blank input, or the loop below proves nothing.
        let refusal = emb.embed(&["   "]).await.unwrap_err().to_string();
        assert!(refusal.contains("empty/whitespace"), "{refusal}");

        let text = format!(
            "\n\n{}{}{}",
            "x".repeat(100),
            " ".repeat(60),
            "y".repeat(100)
        );
        for seg in segment_for_budget(&text, 100) {
            if let Err(e) = emb.embed(&[seg.as_str()]).await {
                assert!(
                    !e.to_string().contains("empty/whitespace"),
                    "segment of {} chars was refused as blank",
                    seg.chars().count()
                );
            }
        }
    }
}
