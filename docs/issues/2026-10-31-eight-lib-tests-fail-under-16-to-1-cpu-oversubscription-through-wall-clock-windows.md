---
id: '20927ccfe293c34e'
kind: bug
status: open
title: 'BUG: eight lib tests fail when the suite runs on cores oversubscribed 16 to 1, each through a wall-clock window or a subprocess deadline'
tags:
- cluster/repro-env-diverges-from-gate-env
opened: 2026-10-31
owner: marius
severity: low
unverified: One run at an artificial 16-to-1 load; none of the 8 failures was re-run alone, and the cause of each is not established.
---

# BUG: eight lib tests fail when the suite runs on cores oversubscribed 16 to 1, each through a wall-clock window or a subprocess deadline

## Summary
One run of the whole `--lib` suite (`--no-default-features`) pinned to four cores with 64 `yes > /dev/null` loops on the same cores took 1600 s and failed 8 of 5394 tests. Each failure waits on a clock or a subprocess. This is an artificial load, far past a real gate lane, so it shows which tests have a wall-clock dependence, not that the gate is flaky at normal load. One run only; none of the 8 was re-run alone.

## Class fit
`cluster/repro-env-diverges-from-gate-env`, the class of `78f30eb614d3fba3`: a test passes in the quiet run a person uses and fails under load. Here the load was made on purpose, so the failures are a map of load sensitivity, not a gate sighting.

## Symptom (Effect)
The 8 failures and their first panic line:
- `lsp::client::tests::workspace_symbols_returns_project_symbols`, `src/lsp/client.rs:2390`: `LSP request timed out after 30s: workspace/symbol`.
- `migrate::sqlite_memory_vectors::hardening_tests::byte_identical_backup_replacement_is_refused_through_the_public_rebuild_call` (`sqlite_memory_vectors_hardening_tests.rs:547`), `empty_rebuild_still_rechecks_every_backup_byte_before_commit` (`:578`) and `same_inode_backup_corruption_is_caught_through_the_public_rebuild_call` (`:519`): `probe did not ...: Err(public rebuild did not reach the coordinated writer wait)`. In each, the rebuild itself returned `Ok`.
- `tools::run_command::attribution::tests::a_module_planted_beside_the_entry_does_not_run_through_the_shipped_path` (`attribution.rs:822`), `dir_trust::a_directory_swapped_in_after_first_use_is_not_run` (`:1208`), `a_red_naming_a_dirty_file_reaches_the_engine` (`:1558`) and `the_same_red_at_exit_zero_still_answers` (`:1606`): the python engine produced no hint (`got None`).

## Reproduction
`scripts/with-slot.sh bash lease-suite.sh 64 <out>` (session scratchpad of `f6c27cf1-225c-4791-9084-17cffa386faf`): build the lib tests, start 64 `taskset -c 60-63 yes` loops, run the test binary under `taskset -c 60-63`. The same session ran `lsp::manager::tests::a_silent_socket_that_appears_late_still_ends_the_connect_with_the_window` in that run and under 64 loops on two cores (15 of 15): it passed both times, so the peer's one sighting of it is not reproduced.

## Environment
Linux, `experiments` at `6ad4a5e4d` plus peers' uncommitted edits in the tree. A rust-analyzer started cold inside the run.

## Root cause

Migrate group (3 tests): confirmed. The probe samples the test thread's `/proc/self/task/<tid>/syscall` for a `clock_nanosleep` for at most 5 s. With a diagnostic added, one failing run showed 107 samples in 5 s: 105 `running` and two syscall 74 (`fsync`), never a sleep. The rebuild had not reached SQLite's busy wait; it was still doing its own work, starved of CPU. So the 5 s deadline measured the load.

LSP test: not confirmed. Alone it passed 4 of 4 under the load that failed it in the suite, so it needs the whole suite's contention.

Attribution group (4 tests): confirmed, and it is a product limit as well as a test one. The shipped path `wip_author_diagnostic` spawns `python3 -I scripts/attribute-red.py`, which scans every transcript under `$HOME/.claude*` and `$HOME/.codex*` (`discover_claude_histories`, `discover_codex_histories`), with a ceiling of `ATTRIBUTION_TIMEOUT_DEFAULT` = 30 s. Timed alone on a small dirty fixture repo on 2026-10-31, with no load from me but on a machine shared with peers: 16.1 s, 20.7 s, 25.5 s (three runs, exit 0, 470 bytes out each). So a quiet call already uses 54 to 85% of the ceiling; any load pushes the scan past 30 s, the timeout returns `None`, and the test asserts "python3 is present but the engine produced no hint". That also explains the quiet-run failure of `the_same_red_at_exit_zero_still_answers`. The module comment recorded 5.0 s then 7.0 s in six days; it is 16 to 25 s now. The tests scan the real corpus of whoever runs them, so they depend on the size of that machine's history.

Product effect, not yet measured in use: on this machine the red-attribution hint is silently absent whenever the scan exceeds 30 s. The code treats a timeout as silence by design (`_ => return None`).

## Evidence
`suite-load.txt` in that scratchpad: `5386 passed; 8 failed; 12 ignored; finished in 1600.35s`. Not checked: whether any of the 8 fails alone, whether the attribution failures are the python engine's own timeout or something else, and whether the tree's uncommitted edits play a part.

## Hypotheses tried
None.

## Fix

Migrate group: fixed. The probe's deadline is 60 s instead of 5 s, and its failure message carries the syscall counts it saw. No assertion changed. Under the same load, 4 of 4 runs of each of the three pass (12 of 12; 12 of 12 failed before).

- Fix SHA (on `experiments`): `0e59c5cd19af0d2e4ea8c7d91451b1b7a0c0e11e`
- patch-id: `393bd5aea1ffe752a18612582ca7e59dee85ef42`

LSP and attribution groups: not fixed.

## Tests added
N/A.

## Workarounds
At a normal gate load these have not been seen to fail, except the two sightings recorded in `78f30eb614d3fba3`.

## Resume

Open: the four `attribution` tests and `lsp::client::tests::workspace_symbols_returns_project_symbols` (30 s LSP request timeout; passes alone under load).

Attribution needs a design choice before a change; none is made:
- Point the engine at an empty history in the tests. The engine reads `HOME` (and `FILE_PROVENANCE_ROOTS` as an override). The shipped spawn path sets no environment on the child, so this needs a field on `AttributionEnv` that `wip_author_diagnostic_in` applies with `Command::env`. This keeps the tests off the machine's corpus and matches the module's rule of no process-global state (see `docs/issues/archive/2026-09-13-an-env-var-set-across-an-await-races-every-sibling-test-that-reads-it.md`). It leaves the product limit as it is. The fixture's file is unattributable, so the `COVERAGE` assertion should hold with an empty history; that is not checked.
- Raise `ATTRIBUTION_TIMEOUT_DEFAULT`. This moves the cliff and changes how long a failing `run_command` can wait.
- Make the scan cheaper (an index or a cache of the transcript scan). This addresses the product limit, and it is the largest change.

## References
- `docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md`
