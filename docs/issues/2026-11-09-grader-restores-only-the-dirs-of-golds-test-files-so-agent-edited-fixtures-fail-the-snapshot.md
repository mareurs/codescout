---
id: '9acb2c379cf66505'
kind: bug
status: open
title: 'setup-ab grader: restore_paths covers only the directories of the gold patch''s test files, so an agent-edited fixture fails a snapshot test (ruff-15626)'
tags:
- cluster/selector-narrower-than-its-population
opened: 2026-11-09
---

## Summary

The grader of the Setup A/B study restores only the directories it derives from the gold patch's test files (`baseline.restore_paths`, `tools/setup-ab-eval/baseline.py:71-93`). For ruff-15626 the gold patch changes only snapshot files, so only `.../flake8_simplify/snapshots` is restored. The prompt tells the agent to verify with local checks, and agents add cases to the fixtures `SIM201.py` and `SIM202.py`. Those fixtures are not restored, the snapshot then differs, and the run fails. Audit A reports that in all 8 runs of the task, pass or fail depends only on whether the agent edited the fixtures: edited (4 Opus runs and Sonnet full) all fail, not edited (3 Sonnet runs) all pass.

## Class fit

The restore selector is built from the files the gold patch touched, which is narrower than the set of test files an agent may legitimately touch while following the prompt. The class `IC-18` is a selector narrower than the population it names. This member differs from the class's tool-facing members: the selector lives in a grader, and the population it misses (fixtures that feed a snapshot) is only reachable through a second file type.

## Symptom (Effect)

A correct fix fails the task when the agent also adds a fixture case. Results for ruff-15626 do not measure the arms. The bare Opus run also failed it, so the task cannot separate the arms either way.

## Reproduction

Read `grades/*/grade.log` of `astral-sh__ruff-15626.*` under `study/runs`; compare the original cases with gold's snapshot (audit report A.md, section ruff-15626, quotes events 339 and 344 of Sonnet full). The code path: `restore_paths(test_files)` returns the directory named by the first matching rule (`tests`, `resources/test/fixtures`, or `src/rules/<x>/<y>/snapshots`) for each of gold's test files only.

## Environment

`tools/setup-ab-eval/baseline.py` at the study commit; task `astral-sh__ruff-15626`.

## Root cause

Verified at the code: `restore_paths` takes `test_files` from gold's patch only. Verified by audit A (not re-run by the controller): the failing grade logs differ from gold only by the agent's added fixture lines.

## Evidence

`tools/setup-ab-eval/baseline.py` lines 71-93 (read 2026-11-09); audit report `/home/marius/.claude-kat/jobs/fbe38184/tmp/audit/A.md`.

## Hypotheses tried

Not verified: whether restoring the fixtures would make the 15626 patches pass.

## Fix

Open. Candidate: also restore the fixture directory that feeds each snapshot directory (for ruff `resources/test/fixtures`), or restore every path the task's own test command reads. Changing it changes grading, so it needs a new grader version and a re-grade of saved patches, not an edit of old results.

## Tests added

N/A — not fixed yet.

## Workarounds

Treat ruff-15626 as not measuring the arms; the batch-1 corrections file does not cover this study.

## Resume

Write a failing test in `test_baseline.py` for a gold patch that touches only snapshots, then widen `restore_paths`.

## References

Codescout memory `research/setup-ab-audit-cs-usage-20261109`.
