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
unverified: 'The migrate fix and the seven attribution test fixes are measured. The mutation check for a missing env detects six of the seven attribution tests. For the many-dirty-files test I measured a different mutant: a 1 ms scan timeout makes the hardened test fail 2 of 2. I did not run the missing-env mutant against it, and the old test was not run against the timeout mutant. The LSP test: 1 failure in 3 loaded runs before the change, 0 in 28 after, but the before sample is three runs, the cause of the slow request is unknown, and it is not shown that the new timeout branch ever ran. The scan floor rests on the assumption that history files are append-only; it was checked on one machine only, by the last 200 lines of each file. The history cache and the policy-loop change were shown equal to the old code on this machine''s corpus only, in one frozen run each (the harness sorts gaps, so it cannot see a change in gap order); the 374-gap difference between separate runs was attributed to drift but not traced. The timings were measured on a shared machine under load, one to a few runs each: the policy loop''s 22% is two runs per version. The Rust caller was run end to end once (18.5 s, one sample under load), not as a distribution. The new attribution columns have no data yet, and why hints stopped on 2026-10-20 is unknown. The whole gate was not rerun after rustfmt changed three files; the format check, usage:: and run_command::attribution were. normalize now compares path case exactly, which Windows Path.relative_to did not; Windows support of this engine is unproven. The peer''s sighting of four tests failing once at normal load was not re-checked. Commit 421b0f896 used --no-verify on the user''s approval after a hook flagged output.rs; its checks were run by hand.'
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

Transcript scan, second and third changes (2026-11-01).

Hoist: `read_claude_history` called `path.resolve()` for every line, 1.9 million calls and 13 million `lstat` calls, about 95 of 378 profiled seconds over this checkout's real Claude history (1,973 files, 8.09 GB). The path is now resolved once per file. Old and new reader give an equal digest (`47f43dcf5d873064`) over 198 real files (861 MB, 24,722 operations, 6,452 gaps); 23.4 s before, 14.9 s after, one run each. A test counts `Path.resolve` calls and fails with the per-line call put back. Gate: clean worktree at `4b57ff115`, every lane exit 0.

- Fix SHA (on `experiments`): `c72131fc9`
- patch-id: `028e04ad26781ff808f1d66756c692b92fe626ff`

Per-file cache: `scripts/history_cache.py` stores one pickle per history file and reuses it while the code digest, path, size and mtime_ns match. The file is `stat`ed before it is read. A result with an "unreadable source" gap is never stored. The directory (`$FILE_PROVENANCE_CACHE_DIR`, else `XDG_CACHE_HOME` or `~/.cache`, under `codescout/history-cache`) is used only when it is a real directory owned by the user with no group or other access; entries are opened without following links. Any error falls back to reading the file. Each file is stored as soon as it is read. Embedded as the seventh engine script. 11 tests in `tests/test_history_cache.py`; four mutants each fail one (ignored header, caching unreadable results, no privacy check, following symlinks). Gate: clean worktree at `c72131fc9`, every lane exit 0, Python 1423 passed.

Equivalence on the real corpus, frozen to files untouched for 15 minutes: in one process, warm, uncached, warm, uncached gave one digest (`6ff5fe9bda2f95cc`, 90,917 gaps); a per-file check found all 2,355 files equal between the reader and the cache. An earlier three-run comparison differed by 374 gaps between the cold and warm runs. The per-file check and the later four-run order test show the cache is not the cause; the cause of that drift between separate runs was not pinned (the policy loop reads the live filesystem, and worktrees came and went; not verified).

Speed through the real entry point (`attribute-red.py`, an untracked path so no floor, `timeout 30` as the Rust caller stops it, cold cache): calls 1 to 4 were killed at 30 s with 821, 1,347, 1,829 and 2,074 cache entries; call 5 answered, in 29.2 s. With the cache full: 16.4, 22.9 and 17.7 s, identical 534-byte output, under a machine load of 18 to 32.

- Fix SHA (on `experiments`): `e8b9935c4`
- patch-id: `eec808468c70c0da134994c569269b18e45cd2a6`

Policy loop (2026-11-01, design `docs/superpowers/specs/2026-11-01-policy-loop-speedup-design.md`): three changes in `scripts/file-provenance.py`, output kept identical for every caller (the red hint, the `file-provenance.py` CLI that `fmt-mine.sh` and the pre-edit hook run, tests). `normalize` tests the resolved path against the root as strings, with a separator in the prefix, instead of `Path.relative_to` (5.8 of its 10.8 profiled seconds). `scan_provenance` reuses `normalize` results within ONE call (27,707 of 76,207 calls had distinct arguments); the memo is local, so a second scan sees a retargeted symlink. `write_targets` returns at once for a tool name it has no branch for (`_TARGET_NAMES`, not named `*_TOOLS` because the Rust drift gate reads such a constant as a list of registered tools). Proof on the real corpus, files untouched for 15 minutes, old and new engine in ONE process, old, new, old, new: one digest on all four (`67c7462ab5cbc348`, 90,808 gaps); `scan_provenance` took 17.9 and 16.1 s old, 14.0 and 12.6 s new, a mean saving of 22% (two runs each, shared machine; the design estimated 25 to 30%). 12 tests in `tests/test_policy_loop_speedup.py`; mutants (separator-less prefix, a memo that outlives the call, `run_command` missing from the set) each fail a test. A fresh Opus review of the commit found no Critical or Important issue; its test-blind-spot finding, re-graded Important, is fixed in `09c63ee15` (an AST walk replaces a regex, with a test that injects a forgotten branch). Gate: clean worktree at `0c2212674`, every lane exit 0, Python 1449 passed, the drift gate ok in both lanes.

- Fix SHA (on `experiments`): `88f77aeae`, test fix `09c63ee15`
- patch-id: `c6c17e8042d7c4e1f33d9e07119ca8e78eefab00`, test fix `6d5dc2713d7b972f9180b23a689b6474002694f1`

Attribution outcome (2026-11-01, design `docs/superpowers/specs/2026-11-01-attribution-outcome-in-usage-db-design.md`): a timed-out scan is silent in the response, so `usage.db` could not tell it from a red that named no dirty file. Read from `usage.db` (`run_command` rows, parsed from the `output_json` text): 16 hinted runs on 2026-10-20 and none on any day since, against 138 red runs with a diagnostic text from 10-21 to 11-01 (163 from 10-17 to 10-20, of which 44 hinted). The count of red runs uses `-->` and `panicked at` as a stand-in for the engine's own prefilter. No hint also held from 10-07 to 10-16. The data cannot say why. Two columns now record what the engine did: `attribution_outcome` (`hinted`, `silent`, `timed_out`, `tooling_failure`, `spawn_failed`; NULL when the engine did not start) and `attribution_ms`. A timeout also writes one log line. The response is unchanged, and a test pins that. Fourteen new tests; four mutants (drop the `timed_out` record, swap `hinted` and `silent`, answer a timeout with text, make the response depend on the sink) each fail a test. Gate in a clean worktree: Python 1454 passed, the clippy, lean and default lanes exit 0. The format lane refused in the fresh worktree (no write record there, so ownership was UNKNOWN). My three files were then formatted with `rustfmt` after I checked that every changed line was mine, and rechecked: `fmt-mine --check` exit 0, `usage::` 108 passed, `run_command::attribution` 40 passed. The whole gate was not rerun on the formatted bytes.

- Fix SHA (on `experiments`): `498b84bc4`
- patch-id: `2f20e37799b706c4475147b5ddd883c92079a369`

A fresh Opus review of that commit found no Critical issue and two Important ones, both fixed in `8cbbdb2d6`: (1) NULL does not mean "a red with no diagnostic text": stage 0 skips only exit 0 with nothing failure-shaped, so any non-zero exit starts the engine and a failed `ls` or a `grep` with no match records `silent` (live row 247866); the docs, the design and the plan were corrected and a test pins it; (2) the engine's own exit-0 notice "red attribution is unavailable" was filed as `hinted` and is now `tooling_failure`, through a prefix shared by `engine_outcome` and `finished_outcome` and a drift test against `scripts/attribute-red.py`. Three new tests, three mutants each killed by the test written for it. The gate was run again in a clean worktree: Python 1454 passed, all four lanes exit 0. The review confirmed that `record` runs inside the sink's scope in production, and the live `usage.db` holds two such rows (`silent` 145 ms, `hinted` 6,839 ms). Deferred Minors: the clock includes the stdin write, the wait-error branch has no test, and the `spawn_failed` arms write no log line.

- Review fix SHA (on `experiments`): `8cbbdb2d6`
- patch-id: `2e14969bcd8139cf7a990e2222682febb9e00a58`

Not fixed:

- The warm engine: `scan_provenance` alone is now about 13 s on the real corpus (was 17 s); measured on 2026-11-01 with phase timers: 12.8 to 13.1 s warm, of which shell-command `write_targets` is 5.81 s (`run_command` 4.04, `Bash` 1.77), `normalize` 1.45, `merge_history_batches` 0.61, and about 4 s is unpickling, sorting and loop overhead that no timer claims. The whole `attribute-red.py` call, warm, at a load average of 40 on 64 cores: 2.8 to 5.4 s for a file with a commit floor, 13.5 to 14.7 s for an untracked file (no floor). Under heavier load it can still pass the 30 s ceiling, and the hint is then silent. What remains in the loop: the shell-command regexes in `write_targets` (about 105,000 calls, 42% of the 249,056), and the `capable` and `write_base` work per operation. The deferred approach 2 (store each operation's targets in the cache entry) would remove the regexes; it needs the cache digest to cover `file-provenance.py` and keeps doc-tool targets dynamic.
- Through the Rust caller there is one sample: a red naming the untracked `bt.txt` on binary `a3c21e540` (it contains `e8b9935c4` and `88f77aeae`) returned the hint in 18.5 s (`usage.db` row 247141), about 4 s more than the standalone call. A distribution needs the new columns to fill.

## Tests added

No new test. The fix changes four existing tests so they scan an empty history. The mutation check above shows that they detect a missing `.envs(...)` line: 0 of 16 loaded runs pass without it.

The three tests in `tests.rs`: the mutation check detects a missing env for two of them (0 of 4 loaded runs pass each). It cannot for the many-dirty-files test, which skips instead of failing. Its evidence is the skip count under Fix: 0 of 4 on the fixed build, a skip against the mutant.

## Workarounds

The attribution tests have a fix in their own code. Before it, at a normal gate load these were not seen to fail, except the sightings below.

- Two sightings are recorded in `78f30eb614d3fba3`.
- A peer session (`codescout-5f`) reported that four attribution-family tests in `run_command` failed once in a gate at normal load (lib lane 696 s) and passed alone and in the next two gates: `dir_trust::a_directory_swapped_in_after_first_use_is_not_run` and the three tests in `src/tools/run_command/tests.rs` listed under Fix. I did not re-check that report. It contradicts the earlier statement that these were not seen at normal load.

## Resume

State on 2026-11-02: migrate group fixed, seven attribution tests fixed, the rest open. CI on `experiments` run `41413376172` (`c3339b9f5`) finished with `success`, every job; the two runs before it, on `32b3df112` and `a298f673d`, failed. Four attribution tests had failed on all three Windows jobs in the `a298f673d` run (three of them already before the 2026-11-01 commits, plus the new `an_engine_that_answers_records_hinted`); they pass in the green run as part of its all-jobs success. I did not read the fix for them, which another session made.

Open:

- `lsp::client::tests::workspace_symbols_returns_project_symbols`: tolerant of a timeout since `56457a853`. The retry rules now run in four unit tests that need no language server, and the test prints the timeout count (`6d2e402be`). Measured on 2026-11-01 with 64 busy loops and the test both pinned to cores 0 to 3: the first run hit the ceiling. Eleven requests answered empty in 0.3 ms to 274 ms, the twelfth took 30.0 s and timed out, and the thirteenth answered with the symbol after 4.1 s. So one request waits while rust-analyzer is not ready, and the server answers soon after the client cancels it. One run, not a distribution. Not established: what rust-analyzer was doing during the 30 s (no server log or progress notification was captured), so "it was still loading the workspace" is a reading of the timings, not a finding. The tolerance of two timeouts matches what was seen, and the cancel-and-retry answered within 4.1 s.
- The product limit, remainder: the warm engine is about 13 s in `scan_provenance` plus unpickling and git, against the 30 s ceiling, so the margin is still modest under load; the cold fill takes about five 30 s calls. Options, none chosen: approach 2 of the design (targets stored in the cache entry); a lean path for the red hint only, which must not touch the CLI (F-206 in `bug-fix-session-log`: the CLI uses records older than the floor); raising `ATTRIBUTION_TIMEOUT_DEFAULT`. Done since: one end-to-end run through the Rust caller (18.5 s, see Fix). Not done: a distribution of attribution times and outcomes (read it from `usage.db` once the new columns have data: `select substr(called_at,1,10) d, attribution_outcome, count(*), max(attribution_ms) from tool_calls where attribution_outcome is not null group by 1,2`). First read, 2026-11-02, a few hours after the binary shipped: 2026-11-01 `hinted` 6 rows (longest 7,669 ms, one of them my own live check) and `silent` 92 rows (longest 5,389 ms); no `timed_out`, `tooling_failure` or `spawn_failed` row. Too short and unloaded to choose between the options; read it again after a few days. Done on 2026-11-01: the stale "~7 s" comments in `attribution.rs` (`f03af0282`) and the companion hook's "~7s" text for the CLI, which now says about 20 s after a timed run of 19.4 s (plugin repo `4f93e80`).
- The commit hook named a stale writer for `output.rs` (see Fix). Filed as `581b05bbe41134f3`.
- Why no red run has carried a hint since 2026-10-20: unknown. Candidates: a timeout (the engine without the cache decoded everything in about 240 s, and the cache shipped on 2026-11-01; I did not check which engine each earlier binary embedded), or reds that name no dirty file. The new columns will separate them.

## References
- `docs/issues/2026-10-31-a-timed-out-filter-test-fails-in-the-loaded-gate-and-passes-alone.md`
