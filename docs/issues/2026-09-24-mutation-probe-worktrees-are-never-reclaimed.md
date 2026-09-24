---
id: '84b3e8d0960b439e'
kind: bug
status: open
title: 'BUG: mutation-probe.sh keys its isolated worktree on the session id and never removes it — 72G across 18 trees'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-09-24
related:
- docs/issues/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
severity: medium
---

# BUG: mutation-probe.sh keys its isolated worktree on the session id and never removes it — 72G across 18 trees

## Summary

`scripts/mutation-probe.sh` creates one linked worktree per Claude session, at `<repo>.worktrees/mutation-<sessionId>`, and nothing ever removes it. cargo builds inside the worktree, so each tree holds a full `target/`. This is the sibling of `docs/issues/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md` (fixed in `3591f2ca`): the same key, and the same missing teardown.

## Symptom (Effect)

Measured 2026-09-24, about 19:45 local, with `du -sh <repo>.worktrees/mutation-*`:

```
18 trees, 72G total: 15 of 3.5-7.5G each, plus 3 of 82-93M
```

Every linked worktree is also named in codescout's `_workspace_notice`, which rides on each tool response in the main checkout until a project is explicitly activated. So the population also grows the per-call output: 19 paths at measurement.

## Reproduction

Run `./scripts/mutation-probe.sh … -- <cmd>` from any session. `<repo>.worktrees/mutation-$CLAUDE_CODE_SESSION_ID` remains after the session exits, and `git worktree list` shows it indefinitely.

## Environment

Linux, `experiments` @ `e60426a9`, several sessions sharing this checkout across several profiles.

## Root cause

`scripts/mutation-probe.sh:153` sets `TREE="${ROOT}.worktrees/mutation-$SID"`, keyed on the session id. The script reuses the tree within a session, which gives it a warm build, but has no teardown and no reuse across sessions. It sets no `CARGO_TARGET_DIR`, so the build tree lives inside the worktree. That last point is inferred from grepping the script, not measured on a run.

## Evidence

`du` as above. The population has grown: `docs/issues/archive/2026-09-18-the-worktree-write-block-names-an-arbitrary-worktree-as-the-remedy.md` § *Population* counted **11** `mutation-<sessionId>` worktrees on 2026-09-18; there were **18** on 2026-09-24.

## Hypotheses tried

N/A.

## Fix

Not started. Candidates:

- **Reuse across sessions** with the same leased-slot pattern as `scripts/gate.sh`: a non-blocking `flock` on an inherited fd, first free slot, never `-o` (`bug-fix-session-log:F-173`). This is the second instance of the pattern; a shared helper is worth considering only at a third.
- **Reap on entry:** remove the worktrees of sessions positively known dead. This depends on the liveness inference that `docs/issues/archive/2026-09-17-a-full-disk-truncates-a-live-sessions-registry-row-so-provenance-reports-it-dead.md` shows fails under disk pressure, so it is the weaker option.
- Any one-time cleanup of existing trees must use `git worktree remove`, never a bare `rm`, so git's worktree registry stays consistent.

## Tests added

None yet: open.

## Workarounds

`git worktree remove --force <path>` for a tree whose session is known dead, then `git worktree prune`.

## Resume

Nothing claimed.

## References

- `scripts/mutation-probe.sh:150-155`
- `docs/issues/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`, the sibling
