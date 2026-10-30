---
id: '1b8543744f80be39'
kind: bug
status: fixed
title: The MCP reindex wrote each embedded chunk to Qdrant as its own durable request, ~25 ms of every ~43 ms spent per chunk
tags:
- cluster/unclassified
closed: 2026-10-30
severity: high
unverified: The end-to-end figure is one reindex of 2,184 chunks compared with a logged linear model (R-squared 0.85), not with the old binary on the same work. The backfill and index_repo loops still write one point per request.
---

# BUG: The MCP reindex wrote each embedded chunk to Qdrant as its own durable request, ~25 ms of every ~43 ms spent per chunk

## Summary

`librarian(action="reindex")` (`src/librarian/tools/reindex.rs`, the embed loop in `call`) embedded one chunk, then called `store.upsert` for that chunk alone, then moved to the next. The Qdrant store sends each `upsert` as one `wait(true)` request, and each such request waits for a WAL sync. The write was therefore paid per chunk.

## Class fit

`cluster/unclassified`. The case shows a fixed per-request cost (a durable sync) paid once per item because the interface took one item, so the per-item time was mostly that fixed cost and not the work.

## Symptom (Effect)

usage.db, 26 `librarian action=reindex` calls: `latency_ms = 582 + 34.3 x embedded`, R-squared 0.85, against R-squared 0.32 for the number of changed artifacts. A reindex of 2 changed files that embedded 3,733 chunks took 99 s.

## Reproduction

`librarian(action="reindex")` after changing a few documents; read `embedded` and the call time.

## Environment

Local llama-server embedder at `127.0.0.1:48081`, Qdrant 1.17.0 on localhost, collection `artifact_chunks_codescout_*` (768-d cosine, keyword indexes on `artifact_id` and `project_id`).

## Root cause

Measured 2026-10-30, not read from code:

- The embedder does not scale with concurrency: 64 real 2 KB texts took 17-20 ms per text sequentially, 18.6 ms with 4 workers and 29-32 ms with 8 and 16 workers. So more embed concurrency would not have helped.
- Qdrant, on a scratch collection shaped like the live ones: 64 single `wait=true` upserts took 24.7 ms per point; 8 concurrent singles 8.0 ms per point; one batch of 16 took 1.34 ms per point; one batch of 64 took 0.66 ms per point.
- So about 18 ms of embedding plus about 25 ms of write per chunk, one at a time.

## Evidence

The two micro-benchmarks above (scratch scripts, collection deleted afterwards) and the usage.db fit.

## Hypotheses tried

1. **Hypothesis:** raise the embed concurrency, as `index_repo` does with `buffer_unordered(8)`. **Verdict:** rejected by measurement; the local server serializes requests.
2. **Hypothesis:** the 13-16 ms per chunk beyond the embed time is the write. **Verdict:** confirmed.

## Fix

Done 2026-10-30:
- `ArtifactVectorStore::upsert_many` (`src/librarian/artifact_store.rs`) with a default that loops `upsert`, so the sqlite-vec store and every test double are unchanged; the Qdrant store overrides it with one `upsert_points` call (`QdrantWrap::artifact_upsert_points`, `src/retrieval/artifact.rs`).
- The reindex loop buffers `UPSERT_BATCH` (64) embedded vectors and writes them in one request (`flush_vectors`). If the store refuses a batch, the chunks are retried one by one, so only a chunk that cannot be written is reported and its artifact stays unstamped, as before.

Measured end to end, release build, real catalog: one reindex that embedded 2,184 chunks (6 updated artifacts) took **34.3 s**; the logged model predicts 75.5 s for that count, so about 2.2 times faster (15.7 ms per chunk). The next two reindexes took 2.7 s and then reported 0 updated, 0 embedded, 0 vectorless, 0 errors.

Not done: the `backfill` loop (`indexer.rs`) and `index_repo` still write one point per request; and the larger cost, re-embedding every chunk of a changed artifact, is the open issue `664ceb9fcb96230c`.

## Tests added

`reindex::tests::a_reindex_writes_vectors_in_batches_not_one_request_per_chunk` (70 files: batches of 64 then the remainder, no single writes, each chunk once; red when the batch size is forced to 1, verified by mutation), `a_refused_batch_is_retried_singly_and_only_the_bad_chunk_is_reported`, and the `#[ignore]` live `qdrant_artifact_upsert_points_lands_a_batch_and_is_idempotent_on_chunk_id`.

## Workarounds

None needed after the fix.

## Resume

Apply `upsert_many` to the `backfill_chunk_vectors_with` and `index_repo` loops, then the per-chunk skip in `664ceb9fcb96230c`.

## References

- `docs/issues/2026-10-30-librarian-reindex-re-embeds-every-chunk-of-an-artifact-when-any-part-changes.md`
- `docs/issues/2026-10-30-librarian-reindex-writes-each-chunk-row-as-its-own-fsynced-commit.md`
