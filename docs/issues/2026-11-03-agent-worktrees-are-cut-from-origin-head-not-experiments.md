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
unverified: Only one probe ran, in one session, on Linux. The subagent report is the subagent's own output; the main session re-derived the HEAD match with git. Whether sessions that started before the edit pick it up was not tested. The three profiles other than .claude-stef were edited after the probe and not probed.
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


### Applied and measured 2026-11-03

The operator approved `git remote set-head origin experiments`; it was run. `git symbolic-ref refs/remotes/origin/HEAD` then gave `refs/remotes/origin/experiments`. A second `Agent(isolation: "worktree")` probe, dispatched from this checkout, got:

- `git rev-parse HEAD`: `71d9995f1bd67e891d2b68b4618380a0f74a0f1a`, which is the tip of `origin/experiments` (`git rev-parse --short origin/experiments` gave `71d9995f1`).
- `git merge-base --is-ancestor 5a8b14691 HEAD`: true (the old base is an ancestor of the new one).
- `git merge-base --is-ancestor 8a2e2ef0b HEAD`: false. `8a2e2ef0b` is a local commit that is not pushed.

The base moved from `5a8b14691` (`master`, 2026-07-05) to the tip of `origin/experiments`, so the harness reads `origin/HEAD` to pick an agent worktree's base. This settles the hypothesis that the first probe could not tell apart.

**What is still wrong:** the base is the REMOTE tip, not the local branch. A subagent does not see any commit that is not pushed. On 2026-11-03 that was 34 commits (`git rev-list --count origin/experiments..experiments`). So a worktree subagent still misses files that exist only in unpushed commits. Pushing `experiments` closes the gap at the moment of the push, and it reopens with the next local commit. `worktree.baseRef: head` is the setting that would follow the local branch; it is not tried.

Undo: `git remote set-head origin master`.

### baseRef=head applied and measured 2026-11-04

With the operator's approval, `"worktree": {"baseRef": "head"}` was added to `settings.json` in `~/.claude-stef`, `~/.claude`, `~/.claude-sdd` and `~/.claude-kat` (operator rule OP-4: all profiles). The key and its values (`fresh` | `head`) come from the Claude Code changelog, which says the setting covers agent-isolation worktrees. All four files parse as JSON afterwards.

Probe: one read-only `Agent(isolation: "worktree")` subagent, dispatched from this session on `experiments` after the `.claude-stef` edit. Its worktree `HEAD` was `10f156d705c8875745c263f2e8f77c48f302dc70`. In the main session, `git merge-base --is-ancestor 10f156d70 HEAD` is true and `git rev-parse --short HEAD` is `10f156d70`, so the base is the local `HEAD`. The tip of `origin/experiments` was `537eb2cdf` and the worktree was 4 commits ahead of it. The subagent also confirmed `e165d1016`, an unpushed commit, is an ancestor of its `HEAD`. This session's own settings were edited mid-session and the probe still used the new value, so no restart was needed in this case.

Undo: delete the `worktree` key from the four files.

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

Applied 2026-11-03 with the operator's approval: `git remote set-head origin experiments` (undo: `git remote set-head origin master`). Re-checked 2026-11-04: `git symbolic-ref refs/remotes/origin/HEAD` gives `refs/remotes/origin/experiments`. The probe in "Applied and measured" under Reproduction showed the base moved to the tip of `origin/experiments`, so the harness reads `origin/HEAD`.

What is still wrong: the base is the REMOTE tip, not the local branch, so a worktree subagent does not see unpushed commits. On 2026-11-04 `git rev-list --count origin/experiments..experiments` gives 2.

Second candidate, not tried: set `worktree.baseRef` to `head`. That is a config change in all four profiles, and its effect on `Agent(isolation: "worktree")` is not checked. It needs the operator's approval.

A hook cannot fix the brief itself. The companion bootstrap already tells a worktree subagent to check that its paths exist in the worktree (bug `2f6b6a8bcc9150c7` Fix, part 3). `set-head` breaks nothing tracked in this repository or in `claude-plugins` (searched 2026-11-03).

## Tests added

None. The behaviour belongs to the harness, which this repository cannot test.

## Workarounds

Brief worktree subagents only with paths that exist at the worktree's base commit, or have the subagent run `git reset --hard experiments` on its own worktree branch before it starts, as the 2026-10-30 subagent did.

## Resume

Both fixes are applied and measured (see "baseRef=head applied and measured 2026-11-04" under Reproduction). Remaining: a worktree subagent started from a session that loaded the setting before it was written might still use the old base; check once after a restart. Nothing else is open on this record; move it to `fixed` with provenance after that check, citing the settings change (not a commit: the settings files are outside every repository).

## References

- `docs/issues/2026-10-26-the-subagent-bootstrap-tells-worktree-subagents-to-activate-and-they-race.md` (bug `2f6b6a8bcc9150c7`, cause B)
