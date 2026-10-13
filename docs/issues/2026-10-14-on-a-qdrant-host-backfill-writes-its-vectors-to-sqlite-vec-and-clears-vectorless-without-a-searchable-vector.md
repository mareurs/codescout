---
id: '314c53be087ef89c'
kind: bug
status: open
title: 'BUG: on a Qdrant host, backfill writes its vectors to sqlite-vec, so it clears vectorless while the searched store still lacks them'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector store
closed: ''
opened: 2026-10-14
related: []
severity: medium
---

# BUG: on a Qdrant host, backfill writes its vectors to sqlite-vec, so it clears `vectorless` while the searched store still lacks them

## Summary

`backfill_chunk_vectors_with` always writes through `write_embeddings_v2_with`, into `artifact_vec_v2`, whatever backend the process resolves. On a process that searches Qdrant, the no-chunk arm creates chunk rows (so `reindex` stops counting the artifact under `vectorless`) and writes vectors that search does not read. The report's `embedded` therefore counts vectors no reader uses. Older than `62177b8c`.

## Symptom (Effect)

Read from the code, not run. After such a backfill, `vectorless` drops to 0 for those artifacts, but their Qdrant points do not exist until a `reindex(reembed=true)` re-embeds the now-existing chunk rows.

## Reproduction

On a `server-stack` build resolving Qdrant, with an artifact that has no chunk rows: run `backfill-chunks`, then look up that artifact's chunk ids in the project's `artifact_chunks_*` collection.

## Environment

Qdrant-resolving process (the `cargo rb` default).

## Root cause

Backfill takes no `ArtifactVectorStore`; `index_repo` does (`store: Option<&dyn ArtifactVectorStore>`) and writes through `store.upsert` when one is configured.

Why the no-chunk arm still matters on Qdrant: `reembed=true` requeues EXISTING chunk rows only (`docs/issues/archive/2026-09-07-vectorless-note-prescribes-a-reembed-that-cannot-reach-it.md`), so creating chunk rows is the only escape from the no-chunk state. Refusing backfill on Qdrant would remove that escape.

## Evidence

Found while designing the fix for `2026-10-14-backfill-vector-gap-selector-reads-sqlite-vec-coverage-on-a-host-that-searches-qdrant.md` (session 1e54b0a7).

## Hypotheses tried

N/A.

## Fix

Unknown — under investigation. Option: give backfill the resolved store and write through `upsert`, as `index_repo` does; that needs a project id per artifact for Qdrant's per-project collections.

## Tests added

N/A — not fixed.

## Workarounds

After `backfill-chunks` on a Qdrant host, run `librarian(action="reindex", reembed=true)`.

## Resume

Open. Not in the current fix set.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `index_repo`)
- `src/librarian/mod.rs` (`build_tool_context_with`)
