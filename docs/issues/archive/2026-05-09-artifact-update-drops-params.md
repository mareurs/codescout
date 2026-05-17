---
status: fixed
opened: 2026-05-09
closed: 2026-05-09
severity: medium
owner: marius
related: []
tags: ["artifact", "update", "augmentation", "params", "silent-drop"]
---

# BUG: `artifact(update, patch={params: ...})` silently dropped `params`

## Summary

Callers following the documented refresh pattern — `artifact(update, patch={params: {...}}, commit_refresh=true)` — saw `params` silently ignored. `UpdatePatch` had no `params` field, so serde dropped the key. The augmentation params remained unchanged. `commit_refresh=true` still fired, recording a refresh with stale params. Documented surfaces directed callers to a non-existent field. Fixed by adding `params` to `UpdatePatch` and routing through `augmentation::merge_params`.

## Symptom (Effect)

- Caller passes `artifact(update, id=X, patch={params: {entries: [...]}}, commit_refresh=true)`.
- Call returns success.
- Subsequent `artifact(get, id=X, full=true)` shows the augmentation params unchanged.
- `commit_refresh` recorded a refresh entry with the stale params.

## Reproduction

Call `artifact(update, id=<any augmented artifact>, patch={"params": {"foo": "bar"}})` and inspect the artifact afterward.

## Environment

- Date: 2026-05-09
- Component: `crates/librarian-mcp/src/tools/update.rs` — `UpdatePatch` struct

## Root cause

`params` belongs to the `artifact_augmentation` table, not `artifact`. The `update` tool only patched the artifact row; the `UpdatePatch` struct had no `params` field, so serde silently dropped the key on deserialization. The prompt surface had been instructing callers to use this path anyway.

## Evidence

Direct: comparing the artifact body before and after the call showed `params` unchanged. Tracing into `update::call` confirmed the field never reached the augmentation layer.

## Hypotheses tried

1. **Hypothesis:** Document the workaround (split into two calls) and leave `update` untouched. **Verdict:** Rejected — the prompt surface already documented the broken path; documenting two calls multiplies the cognitive load. **Evidence link:** see Fix.
2. **Hypothesis:** Add `params` to `UpdatePatch` and route through `augmentation::merge_params`. **Verdict:** Confirmed — adopted as the fix. **Evidence link:** see Fix.

## Fix

`params` added to `UpdatePatch`, routed through `augmentation::merge_params`. Commit `e406218` on `experiments`. Both prompt surfaces updated.

## Tests added

*N/A — migrated from compact form; specific regression test not named in the original entry. Recommend `update_patch_params_merges_into_augmentation`.*

## Workarounds

Pre-fix: split into two calls — `artifact_augment(id, merge=true, params={...})` to update params, then `artifact(update, commit_refresh=true)` to record the refresh timestamp.

## Resume

N/A — fixed.

## References

- Originally tracked as **BUG-056** in `docs/TODO-tool-misbehaviors.md` (deprecated 2026-05-09; superseded by per-file system).
- Fix commit: `e406218` on `experiments`.
