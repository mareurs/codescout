---
id: '3c919d925fa383fe'
kind: bug
status: open
title: worktree-remove suite failed once in the gate with exit 2 before any signal; the cause is not established
tags:
- cluster/unclassified
opened: 2026-11-12
owner: marius
related:
- '3dde1548c3e5c310'
severity: low
unverified: The cause is not established. One failing run, not reproduced alone, under 128 busy loops, or in the next gate run. The script's stderr for the failing case was not captured.
---

# BUG: worktree-remove suite failed once in the gate with exit 2 before any signal; the cause is not established

## Summary

In one of two gate runs on branch `feat/lifecycle-cleanup-20261112`, the Rust wrapper test `worktree_remove_script::the_worktree_remove_suite_passes` failed in the DEFAULT lane. One case, `stop_ends_a_stoppable_holder_and_removes`, saw the script exit 2 where it expects 0. The same tree passed in the next gate run, and the suite passes alone. The branch only changed `src/librarian/lifecycle/`, which this test cannot reach.

## Class fit

Filed as `cluster/unclassified`. The failing assertion reports only a wrong exit code, and the test hides the script's own stderr, so no class trait can be matched yet. Revisit once the stderr of a failing run is captured; the nearest candidate is the class of load-sensitive wall-clock windows in the related oversubscription issue.

## Symptom (Effect)

Gate run 1 (merge commit of the branch with `experiments`, 2026-11-12, 7 m 56 s), DEFAULT lane: `FAIL [37.389s] codescout::worktree_remove_script the_worktree_remove_suite_passes`, `9783 tests run: 9782 passed, 1 failed`. The case printed four assertion failures: `exit 0` (want 0, got 2), `the holder is gone` (want no, got yes), `the directory is gone` (want absent, got present), `git no longer lists it`. Every earlier case of the suite passed, including `holders_without_stop_signal_nothing`, which also uses a real holder and a real `/proc` scan.

## Reproduction

Not reproduced. Run 2 on the next commit (only a string in `checks.rs` differed): DEFAULT green, 9783 tests passed.

## Environment

Linux 7.2.15-zen1, 64 cores, 1443 processes during my timings, about 11 other Claude sessions on the machine, load average 33 to 41 before my loads. The gate builds in a RAM tier (zram) and sets a tmpfs `TMPDIR` (`/tmp/codescout-gate-tmp.*`). Python tests run beside the Rust lanes.

## Root cause

Unknown — under investigation.

## Evidence

- The case runs `scripts/worktree-remove.sh --stop --wait 5 <wt>` through `run_real` (the real `/proc`, no fixture), and the holder is still alive afterwards. So the script refused or failed before it sent a signal. Exit 2 comes from these paths in `scripts/worktree-remove.sh`: `die` (argument errors, the scan failing or timing out at line 727, no main checkout) and the three refusals of `resolve_worktree` (lines 182 to 219).
- `tests/worktree-remove.sh` prints only `want` and `got` for a failed `eq`; it does not print `$ERR`. The stderr that names the path is lost.
- Measured afterwards on the same tree: the suite alone passes 266/0 in 28 s; the suite under 128 busy loops on 64 cores passes 266/0; one real `/proc` scan takes 0.31 s with 1443 processes against a 10 s limit. The script runs with `set -uo pipefail`, without `-e`.

## Hypotheses tried

- A scan timeout under CPU load: not supported (0.31 s scan against a 10 s limit; the suite in the gate took 37 s against 28 s alone; CPU load alone did not reproduce it).
- A vanished `/proc/<pid>` aborting the scan through `-e`: not possible as written, since the script has no `-e`. A different failure of the scan child under heavy process churn is not excluded.
- Not tried: a `git worktree list` failure in `resolve_worktree` under the gate's tmpfs `TMPDIR`; memory pressure making `/proc/<pid>/cmdline` reads block on the mmap lock (the script's own header describes this).

## Fix

Proposed, not applied: when an exit-code assertion fails in `tests/worktree-remove.sh`, print `$ERR` beside `want` and `got`, so the next occurrence names its cause. After that, the fix follows the cause.

## Tests added

N/A. Nothing is fixed. The proposed change is to the test's reporting, and is itself the instrument for finding the cause.

## Workarounds

Re-run the gate once. The suite passed alone and in the next gate run.

## Resume

Open. Next step: make a failed `eq` on an exit code print `$ERR` in `tests/worktree-remove.sh`, then watch for a second occurrence in the gate. If it recurs, read the stderr first and match it against the exit-2 paths above.

## References

- Spec `9189852e75f80f08` (the removal helper) and bug `3dde1548c3e5c310` (the helper's own bug).
- `docs/issues/2026-10-31-eight-lib-tests-fail-under-16-to-1-cpu-oversubscription-through-wall-clock-windows.md` — a known family of load-sensitive failures in this gate; not shown to be the same.
- SDD ledger for the lifecycle cleanup, ruling R6 (scratch, not committed).
