---
id: d291b97d9e69edef
kind: bug
status: open
title: workspace(activate) fails for every path once the active project's directory is gone
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-12
severity: medium
---

# BUG: workspace(activate) fails for every path once the active project's directory is gone

## Summary

When the active project is a linked worktree and that worktree's directory is removed, `workspace(action="activate")` fails for every path, the main checkout included, with `No such file or directory (os error 2)`. The session cannot return home until an empty directory is created at the old path.

## Class fit

No class on the roster says that a tool depends on state under a directory that the operator can delete from outside; the nearest, `IC-17` (a shared resource carries no owner), is about peers. The template says to use the escape hatch rather than force a fit, so this is `cluster/unclassified`.

## Symptom (Effect)

- After `git worktree remove` of the active project's worktree, `workspace(activate, path=<main checkout>)` returns `No such file or directory (os error 2)`.
- `create_file` against the removed active project returns the same text. A read call (`grep`) returns a zero-match result with a warning instead.
- The error output ends with a status block that still says `paths are relative to <the removed path>`.

## Reproduction

Observed 2026-11-12 (probe 2 and probe 3 of bug `3dde1548c3e5c310`).

1. `git worktree add .worktrees/x -b y experiments`.
2. `workspace(activate, path=<.worktrees/x>, read_only=false)`.
3. `git worktree remove .worktrees/x` (run pinned to the main checkout).
4. `workspace(activate, path=<main checkout>)` returns `No such file or directory (os error 2)`.
5. `mkdir .worktrees/x`, then step 4 again: activation succeeds.

## Environment

codescout on `experiments`, Linux, MCP server connected through Claude Code.

## Root cause

Unknown. `Agent::activate` (`src/agent/mod.rs`) works on the target root only, so the failing step comes before it. Suspect, not verified: `Fence::try_mode` (`src/tools/native_provenance_fence.rs`) calls `root.canonicalize()?` on the active root for write-class tools, which fails when that root is gone. The first task of the plan in `docs/superpowers/specs/2026-11-12-worktree-removal-helper-design.md` is to verify this.

## Evidence

- The recovery in step 5 worked, which shows that the failure depends on the old directory existing.
- `create_file` with the default workspace failed with the same `os error 2` and did not recreate the directory.

## Hypotheses tried

None yet beyond the suspect above.

## Fix

Activation must not depend on the previous project's directory. The exact seam is decided after the reproducing test shows where the error starts.

## Tests added

N/A — not fixed. The regression test removes the active project's directory and then activates the main checkout.

## Workarounds

`mkdir` the removed path, activate the main checkout, then delete the empty directory.

## Resume

Write the reproducing test first, and read where the `os error 2` starts.

## References

- Bug `3dde1548c3e5c310` (worktree removal), spec `2026-11-12-worktree-removal-helper-design.md`.
