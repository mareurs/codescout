---
id: 6708cab25f53b797
kind: bug
status: archived
title: 'BUG: join''s git-log parser reads a merge commit''s trailer block as its file list, so session_id is NULL'
tags:
- cluster/addressing-without-an-escape-hatch
- measure
- git
- parser
closed: 2026-09-28
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

Fixed in Task 14 (spec Amendment 7, R110) at `cfe231272789da2ccb485ca5e0565c0d143f5a76`, patch-id `62ffc1a63c86156f07347fcc7ecf6b57c6274c52` (`git show cfe23127 | git patch-id --stable`), on `experiments`.

`_run_git_log` now writes an explicit end-of-message marker (`%x1e`, `_GIT_BODY_END`) right after `%B`, and the `--name-only` file list is whatever follows the LAST such marker -- never the last blank line. A merge (git prints no file section for one) and an `--allow-empty` commit therefore have `files == []` and keep their trailer block in the message. `session_id` is read from git's own trailer parser, `%(trailers:key=Session-Id,valueonly)`, as a separate pretty-format field (first non-empty line), the same definition as `miner.session_id_of`. A merge's `files` is always `[]`, not the files the merge brought in.

Real data, fresh scratch freeze of 2026-09-28 (same corpus, base `e22b5640` vs the fix): all 6 merge rows named above now carry a non-NULL `session_id`; `commits.session_id IS NULL` fell 1855 -> 1849; `files_json` changed on 14 commits, all 2-parent merges, every one now `[]` (the other 8 had a non-`Session-Id` last paragraph read as files). The commit set itself is unchanged (4252 = 4252).

## Tests added

`tests/test_measure_join.py`: `GitLogR110::test_a_merge_with_a_trailer_has_its_session_id_and_no_files`, `GitLogR110::test_an_allow_empty_commit_has_no_files`, `OneSessionIdDefinition::test_join_and_miner_agree_on_a_two_parent_merge`. Each was observed RED against the pre-fix module (`None != 'S-MERGE'`; `(None, ['Session-Id: S-E']) != ('S-E', [])`), and the mutant that restores the last-blank-line split (`I9d`) is killed by the final suite.

## Workarounds

None needed after the fix.

## Resume

Done. A Task 10/11 consumer may now read `commits.session_id` on merges; `files_json` is `[]` for every merge.

## References

- `scripts/measure/join.py` (`_run_git_log`, `_build_commits`)
- `docs/superpowers/plans/2026-09-26-system1-base-rate-measurement.md` (Task 6)
