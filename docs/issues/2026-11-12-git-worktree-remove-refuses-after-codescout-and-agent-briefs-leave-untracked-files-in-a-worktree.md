---
id: '3dde1548c3e5c310'
kind: bug
status: open
title: git worktree remove refuses after codescout and agent briefs leave untracked files in a worktree
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-12
severity: medium
---

# BUG: git worktree remove refuses after codescout and agent briefs leave untracked files in a worktree

## Summary

Every worktree where a codescout write tool ran ends with untracked files that no `.gitignore` rule covers, so `git worktree remove <path>` refuses ("contains modified or untracked files") and each cleanup needs `--force` and a human decision. A subagent brief that names a scratch report outside `.superpowers/` adds one more. After one forced removal, git had already deregistered the worktree but the directory could not be deleted ("Directory not empty"), leaving a dangling directory under `.worktrees/`.

## Class fit

No class on the roster says that a tool leaves per-checkout state that nothing ignores or cleans up; the nearest, `IC-17` (a shared resource carries no owner), is about peers, not about files. The template says to use the escape hatch rather than force a fit, so this is `cluster/unclassified`.

## Symptom (Effect)

- `git worktree remove .worktrees/<name>` exits 128: `contains modified or untracked files, use --force to delete it`.
- The skill `finishing-a-development-branch` forbids forcing on the agent's own initiative, so the agent must stop and ask the operator once per worktree. The operator said on 2026-11-12: "we can't leave dangling codescout/superpowers trash lying around".
- Leaving the worktree behind also blocks `git branch -d`, because the branch stays checked out.

## Reproduction

Observed three times in one session (2026-11-11 and 2026-11-12); the minimal trigger is not isolated.

1. `git worktree add .worktrees/x -b y experiments`, enter it, activate it in codescout with `read_only=false`, and run codescout write and shell tools in it.
2. `git -C .worktrees/x status --porcelain -uall` lists untracked files.
3. `git worktree remove .worktrees/x` refuses.

## Environment

- codescout on `experiments`, Linux, git worktrees under `/.worktrees/` (ignored by `.gitignore` line 7).
- Three worktrees affected: `tracker-lifecycle-plan-a`, `tracker-lifecycle-plan-b`, `lifecycle-grace-ledger-scope`.

## Root cause

Unknown — under investigation. Verified facts:

- `.gitignore` has no rule for `.codescout/native-provenance-v1.counter` or `.codescout/native-provenance-v1.lock`. The lock name is defined in `src/tools/native_provenance_fence.rs` (`LOCK_NAME`). Both files appeared untracked in all three worktrees.
- Not a cause: `.superpowers/`, `.codescout/write.lock`, `.codescout/write.lock.holder`, `.codescout/private-memories/`, `scripts/__pycache__/` and `target/` are all ignored, so they do not block removal.
- `.superpowers-fix-report.md` was untracked and not ignored. A subagent brief written by the controller named that path for the implementer's scratch report. That one is a dispatch defect of the superpowers workflow, not of codescout.

## Evidence

- `tracker-lifecycle-plan-a`: `git status --porcelain -uall` gave `?? .codescout/embeddings`, `?? .codescout/native-provenance-v1.counter`, `?? .codescout/native-provenance-v1.lock`. The entry `.codescout/embeddings` is untracked even though `.gitignore` line 80 ignores `.codescout/embeddings/`; a rule with a trailing slash matches only directories, so it may have been a file or a symlink. Not verified before the worktree was removed.
- `tracker-lifecycle-plan-b`: `git worktree remove --force` exited 255 with `failed to delete '.../tracker-lifecycle-plan-b': Directory not empty`. `git worktree list` no longer named it, but the directory still held `.codescout` and `target`. It needed `rm -rf` of the exact path. Why files reappeared during the removal is unknown.
- `lifecycle-grace-ledger-scope`: `git status --porcelain --ignored -uall` listed untracked `.codescout/native-provenance-v1.counter`, `.codescout/native-provenance-v1.lock`, `.superpowers-fix-report.md`; ignored `.codescout/private-memories/`, `.codescout/write.lock`, `.codescout/write.lock.holder`, `.superpowers/`, `scripts/__pycache__/`, `target/`. `--force` removal left no directory this time.

## Hypotheses tried

None yet. To try: which tool call first creates the two `native-provenance-v1` files; whether `.codescout/embeddings` was a symlink; what recreated `.codescout` and `target` inside `tracker-lifecycle-plan-b` while it was being removed (a pinned `workspace=` call, a background job, or a leased build slot).

## Fix

Proposals, none applied:

1. Add `/.codescout/native-provenance-v1.counter` and `/.codescout/native-provenance-v1.lock` to `.gitignore`, with the crates twin rules that the gate `every_root_codescout_ignore_rule_has_its_crates_twin` requires.
2. Brief templates for subagents must name scratch files only under `.superpowers/` (git-ignored). Applies to the superpowers workflow, not to this repo.
3. Consider a documented safe-removal recipe or a helper that checks `git status --porcelain --ignored`, removes only regenerable files, and then runs `git worktree remove`.

## Tests added

N/A — not fixed. A regression check would be a test that creates a worktree, runs a codescout write tool in it, and asserts `git status --porcelain` is empty.

## Workarounds

`git worktree remove --force <path>` after `git status --porcelain --ignored -uall` shows only regenerable files. If git answers "Directory not empty", confirm with `git worktree list` that git deregistered it, then `rm -rf` the exact path and run `git worktree prune`.

## Resume

Not taken. Start with fix 1, which is one `.gitignore` edit plus its twin rules.

## References

- `.gitignore` lines 7, 80, 210; `src/tools/native_provenance_fence.rs` (`LOCK_NAME`).
- `docs/trackers/tracker-organization-session-log.md` entry W-3 (the work that was cleaned up).
- Memory `worktree-merge-catalog-reconciliation` covers merging worktrees, not removing them.
