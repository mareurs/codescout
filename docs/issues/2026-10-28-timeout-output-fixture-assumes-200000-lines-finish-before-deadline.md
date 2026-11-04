---
id: 3ea82aacec5a211a
kind: bug
status: fixed
title: Timeout-output fixture assumes 200000 lines finish before its deadline
owners:
- marius
tags:
- cluster/unclassified
- testing
- run-command
closed: 2026-11-04
opened: 2026-10-28
severity: medium
unverified: The fix is by reading its assertion and its comment; no contention run was made here. The mutation check this record asked for (removing the timeout-output clamp) was not run.
---

# BUG: Timeout-output fixture assumes 200000 lines finish before its deadline

## Summary

The unchanged repository gate failed in the lean lane because a timed-out capture contained 78310 lines while its test requires exactly 200000. The same binary's single-test rerun passed without source changes. The fixture's exact-count assertion depends on how much work completes before a one-second deadline.

## Class fit

Filed as cluster/unclassified: the observed failure is an execution-speed assumption inside a timeout fixture. It is not the already-recorded temporary-root contamination, socket-length or symlink-root problems; no matching class mechanism has been established.

## Symptom (Effect)

The mandatory gate reports a failure even though the fixture is explicitly exercising partial capture after timeout. This blocks a fully green research verification result. No production defect has been established by this failure alone.

## Reproduction

Run ./scripts/gate.sh on research/checked-real-task-20261024 at 1023edc172b17d389f70a5923bae223a732632b9 with a short private /var/tmp TMPDIR. On 2026-10-28 the lean lane reported 5239 passed, one failed, 11 ignored. The assertion at src/tools/run_command/tests.rs:6813 compared stdout_total 78310 with 200000.

The same built lean binary /home/marius/.cache/codescout-gate/slot-0/debug/deps/codescout-e6dc2d91b2b23543, queried only for tools::run_command::tests::a_timed_out_filter_with_output_over_the_limit_stays_inline with --exact --nocapture in another short private /var/tmp root, passed one test in 2.03 seconds.

## Environment

2026-10-28; Linux; isolated research worktree /tmp/codescout-real-task-20261024. A model author and repository gate ran concurrently. This observation does not isolate CPU contention as the cause. Gate TMPDIR /var/tmp/h9drdy7x; targeted probe /var/tmp/_u_uiu_q.

## Root cause

The test runs sh -c 'seq 1 200000; sleep 30' through grep with timeout_secs 1, then requires stdout_total to equal 200000. The partial prefix observed on the failed run disproves the fixture's assumption that all 200000 lines always reach capture before timeout. Why that run reached 78310 specifically has not been measured. The single-test pass establishes non-reproduction in isolation, not a fix.

## Evidence

Private evidence: .codescout/resident-private/producer-status-xhigh-extension-20261028/gate.log, timeout-probe.log and timeout-probe-result.json in the main repository. Source/build hashes are recorded by gate-started.json and final gate-result.json. Research experiment outcomes use their own unchanged protected contract.

## Hypotheses tried

A single-test rerun using the exact gate binary passed. No source mutation or production fix was applied. No controlled contention experiment was run.

## Fix

Already fixed on `experiments` by commit `1910afb12` (2026-10-31), which this record, written on a research branch, did not know. The test `a_timed_out_filter_with_output_over_the_limit_stays_inline` no longer asserts `stdout_total == 200000`. It asserts `shown < total && total <= 200000`, and its comment says why: how many lines the producer wrote before the one-second kill depends on machine load (40081 to 177372 were seen under contention), so the total is bounded, not exact. The over-limit and timeout assertions are kept (inline delivery, no `@tool_` wrap, `timed_out`, `capture_incomplete`, a handle, `truncated`).

This record is a duplicate of bug `78f30eb614d3fba3` (`docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md`, status fixed): same test, same failure. This record's own figure, 78310 lines against 200000, fits inside the range the fix's comment names.

## Tests added

None in this change. The repair is the relaxed assertion in `1910afb12`. Whether a mutation that removes timeout-output clamping is still detected by the revised test was asked for in this record's Resume; it was not run here.

## Workarounds

The isolated rerun provides diagnostic evidence but does not turn the original gate green.

## Resume

Nothing open on `experiments`, except the mutation check named above.

## Fix provenance

- **SHA:** `1910afb12` (`experiments`)
- **patch-id:** `e8912e70e20ceaf872eb8872457a7e48014efb34`

The commit is an ancestor of `HEAD` and of `origin/experiments` (checked 2026-11-04).

Checked 2026-11-04 on the current tree: `cargo test --lib tools::run_command::tests::a_timed_out_filter_with_output_over_the_limit_stays_inline` gives 1 passed, 0 failed.

## References

- Duplicate of `78f30eb614d3fba3` (fixed).
- The research extension plan exists only on the research branches (commit `f41b306b4`).
