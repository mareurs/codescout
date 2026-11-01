---
id: ff6f32dcdeacee4d
kind: bug
status: fixed
title: 'BUG: on macOS every tempfile fixture is spelled through the /var symlink, so the librarian read door refuses about 250 tests'' files'
owners:
- marius
tags:
- cluster/repro-env-diverges-from-gate-env
- tests
- ci
- macos
- librarian
closed: 2026-11-01
opened: 2026-11-01
severity: high
unverified: The cause is reproduced and the fix verified only on Linux, with TMPDIR at a symlink; the effect on macOS itself is not observed until a CI run contains the fix, and the 20 other lib failures listed under Resume are not addressed.
---

## Summary

On macOS `std::env::temp_dir()` is `/var/folders/<xx>/<hash>/T`, and `/var` is a symbolic link to `/private/var`. Every `tempfile::tempdir()` fixture is therefore spelled through a link. The librarian's read door has a deliberate invariant: when no managed root or repository can be named for a file, it reads only a spelling that crosses no symbolic link at all. So the door refuses the fixtures, and about 250 tests fail with `refusing to read ... it reaches its file through a symbolic link, and no managed root or repository contains it`. The tests assume a canonical temp dir; the CI job did not provide one.

## Class fit

`cluster/repro-env-diverges-from-gate-env`: the Linux machine that gates locally has a canonical temp dir, and the macOS runner that gates in CI does not, so the same tests agree on one and disagree on the other. It differs from the class's other members in that the gap is a property of the platform's default temp path, shared by every fixture, so one cause produces hundreds of failures that read as separate door bugs.

## Symptom (Effect)

CI run 41322768679, `Test (macos-latest / default)`: 268 failing tests (263 in the lib binary, 5 in integration binaries). `local-embed` and `no-features`: 22 each. The same 262 tests (default) and 21 (lean) already failed on Oct 30; 6 and 1 more fail now, but those are tests added later or the same class (see Root cause). The failures are on `experiments` for as long as the read door has existed.

## Reproduction

On Linux, point `TMPDIR` at a symlink and run the lib tests:

1. `mkdir real; ln -s "$PWD/real" link`
2. `TMPDIR="$PWD/link" <lib test executable>`

Result: `7968 passed; 244 failed`. The dominant message is the one from CI. Of macOS CI's 263 lib failures, 243 are among these 244 (92%). With a canonical `TMPDIR` the same run has no such failures.

## Environment

codescout `experiments` at `60dc3569e`; macOS runner `macos-latest`, `TMPDIR=/var/folders/n6/<hash>/T`.

## Root cause

- `src/librarian/read_door.rs`: the module invariant ("When no root can be named, it reads only a spelling that crosses no symbolic link at all") and the refusal at the `real != lexical` comparison. This is by design, not a door bug.
- The read door's own tests call `canon(t.path())` on their fixtures. The other tests (catalog, `append_entry`, `doctor`, `rekey`, `update_entry`, the indexer, the `doc_tool_refs` sidecar tests) use the raw `tempdir()` path.
- `approve_outside_project_is_independent_of_linked_worktrees` (added 2026-10-31) compares `/private/var/...` with `/var/...`: the same respelling.

## Evidence

- Clusters of CI failures by message: 16 + 14 + 12 + 9 + 4 in `librarian::catalog`, `append_entry`, `rekey`, `update_entry` with the refusal message; 13 + 5 + 5 + 3 in `doctor` with empty results; the sidecar tests quote `/var/folders/n6/.../T/.tmpXXXX/seeded.yaml`.
- Overlap script result: lib failures on macOS also reproduced locally with a symlinked `TMPDIR`: 243 of 263.

## Hypotheses tried

- A bug in the read door: no. The door's module doc states the refusal as the invariant, and its own tests pass on a canonical path.
- Fix each fixture to canonicalize: not done. About 250 tests across a dozen modules would change, and there is no macOS machine here to check them.

## Fix

- CI: the `test` job sets `TMPDIR` to the physical path (`pwd -P`) of `runner.temp` on macOS.
- `the_temp_dir_is_spelled_without_a_symbolic_link` (`src/librarian/read_door.rs`) fails first when `TMPDIR` is spelled through a link, and names the remedy, instead of 250 tests failing with door messages. On a developer's own macOS machine, export `TMPDIR="$(cd "${TMPDIR:-/tmp}" && pwd -P)"` before running the tests.

Fixed by `abba2def5` on `experiments` (patch-id `d11b674696404efc0d1888e7888e40ddaf33500b`). The full local gate was green on the merged tip `abba2def5`. The effect on a macOS runner is not yet observed; 20 of the 263 macOS lib failures are not explained by the symlink (see Resume).

## Tests added

`the_temp_dir_is_spelled_without_a_symbolic_link`: RED with `TMPDIR` at a symlink, GREEN with a canonical one (both runs local, Linux). The effect on macOS itself is NOT observed yet; see `unverified`.

## Workarounds

On a macOS machine, set `TMPDIR` to the physical path as above.

## Resume

After the first CI run with the fix, read the macOS jobs. Expected cleared: about 248 of 268. Expected remaining, all different causes (20 lib failures that the symlink does not reproduce):

- 7 socket-path-length tests (`lsp::manager::tests::*` x6, `util::fs::bounded_read_tests::a_socket_is_refused_as_not_a_regular_file`): `path must be shorter than SUN_LEN` (104 bytes on macOS). The runner's temp dir is about 30 characters shorter than `/var/folders/...`, which may be enough; not known.
- 7 `doc`-tool tests: fixed by `f2ef1c72d0fb9618`.
- 3 non-UTF-8 path tests (`read_door_tests::*`, APFS refuses such names), 1 case-insensitive name test (`protected_config_write_tests::the_protected_name_is_matched_ignoring_ascii_case`), `door_layout_table_round6`, and `byte_edge_tests::a_markdown_range_over_a_wide_line_keeps_one_handle` (a byte budget that depends on the fixture path length; see `docs/issues/2026-09-08-the-emission-byte-ceiling-measures-the-fixtures-tempdir-path-length.md`).

## References

- CI runs 41322768679 and 41026635953.
- `docs/issues/2026-09-08-the-emission-byte-ceiling-measures-the-fixtures-tempdir-path-length.md`: an earlier member of the same macOS temp-path family.
