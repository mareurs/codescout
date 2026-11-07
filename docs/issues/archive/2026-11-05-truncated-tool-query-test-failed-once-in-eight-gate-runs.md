---
id: 99a91afd5d137c59
kind: bug
status: fixed
title: 'BUG: tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs failed once in eight gate runs because the clock-derived handle id fell in a time window that matched the test''s placeholder prefix (not load)'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-07
opened: 2026-11-05
severity: low
unverified: STANDING The end-to-end test a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs still draws its id from the clock, so the original flake is reproduced only through the helper's fixed-id test, not end to end. The gate run does not cover the three heavy Python suites, server-stack, librarian code in the lean lane, or the local ONNX weight tests.
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
Verified at the bytes, and it is not a load flake (the earlier title and Environment text pointed at load; this section corrects that). `src/tools/output_buffer.rs` `store_tool` mints ids as `format!("@tool_{:08x}", (now_ms + counter) as u32)`, so the id is derived from the clock. The test `tools::run_command::tests::a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs` asserted `!hint.contains("@tool_abc")`, a magic prefix meant to catch a placeholder. That prefix also matches the REAL id for about 17.5 minutes every 49.7 days. The failing hint named `@tool_abcbcb69`. The low 32 bits of epoch milliseconds on 2026-11-05 from 15:00 to 15:04 EET were `abcb7480` to `abcf1e00`, which is the range of the failing run. So the test failed because the clock minted an id that started with `abc`, not because of load.

## Evidence
Log of the failing run (session scratchpad, not committed): `tc-ab/logs/warm1-1.97.1.log`, lines 4066 to 4075 and 13036 to 13044. The two failures are in different nextest processes, so they are two independent failures of one test, not one failure counted twice.

## Hypotheses tried
None tested at the time of filing. Corrected later: the old Resume step suggested the test shares something with other tests (a global buffer pool, a temp path, the clock, an environment variable). Of those, only the clock was the cause, through `store_tool` ids (see Root cause). Load was not the cause; the title's "under load" and the Environment text did not hold.

## Fix
Test-only change in `src/tools/run_command/tests.rs`. No production code changed.

It adds a helper `hint_names_only_handle(hint, id)`. It is true when the hint contains `id`, and with `id` removed no other `@tool_` handle and no `<field>` remain. The assertion that used `!hint.contains("@tool_abc")` now uses this helper. The second commit makes the helper also require that the hint names the handle (so a hint with no handle fails), with a no-handle control case.

Plan deviation: None given.

Commits (labelled with the branch they were made on, fix/open-issues-batch-20261107-r2; all of them are now on experiments at 143c1941f, the same SHAs because the branch reached experiments by fast-forward):
- 68c31e85b458ac63f17a994c203d8ad067c1095f (branch fix/open-issues-batch-20261107-r2) / patch-id 5b6787338d437534f0bb2eb9ff9c043458cad422
- 7210fec74969b02b010fb2bc535b340475d18fca (branch fix/open-issues-batch-20261107-r2) / patch-id 170efb9ba3ca1b7507ab24d010c57f020e405f90

Verification: `./scripts/gate.sh --check` run on fix/open-issues-batch-20261107-r2 at its tip af3e2a9ec on 2026-11-07 gave FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0 (lean nextest 6227 tests; default lane green). The gate does NOT cover the three heavy Python suites, server-stack, librarian code in the lean lane, or the local ONNX weight tests. Each fix was reviewed adversarially by an Opus reviewer who ran the new test red against the reverted production change (or by mutants) and approved.

## Fix provenance

- **SHA:** `68c31e85b458ac63f17a994c203d8ad067c1095f` (`experiments`)
- **patch-id:** `5b6787338d437534f0bb2eb9ff9c043458cad422`

## Tests added
In `src/tools/run_command/tests.rs`:
- `tools::run_command::tests::a_real_handle_that_starts_with_the_old_placeholder_prefix_is_not_a_placeholder`. It is red with the old predicate on `Next page: sed -n '4,103p' @tool_abcbcb69.` and asserts the old predicate rejects it, then that the helper accepts it.
- `tools::run_command::tests::a_hint_naming_a_different_handle_or_a_placeholder_is_still_rejected`. A control: a hint naming a different handle, a `<field>` placeholder, an extra handle, or no handle at all is rejected (the no-handle case is one row of this test, added by the second commit).

The end-to-end test `a_truncated_tool_query_names_a_real_route_on_its_own_handle_and_it_runs` is unchanged except for its assertion, and still draws its id from the clock.

## Workarounds
Rerun the gate. A single failure that passes on rerun is still a failure to record; do not treat it as noise without reading the test.

## Resume

The fix landed on experiments (commits 68c31e85b458ac63f17a994c203d8ad067c1095f and 7210fec74969b02b010fb2bc535b340475d18fca, tip 143c1941f, reached by fast-forward from fix/open-issues-batch-20261107-r2, so the SHAs are unchanged). `./scripts/gate.sh --check` on that tree gave FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0, PYTHON_LIGHT=0. The file was then archived with `doc(action="move", ...)` to `docs/issues/archive/`; the experiments SHA is recorded under Fix provenance.

Nothing else is pending for the root cause. The original flake is not reproduced end to end, only through the helper's fixed-id test.

## References
- `docs/trackers/rust-toolchain-upgrade.md`, row 6
- codescout memory `research/rust-toolchain-upgrade-20261105`
