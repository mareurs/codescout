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
was not passed through any canonicalization. `project_root` WAS canonicalized, above that push (the
`let project_root = best_effort_canonicalize(project_root)` line sits before the deny-list check and
the `allowed` vector); `cwd`, `extra_write_roots` and `session_roots` were canonicalized below it.

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
  refused. The temp dir is injected so tests need no `TMPDIR` mutation. `write_to_tmp_allowed` lost its
  macOS `ignore` and compares to the canonical target.
- `9a87b975`, `14d7ff7f` the broad-root guard compares the canonical `$HOME` (a symlinked home would
  never equal a canonical root otherwise).
- `2c71a473` companion fix the first one needs: `canonicalize_write_target` resolved only the immediate
  parent, so with the temp root canonical a write below a not-yet-created subdirectory of a symlinked temp
  dir or project root was refused. Measured against `299395e5` with a symlinked TMPDIR:
  `classify_in_project_is_allowed` and `create_text_file_creates_parent_dirs` went green -> red under
  `4915b443` alone. It now resolves through the deepest existing ancestor. This also closed shape 2 of
  `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`,
  a real escape (`create_file` wrote outside the project root through a symlinked directory).
- Review follow-ups. `9bad4fac` injects home and cwd (`WriteEnv`) with no behaviour change; `abdb0b03`
  then fixes what the review found in `2c71a473`:
  - **H1 (high, introduced by `2c71a473`)**: deny entries were still resolved with `best_effort_canonicalize`
    (literal when missing) beside a target resolved through its existing ancestors, so behind a symlinked
    `$HOME` a write to a missing `~/.ssh` or `~/.aws` stopped matching the deny list and a root over the real
    home admitted it. Entries now use `canonicalize_through_existing_ancestor`, like the target.
  - **L2**: project, extra, session and cwd roots that do not exist yet under a symlinked ancestor stayed
    literal while the target was canonical (`approve_write` on a new directory, then a write below it, went
    Allowed -> OutsideRoot). Every root now resolves like the target.
  - **L1**: the temp root and the cwd root are also refused when they are an ANCESTOR of the canonical
    `$HOME`, not only equal to it. `validate_approve_path` is NOT changed (it still tests equality with
    `$HOME` alone): follow-up, see `docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`.
  - **L1 consequence, measured**: it refuses a root that was legitimately allowed before whenever `$HOME`
    lives under the temp dir. Run the `path_security` tests with `HOME=<s>/homelink` (symlink to
    `<s>/T/realhome`) and `TMPDIR=<s>/T`: `write_to_tmp_allowed` fails because `<s>/T` is above the home.
    A sandbox or CI job that sets `HOME=$TMPDIR/home` therefore loses scratch writes (they fail closed
    with an `approve_write` prompt). Nothing else that was allowed before is refused: a temp dir or cwd
    that is not `/`, not `$HOME` and not above it is unaffected, including the normal case of a temp dir
    below `$HOME`. If that sandbox shape matters, apply the ancestor rule to the cwd root only.
  - **L4**: the session-root deny-list test asserted `is_err()`, which `OutsideRoot` also satisfies; it
    now asserts the deny-list refusal.
  - `c9680518` pins the cwd root's resolution (the mutation probe found it unpinned).

## Tests added

All in `src/util/path_security.rs`, none mutating `TMPDIR`, `HOME` or the cwd (they are injected through
`WriteEnv`):

- Temp root: `symlinked_temp_dir_allows_a_write_inside_it`,
  `symlink_inside_temp_dir_pointing_outside_is_still_refused`, `nonexistent_temp_dir_fails_closed`,
  `dotdot_traversal_out_of_the_temp_dir_is_still_refused`,
  `temp_dir_symlinked_to_filesystem_root_adds_no_allowlist_root`,
  `temp_dir_symlinked_to_home_adds_no_allowlist_root` (its home is injected; it used to return silently
  when `$HOME` was unset), `canonical_temp_write_root_is_the_canonical_path_of_an_existing_dir`.
- Missing subdirectory: `symlink_out_of_the_temp_dir_is_refused_even_below_a_missing_subdirectory`,
  `symlinked_temp_dir_allows_a_write_below_a_missing_subdirectory`,
  `project_root_behind_a_symlink_allows_a_write_below_a_missing_subdirectory`.
- Review follow-ups (red at `9bad4fac`, green at `abdb0b03`; `deny_list_holds_for_a_missing_entry_behind_a_symlinked_home`,
  `session_root_not_yet_created_under_a_symlinked_ancestor_admits_its_files`,
  `extra_write_root_not_yet_created_under_a_symlinked_ancestor_admits_its_files`,
  `project_root_not_yet_created_under_a_symlinked_ancestor_admits_its_files`,
  `temp_dir_that_is_an_ancestor_of_home_adds_no_root`, `cwd_that_is_an_ancestor_of_home_adds_no_root`);
  pinned by mutation: `home_behind_a_symlink_is_refused_as_a_temp_root`, `a_cwd_of_the_filesystem_root_adds_no_root`,
  `a_cwd_that_is_home_behind_a_symlink_adds_no_root`, `cwd_not_yet_created_under_a_symlinked_ancestor_admits_its_files`.
- `validate_write_path_session_root_still_respects_deny_list` now asserts the deny-list refusal.
  `write_to_tmp_allowed` asserts the canonical target and runs on macOS (not run there).

## Verified

- `./scripts/gate.sh` with `TMPDIR=/home/marius/.cache/cc-tmp`, on `c9680518`: exit 0, FMT=0 CLIPPY=0
  LEAN=0 (4629 passed) DEFAULT=0 (7014 passed), Python light 1266 passed.
- The same gate with `TMPDIR` a symlink to a real directory: FMT=0 CLIPPY=0; LEAN, DEFAULT and Python light
  are red, and every red test also fails on `299395e5` under the same symlinked `TMPDIR` (checked there):
  7 Rust tests, `tools::core::tests::worktree_read_notice_activate_remedy_names_the_main_repo_not_a_worktree`,
  `tools::grep::tests::a_glob_that_can_reach_a_hidden_root_entry_still_names_the_remedy`,
  `tools::read_file::buffer_edge_tests::a_handle_refusal_quotes_a_key_of_exactly_the_clip_whole`,
  `tools::read_file::buffer_edge_tests::a_value_read_of_a_real_files_handle_names_the_files_route`,
  `tools::read_file::buffer_edge_tests::a_library_file_read_counts_its_source_tag`,
  `tools::symbol::tests::symbols_propagates_error_when_fallback_also_fails` (LEAN and DEFAULT) and
  `librarian::adapter::tests::an_overflowing_librarian_context_hints_the_markdown_and_it_returns_it`
  (DEFAULT only), plus 2 Python tests (`test_a_vote_goes_on_stdin_inside_the_jail_with_three_paths_bound`,
  `test_racing_writer_cannot_be_overwritten_after_preflight`). They are other fixtures that compare a
  literal and a canonical temp path. None is in `path_security`; the two that the temp-root fix alone had
  turned red (`classify_in_project_is_allowed`, `create_text_file_creates_parent_dirs`) pass.
- Mutation probe (`scripts/mutation-probe.sh`, isolated, on the committed bytes of `abdb0b03` plus
  `c9680518`): on the review-fix surface 13 mutations, 12 killed at first; the one survivor (cwd root
  resolved with `best_effort_canonicalize`) is now killed after `c9680518`, so 13 of 13. Killed: deny
  resolver literal; project, extra, session and cwd roots literal; ancestor rule reduced to equality;
  canonical home compare replaced by literal; home clause off; `/` clause off; cwd broad-root guard off;
  cwd guard ignoring home; temp guard ignoring home; deny list ignoring home. The earlier two survivors
  (M4d canonical-home compare, M5 cwd guard) are now killed through the injected-home and injected-cwd seams.

## Workarounds

`approve_write` on the real temp directory.

## Resume

Nothing for the temp-dir root. Open follow-up: a leaf that is itself a symlink, filed in
`docs/issues/2026-10-17-write-allowlist-canonicalizes-only-the-parent-so-a-leaf-symlink-or-a-missing-subdir-escapes.md`.

## References

- `docs/issues/archive/2026-05-24-ci-macos-tempdir-canonicalization.md`
- Found while fixing `write_to_tmp_allowed` (8eeb4c90).
