---
kind: bug
status: fixed
title: 'Stage 2 miner stored the corrected text as the positive sentence''s context'
tags:
- cluster/value-correct-in-a-frame-its-name-does-not-state
opened: 2026-09-24
owner: marius
severity: high
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

**Residuals, open:**

- **147 positives are not inside their `context_before`**, cut by the 1,500-character cap or the prose-line filter. The context builder needs to centre on the positive.
- **In 6 rows the twin already appears on the old side,** so the "correction" pre-existed and those pairs are weak.

Both must be handled before any fold is built.

**Class `cluster/value-correct-in-a-frame-its-name-does-not-state` (IC-24).** The paragraph was exactly right for the corrected sentence, the twin. It was stored under a name, beside the positive, that states the positive's frame.

**Not archived:** no regression test, and the two residuals are open.
