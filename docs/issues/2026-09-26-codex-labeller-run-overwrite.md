---
id: a7f3a7c8ad7d352a
kind: bug
status: fixed
title: 'Codex: labeller relaunch spends again and overwrites prior evidence'
tags:
- codex
- phase1b
- cluster/declared-not-wired
closed: 2026-09-26
severity: medium
---

# Codex: relaunching the labeller overwrites a previous run

**Valid:** dated 2026-09-26

## Summary

The expensive Phase 1b labeller has no refusal for an already used output directory. Re-executing it starts new model calls and overwrites previous raw answers, label files and run-header provenance. This undermines the registered one-run / one-retry limit and makes accidental relaunches costly and difficult to audit.

## Symptom (Effect)

A fake-model invocation into a directory containing prior labels, a run header and a raw batch answer returned exit 0, started all 21 Claude batches, and replaced both the prior labels and the prior raw answer.

## Reproduction

Run the persisted review probe:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/reproduce.py
```

Its `overwrite` scenario creates sentinel previous outputs in a temporary audit directory and invokes the real main path with a fake Claude implementation. External subprocesses are blocked. Shared audit inputs and outputs are untouched.

## Environment

Reviewed at checkout HEAD `16cef832`; runner from `d423236b`. Linux/Python, standard-library fake-model probe, `--only claude --workers 4`.

## Root cause

`run_labellers.py:169` accepts an existing raw directory via `mkdir(exist_ok=True)`. `run_call:79` and `:83` reuse attempt filenames. `main:196` and `:198` overwrite labels and the run header. There is no early exclusive reservation or existing-run refusal.

A related recovery trap is visible in the source: successful Codex labels remain in memory until all Claude batches finish. A terminal Claude failure writes no labels; blindly relaunching the full command would call Codex again as well. This latter path was read, not separately measured.

Measured 2026-09-26 by the reproduction above: 21 fresh fake calls, prior raw overwritten=true, prior labels overwritten=true, exit_code=0.

## Evidence

`docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/runner-results.json`, `overwrite`.

The production audit directory contained only items.jsonl, key.jsonl, menu.json and sample.jsonl at inspection. No actual prior labels were overwritten; this is a reproduced failure mode to close before the first real run.

## Hypotheses tried

An existing completed-run artifact might prevent model calls. Falsified: the runner made calls and replaced it.

## Fix

Implemented `reserve_run`: refuse any prior raw directory, run header or label file; create the header with exclusive mode `x` before model calls, so simultaneous prechecks cannot both claim the run. Labels also use exclusive creation. The header records running/completed/stopped state. Stopped or interrupted runs stay reserved; no automatic resume or resetting of attempt budgets was introduced. `--only` selects a standalone invocation, not two separate writes into one run destination.

**Committed** in `14346eb4` (patch-id `c453deb0228aff223e47e6346ee778cdcab66d6f`) by the integrating session (571eb3d6-c879-43f6-b3f9-5a51e744e1af), before any labelling run.

## Tests added

The new LabellerMain tests cover every prior artifact class, nested entry while a run is in progress, and a controlled race where both starters first observe no header. They require zero model calls on refusal and preservation of prior evidence. The complete Python audit file passes 43 tests. All eight applied mutations against the nine new tests were killed, including skipped reservation and non-exclusive creation; saved in `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/fix-verification.json`.

**Re-verified at commit with `scripts/mutation-probe.sh`:** exclusive creation of the labels file survived, because the reservation already refuses labels present at the start. A new test plants a labels file mid-run (`test_labels_are_never_overwritten_even_when_one_appears_mid_run`); the mutation is now killed. 45 tests; 59 of 60 mutations killed over the Step 1-2 scripts.

## Workarounds

Do not relaunch the command after partial failure. Inspect preserved raw artifacts and the registered stop rule first. A fresh output directory alone does not justify exceeding the registered attempt budget.

## Resume

The correction and targeted checks are complete in the working tree. Incorporate the diff and `docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md` into experiment provenance before the real job. Codex made no commit; the integrating session committed it in `14346eb4` (patch-id `c453deb0228aff223e47e6346ee778cdcab66d6f`).

## References

- `docs/evals/data/2026-09-24-rule-tell/phase1b/run_labellers.py`
- `docs/evals/phase1b-local-classifier-preregistration.md` § Step 1.
