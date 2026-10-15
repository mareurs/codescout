---
id: 305a710b6f4a2613
kind: bug
status: fixed
title: 'BUG: on a Qdrant host, backfill writes its vectors to sqlite-vec, so it clears vectorless while the searched store still lacks them'
owners:
- marius
tags:
- cluster/gate-keyed-on-unobservable-event
topic: backfill-chunks vector store
closed: 2026-10-15
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

Found while designing the fix for `docs/issues/archive/2026-10-14-backfill-vector-gap-selector-reads-sqlite-vec-coverage-on-a-host-that-searches-qdrant.md` (session 1e54b0a7).

## Hypotheses tried

N/A.

## Fix

Verified and committed on **experiments**.

- **SHA:** `934946e95e5f81c5091c80ae95d21c3624133612`
- **patch-id:** `361950b91e00766a6b4fbf03f5e19a63f0f2f120`

`backfill_chunk_vectors` takes a `BackfillTarget` (sqlite-vec, or a store with a `project_id`). A store run writes through `ArtifactVectorStore::upsert` under the scoped project's `abs_path`, the id that `find` (`find.rs`) and `reindex` (`reindex.rs`) use, so the vectors land in the collection its search reads. A store run does not check or migrate `artifact_vec_v2`, because it never writes that table.

The CLI builds the target in `backfill_target`. Two Qdrant runs are refused, because no collection is right for them: `--all` (a catalog-wide walk has no single project) and a process whose context holds no store (Qdrant unreachable, or a build without `server-stack`). The refusal names the way out.

Known limits:

- The Qdrant path was tested against `InMemoryArtifactStore`, not a live Qdrant. `QdrantArtifactStore::upsert` is unchanged and is the call `reindex` already makes.
- `ArtifactVectorStore` has no coverage query, so a Qdrant run still selects only artifacts with no chunk rows. A chunk row that exists without a Qdrant point is repaired by `librarian(action="reindex", reembed=true)`, not by backfill.

## Tests added

- `a_store_backfill_writes_its_vectors_to_the_store_and_not_to_artifact_vec_v2` (`src/librarian/indexer.rs`): observed RED first (0 vectors in the store, 1 expected). It asserts the store holds every chunk's vector, that they are filed under the scope's `project_id`, and that `artifact_vec_v2` holds none.
- `a_store_backfill_does_not_check_artifact_vec_v2s_dimension`: observed RED first (the stale-table refusal).
- `a_catalog_wide_backfill_on_qdrant_is_refused`, `a_qdrant_backfill_with_no_reachable_store_is_refused` and `a_scoped_qdrant_backfill_targets_the_projects_collection_and_sqlite_vec_its_table` (`src/cli/backfill_chunks.rs`): the first two observed RED first.
- `a_qdrant_process_backfills_only_artifacts_with_no_chunk_rows` now runs against a store, and its sqlite-vec control run covers both artifacts.

Mutations with `scripts/mutation-probe.sh`, all killed: the store write dropped, vectors filed under the artifact id instead of the project, the dimension check applied to a store run, `--all` allowed on Qdrant, no store falling back to sqlite-vec, and the project id dropped in the CLI. The full ordered `./scripts/gate.sh --check` was green on the committed bytes: lean library 4509 passed, default library 6825 passed, 0 failed.

## Workarounds

None needed for a project-scoped run. For a chunk row that has no Qdrant point, run `librarian(action="reindex", reembed=true)`.

## Resume

Complete. Fixed in `934946e9`, patch-id above.

## References

- `src/librarian/indexer.rs` (`backfill_chunk_vectors_with`, `index_repo`)
- `src/librarian/mod.rs` (`build_tool_context_with`)
