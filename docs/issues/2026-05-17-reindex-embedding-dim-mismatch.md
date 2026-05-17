---
status: fixed
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

Two layers — both now fixed.

1. **Defensive layer (fixed in `d482ca8a`, 2026-05-17):** `write_embeddings` did not validate that the batch's vector lengths matched the expected dim before issuing the vec0 INSERT. The error fired at the SQL layer rather than at the validation layer — yielding a misleading mid-pipeline diagnostic.

2. **Upstream (fixed 2026-05-17):** Two paths in `RemoteEmbedder::embed` (`crates/codescout-embed/src/remote.rs`) silently produced 1-element vectors:
   - **All-empty batch early return** (`non_empty.is_empty()` branch) returned `Ok(vec![vec![0.0; 1]; texts.len()])` — a 1-element sentinel for every input slot. Intent was "server would 400 on all-empty, return placeholders" but the placeholders didn't match the model's real dim and silently corrupted the vec0 INSERT downstream.
   - **Defensive `map_or(1, ...)` fallback** for the post-loop dim computation: `let dim = embedded.first().map_or(1, |e| e.len());` defaulted dim to 1 when `embedded` was empty (server returned 200 with no data, etc.).

   Both now bail with explicit error messages: the all-empty case names the count ("cannot embed batch — all N text(s) are empty/whitespace"), and the no-data case surfaces "embedding server returned no data and no cached dimensions are available".
## Evidence

- Reproduced live on this project's catalog — see session log `docs/trackers/archive/artifact-code-linkage-session-log.md` F-6.
- Post-fix: validation path verified by the unit test suite (2329 passed). The original triggering condition was not reproduced live today, but the validation path now fires before any DELETE with a diagnostic naming likely causes.

## Hypotheses tried

1. **Hypothesis:** Embedder hits an error condition and returns `vec![0.0]` (or similar 1-element fallback) without bubbling the error up. **Test:** Inspect codescout-embed crate's `embed_batch` path. **Verdict:** Deferred — defensive layer fixed first to stop the data loss (see #7 — cascade-delete). **Evidence link:** see Fix.

## Fix

**Defensive validation layer (fixed in `d482ca8a`, 2026-05-17):** `write_embeddings` now validates dim consistency before any INSERT:

1. Non-empty batch check.
2. All batch vectors share same length > 0.
3. Batch length matches existing `artifact_vec` row blob length (if any).

If any check fails, the error fires before any DELETE — combined with the cascade-delete fix (#7), this prevents data loss.

**Upstream (fixed 2026-05-17):** `RemoteEmbedder::embed` in `crates/codescout-embed/src/remote.rs` now bails on both unsafe paths instead of returning sentinel 1-element vectors. See Root cause for the two specific branches changed. Regression test `embed_returns_err_when_all_inputs_empty` pins the all-empty-batch case.
## Tests added

`crates/codescout-embed/src/remote.rs::tests::embed_returns_err_when_all_inputs_empty` — constructs a RemoteEmbedder (no network), calls `embed(&["", "  ", "\t\n"])`, asserts the returned error message contains the input count. The pre-fix path would have silently returned 3×1-elem vectors; post-fix it returns `Err` so the caller can filter empties or skip the batch.

No regression test for the second branch (`embedded.first()` is None) yet — it requires mocking an HTTP server that returns 200 with empty data, which is an unusual server behavior. The defensive bail in code prevents corruption even without a test; if the path ever fires in production the bail message will name the cause.
## Workarounds

Pre-fix: no workaround — library indexing (0/62 indexed) was blocked by the same code path. Post-fix: validation rejects bad input early; the upstream embedder bug remains to be diagnosed.

## Resume

Concrete next action: in `codescout-embed`, trace the `embed_batch` (or equivalent) function for the jina-embeddings-v2-base-code backend. Find paths that return `vec![0.0]` / short vectors and replace with proper `Result::Err` propagation. Repro: trigger a network failure or a malformed input during embedding; assert the call returns `Err` not a 1-elem `Ok`.

## References

- Originally tracked as **#6** in `docs/issues/bug-tracker.md` (retired after migration to per-file system).
- Session log: `docs/trackers/archive/artifact-code-linkage-session-log.md` F-6.
- Defensive fix commit: `d482ca8a` on `experiments`.
- Related: bug-tracker.md #5 (UNIQUE constraint), #7 (cascade-delete data loss) — same commit fixes all three reindex failure modes.
- Upstream investigation: open in `codescout-embed` crate.
