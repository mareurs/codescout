# File-Diversity Re-Rank for `semantic_search`
> ⚠ **Not currently active.** The cap is implemented and unit-tested as
> `apply_file_diversity_cap` in `src/tools/semantic/semantic_search.rs`, but that
> function carries `#[allow(dead_code)]` and is not wired into the live search
> path — the retrieval stack returns chunks ranked by Qdrant plus the reranker,
> with no per-file cap. Re-wiring is tracked as L-15 in
> `docs/trackers/2026-05-07-legacy-retrieval-removal.md`. Read what follows as a
> design record and a benchmark rationale, not as behaviour you can observe
> today.
`semantic_search` applies a per-file cap to its results before returning them,
so a single highly-relevant file cannot saturate the top-K and crowd out
sibling files that the query is also about.

## Why

Embedding retrieval is biased toward chunk-level relevance, not file-level
coverage. When one file contains many similar chunks (a long `impl` block with
ten methods, a spec with ten matching sections), the nearest-neighbour search
happily fills every slot from that file. The caller — often an agent looking
for "which files are relevant to X" — then loses visibility of related files
that would have ranked just below.

The [metadata-enriched chunks benchmark
(2026-04-20)](../../../../docs/research/2026-04-03-embedding-model-benchmark.md)
observed this explicitly: three test cases (TC-03, TC-13, TC-19) regressed
when container decomposition multiplied per-file chunks, because one file's
methods filled all ten slots.

## How it works

1. `semantic_search` requests `limit * MAX_CHUNKS_PER_FILE` candidates from
   the vector index (default: `limit * 3`).
2. Results come back sorted by cosine similarity, high to low.
3. A post-filter iterates in score order and drops any chunk whose
   `file_path` already appears `MAX_CHUNKS_PER_FILE` times in the kept set.
4. The filtered list is truncated to the user's requested `limit`.

Score ordering is preserved — the cap is a filter, not a re-rank. A file
that legitimately owns multiple top-K-worthy chunks still gets up to
`MAX_CHUNKS_PER_FILE` of them.

## Default

There is no `MAX_CHUNKS_PER_FILE` constant in the tree — the name appears only in
this page and in the design plan that proposed it. `apply_file_diversity_cap`
takes the cap as a parameter, and its tests exercise `3` (the intended default),
`1`, and `0`, where `0` disables the cap entirely.

`3` was chosen to preserve multi-hit files (TC-18 keeps both `markdown.rs` hits)
while preventing single-file saturation (TC-13's `manager.rs` is limited to 3 of
10 slots). Whoever re-wires the cap decides where that constant comes to live.
## When it helps

- Multi-concept queries that should surface more than one file.
- Codebases with long impl blocks, long spec documents, or generated code
  where near-duplicate chunks cluster in a single file.

## When it may hurt

- Queries where the correct answer really is "this one file". The cap still
  surfaces that file's top 3 chunks, but slots 4–10 go to less-relevant
  neighbours instead of more chunks from the winning file. Use
  `detail_level: "full"` with a narrowed query if the agent needs deeper
  context from one file.

## Related

- `docs/research/2026-04-03-embedding-model-benchmark.md` — benchmark rubric
  this feature was tuned against.
- `crates/codescout-embed/` — upstream retrieval pipeline (no changes;
  cap lives entirely in `src/tools/semantic/semantic_search.rs`).
