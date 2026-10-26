---
id: ecb3508a7f51f803
kind: bug
status: open
title: guard_worktree_write refuses writes whose target is outside every project, because it decides on the session and not on the target
tags:
- cluster/unclassified
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

Open. Skip the guard when the resolved target is outside every project root.

## Tests added

None. A test would create a file in a temp dir outside the root, with worktrees present and no activation, and expect success.

## Workarounds

Pin `workspace="<abs path>"` on the call.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, group G3). Fix not started.

## References

- `docs/issues/archive/2026-09-02-the-write-guard-refuses-a-correctly-pinned-call.md`
- `docs/issues/archive/2026-10-06-the-worktree-write-guard-keys-on-the-doc-action-name-and-refuses-previews-and-malformed-calls.md`
