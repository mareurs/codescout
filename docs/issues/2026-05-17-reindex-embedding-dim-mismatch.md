---
status: mitigated
opened: 2026-05-17
closed: 2026-05-17
severity: high
owner: marius
related: []
tags: ["librarian", "reindex", "embedding", "dim-mismatch", "vec0"]
---

# BUG: `librarian(reindex, force=true)` failed with embedding dimension mismatch (768 → 1)

## Summary

`librarian(reindex, force=true)` returned `Dimension mismatch for inserted vector for the "embedding" column. Expected 768 dimensions but received 1.` The embedding pipeline produced a 1-element vector instead of a 768-element one — likely an error sentinel that the writer did not gate against. Defensive validation layer fixed in commit `d482ca8a` (2026-05-17): `write_embeddings` now validates dim consistency before any INSERT. Status `mitigated` (not `fixed`) because the upstream — why the embedder returns 1-elem vectors — is still pending.

## Symptom (Effect)

```
librarian(action="reindex", scope="project", force=true)
→ Err: "Dimension mismatch for inserted vector for the \"embedding\" column.
        Expected 768 dimensions but received 1."
```

Workspace status confirmed `embeddings_model: jina-embeddings-v2-base-code` (768-dim).

## Reproduction

Pre-fix: any condition that caused the embedder to return a sentinel 1-element vector (likely an error path that returned `vec![0.0]` instead of propagating the error). Library indexing (0/62 indexed per `workspace(status)`) was blocked by the same code path.

## Environment

- Date observed: 2026-05-17
- Tool: `mcp__codescout__librarian(action="reindex", force=true)`
- Component: `src/librarian/indexer.rs::write_embeddings`
- Embedder: `jina-embeddings-v2-base-code` (768-dim) via `codescout-embed`

## Root cause

Two layers:

1. **Defensive layer (now fixed):** `write_embeddings` did not validate that the batch's vector lengths matched the expected dim before issuing the vec0 INSERT. The error fired at the SQL layer rather than at the validation layer — yielding a misleading mid-pipeline diagnostic.
2. **Upstream (still pending):** the embedder occasionally returns 1-element vectors, presumably an error sentinel (e.g. `vec![0.0]` returned when the embedder fails). The error was swallowed and a fallback short vector was emitted instead of bubbling up.

## Evidence

- Reproduced live on this project's catalog — see session log `docs/trackers/artifact-code-linkage-session-log.md` F-6.
- Post-fix: validation path verified by the unit test suite (2329 passed). The original triggering condition was not reproduced live today, but the validation path now fires before any DELETE with a diagnostic naming likely causes.

## Hypotheses tried

1. **Hypothesis:** Embedder hits an error condition and returns `vec![0.0]` (or similar 1-element fallback) without bubbling the error up. **Test:** Inspect codescout-embed crate's `embed_batch` path. **Verdict:** Deferred — defensive layer fixed first to stop the data loss (see #7 — cascade-delete). **Evidence link:** see Fix.

## Fix

**Defensive validation layer (fixed in `d482ca8a`, 2026-05-17):** `write_embeddings` now validates dim consistency before any INSERT:

1. Non-empty batch check.
2. All batch vectors share same length > 0.
3. Batch length matches existing `artifact_vec` row blob length (if any).

If any check fails, the error fires before any DELETE — combined with the cascade-delete fix (#7), this prevents data loss.

**Upstream (still pending):** find why the embedder returns 1-elem vectors. Investigation to land in `codescout-embed` crate.

## Tests added

Unit test coverage for the validation path is exercised by the 2329-test suite. Specific test names not enumerated in the commit; recommend `write_embeddings_rejects_dim_mismatch_before_insert`.

## Workarounds

Pre-fix: no workaround — library indexing (0/62 indexed) was blocked by the same code path. Post-fix: validation rejects bad input early; the upstream embedder bug remains to be diagnosed.

## Resume

Concrete next action: in `codescout-embed`, trace the `embed_batch` (or equivalent) function for the jina-embeddings-v2-base-code backend. Find paths that return `vec![0.0]` / short vectors and replace with proper `Result::Err` propagation. Repro: trigger a network failure or a malformed input during embedding; assert the call returns `Err` not a 1-elem `Ok`.

## References

- Originally tracked as **#6** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Session log: `docs/trackers/artifact-code-linkage-session-log.md` F-6.
- Defensive fix commit: `d482ca8a` on `experiments`.
- Related: bug-tracker.md #5 (UNIQUE constraint), #7 (cascade-delete data loss) — same commit fixes all three reindex failure modes.
- Upstream investigation: open in `codescout-embed` crate.
