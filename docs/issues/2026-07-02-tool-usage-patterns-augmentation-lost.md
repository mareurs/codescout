---
status: open
opened: 2026-07-02
closed:
severity: high
owner: marius
related: [f2ecdd76a6189efb]
tags: [librarian, augmentation, tracker, claude-md-drift]
kind: bug
---

# BUG: tool-usage-patterns tracker has lost its augmentation; CLAUDE.md's documented T-N append workflow is broken

## Summary
The Tool Usage Patterns tracker (`id: f2ecdd76a6189efb`,
`docs/trackers/tool-usage-patterns.md`) is documented in `CLAUDE.md` as an
augmented artifact whose `params` hold the structured T-N table and whose
`render_template` renders it at the `[LIVE]` header. The catalog now holds **no
augmentation** for it: `artifact(get)` returns `"augmentation": null`, and it is
absent from `artifact(find, augmented=true)`. The append workflow CLAUDE.md
prescribes — `artifact_augment(id=..., merge=true, params={observations:[...]})`
— will error, because `merge=true` requires an existing augmentation
(`augment::tests::merge_true_without_existing_augmentation_errors`).

## Symptom (Effect)
```
# artifact(action="get", id="f2ecdd76a6189efb", full=true)
"updated_at": 1780489457853,
"freshness": "unknown",
"latest_event": null,
"augmentation": null,        # <-- expected: {prompt, params, render_template, entry_collection, ...}

# artifact(action="find", kind="tracker", augmented=true)  → 3 items:
#   52451519052d207c  windows-platform-support
#   3366f6ae253097bd  headroom-llm-proxy-integration
#   cd886c414f6751b4  legibility-backlog
#   (f2ecdd76a6189efb tool-usage-patterns is NOT in the list)
```

## Reproduction
Minimal, at HEAD `78d9ef21` (branch `experiments`):
1. `artifact(action="get", id="f2ecdd76a6189efb", full=true)` → `augmentation` is `null`.
2. `artifact(action="find", kind="tracker", augmented=true)` → 3 trackers, this id absent.

Both are read-only; reproducible immediately.

## Environment
codescout MCP (live release), Linux, project `codescout`, branch `experiments`,
HEAD `78d9ef21`. Catalog DB is machine-local (not in repo).

## Root cause
Unknown — under investigation. Leading hypothesis: the augmentation row was
dropped by a `delete`+`create` cycle or a reindex path that did not preserve it.
`id = sha256(abs_path)` and augmentation lives only in the machine-local catalog
(not git), so any flow that deletes+recreates the catalog row orphans the
augmentation (documented in CLAUDE.md § tracker archiving warning). The body
prose (per-observation T-001…T-010 analysis) survives in the file; only the
catalog-side augmentation (prompt + params + render_template + entry_collection)
is gone.

## Evidence
### 1. get returns null augmentation
Buffer `@tool_24879a21` from `artifact(get, id=f2ecdd76a6189efb, full=true)`:
top-level `"augmentation": null`; `preview.headings` still lists T-001…T-010
(body prose intact).

### 2. find augmented=true excludes it
`artifact(find, kind=tracker, augmented=true)` returns exactly 3 augmented
trackers (windows-platform-support, headroom-llm-proxy-integration,
legibility-backlog); tool-usage-patterns is not among them — corroborates that
the null in (1) is a real absence, not a get-projection omission.

## Hypotheses tried
1. **Hypothesis:** `augmentation: null` is the known get-projection bug
   (`artifact(get) does not echo entry_collection`).
   **Test:** cross-checked with `find(augmented=true)`.
   **Verdict:** rejected — that bug omits *fields* of a present augmentation; here
   the artifact is absent from the augmented set entirely, so the augmentation
   row itself is gone.
   **Evidence link:** Evidence #2.

## Fix
Plan (needs decision — not yet implemented):
- **Restore:** re-augment via `artifact_augment(id="f2ecdd76a6189efb", ...)` with
  the original `prompt`, `render_template`, `entry_collection="observations"`, and
  a `params.observations` array reconstructed from the body's T-001…T-010 prose.
  Reconstruction is lossy without the pre-loss params snapshot; check
  `artifact_event(action="list", artifact_id=...)` for a prior `field_patch`/params
  history that could seed it.
- **Or accept + document:** if hand-maintaining the body table is now preferred,
  update CLAUDE.md's "Tool Usage Patterns" section to drop the
  `artifact_augment(merge=true)` append instructions (they currently instruct a
  call that errors).
- **Prevent:** investigate whether a reindex/move path can drop augmentation rows
  silently; if so, that is a separate root-cause bug to file.

## Tests added
N/A — not yet fixed. A regression test would assert augmentation survives the
reindex/move path once the root cause is confirmed.

## Workarounds
To append a T-N entry today without the augmentation: edit the body prose
directly via `artifact(update, id=..., patch={body_edits:[...]})` (the analysis
section), and defer the structured params row until augmentation is restored. Do
NOT call `artifact_augment(merge=true)` — it will error.

## Resume
Confirm root cause: run `artifact_event(action="list",
artifact_id="f2ecdd76a6189efb")` and look for the last event that references the
augmentation (a `field_patch` on params, or a status/move event) to date the
loss and identify the triggering operation. If a params snapshot exists in event
history, use it to seed the restore; otherwise reconstruct from body prose. Then
decide restore-vs-document with Marius.

## References
- `CLAUDE.md` § "Tool Usage Patterns — docs/trackers/tool-usage-patterns.md"
  (documents the augmentation + append workflow now broken).
- `docs/issues/2026-07-02-artifact-get-omits-entry-collection.md` (distinct: field
  omission on a *present* augmentation).
- Session: prompt-hamsa / tracker-as-skill work stream, 2026-07-02.
