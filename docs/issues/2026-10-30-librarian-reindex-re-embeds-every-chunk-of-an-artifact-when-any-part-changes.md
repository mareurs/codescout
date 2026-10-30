---
id: '664ceb9fcb96230c'
kind: bug
status: open
title: Librarian reindex re-embeds every chunk of an artifact when any part of it changes
owners:
- marius
tags:
- cluster/attribute-derived-at-container-granularity
opened: 2026-10-30
severity: high
unverified: The saving (64 s to under 5 s for append edits) is estimated from the per-chunk embed cost, not measured.
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

## Fix

Plan: a per-chunk `embedded_input_hash` column (hash of the exact text sent to the embedder), set only after the upsert succeeds; queue only chunks whose hash differs; `reembed=true` ignores it. Needs a migration; `catalog-sql-hazards` applies (`migrate_v6` table copies must carry the column). If Qdrant loses points, the marker lies, so `reembed` is the escape.

## Tests added

None yet.

## Workarounds

None.

## Resume

Read `embed_queue_from_chunks` (`src/librarian/indexer.rs:239-290`) and the stamp loop in `src/librarian/tools/reindex.rs:583-597`; design the column and migration.

## References

- `docs/issues/2026-10-30-librarian-reindex-writes-each-chunk-row-as-its-own-fsynced-commit.md`
