---
id: 44eae6408ddbe92d
kind: bug
status: fixed
title: 'BUG: on a Qdrant host backfill never retries a chunk whose embed failed, because the selector has no arm for a chunk row that lacks a vector'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: 2026-10-18
opened: 2026-10-18
related: []
severity: medium
unverified: STANDING the fix was tested only on InMemoryArtifactStore, which shares the Store target's selector but not the Qdrant client. The Qdrant behaviour was read from the code and was not run against a Qdrant.
---

# BUG: on a Qdrant host backfill never retries a chunk whose embed failed, because the selector has no arm for a chunk row that lacks a vector

## Summary

`backfill_chunk_vectors_with` (`src/librarian/indexer.rs`) selects an artifact for repair by two arms. The first arm, `NOT EXISTS (… artifact_chunk …)`, selects an artifact with no chunk rows. The second arm, `gap_arm`, selects an artifact with a non-blank chunk that has no `artifact_vec_v2` row. `gap_arm` is an empty string unless the target is `BackfillTarget::SqliteVec`. On a `BackfillTarget::Store` run, only the first arm exists.

An embed failure keeps its chunk row. Fix `56072827` made such a failure a per-chunk count instead of an abort. On a Store run the artifact then has chunk rows and no arm selects it. Every later run walks past it, and the chunk never gets a vector in the store that search reads.

## Class fit

This bug shows the trait of the class: a gate needs the event "this chunk has no vector that search can read", and it proxies that event with a value it can read, "this artifact has no chunk rows". On a Qdrant host the proxy is wrong for exactly the chunks that need repair, because the failed run has already created their rows. It differs from the class's other backfill members in one way: it is not wrong about the first run. It becomes wrong only after a run that partly failed.

## Symptom (Effect)

Reproduced by a run on 2026-10-18, in a scratch worktree at `4d9dc2978`. The test was not committed.

- First run, `backfill_rejecting_svc` and `BackfillTarget::Store`: `artifacts=1 embedded=0 embed_failed=1`. The store holds no vector for the chunk.
- Second run, a healthy embedder, same catalog and store: `artifacts=0 embedded=0 embed_failed=0`. The store still holds no vector for the chunk.

The second report reads as a clean no-op while the chunk is still unembedded. A `knn` search never returns that chunk.

## Reproduction

Run on 2026-10-18 with the helpers that exist in the `tests` module of `src/librarian/indexer.rs`:

1. Build the catalog and the artifact as `a_store_backfill_does_not_check_artifact_vec_v2s_dimension` does: `index_repo_sync` over `bare.md` (`# Bare` and a line `beta`), and `InMemoryArtifactStore::default()` from `crate::librarian::artifact_store::test_support`.
2. Run `backfill_chunk_vectors_with` with `backfill_rejecting_svc("beta")` and `BackfillTarget::Store { store: &store, project_id: &project_id }`. Observed: `embed_failed == 1`, no vector in the store.
3. Run it again with `backfill_svc_at_dim(768)`. Observed: `artifacts == 0`, and `store.chunk(&chunk_id)` is still `None`. A fixed selector would embed the chunk in this run.

The assertion `store.chunk(&chunk).is_some()` after step 3 failed with the message "the second run must embed the chunk the first run could not". The fixed code should make it pass, so it is the regression test for any fix.

## Environment

Any process whose `BackfillTarget` is `Store`: the server-stack build, whose default artifact backend is Qdrant, and any project that selects Qdrant in `.codescout/project.toml` or by environment. A sqlite-vec process is not affected, because `gap_arm` selects the artifact again there.

## Root cause

`ArtifactVectorStore` (`src/librarian/artifact_store.rs`) has `upsert`, `delete`, `refile` and `knn`, and no coverage query. The selector therefore cannot ask a store "which of these chunks have a vector?". It has no source for that fact, so the `Store` run keeps only the arm that needs no vector information.

The earlier fix for the backend-blind selector (`8c02ad77`, archived under `docs/issues/archive/2026-10-14-backfill-vector-gap-selector-reads-sqlite-vec-coverage-on-a-host-that-searches-qdrant.md`) made the sqlite-vec arm conditional. That was correct, because the arm read a table that search does not read on a Qdrant host. It left the Store target with no repair arm. This was a stated limit of that fix and of `56072827`, and it is not a regression.

## Evidence

- `gap_arm` is built by `if matches!(target, BackfillTarget::SqliteVec) { format!(…) } else { String::new() }`. Read this session in `backfill_chunk_vectors_with` at commit `56072827`.
- The no-chunk branch writes chunk rows through `embed_queue_items` before it embeds. A failed embed leaves those rows.
- Fix `56072827` documents the limit in its message and in the archived record `docs/issues/archive/2026-10-14-backfill-restarts-at-the-first-page-so-one-input-the-embedder-always-rejects-blocks-every-later-page.md`.

Not checked: whether `index_repo` leaves the same state when an embed fails during ordinary indexing on a Qdrant host.

## Hypotheses tried

One hypothesis, tested by the run above: a chunk row that the failed run created is not selected by the `Store` run's one arm. The run confirmed it. No other hypothesis was tried.

## Fix

Verified and committed on **experiments**.

- **SHA:** `3f1986461c57dce935c5024ec36dbd74e3f898cb`
- **patch-id:** `7fe214f4f224b3a86004340090c76208e5862f96`

A `Store` run now undoes its own rows for an artifact whose chunk failed to embed. In the branch where the run creates the rows (`stored.is_empty()`), if any chunk of the artifact fails, it calls `replace_chunks(.., &[])`, which deletes the rows, and it stores none of that artifact's vectors. The artifact has no rows again, so the first arm of the selector (`NOT EXISTS (… artifact_chunk …)`) selects it on the next run. A retry embeds every chunk of the artifact once more.

The finished vectors of that artifact are dropped too. `replace_chunks` mints new chunk ids for rows it inserts, so a vector kept after the rollback would sit in the store under an id no row names.

This is a third option that the record's list did not hold. It adds no trait method and no catalog state, and it can be tested here. The coverage query and the durable failure record were both heavier: the first needs a `server-stack` half that no lane compiles, and the second needs a table, a schema version and a clean-up.

A sqlite-vec run is unchanged. Its gap arm sees a row without a vector, so it keeps the rows and the vectors that landed, and the next run embeds only the missing chunk.

### Limits

- Not run against a Qdrant. The tests use `InMemoryArtifactStore`, which shares the `Store` target's selector but not the Qdrant client. The change does not touch the client.
- A crash between the creation of the rows and the flush of the page leaves rows without vectors, and no `Store` run selects that artifact. Only a failed embed is fixed here. `librarian(action="reindex", reembed=true)` repairs the crash case. A coverage query on `ArtifactVectorStore` would repair it too, but its Qdrant half is code that no lane compiles.
- A chunk that fails on every run makes its artifact cost one embed call per chunk on every run, and the healthy chunks of that artifact are embedded and thrown away each time. This is the price of the stateless design.
- An embedder outage still makes the run try every chunk. There is no circuit breaker.
- Not checked: whether `index_repo` leaves the same state when an embed fails during ordinary indexing on a Qdrant host.

## Tests added

Observed RED first for three of the four. The reproduction failed with `left: 0, right: 1` on `embedded`, the orphan test with `left: 1, right: 0` on `chunks_under`, and the neighbour test on the assertion that the failed artifact has no rows. The fourth test guards behaviour that did not change, so it passed from the start.

- `a_store_run_retries_a_chunk_whose_embed_failed` (`src/librarian/indexer.rs`): the reproduction of this record.
- `a_store_run_leaves_no_vector_behind_for_an_artifact_with_a_rejected_chunk`: a two-chunk artifact with one rejected chunk leaves no row and no vector, and the retry stores both chunks with none outside the rows.
- `a_store_run_keeps_the_artifacts_whose_chunks_all_embedded`: the rollback touches only the failed artifact.
- `a_sqlite_vec_run_keeps_the_finished_chunks_of_an_artifact_with_a_rejected_chunk`: a sqlite-vec run keeps the vector that landed and embeds only the missing chunk.

Three mutations were each killed: the rollback on every target (the sqlite-vec test), the finished vectors kept after the rollback (the orphan test), and the rollback with no failure (three tests).

`./scripts/gate.sh` ran once. Clippy, the lean lane and the default lane were green: 4845 library tests passed in the default lane, 0 failed. The formatting lane refused `src/librarian/indexer.rs` because it could not establish who owned the bytes. Every hunk of the diff was this change, so the file was formatted with `rustfmt --edition 2021`, which only reflowed the new tests. `rustfmt --check` then exited 0. The three lanes were not run again on the formatted bytes. Not covered by the gate: the heavy Python suites, `server-stack` and the ONNX weight tests.

## Workarounds

`librarian(action="reindex", reembed=true)` embeds every chunk again through `index_repo`, which writes to the store that search reads.

## Resume

Fixed in `3f1986461c57dce935c5024ec36dbd74e3f898cb`. Reopen if a `Store` run leaves an artifact with chunk rows and no vector after a failed embed, or if a Qdrant run shows a different result from the in-memory store.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `gap_arm`)
- `src/librarian/artifact_store.rs` (`ArtifactVectorStore`)
- `docs/issues/archive/2026-10-14-backfill-restarts-at-the-first-page-so-one-input-the-embedder-always-rejects-blocks-every-later-page.md`
