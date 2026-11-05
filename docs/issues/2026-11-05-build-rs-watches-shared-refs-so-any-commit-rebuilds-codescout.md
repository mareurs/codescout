---
id: f8d06297d629928e
kind: bug
status: open
title: 'BUG: build.rs watches the shared git refs, so a commit on any branch re-runs the build script and rebuilds codescout in every checkout and worktree'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-05
severity: med
unverified: The cause is read from build.rs and the rebuilds match the commit times, but no controlled experiment (one commit, one rebuild) was run. The size of the cost on the main checkout, where the shared index changes on every git add, was not measured.
---

# BUG: build.rs watches the shared git refs, so a commit anywhere rebuilds codescout

## Summary
`bake_git_sha` in `build.rs` puts `cargo:rerun-if-changed` on `HEAD`, `index`, `refs` and `packed-refs`, each resolved with `git rev-parse --git-path`. In a linked worktree `HEAD` and `index` are the worktree's own, but `refs` and `packed-refs` are the COMMON git directory's. A commit on any branch, by any session, therefore re-runs the build script of every checkout and worktree that shares the repository, and Cargo rebuilds the `codescout` crate. On the main checkout the shared `index` has the same effect for any `git add`.

## Class fit
A build input is declared wider than what the build output depends on. I did not read the cluster roster, so `cluster/unclassified` is used until a class is adjudicated; this is not a claim that no class fits.

## Symptom (Effect)
A gate that should be warm recompiles the main crate. Measured 2026-11-05 in worktree `rust-1.99.1` (frozen HEAD `6bd3745e4`), same toolchain, same slot, lanes of `scripts/gate.sh --check`:

| run | `Finished` of the test builds | CLIPPY lane | TOTAL |
|---|---|---|---|
| warm1 / warm2, a commit landed on `experiments` since the slot was last built | 52.8 s / 54.8 s lean, 1m32 / 1m30 default | 72 s / 74 s | 339 s / 341 s |
| warm3, no commit since the previous run | 0.2 s, 0.2 s | 1 s | 151 s |

About 3.5 minutes per gate run, on a gate that is 151 s when warm.

## Reproduction
1. In a worktree, run the gate twice in a row. The second run is warm (`Finished` in under a second).
2. From any other checkout of the same repository, make a commit on any branch.
3. Run the gate in the worktree again. `Compiling codescout v0.15.0 (...)` appears and the lanes take minutes.

Steps 1 and 3 were observed in the 2026-11-05 runs; the commit in step 2 was made by other sessions, not by me. I did not repeat the sequence as one controlled experiment.

## Environment
Branch `experiments`, `build.rs` at the state of 2026-11-05, rustc 1.97.1 and 1.99.1, shared repository with linked worktrees and several concurrent sessions.

## Root cause
`bake_git_sha` (`build.rs` lines 55 to 94, read in full). The loop `for name in ["HEAD", "index", "refs", "packed-refs"]` prints `rerun-if-changed` for the path `git rev-parse --git-path <name>` returns. Cargo watches a directory by the newest mtime inside it, so any new or updated loose ref under `refs/` re-runs the script. The baked values are `CODESCOUT_GIT_SHA`, `CODESCOUT_GIT_SHA_FULL` and `CODESCOUT_GIT_DIRTY`. Cargo treats a re-run build script as newer than the crate that depends on it, so the crate is rebuilt, whether or not the values changed. No environment override exists in the function.

## Evidence
- Commit times on `experiments`: 14:49:58, 15:00:24, 15:07:02 on 2026-11-05. Rebuilt runs: warm1 and warm2 of both toolchains (14:58 to 15:21). No rebuild: warm3 of both toolchains (15:22 onward, after the last commit).
- `Finished` lines per run are in the logs under the session scratchpad `tc-ab/logs/` (not committed). Summary: codescout memory `research/rust-toolchain-upgrade-20261105`.

## Hypotheses tried
1. **Hypothesis:** the rebuilds come from the compiler change. **Verdict:** rejected for this effect. Both toolchains rebuilt in warm1 and warm2 and neither in warm3, in their own slots.
2. **Hypothesis:** the rebuilds come from the sccache switch between cold and warm phases. **Verdict:** not excluded by an experiment, but the pattern (rebuild, rebuild, no rebuild) follows the commit times, not the switch, which happened once.

## Fix
Not done. Candidate: watch `HEAD` and only the ref `HEAD` points to, not the whole `refs` directory or `packed-refs`. The baked SHA must still change when this checkout commits, and must be right for a detached HEAD, for a packed ref and for a worktree. Needs a test that makes a commit and checks the baked values change, and one that makes a commit on another branch and checks they do not.

## Tests added
None.

## Workarounds
Run the gate right after the last commit of the day, or accept the cost. Measure speed only from runs whose `Finished` lines are under a second.

## Resume
Read `build.rs` `bake_git_sha`, then decide the watch set. Related tracker: `docs/trackers/rust-toolchain-upgrade.md`, row 2.

## References
- `docs/trackers/rust-toolchain-upgrade.md`
- codescout memory `research/rust-toolchain-upgrade-20261105`
