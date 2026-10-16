---
id: d4925a8421012c00
kind: bug
status: fixed
title: 'BUG: SQLite artifact KNN rejects Librarian context widening beyond 4096 neighbors'
tags:
- cluster/unclassified
- librarian
- sqlite-vec
closed: 2026-10-16
opened: 2026-10-16
owner: marius
related: []
severity: medium
---

# BUG: SQLite artifact KNN rejects Librarian context widening beyond 4096 neighbors

## Summary
Albert reports a real topic-context call failing with `k value in knn query too large, provided 8000 and the limit is 4096`. The generic semantic coordinator widens to 8000; the SQLite adapter passes that request directly to sqlite-vec.

## Class fit
Unclassified: this is an incompatible numerical limit at a backend boundary. It is not a hidden partial result or an ignored parameter; the actual query fails rather than returning a deceptively complete answer.

## Symptom (Effect)
Topic context fails while document lookup and artifact-anchored context still work.

## Reproduction

Measured 2026-10-16 against real sqlite-vec: direct k=8000 and actual topic context with 4081 chunks of one artifact both failed with the exact reported 8000/4096 error. After bounding the SQLite adapter, `./scripts/with-slot.sh cargo test --lib sqlite_` passed 54 tests, with one pre-existing ignored test. The context fixture consumes stored IDs returned by replace_chunks; an initial draft-ID fixture error was corrected before the final run.

## Environment
Linux x86_64; current committed experiments 0007d926 plus backend history a6ec001c in an isolated worktree. Real sqlite-vec in-memory catalog; no private data.

## Root cause

Measured 2026-10-16: semantic_find widens through 255, 510, 1020, 2040, 4080 and 8000 when many chunk candidates collapse to one artifact. SqliteVecArtifactStore::knn formerly passed that k straight into SQL; sqlite-vec refuses values above 4096. The SQLite adapter now bounds requests before SQL. A short result can exhaust backend search budget rather than the entire corpus; the store/coordinator comments state that limit.

## Evidence

Albert maintainer audit U-02 reported the live8000/4096error. Local real-SQLite regressions reproduced it in both direct KNN and topic context. Independent review then demonstrated4096 nearby chunks hiding one farther indexed artifact while candidates_capped=false. The new regression failed on that false flag and passed after the repair; its4081-chunk control remains uncapped.

Local logs: `.codescout/resident-private/albert-audit-20261016/`. Corrected final SQLite suite54passed/1pre-existingignored. Original-snapshot review applied4mutants:4killed/0surviving/0inconclusive. Disclosure follow-up applied3mutants:3killed/0surviving/0inconclusive; those ran before the final initial-request/comment cleanup, followed by a fresh corrected-source54-pass baseline. Final review approved with no remaining findings. Final frozen full gate exited0: FMT0, CLIPPY0, LEAN0, DEFAULT0, PYTHON_LIGHT0; default library6865passed/9ignored. These are separate overlapping configurations, not a unique-test total. No native-client, live-Qdrant or power-loss acceptance claim is made. The precommit dead-artifact-ID helper skipped because its configured lookup found no runnable binary; catalog verification uses MCP separately.

## Hypotheses tried

1. Generic KNN widening exceeds the SQLite backend bound — confirmed by the real-store and actual-context RED runs with the exact8000/4096error; both regressions passed after repair.
2. Bounding SQL alone is sufficient — rejected by independent review's farther-artifact reproduction: the call succeeded while falsely reporting uncapped discovery. The classification/forwarding fix then passed the capped and smaller-corpus controls.

## Fix

The SQLite adapter bounds direct requests to max_knn_k=4096. The coordinator bounds initial requests only by the backend ceiling, preserving larger initial requests on default/Qdrant stores; retry queries use min(generic8000, backend bound). SemanticPage.candidates_capped distinguishes a full K budget with a short artifact page from ordinary corpus exhaustion; context forwards that flag through its existing overflow metadata and hint. Exact commit identifiers are recorded in Fix provenance below.

## Fix provenance

- **SHA:** `f5f1c6f7d2260515388437a8c2d5bfb5ad7c02a4` (`experiments`)
- **patch-id:** `e351d6450750044cf1b0169d83f911bacdc0ad13`

## Tests added
`sqlite_knn_bounds_large_requests_without_losing_nearest_chunks`; `sqlite_topic_context_survives_knn_widening_past_backend_limit`.

## Workarounds
Use artifact-anchored context when the relevant artifact ID is known.

## Resume

N/A — fixed and verified on experiments. Build/restart and Albert's native acceptance remain separate operator actions.

## References
Albert audit U-02; src/librarian/artifact_store.rs; src/librarian/tools/context.rs.
