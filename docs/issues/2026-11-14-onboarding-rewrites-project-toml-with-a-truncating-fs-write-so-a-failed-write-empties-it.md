---
id: '701f140f1a4545da'
kind: bug
status: open
title: 'BUG: onboarding rewrites .codescout/project.toml with a truncating std::fs::write, so a failed write empties a file that git may not hold'
owners:
- marius
tags:
- cluster/unclassified
- onboarding
opened: 2026-11-14
severity: medium
unverified: 'Read from the code, not reproduced. Not established: that any write has failed this way, whether git holds the project.toml of a given project (the comment above record_refresh_request says the file can be gitignored), and how a Windows target behaves. The create-if-absent sites (L1600, L1813) are low severity: a file that exists at check time is not touched, and the race needs a second writer in a short window.'
---

# BUG: onboarding rewrites .codescout/project.toml with a truncating std::fs::write, so a failed write empties a file that git may not hold

## Summary
`patch_project_table` rewrites an existing, user-authored `.codescout/project.toml` with `std::fs::write` (`src/tools/onboarding.rs:140`). That call truncates the file and then writes. A failure after the truncate (disk full, process kill) leaves a short or empty file. The same file holds the project's `[security]` table and its embedding settings. Two more writes in the same file create a file after an `exists` check, and they have the check-then-write race.

## Class fit
cluster/unclassified: the writers sit outside the atomic write door that `write_catalogued_file` uses (`atomic_write` in `src/util/fs.rs`). No existing class names a file writer that truncates in place or that checks and then writes. This bug differs from a806db1ec4ea162b: that writer used the door and the door was weak. These writers never used the door. It also differs from the `create_file` sibling bug, which already goes through `atomic_write`.

## Symptom (Effect)
A write fails after the truncate. `project.toml` is empty or cut short. The next project load reads a file with no `[project]` table or a parse error. The user's settings are gone unless git or a backup holds them.

## Reproduction
Not reproduced. Read path: `patch_project_table` parses the raw table, applies the change, and calls `std::fs::write(config_path, toml::to_string_pretty(&doc)?)` (L140). Fill the target file system during that call.

The production callers of `patch_project_table` are four:
- `record_refresh_request` (L161), when the file exists.
- `stamp_witnessed_refresh` (L205), when a refresh was witnessed.
- `remember_decline` (L1045), when the file is a regular file.
- `forget_decline` (L1058), when the file carries the decline marker.

The create-if-absent writes are two:
- `write_workspace_config_if_needed` writes `workspace.toml` with `std::fs::write` (L1600) after `!workspace_config_path.exists()` (L1569).
- `perform_full_onboarding` writes `project.toml` with `std::fs::write` (L1813) after `!config_path.exists()` (L1773).

## Environment
Branch feat/atomic-write-door-20261114 at aa29cd410. Found during the final review of the atomic write door.

## Root cause
`std::fs::write` opens the path with truncate. `patch_project_table` has no staging file and no rename. The read at its start (a bounded regular-file read) and the write at its end are two separate steps, so a path swapped in between is also written. (inferred from `src/tools/onboarding.rs:119-140` - not measured)

The two create-if-absent writes open the path with truncate and without `create_new`. A file that appears after the `exists` check is overwritten. (inferred from `src/tools/onboarding.rs:1569-1600` and `:1773-1813` - not measured)

## Fix
Use the pair, not `atomic_write` alone. Call `crate::util::fs::refuse_unwritable_existing(path)` first, then `crate::util::fs::atomic_write(path, text)`. The probe is needed because `atomic_write` replaces a read-only file (a rename needs no permission on the file). Without the probe, a read-only `project.toml` would now be replaced where it is refused today.

Check one side effect before the change. `atomic_write` runs the protected-files guard (`src/config/protected_files.rs`), and the module doc of that file names `onboarding` as outside the guard today. `patch_project_table` carries `[security]` over as read, so the guard should pass. A test must show it passes for each caller.

For the two create-if-absent writes, `atomic_write` alone still replaces a file that appears in the window. Publish them with a no-clobber step (stage the bytes, then `hard_link` and `remove_file`, or `OpenOptions::create_new` as the minimum), and treat `AlreadyExists` as "a file is there, leave it".

A pinned test breaks under the pair. `patch_project_table_still_patches_a_regular_file_and_reports_a_missing_one_as_before` (`src/tools/onboarding.rs:3598`) patches `link.toml`, a symbolic link to `real.toml`, and asserts that `real.toml` carries the new key. The probe opens with `O_NOFOLLOW`, so it refuses the link with `ELOOP` (`a_symlink_at_the_path_is_not_followed_by_the_check`, `src/util/fs.rs`). Without the probe, the `rename` in `atomic_write` would replace the link itself and leave `real.toml` unchanged. Either way the test fails. The fix must choose: resolve the link to its target before the pair and keep the test, or refuse a link and change the test. This bug does not choose.

Tests: a failing write into the staging file leaves the old `project.toml` bytes; a read-only `project.toml` stays unchanged and the call fails; each of the four callers still writes its key.

## Fix provenance

For `patch_project_table` only.

- **SHA:** `9fb8984bf` (`fix/patch-project-table-atomic-20261115`; resolve a link, `refuse_unwritable_existing`, then `atomic_write`; the five tests)
- **patch-id:** `adcccfe574448a19bc342af397960995ee58406b`
- **SHA:** `5f6cef6cb` (review fixes: only a link is resolved, the mode test uses 0640, the read-only-directory test asserts EACCES, docs narrowed)
- **patch-id:** `89d3651fe7c01d0ef6fd6cad1cac55718df49d31`
- **SHA:** `fb482d04f` (the doc names the staging leftover `project.toml.<hex>.tmp`)
- **patch-id:** `f37a2f0e1ab35c3b4bb549ad62675943e9f0f6d0`

## Tests added

In `src/tools/onboarding.rs` (module `atomic_patch_tests`, Unix only):
- `a_patch_in_a_read_only_directory_is_refused_and_leaves_the_old_bytes` (failed on the old write)
- `a_read_only_project_toml_is_refused_and_unchanged` (fails if the probe is removed)
- `a_link_to_the_config_is_patched_through_and_stays_a_link` (fails if the link is not resolved)
- `a_patch_keeps_the_file_mode`
- `a_patch_of_the_protected_project_toml_passes_the_guard_and_keeps_security`

The two named mutants were run by hand: each was caught by the test named for it. They are not a standing probe.

## Resume

Partly done. `patch_project_table`, the medium part, replaces the file atomically since `fb482d04f` on `experiments` (see `## Fix provenance`).

Not started: the two create-if-absent writes, now at `src/tools/onboarding.rs:1629` (`workspace.toml`) and `:1842` (`project.toml`). Both still use `std::fs::write` after an `exists` check. They need a no-clobber step, and `atomic_write` alone does not give one. The `doc(create)`, `doc(move)` and `create_file` sibling bugs need the same helper, so build it once for all of them. This bug stays open until those two writes are fixed.

Behaviour the fix changed, for whoever reads a failure later:
- The replace needs write permission on the directory of `project.toml`. `record_refresh_request` now fails with a read-only `.codescout/` and a writable `project.toml`, where it succeeded before.
- `atomic_write` calls `observe_write` on Linux, so the onboarding stamps now emit `dispatch-observed` facts where `std::fs::write` emitted none.
- A link to the config is resolved first: the target is patched and the link stays a link. A link swapped in after the resolve is not closed.

Parked review note: under root, the two permission tests pass without asserting anything, the same idiom as the tests of `refuse_unwritable_existing`.

## References
- `src/tools/onboarding.rs:140`, `:161`, `:205`, `:1045`, `:1058`, `:1600`, `:1813`
- `src/util/fs.rs` (`atomic_write`, `refuse_unwritable_existing`)
- a806db1ec4ea162b (the door itself)
- 2026-10-20 reads-of-repository-files bug (names `patch_project_table` as a writer outside the protected-files guard; a different defect)
