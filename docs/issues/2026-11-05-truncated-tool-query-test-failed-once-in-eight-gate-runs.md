---
id: '649df5b30d126f2b'
kind: bug
status: open
title: 'BUG: tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs failed once in eight gate runs under load'
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-11-05
severity: low
unverified: One observation in eight gate runs (two lane failures of the same run). The cause is not known and the failure was not reproduced. The test body was not read.
---

# BUG: a run_command test failed once in eight gate runs

## Summary
`tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs` failed once in the lean lane and once in the default lane of ONE gate run, and passed in the seven other runs of the same measurement. The assertion at `src/tools/run_command/tests.rs:3243` saw the text "Output cut to fit the response budget (stdout 3/6)".

## Class fit
A test that depends on something outside its own fixture. I did not read the cluster roster, so `cluster/unclassified` is used until a class is adjudicated.

## Symptom (Effect)
In run `warm1` on toolchain 1.97.1 (2026-11-05, about 15:00 to 15:04), both nextest lanes ended with `Summary ... 1 failed`. The lean lane failed test 4025 of 6162, the default lane test 6769 of 9030. The failure took 0.054 s and 0.061 s. The stderr line: `thread '...a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs' panicked at src/tools/run_command/tests.rs:3243:5`, then `Output cut to fit the response budget (stdout 3/6). Next page: sed -n '4,103p' @tool_abcbcb69.` The gate exit was `LEAN=100 DEFAULT=100`.

## Reproduction
Not reproduced. The same test passed in the cold run and in every other warm run of both toolchains (1.97.1 and 1.99.1, same frozen HEAD `6bd3745e4`).

## Environment
Worktree `rust-1.99.1` at `6bd3745e4`, gate through `scripts/with-slot.sh --slot ram-2`, machine load 4.5 cores of other work, nextest default profile. That run had also recompiled the main crate (see the build.rs bug), so it started right after a build.

## Root cause
Not found. I have not read the test body at `tests.rs:3243`.

## Evidence
Log of the failing run (session scratchpad, not committed): `tc-ab/logs/warm1-1.97.1.log`, lines 4066 to 4075 and 13036 to 13044. The two failures are in different nextest processes, so they are two independent failures of one test, not one failure counted twice.

## Hypotheses tried
None tested.

## Fix
Not done.

## Tests added
None.

## Workarounds
Rerun the gate. A single failure that passes on rerun is still a failure to record; do not treat it as noise without reading the test.

## Resume
1. Read the test at `src/tools/run_command/tests.rs` around line 3243 and find what it shares with other tests (a global buffer pool, a temp path, the clock, an environment variable).
2. Run it alone many times (`cargo nextest run -E 'test(a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs)' --retries 0` in a loop) and under the full suite with the same load.

## References
- `docs/trackers/rust-toolchain-upgrade.md`, row 6
- codescout memory `research/rust-toolchain-upgrade-20261105`
