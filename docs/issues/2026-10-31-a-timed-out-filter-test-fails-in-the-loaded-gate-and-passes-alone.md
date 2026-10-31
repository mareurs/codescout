---
id: '78f30eb614d3fba3'
kind: bug
status: open
title: 'BUG: a_timed_out_filter_with_output_over_the_limit_stays_inline fails once in a loaded gate lane and passes alone (seen twice, cause unproven)'
tags:
- cluster/repro-env-diverges-from-gate-env
- gate
- flaky-test
- run_command
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
Not yet reproducible: best lead is to run the lean lane while the machine is busy with other test runs.

What I did observe myself, on 2026-10-31:
- `scripts/with-slot.sh cargo test --no-default-features --lib a_timed_out_filter_with_output_over_the_limit_stays_inline` in the same worktree: `1 passed; 0 failed`, 1.26 s.
- Four later full gates on merged trees: lean lane `LEAN=0` every time, the test among them.

## Environment
Linux, `experiments` branch. Several Claude sessions and their gates ran on the machine at the same time; `scripts/peer-sessions.sh` listed 28 live sessions that day.

## Root cause
Unknown, under investigation. Hypothesis, untested: the test runs `sh -c 'seq 1 200000; sleep 30' | grep .` with `timeout_secs: 1` (`src/tools/run_command/tests.rs:6862-6890`) and asserts that `stdout_total` is `200000`. That assertion holds only if `seq` and `grep` finish all 200000 lines inside the one-second wall-clock window, before the timeout kills the process group. On a loaded machine they may not, and then the capture holds only the lines written so far. 90598 of 200000 is consistent with a kill part way through. The `sleep 30` makes the timeout fire on purpose, so the one-second window is also the test's own deadline.

## Evidence
- The test body, read at `src/tools/run_command/tests.rs:6862-6890`.
- Passed alone (above), and in four full gates; failed in two loaded ones (one of them by report only).
- Not checked: whether the failing run's `stdout_total` varies from run to run, and whether the other assertions in the test passed in the failing run.

## Hypotheses tried
None run. The load hypothesis above could be tested by running the test under artificial CPU contention (for example with many `yes > /dev/null` processes) and watching `stdout_total`.

## Fix
Not done. Options, none tried: raise `timeout_secs` so the producer always finishes first (the test then takes that long); make the producer's output bounded by something other than wall-clock time; or assert `stdout_total` is positive and at most 200000 and keep the clamp assertions, which are what the test is about.

## Tests added
N/A: no fix yet, and the test is the thing under suspicion.

## Workarounds
Re-run the test alone. If it passes, treat the gate failure as this bug, and say so in the report instead of calling the lane green.

## Resume
Reproduce under load first. Until then this is two sightings and a hypothesis. Also seen once, by a peer session and not by me: `lsp::manager::tests::a_silent_socket_that_appears_late_still_ends_the_connect_with_the_window` took 4.4 s inside a 3 s window in a full run and passed alone three times. It may share the cause (a wall-clock window under load); that is not established.

## References
- `src/tools/run_command/tests.rs:6862-6890`
- `docs/issues/archive/2026-09-24-residual-doc-get-line-range-frame-decision.md` (first sighting)
