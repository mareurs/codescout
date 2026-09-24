---
id: a4b83b599ade12ae
kind: research
status: draft
title: Codex review — phase-1/phase-2 logic, harness and cheap measurements
---

# Codex review — phase-1/phase-2 logic, harness and cheap measurements

**Valid:** dated 2026-09-24

**Reviewed revision:** `a8835d06b41dd6901943a796ad2b18aa8abad933`. Read-only source review plus offline arithmetic and synthetic probes; no model calls, training, replay generation, or test-suite reruns. Production and peer files were not edited.

## Assessment

The campaign now has committed raw model outputs, explicit instrument corrections and a negative end-to-end result. Phase 2 supports a conditional effect of hand-authored claim-bound reminders at two decision points. S0 is not a deployable selector under its registered rules. The immediate priority before Stage 2 is fixing the data/selection contract, not another prompt sweep.

## Independently recounted

From docs/evals/data/2026-09-24-rule-tell/:

- S0 form 2b and form 3: each 924 unique `(case, side, rule)` triples, 42 groups of 22, no duplicate triples, no error rows, identical key sets.
- Form 2b, 17 yes/partial positives: gold fired 3, wrong-rule-only 1, silent 13. Negative any-fire 2/21.
- Form 3: gold 3, wrong-rule-only 4, silent 10. Negative any-fire 5/21. The reported predominance of silence is supported.
- Six replay files checked (fork-dp1-n10, fork-rtd3r, fork-e2s-dp1, fork-e2s-rtd3, phase2-dp1-n10, phase2-dp1-rtd910-n10): ten rows per represented arm, no duplicate `(arm, run)` keys, no error rows. This validates completeness of those files, not semantic scoring.
- S0 injections: 2/10 DP1 and 6/10 RTD-3 have no injection; neither injection file records errored rules.
- Clean API-score log tables agree with the report: RTD-8 baseline 10/10, bound 0/9; RTD-9 6/10 and 0/9; RTD-10 9/10 and 0/10. These are saved aggregate judge results, not independently re-adjudicated labels.
- L0 probabilities, reconstructed in the gate's logged order (ten successive groups of 22 distinct rules): semicolon p=.710439 vs clean-max .199875; sessionid .5 vs .175049; cannot .600599 vs .600599; contradiction .360972 vs .257175, rank 12; member .639028 vs .439109. Thus only three examples have BOTH top-2 gold rank and strict separation above clean max, not four as the report's conjunction states. Four have top-2 rank and a different four have strict separation. Threshold calibration alone cannot separate the identical cannot_happen scores in this sample.

## Findings

### High priority before training: held-out T is also a selection set

Stage 3 in docs/evals/phase1-local-classifier-preregistration.md prohibits selecting on T; Stage 4 chooses the C1 local arm by T any-fire rate. Use validation for this choice, freeze it, and then inspect T. Filed as docs/issues/2026-09-24-codex-local-selector-test-set-selection.md. This is a prospective protocol flaw; no trained result is alleged contaminated.

### High priority before data freeze: a correction diff does not supply every negative label

Stage 2 calls every other sentence a negative for every rule. Neither a mined edit nor a generator instructed to plant one violation establishes that proposition. Corrections can leave other violations untouched, and the rule families overlap. Preserve unknown labels for unaudited sentence/rule pairs; audit multi-label negatives explicitly, not just the intended positive. Keep each incident, original/correction pair and generated variant family together across train/validation/calibration/T. The written source-document split for T and eight-token shingle filter do not by themselves specify this for all folds. These are design recommendations; no training dataset was measured here.

### Medium: whole missing texts still bypass Score A completeness

The earlier within-text fix works: one of 22 rule rows returns 2. Actual offline report_corpus probes still return 0 for zero rows and for one complete negative text (22 rows), with zero incomplete groups. Validate the expected case/side manifest as well as the per-text rule grid. Filed as docs/issues/2026-09-24-codex-phase1-missing-case-groups.md. Both committed corpus files are complete, so the measured S0 numbers stand.

### Medium: saved phase-2 scoring loses row-level adjudication and provenance

scripts/phase2-score-dp1.py main prints gate outcomes and aggregate arm counts but does not persist `(input hash, arm, run, checker version, model/channel, votes, verdict)` records. SubscriptionJudge.complete discards result metadata and returns only text; judge reduces that to YES/NO. The committed replay rows can be inspected, but the scorer's historical per-row decisions cannot be reconstructed from its aggregate logs without fresh model calls. Persist ballots and final per-row verdicts before aggregation in future runs. Do not rerun existing experiments just to retrofit these records.

The phase-2 scorer also lacks the selector's clean-profile refusal; the handoff explicitly discloses that limitation. A shared judge-channel check and recorded configuration fingerprint would prevent recurrence rather than relying on an env-var reminder. This review does not claim that the saved clean runs were dirty.

## Bound conclusions

The 5/17 family result is a post-hoc regrouping of fixed outputs, not an upper bound on what changing specs and re-running could produce. Arbitrarily crediting every non-silent positive would give at most 7/17 on form 3's existing rows, still below .5; this supports stopping mere relabelling, not declaring spec changes incapable of helping.

The stripped-arm claim also remains overstated: under the clean checker s0=7/8=.875 is BELOW arm0=9/10=.9, so the written 's0 is not below arm0: holds' is arithmetically false on that comparator. The small cells do not establish equivalence or a reliable effect of stripping.

No latency/throughput benefit has been established here. The 22 per-rule calls are an architectural cost, not a measured runtime estimate. L0 is a failed zero-shot baseline, not evidence that fine-tuning will recover the task.

## Leakage audit scope (LLM lens)

Prediction contract: from a draft available before publication, return applicable violated rules and their claim spans. Labels currently come from authored correction pairs; proposed labels come from mined diffs and synthetic construction. No Stage-2 train/validation/calibration/T dataset was inspected, so split counts, cross-fold duplicate overlap and permutation results are unavailable. Verdict: insufficient evidence to certify a clean future training pipeline; the T-selection contradiction is directly verified. No null/model experiment was run, per the user's instruction to reuse correct-code results and measure cheaply.
