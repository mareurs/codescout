---
id: 60d07b752885b353
kind: bug
status: fixed
title: 'BUG: build.rs watches the shared git refs, so a commit on any branch re-runs the build script and rebuilds codescout in every checkout and worktree'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-07
opened: 2026-11-05
severity: med
unverified: The rebuild-cost benefit (cargo Finished lines in a sibling worktree after a commit elsewhere) was never measured, only the watch set. After git gc (pack-refs --all --prune) every checkout whose branch has had no commit since falls back to watching refs/heads recursively until its next commit. Reftable repositories are unsupported. A symlinked ref is handled by reasoning only.
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
- Fix verification, 2026-11-07: `./scripts/gate.sh --check` on `fix/open-issues-batch-20261107-r2` at `af3e2a9ec`: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0. Details and gate coverage limits in `## Fix`.

## Hypotheses tried
1. **Hypothesis:** the rebuilds come from the compiler change. **Verdict:** rejected for this effect. Both toolchains rebuilt in warm1 and warm2 and neither in warm3, in their own slots.
2. **Hypothesis:** the rebuilds come from the sccache switch between cold and warm phases. **Verdict:** not excluded by an experiment, but the pattern (rebuild, rebuild, no rebuild) follows the commit times, not the switch, which happened once.

## Fix
Fixed on branch `fix/open-issues-batch-20261107-r2`. All of the commits below are now on `experiments` at `143c1941f`, which is the same SHA because the branch reached `experiments` by fast-forward. `./scripts/gate.sh --check` on that identical tree: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0.

**Where the change lives:** `build.rs`, function `bake_git_sha` (line numbers: Unknown). It now watches `HEAD`, `index` and ONLY the ref `HEAD` points to (`git symbolic-ref -q HEAD`, then `git rev-parse --git-path <ref>`), instead of the shared `refs` directory and `packed-refs`.

**What it does:**
- Detached HEAD: watches `HEAD` and `index` only.
- Branch has a loose ref: watches that one ref file.
- Branch has no loose ref (packed or unborn): watches `packed-refs` if it exists, plus the nearest existing ancestor directory of the missing loose ref (normally `refs/heads`).

**Plan deviation:** the candidate above said to watch `packed-refs` for a packed ref. That alone would have missed a commit on a packed branch, because such a commit creates the loose ref and leaves `packed-refs` untouched. The nearest existing ancestor directory of the missing loose ref is watched as well. The existing cargo-incremental tests in `tests/test_build_git_dirty.py` exercise this case.

**Commits (labelled with the branch they were made on; all are now on `experiments`, same SHAs):**
- d5ef8e0d81472d509bbb7b2d47cfe8cea86d475b (branch fix/open-issues-batch-20261107-r2) / patch-id c7da7534164dbd2f4f8b29671f63c08419d4f0ce: the fix.
- 631a804662b1f6da98db61e8b5ce7699f3217858 (branch fix/open-issues-batch-20261107-r2) / patch-id 257478350380dfcc4643a8745b189c6515da7223: the packed and unborn tests assert the exact watch set.
- daffefaf938c889208bdaacbafd60e10e6fd691a (branch fix/open-issues-batch-20261107-r2) / patch-id 1383319d70ff8f30a3bcdba17832c408f7a2cb31: the comment names `git gc` and `pack-refs --prune`.

**Verification:** `./scripts/gate.sh --check` on `fix/open-issues-batch-20261107-r2` at its tip `af3e2a9ec`, 2026-11-07: FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover: the three heavy Python suites, server-stack, librarian code in the lean lane, the local ONNX weight tests. Each fix in the batch was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

**Not measured:** the rebuild-cost benefit itself (cargo `Finished` lines in a sibling worktree after a commit elsewhere). Only the watch set was checked. See the `unverified` field for the other limits.

## Fix provenance

- **SHA:** `d5ef8e0d81472d509bbb7b2d47cfe8cea86d475b` (`experiments`)
- **patch-id:** `c7da7534164dbd2f4f8b29671f63c08419d4f0ce`

## Tests added
Five tests in `tests/test_build_git_dirty.py` (line numbers: Unknown), all red before the fix:
- a linked worktree watches only its own branch ref;
- detached HEAD;
- packed branch;
- unborn branch;
- a commit on the checked-out branch changes the baked SHA.

The whole file passes: 29 tests, 18 subtests, at the fix commit. The packed and unborn tests assert the exact watch set (commit 631a804662b1f6da98db61e8b5ce7699f3217858, patch-id 257478350380dfcc4643a8745b189c6515da7223). No test makes a commit on another branch and checks that no rebuild follows; the worktree test asserts the watch set instead.

## Workarounds
Run the gate right after the last commit of the day, or accept the cost. Measure speed only from runs whose `Finished` lines are under a second.

## Resume
The fix landed on `experiments` (see `## Fix` and `## Fix provenance`), `./scripts/gate.sh --check` was green on that tree, and this file was archived into `docs/issues/archive/`. Optionally measure the benefit: commit in one checkout, then compare the `Finished` lines of a gate run in a sibling worktree. Related tracker: `docs/trackers/rust-toolchain-upgrade.md`, row 2.

## References
- `docs/trackers/rust-toolchain-upgrade.md`
- codescout memory `research/rust-toolchain-upgrade-20261105`
