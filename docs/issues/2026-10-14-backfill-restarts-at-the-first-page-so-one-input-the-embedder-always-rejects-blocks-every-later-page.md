---
id: '4907283344727074'
kind: bug
status: open
title: 'BUG: backfill restarts at the first page on every run, so one input the embedder always rejects blocks every later page'
owners:
- marius
tags:
- cluster/unclassified
topic: backfill-chunks liveness
closed: ''
opened: 2026-10-14
related: []
severity: medium
---

# BUG: backfill restarts at the first page on every run, so one input the embedder always rejects blocks every later page

## Summary

Since `62177b8c`, each backfill run starts its page walk at `(i64::MIN, "")`, and the persisted cursor is never read. An embed failure propagates through `pending.push(res?)`, which also drops that page's vectors that were computed but not yet written. If one chunk is rejected deterministically, every run re-selects the same artifact in the same page and fails at the same point, so no artifact at or after that page is ever repaired. Before `62177b8c`, the first failure left chunk rows, the `NOT EXISTS chunk` selector stopped selecting the artifact, and later runs made progress (while leaving that artifact vectorless — the R-01 defect).

## Symptom (Effect)

Not reproduced. The mechanism follows from the code: `cur_updated`/`cur_id` are initialised to `i64::MIN`/`""` at every entry; the failing artifact keeps a missing vector, so the gap predicate keeps it in the selection; `?` returns before the page flush.

## Reproduction

Use an embedder that returns an error for one specific text, and a catalog where that artifact sorts early by `(updated_at, id)`; run backfill twice. Expected (mechanism): both runs fail at the same artifact, and no later page is written.

## Environment

Any remote embedder that rejects a specific input (HTTP 4xx/5xx on that payload). Not observed in the field.

Since `8c02ad77`, only a process whose backend is `SqliteVec` re-selects the failed artifact. The no-chunk arm writes chunk rows through `replace_chunks` (inside `embed_queue_items`) before it embeds, and the gap arm is the only predicate that selects an artifact that has chunk rows. On any other backend the selector has no gap arm, so a later run does not select that artifact again and walks past it, and that run gives it no vector.

## Root cause

Liveness was traded for completeness: the fix removed the cursor's progress (correctly — it concealed holes) without adding a way to move past an item that cannot succeed.

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), findings 2 and 11 (the discarded page vectors come from the same `?`).

## Hypotheses tried

N/A.

## Fix

Unknown — under investigation. Options: write `pending` before propagating the error; count a per-item failure and continue rather than abort the run.

## Tests added

N/A — not fixed.

## Workarounds

Fix or replace the embedder for the rejected input, then rerun.

## Resume

Open. Not in the current fix set (findings 1 and 2).

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`)
