---
id: db5303eb3b9c2781
kind: bug
status: open
title: 'BUG: the Windows lib tests fail in 299 places once they compile; the read door refuses the runner temp dir, and most of the rest is not yet classified'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- windows
- librarian
opened: 2026-11-01
severity: high
unverified: Only the count, the failing test names and one message family (75 tests whose output carries "refusing to read ... symbolic link") are read from the CI log. The mechanism for that family is a hypothesis, no Windows reproduction has been run, and 219 of the 299 failures have no attributed cause.
---

## Summary

Since `fdf239b77f544684` was fixed, the three Windows CI test jobs compile and run. They fail at the test level: 299 tests on `windows-latest / default`, 42 on `local-embed` and 42 on `no-features` (the 2026-10-30 run, before the compile error, had 297, 40 and 40). The lean 42 are a subset of the default 299. This bug tracks the whole state; it should be split into per-cause bugs as causes are established.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the local gate runs on Linux only, and the tests assume Linux-shaped paths and temp directories, so a Windows-only failure cannot appear in it. It differs from the class's other members in size (hundreds of tests, several causes) and in that no Windows reproduction environment is wired into the gate.

## Symptom (Effect)

CI run 41331598643 (tip `7b7da56cb`), jobs `Test (windows-latest / default | local-embed | no-features)`: `Summary 7945 tests run: 7646 passed, 299 failed` for `default`; 42 failed for each lean job.

Failing modules on `default` (count of tests): `librarian::tools::doctor` 106, `librarian::tools::append_entry` 36, `librarian::indexer::tests` 30, `librarian::catalog::augmentation` 29, `librarian::catalog::rekey` 15, `librarian::tools::find` 12, `librarian::tools::update_entry` 11, `tools::read_file::buffer_edge_tests` 11, `util::path_security::tests` 5, `tools::run_command::attribution` 4, `tools::symbol::kotlin_lsp_tests` 4, and about 30 more in single digits.

The 42 lean failures: `tools::read_file::buffer_edge_tests` 11, `util::path_security::tests` 5 (all about `..` handling), `tools::run_command::attribution` 4, `tools::symbol::kotlin_lsp_tests` 4, `tools::memory::tests` 3, `config::project::tests` 2, `tools::core::tests` 2, `tools::markdown::byte_edge_tests` 2, `server::*` 5, `tools::grep::tests` 1, `tools::run_command::observe` 1, and one worktree test.

## Reproduction

Not reproduced. `scripts/build-windows.sh` and the `windows-gnu` lane run the tests under wine for the lean build; that is the nearest local route, and it has not been tried for this bug.

## Environment

GitHub `windows-latest`. The runner's temp dir appears in messages as `C:\Users\RUNNER~1\AppData\Local\Temp\...` (an 8.3 short name) and, in another message, as `\\?\C:\Users\runneradmin\AppData\Local\...`.

## Root cause

Established: 75 failing tests print `refusing to read C:\Users\RUNNER~1\AppData\Local\Temp\...: it reaches its file through a symbolic link, and no managed root or repository contains it to judge the link against`. That message comes from the `None` branch of `ReadDoor::verdict` in `src/librarian/read_door.rs`, which compares the opened file's canonical path (`real`) with the spelled path as plain components (`real != lexical`).

Hypothesis, not verified: on Windows `std::fs::canonicalize` returns the verbatim form (`\\?\C:\...`) and expands 8.3 short names, so `real` never equals a spelled temp path; the macOS bug `ff6f32dcdeacee4d` is the same comparison failing for a different reason (`/var` is a link). If so, the door refuses every unmanaged path on Windows, not only the runner temp dir, and the fix belongs in the door (compare after normalising the verbatim prefix and short names, or detect links by `symlink_metadata` on each component), not in CI. This is security-relevant code and the choice needs a decision.

Not established: the cause of the other 224 failures. A first pass over the failure output found `invalid configuration for \\?\C:\Users\runneradmin\...` (2 tests), `unknown tool: 'doc'` (2, including the integration test `the_server_wires_a_frontmatter_write_through_to_the_catalog_row`), and for most of the rest only the assertion text (`left == right`), with no attributed cause. The 11 `buffer_edge_tests` and the 5 `path_security` tests are good first candidates because they are lean-lane failures that need no librarian.

## Evidence

CI run 41331598643 logs; the lists of failing test names are in the CI summary of each job.

## Hypotheses tried

None run.

## Fix

None yet.

## Tests added

None.

## Workarounds

None.

## Resume

1. Reproduce the 42 lean failures under wine with `scripts/build-windows.sh` (no `ort-sys` problem on the lean build), or on a Windows machine.
2. Read the full failure output of one test per module; split this file into one bug per cause.
3. Decide, for the read door, what `real` should be compared with on Windows, and test it against a path with a short name and a path through a junction.
4. Consider whether the Windows test lanes should stay required while this is open; both red states (compile, then 299 failures) went unnoticed behind a different red job.

## References

- CI runs 41331598643 (compiles, 299/42/42) and 41026635953 (2026-10-30, 297/40/40).
- `fdf239b77f544684` (compile error, fixed), `ff6f32dcdeacee4d` (the macOS form of the same door comparison), `src/librarian/read_door.rs`.
