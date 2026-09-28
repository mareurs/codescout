---
id: caab4cf043572d72
kind: research
status: active
title: 'Codex: Phase1b stop review — ranking failures, evidence limits, shell fixes'
owners:
- codex
tags:
- codex
- phase1b
- system1
- review
topic: System 1 local classifier evaluation
---

# Codex Phase1b stop review

**Valid:** dated 2026-09-27

**Reviewed tree:** a5372e000a525f2f8e59d416cf690781598ac02c plus the narrowly listed working-tree fixes below. Author: Codex. This review uses saved model outputs; it launches no model, training job, or held-out evaluation.

## Verdict

Stopping this registered campaign is correct. All nine checkpoints fail on both their own final menus and the common menu. NC fails its three-seed ship condition; the registered N-versus-B causal claim remains withheld. The failure applies to this data, training recipe, representation and operating rule. It does not establish that System 1, a rule-conditioned model, or a background Codescout agent cannot work.

I found and fixed an orchestration defect in all three Stage-2 shell wrappers. It does **not** explain the historical model failure: every expected child execution in the recorded campaign exited zero, and a separate recount agrees with all 2,025 logged gate decisions.

## What completed since the previous review

- `14346eb4` contains the earlier Codex labeller failure-ordering and overwrite fixes; `a587a90f` contains their evidence.
- `90e0965d` adds the blocked-sync channel control. This is stronger than comparing two directories that both synchronize: the blocked directory prevents the additional skill/plugin sync while recording the same 412 input-token count. This measures the effect of that sync under those invocation flags; it does not explain every context token.
- `82d18742` records the completed labelling/admission step: 13 admitted heads, 169 accepted counterexamples, 12 surviving new clean texts.
- `08a5544e` records completed training and calibration; `5a477c51` adds the gate adapter before its run; `a5372e00` records the gate failure and stop.

These are historical observation anchors, not fix identifiers for this review's uncommitted changes.

## Independent saved-output checks

`docs/evals/data/2026-09-24-rule-tell/phase1b/codex-stop-review/analyze_saved.py` uses only the standard library. It reads the gate fixtures as AST literals, saved JSON/JSONL, and the already-scored calibration cells. It imports neither the model runner nor the gate implementation.

It checks each logged probability against its logit and saved temperature, each verdict against its saved threshold, the complete expected `(text, head)` population for both menus, per-text gate totals, span placement, saved summary pass/fail values, and the Step-4/scored-file hashes. All 2,025 logged cells across nine checkpoints agree. Own/common readings reuse scores; these are **not** 2,025 independent examples.

The stored model-run artifacts also report exact frozen-validation parity and determinism. I read those records and the implementation; I did not rerun GPU parity. This distinction matters: recomputing arithmetic from saved logits verifies scoring, not the model that originally produced the logits.

Own-menu outcomes:

| Arm / seed | Main gate | Span gate | Main failures |
|---|---:|---:|---|
| B / 20260935 | 17/19 | 1/2 | sessionid positive; clean-12 |
| B / 20260937 | 17/19 | 1/2 | sessionid positive; clean-12 |
| B / 20260940 | 17/19 | 1/2 | sessionid positive; clean-12 |
| N / 20260935 | 18/19 | 1/2 | clean-13 |
| N / 20260937 | 18/19 | 1/2 | clean-13 |
| N / 20260940 | 17/19 | 1/2 | clean-12; clean-13 |
| NC / 20260935 | 17/19 | 2/2 | clean-12; clean-13 |
| NC / 20260937 | 18/20 | 2/2 | clean-12; clean-13 |
| NC / 20260940 | 18/19 | 2/2 | clean-13 |

The denominators differ because pruning changes the applicable positive cases. The common menu is only `d_adjacency`, `d_semicolon`, `d_sessionid`. A higher fraction on a pruned menu is not full-menu recovery. NC's recovery of both span examples in all three seeds is observable progress, but it is not the registered causal or ship result.

## New diagnostic: calibration cannot repair two NC sessionid rankings

Clean-13 merely says that the session named codescout-26 exited and its name no longer resolves. The positive attributes a commit to a session using its display name. The first is clean under the registered rule, and both audit labellers accepted it.

| NC seed | Clean-13 raw logit | Positive raw logit | Clean ranks higher? |
|---|---:|---:|---|
| 20260935 | 1.374430 | 1.369277 | yes |
| 20260937 | 0.719255 | 3.713685 | no |
| 20260940 | 6.479461 | 4.578786 | yes |

For the two inverted pairs, no scalar threshold can reject the clean sentence while retaining the positive, and positive temperature scaling preserves that ordering. The first inversion is small; the statement concerns the recorded deterministic scores, not a confidence interval about future text. All three N seeds also invert this pair. This rules out a threshold-only rescue of those checkpoints on these examples; it does not prove which token or training mechanism caused the inversion.

The semicolon pair is different: all nine checkpoints rank the actual cargo-lane violation above clean-12's legitimate `npm ci && npm run build`. False positives at the registered thresholds therefore do not establish identical representations or complete inability to distinguish the two. No thresholds were tuned on the gate in this review.

## New diagnostic: the real-example denominator is tiny

In the saved calibration own-cell population for admitted heads there are **154 synthetic positives and two mined positives**, plus 154 synthetic negatives, three mined negatives, and 26 counterexamples. Counting removed heads as non-firing, every checkpoint misses the same two mined positives. These are two distinct examples reused across nine checkpoints, not eighteen independent failures.

NC detects 101, 123, and 103 of the 154 synthetic positives at the three seeds respectively, and zero of the two mined positives. Calibration selected the menus and temperatures; these are descriptive diagnostics, not held-out performance estimates. Two mined positives cannot establish deployment recall. They do establish that this calibration population provides very little evidence about real corrections.

Accepted counterexample rows cover `closed_population` (44), `d_adjacency` (1), `d_semicolon` (43), `open_artifact` (36), and `run_tool` (45). There are **zero targeted `d_sessionid` counterexample rows** in that file. This does not mean the head saw no negatives: own negatives and cross-rule negatives are separate training sources. It means NC's targeted additions did not directly address the gate's surviving session-name boundary.

## Code defect fixed: wrappers erase failure status

Bug: `docs/issues/archive/2026-09-27-codex-phase1b-shell-failures-return-success.md` (`e76b043bbdd97622`).

- `stage2/lanes.sh`: a failed training child was followed by successful echoes; background lane status was discarded by bare `wait`; the script returned zero. The fix accumulates failures inside each lane, returns the lane result, waits for both explicit PIDs, and returns the aggregate status.
- `stage2/step4.sh`: failed scoring/calibration children set `fail=1` but the final echo returned zero. A failing common-menu child did not set `fail` at all. Both results now reach the process exit status.
- `stage2/step5.sh`: the same two defects affected gate execution and final summary generation. Both now propagate.

Independent checkpoint runs still finish, so diagnostics are preserved. Successful execution with a negative scientific result still returns zero: the classifier's verdict remains in the result JSON. An infrastructure failure returns nonzero. The fix changes no label, weight, menu, threshold, or historical result.

Verification: eight focused unittest methods pass, including all three successful-wrapper controls. Against the pre-fix control flow, seven methods fail as expected. Nine concrete mutations applied to disposable script copies are all caught, with zero survivors and zero test errors. Tests execute the actual wrapper control flow with only machine-local path bindings relocated and Python children replaced by cheap fakes. No model calls occur. Bash syntax, Python compilation, and the changed-wrapper diff checks also pass.

Changes are left uncommitted for the peer session to review and integrate. No Rust code changed; the full Rust gate and healthy model-test suites were not rerun, following the operator's request to keep verification cheap. This is not a claim of a freshly green whole-repository gate.

## Handoff: what the next decision should be

1. Keep the registered stop and leave T/Score B unopened by this work. Preserve the current gate as a known diagnostic, not a fresh generalization test for a design it has now influenced.
2. Before buying another training run, define the product contract: a high-precision warning that may interrupt System 2 versus a candidate handed to System 2 for cheap adjudication. Their acceptable false-positive costs and evaluations differ. Do not retrospectively lower this campaign's gate.
3. Build a small, independently adjudicated development set from recent real traces, with explicit cue-preserving clean cases, multiple violations per rule, and paragraph context. Split by source/template rather than sentence alone. This must add genuinely new examples, not paraphrases of clean-12/13 repeatedly selected until they pass.
4. Compare a rule-conditioned contextual baseline on that development set before committing to another fine-tune. Treat a larger backbone, richer input, or targeted negatives as competing hypotheses. The current campaign has not isolated which one is the bottleneck.
5. Keep measurement of the three-role mechanism moving independently: deterministic checks, fast candidate detection, and System-2/tool adjudication. The background miniagent proposal is another capability with another eval; a failed fixed-head text classifier is not its evaluation.

## Evidence and reproduction

Directory: `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-stop-review/`.

- `saved-diagnostics.json`: full independent recount, rankings, calibration source counts, input hashes.
- `run-status-checks.json`: exact expected historical child identities/statuses and source log hashes; accepted counterexample counts.
- `shell-verification.json`: pre-fix failures, fixed tests, nine applied mutations, file hashes.
- `static-checks.json`: cheap syntax/compilation/diff checks.
- `analyze_saved.py`: repeat the saved-output analysis without inference.
- `verify_shell_fixes.py`: repeat fake-child controls and applied mutations without changing tracked wrappers.

Tests: `tests/test_phase1b_stage2_shell.py`.

Related: `docs/evals/phase1b-local-classifier-preregistration.md`; `docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md`; `docs/research/2026-09-26-codex-three-role-intervention.md`.
