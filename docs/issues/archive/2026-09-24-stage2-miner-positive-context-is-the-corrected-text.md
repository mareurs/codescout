---
kind: bug
status: archived
title: Stage 2 miner stored the corrected text as the positive sentence's context
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-24
owner: marius
severity: high
unverified: TRACKED eb9014f896a48715 — 10 mined rows already carry the twin inside context_before and nothing flags or excludes them; none reached a frozen set (measured 2026-10-01, with a control).
---

# Stage 2 miner stored the corrected text as the positive sentence's context

**Valid:** dated 2026-09-24

## Observed

Reported by the Codex follow-up review (`docs/research/2026-09-24-codex-rule-tell-followup-review.md`, finding 1), which did not file it. At `f828134a`, `change_blocks` in `docs/evals/data/2026-09-24-rule-tell/stage2/mine_pairs.py` built each row's `paragraph` from the hunk's **new** side only. A positive is a **removed** sentence, so the context stored beside it was the corrected text.

## Reproduction

Re-derived on the committed `mined-candidates.jsonl` (946 rows), normalising whitespace: the positive appears inside its own `paragraph` in **30** rows, and the corrected twin in **605**. Codex's figures, reproduced exactly.

## Impact and bound

If used as model input, the context would put the answer, the correction, next to the sentence being classified: a label leak. The rows were declared candidates, not frozen folds, and no model was trained on them. Severity `high` on the data-loss-risk arm of the rubric: this would have produced a trained arm that scores well by reading the fix.

## Fix

**Fixed in `a63adc78`** (patch-id `2c568203f402597d7f6958b8dd616225a1646772`). `change_blocks` yields both sides. Rows carry `context_before` (old side, the positive's own) and `context_after` (new side), and `paragraph` is gone. The leakage filter now shingles both contexts, which drops 6 more candidates against held-out texts (946 → 940).

**Verified after the fix, on the re-run:**

- positive inside `context_before`: 793 of 940;
- positive inside `context_after`: 30;
- twin inside `context_after`: 601;
- twin inside `context_before`: 6.

**Residuals, as of 2026-10-01:**

- **147 positives not inside their `context_before`: closed** by `98dbd016`, which centres the window on the sentence. Re-measured on the committed rows: positive inside `context_before` in 944 of 944.
- **The twin already on the old side: still open, now 10 rows, and split out** as `docs/issues/2026-10-01-residual-mined-rows-with-the-twin-already-in-context-before.md` (`eb9014f896a48715`). Measured there: none of the 10 reached a frozen set as a positive.

**Class `cluster/value-correct-in-a-frame-its-name-does-not-state` (IC-24).** The paragraph was exactly right for the corrected sentence, the twin. It was stored under a name, beside the positive, that states the positive's frame.

**Regression tests added 2026-10-01** in `b3f08301`: `tests/test_stage2_mine_pairs.py::ChangeBlocks`, `Window`, `MineKeepsEachSidesContext` and `MineKeepsNoteContext`. A mutation that builds the old-side context from the new side, which is this bug, fails between one and four tests at each of the seven sites that could reintroduce it (both side-selecting list comprehensions in `change_blocks`, and the four context assignments and the old-side paragraph in `mine()`).

## Fix provenance

- **SHA:** `a63adc78` (`experiments`)
- **patch-id:** `2c568203f402597d7f6958b8dd616225a1646772`

**Recorded 2026-09-28** by session `82cff72e`; the patch-id was re-derived from `a63adc78` and matches. Two further pairs. The second is the commit that closed the first residual; the third is the regression tests (re-derived through a file; both SHAs are ancestors of `HEAD`).

- **SHA:** `98dbd0166a80b9bb915961d3739d3e4b24998746` (`experiments`), residual 1: the window centred on its sentence
- **patch-id:** `5130236a08fb26cf3c7cbace5077329f4f01f7a4`
- **SHA:** `b3f083014d85a11691a47f17d7d8176ac3412624` (`experiments`), the regression tests
- **patch-id:** `4aa25a7319165b1ec16036734e47b9cce7f10414`
