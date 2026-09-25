---
id: '665ab7a1cb2ddd20'
kind: research
status: draft
title: Codex Stage 2 freeze review — data checks and a surviving count-guard mutation
---

# Codex Stage 2 freeze review

**Valid:** dated 2026-09-25

Reviewed campaign changes after `71125e36` through `d643c001684fe675fc682acffee6f1d817bdcc94`: top-up planning/draw, relational re-audit, round-2 audit, trainability counting and frozen datasets. This is a review, not permission to train or a claim of model performance. No model calls, training, or test-suite reruns.

## Finding — P2: the freeze count assertion checks inputs, not emitted positives

`freeze_stage2.py` counts items before `row()` can drop their target and asserts that count against `trainable.json`. Four rules differ in the actual train file: closed_population 52→51, d_adjacency 55→54, d_sessionid 72→70, question_asked 78→76. The current menu still clears 50 per rule, but the described guarantee is false.

Observed rather than inferred: baseline freeze redirected to /tmp returned 0 and reproduced all six committed JSONL files byte for byte. An in-memory mutation dropping every positive train row also returned 0, emitted zero train positives, and passed the assertion. **One candidate mutation applied; one survived.** No shared code or data edited. Fix the check on emitted rows, explicitly reconcile segmentation drops, and correct the preregistration claim before relying on this gate.

Filed: docs/issues/2026-09-25-codex-freeze-positive-count-guard.md (`65605f410dde3474`).

## What the offline measurements establish

- Top-up manifest: 2,483 rows and distinct seed IDs, matching the registered RNG draw from unused training seeds; every stored text matches its SHA-1. This verifies the manifest's internal hash consistency, not a fresh reconstruction of seed text from historical git blobs. Generation artifacts contain 2,483 rows, of which 1,210 have construction `ok=true`.
- Round-2 sample IDs reproduce RNG 20260934. Its 192 ballots contain 32 disagreements and zero invalid answers; recomputing each cell's counts and >20% decision agrees with decisions.json.
- Relational re-audit uses exactly the original 22 contradiction pair IDs; two disagreements, zero invalid, and cell decisions agree with ballots. Read the revised question: it distinguishes the counterpart in the contradiction from a separate breach.
- Six dataset SHA-256 values match the freeze manifest. Sizes: train 1,791/895 positives; val 521/261; cal 359/179; T 39/18; tsyn-in 296/148; tsyn-cross 592/296.
- All 3,598 frozen rows have a valid segmenter target matching the source positive/twin/fix; text, rule and binary label agree with provenance. This is structural validation, not fresh semantic adjudication.
- No quarantined pair appears in any frozen set, under the registered relational replacement of v1 contradiction verdicts.
- Train has 1,762 Claude-generated synthetic rows and 29 mined rows; no Codex-generated training rows.
- Exhaustive frozen-text eight-word shingles (`re.findall(r'\w+', text.lower())`) show zero train overlap with val, cal, T or either T-syn. Validation and calibration also have zero shingle overlap with the three test sets.
- No source group is shared between any train/val/cal/T split or those splits and either T-syn. The two T-syn sets share 107 source groups, consistent with their paired-seed design; they are not independent test samples.
- All 14 menu rules retain at least 50 emitted train positives, minimum 51, maximum 102. Eight rules remain Haiku-only.

## Limits and non-findings

Val/cal share six distinct eight-word shingles, attributable to two pair combinations (release-profile wording and a Dana attribution sentence). Their source groups are disjoint. The registered shingle rule names train-versus-val and train-versus-cal, so this is disclosed overlap, not a claimed violation of that rule. T-syn-in/cross share one distinct eight-word shingle and intentionally reuse seeds.

The target segmentation losses are already disclosed: T retains 18 of 27 positive items; claims on its per-rule performance remain withheld. No new evidence here expands its representativeness. The top-up reached 14 rules rather than the predicted minimum of 15; that miss is reported rather than hidden.

Item quarantine and the relational audit address the prior review's concrete data-admission concerns. Independent recomputation confirms saved counts and decisions, not the semantic correctness of every generated label. Calibration has only 4 positives for d_semicolon and member_vs_population; any per-rule recall-0.9 threshold remains statistically fragile and needs its denominator reported.

The current frozen data passes the checks above. Correct the count guard before treating the freeze as an enforced training prerequisite. Model quality and general sufficiency detection remain unestablished by this review.
