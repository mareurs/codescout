---
id: '8c86203711a9520d'
kind: bug
status: fixed
title: 'RESIDUAL: Implement a wrapping form for the **Members:** line (26 KB single line), and reindex to re-embed the 7 vectorless artifacts'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-10-16
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md
severity: low
unverified: 'The already-stored over-long **Members:** lines (IC-18 81 KB, IC-14 73 KB, IC-6 60 KB, IC-2 53 KB, ...) are still embedded under the OLD mid-word cut: the new boundary-aware segmentation only applies to text embedded from now on. Running `librarian(action="reindex", reembed=true)` has NOT been done; until it is, existing vectors are unchanged. Discharge with CLEARED once it has run and `index(action="verify")` reports 0 vectorless.'
---

## Summary

Remaining work split out of `docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Implement a wrapping form for the **Members:** line (26 KB single line), and reindex to re-embed the 7 vectorless artifacts.


**Half discharged 2026-09-24:** the re-embed is done — `reindex(reembed=true)` on the rebuilt
binary reported 34346 embedded and 0 vectorless. What remains is the wrapping form for the
`**Members:**` line.

## Parent caveat, verbatim

`docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md` (status `fixed`):

> The 7 artifacts are still vectorless on disk: the code no longer produces the failure, but repairing the existing rows needs `cargo rb` plus a reindex, which has not been run. And the second half this file asks for -- a wrapping form for the `**Members:**` line -- is NOT done; it is now a vector-quality concern (a 26 KB line pools to one blurry vector) rather than the data-loss one it was.

## Fix

Decided 2026-10-16 (option A, chosen by the user): fix it where the vector is made, not in
the ledger. No ledger-format change and no parser change.

**Why not wrap the line in the ledger.** The premise changed since the parent was filed.
`scripts/pre-commit-ledger-counts.py:940` now says the `**Members:**` lines are frozen and
nothing reads them for the gate, so they no longer grow. They are still huge (IC-18 81 KB,
IC-14 73 KB, IC-6 60 KB, IC-2 53 KB). But `parse_bare_n_claims` (that script and
`tests/issue_clusters.rs`) reads only lines that START with `**Members:**`: hand-wrapping a
line would hide any `n=` on a continuation line from the bare-n gate, unless both parsers
were also changed, plus about 20 tracked docs rewritten.

**What changed.** `segment_for_budget` (`src/embed/document.rs`) used to hard-split a line
longer than the budget every `budget` chars, wherever that fell, so a vector was pooled
from half-sentences. The split now lives in `split_long_line`. Each cut is chosen inside the
next `budget`-char window, in this order:

1. before the last `  **+1:` entry marker, so a segment starts on an entry;
2. else just after the last whitespace char, so a word is never split;
3. else at `budget` chars on a char boundary, for text with no whitespace at all.

A boundary qualifies only in the window's second half, so no sliver segments and the
segment count stays within 2x of the minimum.

**Lossless, with no whitespace dropped.** The whitespace stays on the earlier segment, so
the concatenation of the segments is the input byte for byte (the existing
`segmenting_never_drops_or_duplicates_content` asserts exactly that and still passes
unchanged). Cuts are byte offsets from `char_indices`, so no cut falls inside a UTF-8
character. Termination: every iteration emits a non-empty piece and shortens the rest.
Lines at or under the budget and the newline-joined path are untouched.

- **SHA:** `a5c3adcc` (branch `worktree-agent-aa5d85ef986eb51aa`, on top of `experiments` `3923da8c`)
- **patch-id:** `40a6d107bfdc50c2b93a032f2e2b9cc1ccba3786`

### Review follow-up 2026-10-16: no segment may be whitespace-only

An Opus review of the integrated branch confirmed on a verbatim copy that the first cut
could emit an all-whitespace segment: `"x"*100 + " "*60 + "y"*100` at budget 100 gave
`[x100][60 spaces][y100]`, where the old code gave none. `RemoteEmbedder::embed` refuses an
all-whitespace input (`crates/codescout-embed/src/remote.rs`, `embed`), and `embed_artifact`
and `embed_document_pooled` propagate the error, so such a document would have got NO
vector. Reproduced at the bytes before the fix, including end to end: with a double that
returns the real refusal message, `embed_artifact` failed. A second shape was found while
tracing it: `embed_artifact` composes `"{title}\n\n{body}"`, so an untitled artifact whose
first line is over budget starts with a blank `"\n\n"` group that the long line flushed as
a segment of its own. That one also fails under the pre-change code.

Fixed in two layers (`src/embed/document.rs`):

1. Cut candidates (marker and whitespace) must leave a piece that holds non-whitespace.
2. `absorb_blank_segments` runs last. Each blank segment joins the previous segment, else
   the next, else takes the last word (or char) of the previous segment or the first word
   (or char) of the next, whichever stays within the budget. All moves are byte-exact between
   neighbours on char boundaries, so the concatenation is unchanged.

**Documented limits.** A blank segment no neighbour can absorb within the budget stays blank:
a whitespace run longer than the budget, or a budget so small that a lone newline cannot join
anything (budget 1). Input that is blank throughout is left as it was. The pass never exceeds
the budget.

Tests, red first (each failed on the code as reviewed): four shape tests (`a_blank_run_between_two_long_tokens_is_not_its_own_segment`,
`a_trailing_blank_run_is_absorbed`, `a_leading_blank_group_before_a_long_line_is_absorbed`,
`input_that_is_blank_throughout_is_left_alone`); the property
`no_segment_is_whitespace_only_when_it_can_be_avoided` (1500 seeded texts x 5 budgets, gated
on the longest whitespace run being at most an eighth of the budget, asserting no blank
segment and none over budget); `no_segment_of_the_repro_meets_the_real_embedders_blank_refusal`
(feeds each segment to a real `RemoteEmbedder` and fails if any is refused as blank, after
checking the oracle does fire on a blank input); and
`librarian::embedding::tests::a_blank_run_inside_a_long_artifact_does_not_cost_it_its_vector`.
Mutations: disabling the absorb pass turned the leading, trailing, property and artifact tests
red; dropping the content check on whitespace cuts turned the repro red.

**A reindex with `reembed=true` is still needed and has NOT been run** (see the next section).

### A reindex is needed afterwards. NOT run.

Changing segmentation changes the vectors, but only for text embedded from now on. Every
artifact already stored keeps the vector it got under the old cut. To re-embed the stored
over-long `**Members:**` lines (and everything else) under the new cut, run on the
rebuilt binary (`./scripts/rb.sh`, then `/mcp`):

```
librarian(action="reindex", reembed=true)
```

`reembed=true` re-queues every file even when its content hash is unchanged; without it the
unchanged `IC-*.md` files are skipped and keep their old vectors. Afterwards
`index(action="verify")` should still report 0 vectorless. Memories are affected only if one
holds a single line longer than the model budget (about 5222 chars for CodeRankEmbed); none
is known to, and re-writing one goes through the same path.

## Tests added

`src/embed/document.rs`, module `segment_tests` (6), red-first. Against the old code the
two behaviour tests failed (a segment ending mid-word at "...eta the", a segment starting
mid-marker at "1: `slug-name`") while the invariants passed, so those act as guards:

- `any_text_at_any_budget_is_lossless_bounded_and_terminates`: property test, 300 seeded
  random texts (ASCII, tab, 2/3/4-byte UTF-8, the marker, newlines) x 9 budgets from 1 to
  1000; asserts byte-exact concatenation, at most `budget` chars, no empty segment, returns.
- `a_line_with_no_whitespace_is_hard_cut_at_exactly_the_budget`: every UTF-8 width x budgets
  1..=9; exact piece size and count.
- `a_long_prose_line_is_split_between_words_not_inside_them`.
- `a_members_style_line_is_cut_at_entry_boundaries`.
- `an_early_boundary_does_not_produce_a_sliver`.
- `lines_within_budget_keep_their_existing_segmentation`.

Mutations: dropping the half-window floor turns `an_early_boundary_does_not_produce_a_sliver`
red; dropping the marker arm turns `a_members_style_line_is_cut_at_entry_boundaries` red.
The existing `tools::memory::tests::{segmenting_never_drops_or_duplicates_content,
content_within_budget_is_never_segmented}` and the six `librarian::embedding` tests stay
green. Clippy (`--workspace --all-targets --features local-embed -D warnings`) is clean. The
full gate was not run.

## References

- `docs/issues/archive/2026-09-04-the-chunker-budget-is-not-a-bound-a-single-line-cannot-be-split.md` — parent
