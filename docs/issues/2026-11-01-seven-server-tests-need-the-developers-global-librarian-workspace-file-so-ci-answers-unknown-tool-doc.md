---
id: f2ef1c72d0fb9618
kind: bug
status: fixed
title: 'BUG: seven server tests build their librarian from the developer''s global workspace file, so a CI runner without it answers `unknown tool: ''doc''`'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- librarian
- server
closed: 2026-11-01
opened: 2026-11-01
severity: medium
unverified: The 7 tests are verified RED (122 passed, 7 failed) and GREEN (129 passed, 0 failed) with an empty home on this machine; the ubuntu CI result after the fix is not yet observed.
---

## Summary

Seven tests build a `CodeScoutServer` from a `ServerEnv` whose `LibrarianEnv` names a catalog (`db`) but no `workspace`. `workspace: None` means "read `$HOME/.config/librarian/workspace.toml`". A machine that has that file builds the librarian runtime and registers the `doc` tool. A CI runner does not have it, so `build_tool_context_with` fails, `try_build_runtime_with` logs the error at `info` level and returns `None`, and the first call to `doc` answers `unknown tool: 'doc'`. The same 7 tests have failed on every ubuntu CI run in the logs I read (Oct 30 and Nov 1) and have never failed locally.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the machine that gates a commit locally (it has a global librarian workspace file) is not the machine that gates it in CI (a fresh runner has none), so the local gate stayed green while CI stayed red. It differs from the class's other members in where the gap hides: production code swallows the failure on purpose (the librarian is best-effort), so the only visible trace is a message about a tool that "does not exist".

## Symptom (Effect)

CI jobs `Test (ubuntu-latest / default)` and `Test (server-stack ...)` fail the same 7 tests:

- `server::guide_hint_tests::a_caller_who_read_the_whole_guide_is_not_held`
- `server::guide_hint_tests::a_different_call_is_held_until_one_reissue_has_run`
- `server::guide_hint_tests::a_forced_delete_is_held_and_an_unforced_one_is_not`
- `server::guide_hint_tests::a_forced_update_is_held_on_its_first_call`
- `server::guide_hint_tests::parallel_first_destructive_calls_are_all_held`
- `server::guide_hint_tests::the_first_destructive_call_is_held_and_serves_its_guide_before_the_side_effect`
- `server::config_reload_tests::the_failed_reload_notice_rides_a_held_destructive_response`

Message: `dispatch ok: ErrorData { code: ErrorCode(-32602), message: "unknown tool: 'doc'", data: None }`. The `local-embed` and `no-features` ubuntu lanes pass only because these tests are `cfg(feature = "librarian")` and are not compiled there. The same 6 guide-hint tests also fail on macOS CI.

## Reproduction

Build the lib test binary, then run it with an empty home (nothing else on the machine may satisfy the test):

1. `cargo test -p codescout --lib --no-run` (note the executable path it prints).
2. `HOME=$(mktemp -d) <executable> guide_hint_tests config_reload_tests`

Result before the fix: `122 passed; 7 failed`, the 7 above. Result after: `129 passed; 0 failed`. The run leaves no `.config/librarian/workspace.toml` behind.

## Environment

codescout `experiments` at `60dc3569e`. Local machine has `~/.config/librarian/workspace.toml`. CI: the runs listed under References.

## Root cause

- `src/librarian/mod.rs` `load_workspace_config`: `env.workspace == None` selects `workspace::default_config_path()`, which is `$HOME/.config/librarian/workspace.toml`. A missing file is an error (`Load workspace config from ... No such file or directory`).
- `src/librarian/adapter.rs` `try_build_runtime_with` turns that error into `None` (`tracing::info!("librarian disabled: ...")`), so the server starts without `doc` and `librarian`.
- 18 test sites wrote `LibrarianEnv { db: Some(dir.path().join("librarian.db")), ..Default::default() }` with no `workspace`. The sibling helper `make_server` in the same module injects an empty workspace file, for exactly this reason (archived bug `2026-07-02-windows-gnu-wine-20-test-failures.md`), but the other sites were written without it. Only 7 of the 18 call `doc`, so only 7 fail.
- Order dependence: some other test in the full run creates the global file as a side effect (`LibrarianEnv::from_env()` asks for `init_default_workspace`). A full lib run with an empty home passed 8212/8212 for that reason, so whether these 7 fail depends on which test runs first.

## Evidence

- CI run 41322768679 (Nov 1) and 41026635953 (Oct 30): the same 7 names fail in `Test (ubuntu-latest / default)`; the Oct 30 libtest summary reads `8028 passed; 7 failed`.
- A temporary `eprintln!` in `try_build_runtime_with` (reverted) printed: `Load workspace config from <fresh home>/.config/librarian/workspace.toml (the global librarian file, not .codescout/workspace.toml) ... No such file or directory`.
- After the full-suite run, `<home>/.config/librarian/workspace.toml` existed in the fake home that I had created empty.

## Hypotheses tried

- The lane lacks the `librarian` feature: no. The tests are compiled in `default`, and the lanes that fail are exactly the ones that compile them.
- The embedder or ONNX weights are missing: no. The error is the workspace file; the embedder is optional in `build_tool_context_with`.

## Fix

`test_support::librarian_env(dir)` in `src/server.rs` writes an empty workspace file and returns a `LibrarianEnv` with `workspace` and `db` set inside `dir`. All 18 literals now call it.

Fixed by `bef7eb8e1` on `experiments` (patch-id `900da4f7e82f3afa3a60a5efa8245d3a5d8428f6`). With a fresh empty HOME the server tests went from 122 passed / 7 failed to 129 passed / 0 failed. The full local gate was green on the merged tip `abba2def5`. The ubuntu CI result is not yet observed.

## Tests added

None new. The 7 failing tests are the regression tests: RED `122 passed; 7 failed`, GREEN `129 passed; 0 failed`, both with an empty home. No guard against a future literal without `workspace` was added; a new test would have to scan source text.

## Workarounds

On a machine without the global file, `LIBRARIAN_WORKSPACE=/path/to/empty.toml` does not help these tests (they build the env explicitly). None needed after the fix.

## Resume

Confirm `Test (ubuntu-latest / default)` and `Test (server-stack ...)` are green on the first CI run that contains the fix.

## References

- CI runs 41322768679 and 41026635953 (github.com/mareurs/codescout/actions/runs/...).
- `docs/issues/archive/2026-07-02-windows-gnu-wine-20-test-failures.md`: the same cause in `make_server`, fixed there only.
