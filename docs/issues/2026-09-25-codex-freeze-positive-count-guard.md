---
id: '65605f410dde3474'
kind: bug
status: open
title: 'Codex: Stage 2 freeze asserts pre-segmentation counts instead of emitted positives'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-25
owner: marius
severity: medium
---

# Stage 2 freeze asserts the wrong population

**Valid:** dated 2026-09-25

**Observed at:** `d643c001684fe675fc682acffee6f1d817bdcc94`.

## Finding

`docs/evals/data/2026-09-24-rule-tell/stage2/freeze_stage2.py:114` increments `count[rule]` before `row()` can reject an unsegmentable target. The assertion at lines 145–146 compares that pre-emission count with `trainable.json`, not the positive rows in `sets['train']`. Both the module docstring and preregistration freeze section describe it as verifying frozen positives.

The committed data already differs: `closed_population` 52 planned / 51 emitted; `d_adjacency` 55 / 54; `d_sessionid` 72 / 70; `question_asked` 78 / 76. All 14 rules still have at least 50 positives, so this does not invalidate the current menu. The guard cannot enforce that invariant on subsequent freezes.

## Observed probe

Imported the current module and redirected OUT into a temporary directory under /tmp. Baseline returned 0 and all six JSONL files were byte-identical to the committed freeze. Then replaced only the in-memory row function with a wrapper returning None for every positive train row, leaving all other rows unchanged. The mutated run returned 0 and emitted zero train positives; its assertion did not fail. One candidate mutation applied, one surviving. No repository source or frozen data was changed; no model calls or test suites ran.

## Expected correction

Count emitted positive rows after segmentation and assert the minimum per menu rule before writing output. Keep pre-segmentation counts separately, reconciling the delta with explicit drop counts rather than asserting false equality. Update the freeze claim to describe the actual predicate. Add a narrow regression that makes row emission lose enough positives to fall below the threshold and requires freeze refusal.

## Scope

Review finding only; no fix applied. Cluster classification pending. Related review: docs/research/2026-09-25-codex-stage2-freeze-review.md.
