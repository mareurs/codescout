---
status: open
opened: 2026-10-18
closed:
severity: medium
owner: marius
related: [docs/issues/archive/2026-09-04-artifact-vector-delete-has-no-production-caller-so-every-archive-strands-its-vectors.md, docs/issues/2026-10-17-the-librarian-applies-no-read-deny-list-to-a-catalogued-outward-link.md]
tags:
- cluster/unclassified
kind: bug
unverified: 'Read, not run: no test drives librarian(action="reindex") against a store that holds vectors for a row whose file vanished. The CLI path (indexer::index_repo) does delete them and has a test.'
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

Not applied: out of the read-door fix's scope. Delete `report.removed_ids` from `ctx.artifact_store` in the tool,
as `index_repo` does, and add a tool-level test with an in-memory store.

## Tests added

N/A: filed, not fixed.

## Workarounds

`codescout reindex` (the CLI) deletes them.

## Resume

Add the delete and a test through `reindex::call` with `InMemoryArtifactStore`.

## References

- `src/librarian/tools/reindex.rs`, `src/librarian/indexer.rs` (`index_repo`)
