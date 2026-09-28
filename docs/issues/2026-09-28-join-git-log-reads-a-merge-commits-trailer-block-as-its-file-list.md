---
id: '91bafcc7d4137bf9'
kind: bug
status: open
title: 'BUG: join''s git-log parser reads a merge commit''s trailer block as its file list, so session_id is NULL'
tags:
- cluster/addressing-without-an-escape-hatch
- measure
- git
- parser
---

**Valid:** dated 2026-09-28

## Summary

`scripts/measure/join.py::_run_git_log` (Task 6 of the system1 base-rate measurement) runs `git log <sha> --name-only` and splits each commit's text at its LAST blank line, reading what follows as the file list. `git log` prints no file list for a merge commit, so for a merge the last paragraph of the message body — the trailer block — is taken as the file list. The row's `session_id` is then NULL although the commit carries a `Session-Id:` trailer, and its `files_json` holds the trailer lines as "files".

## Symptom (Effect)

In the Task 8 real-data build (window 2026-08-03 → 2026-09-28), **6 of 4240** `commits` rows have `session_id` NULL while `git log -1 --format=%B` shows a `Session-Id:` line: `506924f29453`, `8c795a922658`, `7212dd810ec6`, `4ff09107a3c4`, `4485eeb0adb3`, `5eea93012684`. All six are 2-parent merges. Nothing fails; the row is a valid row with a wrong value.

## Reproduction

At tree `3b0a3b3a`, branch `experiments`: load `scripts/measure/join.py` by path and call `_run_git_log(repo, <full sha>, <its %cI>, <its %cI>)` for `506924f29453` or `4485eeb0adb3`. Each returns one row with `session_id=None` and `len(files)=2`, while its `%B` has exactly one `Session-Id:` line and `%P` has two parents. (Controller probe `ctl_m2_join.py`, session scratchpad.)

## Environment

git 2.55.0; Python 3.13 (`~/work/claude/prompt-engineering/.venv/bin/python`).

## Root cause

`_run_git_log`'s own docstring names it as a *known limitation*: "a commit that touches NO files but carries a multi-paragraph body is indistinguishable from one whose last paragraph IS the file list … Every real commit in this repo's convention touches at least one tracked file". The premise is false for merges: `git log --name-only` diffs a merge against nothing by default, so it prints no file section. The layout has no **disambiguator** between "an empty file list" and "a body paragraph", and the parser resolves the ambiguity the wrong way for every merge whose body ends in a trailer block.

## Evidence

- Found by the Opus task review of Task 8 (`.superpowers/sdd/2026-09-26-system1-base-rate-measurement/task-8-review.md`, M2), measured over the Task 8 R95 events.db.
- Reproduced independently by the controller through `join._run_git_log` itself (above).

## Hypotheses tried

None beyond the reproduction; the docstring already states the mechanism.

## Fix

Not yet fixed. Shape: emit an explicit end-of-body marker in the pretty format (e.g. `%B%x00` then the file list), so the file section is located by the marker rather than by the last blank line. A merge then has an empty file list and its trailers stay in the body. Add a fixture with a two-parent merge carrying a `Session-Id:` trailer. Scheduled for the system1 plan's final whole-branch review fix dispatch (Task 6 is closed).

## Tests added

None yet.

## Workarounds

`scripts/measure/miner.py` is unaffected: it skips merges before reading anything and reads Session-Ids with its own `session_id_of`. Any other reader of `events.db` `commits.session_id` or `files_json` must treat merge rows as unreliable.

## Resume

Fix in the final-review dispatch. Then check whether any Task 10/11 consumer reads `commits.session_id` for merges.

## References

- `scripts/measure/join.py` (`_run_git_log`, `_build_commits`)
- `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md` (Task 6)
