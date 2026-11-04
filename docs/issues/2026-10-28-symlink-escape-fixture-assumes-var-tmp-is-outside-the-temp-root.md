---
id: 78b8d700fddbd59e
kind: bug
status: fixed
title: Symlink escape fixture assumes /var/tmp is outside the active temporary root
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- testing
- path-security
closed: 2026-11-04
opened: 2026-10-28
severity: medium
unverified: Linux only. macOS was not run (the test is cfg(unix); /var/tmp canonicalizes to /private/var/tmp there). The /opt fallback candidate was not exercised. Before the fix the failure reproduced on experiments with TMPDIR=/var/tmp, as the record said.
---

# BUG: symlink escape fixture assumes /var/tmp is outside the active temporary root

## Summary

util::path_security::tests::symlink_write_escape_caught fails with TMPDIR=/var/tmp. It creates a symlink to /var/tmp and expects a write through it to be rejected, although the launcher selected that same directory as the temporary write root. With a short private TMPDIR child under /var/tmp, the unchanged lean binary passes the exact test.

## Class fit

This is cluster/repro-env-diverges-from-gate-env: a fixture's hardcoded outside directory collides with a launcher-controlled allowed root. Unlike the long-path socket and byte-sweep failures, directory identity rather than string length invalidates this test's premise.

## Symptom (Effect)

The lean full-gate lane with TMPDIR=/var/tmp failed at src/util/path_security.rs:3570 with assertion failed: result.is_err(). All other active lean tests passed in that run: 5239 passed, 1 failed, 11 ignored.

## Reproduction

At research worktree HEAD 0cb5ca9f48791ff81f769de48b4679d2f78a7bfa, run the lean test binary with TMPDIR=/var/tmp and filter util::path_security::tests::symlink_write_escape_caught.

Observed failure: full gate short-gate.log. Control: the same lean binary with TMPDIR=/var/tmp/ztn9txwp passed the exact test.

## Environment

Linux; research worktree /tmp/codescout-real-task-20261024. The alternate temp root was needed because /tmp/.git existed. That unrelated marker was not modified.

## Root cause

src/util/path_security.rs::tests::symlink_write_escape_caught hardcodes the link target /var/tmp and comments that it is outside the project and /tmp. validate_write_path obtains WriteEnv::ambient; ambient_with obtains its temp_dir from crate::platform::temp_dir(). With the chosen TMPDIR exactly equal to /var/tmp, the test's target is no longer outside the active temporary root.

This is not evidence of a write escaping its configured allowlist.

## Evidence

Main checkout .codescout/resident-private/producer-status-pilot-20261028 contains short-gate.log, private-short-probe-symlink.log and private-short-probes.json. The latter records the actual private TMPDIR and probe exit zero.

## Hypotheses tried

Move TMPDIR from /var/tmp to its short private child /var/tmp/ztn9txwp. The exact test passes without source changes.

## Fix

Applied 2026-11-04 in `src/util/path_security.rs`. The test `symlink_write_escape_caught` now links to `a_real_dir_outside_the_temp_root()`: the first of `/var/tmp`, `/usr`, `/opt` (canonicalized) that is neither inside nor above the canonicalized `crate::platform::temp_dir()`, the dir the validator reads. The assertion `result.is_err()` is unchanged, and the validator is unchanged.

## Tests added

No new test. The existing test was repaired, and its two branches were run on `experiments` (`scripts/with-slot.sh cargo test --lib util::path_security::tests::symlink_write_escape_caught`):

| TMPDIR | before | after |
|---|---|---|
| default (`/home/marius/.cache/cc-tmp`) | passes | passes |
| `/var/tmp` | fails at the `is_err` assertion | passes (the helper picks `/usr`) |

The whole `util::path_security::` module passed with `TMPDIR=/var/tmp`: 366 passed, 0 failed. `TMPDIR=/usr` was also tried and fails earlier, at `tempdir().unwrap()`, because `/usr` is not writable; that is a bad TMPDIR, not this bug. The `/opt` candidate was not exercised.

## Workarounds

Use a short private temporary directory whose allowed root does not include /var/tmp itself.

## Resume

Fixed for the reported case. Open only for platforms: the test is `cfg(unix)`; macOS (where `/var/tmp` canonicalizes to `/private/var/tmp`) was not run.

## Fix provenance

- **SHA:** `7a96d9b09` (`experiments`)
- **patch-id:** `87330b35922605d334132eee24b688b50314200b`

Gate: `PYTHON_LIGHT=0 ./scripts/gate.sh` in a detached worktree at `7a96d9b09` exited 0 (FMT 0, CLIPPY 0, LEAN 0, DEFAULT 0; 5998 and 8860 tests passed). A first gate run in the shared tree had 9 failures in `engines`, `prompts::guide_index` and `tools::session_key`, all in files carrying a live peer session's uncommitted edits; none were in `path_security`, and the clean worktree run did not reproduce them.

## References

- The fresh pilot and gate record exists only on the research branches, in commit `1023edc17` (`docs/research/2026-10-28-fresh-producer-status-pilot.md`); it is not on `experiments`. Read it with `git show 1023edc17:docs/research/2026-10-28-fresh-producer-status-pilot.md`.
