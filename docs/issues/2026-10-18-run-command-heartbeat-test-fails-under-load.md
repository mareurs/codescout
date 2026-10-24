---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-24
opened: 2026-10-18
owner: marius
related: []
severity: low
unverified: The failure was NOT reproduced by CPU load alone (96 of 96 passed under 192 burners); only an injected process stall reproduced it. That the 2026-10-18 gate failure was a stall of that kind is the mechanism the evidence supports, not something measured.
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

Fixed in `src/tools/run_command/tests.rs` (a test flaw; no production change). The old shape ran `sleep 5` under a 6 s timeout and raced the command's exit against the first 3 s heartbeat tick, so a stall past the command's end made the call return first. The command now loops until a marker file exists; the test writes the marker only after it has seen TWO heartbeats, and asserts the rate and text: N1 >= 3 and N2 - N1 >= 3 parsed from `"{N}s elapsed"`. The 20 s wait and `timeout_secs: 120` are upper bounds for a dead heartbeat, not the pass condition.

- Fix SHA (branch `integration/fixes-20261024`, not yet on `experiments`): `982a9403e0f286532182db199716e8b18f880397`
- patch-id: `b9806d894021619d934135c41e637e868d691738`

An independent Opus review found the first version no longer checked the heartbeat rate; the rate and text assertions are the fix for that.

## Tests added

`run_command_heartbeat_emits_progress_text` rewritten. Mutation checks against it, each reverted: interval 3 s to 30 s fails, 3 s to 1 ms fails, 3 s to 3600 s fails. With a SIGSTOP stall injected (stop at 1.5 s for 5.5 s) it passes 6 of 6 where the old test passed 0 of 6. Gate on the merged tree: exit 0.

## Workarounds

Rerun the single test, and read a red from it on an otherwise green gate as a load flake until the cause is
established.

## Resume

Measure the heartbeat's first tick time under synthetic load before changing the test's budget.

## References

- `src/tools/run_command/tests.rs`
