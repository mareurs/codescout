---
id: d91bf97d9b769e09
kind: bug
status: fixed
title: guard_worktree_write refuses writes whose target is outside every project, because it decides on the session and not on the target
tags:
- cluster/unclassified
closed: 2026-10-31
local_fix_branch: research/checked-real-task-20261024
local_fix_commit: bc32dc291ebee89bc630181fac72f82290f6f6af
local_fix_patch_id: 7e747cb09376244a88ffc05c50488b671dd3a389
opened: 2026-10-26
owner: marius
severity: low
---

# BUG: `guard_worktree_write` refuses writes whose target is outside every project, because it decides on the session and never looks at the target

## Summary

`guard_worktree_write` (`src/tools/core/guards.rs:47-112`, read 2026-10-26) takes only the `ToolContext`. It allows a write when the call carries a `workspace=` pin, when the project was chosen this session, or when no git worktrees exist. It never inspects the write's target path. A write to a scratchpad under `/tmp` or `~/.cache` is therefore refused as "ambiguous which project" while worktrees exist and no `activate` was called.

## Class fit

The guard exists to stop a write landing in the wrong project, but it fires on writes that cannot land in any project. No existing class fits that trait cleanly, so this bug takes the sanctioned unclassified escape rather than forcing a fit. It differs from guards that are too narrow, because this one is too wide.

## Symptom (Effect)

An agent that only wants to write a scratch script gets `Write blocked: git worktrees detected but workspace(action='activate') has not been called` and must activate or pin.

## Reproduction

Observed in `usage.db`, not constructed: `err_family='worktree_activate_required'`, 72 rows in 32 sessions (2026-09-27..2026-10-26). 36 rows in 19 sessions have an absolute target outside `/home/marius/work/claude/codescout` (`input_json` `path` or `file_path`), 36 are inside or relative.

## Environment

codescout `experiments` at `8df90f1b9`. Last occurrence seen 2026-10-26 18:38 (subagent report; its target path was not inspected).

## Root cause

The decision uses `ctx.workspace_override`, `is_project_chosen_this_session` and `list_git_worktrees(&root)` and nothing about the target.

## Evidence

The guard's own contract (doc comment) lists three allow conditions and none concerns the target. The refusal and hint do name both remedies, and agents learned them: the subagent counted 15 `create_file` retries that carried a pin and 11 sessions that activated. Volume fell 42, 19, 9, 2 per week (subagent counts).

## Hypotheses tried

Whether an outside-project write would be refused later by the write-root checks regardless: not checked.

## Fix

Integrated 2026-10-31 in `67e971a7b` on `experiments`. It is the change written and independently reviewed on 2026-10-27 as `bc32dc291` on `research/checked-real-task-20261024`; the patch-id is identical (`7e747cb09376244a88ffc05c50488b671dd3a389`), so no line differs.

`create_file`, `edit_file` and `approve_write` now call `guard_worktree_write_for_target` (`src/tools/core/guards.rs`). A write whose target resolves outside the active project, its linked worktrees and its discovered members skips the project-selection guard and reaches the normal write authorization. A target inside any of those roots, one with no `path`, an empty or NUL-bearing path, or one that cannot be resolved keeps the old guard. Atomic file writes are classified by the parent plus the unresolved leaf, so a leaf symlink counts as the entry replaced; a directory approval resolves the leaf directory.

The patch was applied to the working tree and committed by pathspec, because a peer's staged file blocks `git cherry-pick`.

## Tests added

`tests/worktree_write_scope.rs` (6 tests) and `tests/worktree_write_scope_review.rs` (11 tests, through the production `call_content` entry). They cover outside-target controls, main and linked roots, directory aliases, discovered and relocated members, malformed and opaque paths, normalised aliases and both file-symlink directions.

Checked again here on 2026-10-31: with only the tests applied, the three `*_outside_project_is_independent_of_linked_worktrees` tests failed with the original refusal and the three protection controls passed. The review binary did not run in that red run, because cargo stopped at the first failing binary. With the source applied, 6 and 11 pass. A mutant that always answers "outside every project" fails 11 (the 3 protection controls and 8 review tests). The original branch recorded eight mutations, all killed; I did not re-run those eight.


## Fix provenance

- **SHA:** `67e971a7b` (`experiments`)
- **patch-id:** `7e747cb09376244a88ffc05c50488b671dd3a389`

## Workarounds

Pin `workspace="<abs path>"` on the call.

## Resume

Fixed and archived. Gate on `experiments` with the fix: `PYTHON_LIGHT=0 CLIPPY=0 LEAN=0 DEFAULT=0`; `FMT=1` was `ownership-refused` because peers have dirty files, and `rustfmt --check` passes on the six files. The heavy Python suites were not run.

**Seen live 2026-10-31** on a server rebuilt after `67e971a7b`, in a session with no project activated and nine linked worktrees present. `create_file` of a scratch file under the session scratchpad, outside every project, returned `status: ok` and the file existed on disk. `create_file` of a probe file under the linked worktree `/tmp/codescout-real-task-20261024` was refused with `Write blocked: git worktrees detected but workspace(action='activate') has not been called`, and the probe file was not created. Only the `create_file` path was run live. `edit_file` and `approve_write` were not, and `workspace=` pinned calls were not tried. The pre-fix refusal for the first case was not re-run on the old binary here; the 72 logged refusals and the red test run stand for it.

## References

- `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`
- `docs/issues/archive/2026-10-06-the-worktree-write-guard-keys-on-the-doc-action-name-and-refuses-previews-and-malformed-calls.md`
