---
status: open
opened: 2026-06-03
closed:
severity: medium
owner: marius
related: []
tags: [artifact, librarian, delete, workspace, subagent, abs_path]
kind: bug
---

# BUG: `artifact(action="delete")` refuses an in-workspace artifact — "outside every workspace root" (suspected parallel-subagent abs_path)

## Summary
`artifact(action="delete", id=...)` refused to delete three trackers that live under
the active project's `docs/trackers/`, erroring **"outside every workspace root —
refusing to delete docs/trackers/<file>.md"**. The active project was `codescout`
(verified via `workspace(action="status")` immediately before), and the files were at
`codescout/docs/trackers/*.md` — plainly under the workspace root. The artifacts had
been created moments earlier by **three parallel general-purpose subagents** (via
`artifact` create + `artifact_augment`) during the augmented-tracker eval Run 1.
Cleanup required bare `rm` + `librarian(action="reindex", force=true)`, which dropped
the artifact rows (`removed: 3`) but left **orphan augmentation rows** in the local
catalog (`orphans_removed: 0`).

## Symptom (Effect)
For all three ids (`d0801763526d35b4`, `db3dfa86b0cb58d7`, `2782b9905883e711`):

```
artifact(action="delete", id="<id>")
→ error: "artifact '<id>' is outside every workspace root —
          refusing to delete docs/trackers/<file>.md"
```

The error names the correct `rel_path` — so the catalog *knows* the artifact and its
path — yet the guard claims it is outside every registered workspace root.

## Reproduction (hypothesized — NOT yet confirmed)
At codescout `3e1be988`, branch `experiments`, v0.14.0:

1. From one session, dispatch ≥2 **parallel** general-purpose subagents that each
   create a librarian artifact under the active project (`artifact(action="create")` +
   `artifact_augment(entry_collection=...)`).
2. After they finish, from the parent, call `artifact(action="delete", id=<one of theirs>)`.
3. Observe the "outside every workspace root" refusal even though
   `workspace(action="status").project_root` is the repo containing the file.

Which factor triggers it is unconfirmed: parallel creation, subagent-context workspace
resolution, or an `abs_path` normalization at create time.

## Environment
codescout v0.14.0, commit `3e1be988`, branch `experiments`. Active project `codescout`.
Artifacts created by 3 concurrent `general-purpose` subagents during the
`augmented-tracker-discovery` eval Run 1 (2026-06-03).

## Root cause
**UNCONFIRMED — forensic trail removed during cleanup.** The `reindex --force` dropped
the artifact rows, so the stored `abs_path` that the delete guard rejected is no longer
inspectable. Hypothesis: the artifact's stored `abs_path` was written in a form the
delete guard's "under a managed workspace root" check rejects (non-canonical / relative
/ wrongly-prefixed), possibly because the *creating subagent's* workspace resolution
differed from the parent's at create time — the documented shared-server, process-global
active-project behavior under concurrent subagents. The delete guard likely compares
`artifact.abs_path` against the registered workspace roots and fails on a
canonicalization mismatch.

## Evidence
- The three refusals quoted above (eval Run 1, 2026-06-03).
- `workspace(action="status")` at the same time: `project_root ==
  "/home/marius/work/claude/codescout"`; files were
  `docs/trackers/{ingestion-defects,auth-refactor-session-log,retrieval-eval-defects}.md`.
- `librarian(action="reindex", force=true)` reported `removed: 3`, `orphans_removed: 0`.
- Recorded in `docs/evals/augmented-tracker-discovery.md` § "Run 1" (Tooling defect observed).

## Hypotheses tried
None — observed during cleanup, logged at user request. The `rm` + reindex was a
workaround, not an investigation.

## Fix
Not implemented. Direction once confirmed:
1. Canonicalize **both** sides (artifact `abs_path` and each workspace root) before the
   delete-guard "is under a root" comparison.
2. Ensure `artifact(action="create")` stores a canonical absolute `abs_path` regardless
   of whether a parent or a subagent context creates it.
3. Add a `librarian(action="doctor")` / reindex path that cleans **orphan augmentation
   rows** whose artifact row no longer exists (the residue this incident left).

## Tests added
None yet. Regression test once root cause is confirmed: create an artifact under the
active project root, assert `artifact(action="delete")` succeeds; plus a variant that
reproduces the concurrent-subagent `abs_path` form.

## Workarounds
- `rm` the file(s), then `librarian(action="reindex", force=true, scope="project")` to
  drop the now-missing-file artifact rows. **Caveat:** this leaves orphan augmentation
  rows in the local catalog (no API cleans them once the artifact row is gone); they are
  local-only (the catalog DB is gitignored) and harmless to the repo.
- To avoid: do not have parallel subagents create artifacts the parent will need to
  delete; or delete from within the creating subagent's context.

## Resume
1. Reproduce per the hypothesized recipe; **before any cleanup**, capture the stored
   `abs_path` of the un-deletable artifact directly from the catalog DB
   (`sqlite3 ~/.local/share/librarian/catalog.db "SELECT id, abs_path FROM artifacts WHERE id='<id>'"`)
   — that is the missing forensic datum.
2. Compare against the delete-guard workspace-root check; confirm the canonicalization
   mismatch.
3. Inspect `abs_path` assignment in `artifact(action="create")` under a subagent context.
4. Consider the doctor/reindex orphan-augmentation cleanup (Fix step 3).

## References
- `docs/evals/augmented-tracker-discovery.md` § "Run 1" — observation source.
- CLAUDE.md § "Concurrent multi-workspace: one server, one active project" — the
  shared-server / process-global active-project hazard the hypothesis links to (see also
  the 2026-05-30 shared-server-global-active-project-race bug).
