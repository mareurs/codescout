---
id: '65605f410dde3474'
kind: bug
status: fixed
title: 'Codex: Stage 2 freeze asserts pre-segmentation counts instead of emitted positives'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-09-25
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

Found by the review, whose probe was reproduced by the registering session (session `571eb3d6`). Class: IC-24, `value-correct-in-a-frame-its-name-does-not-state`: correct as an item count, published as a row count. Related review: `docs/research/2026-09-25-codex-stage2-freeze-review.md`.

## Fix

**Fixed in `da67db02`, patch-id `99a0f1838c3548255280b22d3ed134fb018fe7e6`.**

- `check_menu_positives` asserts that every menu rule has at least 50 positive rows in what is actually written to train.
- Per rule, emitted positive rows = items − positive rows dropped as not one unit.
- Both run before any file is written.
- The review's probe (every positive train row dropped in memory) is now refused, naming each emptied rule.
- A normal re-run reproduces all seven frozen hashes byte for byte. The data and the 14-rule menu are unchanged.

## Tests added

`tests/test_stage2_synthetic.py`, class `FreezeMenuGuard`: exactly 50 passes; 49 raises; a menu rule with no positive rows raises; negatives do not count; off-menu positives do not count.

One mutation per guard site, each run in an isolated worktree, each killed: counting negatives (2 fail), moving the boundary by one (3 fail), and counting off-menu rules (1 fail).

**Not unit-tested:** the per-rule reconciliation assert inside `main`. It is exercised only by the real freeze run, which passes it for all 14 menu rules.
