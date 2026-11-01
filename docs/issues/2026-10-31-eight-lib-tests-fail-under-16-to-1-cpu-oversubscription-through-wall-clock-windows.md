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
unverified: 'The migrate fix and the seven attribution test fixes are measured. The mutation check for a missing env detects six of the seven attribution tests. For the many-dirty-files test I measured a different mutant: a 1 ms scan timeout makes the hardened test fail 2 of 2. I did not run the missing-env mutant against it, and the old test was not run against the timeout mutant. The LSP test: 1 failure in 3 loaded runs before the change, 0 in 28 after, but the before sample is three runs, the cause of the slow request is unknown, and it is not shown that the new timeout branch ever ran. The scan floor rests on the assumption that history files are append-only; it was checked on one machine only, by the last 200 lines of each file. The scan numbers were measured under a loaded shared machine, each engine run once per set. The peer''s sighting of four tests failing once at normal load was not re-checked. Commit 421b0f896 used --no-verify on the user''s approval after a hook flagged output.rs; its checks were run by hand.'
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

LSP test: reproduced, cause of the slow request not confirmed. Alone it passed 4 of 4 under the load that failed it in the suite. Run with the whole `lsp::client::tests::` module (all rust-analyzer tests in parallel) under 64 busy loops on cores 60 to 63, it failed 1 run in 3: `called Result::unwrap() on an Err value: LSP request timed out after 30s: workspace/symbol`, the run took 51 s against 17 to 20 s for the passing runs. The test retried only on an empty answer, so a timed-out request went through `.unwrap()`. Why one request passes the 30 s ceiling is not known.

Attribution group (4 tests): confirmed, and it is a product limit as well as a test one. The shipped path `wip_author_diagnostic` spawns `python3 -I scripts/attribute-red.py`, which scans every transcript under `$HOME/.claude*` and `$HOME/.codex*` (`discover_claude_histories`, `discover_codex_histories`), with a ceiling of `ATTRIBUTION_TIMEOUT_DEFAULT` = 30 s. Timed alone on a small dirty fixture repo on 2026-10-31, with no load from me but on a machine shared with peers: 16.1 s, 20.7 s, 25.5 s (three runs, exit 0, 470 bytes out each). So a quiet call already uses 54 to 85% of the ceiling; any load pushes the scan past 30 s, the timeout returns `None`, and the test asserts "python3 is present but the engine produced no hint". That also explains the quiet-run failure of `the_same_red_at_exit_zero_still_answers`. The module comment recorded 5.0 s then 7.0 s in six days; it is 16 to 25 s now. The tests scan the real corpus of whoever runs them, so they depend on the size of that machine's history.

Correction, measured 2026-11-01: the 16 to 25 s was my fixture repo, which has no Claude history of its own. On the real checkout the old engine took 168 to 203 s per call on five sets of real dirty paths, so the 30 s ceiling cut it every time and the hint was silent. A profile of the fixture run: 14.5 of 17.3 s in `read_codex_history` over 387 Codex files (1.04 GB, 547,579 `json.loads` calls). Only 251 of those files (840 MB) mention this checkout's path, so a filter on the path would have cut about 20%.

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

Three tests in `src/tools/run_command/tests.rs`: fixed in the test code and one signature. `handle_successful_output_with` has a tenth parameter, `attribution: Option<&AttributionEnv>`. `None` is the shipped path and calls the same `wip_author_diagnostic` as before. `Some(env)` calls `wip_author_diagnostic_with`. The three tests, `a_red_attaches_wip_authors_on_the_main_arm`, `a_red_attaches_wip_authors_on_the_buffer_only_arm` and `a_red_naming_many_dirty_files_carries_a_bounded_wip_authors`, run the engine over an empty history in the precheck and in the call under test. `no_history` moved to module level under `#[cfg(test)]`, so `tests.rs` shares it. Each test alone, 64 busy loops on 2 cores: fixed, 12 of 12 loaded runs and 3 of 3 quiet runs pass. With the `Some(env)` arm calling the shipped function, the main-arm and buffer-only tests pass 0 of 4 loaded runs each. The many-dirty-files test passes 4 of 4 either way, because it turns a missing diagnostic into a green skip: against the mutant it printed `skipping: no diagnostic at 3 files` and passed in 32.7 s (one run). On the fixed build it skipped 0 of 4 loaded runs, each 24 to 32 s. Gate: `./scripts/gate.sh` in a clean worktree at `8a12d9920` with only this diff, every lane exit 0.

The commit hook refused the commit twice. It named `output.rs` as written by another session, `f6c27cf1-225c-4791-9084-17cffa386faf`, which is the older id of this conversation. The staged diff of the file was byte-identical to my diff of it in the worktree (45 lines), no peer commit touched it since my base, and the file was clean before I applied the patch. I committed with `--no-verify` on the user's explicit approval, after `rustfmt --check` on the four files. A question to the other session was held, not delivered.

- Fix SHA (on `experiments`): `421b0f896cd5facc527453fc2304796977677690`
- patch-id: `6c0483bc99a64b23305f96606bcaaa662b9d2e55`

LSP test: the retry loop treats a request timeout as "not ready yet", like an empty answer. A third timeout fails the test, and any other error fails it at once with its text. 28 of 28 loaded runs of the module passed (8, then 20), where 1 of 3 failed before. One of the 20 took 57 s, the same slowness as the failing run, and passed. The test prints nothing on a retry, so it is not shown that the new branch ran in that run. Gate: `./scripts/gate.sh` in a clean worktree at `d00107d9b` with only this diff, every lane exit 0.

- Fix SHA (on `experiments`): `56457a853`
- patch-id: `074c352b69bac3ea6f8155309d52eace357f39ce`

Many-dirty-files test hardened: `a_red_naming_many_dirty_files_carries_a_bounded_wip_authors` skips only when `python3` is absent. With `python3` present, a missing diagnostic at 3 files fails it with `python3 is present but the engine produced no diagnostic at 3 files`. `python3_present` is `pub(crate)` with a `#[cfg(test)]` re-export for `tests.rs`. 3 of 3 quiet runs pass. With a 1 ms scan timeout in `no_history()` it fails 2 of 2 with the new message; that mutation is reverted and absent from the commit. The old test against that mutant was not run. Gate: `./scripts/gate.sh` in a clean worktree at `cf76d04cd` with only this diff, every lane exit 0.

- Fix SHA (on `experiments`): `4bc4b0960`
- patch-id: `8d5ad2c56ea2e3d8c516bd5f3ae55c3af8ebe6c1`

Transcript scan (product limit), partly fixed: `scan_provenance`, `scan_with_evidence` and `scan` take an optional `modified_since`, and history files last written before it are not read. `attribute-red.py` passes the oldest last commit among the hit paths minus 24 h (`scan_floor`). An unreadable mtime keeps the file; a path with no commit, or a time that does not parse, means no cut. The assumption is that history files are append-only: on this machine none of 7656 files had a record in its last 200 lines newer than its mtime plus 2 s (596 had no timestamp there and were not judged). Old engine against new on five sets of real dirty paths in this checkout: outputs byte-identical in all five. Recently committed paths: 186 to 203 s before, 12.4 to 12.8 s after, inside the 30 s ceiling. A path committed earlier: 200 s before, 116 s after. A set with an untracked path: 168 s before, 189 s after. Three tests in `tests/test_harness_provenance.py`; flipping the mtime comparison and dropping a file with an unreadable mtime each fail them. Gate: `./scripts/gate.sh` in a clean worktree at `5411907f8` with only this diff, every lane exit 0.

- Fix SHA (on `experiments`): `14f512875`
- patch-id: `c9d9fc4e3c6b3fe5d5197f9bede19e00a1fa61b2`

Not fixed:

- The product limit for paths last committed long ago (116 s measured at a 60-day-old commit on the real checkout, against the 30 s ceiling) and for untracked paths (189 s). A new file has no commit time, so its window is unbounded. The hint is still silent there.

## Tests added

No new test. The fix changes four existing tests so they scan an empty history. The mutation check above shows that they detect a missing `.envs(...)` line: 0 of 16 loaded runs pass without it.

The three tests in `tests.rs`: the mutation check detects a missing env for two of them (0 of 4 loaded runs pass each). It cannot for the many-dirty-files test, which skips instead of failing. Its evidence is the skip count under Fix: 0 of 4 on the fixed build, a skip against the mutant.

## Workarounds

The attribution tests have a fix in their own code. Before it, at a normal gate load these were not seen to fail, except the sightings below.

- Two sightings are recorded in `78f30eb614d3fba3`.
- A peer session (`codescout-5f`) reported that four attribution-family tests in `run_command` failed once in a gate at normal load (lib lane 696 s) and passed alone and in the next two gates: `dir_trust::a_directory_swapped_in_after_first_use_is_not_run` and the three tests in `src/tools/run_command/tests.rs` listed under Fix. I did not re-check that report. It contradicts the earlier statement that these were not seen at normal load.

## Resume

State on 2026-11-01: migrate group fixed, seven attribution tests fixed, the rest open.

Open:

- `lsp::client::tests::workspace_symbols_returns_project_symbols`: tolerant of a timeout since `56457a853`. Open: why one request passes 30 s under load, and proof that the timeout branch runs (the test prints nothing on a retry).
- The product limit, remainder: old-commit and untracked paths still exceed the 30 s ceiling. Not profiled on the real checkout: how much of the 116 s is the Claude project history and how much the Codex files. Options, none chosen: a per-file cache keyed on path, size and mtime in the engine's private directory (sound, no append-only assumption, larger change); a faster Claude-history read; raising `ATTRIBUTION_TIMEOUT_DEFAULT` (does not cut the cost).
- The commit hook named a stale writer for `output.rs` (see Fix). Filed as `581b05bbe41134f3`.

## References
- `docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md`
