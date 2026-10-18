---
id: 97c5eae394c70358
kind: bug
status: fixed
title: 'BUG: backfill restarts at the first page on every run, so one input the embedder always rejects blocks every later page'
owners:
- marius
tags:
- cluster/unclassified
topic: backfill-chunks liveness
closed: 2026-10-16
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

Verified and committed on **experiments**.

- **SHA:** `56072827be115ff4f2fd466920966e707afaf71d`
- **patch-id:** `f5e660ee53f8ed56a31482ceddd726c75261a28f`

`backfill_chunk_vectors_with` no longer ends the run on an embed failure. It maps each chunk of a page to its embed result, keeps the vectors that succeeded, and counts each failure. `BackfillReport` gains `embed_failed` (the count) and `embed_errors` (the first 20 failures, as `<chunk_id>: embed failed: <error>`). The design follows `embed_errors` in `src/librarian/tools/reindex.rs`.

A failed catalog or store write still ends the run. The cursor must never pass a vector that was not written, so that rule is unchanged.

The `backfill-chunks` CLI prints `embed_failed` and `embed_errors`. It exits non-zero when any chunk stayed unembedded, so a script cannot read a partial run as a complete one.

### Limits

- A store or catalog write failure still aborts the run. That page's `pending` vectors are lost, and the next run restarts at page one.
- On a Qdrant host the selector has no gap arm. Backfill does not select a failed chunk's artifact again there, so it does not retry that chunk. `librarian(action="reindex", reembed=true)` repairs it.
- An embedder outage makes the run try every chunk, report `embed_failed = N` and exit non-zero. There is no circuit breaker.
- On a sqlite-vec host each run selects the failed chunks again and retries each one once. This is intended: a chunk that always fails costs one embed call per run and blocks nothing.
- The persisted cursor is still not read. A run still starts at page one. Liveness now comes from the run going on past a failure, not from resuming.

## Tests added

Observed RED first. The four indexer tests failed at `unwrap()` on the run's `Err`. The two CLI tests failed against a stub.

- `a_chunk_the_embedder_rejects_does_not_block_the_artifacts_after_it` (`src/librarian/indexer.rs`)
- `a_rejected_chunk_keeps_the_vectors_of_its_artifacts_other_chunks` (`src/librarian/indexer.rs`)
- `the_report_names_a_failed_chunk_and_caps_its_sample` (`src/librarian/indexer.rs`)
- `the_printed_report_carries_the_failed_chunks` (`src/cli/backfill_chunks.rs`)
- `a_run_that_left_chunks_unembedded_exits_with_an_error` (`src/cli/backfill_chunks.rs`)

Two existing tests changed because their premise was this defect. `backfill_retry_recovers_chunks_left_by_an_embedding_failure` now expects the first run to return `Ok` with `embed_failed == 2`. `backfill_cursor_never_passes_vectors_waiting_for_the_final_flush` now fails page 2 at its write (a wrong-width vector) and asserts the error contains "dimension". An embed failure no longer ends a run, so the old setup could not reach the invariant that test guards.

Seven mutations were each killed: the failure count not incremented, the sample cap off by one (`<=`), a healthy vector dropped, the error message shape changed, the exit-0 condition widened, `embed_errors` not printed, and a wrong count printed.

The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4509 passed, default library 6835 passed, 0 failed. An earlier run showed 2 failures, both `Disk quota exceeded` on a `/tmp` write, in tests that this change does not touch. Both passed when run alone, and the rerun was green.

## Workarounds

Fix or replace the embedder for the rejected input, then rerun.

## Resume

Complete. Fixed in `56072827`, patch-id above. The limits are listed under Fix.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`)
