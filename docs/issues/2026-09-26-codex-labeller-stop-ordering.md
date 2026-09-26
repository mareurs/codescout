---
id: ea84bb940596ddf3
kind: bug
status: fixed
title: 'Codex: ordered labeller results delay terminal failure and start pending calls'
tags:
- codex
- phase1b
- cluster/guard-narrower-than-its-name
closed: 2026-09-26
severity: high
---

# Codex: the labeller stop waits behind earlier batches

**Valid:** dated 2026-09-26

## Summary

The registered Phase 1b runner promises to cancel Claude batches not yet started after a call fails twice. In a controlled fake-model run, batch 1 failed twice while batch 0 waited; batches 2 through 20 still started. All 21 batches ran, making 22 fake calls including the retry, before the runner returned exit 4.

## Symptom (Effect)

Exit 4 and absence of labels are correct, but the stop does not bound further model expenditure when an earlier batch is slow. This matters before launching the real 21-batch labelling job.

## Reproduction

At checkout HEAD `16cef832`, with the relevant scripts unchanged from `d423236b`:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/reproduce.py
```

The reproducer imports the real runner, substitutes fake model calls, creates temporary audit files, and blocks external subprocess calls. Batch 0 waits for batch 20; batch 1 fails twice; the remaining batches return valid synthetic answers. This constructs an ordering, without relying on sleep timing or spending model calls.

## Environment

Shared Linux checkout, Python standard-library ThreadPoolExecutor; four workers, 505 existing blind items, 21 batches. No real labels were produced or changed.

## Root cause

`docs/evals/data/2026-09-24-rule-tell/phase1b/run_labellers.py:187` consumes `ex.map` in input order. A later future's Stop is not propagated to the main thread while it is waiting for an earlier future. That failed worker can take more queued jobs. `shutdown(cancel_futures=True)` at line 189 only runs once the ordered iterator raises.

Measured 2026-09-26 by the fake-model reproduction above, not inferred solely from source.

## Evidence

Saved `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/runner-results.json`, `ordered_stop`:

- distinct_batches_started: 21
- total_calls_including_retries: 22
- distinct_batches_started_after_second_failure: 2 through 20
- exit_code: 4
- label_file_exists: false

The eight existing Labellers tests pass. Two actual in-memory mutations also pass those eight tests: remove pending cancellation; convert the Stop exit from 4 to 0. Observed surviving mutations: 2 of 2 in that focused suite. The registered 45-of-45 mutation result covers a different set of mutations; it does not establish this main-path contract.

## Hypotheses tried

The hypothesis that cancellation in the finally block prevents further starts was falsified by the controlled ordering above.

## Fix

Implemented in the shared working tree. `run_claude_batches` holds at most the worker count in pending futures, observes completed futures before refilling, and sets a worker-visible stop event on failure. Each attempt checks the event; cancellation bypasses the retry path. Results are reassembled in original item order. Already running calls can finish. See `docs/research/2026-09-26-codex-phase1b-labelling-preflight-review.md` § Fixes applied by Codex.

**Committed** in `14346eb4` (patch-id `c453deb0228aff223e47e6346ee778cdcab66d6f`) by the integrating session (571eb3d6-c879-43f6-b3f9-5a51e744e1af), before any labelling run.

## Tests added

`tests/test_phase1b_audit.py::LabellerMain` now exercises the real main path with fake models, including a slow first batch, later terminal failure, suppression of an in-flight batch's retry, preserved output order and exit 4. The complete Python audit file passes 43 tests. Eight applied mutations against the nine new tests were all killed, including reintroducing the ordered-map scheduler. Evidence: `docs/evals/data/2026-09-24-rule-tell/phase1b/codex-preflight-review/fix-verification.json`.

**Re-verified at commit with `scripts/mutation-probe.sh`** (isolated worktrees, not in-memory): the worker-side stop signal survived, because the controller's `finally` also sets it and both tests released the blocked batch only after the controller had seen the failure. A seam test now holds the controller in its first `wait` (`test_workers_see_a_terminal_failure_before_the_controller_does`, 5 of 5 passes, mutation killed). The item-order test was racy (the ordering mutation survived one run in two) and now releases batch 0 on batch 2's start (3 of 3 kills). The refill loop's stop check survives by design, commented in the code: every attempt re-checks the signal, so it can save a submission but never a model call. 45 tests; 59 of 60 mutations killed over the Step 1-2 scripts.

## Workarounds

Use the corrected runner. No expensive labelling was started during this work.

## Resume

The code correction and targeted verification are complete. The implementing session should incorporate the working-tree diff and this evidence into experiment provenance before the real run. Codex made no commit; the integrating session committed it in `14346eb4` (patch-id `c453deb0228aff223e47e6346ee778cdcab66d6f`).

## References

- `docs/evals/phase1b-local-classifier-preregistration.md` — Stage 2, Step 1 scripts.
- `tests/test_phase1b_audit.py` — Labellers helper tests.
