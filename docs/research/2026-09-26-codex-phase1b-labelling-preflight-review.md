---
id: '6e82eab6fa18b6bc'
kind: research
status: draft
title: Codex — Phase 1b labelling preflight review
tags:
- codex
- phase1b
- eval
- review
---

# Codex — Phase 1b labelling preflight review

**Valid:** dated 2026-09-26
**Verdict after the operator requested implementation:** the two reproduced runner defects are corrected in the shared working tree and verified with fake-model regressions and applied mutations. The prepared inputs passed the checks below. The judge-channel token-count conclusion still needs narrower wording or stronger evidence. No real labelling, training or held-out scoring ran.

## Scope and prediction contract

Reviewed the registered Stage 2 protocol (`ec728270`), the clean-text generator and output (`58c91dad`, `8340c5c3`), the draws (`7713e070`), blind items (`b009d4d5`), `run_labellers.py` and `score_audit.py`.

Checkout started at `16cef832` and advanced to `445f4042` during review. The intervening commit saved the three-role design document; a path-scoped git diff confirmed no changes to the reviewed runner, scorer, protocol, training code or audit tests.

The labelling contract is: given one sentence in its paragraph and the 14-rule menu, identify visible violations or uncertainty. Codex and Claude label independently; their union controls admission, masks and clean/counterexample acceptance. This is an audit of proposed negatives, not a fresh measurement of classifier generalisation.

## Fixes applied by Codex

The operator asked for repaired code and a concrete handoff. Changes are in `phase1b/run_labellers.py` and `tests/test_phase1b_audit.py`; no commit or push was made by this review.

- **Failure observation and expenditure:** replace eager ordered `Executor.map` with at most `--workers` pending futures, read completed futures before refilling, and set a worker-visible stop signal on terminal failure. Check that signal before each model attempt. Cancellation is not retried as a model failure. Already running calls can finish; successful answers are assembled back in original item order.
- **Run ownership and preservation:** `reserve_run` refuses existing raw/header/label artifacts and exclusively creates the run header before model calls. This also refuses simultaneous starters. Stopped/interrupted runs remain reserved, and output labels are created exclusively. `--only` is a standalone invocation, not incremental resumption in the same directory.
- **Cheap preconditions:** reject non-positive worker counts before creating outputs; construct/validate the Claude channel before spending the preceding Codex call. The audit's rows, prompts, menu, retry limit, admission rules and seeds are unchanged.

**What the original checks missed:** per-call retry tests did not exercise the orchestration that observes a later failure behind an earlier slow future. `shutdown(cancel_futures=True)` was reasoned about as if it observed the failure immediately. Likewise, a protocol saying “run once” did not enforce single use of an output directory. The new tests exercise those exact paths, including two starters whose prechecks both saw no prior header.

### Observed verification

- The first six new integration tests were run before the fix. After correcting test-fixture isolation, the old runner produced seven failing assertions/subtests and zero errors; the failures covered pending-call starts, reused evidence, in-progress reuse and late worker validation.
- Final command: `PYTHONDONTWRITEBYTECODE=1 python3 tests/test_phase1b_audit.py` — **43 tests passed**. Nine are the new main-path/ownership/cancellation tests.
- `codex-preflight-review/verify_fixes.py` applied eight separate mutations in memory and ran those nine tests against each. **Eight killed, zero survived.** These include restoring the old ordered map, bypassing reservation, non-exclusive header creation, success on terminal failure, wrong result ordering, retrying cancellation, retrying after another batch's terminal failure, and late worker validation.
- `run_labellers.py --dry-run` still reports **505 items / 21 Claude batches**, and `git diff --check` passed for the implementation and tests.
- No model calls, training or full Rust gate were run. Verification is scoped to this Python harness; the shared checkout has unrelated Rust work in progress.

Saved evidence: `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/fix-verification.json` and `final-checks.json`. The former records the exact runner/test SHA-256 values used for the mutation pass. The pre-fix runner is preserved as `run_labellers.before.txt`; `reproduce.py` explicitly loads that snapshot so historical reproduction does not silently become a test of the repaired implementation.

The findings below describe the pre-fix version and its line numbers; their requested code corrections have now been applied.

## Findings

### P1 — terminal failure does not stop pending batch expenditure promptly

`run_labellers.py:185` through its ordered `ex.map` consumption and shutdown at line 189.

A slow batch 0 delays observation of a twice-failed batch 1. The worker that failed can take queued work while the main thread waits for batch 0. Cancellation in the finally block arrives too late.

**Measured with the real main path and fake models:** all 21 distinct batches started; 22 calls including the retry; batches 2 through 20 started after batch 1's second failure. The script eventually returned exit 4 with no labels, as designed, but spent the rest of the simulated batch budget.

**Action:** bounded, failure-aware scheduling with a shared terminal-stop signal; preserve item order only when assembling results. Running calls may finish; pending work must not start after terminal failure is recorded. Add the blocked-earlier / failed-later regression.

Bug: `docs/issues/2026-09-26-codex-labeller-stop-ordering.md` (`ea84bb940596ddf3`).

### P2 — relaunch repeats model calls and overwrites run evidence

`run_labellers.py:169`, `run_call:79` / `:83`, and output writes at `:196` / `:198`.

**Measured:** a temporary directory with previous labels, header and raw batch answer was accepted. All 21 fake Claude batches ran; the old labels and raw answer were replaced; exit 0. The actual production audit directory still contained only its four input files at inspection, so no real previous answers were lost.

**Action:** refuse reuse before calling either model, reserve the destination exclusively, and preserve attempt history. Resumption, if added, must not reset the registered retry budget or discard accepted work. There is no reason to change or redraw the committed inputs.

Bug: `docs/issues/2026-09-26-codex-labeller-run-overwrite.md` (`a7f3a7c8ad7d352a`).

### Measurement caveat — 412 versus 412 does not establish absence of shared context

The operator-relayed report says both the existing and fresh directory produced 412 input tokens, and both synced the same skills. The scratch `judge_channel_probe.py` was inspected: same model argument and auditor system prompt, tools disabled, cwd=/tmp, API-key variables stripped; its total sums uncached, cache-creation and cache-read input tokens. Thus the arithmetic accounts for caching.

The comparison varies directory history; it does not demonstrate that the compared requests differ in whether synced skill content is injected. Equal sizes also do not establish equal content. Its defensible conclusion is **no observed input-token difference between these two configurations**. It cannot by itself establish that skills add nothing to both, or that model/system-prompt differences explain the entire historical 249→412 gap.

The inspected historical `judge-init-clean.jsonl` records 249 input tokens under Haiku; `judge-init-main.jsonl` records 299 under Sonnet. Those are different model conditions, not a matched negative control for the current Opus auditor. No model calls were repeated in this review, and the 412 result remains the supplied session report rather than a new Codex measurement.

**Action before treating the channel as verified clean:** retain evidence of the actual request/context under the intended invocation, or a control that demonstrably removes the suspected injection mechanism while holding model and prompts fixed. Report the CLI/config versions and relevant context provenance. Merely creating another directory that syncs the same material repeats the same unresolved comparison. This review establishes no actual contamination.

## Prepared data: cheap verification results

Read the complete relevant JSONL inputs and reconstructed the deterministic draw/blinding with the committed functions:

| Property | Observed |
|---|---|
| Audit sample | 300 records, exactly equal to the registered draw; train 198, val 62, cal 40 |
| Blind items | 505 distinct IDs; each item has only id, sentence and paragraph |
| Item/key reconstruction | Both exactly match the committed sources and seeded blinding |
| Sources in the key | 300 audit, 193 counterexamples, 12 clean |
| Candidate counts | closed_population 50; d_semicolon 47; run_tool 50; open_artifact 44; d_adjacency 2 |
| New candidate versus new candidate cross-fold 8-token overlap | 0 distinct shared shingles, 0 affected texts |
| Candidate versus frozen other-fold 8-token overlap | 0 candidates with an overlap |
| Codex clean generation | Header input hashes match current prompt/menu; one exec event in the saved log, reading its two intended files |

The overlap check applies to the 193 committed candidates and frozen train/val/cal rows. It is not a new full leakage certification or a re-read of T/T-syn. Short semantic similarity and shared cue patterns are outside this shingle predicate.

An additional concern was checked and rejected: `train_arm.cross_cells:284` explicitly excludes counterexample rows from the cross-rule term because their paragraphs were not in the audit population. Candidate context therefore does not silently receive the audit's blanket cross-negative labels through that path.

The scorer implements the registered per-head union/Wilson admission, global (sentence, head) masking, conservative candidate acceptance and clean-text stop bars in the inspected code. Actual admission results do not exist yet and were not fabricated.

## Harness evidence and limits

Persisted artifacts:

- `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/reproduce.py`
- `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/runner-results.json`
- `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/data-checks.json`

The runner reproductions use fake models and temporary output directories. The eight existing `Labellers` tests were run as a focused baseline: all passed. Two actual in-memory mutations were then applied separately: remove pending cancellation, and turn the terminal Stop exit into success. Each still passed those eight tests: **2 surviving mutations out of 2 applied**. This is explicitly that focused test class, not a claim about every repository test. No source mutation was written into the shared checkout.

No expensive model or training tests were rerun. The previous null/permutation measurements are recorded in `docs/research/2026-09-26-codex-phase1b-stage1-review.md`; this preflight makes no new null-test claim. Validation and the gate remain design/selection surfaces, with T/T-syn reserved for the registered downstream use.

## Decision for the implementing session

Keep the current 505-item input set and use the corrected working-tree runner. Read § Fixes applied for the exact change and verification evidence; the reproduced scheduler and overwrite failures are repaired.

The remaining point is the interpretation of the 412-token control. Equal sizes in two configurations that both sync skills establish no size difference; they do not prove absence of shared injection. The channel evidence/wording is still the labelling session's item to settle before claiming a clean channel.

Review and incorporate these harness corrections into the experiment provenance before the real run. They change scheduling and evidence preservation, not labels, admission criteria, prompts or seeds. No fresh draw and no model rerun were performed.

Confidence is high for the reproduced runner failures, their targeted fixes, and exact data comparisons. Channel cleanliness remains unresolved by the provided token-count control.
