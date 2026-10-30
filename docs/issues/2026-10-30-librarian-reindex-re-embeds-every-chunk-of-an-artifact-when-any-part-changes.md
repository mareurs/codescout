---
id: '664ceb9fcb96230c'
kind: bug
status: fixed
title: Librarian reindex re-embeds every chunk of an artifact when any part of it changes
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
closed: 2026-10-30
opened: 2026-10-30
severity: high
unverified: The saving is shown by tests that count embeds (an append embeds 1 chunk, a rename 2, reembed all), not yet by a run on the real 482 MB catalog. The migration (v14) has not run on that catalog.
---

# BUG: Librarian reindex re-embeds every chunk of an artifact when any part of it changes

## Summary

`replace_chunks` (`src/librarian/catalog/chunk.rs:413-600`) keeps the `chunk_id` of unchanged chunks, but `embed_queue_items` (`src/librarian/indexer.rs:231-235`) passes every stored row to `embed_queue_from_chunks`, so every chunk is queued. The `embedded_sha256` stamp is per artifact. Appending one entry to a 499-chunk tracker re-embeds all 499 chunks.

## Class fit

`cluster/attribute-derived-at-container-granularity`: the "already embedded" fact is stamped on the artifact (the container), while the work it gates is per chunk. It differs from the class's other members in that the cost is time, not a wrong value.

## Symptom (Effect)

usage.db, `librarian action=reindex`: 26 calls, avg 30 s, max 127 s. Latency follows chunks embedded, not files changed:

```
 99 s   3,733 chunks   2 files changed
127 s   2,505 chunks  25 files changed
 64 s   1,375 chunks   7 files changed
 32 s     938 chunks   0 files changed
1.4-3.1 s   0 chunks
```

Pending on 2026-10-30: 36 artifacts / 1,463 chunks with a stale stamp, including `reconnaissance-patterns.md` (499) and `prompt-hamsa-audit-log.md` (223).

## Reproduction

Append one entry to a large tracker, then `librarian(action="reindex")`; read the embedded-chunk count in the response.

## Environment

Catalog `~/.local/share/librarian/catalog.db` (482 MB, 127,305 `artifact_chunk` rows); embedder llama-server at `127.0.0.1:48081` (~21 ms per single embed, measured).

## Root cause

- `src/librarian/indexer.rs:231-235`: `replace_chunks` → `embed_queue_from_chunks(id, title, stored, &titles)` with all stored rows.
- measured 2026-10-30 (librarian explorer of this session): usage.db reindex rows above; 32 single embeds 667 ms.

## Evidence

See Symptom. Code spot-checked 2026-10-30 at `src/librarian/indexer.rs:218-236`.

## Hypotheses tried

1. **Hypothesis:** batching embeds would help. **Test:** 32 single requests vs one batch of 32. **Verdict:** rejected — 667 ms vs 866 ms.
2. **Measured 2026-10-30, after the batched-write fix** (`docs/issues/2026-10-30-the-mcp-reindex-wrote-each-embedded-chunk-to-qdrant-as-its-own-durable-request.md`): the local embedder serializes requests (17-20 ms per 2 KB text sequentially; 29-32 ms at 8 and 16 workers), so embedding is the floor at roughly 16-18 ms per chunk. A reindex of 2,184 chunks now takes 34.3 s, about 15.7 ms per chunk. Skipping chunks whose embedded text did not change is therefore now the only large lever left: appending one entry to a 499-chunk tracker is still ~8 s of embedding for what should be one chunk.

## Fix

Done 2026-10-30. A nullable `artifact_chunk.embedded_input_hash` column (migration v14) holds a hash of the exact embed input: the artifact title and the composed chunk text (`chunk::embed_input_hash`). It is not `content_hash`, which misses the entry title; `replace_chunks` documents why.

- `embed_queue_items` marks each queue item `up_to_date` when the stored marker equals its current input hash.
- The MCP reindex drain counts an `up_to_date` item as landed without embedding it, unless `reembed=true`. The per-artifact stamp logic is unchanged, so an artifact whose chunks were all skipped still gets stamped.
- The drain writes the markers after the vectors landed, in one commit. A failed upsert leaves its chunk unmarked.
- A marker is cleared wherever a vector is removed or rewritten outside the drain: `delete_chunk_vectors`, `write_embeddings_v2_with`, `rebuild_artifact_vec_v2_at_dim`.
- The response gains `embed_skipped_chunks`.

Limits, stated plainly:
- The embedder exposes no model identity, so a model or backend switch needs `reembed=true`, its documented lever.
- A store that loses points leaves markers that lie (Qdrant has no foreign key to the catalog). `reembed=true` is the repair.
- The CLI `index_repo` and the backfill ignore the flag and do not write markers. That under-claims and is safe.
- An older binary on the shared catalog ignores the column. Its vector writes do not clear markers.

## Tests added

In `src/librarian/tools/reindex.rs`: `appending_an_entry_embeds_only_the_chunk_that_is_new` (embeds 1, skips the rest, and a third run queues nothing), `renaming_an_entry_re_embeds_its_mid_entry_chunks`, `reembed_ignores_the_markers`, `a_chunk_whose_upsert_failed_is_not_marked_embedded`. In `indexer.rs`: `a_marker_does_not_outlive_the_vector_it_vouched_for`. In `catalog/mod.rs`: `v14_adds_a_nullable_embedded_input_hash_and_is_idempotent`; the eight version assertions moved from 13 to 14.

Mutations run: hashing the chunk body without the entry title fails the rename test; marking a chunk whose upsert failed fails the failed-upsert test. Both reverted.

## Workarounds

None.

## Resume

Read `embed_queue_from_chunks` (`src/librarian/indexer.rs:239-290`) and the stamp loop in `src/librarian/tools/reindex.rs:583-597`; design the column and migration.

## References

- `docs/issues/2026-10-30-librarian-reindex-writes-each-chunk-row-as-its-own-fsynced-commit.md`
