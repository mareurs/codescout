---
id: '9ba74ea94d26f530'
kind: bug
status: taken
title: 'BUG: SQLite artifact KNN rejects Librarian context widening beyond 4096 neighbors'
tags:
- cluster/unclassified
- librarian
- sqlite-vec
claimed_by: 01a13475-9d76-7942-81ce-e2a4457bf033
closed: null
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

External report: codescout-maintainer-audit-2026-10-16.md, U-02. Local logs are under `.codescout/resident-private/albert-audit-20261016/`. Initial tests reproduced exact8000/4096errors. Independent review then demonstrated4096 nearby chunks hiding one farther indexed artifact with candidates_capped=false. The corrected regression genuinely failed on that false flag, then passed for both4081 uncapped and4096near+1far capped cases. SQLite final focused suite:54passed,1pre-existingignored. Independent original snapshot mutations:4killed,0survived,0inconclusive. Fresh full gate/focused follow-up review pending.

## Hypotheses tried
1. Generic KNN widening exceeds the SQLite backend bound — real-store and actual-context regressions pending.

## Fix

The SQLite adapter bounds direct requests to its declared max_knn_k=4096. The coordinator bounds initial requests by the backend ceiling, preserving larger initial requests on default/Qdrant stores; retry queries use min(generic8000, backend bound). SemanticPage.candidates_capped distinguishes a full K budget with a short artifact page from ordinary corpus exhaustion; context forwards that flag through its existing overflow metadata and hint. Fix SHA and patch-id will be recorded after verified integration.

## Tests added
`sqlite_knn_bounds_large_requests_without_losing_nearest_chunks`; `sqlite_topic_context_survives_knn_widening_past_backend_limit`.

## Workarounds
Use artifact-anchored context when the relevant artifact ID is known.

## Resume

Focused independent review approved the corrected snapshot. The final frozen-source ordered gate is running. Commit verified repairs, integrate experiments, reconcile catalog records, and record SHA plus patch-id.

## References
Albert audit U-02; src/librarian/artifact_store.rs; src/librarian/tools/context.rs.
