---
id: eb9014f896a48715
kind: bug
status: fixed
title: 'RESIDUAL: 10 mined candidates already carry the twin inside context_before, and nothing flags or excludes them'
owners:
- marius
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
closed: 2026-10-06
unverified: The T-set loop in freeze_stage2.py (:168-173) still builds mined positives from context_before with no twin check, there is no mining-time flag (gitlog.patch absent), and the freeze-level skip is not unit-tested; none of the ten rows is in a frozen set today, so no leak is observed.
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

A guard, not a regeneration. `b45bcce4` (3 files, 59 insertions), on `experiments`:

- `mine_pairs.twin_in_context_before(row)` in `docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py`: whitespace-normalised containment of the row's twin in its own `context_before`, and False for a row with no twin (an unguarded `"" in text` is True, which would flag the 9 twinless rows).
- The mined-rows items loop in `docs/evals/data/2026-09-24-rule-tell/stage2/freeze_stage2.py` (`split == "rest"` rows, which feed the train / val / cal folds; the guard is at `:136`) skips a row for which it is True. The skip count is printed to stdout (`mined rows skipped, twin already in context_before:`) and deliberately not added to the manifest counter `n`, so the manifest bytes cannot move.

The ten committed rows (0-based ids 267, 327, 439, 534, 592, 593, 677, 810, 929, 941) are exactly the rows the predicate flags. None reaches a frozen set, so the frozen data does not change: the freeze re-run at the time of the fix reproduced every per-set sha256, and `git diff --stat fac7abce HEAD -- docs/evals/data` shows only the two scripts (re-checked at HEAD `fda10a31`: `freeze_stage2.py` 5 insertions, `mine_pairs.py` 10 insertions).

Not done:

- The T-set loop (`freeze_stage2.py:168-173`) still builds mined positives from `r["context_before"]` with no twin check. None of the ten ids is in T today (see Evidence), so nothing leaks, but a re-draw into T would.
- No mining-time flag on each row: `gitlog.patch` is not committed, so `mined-candidates.jsonl` cannot be regenerated here.
- The skip is not unit-tested at freeze level; only a monkeypatch run, not committed, showed it is wired.

## Tests added

Five tests in `tests/test_stage2_mine_pairs.py`, class `TwinInContextBefore` (run at HEAD `fda10a31`: 5 tests, OK):

- `test_a_twin_inside_context_before_is_flagged` — the predicate returns True when the twin is in the window.
- `test_the_comparison_ignores_whitespace_differences` — a twin split across a line break or double space still matches (the measurement was whitespace-normalised).
- `test_a_twin_absent_from_context_before_is_not_flagged` — the negative twin of the first, so a predicate that is True for every row fails.
- `test_a_row_with_no_twin_is_not_flagged` — `None` and `""` twins return False (the unguarded `"" in text` trap).
- `test_the_committed_candidates_flag_exactly_the_ten_rows_the_issue_measured` — over the committed `mined-candidates.jsonl`, the flagged ids equal `[267, 327, 439, 534, 592, 593, 677, 810, 929, 941]`.

The freeze-level skip in `freeze_stage2.py` has no test of its own.

## Fix provenance

- **SHA:** `b45bcce4` (`experiments`)
- **patch-id:** `26c0c9fdaa143295cdfb4a8bf9ca813a717841ef`

## Resume

Closed on 2026-10-06 by `b45bcce4` as a guard on the latent defect; the frozen data was clean before and is unchanged. Residual follow-ups (listed, not filed):

- Add the same `twin_in_context_before` check to the T-set loop in `freeze_stage2.py` (`:168-173`), or document why T is exempt.
- A mining-time boolean (`twin_in_context_before`) on each row, once `gitlog.patch` is available to regenerate `mined-candidates.jsonl`.
- A freeze-level test that the skip is wired (a row carrying its twin in `context_before` must not appear in the frozen output).
- Surface the skip count in the manifest only if a re-freeze is ever accepted to change manifest bytes.
