---
status: open
opened: 2026-07-27
closed:
severity: high
owner: marius
related:
  - docs/issues/2026-07-27-indexer-walks-git-and-tool-state-dirs.md
tags: [indexer, chunker, retrieval, performance, embedding-cost]
kind: bug
---

# BUG: AST chunker has no minimum chunk size — 12% of chunks are single lines, inflating re-embed to ~2h

## Summary

`nodes_to_chunks` emits one chunk per inner declaration "regardless of size",
with no floor. On a Kotlin codebase this produces 5,133 single-line chunks and
14,926 chunks of ≤5 lines out of 43,582 — despite a configured `chunk_target` of
1200 characters. The corpus is several times larger than the target implies, and
because chunk ids are content-addressed, any chunker change re-embeds all of it.
That is the direct cause of the ~2.5-hour index the user flagged.

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

Not yet implemented — needs a decision, and re-tuning invalidates the whole
corpus (every boundary change re-embeds everything, so batch it with any other
chunker change).

Candidates, cheapest first:

1. **Minimum chunk size with sibling merge.** Introduce `AST_CHUNK_MIN` (~200-300
   chars) and coalesce consecutive inner declarations below it into one chunk,
   keeping the container header. Directly removes the 12% single-line and much of
   the 34% ≤5-line population.
2. **Skip trivial declarations.** Do not emit standalone chunks for enum members,
   plain property declarations, or import groups — fold them into the container
   header chunk.
3. **Reconcile the overlap with the documented intent.** Either stop emitting the
   container header when it duplicates the first inner chunk's leading lines, or
   update `split_file`'s doc comment, which currently claims zero overlap.
4. **Reconsider markdown.** 17,067 of 43,582 chunks (39%) are `.md`, one session
   log alone producing 429. Worth deciding whether `docs/` belongs in the code
   corpus at this granularity.

Any of these should be validated against the retrieval benchmark
(`docs/research/2026-05-06-retrieval-stack-benchmark.md`) before landing — smaller
chunks were chosen deliberately for precision, so a floor trades recall sharpness
for cost and must be measured, not assumed.

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

**Clean baseline obtained 2026-07-27** (this step is done). All four concurrent indexers
were killed and the coverage probe re-run on an idle GPU. Result is identical to the
mid-run measurement:

```
mean coverage factor            : 1.27x   (unchanged)
files with >1.05x coverage      : 1047 / 1963  (53.3%)
keys with 2+ distinct end_lines : 1877  (4.1%)
worst: 8.01x Stage1Assertions.kt · 5.53x PreSolveDataValidation.kt · 5.41x SolverExplainabilityTables.kt
```

So the overlap is **structural to the chunker**, not an artifact of concurrent writers or
of measuring mid-run. Hypothesis 2's "partially rejected" verdict now rests on clean data.

**Important context added 2026-07-27:** part of the observed index *duration* was never
this bug. Four `codescout index` processes were running concurrently on the same project
(see [[2026-07-25-concurrent-index-no-project-lock]]). That inflated wall-clock time but
not chunk counts or coverage — the corpus numbers in this file stand.

Next action: decide between fix candidates 1 (minimum chunk size + sibling merge) and 2
(skip trivial declarations), and measure against
`docs/research/2026-05-06-retrieval-stack-benchmark.md`. Note the ordering decision already
taken: the throughput work
([[../superpowers/specs/2026-07-27-embedder-batch-concurrency-design]]) lands first because
it leaves vectors byte-identical and needs no score re-validation, whereas this change does.
Re-measure index duration after that ships to see how much of this bug's cost survives.

When a re-index is next run, use `codescout index --force` — its delete pass clears the
4.1% of superseded chunks that accumulated from the interrupted runs.
## References

- `src/embed/ast_chunker.rs:828` — `AST_CHUNK_TARGET`
- `src/embed/ast_chunker.rs:537-546, 610-660` — inner-node decomposition policy
- `src/embed/ast_chunker.rs:844-886` — `split_file`, target-as-ceiling
- `src/retrieval/sync.rs:113-119` — walker
- `src/retrieval/sync.rs:148` — unchanged-id skip
- `src/retrieval/sync.rs:179-190` — delete-after-full-walk
- `src/retrieval/sync.rs:205` — `STACK_CHUNK_TARGET = 1200`
- `docs/research/2026-05-06-retrieval-stack-benchmark.md` — chunk×model matrix
