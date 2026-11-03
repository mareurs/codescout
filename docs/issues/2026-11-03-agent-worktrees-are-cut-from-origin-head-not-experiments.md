---
id: '892d22784432cf8b'
kind: bug
status: open
title: 'BUG: agent worktrees are cut from the default branch (origin/HEAD = origin/master, 2026-07-05), not from experiments, so a subagent''s read_file misses the files its parent briefed'
tags:
- cluster/unclassified
closed: ''
opened: 2026-11-03
owner: marius
related:
- docs/issues/2026-10-26-the-subagent-bootstrap-tells-worktree-subagents-to-activate-and-they-race.md
severity: medium
unverified: Which ref the harness reads to choose an agent worktree's base is not verified. The match to origin/HEAD is by commit equality (the 2026-10-30 worktree base equals master and origin/master), not by reading the harness. The candidate fix has not been run.
---

# BUG: agent worktrees are cut from the default branch (origin/HEAD = origin/master, 2026-07-05), not from experiments, so a subagent's read_file misses the files its parent briefed

## Summary

A subagent that runs in its own git worktree gets a worktree whose base commit is the default branch's tip, `5a8b14691` (2026-07-05), not `experiments`. The parent briefs paths that exist on `experiments`. So the subagent's `read_file` answers "file not found" even with a correct `workspace=` pin. This is cause B of bug `2f6b6a8bcc9150c7`, split out on 2026-11-03 because cause A there is fixed and this one is not.

## Class fit

No class on the roster was adjudicated for this bug, so it takes the sanctioned escape. The trait is that a default (the remote's default branch, correct for releases) is used as the base for work that runs on a different branch, and nothing at the point of use says which branch was chosen. A second member is needed before a class is claimed.

## Symptom (Effect)

A worktree subagent cannot read files that its brief names, and the error shows that it searched its own worktree. Bug `2f6b6a8bcc9150c7` § Re-verification 2026-10-30: the worktree `.claude/worktrees/agent-a88be249b09c25d52` was cut from `5a8b14691`, the issue files the brief named exist only on `experiments`, and the subagent had to `git reset --hard` its own branch to `experiments` before it could work.

## Reproduction

Not reproduced on 2026-11-03: `git worktree list` shows no agent worktree today, and no `refs/heads/worktree-*` branch exists. Best lead: dispatch one subagent with worktree isolation from this checkout (on `experiments`), then run `git -C <its worktree> rev-parse HEAD` and compare it with `git rev-parse experiments` and `git rev-parse origin/master`.


### Reproduced 2026-11-03

The main session dispatched one read-only subagent with `Agent(isolation: "worktree")` from this checkout, on `experiments`. Inside its worktree `.claude/worktrees/agent-a6b2b3df33ca1e761` (branch `worktree-agent-a6b2b3df33ca1e761`):

- `git rev-parse HEAD` gave `5a8b146917ca9ae72bf75161e39337d2ef34e280`.
- `git log -1` gave `5a8b14691 2026-07-05 docs(issues): close tool-usage-patterns augmentation-loss (restored + root cause fixed)`.
- `git merge-base --is-ancestor 8a2e2ef0b HEAD` was false. `8a2e2ef0b` is an `experiments` commit from 2026-11-02, and the object exists in the shared store.

The worktree was removed automatically when the subagent ended unchanged (`git worktree list` then showed no `agent-a6b2b3df` entry). So the symptom is current. **Still not shown:** whether the harness reads `origin/HEAD`, the remote's default branch through another path, or `master`, because all three name the same commit.

The tool description of `EnterWorktree` in this session says its setting `worktree.baseRef` defaults to `fresh`, which "branches from origin/<default-branch>", and that `head` "branches from your current local HEAD". No profile sets `worktree` in its `settings.json` or `settings.local.json`, and neither does this project's `.claude/`. Not checked: whether `Agent(isolation: "worktree")` reads the same setting.

## Environment

codescout main checkout `/home/marius/work/claude/codescout`, branch `experiments`. Claude Code agent worktrees under `.claude/worktrees/agent-*`.

## Root cause

Not established. The candidate mechanism: the harness cuts an agent worktree from the remote's default branch, and that branch is `master`, which is 6,426 commits behind `experiments`.

- measured 2026-11-03: `git symbolic-ref refs/remotes/origin/HEAD` gives `refs/remotes/origin/master`; `git rev-parse origin/master master` both give `5a8b146917ca9ae72bf75161e39337d2ef34e280` (committed 2026-07-05); `git rev-list --count 5a8b14691..experiments` gives `6426`.
- measured 2026-10-30 (bug `2f6b6a8bcc9150c7`): the agent worktree's base was `5a8b14691`.
- Not read: which ref the harness itself uses to pick the base. That the base equals `origin/HEAD` is inferred from commit equality.

## Evidence

- Bug `2f6b6a8bcc9150c7` § Root cause (cause B), § Re-verification 2026-10-30 and § Cause A closed, cause B open (2026-11-02).
- The 2026-11-03 `git` outputs under § Root cause.

## Hypotheses tried

1. **Hypothesis:** the harness cuts the worktree from `origin/HEAD`. **Test:** compare the observed base with `origin/HEAD`'s target. **Verdict:** consistent (same commit), not confirmed: `master` and `origin/master` are the same commit, so the comparison cannot tell them apart.

## Fix

Not applied. The smallest candidate step is `git remote set-head origin experiments` (undo: `git remote set-head origin master`). It changes shared repository state that every session and tool reading `origin/HEAD` sees, so it needs the operator's approval. **It has NOT been run.** After it runs, check the fix with the step in § Reproduction. If the base is still `5a8b14691`, the harness reads something other than `origin/HEAD`, and this hypothesis is wrong.

A hook cannot fix the brief itself. The companion bootstrap already tells a worktree subagent to check that its paths exist in the worktree (bug `2f6b6a8bcc9150c7` § Fix, part 3).


Second candidate, found 2026-11-03: set `worktree.baseRef` to `head`. That is a config change, so it would go to all four profiles. It is the documented control for `EnterWorktree`; its effect on `Agent(isolation: "worktree")` is not checked. No script in this repository reads `origin/HEAD`, and no file in `claude-plugins` does either (searched 2026-11-03, hidden paths included), so `set-head` breaks nothing tracked in those two repositories.

## Tests added

None. The behaviour belongs to the harness, which this repository cannot test.

## Workarounds

Brief worktree subagents only with paths that exist at the worktree's base commit, or have the subagent run `git reset --hard experiments` on its own worktree branch before it starts, as the 2026-10-30 subagent did.

## Resume

Ask the operator whether to run `git remote set-head origin experiments`. If approved, run it, dispatch one worktree subagent, and compare its `HEAD` with `experiments` (§ Reproduction). Record the result here.

## References

- `docs/issues/2026-10-26-the-subagent-bootstrap-tells-worktree-subagents-to-activate-and-they-race.md` (bug `2f6b6a8bcc9150c7`, cause B)
