---
id: f2c1019239ed99da
kind: research
status: draft
title: Codex follow-up review — form 4q and Stage 2 miner
---

# Codex follow-up review — form 4q and Stage 2 miner

**Valid:** dated 2026-09-24

Reviewed through `898d3ea37e16e31b0dc76f77e0ed631dcbe7a9c8`. Source review and cheap measurements on committed data only. No model calls, training, test-suite reruns, or production-code edits.

## Earlier findings addressed

The controlling Stages 2–4 amendment selects C1 on validation instead of T, masks unconfirmed sentence/rule cells, and groups incident variants across folds. `report_corpus` now checks the full expected case/side manifest and returns nonzero for missing groups. The phase-2 scorer requires a ballot output file, writes three votes per scored row with arm/run and header hashes, and refuses the dirty judge profile by default. These conclusions are based on reading the changed implementation/protocol; no redundant reruns.

## New findings

### Miner overlap census undercounts: 20 reported, 25 measured

Same 946 rows, same document keys, same positive/twin eight-word shingles. The one-owner map creates a star rather than all pairwise links; five document pairs are missing. Filed in docs/issues/2026-09-24-codex-miner-shingle-pair-undercount.md. Star edges still preserve components, so this does not itself demonstrate broken fold isolation.

### Positive-example context is the post-correction hunk

`change_blocks` explicitly returns the new-side hunk. `mine` writes its first 1500 normalized characters as `paragraph` alongside the removed positive sentence. Measured by whitespace-normalized substring: only 30/946 positives occur in `paragraph`, while 605/946 twins occur there. This field is useful review context but unsafe as the original draft input: it can reveal the correction, omit the target sentence, and contradict the positive label. Before building training examples, retain distinct before/after contexts and verify the target span against the correct one. This is prospective: the saved rows have rule=null throughout and are not training examples yet. No trained-model contamination is claimed.

### The mining feasibility verdict exceeds the sample

The status note concludes that mined pairs alone cannot bring any rule to 50 positives, extrapolating a pooled ten-row manual sample through keyword rule hints. Those hints are explicitly not labels, 615 rows have no hint, and no per-rule precision was measured. The conclusion is not established by this instrument. The defensible result is that no rule has yet been shown to reach 50 adjudicated positives. A stratified labeling pilot is needed before ruling the source out or choosing a synthetic-first budget.

## Form 4q arithmetic and carry verification

The committed corpus has 924 rows and 924 unique `(case, side, rule)` keys, the same key set as form 3, zero errors. Every non-question_asked row equals its form-3 source after removing carried_from; zero mismatches. Four changed verdicts, all NO to YES: RTD-1 positive, RTD-12 positive, RTD-8 positive and RTD-13 negative. This supports the report: 5/17 gold-rule hits overall, no increase in negative texts with any fire (still 5/21), but an additional wrong rule on RTD-8 and an additional fire on an already-flagged negative. It does not establish a clean standalone question_asked classifier or a Score-B effect.

The form-4q reading is explicitly post-selection/tailored. Re-running the same gate provides another draw, not new held-out evidence; the report appropriately does not promote this to a generalization claim.

## Assessment

The harness repairs address the reported failure modes and the form-4q result is numerically coherent. Keep it diagnostic. The next load-bearing work is reconstructing historical before/after context and assigning auditable labels to candidate pairs; a larger candidate count alone does not establish training viability.
