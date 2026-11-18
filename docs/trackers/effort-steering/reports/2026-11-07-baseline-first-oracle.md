---
id: 12f32461b6bdfadf
kind: report
status: done
title: 'Baseline-only oracle authors: both low and xhigh miss the saved wrong repair'
tags:
- effort-steering
- generic-mechanism
- evaluation-integrity
---

# Baseline-first oracle development pair

## Current state

Closed 2026-11-08: **NO-GO for this baseline-only oracle-authoring mechanism on the exposed development task.** Both authors distinguish the original bug from the qualified reference, but both also accept the saved wrong repair. This is not evidence of a successful low=xhigh repair mechanism or of general effort equivalence.

Both author outputs were validly closed and frozen before candidate/reference evaluation. Actual model costs: low USD 0.2914831; xhigh USD 1.4046817999999996. Source-only onboarding was reused. No additional model/provider call was used for offline continuation or the audit.

Each row below is one generated named assertion; columns are observed outcomes on the original baseline, saved wrong repair and qualified reference:

| Author / named assertion | Baseline | Wrong repair | Reference |
|---|---|---|---|
| low / contrast_invalid_op_on_ans_last_keeps_ans_intact | fail | pass | pass |
| low / contrast_invalid_op_on_streamed_ans_last_keeps_ans_intact | fail | pass | pass |
| low / contrast_valid_transform_of_ans_last_still_stores | pass | pass | pass |
| xhigh / contrast_failed_last_pipeline_leaves_ans_fetchable | fail | pass | pass |
| xhigh / contrast_failed_last_pipeline_stores_no_error_value | fail | pass | pass |
| xhigh / contrast_invalid_ops_on_last_still_report_errors | pass | pass | pass |
| xhigh / contrast_valid_last_transform_is_still_stored | pass | pass | pass |

All three low and all three final xhigh evaluator cells are valid. Xhigh completed in offline-single-job-2 with CARGO_BUILD_JOBS=1 and the same 4 CPU / 8 GiB container, test bytes, patches and commands. Its three raw logs independently yielded all twelve named statuses. All builds exited zero; final resource samples exist; each container has environment proof and fresh absence verification. Sampled host-available minima were 50.295, 49.957 and 48.022 GiB respectively, with zero observed OOM kills. These are host-availability observations, not measured workload memory peaks. Compile scheduling differs from the earlier low evaluation, so elapsed-time comparisons are not claimed.

[The final xhigh audit](../../../research/data/2026-11-08-baseline-first-xhigh-audit.json) checks registered input/log hashes, raw named statuses, complete resource samples, profile, environment and cleanup without a native regrade. SHA256: d1ca6cbeb9cd2b4a0079b4479d83e4f33470a66b8c82b645af622ba9b5191d0b. [The earlier partial audit](../../../research/data/2026-11-07-baseline-first-partial-audit.json) covers both paid authors and the nine low statuses.

[The scientific evidence archive](../../../research/data/2026-11-08-baseline-first-scientific-evidence.tar.gz) contains 62 allowlisted payloads plus manifest, 41,770 bytes, SHA256 49cabdd3aa4c4fed4bdb2b73410845a90841296a2290bf579bfaa8eeea98890a. Every payload was reopened and hash-verified. It preserves frozen contracts/test patches, evaluator registrations/logs/resources/cleanup, the interrupted original xhigh cell and both wait-only attempts. It excludes credentials, client homes and raw provider/debug captures.

Historical resource interruptions remain infrastructure-invalid, never xhigh quality failures: the original xhigh baseline build passed but test compilation was stopped by our 24 GiB host-headroom rule at about 22.375 GiB available, with zero recorded OOM kills. Subsequent 32 GiB and 28 GiB admission waits expired before creating any cell. Fresh available memory later exceeded the unchanged admission threshold, and the final three cells completed. Installed RAM was not demonstrated insufficient; this was a timestamped headroom-policy block. No job remains active.

[Design](../../../superpowers/specs/2026-11-07-baseline-first-oracle-design.md) and [execution plan](../plans/2026-11-07-baseline-first-oracle.md). This is one exposed development task, not held-out population evidence. The preceding [differential-oracle experiment](2026-11-07-differential-oracle.md) remains a separate negative result.

## Qualification evidence
The closed controller/review evidence is preserved in [the allowlisted archive](../../../research/data/2026-11-07-baseline-first-controller-evidence.tar.gz): 77 payloads plus manifest, 296,876 bytes, SHA256 b4bbef977fa7799afc81d2b7de2dacafb678bdf14068cf3ae57d7fa25e32ab12. Every payload was reopened and hash-verified. This archive excludes active author state, requests/responses, debug files and client/native/auth homes; it contains no scientific outcome. The process-lifetime lesson is recorded as F-217 in the bug-fix session log.

The actual base image was checked at original public HEAD with an empty tracked diff, no proposal directory, contract file or new test module. The independent review also rehashed all 985 manifest-listed snapshot payload records and observed distinct physical files across original/low/xhigh. Qualified preparation validation passes both states. Public prompt SHA256: 8f183e2168957704884af0fde67d6e559738dfd0cd9e46292695b16e5273a489.

One independent review found four concrete defects and missing regression coverage before any new paid run. The [runtime-wrapper issue](../../../issues/2026-11-07-baseline-pair-nested-runtime-wrapper.md), [mount syntax issue](../../../issues/2026-11-07-baseline-native-equals-mount-bypass.md), [schema issue](../../../issues/2026-11-07-oracle-contract-nonobject-clause.md), [test inventory issue](../../../issues/2026-11-07-oracle-capture-undeclared-tests.md), and [coverage issue](../../../issues/2026-11-07-baseline-controller-integration-test-gaps.md) were filed immediately and are locally mitigated. No product-source fix commit is claimed.

After one parent fix pass, all 36 resident tests pass, including actual controller/preparer entry points against controlled filesystem and external author/evaluator boundaries. These fixtures do not run a model or native workload. Original review: 16 applied mutations, seven killed and nine survived. First fix verification: 20 applied, 18 killed and two survived. Final verification after observing and closing missing inline-declaration coverage: 20 applied, 19 killed and one survived. Original results are retained.

## Rulings and limits

The surviving M11 mutation removes a redundant second visibility call. Initial validation and immutable registered-input hashes remain, so this is not a demonstrated admission hole; it is explicitly retained as redundant rather than labelled a killed mutation. Eager evaluation M12 fails fixed-pair completion because complete-pair freezing blocks it; no private-feedback leakage was demonstrated. Wrong snapshot origin M14 fails the preparer fixture; it did not demonstrate successful prior-state admission.

The single-author runner now runs in a fresh process per arm, preserving the qualified compatibility installer's process lifetime. The constrained capture parser admits a new test module and its registration, then requires supported executable declarations to be accounted for. Unsupported Rust forms are protocol-invalid; this is not a general Rust parser. Empty bodies or plausible source citations do not establish oracle quality; only the later real matrix plus contract review can do that. Parent fixes were not independently re-reviewed; post-fix evidence is red-green regressions and applied mutations.

The unchanged qualified source worktree and source gate are reused. The repository gate does not cover this resident Python orchestration; its separate suite is the relevant check. Original source/registration are retained and a prepaid v2 registration changes only corrected source hashes, with public inputs unchanged.

### What the existing outputs establish

Both frozen contracts explicitly classify mixed/partially successful stream behavior as unresolved and assign it no test. Low checks a wholly successful transform; xhigh additionally checks rendering, nested error values and error reporting. These checks all accept both the saved wrong proposal and the qualified reference. Greater source/render tracing and higher effort did not supply the missing discriminating preservation test on this case.

The result isolates an oracle-coverage gap, rather than the earlier verifier's rejection of a correct reference. It does not prove that public evidence uniquely determines mixed-stream semantics, that another prompt can recover it, or that a baseline-derived behavior should automatically become the specification.

Roadmap: do not transfer this unchanged mechanism or spend on another same-case effort rerun. The next candidate should handle unresolved clauses explicitly: seek a justified public invariant, construct boundary/characterization cases and distinguish observation from intended behavior; if semantics remain ambiguous, abstain or request a specification decision. Any such generic mechanism needs a separate prospective design and a source-only author boundary. This exposed missing case may inform development but cannot become a private task-specific hint or count as held-out transfer success.

## Costs and next decision

Actual pair model cost is USD 1.6961648999999996, counted once from final cumulative receipts. Known series model subtotal remains USD 64.106761199999998654 plus the original unresolved USD 4 startup reservation; standing series ceiling USD 200. The final offline continuation and audit made no provider calls. No further authoring/onboarding rerun was needed.

Decision: close this registered pair as measured NO-GO, retain both frozen outputs and all invalid/wait-only artifacts, and develop an explicit uncertainty-handling mechanism before any further model run. Correct-reference retention alone is insufficient: a usable oracle must also reject the saved wrong repair through a justified preservation check. No merge or push was performed.
