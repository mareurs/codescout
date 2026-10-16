---
id: 271253d651b1ebe2
kind: bug
status: fixed
title: 'BUG: backfill''s vector-gap selector reads sqlite-vec coverage on a host that searches Qdrant, so it selects every chunked artifact'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector-gap selection
closed: 2026-10-14
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

The store is a per-PROCESS choice, not a per-project one. Every project in this catalog resolves `default`, yet MRV-poc has 12,957 of 15,195 chunks in sqlite-vec and also a Qdrant collection, while codescout has 43 of 38,531 (the Symptom block's `chunks_with_sqlite_vec`). A coverage gap in `artifact_vec_v2` is evidence only for a process that searches `artifact_vec_v2`.

## Evidence

Review of `62177b8c~1..6cf6d6e8` (session 1e54b0a7), finding 1, verified with read-only scripts (`measure_backfill.py`, `backend_mix.py`) in that session's scratchpad.

## Hypotheses tried

1. The project has a per-project backend setting: refuted. No project in the catalog sets `vector_backend`; the store follows the build and env of the process that wrote it.

## Fix

Verified and committed on **experiments**.

- **SHA:** `8c02ad779926ec33f51ab436e81fe7f056a7fc9d`
- **patch-id:** `00eabb22f09011ada6735a64186700340a27d39b`

`backfill_chunk_vectors` takes the resolved `ArtifactBackend` and applies the vector-gap arm only for `SqliteVec`. The no-chunk arm stays on every backend, because `reembed=true` reaches existing chunk rows only. The CLI resolves the backend in `backfill_backend()` from the same project input as `build_tool_context_with`; `--all` takes the host setting, as the CLI reindex does. `ArtifactBackend::resolve_with` takes the env value as an argument, so the decision is tested without process env.

Known limit: under `--all`, a project whose `project.toml` sets a backend different from the host's is resolved by the host setting. No project in this catalog sets one. Backfill on a Qdrant host still writes its no-chunk vectors to `artifact_vec_v2`; that older defect is `docs/issues/archive/2026-10-14-on-a-qdrant-host-backfill-writes-its-vectors-to-sqlite-vec-and-clears-vectorless-without-a-searchable-vector.md`, fixed in `934946e95e5f81c5091c80ae95d21c3624133612`.

## Tests added

- `a_qdrant_process_backfills_only_artifacts_with_no_chunk_rows` (`src/librarian/indexer.rs`): observed RED first, the Qdrant run reported `artifacts: 2` for 1 expected. A `SqliteVec` control run on the same fixture repairs the gap, so the Qdrant result is not vacuous.
- `a_scoped_run_takes_its_projects_backend_and_all_takes_the_hosts` (`src/cli/backfill_chunks.rs`): two projects with opposite settings, so it discriminates on the lean and the server-stack build.

Applied mutations with `scripts/mutation-probe.sh` in an isolated worktree: gap arm on for every backend (killed by the first test alone), gap arm off for every backend (killed by 13 tests), CLI ignores `--all` and CLI ignores the env var (each killed by the CLI test). The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4479 passed, default library 6776 passed, 0 failed.

## Workarounds

On a Qdrant host, do not run `backfill-chunks` for vector gaps; `librarian(action="reindex", reembed=true)` re-embeds existing chunk rows into the configured store.

## Resume

Complete. Fixed in `8c02ad77`, patch-id above.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `index_repo_sync` vectorless comment)
- `src/cli/backfill_chunks.rs` (`run`)
- `src/librarian/artifact_store.rs` (`ArtifactBackend::resolve`)
- `src/librarian/mod.rs` (`build_tool_context_with`)
- `docs/issues/archive/2026-10-13-backfill-retry-skips-existing-chunks-without-vectors.md`
