---
kind: bug
status: open
tags:
- cluster/unclassified
closed: null
opened: 2026-09-08
owner: marius
related: []
severity: low
---

# BUG: the staleness reason says "N of M anchored files changed" while N counts changed plus deleted, so the number a reader acts on names the wrong population

## Summary

`workspace(action="status")` renders each stale memory's reason as
`"{N} of {M} anchored files changed"`. N is `report.stale_files.len()`, which includes
anchors whose file was **deleted**, not only those whose content changed. The
distinction matters to the reader: a changed anchor means "re-read this and check the
claim"; a deleted anchor means "this memory may describe something that no longer
exists", which is a different and usually more urgent kind of wrong.

## Symptom (Effect)

Observed live 2026-09-08, before the memory refresh:

```
architecture:  "17 of 23 anchored files changed"
               changed_files: 15 entries
               deleted_files:  2 entries   (src/tools/usage.rs, src/tools/ast.rs)

worktree-merge-catalog-reconciliation:
               "4 of 8 anchored files changed"
               changed_files: 3 entries
               deleted_files: 1 entry
```

The response carries `changed_files` and `deleted_files` as separate arrays, so the
correct decomposition is right there — only the prose summary conflates them. A reader
who trusts the sentence and skips the arrays plans the wrong work.

## Reproduction

`workspace(action="status")` on a project with a memory anchoring a deleted file.
Compare the `reason` string against `len(changed_files)`.

## Environment

codescout 0.15.0, branch `experiments`. Observed at `5ec4bac8`; the two example
memories have since been repaired, so reproducing needs a memory that currently anchors
a deleted path.

## Root cause

`src/memory/anchors.rs:258-263` builds the string as
`format!("{} of {} anchored files changed", total_stale, total_anchored)` where
`total_stale = report.stale_files.len()`.

`check_path_staleness` (`src/memory/anchors.rs:184-207`) classifies each anchor as
`AnchorStatus::Deleted` when the file is missing and `AnchorStatus::Changed` when
`hash_file(&full) != anchor.hash`. Both land in `stale_files`. So the count is correct
as a count of *stale* anchors and wrong only as a description of *why* they are stale.

Measured 2026-09-08 from two live `workspace(action="status")` responses, decomposition
read from the response's own `changed_files` / `deleted_files` arrays.

## Evidence

The response shape already separates what the sentence merges:

```json
{ "topic": "architecture",
  "reason": "17 of 23 anchored files changed",
  "changed_files": [ ...15... ],
  "deleted_files":  [ "src/tools/usage.rs", "src/tools/ast.rs" ] }
```

## Hypotheses tried

1. **Hypothesis:** deleted anchors are rare enough that the conflation never misleads.
   **Test:** of 16 stale memories on 2026-09-08, two carried deleted anchors — 12.5%.
   **Verdict:** rejected as a reason to leave it, though it does justify `severity: low`.

## Cluster — why `unclassified` rather than a forced fit

`IC-20` (`floor-published-under-the-name-of-a-total`) is the near miss and does **not**
fit, in two ways that matter. Its claim turns on the true value being **unknowable
because the walk stopped**; here it is fully knowable from the same response, which
carries `changed_files` and `deleted_files` as separate arrays. And the direction is
inverted: `IC-20` is a *subset* published under the whole population's name, this is the
*whole* published under a subset's name. The remedies rhyme (both are renames) but the
diseases differ. Filed under the escape hatch rather than moved toward `IC-20`'s
threshold on a member that is really its mirror image.

## Fix

Not started. Smallest correct change is to say "stale" rather than "changed", which is
accurate for both classes and costs no bytes:

```
"{N} of {M} anchored files stale"
```

A fuller form naming both counts reads better but is longer, and this string appears
once per stale memory in a response that already lists them.

## Tests added

None yet. A regression test would build a fixture memory anchoring one changed and one
deleted file and assert the reason string does not claim both "changed" — asserting on
the *name* of the failure state rather than on a count, which is the discriminator this
repo's own testing discipline asks for.

## Workarounds

Read `changed_files` and `deleted_files` from the response rather than the `reason`
string. The arrays are correct; only the summary is not.

## Resume

Change the format string at `src/memory/anchors.rs:262` from "changed" to "stale", or
render both counts. Check whether any doc or skill quotes the current wording before
changing it.

## References

- `src/memory/anchors.rs:258-263` (the string), `:184-207` (`check_path_staleness`)
- `src/tools/config/mod.rs:591-607` (the status handler that surfaces it)
- Found while re-deriving all 16 stale memories, `09ecb58d`
