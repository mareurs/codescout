---
status: open
opened: 2026-08-26
closed:
severity: high
owner: marius
related: [docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md, docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md]
tags: [retrieval, indexing, status, silent-failure]
unverified: 'the originally reported symptom (docs/ = 0 indexed files) is NOT reproducible at d5ed4d6f — docs/ is now fully indexed. The live, verified defect is the status-completeness gap; the original drop mechanism remains unidentified.'
last_observed: 2026-08-26
kind: bug
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
  (`docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`);
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

Then this bug's status gap reports the result as `indexed: true, queryable:
true`. That is the complete causal chain for the reporter's 486-of-1606 index:
an oversized chunk truncates the build, and status calls the truncation healthy.

`docs/` being markdown-heavy (25232 chunks over 1089 files) makes it a plausible
source of the oversized chunk, though the walk ordering that left `docs/` at
exactly 0 while `src/` kept 298 is not established — do not claim it.

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
   **Verdict:** **confirmed.** `embed_batch_dyn(&texts).await?` at `:238`
   propagates out of `flush_pending`, and `?` at `:403`/`:409` is an early
   return out of the walk. Prior batches are already committed at `:241`, and
   the prune at `:422` is skipped. An oversized chunk therefore truncates the
   build and leaves a durable partial index — which check 4 above then reports
   as healthy. See Root cause.

## Fix
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

## Tests added

None yet. The reporter's requested shape is right and is the part a
`stream_index` unit test cannot cover:

- an integration test driving the **same asynchronous tool path** as
  `index(action="build")` against a temp checkout containing `src/`, `docs/` and
  `scripts/`, then asserting persisted file paths in the final store;
- a status test asserting a deliberately partial store does **not** report an
  unqualified `queryable: true`.

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

Test hypothesis 5 first — it is the cheapest and would collapse three bugs into
one. Read the embed-failure path in `src/retrieval/sync.rs` (`stream_index`,
around the `flush_pending` call at `:380-381`) and determine whether a single
oversized chunk's HTTP 400 aborts the whole walk with `?` or is skipped. If it
aborts, that is the mechanism behind the reporter's 486-of-1606 index and this
file's Root cause section should be rewritten to say so.

Then implement Fix step 2 (zero-files-in-an-eligible-top-level-directory) as the
smallest useful guard, in `src/tools/semantic/index.rs:507-514`.

## References

- GitHub issue #17 — <https://github.com/mareurs/codescout/issues/17>
- `src/tools/semantic/index.rs:499-514` (the `(0,0)`-only discriminator),
  `:533+` (background indexing state), `:738-745` (`format_index_status`)
- `.codescout/project.toml` `[ignored_paths]` — gitignored; why `scripts/` is
  correct at 15 of 19
- `docs/issues/2026-08-26-dense-embedder-slot-context-drops-large-embeds.md`,
  `docs/issues/2026-08-26-catalog-reindex-fails-closed-on-embedding-error.md` —
  candidate mid-build abort mechanisms
