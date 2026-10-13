---
id: '150e28d405754c68'
kind: bug
status: open
title: 'BUG: backfill''s vector-gap selector reads sqlite-vec coverage on a host that searches Qdrant, so it selects every chunked artifact'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: ''
opened: 2026-10-14
related: []
severity: high
---

# BUG: backfill's vector-gap selector reads sqlite-vec coverage on a host that searches Qdrant, so it selects every chunked artifact

## Summary

`62177b8c` (Albert R-01) made `backfill_chunk_vectors_with` select an artifact when any of its chunks has no `artifact_vec_v2` row. That table is the searched store only when the process resolves `ArtifactBackend::SqliteVec`. On a process that resolves Qdrant (the default on the `server-stack` build that `cargo rb` ships), `index_repo` writes through `ArtifactVectorStore::upsert` and leaves `artifact_vec_v2` empty, so the gap arm selects nearly every chunked artifact and re-embeds it into a table that search does not read.

## Symptom (Effect)

Measured 2026-10-14 on the live catalog (`~/.local/share/librarian/catalog.db`, read-only), project `/home/marius/work/claude/codescout`:

```
artifacts=2043 chunked_artifacts=2033 chunks=38531 chunks_with_sqlite_vec=43
old selector (no chunk rows)=10  new selector=2043
non-blank chunks the new run would embed=37151
```

The same project's Qdrant collection `artifact_chunks_codescout_dc6a871595179329` holds 48,223 points. Catalog-wide the selection grows from 76 to 4,255 artifacts. No backfill was run; these are selector populations computed with the selector's own predicates over the vec0 `_rowids` shadow table.

## Reproduction

On a `server-stack` build with no `CODESCOUT_ARTIFACT_BACKEND` and no `[librarian] vector_backend`, run `codescout backfill-chunks` in a project indexed through Qdrant. The report's `artifacts` approaches the number of chunked artifacts, and `embedded` approaches the number of chunks.

## Environment

Linux; `cargo rb` = `build --release --features server-stack` (`.cargo/config.toml`). `ArtifactBackend::resolve` returns Qdrant by default on that build.

## Root cause

- The page query (`src/librarian/indexer.rs`, `backfill_chunk_vectors_with`) ORs `EXISTS (chunk with NOT EXISTS artifact_vec_v2 row)` into the no-chunk arm with no backend condition.
- `src/cli/backfill_chunks.rs` never resolves the backend.
- `index_repo_sync`'s own `vectorless` comment states the hazard: counting `artifact_vec_v2` "would report EVERY artifact as vectorless whenever an external store is configured".

The store is a per-PROCESS choice, not a per-project one. Every project in this catalog resolves `default`, yet MRV-poc has 12,957 of 15,195 chunks in sqlite-vec and also a Qdrant collection, while codescout has 0. A coverage gap in `artifact_vec_v2` is evidence only for a process that searches `artifact_vec_v2`.

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), finding 1, verified with read-only scripts (`measure_backfill.py`, `backend_mix.py`) in that session's scratchpad.

## Hypotheses tried

1. The project has a per-project backend setting: refuted. No project in the catalog sets `vector_backend`; the store follows the build and env of the process that wrote it.

## Fix

Unknown — under investigation.

## Tests added

N/A — not fixed yet.

## Workarounds

On a Qdrant host, do not run `backfill-chunks` for vector gaps; `librarian(action="reindex", reembed=true)` re-embeds existing chunk rows into the configured store.

## Resume

Planned fix: pass the resolved `ArtifactBackend` into backfill and apply the gap arm only for `SqliteVec`; other backends keep the pre-`62177b8c` population (no chunk rows).

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `index_repo_sync` vectorless comment)
- `src/cli/backfill_chunks.rs` (`run`)
- `src/librarian/artifact_store.rs` (`ArtifactBackend::resolve`)
- `src/librarian/mod.rs` (`build_tool_context_with`)
- `docs/issues/archive/2026-10-13-backfill-retry-skips-existing-chunks-without-vectors.md`
