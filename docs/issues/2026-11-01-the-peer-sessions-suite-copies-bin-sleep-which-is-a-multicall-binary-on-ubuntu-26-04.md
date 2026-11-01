---
id: 546c44c88c8b1343
kind: bug
status: open
title: 'BUG: tests/peer-sessions.sh copies /bin/sleep, which is a multicall binary on Ubuntu 26.04, so the copy exits and 9 of 22 cases read "?"'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- shell
opened: 2026-11-01
severity: medium
unverified: Verified RED and GREEN in a clean ubuntu:26.04 container (7 failures before, 0 after) and GREEN on this machine; the container showed 7 failures where the runner showed 9, so the 2 extra runner failures are not reproduced. The runner result after the fix is not yet observed.
---

## Summary

`tests/peer-sessions.sh` builds its fixture processes by copying `/bin/sleep` to a new name and running the copy. On the `ubuntu-latest` runner (now Ubuntu 26.04) `/bin/sleep` is a symlink to the uutils `coreutils` multicall binary, which picks its program from `argv[0]`. The copy, named `prog`, prints `coreutils: unknown program 'prog'` and exits at once. No fixture process exists, so every case that reads `/proc/<pid>` reports `?`. The CI job `Shell suites (hooks, provenance, peer-sessions)` ended with `passed=13 failed=9`.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the suite passes on this machine, where `/bin/sleep` is GNU coreutils, and fails on the CI image, where it is not. It differs from the class's other members in that the divergence is the base image of the runner (`ubuntu-26.04` on both CI runs read; this machine runs a different distribution), not the files or variables of a developer.

## Symptom (Effect)

CI run 41322768679, job `Shell suites (hooks, provenance, peer-sessions)`, step `Run bash tests/peer-sessions.sh`: 9 lines `FAIL`, each with `got=[?]` where `current`, `prog`, `REPLACED`, `ok` or `STOPPED` was wanted, or `got=[]` for the newest `codescout` child. The summary line is `passed=13 failed=9`.

## Reproduction

`docker run --rm -v "$PWD":/w:ro -w /w ubuntu:26.04 bash tests/peer-sessions.sh`

Before the fix the container printed `coreutils: unknown program 'prog'`, `coreutils: unknown program ''a) T b''` and `coreutils: unknown program 'codescout'`, and ended with `passed=15 failed=7`. After the fix: `passed=22 failed=0`.

## Environment

Runner image `ubuntu-26.04`, version 20261025.179.1 (from the job log). In the container, `/bin/sleep -> ../lib/cargo/bin/coreutils/sleep`.

## Root cause

A multicall binary dispatches on the name it was started under. Copying it under another name removes the dispatch key. The suite assumed `/bin/sleep` is a stand-alone executable.

## Evidence

The container run above. The job log lines with `want=[current] got=[?]`.

## Hypotheses tried

None needed: the container reproduced the exact message.

## Fix

The fixtures are copies of `bash`, which chooses nothing from its name. Each parks without a child process: `read -t 300 <>"$T/hold"` on a FIFO opened read-write waits for its timeout, so the process id the test reads is the id of the copy itself. A comment in `tests/peer-sessions.sh` states why. Fix SHA is added when the fix is on `experiments`.

## Tests added

None new: the suite is the test. RED is the container run before the change and the CI job; GREEN is the container run after it.

## Workarounds

None.

## Resume

1. Read the `Shell suites` job on the first CI run containing the fix. Expect 22 passed.
2. Two of the 9 runner failures did not reproduce in the container (7 there). If the job still fails, those two are the first place to look.

## References

- CI run 41322768679.
- `tests/peer-sessions.sh`, `scripts/peer-sessions.sh`.
