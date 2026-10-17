---
status: fixed
opened: 2026-10-16
closed: 2026-10-17
severity: low
owner: marius
related: [docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md]
tags:
- cluster/unclassified
kind: bug
---

# BUG: the write allowlist holds the temp dir un-canonicalized, so a write into a symlinked TMPDIR is denied as outside the root

## Summary

`classify_write_path` (`src/util/path_security.rs`) canonicalizes the write target
(`canonicalize_write_target`) and the project root (`best_effort_canonicalize(project_root)`),
but pushes the temp dir as `crate::platform::temp_dir()` as-is. When `$TMPDIR` is, or sits
under, a symlink, the canonical target never starts with the literal temp dir, so the
"system temp directory is always writable" rule does not apply and the write is
`OutsideRoot`.

## Class fit

No existing class names a comparison of a canonical path against a root held in its literal
form. `IC-5` (the reproduction environment is not the gating environment) is nearest because it
only shows on a host with a symlinked temp dir, but the fault is in the production comparison,
not in a repro. Filed unclassified; revisit if a second allowlist root shows the same shape.

## Symptom (Effect)

Fixed; see `## Fix`. Before it, a write to `<symlinked TMPDIR>/x.txt` was refused with "outside the
project root. Call approve_write(...)". It failed closed, so there was no security exposure; the cost was
a spurious approval prompt.

## Reproduction

Scratch test (not committed), run alone: create `real = tempdir()`, symlink `link -> real`,
`set_var("TMPDIR", link)`, then `classify_write_path(link/x.txt, project, default_config(), [])`.

## Environment

Linux, codescout `experiments` at 3923da8c plus 8eeb4c90.

## Root cause

`src/util/path_security.rs`, in `classify_write_path`: `allowed.push(crate::platform::temp_dir())`
is not passed through `best_effort_canonicalize`, unlike `project_root`, `cwd`, `extra_write_roots`
and `session_roots` directly below it.

## Evidence

Measured 2026-10-16 with the scratch test above:
`temp_dir="/home/marius/.cache/cc-tmp/.tmpyi5xYh/tmplink"`,
`decision=OutsideRoot { resolved: "/home/marius/.cache/cc-tmp/.tmplT6k5I/x.txt" }`
(the resolved path is the symlink's target, so the literal root cannot match).

## Hypotheses tried

None needed; read at the bytes and confirmed by the run above.

## Fix

Fixed on branch `worktree-agent-a5bf8d6383eb7a8b1` (cut from `experiments` at `299395e5`; SHAs are branch
SHAs and change if the commits are rebased or cherry-picked, so re-cite after landing):

- `4915b443` the fix: `classify_write_path` now allows `canonical_temp_write_root(temp_dir)` instead of
  the literal temp dir. It uses `fs::canonicalize` directly, not `best_effort_canonicalize`, so a temp dir
  that cannot be canonicalized yields NO root (fail closed), and a canonical root of `/` or `$HOME` is
  refused. The temp dir is injected through `classify_write_path_with_temp` so tests need no `TMPDIR`
  mutation. `write_to_tmp_allowed` lost its macOS `ignore` and compares to the canonical target.
- `9a87b975`, `14d7ff7f` the broad-root guard compares the canonical `$HOME` (a symlinked home would
  never equal a canonical root otherwise).
- `2c71a473` companion fix the first one needs: `canonicalize_write_target` resolved only the immediate
  parent, so with the temp root canonical a write below a not-yet-created subdirectory of a symlinked temp
  dir or project root was refused. Measured against `299395e5` with a symlinked TMPDIR:
  `classify_in_project_is_allowed` and `create_text_file_creates_parent_dirs` went green -> red under
  `4915b443` alone. It now resolves through the deepest existing ancestor. This also closed shape 2 of
  `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`,
  a real escape (`create_file` wrote outside the project root through a symlinked directory).

## Tests added

All in `src/util/path_security.rs`, none mutating `TMPDIR`:
`symlinked_temp_dir_allows_a_write_inside_it`, `symlink_inside_temp_dir_pointing_outside_is_still_refused`,
`nonexistent_temp_dir_fails_closed`, `dotdot_traversal_out_of_the_temp_dir_is_still_refused`,
`temp_dir_symlinked_to_filesystem_root_adds_no_allowlist_root`, `temp_dir_symlinked_to_home_adds_no_allowlist_root`,
`canonical_temp_write_root_is_the_canonical_path_of_an_existing_dir`,
`symlink_out_of_the_temp_dir_is_refused_even_below_a_missing_subdirectory`,
`symlinked_temp_dir_allows_a_write_below_a_missing_subdirectory`,
`project_root_behind_a_symlink_allows_a_write_below_a_missing_subdirectory`. `write_to_tmp_allowed` now
asserts the canonical target and runs on macOS (not run there).

## Verified

- `./scripts/gate.sh` with `TMPDIR=/home/marius/.cache/cc-tmp`: exit 0, FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0,
  Python light 1266 passed.
- The same gate with `TMPDIR` a symlink to a real directory: FMT=0 CLIPPY=0; LEAN, DEFAULT and Python light
  are red, and every red test also fails on `299395e5` under the same symlinked `TMPDIR` (6 Rust tests in
  both lanes plus `librarian::adapter::tests::an_overflowing_librarian_context_hints_the_markdown_and_it_returns_it`
  in DEFAULT, and 2 Python tests), i.e. other fixtures that compare a literal and a canonical temp path. None
  is in `path_security`; the two that the temp-root fix alone had turned red now pass.
- Mutation probe (`scripts/mutation-probe.sh`, isolated, on the committed bytes): 14 mutations, 12 killed,
  2 survived. Survivors: replacing the canonical `$HOME` compare with the literal one (equivalent unless
  `$HOME` is a symlink, which it is not on the measuring host), and the cwd root's broad-root `if` (a
  use site that needs `set_current_dir`; it had no test before this change either).

## Workarounds

`approve_write` on the real temp directory.

## Resume

Nothing for the temp-dir root. Open follow-up: a leaf that is itself a symlink, filed in
`docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`.

## References

- `docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md`
- Found while fixing `write_to_tmp_allowed` (8eeb4c90).
