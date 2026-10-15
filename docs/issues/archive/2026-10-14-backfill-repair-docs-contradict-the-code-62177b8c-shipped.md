---
id: 42b5078cd3b0ab02
kind: bug
status: fixed
title: 'BUG: backfill-repair docs contradict the code that 62177b8c shipped'
owners:
- marius
tags:
- cluster/doc-contradicted-by-code
topic: backfill-chunks docs
closed: 2026-10-15
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

Verified and committed on **experiments**.

- **SHA:** `934946e95e5f81c5091c80ae95d21c3624133612`
- **patch-id:** `361950b91e00766a6b4fbf03f5e19a63f0f2f120`

Comment and doc edits only, each checked against the code beside it first:

- `chunk.rs`, `replace_chunks`: the "do not filter" warning stays, and a new paragraph says why backfill's filter is safe. Backfill calls `embed_queue_items` only for an artifact with no stored chunks, so the filter drops nothing there. Its repair arm filters a queue rebuilt from stored rows, not this writer's queue.
- `rebuild_artifact_vec_v2_at_dim`: `ensure_artifact_vec_v2_dim` is its one caller (a grep for `rebuild_artifact_vec_v2_at_dim(` found one call site, at `indexer.rs:1003`), and both `write_embeddings_v2_with` and the backfill preflight reach it through that function.
- `BACKFILL_CURSOR`, the entry comment, the "Write, THEN" comment and the end-of-run comment: the marker is cleared at entry and at the end, and nothing reads it. The end-of-run comment now names what makes a run terminate: the in-memory `(cur_updated, cur_id)`.
- Test comment in `backfill_cursor_never_passes_vectors_waiting_for_the_final_flush`: the first page writes one vector and records its marker because every nonempty page flushes.
- `BackfillReport`: `missing_file`, `unreadable_encoding` and `unreadable_other` count only artifacts with no chunk rows, because only that arm reads the file. Only `missing_file` was named above. The other two docs had the same flaw, so I qualified all three.
- `ArtifactBackend::Qdrant`: one collection per project, named by `artifact_collection_name`; the shared `artifacts` collection is superseded.

## Tests added

None: the change is comments only, and a comment assertion would test the source text. The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4509 passed, default library 6825 passed, 0 failed.

## Workarounds

Trust the code over these comments.

## Resume

Complete. Fixed in `934946e9`, patch-id above.

The hazard the `chunk.rs` warning describes is real and stays documented: a repair that fills one missing vector keeps the other chunks' older vectors, so a reused chunk can keep a vector embedded under an older entry title until a content change or `reembed=true` replaces it.

## References

- `src/librarian/catalog/chunk.rs` (`replace_chunks`)
- `src/librarian/indexer.rs` (`rebuild_artifact_vec_v2_at_dim`, `BACKFILL_CURSOR`, `BackfillReport`, `backfill_chunk_vectors_with`)
- `src/librarian/artifact_store.rs` (`ArtifactBackend`)
