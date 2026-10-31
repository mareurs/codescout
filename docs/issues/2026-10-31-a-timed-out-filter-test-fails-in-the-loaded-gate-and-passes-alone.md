---
id: '78f30eb614d3fba3'
kind: bug
status: fixed
title: 'BUG: a_timed_out_filter_with_output_over_the_limit_stays_inline fails once in a loaded gate lane and passes alone (seen twice, cause unproven)'
tags:
- cluster/repro-env-diverges-from-gate-env
- gate
- flaky-test
- run_command
closed: 2026-10-31
opened: 2026-10-31
owner: marius
severity: low
---

# BUG: a_timed_out_filter_with_output_over_the_limit_stays_inline fails once in a loaded gate lane and passes alone (seen twice, cause unproven)

## Summary
`tools::run_command::tests::a_timed_out_filter_with_output_over_the_limit_stays_inline` has failed in a full `scripts/gate.sh` lane that ran next to other sessions, and has passed every time it was run alone or in a quieter gate. It was seen twice, in two sessions. Nobody has reproduced the failure on demand, so the cause below is a hypothesis.

## Class fit
The test passes in the run a person uses to check it (alone, in a worktree with nothing else running) and fails in the run that gates (the whole lane, on a machine shared with other sessions). That is the trait of this class: the reproduction environment is not the gating environment, so the local run reads as authoritative and wrong. It differs from the class's other members in that both runs use the same toolchain and the same commit; only machine load differs.

## Symptom (Effect)
Second sighting, reported by a subagent that ran the gate's lean lane (`cargo test --workspace --no-default-features`) on a branch based on `0f8705532`. I did not read the failing output myself, only the subagent's report of it:

```
tools::run_command::tests::a_timed_out_filter_with_output_over_the_limit_stays_inline
src/tools/run_command/tests.rs:6889
assertion expected 200000 but got 90598
```

Line 6889 is the last assertion of the test, `assert_eq!(parsed["stdout_total"], 200000, ...)`. The same lane passed 5380 other tests. The default lane (8078 tests) passed.

First sighting: `docs/issues/archive/2026-09-24-residual-doc-get-line-range-frame-decision.md`, line 39. A first full run failed this test, it "passed alone and in the second full run", and its author "did not find its cause".

## Reproduction

Reproduced on demand on 2026-10-31. The suite's test binary was pinned with `taskset -c 62,63` and 64 `yes > /dev/null` loops ran on the same two cores. The test ran 20 times: 14 passed and 6 failed. The failing runs reported `stdout_total` 148115, 177372, 156307, 42811, 72849 and 40081, each time on the last assertion. Without load, 30 of 30 runs passed (10 with no loops, 20 with 8 loops). The scripts are `flaky-load.sh` and `lease-run.sh` in the session scratchpad; the first needs only the test binary path in `BIN`.

## Environment
Linux, `experiments` branch. Several Claude sessions and their gates ran on the machine at the same time; `scripts/peer-sessions.sh` listed 28 live sessions that day.

## Root cause

The load hypothesis is confirmed. The test runs `sh -c 'seq 1 200000; sleep 30' | grep .` with `timeout_secs: 1`. The process group is killed at the timeout, so `stdout_total` is the number of lines the producer and the filter finished before the kill. That is 200000 only when the machine is quiet. The assertion `stdout_total == 200000` tested the producer's speed, which the clamp does not promise.

## Evidence
- The test body, read at `src/tools/run_command/tests.rs:6862-6890`.
- Passed alone (above), and in four full gates; failed in two loaded ones (one of them by report only).
- Not checked: whether the failing run's `stdout_total` varies from run to run, and whether the other assertions in the test passed in the failing run.

## Hypotheses tried

Load: confirmed, see Reproduction. A first load run with 8 loops on two cores did not reproduce it (20 of 20 passed); 64 loops did.

## Fix

The exact total is replaced by a bound: `shown < total && total <= 200000`. The test is about the clamp, so it keeps what the clamp promises: the total counts the lines the clamp cut away, and never exceeds what the producer could write.

- Fix SHA (on `experiments`): `1910afb125440aebb993f8170ae99532b4533bd7`
- patch-id: `e8912e70e20ceaf872eb8872457a7e48014efb34`

After the change, 30 of 30 runs passed under the same 64-loop load that failed 6 of 20 before. That count is weak evidence alone: at a 30% failure rate, 30 clean runs have a chance of about 0.00002, but a clean run also fits a test that cannot fail. The mutation below is the stronger check.

## Tests added

No new test: the changed assertion in `tools::run_command::tests::a_timed_out_filter_with_output_over_the_limit_stays_inline` is the regression test. Mutation: in `attach_partial_capture` (`src/tools/run_command/output.rs`), `response["stdout_total"] = json!(cut.shown)` instead of `json!(cut.total)`. The test failed at `tests.rs:6894`, the new assertion. The mutant was reverted.

## Workarounds
Re-run the test alone. If it passes, treat the gate failure as this bug, and say so in the report instead of calling the lane green.

## Resume
Reproduce under load first. Until then this is two sightings and a hypothesis. Also seen once, by a peer session and not by me: `lsp::manager::tests::a_silent_socket_that_appears_late_still_ends_the_connect_with_the_window` took 4.4 s inside a 3 s window in a full run and passed alone three times. It may share the cause (a wall-clock window under load); that is not established.

## References
- `src/tools/run_command/tests.rs:6862-6890`
- `docs/issues/archive/2026-09-24-residual-doc-get-line-range-frame-decision.md` (first sighting)
