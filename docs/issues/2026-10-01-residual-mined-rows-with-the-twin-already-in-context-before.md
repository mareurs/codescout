---
id: eb9014f896a48715
kind: bug
status: open
title: 'RESIDUAL: 10 mined candidates already carry the twin inside context_before, and nothing flags or excludes them'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
---

# RESIDUAL: 10 mined candidates already carry the twin inside context_before, and nothing flags or excludes them

**Valid:** dated 2026-10-01

## Summary

Remaining work split out of `docs/issues/archive/2026-09-24-stage2-miner-positive-context-is-the-corrected-text.md`, whose second residual was *"in 6 rows the twin already appears on the old side"*. After `98dbd016` re-centred the context windows the figure is **10**. In those rows the correction (the twin) is already present in the text the pipeline uses as the positive's context, so the label-leak the parent bug is about survives for them. The parent's `unverified:` now opens `TRACKED <this file's id>`.

## Evidence

Measured 2026-10-01 on the committed `docs/evals/data/2026-09-24-rule-tell/stage2/mined-candidates.jsonl`, whitespace-normalised:

- 944 rows; 935 have a twin. Twin inside `context_after`: 935 of 935. Positive inside `context_before`: 944 of 944. No row carries the old `paragraph` field.
- **Twin inside `context_before`: 10 rows**, ids (0-based line order) 267, 327, 439, 534, 592, 593, 677, 810, 929, 941.
- **None of the 10 reaches a frozen set as a mined positive.** The ids `mined-<i>:pos` for those rows appear in none of `frozen/{T,cal,train,val,tsyn-in,tsyn-cross}.jsonl`. Control, run first so the zero is a measurement: the same pattern finds 18 / 2 / 14 / 6 mined positives in T / cal / train / val, so the id format is the one the files use.
- Nothing downstream handles them. `freeze_stage2.py` builds a positive from `r["context_before"]` and a negative from `r["context_after"]` with no check on whether the twin is inside the positive's context; no script under `stage2/` filters or flags these rows, and no tracker records them. The commit that moved the count to 10 (`98dbd016`) said they were *"to be flagged at labelling"*; no code does so.

## Why it is still a defect, and why it is not fixed here

The frozen data is clean, so the defect is **latent**: a re-freeze, a larger draw, or a new consumer of `mined-candidates.jsonl` would feed these rows through unchanged.

It is not a drive-by, for two reasons that are facts about this tree and not preferences. `mined-candidates.jsonl` cannot be regenerated here, because its input `gitlog.patch` is not committed. And `freeze_stage2.py`'s outputs are frozen, so changing its selection could change which rows are drawn.

## Fix

Not started. Options, none free: a boolean on each row at mining time (`twin_in_context_before`) so a consumer can exclude it, which needs `gitlog.patch` to regenerate; or a filter in the freeze step, which has to be shown not to change the frozen draw. The miner's own tests are in `tests/test_stage2_mine_pairs.py`, and a flag at mining time is where its end-to-end fixtures would extend.

## Tests added

None. The regression suite for the miner's context handling landed in `b3f08301` and covers the fixed behaviour; it does not cover this residual, which is unhandled behaviour and not a fixed one.
