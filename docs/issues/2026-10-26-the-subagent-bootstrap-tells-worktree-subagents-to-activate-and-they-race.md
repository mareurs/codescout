---
id: '2f6b6a8bcc9150c7'
kind: bug
status: open
title: the subagent bootstrap tells every worktree subagent to activate, so parallel subagents race on the shared active-project slot
tags:
- cluster/doc-contradicted-by-code
opened: 2026-10-26
owner: marius
severity: medium
---

# BUG: the subagent bootstrap tells every worktree subagent to `activate`, so parallel subagents race on the one shared active-project slot

## Summary

`claude-plugins:codescout-companion/hooks/subagent-guidance.mjs` injects `PROJECT BOOTSTRAP: workspace(action="activate", path="<own root>") is your FIRST codescout action` into every subagent. For a subagent that runs in its own git worktree, `<own root>` is that worktree. Several subagents started together therefore activate several roots in the same second, and activation is process-wide, so each unpinned call after the last activation resolves against the last writer's worktree. `get_guide("workspace-state")` says the opposite: "Brief subagents to pin; do not brief them to activate."

## Class fit

The repo's guide and the companion plugin's injected text give contradictory instructions for the same situation. That is the doc-contradicted-by-code trait. It differs from the class's other members because one side is prose in another repository that the server cannot see or test, so no test in this repo can pin the agreement.

## Symptom (Effect)

Subagents fail to read files that exist in their own worktree, because the error shows they searched another agent's worktree. `path_not_found` and `memory_target_not_found` bursts follow.

## Reproduction

Observed, not constructed. Session `5b936994-5ce8-41f4-9839-f615e56f3efd`, 2026-10-24:
1. Ten subagents each called `workspace(action="activate")` on their own `.claude/worktrees/agent-<id>` between 03:08:14.027 and 03:08:17.894 (3.9 s).
2. At 03:08, subagents `aa685925`, `a4d168bd` and `aa922711`, calling `read_file` with no `workspace=` pin, got `file not found … (searched …/.claude/worktrees/agent-a1ec5bbc4107d1c6b/docs/issues/…)`. `a1ec5bbc` is another agent's worktree, one of the ten.
3. Subagent `aac3dcf3` pinned `workspace=` to its own worktree and searched its own tree, so pinning resolves the root.

## Environment

codescout `experiments`; companion plugin `hooks/subagent-guidance.mjs` lines 44-52 (read 2026-10-26). `usage.db`: 246 `workspace(activate)` calls across 101 sessions in 2026-09-27..2026-10-26, 143 of them from subagents.

## Root cause

Two causes, found separately.
- **A (race, verified):** the hook prescribes `activate`; activation moves the process-wide default; concurrent subagents overwrite each other. The project declared unpinned concurrent use unsupported on 2026-10-16 (`docs/issues/archive/2026-09-24-residual-subagent-activate-displacing-home-default.md`, option 2, documentation plus a notice) and declined options A (refuse subagent `activate`) and B (per-principal default), adding "should the soft form prove insufficient, the design notes for A and B are in this file's § Fix". The hook still prescribes the unsupported pattern.
- **B (base mismatch, subagent finding, partly verified):** after pinning, the subagent still got `file not found`, because the parent briefed paths from the main checkout and the worktree HEAD (`5a8b14691`) predates the archive move `8f480f02f`. Not verified by the controller beyond the pinned failing row above.

## Evidence

- `usage.db` rows above (query `tool_calls` where `tool_name='workspace'`, `session_id='5b936994…'`, 2026-10-24 03:08).
- The same session produced 18 `path_not_found` errors in one minute and 8 `memory_target_not_found` "topic not found" errors (worktree has fewer memory topics than main; subagent finding).
- The same pattern shows in sessions `500c9b13` and `e98b5705` (10-16..19; 57 and 18 `path_not_found`).
- `get_guide("workspace-state")` lines 140, 183 and 189: "subagents that activate different workspaces race — last writer wins"; "Brief subagents to pin; do not brief them to activate."

## Hypotheses tried

Whether the soft notice ("warns the switcher; tells the harmed party once per move") fired in the 10-24 burst: not checked.

## Fix

Open. In the companion repo, change the bootstrap for a subagent whose root differs from the parent's to "pin every call with workspace=<root>" and not "activate". Separately, the parent should brief paths valid in the worktree's base commit.

## Tests added

None. A test in the companion repo would assert the injected text for a worktree cwd names `workspace=` and does not instruct `activate`.

## Workarounds

Brief each subagent explicitly to pin `workspace="<abs worktree>"` on every call.

## Resume

Found by the 2026-10 friction deep dive (`docs/trackers/friction-deep-dive-2026-10.md`, groups G3 and G4). Fix belongs mainly in `claude-plugins`; cite it as `claude-plugins:codescout-companion/hooks/subagent-guidance.mjs`.

## References

- `docs/issues/archive/2026-09-01-workspace-activation-is-process-wide-and-a-subagent-can-flip-it.md`
- `docs/issues/archive/2026-09-24-residual-subagent-activate-displacing-home-default.md`
- `docs/adrs/2026-09-14-a-subagent-is-a-principal.md`
