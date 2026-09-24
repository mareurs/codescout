---
id: de0b7a1d8e53e685
kind: bug
status: fixed
title: 'BUG: mutation-probe.sh keys its isolated worktree on the session id and never removes it — 72G across 18 trees'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-09-24
opened: 2026-09-24
related:
- docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md
severity: medium
unverified: 'CLEARED 2026-09-24. Was: the mutation-probe-tests CI job had not yet run cases 22-26. It ran green in CI run 36041448098 on ea972b40.'
---

# BUG: mutation-probe.sh keys its isolated worktree on the session id and never removes it — 72G across 18 trees

## Summary

`scripts/mutation-probe.sh` creates one linked worktree per Claude session, at `<repo>.worktrees/mutation-<sessionId>`, and nothing ever removes it. cargo builds inside the worktree, so each tree holds a full `target/`. This is the sibling of `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md` (fixed in `3591f2ca`): the same key, and the same missing teardown.

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

The tree is now leased per RUN from a pool, `<repo>.worktrees/mutation-slot-N`, with the same lock as `scripts/gate.sh`. It takes the first slot whose `mutation-slot-N.lock` a non-blocking `flock` can take, on an fd the test command inherits, deliberately without `-o` (`bug-fix-session-log:F-173`). A later run from ANY session reuses a freed tree warm, and two concurrent runs from one session get separate trees; before this, they shared one, and each run's `reset --hard` reverted the other's mutation.

A second leak was found and fixed in the same change. `cleanup()` never removed `WTPATCH` (the carried working-tree patch), so every isolated run left one file in `/tmp`. On 2026-09-24 there were 398 `/tmp/tmp.*` files beginning `diff --git` plus 244 empty ones, and a single probe run moved the count from 646 to 647.

**Fixed:** `b8b7bf85` on `experiments`, 2026-09-24.
**patch-id:** `44d11cb407376015c7279aad914979445bf34ed4`

Gate green on it: `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`.

## Tests added

`tests/mutation-probe.sh` gained cases 22–26. The suite already runs in CI job `mutation-probe-tests`.

- **22:** a second session reuses the freed tree, and one worktree is left.
- **23:** a concurrent same-session run gets its own tree.
- **24:** SIGKILLing the probe while its command runs keeps the tree leased. A precondition checks that the killed pid is the probe, read from its own marker.
- **25:** a `flock` that exits 127 stops the probe with exit 2 and creates no worktree. It runs under `timeout 20`.
- **26:** an isolated run leaves `TMPDIR` empty.

**Red observed first.** Cases 22–25 gave 53 passed / 6 failed against the old script; every failure was a new assertion, and the new controls passed. Case 26 gave 60/1 before the `WTPATCH` fix. The final run is 61/0.

**Mutations, one per guarded site, all KILLED**, via the probe mutating itself: the outer run executes from the main checkout, and the inner suite runs in the leased tree.

- **N1**, lease never checked: 57/4.
- **N2**, command does not inherit the lease: case 24 fails. This mutant also breaks `--shared` capture, since its `{SLOT_FD}>&-` is a bad redirect where no slot is leased.
- **N3**, never reuse a slot: 59/2.
- **N4**, flock-failure guard removed: 60/1, with the loop bounded by `timeout`.
- **N5**, `WTPATCH` kept: 60/1.

After every mutation run I checked for survivors (hold processes, `/tmp` inodes) and found none.

## Workarounds

`git worktree remove --force <path>` for a tree whose session is known dead, then `git worktree prune`.

**Applied 2026-09-24, with operator approval, to DEAD sessions only.** Liveness was taken from registry rows in every `~/.claude*/sessions/` plus `CLAUDE_CODE_SESSION_ID` in process environments: 29 live ids, and 0 unreadable rows for a live pid. I also checked that no process had its cwd or an open file inside any tree, and that none was `locked`. 11 trees (~42G) were removed and 7 live sessions' trees (~30G) kept. Once no probe runs from a pre-fix copy of the script, nothing uses those 7 either. Removing them is a further operator decision.

**Removed later on 2026-09-24, on operator instruction and after checking that each was merged.** Commits: all 7 were detached HEADs already reachable from `experiments`, with 0 commits of their own. Uncommitted content: of 96 dirty or untracked files, 33 matched neither git history nor the main checkout. Most were old snapshots of peers' in-progress work, carried in by probe runs, but not all could be proved superseded. So each tree's full uncommitted state was saved before removal to `~/.cache/codescout-worktree-salvage/2026-09-24/<sessionId>/`: `HEAD`, `tracked.patch` (`git diff HEAD --binary`) and `untracked.tar.gz`, 2.8M in total. Each was verified by file count and by `git apply --check -R` against its own tree. To restore one: `git worktree add --detach <path> $(cat HEAD)`, `git apply tracked.patch`, `tar -xzf untracked.tar.gz`. Remaining worktrees: the main checkout, `check-codescout-integration` (a named branch, not a probe tree) and `mutation-slot-0`.

## Resume

Nothing claimed.

## References

- `scripts/mutation-probe.sh:150-155`
- `docs/issues/archive/2026-09-24-gate-per-session-target-dirs-are-never-reclaimed.md`, the sibling
