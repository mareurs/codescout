---
id: ec55bdbaad94714b
kind: bug
status: open
title: 'BUG: on a Qdrant host backfill never retries a chunk whose embed failed, because the selector has no arm for a chunk row that lacks a vector'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: ''
opened: 2026-10-18
related: []
severity: medium
unverified: STANDING the Qdrant behaviour is read from the code and was not run against a Qdrant. The reproduction used InMemoryArtifactStore, which shares the Store target's selector but not the Qdrant client.
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

Unknown. Options:

- **Coverage query.** Add a method to `ArtifactVectorStore`, for example "which of these chunk ids have a vector". A `Store` run then walks the chunk rows in pages, asks the store which ids are present, and embeds the others. The method follows `retired_collections`: a defaulted method that only the backends that can answer override. Six types implement the trait, so a required method would touch three test doubles in `src/librarian/tools/doctor.rs` and `src/librarian/tools/mv.rs`. The Qdrant half is code behind `server-stack`. The gate does not compile it, and I have not checked for a local Qdrant to test it against. The sqlite-vec and in-memory halves can be tested locally.
- **Durable failure record.** Record each failed chunk id, for example in a catalog table or a `meta` key, and select those artifacts again on a `Store` run. This needs no store change and tests locally. A later `reindex` can embed the chunk without clearing the record, so the next run embeds it once more. `upsert` is idempotent, so that is harmless. The record needs a bound, because an embedder outage fails every chunk.
- **Say so in the report.** This option does not work as first written. A second run sees the same catalog as a run that never failed, so it cannot tell that an earlier run left a chunk unembedded. A report that names earlier failures needs the durable record of the previous option, so it is not a separate option.

## Tests added

N/A. Not fixed.

## Workarounds

`librarian(action="reindex", reembed=true)` embeds every chunk again through `index_repo`, which writes to the store that search reads.

## Resume

Open. Choose between the coverage query and the durable failure record. The reproduction above is the red test for either. Do not start the Qdrant half of the coverage query without a way to run it, because no gate lane compiles `server-stack`.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `gap_arm`)
- `src/librarian/artifact_store.rs` (`ArtifactVectorStore`)
- `docs/issues/archive/2026-10-14-backfill-restarts-at-the-first-page-so-one-input-the-embedder-always-rejects-blocks-every-later-page.md`
