---
status: open
opened: 2026-10-18
closed:
severity: low
owner: marius
related: []
tags:
- cluster/unclassified
kind: bug
unverified: 'Cause not established: seen once, under load, in the default lane of a gate run, and it passed in the lean lane of the same run and in an isolated rerun. The suspected load cause is reasoned, not reproduced.'
---

# BUG: `run_command_heartbeat_emits_progress_text` fails under load: a 5 s sleep under a 6 s timeout leaves one heartbeat no margin

## Summary

The test runs `sleep 5` with `timeout_secs: 6` and asserts that at least one 3-second heartbeat
`report_text()` fired. In one `./scripts/gate.sh --check` run (worktree `agent-a8918c1498fdd30e8`, 2026-10-18), the
default lane failed on it with `expected at least 1 report_text() from run_command heartbeat`. That lane took 1,428 s
while mutation probes were also running on the host. The lean lane of the same gate passed it, and so did an
isolated rerun (`scripts/with-slot.sh cargo test --lib -- tools::run_command::tests::run_command_heartbeat_emits_progress_text`,
1 passed, 5.06 s).

## Class fit

No class fits cleanly, so this is `cluster/unclassified`. It is a timing assertion with a fixed wall-clock budget that a
loaded host can miss. It differs from a logic regression because the code under test was not changed in the run that
failed (the branch touched only `src/librarian`).

## Symptom (Effect)

The gate goes red (`DEFAULT=101`) on a branch that did not touch `run_command`.

## Reproduction

Not reproduced on demand. It was seen once, under load, as described in the Summary.

## Environment

Linux, `cargo test --workspace` default features, host loaded by concurrent `scripts/mutation-probe.sh` runs.

## Root cause

Unknown, under investigation. Suspected, not shown: the heartbeat task's first tick, or the command's start, slips
under load, so no tick lands inside the window.

## Evidence

`src/tools/run_command/tests.rs` (`run_command_heartbeat_emits_progress_text`): the assertion message above, from the
gate log of that run.

## Hypotheses tried

None run.

## Fix

N/A, not started.

## Tests added

N/A: the defect is in a test.

## Workarounds

Rerun the single test, and read a red from it on an otherwise green gate as a load flake until the cause is
established.

## Resume

Measure the heartbeat's first tick time under synthetic load before changing the test's budget.

## References

- `src/tools/run_command/tests.rs`
