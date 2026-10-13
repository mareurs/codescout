---
id: '288de7d32191d02b'
kind: bug
status: open
title: 'BUG: backfill-repair docs contradict the code that 62177b8c shipped'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
topic: backfill-chunks docs
closed: ''
opened: 2026-10-14
related: []
severity: low
---

# BUG: backfill-repair docs contradict the code that 62177b8c shipped

## Summary

`62177b8c` changed the backfill selector, the cursor's role and the rebuild call path, but several comments still describe the old code. One of them, in `chunk.rs`, forbids exactly the filter that the new repair path applies.

## Symptom (Effect)

Each statement below is false against `62177b8c`, read 2026-10-14:

- `src/librarian/catalog/chunk.rs`, `replace_chunks` doc: "**So do not filter that queue against chunks that already hold a vector.**" The repair path's `items.retain(|item| missing.contains(&item.chunk_id))` does that filter. The same commit edited this paragraph and kept the warning.
- `rebuild_artifact_vec_v2_at_dim`: "Only called from `write_embeddings_v2_with`". The backfill preflight now reaches it through `ensure_artifact_vec_v2_dim`.
- `BACKFILL_CURSOR`: "the resume cursor". Production code writes it and never reads it.
- The end-of-run comment in `backfill_chunk_vectors_with` argues from "the next run's `updated_at > cursor` predicate", which no longer exists.
- Test `backfill_cursor_never_passes_vectors_waiting_for_the_final_flush`: "its one vector sits below the flush threshold". Every nonempty page now flushes.
- `BackfillReport::missing_file`: "Artifacts whose file is genuinely absent". An absent file with stored chunks is now repaired from the stored rows and not counted.

Also found, older than `62177b8c`: `ArtifactBackend::Qdrant` says "a shared Qdrant `artifacts` collection". Chunk vectors live in per-project `artifact_chunks_<base>_<hash>` collections; `build_tool_context_with` says the shared collection is "superseded rather than migrated". (That stale name led the reviewing session to count the wrong collection once.)

## Reproduction

Read each cited comment against the code beside it.

## Environment

N/A.

## Root cause

The comments were true for the pre-`62177b8c` code and were not revisited when the selector and cursor changed.

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), findings 5, 7 and 13.

## Hypotheses tried

N/A.

## Fix

Unknown — under investigation.

## Tests added

N/A — not fixed.

## Workarounds

Trust the code over these comments.

## Resume

Open. The `chunk.rs` warning matters most: it describes a real hazard (a reused chunk keeping an old-title vector) that the repair path's filter can now produce until the next reindex.

## References

- `src/librarian/catalog/chunk.rs` (`replace_chunks`)
- `src/librarian/indexer.rs` (`rebuild_artifact_vec_v2_at_dim`, `BACKFILL_CURSOR`, `BackfillReport`, `backfill_chunk_vectors_with`)
- `src/librarian/artifact_store.rs` (`ArtifactBackend`)
