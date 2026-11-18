---
id: 6973c3ace7e07e12
kind: report
status: done
title: Uncertainty follow-up did not reject the wrong repair or resolve the missing case
tags:
- effort-steering
- evaluation-integrity
---

# Low uncertainty-resolution follow-up

## Current state

Closed 2026-11-08, measured **NO-GO** for this uncertainty-follow-up variant. Author job5c and offline job5d both exited0. All three final evaluator cells are valid, but the saved wrong repair passes every generated assertion; reference retention alone is insufficient. The final contract remains explicitly unresolved. No active job or further same-case model retry.

| Named assertion | Baseline | Wrong repair | Reference |
|---|---|---|---|
| contrast_invalid_op_on_ans_last_keeps_ans_intact | fail | pass | pass |
| contrast_invalid_op_on_streamed_ans_last_keeps_ans_intact | fail | pass | pass |
| contrast_valid_transform_of_ans_last_still_stores | pass | pass | pass |
| contrast_valid_str_length_on_ans_last_strings_stores | pass | pass | pass |

The qualified author workflow settled with final cost USD0.2943323, one native connection, complete context/capture evidence, zero observed OOM kills and fresh owned-container absence. All16 actual captured forwarded requests used claude-sonnet-5-5 effort low. The frozen output contains four tests, including one additional wholly successful string transform; no discriminating mixed-result preservation assertion was added.

All twelve final named statuses were independently re-parsed from raw logs and agree with results.json. All three builds exited0. Final resource samples, correct8GiB/4CPU/network-none profiles, actual CARGO_BUILD_JOBS=1 proof and fresh cleanup checks pass. Sample row counts27/30/33; sampled host-available minima32.901/26.108/30.649GiB, zero OOM kills. These are host-headroom observations, not workload memory peaks.

## Frozen output

The requirement still checks all-error record pipelines leave a readable stored result, both value- and stream-sourced. Preservation checks cover a wholly successful first transform and a wholly successful string-length transform. The author retains uncertainty about non-ans error-containing or partly successful streams and whether a fix should be keyed by origin or by any error item. Actual test bodies assert stored values; no new mixed-result preservation assertion is present.

Capture SHA256015c0279b34853e37e80bab6ff06ddba1dc46d696e651a311f559bc74225cb92; test patch98cd26333163fd8ce2f795eb3c0baf337ab1ece59384402268718047a9a84803; contract3f77d05304ba61d8c6cf3b94f66478ba1cce44219b91a445b8eec264398e545b. Frozen receipt remains before host-only comparison; do not edit or re-author after outcomes.

## Design and costs

[Design](../../../superpowers/specs/2026-11-08-oracle-uncertainty-resolution-design.md), [plan](../plans/2026-11-08-oracle-uncertainty-resolution.md). A fresh low author saw only the original public baseline/issue/guidance, its own pre-evaluation frozen contract/test patch and generic uncertainty-investigation instruction. Original source-only onboarding reused. No proposed repair/reference/private witness/xhigh result supplied.

Original low USD0.2914831 plus follow-up USD0.2943323 totals USD0.5858154 in oracle-author model cost, compared descriptively with prior xhigh USD1.4046817999999996. This is not complete repair-workflow cost or a causal prompt-effect estimate: extra input/time/stochastic draw are confounded on one exposed development task. Known series model subtotal USD64.401093499999998654 plus original unresolved USD4 startup hold. New USD1 reservation is settled by final cost; series ceiling USD200.

## Admission and limits

New-script import collision and test holes were filed immediately as02b930bd7339c2e5 and abdd41858ee6e1ed, then mitigated resident-only in one parent fix pass after one independent review. Actual qualified import and prepared validation passed;985 cloned manifest payloads hash/inode checked. Final focused suite24passes. Original16mutants7killed/9survived; reapplied16 after fixes14killed/2minor survivors M14/M15 (parent resource/gate receipt fixture gaps, guards unchanged). No product fix commit or re-review. Existing qualified source/gate reused; no merge or push.

## Next check

### Observed actor process

The saved stream contains one temporary observation test with six explicit baseline input/command pairs and six emitted PROBE observations, subsequently removed from the final test file. The cases inspect missing-field errors, all-error record transforms, integer scalar/list failures, a single-record failure and ans exit-code access. None of those six inputs exercises a partly successful stream. The final unresolved clause still names that uncertainty. Thus the instruction produced real extra observation work, but did not ensure executable coverage of the uncertainty itself. This is a coverage/process result, not a claim that public evidence uniquely determines the correct intended semantics.

### Evidence and decision

[Independent audit](../../../research/data/2026-11-08-oracle-uncertainty-audit.json), SHA2564def69a367bcc0b051361c0b5bb2515f4b53053ec9fecb9fa22ddff80eb4d0b3, binds author proof/cost, all twelve raw statuses, frozen inputs, six log hashes, resource finals, compile environment and cleanup. Audit used no provider call or native regrade.

[Allowlisted evidence archive](../../../research/data/2026-11-08-oracle-uncertainty-evidence.tar.gz):79payloads plus manifest,54061bytes, SHA256f1cfe2046e824a2269fbd29f6353f8bf5d142aaa0ba871da9c24aa6f3dbe629e; all payloads reopened and hash-verified. It preserves the generic instruction, controller/review/mutation evidence, frozen output, compact author receipts, extracted public observations and offline logs/proofs. Raw provider requests/responses/debug/auth/client homes and full author Docker inspections are excluded.

Close this hypothesis and stop prompt tuning on this exposed case. No generic low-repair mechanism has been validated by these results. Any further mechanism needs a distinct prospective design and independent task population; this known missing example must not be quietly supplied as a private solution hint or counted as transfer evidence. No paid or native job remains active; no merge/push.
