---
id: '7424046009e44e1f'
kind: research
status: active
title: 'Codex review: System1 judge gate and evidence preservation'
owners:
- codex
tags:
- system1
- judge
- review
---

## Verdict and scope

Reviewed Task 9 implementation and the saved gate at `8d9b2045`, including prompt construction, channel, voting, scoring, reporting and CLI output handling. The registered INCONCLUSIVE stop is correct. This is not a completed whole-branch review of Tasks 1–14, and does not establish that System 1 lacks value.

No model calls or training runs were made. Existing Rust suites were not rerun; changes are confined to Python evidence preservation, rendering and regression tests. Changes below are uncommitted for peer integration.

## Findings

1. **Medium — evidence can be overwritten on relaunch (fixed in working tree).** `run_gate` checked log location, not freshness. A two-run fixture reproduced 15 existing logs replaced by 15 new fake calls. It now rejects existing evidence and reserves the directory using exclusive creation before constructing the channel. A reservation survives failure before the first log. Two simultaneous contenders cannot both enter. Dry runs do not reserve vote directories. Bug: `docs/issues/archive/2026-09-29-codex-judge-gate-relaunch-overwrites-evidence.md`.

2. **Medium — CLI report outputs were destructive and validated too late (fixed in working tree).** `run.py::_gate` wrote text/JSON after judging, overwriting prior files, permitting both destinations to name the same file, and discovering a bad JSON parent after the expensive work. Both destinations are now resolved and exclusively reserved before judging. Failed attempts leave reservation markers; use new output paths. Nested destinations work; a scientifically negative result still exits 1. This protects named destinations, not attempts deliberately launched with entirely new paths. Filesystem failure during final writing is still possible; this is not a crash-atomic archival service.

3. **Important interpretation error — 17/52 is firing, not an established false-positive rate.** The controls are not independently adjudicated error-free. The spec and `score_gate` already state the distinction, but `format_gate` dropped it. It now preserves that caveat and reports the full majority split: **17 true, 13 false, 22 null**. Null is not clean. Consequently “a third of correct passages called mistakes” and a recommendation justified solely by “33% FPR” exceed the evidence.

4. **Important gold/target mismatch remains open.** RTD-17's source (`docs/evals/rule-tell-detection.md`, RTD-17) explicitly describes a then-true statement without a timestamp, detectable in form, and decay rather than error. The gate reuses its `text_detectable: yes` for a prompt asking whether a mistake occurred. Those are different targets. RTD-13 also warrants human adjudication: its extracted positive describes `link_scan` materializing citations; the negative rejects a second, write-time mechanism. This need not reverse the selected positive. The judges' refusal to call it a correction is not automatically a detectability failure. Do not relabel this completed gate after observing outputs; fix the contract before registering fresh cases.

5. **Report errors corrected explicitly by appended erratum.** All four no cases miss external, not three. RTD-13 has three not-correction votes; RTD-17 and RTD-21 have two. Their null detectability is a majority conditional on not-correction, not a missing majority. RTD-20's audit is majority abstention. Original `gate.txt` remains unchanged.

## Independent saved-output checks

The recount checks every stored vote's three majority fields against strict majority, matches the multiset of raw saved answers to logged answers, and recomputes the four score counts. It confirms **14/21, 4/4, 5/8, 17/52**.

All 243 saved vote records have one attempt. All 243 logs have the sequence thread started → turn started → agent message → turn completed, with no tool events. Summed usage: 7,652,195 input, 2,799,360 cached input, 66,385 output, 37,820 reasoning output. This review did not independently rerun the private-global-text scan or verify historical auth-file mutation times.

Evidence directory: `docs/evals/data/2026-09-27-system1-base-rates/codex-review-2026-09-29/`:
- `relaunch-before.json`: observed pre-fix overwrite.
- `recount_saved.py` and `saved-diagnostics.json`: repeatable recount, source JSON and per-log SHA256 hashes; no raw private prompts.
- `check_mutations.py` and `mutations.json`: six mutations applied to imported function objects, never shared source files; **six assertion failures, zero errors, zero survivors**. Mutations remove reservation, bypass legacy evidence checking, replace exclusive creation in both log and CLI guards, reserve during dry runs, and erase the firing-rate caveat.
- Initial recount used the wrong selector (`kind == correction`, while kind is `rtd` and mode is `correction`); its empty corrections list exposed the error. Corrected to mode, asserted 21 correction / 8 RTD audit cases, and calibrated all four counts against the registered score before publication.

## Verification and handoff

66 judge tests and 5 CLI output tests pass. New regressions were observed failing before their respective fixes. Two existing guard tests now allocate fresh log directories per attempted launch so they still reach the guard they test.

Integrate `scripts/measure/judge.py`, `scripts/measure/run.py`, `tests/test_measure_judge.py`, new `tests/test_measure_judge_reservation.py`, new `tests/test_measure_run_outputs.py`, this review, evidence directory, bug record, and the dated `gate-notes.md` erratum. Do not stage unrelated peer changes.

Keep the frozen prompt, labels, thresholds and historical gate output unchanged. The next useful step is human adjudication of the target: rule-form warning, factual mistake, and correction relationship must not share a gold label without an explicit mapping. A future judge attempt requires freshly registered held-out cases. The expensive real-corpus measurement stays stopped.

The raw JSON and logs are preserved privately, outside the repo, as corpus `2026-09-29-judge-gate-1` under `~/work/claude/measurement-corpora/` (frozen 2026-09-29). Its committed manifest is `docs/evals/data/2026-09-27-system1-base-rates/corpora/2026-09-29-judge-gate-1.manifest.json`, and all 243 vote-log hashes and the `gate.json` hash recorded in this review match the frozen copy. It is one copy on one disk, so an off-machine backup is still owed. This review did not publish raw transcripts.

## Subsequent operator clarification — 2026-09-29

The operator approved the broader useful-intervention scope, including checks and qualifications before mistakes occur. Decision and completed six-case human worksheet: `docs/research/2026-09-26-codex-three-role-intervention.md`, section “Operator decision — broader useful interventions (2026-09-29)”. The operator individually approved cases A–F: quiet qualification, targeted verification, no duplicate reminder, immediate correction through System 2 before a false claim is sent, retrieval of missing evidence without presuming falsehood or rerunning tests, and silence for properly scoped historical observations. Human involvement is reserved for needed input in these examples. These are development-policy decisions, not held-out performance evidence; the implementation schema and broader escalation cases remain to be designed. The completed gate remains INCONCLUSIVE; no prompt, gold labels or scoring thresholds were changed.
