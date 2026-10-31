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
unverified: The migrate and four-test attribution fixes are measured (16 of 16 loaded runs and a mutant that fails 0 of 16 for the attribution tests). The three tests in run_command/tests.rs are not covered. The LSP test does not fail alone. The peer's sighting of four tests failing once at normal load was not re-checked.
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

Attribution group, the four tests listed under Symptom: fixed in the test code only. `AttributionEnv` has a new field, `engine_env`, a list of variables set on the engine child. It is empty on the shipped path. A test helper, `no_history`, points `FILE_PROVENANCE_ROOTS` at an empty directory and `FILE_PROVENANCE_CODEX_ROOTS_JSON` at `[]`. The engine then takes about 0.12 s and prints the same 470 bytes. Each of the four tests alone, 64 busy loops on 2 cores: before the fix 4 of 4 runs failed for each test. After the fix 16 of 16 loaded runs and 4 of 4 quiet runs pass. With the `.envs(...)` line removed, 0 of 16 loaded runs pass and 4 of 4 quiet runs pass. Gate: `./scripts/gate.sh` in a clean worktree at `304a66a55` with only this diff, every lane exit 0. A gate in the shared tree was red in other sessions' unfinished files.

- Fix SHA (on `experiments`): `b33315316110f787b58cb88997ed3e3bf13939a5`
- patch-id: `3449c12dd208615c1fcb7a069a59b015c0be17c9`

Not fixed:

- The product limit: the real transcript scan takes 16 to 25 s against a 30 s ceiling.
- Three tests in `src/tools/run_command/tests.rs`: `a_red_attaches_wip_authors_on_the_main_arm`, `a_red_attaches_wip_authors_on_the_buffer_only_arm` and `a_red_naming_many_dirty_files_carries_a_bounded_wip_authors`. They reach the engine through `output::handle_successful_output`, which calls `wip_author_diagnostic` with `from_env`. The env must be threaded through that path.
- `lsp::client::tests::workspace_symbols_returns_project_symbols`.

## Tests added

No new test. The fix changes four existing tests so they scan an empty history. The mutation check above shows that they detect a missing `.envs(...)` line: 0 of 16 loaded runs pass without it.

## Workarounds

The attribution tests have a fix in their own code. Before it, at a normal gate load these were not seen to fail, except the sightings below.

- Two sightings are recorded in `78f30eb614d3fba3`.
- A peer session (`codescout-5f`) reported that four attribution-family tests in `run_command` failed once in a gate at normal load (lib lane 696 s) and passed alone and in the next two gates: `dir_trust::a_directory_swapped_in_after_first_use_is_not_run` and the three tests in `src/tools/run_command/tests.rs` listed under Fix. I did not re-check that report. It contradicts the earlier statement that these were not seen at normal load.

## Resume

State on 2026-10-31: migrate group fixed, the four attribution tests fixed, the rest open.

Open:

- Three tests in `src/tools/run_command/tests.rs` that go through `output::handle_successful_output`. They need `engine_env` threaded through that path, which is a wider change.
- `lsp::client::tests::workspace_symbols_returns_project_symbols`: a 30 s LSP request timeout. It passes alone under load, so it needs the whole suite's contention.
- The product limit: the red-attribution hint is silently absent whenever the transcript scan exceeds 30 s. Options, none chosen: raise `ATTRIBUTION_TIMEOUT_DEFAULT`, or make the scan cheaper with an index or a cache. The cheaper scan is the larger change.

## References
- `docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md`
