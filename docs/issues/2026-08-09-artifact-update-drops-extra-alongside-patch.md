---
id: '973f0c0f6721ea83'
kind: bug
status: open
title: artifact(update) silently drops top-level `extra` when `patch` is also passed
owners:
- marius
tags:
- librarian
- artifact
- silent-data-loss
- tool-quirk
closed: null
opened: 2026-08-09
related:
- docs/issues/2026-08-09-path-strip-corrupts-file-content-and-root-fields.md
severity: medium
---

# BUG: artifact(update) silently drops top-level `extra` when `patch` is also passed

## Summary
Calling `artifact(action="update", …)` with top-level `status` **and** `extra` alongside a
`patch` object applies the `status` (auto-lifted, with a warning) but **silently discards
`extra`**. No error, no warning naming `extra`. The caller believes the frontmatter key was
written; it was not.

## Symptom (Effect)
Observed 2026-08-09 while closing a bug file during Task 5 of the field-aware-path-strip
plan. The call passed `status="fixed"`, `extra={"closed": "2026-08-09"}` and a
`patch={body_edits: [...]}`. Result: `status` became `fixed`, body edits applied,
`closed` absent from frontmatter. Re-issuing as a `patch`-only call
(`patch={"status": …, "extra": {…}}`) wrote both.

## Reproduction
```
git rev-parse HEAD   # 2aecc0bf (experiments)
```
1. `artifact(action="update", id=<any>, status="fixed", extra={"closed":"2026-08-09"}, patch={body_edits:[…]})`
2. `artifact(action="get", id=<same>)` → `status` is `fixed`, `extra.closed` is missing.
3. Re-issue as `artifact(action="update", id=<same>, patch={"status":"fixed","extra":{"closed":"2026-08-09"}})` → both land.

## Environment
codescout MCP server (release binary), project codescout, branch `experiments`.

## Root cause
Unknown — not investigated. Best lead: the auto-lift path that promotes top-level `status`
into `patch` appears to handle `status` only, so a sibling top-level `extra` is neither
lifted nor rejected. The warning emitted mentions the lift but not the dropped key.

## Evidence
Reported by the Task 5 implementer subagent; full transcript in
`.superpowers/sdd/2026-08-09-field-aware-path-strip/task-5-report.md`. The workaround
(patch-only form) was confirmed to work in the same session.

## Hypotheses tried
1. **Hypothesis:** `extra` requires the `patch` form. **Test:** re-issued patch-only.
   **Verdict:** confirmed workaround; does not explain why the mixed form is silent
   rather than rejected. The tool's own contract says unknown `patch` keys return a
   `RecoverableError` — the mixed form should be at least as loud.

## Fix
Not implemented. Two candidate directions: lift every recognised top-level field into
`patch`, or reject the mixed form with a `RecoverableError` naming the dropped key.
Silence is the defect regardless of which is chosen.

## Tests added
None yet.

## Workarounds
Pass everything inside `patch`: `artifact(action="update", id=…, patch={"status": …,
"extra": {…}, "body_edits": [...]})`. Verify with `artifact(action="get", id=…)` that
`extra` actually landed — the write reports success either way.

## Resume
Locate the top-level-field lift in the artifact update handler
(`src/librarian/tools/update.rs`) and determine whether `extra` is dropped before or
during the merge. Decide between lift-all and reject-mixed, then add a regression test
asserting `extra` survives a mixed-form call (or that the call errors).

## References
- Discovered during: `docs/superpowers/plans/2026-08-09-field-aware-path-strip.md` Task 5.
- Handler: `src/librarian/tools/update.rs`.

