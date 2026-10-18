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
---

# BUG: on a Qdrant host backfill never retries a chunk whose embed failed, because the selector has no arm for a chunk row that lacks a vector

## Summary

`backfill_chunk_vectors_with` (`src/librarian/indexer.rs`) selects an artifact for repair by two arms. The first arm, `NOT EXISTS (… artifact_chunk …)`, selects an artifact with no chunk rows. The second arm, `gap_arm`, selects an artifact with a non-blank chunk that has no `artifact_vec_v2` row. `gap_arm` is an empty string unless the target is `BackfillTarget::SqliteVec`. On a `BackfillTarget::Store` run, only the first arm exists.

An embed failure keeps its chunk row. Fix `56072827` made such a failure a per-chunk count instead of an abort. On a Store run the artifact then has chunk rows and no arm selects it. Every later run walks past it, and the chunk never gets a vector in the store that search reads.

## Class fit

This bug shows the trait of the class: a gate needs the event "this chunk has no vector that search can read", and it proxies that event with a value it can read, "this artifact has no chunk rows". On a Qdrant host the proxy is wrong for exactly the chunks that need repair, because the failed run has already created their rows. It differs from the class's other backfill members in one way: it is not wrong about the first run. It becomes wrong only after a run that partly failed.

## Symptom (Effect)

Not reproduced by a run. The mechanism follows from the code read at `56072827`.

After one run in which a chunk's embed failed, the CLI prints `embed_failed = N` and exits non-zero. A second run on the same Qdrant host prints `artifacts = 0`, `embedded = 0` and exits 0. The second report reads as a complete run, but the failed chunk still has no vector in the store. A `knn` search never returns that chunk.

## Reproduction

Not run. A test can use the helpers that exist in the `tests` module of `src/librarian/indexer.rs`:

1. Build the catalog and the artifact as `a_store_backfill_does_not_check_artifact_vec_v2s_dimension` does: `index_repo_sync` over a file whose text contains a marker, and `InMemoryArtifactStore::default()` from `crate::librarian::artifact_store::test_support`.
2. Run `backfill_chunk_vectors_with` with `backfill_rejecting_svc(<marker>)` and `BackfillTarget::Store { store: &store, project_id: &project_id }`. Expect `embed_failed == 1` and no vector in the store.
3. Run it again with `backfill_svc_at_dim(768)`.

Expected from the mechanism: the second run reports `artifacts == 0`, and `store.chunk(&chunk_id)` is still `None`. A fixed selector would embed the chunk in the second run.

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

N/A. The mechanism is read from the code, and no run has tested it.

## Fix

Unknown. Options:

- Add a coverage query to `ArtifactVectorStore`, for example "which of these chunk ids have a vector". The Store run then gets its own gap arm. This changes a trait with more than one implementation, and the Qdrant implementation needs a point lookup by id.
- Record each failed chunk durably, for example in a catalog table or a `meta` key, and select those artifacts again on a Store run. This needs no store change, but it adds state that can go stale when a later `reindex` embeds the chunk.
- Leave the limit and make the second run say so. The second report must not read as a complete run while `embed_failed` chunks of an earlier run remain. This is the smallest change, and it does not repair anything.

## Tests added

N/A. Not fixed.

## Workarounds

`librarian(action="reindex", reembed=true)` embeds every chunk again through `index_repo`, which writes to the store that search reads.

## Resume

Open. Choose the fix option first. The coverage query is the only option that repairs the chunk without extra state, and it needs a decision on the trait.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `gap_arm`)
- `src/librarian/artifact_store.rs` (`ArtifactVectorStore`)
- `docs/issues/archive/2026-10-14-backfill-restarts-at-the-first-page-so-one-input-the-embedder-always-rejects-blocks-every-later-page.md`
