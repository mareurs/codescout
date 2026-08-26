---
kind: bug
status: fixed
tags:
- retrieval
- indexing
- status
- silent-failure
closed: 2026-08-27
last_observed: 2026-08-26
opened: 2026-08-26
owner: marius
related:
- docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md
- docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md
severity: high
unverified: the originally reported symptom (docs/ = 0 indexed files) is NOT reproducible at d5ed4d6f — docs/ is now fully indexed. The live, verified defect is the status-completeness gap; the original drop mechanism remains unidentified.
---

# BUG: `index(action="status")` reports `indexed: true, queryable: true` off a single chunk — it never checks coverage

## Summary

A partially built semantic index is indistinguishable from a complete one. The
only thing `index(action="status")` checks is whether the chunk count is
non-zero; there is no comparison against the files the walk was supposed to
cover. A build that dies partway leaves a confidently healthy status over an
index with holes, and nothing marks those files dirty, so a later no-op sync
never reconciles them.

Imported from GitHub issue #17 (reporter: mic-urs, 2026-08-26), which reported
this as `docs/` being omitted from the index. **That symptom is no longer
reproducible** (see Evidence). Re-triage found a live, adjacent defect that
fully explains why the reported condition was invisible, and that one is
verified at the code.

## Symptom (Effect)

Reported (2026-08-26, reporter's checkout): the index reported 486 indexed files
and 21,557 chunks and answered queries, while `code_chunk` held:

- `docs/`: **0** distinct indexed files (1,086 tracked, not git-ignored)
- `scripts/`: 2 distinct indexed files (19 tracked)
- `src/`: 298, `tests/`: 153

Verified live (this session, at `d5ed4d6f`): a status envelope of
`indexed: true, queryable: true` is emitted for *any* non-zero chunk count,
carrying no coverage or completeness field of any kind.

## Reproduction

The reported `docs/` = 0 condition: **not reproducible** at `d5ed4d6f`.

The status-completeness gap, reproducible by construction:

1. Build an index, then delete all but one chunk row from `code_chunk` (or
   interrupt a build partway).
2. `index(action="status")`.
3. Observe `indexed: true, queryable: true` with no indication that 1 of N
   files is present.

## Environment

- Reporter: codescout `experiments`, sqlite-vec backend, local ONNX embedder.
- Re-triage: `d5ed4d6f`, `CODESCOUT_VECTOR_BACKEND=sqlite-vec`, dense =
  CodeRankEmbed-Q4_K_M via llama-server at 127.0.0.1:48081.

## Root cause

`src/tools/semantic/index.rs:499-514` — `IndexStatus::call` branches on
`project_index_stats` and the *only* discriminator is the zero case:

```rust
match client.project_index_stats(&collection, &project_id).await {
    Ok((0, 0)) => json!({ "indexed": false, /* … */ }),
    Ok((chunk_count, file_count)) => json!({
        "indexed": true,
        "queryable": true,          // ← unconditional on any non-zero count
        "file_count": file_count,
        "chunk_count": chunk_count,
    }),
    // …
}
```

`file_count` is *reported* but never *checked* against the number of eligible
files the walk would enumerate. One chunk yields the same
`indexed: true, queryable: true` shape as full coverage. `format_index_status`
(`:738-745`) then renders it as `good · queryable · N files · M chunks` — the
word "good" is derived from nothing but non-emptiness.

Background indexing state *is* appended afterwards (`:533+`), which is what the
SessionStart banner reads — so an index that is still building does say so. The
uncovered case is an index whose build **terminated early**: `IndexingState` has
nothing in flight, the collection holds a partial index, and status calls it
good.

`measured 2026-08-26: read_file src/tools/semantic/index.rs:480-535 — the (0,0)-only
discriminator confirmed by inspection.`

### Why the reported `docs/` = 0 is probably downstream of this, not separate

The reporter's own numbers are consistent with a **partial** index, not a
docs-specific drop: 486 files against 1,606 today. A build that aborts partway
through the walk would omit whatever it had not yet reached, and status would
still say `indexed: true, queryable: true`. Two live bugs can abort a build
mid-flight, both filed this session:

- an oversized chunk failing the dense embedder with HTTP 400
  (`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`);
- a bare `?` on an embed call aborting a whole target loop
  (`docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md`).

**This is now a finding, confirmed at the code on 2026-08-26.** The mechanism is
exact:

```
sync.rs:238  flush_pending →  let embeds = embedder.embed_batch_dyn(&texts).await?;
sync.rs:403  stream_index  →  added += flush_pending(...).await?;
sync.rs:409  stream_index  →  added += flush_pending(...).await?;
sync.rs:422  stream_index  →  the prune step — skipped by the early return
```

Four consequences compound:

1. **Batch granularity.** `embed_batch_dyn` takes the whole `pending` buffer.
   One oversized chunk fails *every* chunk in that flush batch, not just itself.
2. **The walk aborts.** `?` at `:403`/`:409` is an early return out of
   `stream_index`, mid-walk. Every file not yet reached is simply absent.
3. **Earlier batches are already committed.** `store.upsert_chunks` at `:241`
   ran for every prior flush, so a partial index is durably persisted.
4. **The prune at `:422` is skipped**, so stale rows also survive.

Then this bug's status gap reports the result as `indexed: true, queryable: true`.

### CORRECTION 2026-08-26 — the mechanism is real, the trigger is NOT reachable

An earlier revision of this section called the above "the complete causal chain
for the reporter's 486-of-1606 index: an oversized chunk truncates the build, and
status calls the truncation healthy." **The second half stands; the first half was
wrong, and was measured wrong rather than reasoned wrong.**

Measured on the live stack, 2026-08-26:

| Probe | Result |
|---|---|
| Per-input ceiling, by binary search on `POST /v1/embeddings` | between **8000 and 8250 chars** (~2000-2062 tokens) — matches CodeRankEmbed's `n_ctx_train` = 2048 exactly |
| `256 × 1200`-char inputs in one request (production's `DEFAULT_FLUSH_BATCH`) | **HTTP 200** — the limit is per input, not per batch total |
| `MAX(LENGTH(content))` grouped by language | **1200 for every language**, markdown included; **zero** chunks over 8000 — but see the substrate warning below |

> **⚠ Substrate correction, 2026-08-26 (`bug-fix-session-log:F-66`).** The third
> row read `.codescout/embeddings/codescout.db`, which on this host is a
> **retired** store: `CODESCOUT_VECTOR_BACKEND` is unset, `VectorBackend::resolve`
> defaults to Qdrant under `server-stack`, and that file's mtime predates the
> session. The live index reports 1611 files / 47 647 chunks where the sqlite file
> says 1593 / 46 979.
>
> **The conclusion stands on other evidence.** `chunk_target` defaults to 1200 and
> the chunker enforces it as a hard cap — a property of the code, not of any store
> — and the same ceiling was independently measured in
> `docs/issues/archive/2026-08-11-chunk-size-for-model-dead-on-production-path.md`.
> The first two rows are unaffected: both were direct HTTP calls to the live
> embedder. Only the per-language table needs its provenance re-labelled.

`chunk_target` defaults to 1200 chars (`src/retrieval/sync.rs`,
`STACK_CHUNK_TARGET`) and the chunker enforces it as a hard cap — so a code or
markdown chunk sits ~6.7× below the ceiling and **cannot** trigger the refusal.
That holds on the reporter's configuration too: `--ctx-size 8192 --parallel 8`
gives 1024 tokens/slot ≈ 4000 chars, still over 3× a 1200-char chunk.

Two consequences, both honest:

- **The abort mechanism is confirmed** (read at the code, `sync.rs:238` → `:403`)
  and worth fixing on its own — any batch failure, from any cause, truncates the
  walk and the result reports healthy. That is what `a5f8e5ad` fixed.
- **What truncated the reporter's index is still unknown.** An oversized chunk is
  now ruled out. Do not re-file it as the cause. The `docs/`-is-markdown-heavy
  hypothesis is also refuted by the table above — markdown obeys the same cap.

The reachable surface for an oversized payload is `cross_embed_memory`, which
sends a whole memory in one request with no cap at all — see
`docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`.

**The codebase already knows this hazard class and reasoned about it once, in
the wrong place.** `src/retrieval/sync.rs:581` in `sync_worktree` says:

> Every `flush_pending(...).await?` below is an early return sitting between a
> committed upsert and this write.

That comment exists to order a sidecar write against the same early return — for
a different consequence (a stale-sidecar double-serve). The identical reasoning
was never applied to `stream_index`, where the consequence is a silently
truncated index.

## Evidence

### `docs/` is fully indexed at `d5ed4d6f` — the reported symptom is gone

`index(action="status")`: `file_count: 1606`, `chunk_count: 47457`,
`git_sync: up_to_date`, `last_indexed_commit: d5ed4d6f`.

Direct query of the same table the reporter inspected:

```
sqlite3 .codescout/embeddings/codescout.db \
  "SELECT substr(file_path,1,instr(file_path||'/','/')-1) AS topdir,
          COUNT(DISTINCT file_path) AS files, COUNT(*) AS chunks
   FROM code_chunk GROUP BY topdir ORDER BY files DESC;"

topdir   files  chunks
docs      1089   25232      ← reporter measured 0
src        298   19685
tests      153    1380
scripts     15     112      ← reporter measured 2
crates       6     244
```

### The reporter's premise "no configured ignored paths" is false for this repo

`.codescout/project.toml` `[ignored_paths]` has six patterns, two of which
land in `scripts/`:

```toml
patterns = [
    "docs/research/2026-04-03-embedding-model-benchmark.md",
    "docs/research/2026-05-06-retrieval-stack-benchmark.md",
    "docs/trackers/retrieval-benchmark.md",
    "scripts/run-tc-benchmark.py",
    "scripts/tc-suites/**",
    ".codescout/projects/**",
]
```

`scripts/` at 15 of 19 tracked is therefore **entirely correct**: 2 files are
explicitly ignored (`run-tc-benchmark.py`, `tc-suites/legacy-natural.json`) and
2 are JSON (`package.json`, `tc-kotlin.json`), which is not in this project's
`languages` list. Confirmed by diffing `git ls-files 'scripts/*'` against the
indexed set, 2026-08-26.

Note this does **not** explain the reported `docs/` = 0 — the three ignored
docs entries are specific files, not a subtree glob. That observation was
genuinely anomalous.

## Hypotheses tried

1. **Hypothesis:** Markdown recognition, chunking, sqlite-vec persistence or the
   ONNX embedder drops `docs/` in isolation.
   **Test:** reporter's existing lower-level checks — `indexable_files` sees
   Markdown under `docs/`; `stream_index` stores a temp Markdown doc; a
   full-checkout `stream_index` test sends `docs/` chunks to a recording store.
   **Verdict:** rejected by the reporter, correctly. Narrows to the live path.
2. **Hypothesis:** `scripts/` at 15-of-19 is a second instance of the same drop.
   **Test:** diffed tracked vs indexed and read `[ignored_paths]`.
   **Verdict:** rejected — fully explained by config. Not evidence of a bug.
3. **Hypothesis:** the `docs/` = 0 condition still reproduces.
   **Test:** re-ran the reporter's own SQL at `d5ed4d6f`.
   **Verdict:** rejected — 1089 files / 25232 chunks.
4. **Hypothesis:** status cannot distinguish partial from complete coverage,
   which is why (1) was invisible rather than loud.
   **Test:** read `src/tools/semantic/index.rs:499-514`.
   **Verdict:** **confirmed.** This is the live defect.
5. **Hypothesis:** the original partial index was caused by a mid-build embed
   abort (bugs #15 / #19 above).
   **Test:** read `flush_pending` (`src/retrieval/sync.rs:223-243`) and its two
   call sites in `stream_index` (`:403`, `:409`).
   **Verdict:** **split — mechanism confirmed, trigger REFUTED.**
   `embed_batch_dyn(&texts).await?` at `:238` propagates out of `flush_pending`,
   and `?` at `:403`/`:409` is an early return out of the walk; prior batches are
   already committed at `:241` and the prune at `:422` is skipped. So *a* batch
   failure does truncate the build and leave a durable partial index that check 4
   reports as healthy — confirmed, and fixed in `a5f8e5ad`.
   But **an oversized chunk cannot be that failure.** Measured 2026-08-26: the
   per-input ceiling is 8000-8250 chars, every live chunk is ≤1200 in every
   language, and a 256×1200 batch returns HTTP 200. See the CORRECTION block in
   § *Root cause* for the full table. This entry previously read "confirmed" and
   overstated what had been established — the code was read, the substrate was
   not measured until later.
6. **Hypothesis:** `docs/` being markdown-heavy makes markdown the likely source
   of an oversized chunk (the reporter's missing corpus was `docs/`).
   **Test:** `MAX(LENGTH(content))` grouped by language over all 47k live chunks.
   **Verdict:** rejected — markdown's max is 1200, identical to Rust's. The
   chunker's cap is language-independent.
7. **Hypothesis:** production's `DEFAULT_FLUSH_BATCH = 256` makes the *batch
   total* (~307k chars) exceed a server limit, even though each input is small.
   **Test:** posted 128 and 256 inputs of 1200 chars to the live embedder.
   **Verdict:** rejected — both returned HTTP 200. The limit is per input.

## Fix
### Reporting gap closed 2026-08-26 — status no longer implies completeness

- **SHA:** `48825529` (`experiments`)
- **patch-id:** `f855a10a1afca25211fb9504fba17964d6ab29a9`

`fix(index): stop status implying completeness, and fold in the cheap integrity
check`. This is the half `e5821fec` left open: `verify` closed *detection*, but it
is opt-in, so a caller who never ran it saw exactly the envelope this file
complains about.

**What `status` now says.** `chunks_without_vectors` + `integrity: ok|degraded`
from one indexed COUNT, and — the part that matters most —
`coverage: "unchecked"` with a `coverage_hint` naming `verify`. Coverage cannot be
answered cheaply, because it needs the indexer's own walk. So `status` states that
rather than letting silence imply the opposite: `file_count`/`chunk_count` are what
the store **holds**, never proof it holds everything eligible.

**`queryable` is deliberately still `true`.** An index with a hole is queryable —
it simply cannot return the holed chunks. Downgrading it would break every caller
that branches on it, so the honest signal had to be an additional field rather than
a lie in an existing one.

**Why the fold is safe here and would not be on the activation path.** `status`
already calls `project_index_stats`, which enumerates the project, so one more
indexed COUNT is proportionally nothing. Activation asks `project_has_chunks`
instead, and `check_has_index`'s doc comment records why: `project_index_stats`
could not finish inside `FIRST_PROBE_TIMEOUT` on a real corpus, so every large
project reported as unindexed and — a timeout being deliberately uncached —
re-scanned on every activation
(`docs/issues/archive/2026-08-08-index-probe-scrolls-the-whole-corpus-to-answer-a-yes-no.md`).
**Orphan detection therefore stays in `verify`**: it needs `chunk_refs` over every
chunk plus a stat per stored file, which is that same enumeration class.

**Item 1 is inert on a Qdrant host, and that is expected.**
`count_chunks_without_vectors` returns 0 structurally under Qdrant, since a point
carries payload and vector together — it is a real measurement only under
sqlite-vec. The `coverage: "unchecked"` change is the backend-independent one, and
is why the fold was worth doing anyway.

**Also: the word "good" is gone.** `format_index_status` opened every compact line
with `good · queryable · N files · M chunks`, and "good" was derived from nothing
but non-emptiness — this file's own § *Root cause* names it. Now `indexed ·
queryable · …`, or `DEGRADED · N chunk(s) have no vector · …` when there is a real
signal. A permanent "coverage unchecked" nag on every line was considered and
rejected: noise gets learned around, and the JSON carries the hint for anyone
reading it.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` →
4494 passed, 0 failed, 46 ignored.

**What is left before this can be archived.** Only the durable half: nothing
persists "the last refresh was partial" across calls, so a caller who does not look
at the envelope that reported it still cannot find out later. That is
`docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md` step 2,
and the two should be designed together — they are one missing state model, which
is the third acceptance criterion that bug already asks for.
### `index(action="verify")` shipped 2026-08-26 — headline defect closed

- **SHA:** `e5821fec` (`experiments`)
- **patch-id:** `1d51dfb60e6b913f2478de9d848c96d36eb352cc`

`feat(index): add index(action="verify") — a read-only integrity check`. Reports
coverage against the indexer's own walk, orphaned rows, chunks with no vector, and
`empty_eligible_dirs` — this file's requested minimum bar — then derives **one**
verdict (`complete | stale | incomplete`) rather than leaving a reader to
reconcile six fields.

The verdict ordering is the design: `stale` is checked **before** `incomplete`,
because an index legitimately behind HEAD is *expected* to be short of the files
those commits added. Calling that incomplete would fire on nearly every project
nearly all the time, and a check that always complains gets turned off. A vector
hole or a wholly-absent eligible directory outranks staleness, since no number of
pending commits explains either. Four tests pin those arms.

**Live output on this repo, read after `cargo rb` + reconnect** — and this is the
first substrate-correct measurement of this project's index in the whole
investigation:

```
verdict: "stale"          expected_files: 1612   stored_files: 1611
missing_count: 7          orphan_count: 6        empty_eligible_dirs: []
chunks_without_vectors: 0 git_sync: behind 12 (c0e3a574 → 8baa4952)
hint: "Index is 12 commit(s) behind HEAD; the 7 missing file(s) are explained by that."
```

Three things worth reading off that:

1. **`stale`, not `incomplete`** — 7 missing and 6 orphans while 12 commits behind
   is exactly the case that must not cry wolf. The arm ordering earns its keep on
   the first real invocation.
2. **The missing and orphan lists are a matched pair.** Six of the seven missing
   are `docs/issues/archive/X`; all six orphans are `docs/issues/X` — the same
   files either side of an archive move. Neither list alone says that; together
   they diagnose it.
3. **`empty_eligible_dirs: []` is the independent confirmation** that `docs/` is
   fully covered — the claim the retired-store table was originally cited for
   (see the substrate warning under § *Root cause*).

**`chunks_without_vectors: 0` proves nothing on this host.** The backend is
Qdrant, whose impl returns `Ok(0)` structurally because a point carries payload
and vector together. The field is only a measurement under sqlite-vec. That is
documented at both impls, and is exactly why the trait method has no default.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` →
4492 passed, 0 failed, 46 ignored.

**Still open.** This closes the *detection* gap, not the reporting one:
`index(action="status")` itself is unchanged and still answers `indexed: true,
queryable: true` off a non-zero chunk count. `verify` is opt-in, so a caller who
never runs it sees exactly what they saw before. Folding the cheap tier
(vector holes, orphans) into `status` needs no walk and would make the default
surface honest — that plus the durable degraded marker from
`docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md`
step 2 is what would let this bug be archived.
### Progress 2026-08-26 — root cause fixed, headline defect still open

**Fixed:** the mechanism that *created* the partial index. `flush_pending`
(`src/retrieval/sync.rs`) no longer lets a batch failure abort the walk. On an
`embed_batch_dyn` error it retries chunk-by-chunk, stores everything that
embeds, and reports the rest through a new `skipped` channel threaded
`flush_pending` → `stream_index` → `SyncReport.skipped` → the `index` tool's
status `detail` (which now says `INDEX INCOMPLETE` with a sample).

A minimal `"ok"` probe distinguishes "one oversized chunk" from "embedder down",
because the two need opposite handling and "all chunks in this batch failed" is
ambiguous between them. A dead embedder still aborts loudly — skipping
everything would turn an outage into an empty-but-successful sync, which is
worse than today's behavior.

**Still open — this file's headline defect is untouched.** The `(0,0)`-only
discriminator at `src/tools/semantic/index.rs` is unchanged: a partial index
still reports `indexed: true, queryable: true`. Fix steps 1-3 below are all
outstanding. The difference is that a *newly* truncated index now announces
itself in the sync report; an index truncated by any other cause, or one
truncated before this change, still reads as healthy.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings` (also with
`--features dashboard`), `cargo test` → 4470 passed, 0 failed, 46 ignored.

**Fix provenance (root cause only — this bug's headline defect stays open):**

- **SHA:** `a5f8e5ad` (`experiments`)
- **patch-id:** `f248f9e159cf60f848ddf487182f3dc3125ba21b`

`fix(retrieval): stop one oversized chunk from truncating the whole index build`.
That commit removes the *cause* of partial indexes; the `(0,0)`-only status
discriminator this file is named for is untouched. The patch-id is the durable
anchor — `experiments` is rebased after every ship, so the SHA will orphan.

Plan (not yet implemented). The reported drop is unreproducible, so the fix
targets the verified gap — and it is also the instrument that would have caught
the drop:

1. **Give status a coverage signal.** Compare stored distinct `file_path` count
   against the eligible-file count from the same walk `index(build)` uses
   (`indexable_files` + resolved ignore patterns). Report `expected_files`
   alongside `file_count`, and downgrade `queryable: true` to a degraded state
   when coverage is materially short.
2. **Refuse to call an index complete when an eligible top-level directory has
   zero stored files** — the reporter's own minimum bar, and a cheap, high-signal
   invariant.
3. **Instrument the build path** with per-directory / per-language counters:
   enumerated, unreadable, chunked, submitted for embedding, successfully
   embedded, stored, pruned. Capture the resolved root and ignore patterns at
   job start. Without this, a recurrence is as unattributable as this one was.
4. Do **not** weaken the `(0,0) → indexed: false` branch; it is correct.

No SHA, no patch-id — not yet fixed.

### Durable marker shipped 2026-08-27 — headline defect closed

**SHA:** `196f1b94` (`experiments`)
**patch-id:** `2a8aa5e71f2ae98171628a665f81af7c9cfda31d`

`fix(index): persist a durable last-sync-skipped marker, and read it in status`.

**What was missing after the two entries above:** `a5f8e5ad` stopped a batch failure
from truncating the walk and reported `skipped` in the in-memory `SyncReport`; `e5821fec`
added opt-in walk-based detection via `verify`. Neither made the fact survive past the
call that discovered it — `status` itself still answered `indexed: true, queryable: true`
off a non-zero chunk count alone, exactly this file's headline complaint, for a caller who
never ran `verify` and wasn't looking at the sync's own return value.

**What shipped.** `IndexState` (`src/retrieval/index_state.rs`) gains
`last_sync_skipped_count` / `last_sync_skipped_sample`, sourced from `SyncReport.skipped`.
`write_index_state_with_dirty` takes a new required `skipped: &[String]` parameter — no
default, matching this file's own existing `ModelStamp` precedent ("a defaulting
convenience wrapper is what produced the `dirty_paths` wipe") — so the compiler named
both production call sites. `sync_project` passes its real `skipped` list; `status` reads
it back, sets `last_sync_skipped: {count, sample}` when non-zero, upgrades `integrity` to
`degraded`, and `format_index_status` renders `DEGRADED · last sync skipped N chunk(s)…`
ahead of a vector hole (a skipped chunk never reached the store at all, the more severe of
the two cheaply-knowable facts).

**Known, deliberate gap: `sync_worktree`.** Its sidecar write happens BEFORE the embed
pass runs — a load-bearing ordering documented at that call site (I3: written early to
avoid a double-serve race between main and a worktree delta) — so this run's own skip
count is not knowable at the point the write happens. It passes `&[]`, commented as "not
yet knowable here", not as a claim of cleanliness. A worktree delta sync's skips are
therefore still invisible to `status`. Closing this would need a second sidecar write
after the embed pass, with its own reasoning against the same early-return hazard — left
for a follow-up rather than folded into this pass.

**What is still NOT covered, and is not a new gap.** A build interrupted by something
other than a per-chunk embed failure — a killed process, a server restart mid-walk —
never returns, so no sidecar write happens for that run at all; `status` would then read
the PREVIOUS successful sync's clean marker. This was already honestly disclosed by
`coverage: "unchecked"` (added in the first Fix entry above) rather than hidden, and
`index(action="verify")`'s walk-based check still catches it when run. Nothing here
claims to close that residual case — only the confirmed mechanism (a batch/embed failure
surviving as a silent truncation) is closed.

Gate: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` → 4411
passed, 0 failed, 8 ignored.

This bug's own two acceptance criteria for archiving — the cheap-tier fold (done in the
first Fix entry) plus this durable marker — are both met. Archiving now.
## Tests added

Eight, across three files (`src/retrieval/index_state.rs`, `src/retrieval/sync.rs`,
`src/tools/semantic/tests.rs`):

- `index_state::tests::last_sync_skipped_round_trips`,
  `::sidecar_written_before_last_sync_skipped_existed_still_parses`,
  `::a_clean_sync_records_zero_skipped_not_a_stale_carry_over`,
  `::last_sync_skipped_sample_is_capped_but_count_stays_exact` — the writer/reader layer.
- `sync::tests::sync_project_records_its_own_skip_count_in_the_durable_sidecar` —
  end-to-end through the real `sync_project` path with a `CeilingEmbedder`, asserting the
  sidecar's count matches what the sync actually skipped and the sample names the
  offending file.
- `sync::tests::sync_project_clears_a_previously_recorded_skip_count_on_a_clean_run` — the
  converse: a clean sync must not carry a stale skip count forward.
- `semantic::tests::format_index_status_leads_with_degraded_when_the_last_sync_skipped_chunks`,
  `::format_index_status_skipped_outranks_a_vector_hole` — the rendering layer, including
  priority against the existing vector-hole arm.

`IndexStatus::call` itself (the JSON-assembly splice reading the sidecar and setting
`result["last_sync_skipped"]`) has no direct unit test, consistent with the rest of that
function: it requires a live `RetrievalClient::from_env` connection and has never had one.
The splice reads exactly the fields the writer test round-trips, in the same `if let
Some(...)` pattern already used two lines below for `indexed_with_model` — covered by
construction plus live verification (`cargo rb` + reconnect + a real
`index(action="status")` call) rather than a mocked unit test.
## Workarounds

Do not trust `indexed: true` as a completeness claim. Verify coverage directly:

```
sqlite3 .codescout/embeddings/<project>.db \
  "SELECT substr(file_path,1,instr(file_path||'/','/')-1) AS topdir,
          COUNT(DISTINCT file_path) FROM code_chunk GROUP BY topdir;"
```

Compare against `git ls-files`, remembering to subtract `[ignored_paths]` and
any extension outside the project's `languages`. `index(action="build",
force=true)` rebuilds if a directory is short.

## Resume

N/A — fixed and archived. The sibling gap in `librarian(action="reindex")`'s catalog/
embedding path (`docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md`
step 2) is intentionally NOT covered here — separate store (the librarian catalog vs. this
file's code-chunk index), separate change. That bug's own Resume now points back at this
fix as a worked precedent for the shape of a durable partial-sync marker, not as something
that already covers it.
## References

- GitHub issue #17 — <https://github.com/mareurs/codescout/issues/17>
- `src/tools/semantic/index.rs:499-514` (the `(0,0)`-only discriminator),
  `:533+` (background indexing state), `:738-745` (`format_index_status`)
- `.codescout/project.toml` `[ignored_paths]` — gitignored; why `scripts/` is
  correct at 15 of 19
- `docs/issues/archive/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`,
  `docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md` —
  candidate mid-build abort mechanisms
