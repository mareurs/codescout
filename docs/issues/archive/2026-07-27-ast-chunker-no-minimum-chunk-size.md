---
kind: bug
status: fixed
title: 'BUG: AST chunker has no minimum chunk size — 12% of chunks are single lines (the "~2h re-embed" in the original title measured 8.06 min on 2026-08-07; deferred by decision)'
tags:
- indexer
- chunker
- retrieval
- performance
- embedding-cost
closed: 2026-08-08
opened: 2026-07-27
owner: marius
related:
- docs/issues/2026-07-27-indexer-walks-git-and-tool-state-dirs.md
severity: high
---

# BUG: AST chunker has no minimum chunk size — 12% of chunks are single lines, inflating re-embed to ~2h

## Summary

> **FIXED — and this file spent a day saying otherwise.** Candidate 1 (`AST_CHUNK_MIN`
> plus sibling coalescing) shipped in **`ca442498`** on 2026-08-06. Candidate 3 (the
> container-recursion overlap) shipped the same day in **`45669701`**. The "DEFERRED,
> neither candidate was chosen" text below was written on 2026-08-07 — *a day after the
> fix* — by a session that corrected this file's numbers carefully and never opened
> `src/embed/ast_chunker.rs`. Every correction it made was to the measurements, and the
> measurements were all still true; the thing that had changed was the code.
>
> **Measured 2026-08-08, after a full `index --force` on the fixed chunker:**
>
> | | pre-fix | post-fix (codescout) |
> |---|---|---|
> | single-line chunks | 11.8% | **6.3%** (2,113 / 33,735) |
> | ≤5-line chunks | 34.3% | **29.0%** |
> | empty sparse vectors | 7.7% | **0 of 2,000 sampled** |
>
> The empty-sparse figure is the one that matters, because it was this file's last live
> argument: single-line chunks yield no SPLADE terms, so they were dense-only in RRF and
> invisible to the sparse leg. A 2,000-point sample of the rebuilt corpus finds **zero**
> empty sparse vectors (min 2 terms, max 285, mean 125). That argument is now falsified,
> not deferred.
>
> The residual 6.3% is **by design**, not leftover defect. The coalescer merges *runs of
> contiguous undersized declarations*; lone small declarations, gap chunks, and the
> whole-file fallback path are deliberately excluded — see `ca442498`'s rationale for why
> admitting gap chunks collapsed a single-method impl into a whole-file chunk.
>
> **Candidates 2 (skip trivial declarations) and 4 (reconsider markdown granularity) were
> never shipped and are now unmotivated** — the retrieval-quality argument that justified
> them is the one the measurement just closed. They remain available if a future
> measurement revives them; nothing currently points at them.
>
> **Not reindexed elsewhere.** `backend-kotlin-single-stage` still sits at 11.7%
> single-line / 33.6% ≤5-line — statistically this file's original numbers, preserved.
> The code fix reaches a corpus only through `index --force`, because chunk ids are
> content-addressed and a normal sync skips every unchanged chunk by id.

> **Title corrected 2026-08-07.** It claimed single-line chunks inflate re-embedding to
> ~2 h. A full `index --force` that day took **483,729 ms = 8.06 min** — off by roughly
> 15x. The likely source of the error is recorded in `src/retrieval/embedder.rs`, whose
> own comment says the throughput sweep those numbers came from was *"taken while four
> duplicate indexers were competing for the same GPU"* — i.e. contaminated by
> `docs/issues/archive/2026-07-25-concurrent-index-no-project-lock.md`, the very bug whose
> precondition this file's Resume told you to check first.

`nodes_to_chunks` emitted one chunk per inner declaration "regardless of size", with no
floor. On a Kotlin codebase that produced 5,133 single-line chunks and 14,926 chunks of
≤5 lines out of 43,582 — despite a configured `chunk_target` of 1200 characters. That is
the defect; `AST_CHUNK_MIN = 250` plus `coalesce_small_chunks` is the fix.
## Symptom (Effect)

`project_id=backend-kotlin`, measured live 2026-07-27:

```
total chunks    : 43582
distinct files  : 1812
chunks per file : 24.1

chunk line-span distribution
  p10    1   p50    9   p90   28   p99   40   max 69
  chunks with span <= 5 lines : 14926   (34%)
  chunks with span == 1 line  :  5133   (12%)

by extension
 24648  .kt      17067  .md       895  .rs       850  .py
```

The median chunk is **9 lines**. With `chunk_target = 1200` chars (≈30 lines of
Kotlin) the median should be ~3× larger.

Worst single file:

```
ktor-server/src/main/kotlin/edu/planner/validation/PreSolveDataValidation.kt
  1872 source lines → 724 chunks
```

Duration arithmetic, which matches the observed runtime almost exactly:

```
43,582 chunks ÷ ~350 chunks/min (measured dense throughput) ≈ 125 min
observed: 2h39m and still running
```

Secondary effect — line coverage is not 1:1, contradicting the design comment on
`split_file` ("`chunk_overlap` has been removed: AST chunks have clean semantic
boundaries, so overlap is meaningless"):

```
files indexed                   : 1832
files with >1.05x line coverage :  939  (51.3%)
mean coverage factor            : 1.27x
worst: 8.01x Stage1Assertions.kt · 5.53x PreSolveDataValidation.kt
       5.41x SolverExplainabilityTables.kt · 5.20x StudentGroupRepository.kt
```

## Reproduction

Index a Kotlin project with many small declarations (enums with doc-commented
members, data classes, table definitions), then aggregate `end_line - start_line`
over the collection. Scripts used are in this session's scratchpad
(`audit_chunks.py`, `probe_file.py`, `probe_generations.py`).

## Environment

- codescout binary built 2026-07-25 19:27
- `CODESCOUT_CHUNK_TARGET` unset → `STACK_CHUNK_TARGET = 1200` (`src/retrieval/sync.rs:205`)
- `AST_CHUNK_TARGET = 3000` (`src/embed/ast_chunker.rs:828`), applied as
  `chunk_size.min(AST_CHUNK_TARGET)` → effective target 1200

## Root cause

Two independent contributors.

**1. No minimum chunk size (the dominant cost).** `nodes_to_chunks`
(`src/embed/ast_chunker.rs:537-546` doc, `:610-660` code) decomposes any container
with extractable inner nodes "**regardless of size** — producing one chunk per
inner declaration plus a header chunk for the container signature". `chunk_size`
is consulted only as a *ceiling* (`content.len() <= chunk_size`) and in
`enforce_max_chunk_size`. Nothing merges adjacent small declarations. A Kotlin
enum member like

```kotlin
    /** Gantt session_duration > subject max_session_duration … (H9). */
    ALLOCATION_SESSION_GT_MAX,
```

becomes its own embedding. A single-line chunk carries almost no retrievable
signal but costs a full embed round-trip and a stored vector.

This is *by design* as written — the design just has no floor to match its
ceiling.

**2. Line-coverage overlap.** Mean 1.27×, up to 8×. Container-header chunks are
emitted at the same `expanded_start` as the first inner node
(`extract_container_header(&lines, expanded_start, node_end, …)`, `:632`), and
`expand_doc_comment_start` pulls each inner node's start back over its preceding
doc comment. Sampled keys show the nesting signature — same start, growing ends:

```
L1601 ends=[1603, 1613, 1640]  scheduling.rs
L1    ends=[7, 16, 44]         ExpoPushToken.kt
```

**Confidence:** (1) is measured and traced to the documented policy. (2) is
measured; the attribution to header emission + doc-comment expansion is inferred
from the code path and the same-start/growing-end signature, not proven by
instrumenting the chunker.

A third, smaller contributor: only 4.0% of `(file, start_line)` keys carry 2+
distinct `end_line`s, so stale-generation accumulation from interrupted runs is
present but minor. Note that superseded chunks are deleted only *after* a
complete walk (`src/retrieval/sync.rs:179-190`), so an interrupted index leaves
its predecessor's chunks behind permanently.

**Why this run re-embedded everything:** `chunk_id` encodes the content hash, and
unchanged ids are skipped (`sync.rs:148`). `index-state.json` records the last
successful index at 2026-07-23; the binary was rebuilt 2026-07-25. Any chunker
behaviour change between those dates shifts every boundary, changes every id, and
forces a full re-embed of the corpus.

## Evidence

### 2026-08-08 — post-fix measurement, after a full `index --force`

Method, so this is re-runnable rather than quoted. Chunk-size distribution: scroll the
whole `code_chunks` collection for `start_line`/`end_line`/`project_id`/`language` and
histogram `end_line - start_line + 1`. 579,311 points, 21 project ids.

```
project                     chunks   1-line       %    <=5line       %
codescout                    33735     2113     6.3%      9798    29.0%
backend-kotlin-single-stage  48560     5672    11.7%     16306    33.6%
```

`codescout` was rebuilt on the fixed chunker (`index --force`, 506 s, 33,764 chunks,
`last_indexed_commit: 2bc0f9f0`). `backend-kotlin-single-stage` has not been reindexed
since, and still carries this file's original 11.8% / 34.3% figures — which is the
cleanest available demonstration that the code fix reaches a corpus only through
`--force`.

Empty-sparse re-measurement, mirroring the 2026-08-07 method (2,000-point sample of the
live collection, this time scoped to `codescout` and asking for the sparse vector):

```
sampled: 2000   empty-sparse: 0   = 0%
sparse terms per chunk: min=2 max=285 mean=125
```

Zero, against 7.7% before. The path was verified on a sample point rather than trusted —
a wrong jq path here would have reported *every* point as empty, not none, so the failure
mode is discriminable from the result.


`split_file`, `src/embed/ast_chunker.rs:844-886` — target used only as a ceiling:

```rust
// Cap all paths at AST_CHUNK_TARGET — smaller chunks produce sharper
// embeddings for retrieval regardless of file type.
let target = chunk_size.min(AST_CHUNK_TARGET);
```

`nodes_to_chunks` doc, `:537-541`:

```
/// When a node can be decomposed into inner declarations (methods, constructors,
/// etc.) via `inner_node_types`, it is always recursed into — regardless of size —
/// producing one chunk per inner declaration plus a header chunk for the container
/// signature.
```

## Hypotheses tried

1. **Hypothesis:** the index is slow because it walks gitignored build output.
   **Test:** counted chunks matching `build/`, `.gradle/`, `node_modules/`,
   `target/`, `dist/`, `out/`, `.idea/`, `generated/`, `bin/`, `vendor/`,
   `coverage/`, `.cache/`.
   **Verdict:** rejected — all zero. (A separate, much smaller dot-directory leak
   exists: `docs/issues/2026-07-27-indexer-walks-git-and-tool-state-dirs.md`.)

2. **Hypothesis:** the overlap is accumulated stale chunk generations from
   interrupted runs, not a chunker property.
   **Test:** counted `(file, start_line)` keys carrying multiple distinct
   `end_line`s.
   **Verdict:** partially rejected. Only 4.0% of keys, max 5 variants — real but
   minor. The dominant overlap has the same-start/growing-end nesting signature.

3. **Hypothesis:** an early single-file probe showing 5.53× coverage was
   representative.
   **Verdict:** rejected — the corpus mean is 1.27×. 5.53× is a tail case. Do not
   quote the single-file number as typical.

> **2026-08-06 — candidate 1 is IMPLEMENTED on `experiments`, but this file stays
> `open`: the benchmark validation it requires has NOT been run.**
>
> `ca442498` (`experiments`) adds `AST_CHUNK_MIN = 250` and `coalesce_small_chunks`,
> which is candidate 1 as written here — a floor with sibling merge, container header
> preserved. Code gate is green (`cargo fmt --check`, `cargo clippy -- -D warnings`,
> `cargo test` across all three CI feature configs) with seven tests, and both
> behaviours are mutation-verified: dropping the gap exclusion kills 4 tests,
> `AST_CHUNK_MIN = 0` kills the end-to-end coalesce test.
>
> **What is still owed, and why the status is not `fixed`:** this file's own Fix
> section requires validation against
> `docs/research/2026-05-06-retrieval-stack-benchmark.md` before landing, on the
> grounds that small chunks were chosen deliberately for precision and a floor
> trades recall sharpness for cost — "must be measured, not assumed". That
> measurement has not happened. The code proves the floor works as specified; it
> does not prove the specification was a good trade.
>
> It also jumped the ordering decision recorded in *Resume* — the throughput work
> was to land first because it leaves vectors byte-identical, whereas this change
> invalidates the corpus. That sequencing was not honoured.
>
> Candidate 3 (container-header / leading-gap overlap) turned out to be a distinct,
> pre-existing defect and is now tracked on its own:
> `docs/issues/archive/2026-08-06-ast-chunker-recursion-duplicates-leading-gap.md`. The
> coalescing change closed only the half where merging *swallowed* that gap and lost
> the declaration's metadata header; the duplication itself is untouched.
>
> **To close this file:** run the retrieval benchmark against a corpus rebuilt at
> `ca442498` and compare scores to the pinned baseline. If recall holds, flip to
> `fixed`, label the SHA `experiments`, and note the master-side SHA is still owed
> after cherry-pick. If recall drops, `AST_CHUNK_MIN` is the tuning knob — or
> candidate 2 (skip trivial declarations) is the narrower alternative.
## Fix

**Candidate 1, implemented in `ca442498` (`experiments`, 2026-08-06).** Promotion is by
fast-forward, so that SHA *is* the master SHA.

`AST_CHUNK_MIN = 250` (`src/embed/ast_chunker.rs:546`) is the floor to `AST_CHUNK_TARGET`'s
ceiling; `coalesce_small_chunks` (`:572`, wired in at `:778`) merges runs of adjacent,
contiguous, undersized declarations into one chunk. Only *contiguous* chunks merge, which
is what keeps reconstruction exact. Gap chunks are excluded from runs — admitting either
bracketing gap collapsed a single-method impl into a whole-file chunk and dropped the
method's own name from its metadata. Seven tests (`:2267`–`:2392`), both behaviours
mutation-verified.

**Candidate 3, implemented in `45669701`** (same day): the container recursion was handed
the whole file with `prev_end` reset to 0, duplicating every line before the container
and — contrary to this file's own note — every line after it, to EOF. Both gap branches
are now bounded by an explicit line window.

**Candidates 2 and 4 not implemented, and no longer motivated.** See § Summary: the
retrieval-quality argument they rested on was closed by measurement on 2026-08-08.

The original candidate list is preserved below for the record, because the reasoning
about *why* smaller chunks were chosen deliberately is still the right context for anyone
reopening this area.

1. **Minimum chunk size with sibling merge.** ✅ shipped — `ca442498`.
2. **Skip trivial declarations.** Not shipped; unmotivated as of 2026-08-08.
3. **Reconcile the overlap with the documented intent.** ✅ shipped — `45669701`.
4. **Reconsider markdown.** Not shipped. Markdown is still 60% of codescout's corpus
   (20,124 of 33,764 chunks). Worth noting it is *not* a chunk-size problem: markdown
   chunks carry no AST header at all (`CodeChunk.metadata` is `None` for non-AST
   languages), so this is a different question from the one this file asks.
## Tests added

None yet. Two worth adding: a chunk-size distribution assertion over a fixture
tree (no chunk below the floor except an indivisible tail), and a line-coverage
assertion (sum of spans ÷ distinct lines covered ≤ some bound) that would have
caught the 8× file.

## Workarounds

- Raise `CODESCOUT_CHUNK_TARGET` — but note it is only a ceiling, so it will not
  remove single-line chunks. Limited benefit.
- Exclude `docs/` via `ignore_patterns` to cut 39% of the corpus, at the cost of
  documentation search.
- Let the current run finish. Once ids stabilise, subsequent indexes skip
  unchanged chunks (`sync.rs:148`) and are fast; the 2-hour cost is paid per
  chunker change, not per index.

## Resume

N/A — fixed 2026-08-08, verified by measurement rather than by reading the fix.

**If you reopen this area, reindex first.** Only codescout's corpus reflects the fixed
chunker. Other projects carry pre-fix distributions until someone runs `index --force`
on them — chunk ids are content-addressed, so a normal sync skips every unchanged chunk
by id and will never update it. `backend-kotlin-single-stage` is the clearest example, at
11.7% single-line today.

**The lesson this file is the exhibit for:** it was corrected twice, carefully, by
sessions that never opened `src/embed/ast_chunker.rs`. Both corrections were to numbers,
both numbers were right, and the code had moved underneath. Before working any bug file
older than the last commit touching its subject, run `git log -- <the file the bug names>`.
One command, and it would have caught this on 2026-08-07.
## References

- `src/embed/ast_chunker.rs:828` — `AST_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:537-546, 610-660` — inner-node decomposition policy
- `src/embed/ast_chunker.rs:844-886` — `split_file`, target-as-ceiling
- `src/retrieval/sync.rs:113-119` — walker
- `src/retrieval/sync.rs:148` — unchanged-id skip
- `src/retrieval/sync.rs:179-190` — delete-after-full-walk
- `src/retrieval/sync.rs:205` — `STACK_CHUNK_TARGET = 1200`
- `docs/research/2026-05-06-retrieval-stack-benchmark.md` — chunk×model matrix
