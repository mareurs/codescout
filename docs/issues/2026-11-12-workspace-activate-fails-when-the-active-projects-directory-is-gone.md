---
id: d291b97d9e69edef
kind: bug
status: fixed
title: workspace(activate) fails for every path once the active project's directory is gone
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-12
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

Verified by reading the code and by the dispatch test `activation_proceeds_after_the_default_root_was_deleted`.

- `workspace(action="activate")` has `Workspace::is_write` true (`src/tools/config/mod.rs`), so every call runs through `provenance_fence_context::scope_root` (`src/tools/provenance_fence_context.rs`), which fences a write on the active root.
- At `src/tools/provenance_fence_context.rs:183` the fence runs `canonical_checkout(root).or_else(|_| root.canonicalize())?`. When the active root is gone, both calls fail and the `?` returns `No such file or directory (os error 2)` before `Agent::activate` runs.
- The earlier suspect `Fence::try_mode` in `src/tools/native_provenance_fence.rs` was wrong: the failing call is in `scope_root`, not in the fence type.
- A read tool (`grep`) is not a write, so `scope_root` returns early for it. That is why reads gave a warning and not an error.

## Evidence

- The recovery in step 5 worked, which shows that the failure depends on the old directory existing.
- `create_file` with the default workspace failed with the same `os error 2` and did not recreate the directory.

## Hypotheses tried

1. Suspect `Fence::try_mode` (`src/tools/native_provenance_fence.rs`): not the failing seam.
2. Read `scope_root` and its callers: the `?` at `src/tools/provenance_fence_context.rs:183` is the failing call. Confirmed by the dispatch test, which fails before the fix and passes after it.

## Fix

`scope_root` runs a call unfenced when all of these hold: the tool is a write, the tool name is `workspace`, `action` is `activate`, and `root.try_exists()` is `Ok(false)`. Nothing under a missing root can be fenced, and activation is the call that every write refusal prescribes.

The pass-through names only activation. A future workspace write action does not skip the fence by riding on it. A missing root for any other write tool is still refused (`a_non_workspace_write_on_a_missing_root_is_still_refused`).

Applied in `src/tools/provenance_fence_context.rs` (`scope_root`, the `#[cfg(unix)]` block). The gate was green on the merged tree on 2026-11-12: FMT, CLIPPY, LEAN and DEFAULT lanes exit 0, Python 1836 passed. The macOS lane was not compiled locally; the fix is `#[cfg(unix)]`, so CI must confirm it.

## Tests added

- `a_workspace_write_on_a_missing_root_runs_unfenced` (`src/tools/provenance_fence_context_tests.rs`)
- `a_non_workspace_write_on_a_missing_root_is_still_refused` (same file)
- `a_workspace_write_that_is_not_activation_on_a_missing_root_is_still_refused` (same file)
- `activation_proceeds_after_the_default_root_was_deleted` (`src/server/workspace_pin_tests.rs`): deletes the default root, activates another root, and asserts `status == "ok"` and the new `project_root`.

## Fix provenance

- **SHA:** `c11b3f0f6` (`experiments`)
- **patch-id:** `855621febdf8`
- **SHA:** `d730d1800` (`experiments`)
- **patch-id:** `121783d866c7`
- **SHA:** `e43843b52` (`experiments`)
- **patch-id:** `701db11002ff`

## Workarounds

Not needed on a build that contains the fix. On an older server binary: `mkdir` the removed path, activate the main checkout, then delete the empty directory. The convention page `docs/conventions/removing-a-worktree.md` names this symptom.

## Resume

N/A — fixed. Check that CI is green on the macOS lane, and re-test with a server binary built from this fix: remove the active worktree, then `workspace(activate)` the main checkout.

## References

- Bug `3dde1548c3e5c310` (worktree removal), spec `2026-11-12-worktree-removal-helper-design.md`.
