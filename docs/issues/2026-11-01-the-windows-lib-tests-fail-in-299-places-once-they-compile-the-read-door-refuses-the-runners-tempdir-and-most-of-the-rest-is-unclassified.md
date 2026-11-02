---
id: db5303eb3b9c2781
kind: bug
status: fixed
title: 'BUG: the Windows lib tests fail in 299 places once they compile; the read door refuses the runner temp dir, and most of the rest is not yet classified'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- windows
- librarian
closed: 2026-11-02
opened: 2026-11-01
severity: high
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

Not reproduced on a real Windows machine. Three local routes were used, each with a limit:

- `scripts/build-windows.sh test --lib` under wine 11.20 reproduced the `unknown tool: 'doc'` pair (a TOML parse failure, fixed by `ef326aec5`) and, with PortableGit and an embeddable Python on `WINEPATH`, the four `attribution` failures (fixed by `907db5eb8`).
- Wine does not reproduce the 8.3 short-name failures: `canonicalize` there does not expand short names, and a hand-made short `TMP` (`RUNN~1P1`) is not expanded by `GetLongPathNameW`, so a run with it fails 255 tests for a reason that is the wine setup, not the product.
- The wine lane in CI is a different population (python3 absent, the Cygwin `FAST_CWD` warning, three Win32 divergences); nine of its tests are skipped by name in `.github/workflows/ci.yml`, each with its cause.

## Environment

GitHub `windows-latest`. The runner's temp dir appears in messages as `C:\Users\RUNNER~1\AppData\Local\Temp\...` (an 8.3 short name) and, in another message, as `\\?\C:\Users\runneradmin\AppData\Local\...`.

## Root cause

Several causes, not one. Each is a way one location is spelled differently on Windows, or a Windows-specific input the tests did not account for:

1. **The read door (`src/librarian/read_door.rs`, the no-root arm).** It refused when `real != lexical`, and on Windows `canonicalize` returns `\\?\C:\...` with long names while a temp dir is `C:\Users\RUNNER~1\...`. Fixed by `windows_spelling::agrees`, which compares both in one spelling (verbatim prefix, separators, case, 8.3 names, a drive-less rooted path). This was the 75-test family.
2. **A missing file under a short-named directory** (read door, backfill of an absent source): `GetLongPathNameW` fails for a path that does not exist, so `long_form` left the short name alone and the door took the file for a link. Fixed by `c3339b9f5` (another session): `long_form` expands the existing ancestors and appends the missing tail.
3. **Spelling of paths in tests and in shown text** (routes and hints are JSON-quoted literals; the product shows the verbatim-stripped path; library paths compare by long name; the catalog stores forward-slash paths; the worktree notice, the memory `write_target`, the `read_file` route and the approved path come back canonical while fixtures are short). Fixed in the product where it showed a verbatim path (`7c468bc04`) and otherwise in the tests (`7c468bc04`, `86d3c05b3`, `f9e103225`, `cf810a299`, `b45b609c1`).
4. **`..` inputs** (`util::path_security`): Windows resolves `..` lexically before touching the disk, so `Path::join` and `format!` produced different inputs than Linux. Test inputs are spelled by hand (`8455fa93e`).
5. **`scripts/file-provenance.py::normalize`** returned `src\held.rs` on Windows while `git status` prints `src/held.rs`, so the attribution engine never matched a dirty file. Fixed in the product (`907db5eb8`). Found by reproducing the engine under wine with a real Python.
6. **A TOML basic string holding a backslash path** in two test fixtures: the file did not parse and the server started without the librarian (`ef326aec5`).
7. **Smaller:** an absolute glob compared with the walk root in two spellings (product, `f9e103225`); `HOME` is `USERPROFILE` on Windows; CRLF after checkout; two Windows-only error wordings for a file where a directory is needed; a pin test that needs to delete a directory the server holds open (Unix only).

The security-relevant decision the first version of this file asked for (what `real` is compared with on Windows) was taken as: compare after normalising both sides, and keep refusing a real link; `windows_spelling::agrees` has pure tests for the refusing cases.

## Evidence

CI run 41413376172 on tip `c3339b9f5` (2026-11-02): every job succeeded, including `Test (windows-latest / default | local-embed | no-features)`, `Test (macos-latest / ...)`, `Windows-gnu cross (MinGW + wine)` and `Audit Doc Refs`.

What that run is evidence for: the tests listed in the earlier runs now pass on those lanes. What it is not evidence for: the nine wine-lane tests that are skipped by name (their coverage is the `windows-latest` lanes, which passed); the final tree has not been through `./scripts/gate.sh` on Linux in one piece (the Ubuntu CI lanes passed).

Earlier runs: 41331598643 (299/42/42), 41342167539 (299/42/42, wine red at clippy), 41344511357 (71/42/42 Windows, 13/9/9 macOS, wine 72), 41402885985 (about 10 per Windows job, macOS and wine green, `Audit Doc Refs` red).

## Hypotheses tried

None run.

## Fix

Commits on `experiments` (patch-ids from `git show <sha> | git patch-id --stable`):

| SHA | patch-id | What |
|---|---|---|
| `f5c3c2168` | `ff04ac6e0a55` | read door: Windows respellings of one location |
| `7c468bc04` | `cb9ab2c22533` | tests parse routes/hints as quoted literals; product strips the verbatim marker; library paths by long name |
| `8455fa93e` | `4ad9894adfb5` | `path_security` test inputs spelled by hand |
| `86d3c05b3` | `2c297ba68a23` | read door resolves a drive-less rooted spelling; librarian tests use forward-slash catalog paths |
| `f9e103225` | `7ca55e9b7081` | grep glob in one spelling; config, server, pin, doctor, observe, Kotlin tests |
| `ef326aec5` | `4b18e6dad523` | `workspace.toml` fixtures use a TOML literal string |
| `15bbdf140` | `17679c56e2a8` | wine lane: eight wine-only tests skipped, causes recorded |
| `f5a2df69e` | `aa48e18b4783` | wine lane: a ninth skip |
| `cf810a299` | `42612eb7b9ec` | approved path compared in one spelling |
| `b45b609c1` | `0db8de019f5f` | worktree notice, memory `write_target`, `read_file` route in one spelling |
| `907db5eb8` | `271f48ee0bad` | `file-provenance.py::normalize` returns the git form |
| `c3339b9f5` | `c878fdcc95b0` | `long_form` expands the existing ancestors of a missing path (another session) |

The `Audit Doc Refs` job went red in the same run for an unrelated cause (the new root `tools/` directory made `tools/list` and two citations resolve as missing files); it is fixed by `b1f9a7752` (patch-id `8b5ce03ba899`) and is not part of this bug.

## Tests added

- Pure: `src/librarian/windows_spelling.rs` (7 tests on the comparison), `src/util/long_path.rs` (one Linux test), and the fake-expander test for the missing-tail case in `c3339b9f5`.
- Windows only: `src/librarian/windows_spelling_tests.rs` (a real short name expands; a drive-less rooted spelling agrees; a missing file under a short-spelled directory agrees).
- No test was added for `file-provenance.py::normalize`: on Linux the old and new behaviour are identical, so a Linux test cannot tell them apart. The discriminators are the four `attribution` tests on Windows.

## Workarounds

None.

## Resume

None open for this bug. Not archived: archive after `./scripts/gate.sh` has run on the final tree. Two follow-ups that are not this bug:

- The wine lane needs a faithful Windows short-name environment to reproduce the `RUNNER~1` family locally; wine's own short names are not expanded by `GetLongPathNameW`.
- Whether `tools/` at the repo root (the `setup-ab-eval` study) should keep that name, since it already collided once with doc references to `src/tools`.

## References

- CI runs 41331598643 (compiles, 299/42/42) and 41026635953 (2026-10-30, 297/40/40).
- `fdf239b77f544684` (compile error, fixed), `ff6f32dcdeacee4d` (the macOS form of the same door comparison), `src/librarian/read_door.rs`.
