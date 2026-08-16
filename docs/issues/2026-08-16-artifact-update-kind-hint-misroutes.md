---
id: '6c0952e8908fbf00'
kind: bug
status: open
title: artifact(update) rejects extra.kind with a hint naming a parameter update does not accept
tags:
- librarian
- artifact
- misleading-error
- tool-quirk
---

## Summary

There is no way to write `kind:` into an artifact's frontmatter through the librarian.
`artifact(update)` rejects it via `extra`, and the rejection's hint points at a parameter
`update` does not have — so following the hint verbatim produces a second error.

## Symptom (Effect)

```
artifact(action="update", id="3505e9242548bda4", patch={"extra": {"kind": "tracker"}})
→ {"ok": false,
   "error": "extra must not contain frontmatter field(s) the schema already models: kind",
   "hint": "pass `kind=` as its own parameter instead. Reserved: id, kind, status,
            title, owners, tags, topic, time_scope. `extra` is for keys outside
            the schema (opened, closed, severity, owner, related, …)."}

artifact(action="update", id="3505e9242548bda4", kind="tracker")
→ missing field `patch`
```

Adding `patch` alongside `kind` does not help: `kind` is not among the keys `patch`
accepts (`status, title, owners, tags, topic, time_scope, extra, body, body_edits, params`).

## Reproduction

Any artifact whose file lacks a `kind:` key. Observed 2026-08-16 on
`docs/trackers/structural-edit-gate-session-log.md` (id `3505e9242548bda4`), which had no
YAML frontmatter at all. `artifact(update, patch={status, title})` synthesized a frontmatter
block containing exactly `status:` and `title:` — no `kind:` — and no subsequent call could
add one.

## Environment

codescout `experiments`, Linux, Claude Code stdio, 2026-08-16, during a tracker-hygiene sweep.

## Root cause

Inferred from the two error messages, not read from source. The hint text appears written
for `create` (where `kind` *is* a top-level parameter) and is emitted from a validation path
shared with `update`, where the advice does not apply. The underlying gap is that `kind` is
deliberately excluded from `patch`'s accepted keys — reasonable, since `kind` drives catalog
classification — but nothing offers a sanctioned way to set it on an artifact that lacks it.

## Evidence

The catalog and the file disagree and cannot be reconciled through the API: the catalog row
for `3505e9242548bda4` carries `kind: tracker` (it is returned by
`artifact(find, kind="tracker")`), while the file on disk has no `kind:` key. Any consumer
reading the file rather than the catalog — a git-mv, a fresh clone's reindex, a non-librarian
reader — sees an unclassified document.

## Hypotheses tried

1. **Hypothesis:** `kind` is accepted as a top-level param on `update`, as the hint says.
   **Test:** `artifact(action="update", id=..., kind="tracker")`.
   **Verdict:** rejected — `missing field 'patch'`; `kind` is not read on this action.

## Fix

Not implemented. Either:

1. **Make the hint action-aware** — on `update`, say that `kind` is not settable and name
   what is (the cheap fix; removes the misrouting but leaves the gap), or
2. **Accept `kind` in `patch`** for artifacts, re-classifying the catalog row atomically
   (closes the gap; needs thought about reclassification side effects).

Fix 1 is worth doing regardless: a hint that names a parameter the action does not accept
costs a wasted round-trip every time it fires.

## Tests added

None yet.

## Workarounds

None through the librarian. The frontmatter must be written by another route, or the
artifact left classified in the catalog only.

## Resume

Find the `extra` reserved-key validation that emits "pass `kind=` as its own parameter
instead" and check whether it can see which action invoked it. If it can, branch the hint on
create-vs-update. Then decide separately whether `patch` should accept `kind`.

## References

- Surfaced by the tracker-hygiene sweep 2026-08-16 (`docs/trackers/tracker-hygiene-log.md`),
  detector D4 (frontmatter-catalog-mismatch) — this bug is why D4's "no `kind:`" sub-class
  could not be fully fixed in that sweep.

