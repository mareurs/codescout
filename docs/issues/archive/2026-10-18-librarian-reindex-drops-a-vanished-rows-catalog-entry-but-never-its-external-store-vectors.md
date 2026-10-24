---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-18
owner: marius
related:
- docs/issues/archive/2026-09-04-artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md
- docs/issues/2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md
severity: medium
unverified: 'A real Qdrant `delete` and its transient-error behaviour are exercised by no test (the in-memory double matches Qdrant''s exact artifact-id selection by reading). A failed delete is reported but never retried: the vectors stay orphaned in the external store until removed by hand. `delete_orphan_repos` (the `orphan_removed` sweep) was not read for external-store vectors.'
---

# BUG: `librarian(reindex)` drops the catalog row of a vanished file but never deletes its vectors from an external store

## Summary

`indexer::index_repo_sync` reports the ids of rows it deleted in `IndexReport.removed_ids` so an async caller
can drop their vectors. The CLI path (`indexer::index_repo`, `src/librarian/indexer.rs`, the loop over
`report.removed_ids` calling `s.delete(id)`) does. The MCP tool `librarian(action="reindex")`
(`src/librarian/tools/reindex.rs` `call`) does not read `removed_ids` at all, so on a Qdrant-backed install the
points of every vanished file survive: answered by KNN, unresolvable at hydration.

## Class fit

A cleanup obligation carried in a report field that one of two callers never reads, so the store and the
catalog drift apart on the path the MCP server uses while the CLI path stays correct. No class names it.

## Symptom (Effect)

After a file is deleted and the project reindexed through the MCP tool, its chunk vectors stay in the Qdrant
collection. sqlite-vec is unaffected (the `artifact_chunk` cascade and `artifact_vec_v2_cascade_delete` take
them).

## Reproduction

Read, not run. `grep -n removed_ids src/librarian` finds it only in `indexer.rs`; `reindex.rs` never names it.

## Environment

Any install whose artifact backend is Qdrant.

## Root cause

The archived issue `2026-09-04-artifact-vector-delete-has-no-production-caller-...` added the delete to
`index_repo` (the CLI's caller). The MCP tool runs `index_repo_sync` and its own embed loop, and the delete was
never added there.

## Evidence

`src/librarian/indexer.rs` `index_repo`: `for id in &report.removed_ids { s.delete(id).await?; }`.
`src/librarian/tools/reindex.rs`: no reference to `removed_ids`. Found while fixing the read-door issue, where
the tool now deletes the vectors of reclaimed and kept ESCAPING rows only (`reclaimed_escaping_ids`,
`escaping_kept_ids`), deliberately not widened to every removed row, which is this issue.

## Hypotheses tried

N/A (read, not run).

## Fix

Fixed in `src/librarian/tools/reindex.rs`. The MCP reindex tool now deletes external-store vectors for every id in `report.removed_ids`, chained with `escaping_kept_ids` (`reclaimed_escaping_ids` is a subset of `removed_ids`). The delete is by exact artifact id: ids are fixed-length sha256 hex and a Qdrant point id is a UUIDv5 of the per-artifact chunk id, so an alias and its target never share a point. A failed delete no longer aborts the call (the catalog rows are already committed as deleted, so nothing would retry): failures are collected and reported as `vector_delete_error_count`, `vector_delete_errors` (sample of 20) and `vector_delete_note`, shaped like the `embed_*` fields. The CLI path (`indexer::index_repo`) is unchanged.

- Fix SHA (on `experiments`; merged 2026-10-24 by fast-forward, tip `e0255583b`): `dca0e573e0061225695d42176823f3852131c2f3`
- patch-id: `ce8308e6eb2c26ec0ea35eb9b9530bc5cf915f44`

An independent Opus review found no over-deletion; its one SHOULD-FIX (a failed delete aborting the reindex) is in the commit.

## Fix provenance

- **SHA:** `dca0e573e0061225695d42176823f3852131c2f3` (`experiments`)
- **patch-id:** `ce8308e6eb2c26ec0ea35eb9b9530bc5cf915f44`

## Tests added

In `src/librarian/tools/reindex.rs` (in-memory store, tempdirs, no Qdrant, no `~/.codescout`):

- `a_vanished_file_has_its_vectors_dropped_from_the_external_store` (survivor and a name-prefix sibling keep their vectors)
- `a_failed_vector_delete_is_reported_and_does_not_abort_the_reindex` (a store whose `delete` fails for chosen ids; two roots; red with the `?` put back)
- `dropping_a_symlink_alias_row_leaves_its_targets_vectors` (guards over-deletion; not red-before-fix)

Gate on the merged tree: exit 0.

## Workarounds

`codescout reindex` (the CLI) deletes them.

## Resume

Add the delete and a test through `reindex::call` with `InMemoryArtifactStore`.

## References

- `src/librarian/tools/reindex.rs`, `src/librarian/indexer.rs` (`index_repo`)
