---
id: db78f37db84ba413
kind: plan
status: draft
title: No-clobber publish for create_file, doc create, doc move and the two onboarding creates
tags:
- atomic-write-door
- no-clobber
---

# No-clobber publish — implementation plan

**Bugs closed (the no-clobber half):** beae053db1c0a3f6 (`create_file`), 07ebee9fd570c738 (`doc(create)`), 3ca37d8db3572e4e (`doc(move)`), and the two create-if-absent writes of af3e4091f67e2f67 (`onboarding.rs`: `workspace.toml`, `project.toml`).
**Builds on:** `atomic_write` and `refuse_unwritable_existing` (`src/util/fs.rs`), merged in a806db1ec4ea162b and af3e4091f67e2f67.

## Decision record

**Decision:** `src/util/fs.rs` gets two functions. `rename_no_clobber(from, to)` publishes an existing file at `to` and fails with `AlreadyExists` when anything is at `to`. `write_new_utf8(path, content)` stages the bytes the way `atomic_write` does, then publishes them with `rename_no_clobber`. Four call sites use `write_new_utf8` and one uses `rename_no_clobber`.
**Mechanism:** `std::fs::hard_link(from, to)` then `std::fs::remove_file(from)`. A hard link fails with `AlreadyExists` and never replaces. It needs no new `unsafe` and no per-OS code, and std documents it for Linux, macOS and Windows.
**Context:** a check-then-write on a separate call is not atomic. `std::fs::rename` replaces on every platform, so a rename cannot be the no-clobber step.
**Alternatives considered:**
- `renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on macOS, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows. Rejected: three `unsafe` bindings, and the Windows one cannot be built or run here. Revisit when the hard-link fallback fires in the field.
- `OpenOptions::create_new` and a direct write. Rejected: a failure mid-write leaves a partial file at the final path, which is the defect a806db1ec4ea162b removed.
- A `flock` on the directory. Rejected: advisory, and no other writer takes it.
**Fallback:** when `hard_link` fails with anything but `AlreadyExists` (a file system without hard links, such as FAT), `rename_no_clobber` falls back to "refuse if `symlink_metadata(to)` succeeds, else `rename`". That is today's behaviour, so no file system gets worse. The fallback is the old race and is documented as such.
**Consequences:** now easier: a concurrent creator can no longer be replaced silently. Now harder: both names exist for a moment between the link and the remove; a crash there leaves one extra name.
**Change scenarios absorbed:** a fourth no-clobber caller needs one call, not a new mechanism; a native no-clobber rename replaces only the body of `rename_no_clobber`.
**Revisit-when:** a user reports the fallback firing, or a Windows CI failure traces to `hard_link`.
**Confidence:** medium. The mechanism is proven on Linux by the tests below. macOS (std uses `linkat` with no flags, so a link is not followed) and Windows (NTFS only) are not run here.

## Findings that shape the tasks (read before dispatch)

1. **`doc(create)` cannot "take the catalog row back" on `AlreadyExists`.** The artifact id is `sha256(abs_path)` (`ids::artifact_id_from_abs`). The losing creator upserts the same row id, and its `undo_failed_create` would delete the row and augmentation that the winner relies on. The fix is therefore an order change: publish the file inside the open catalog transaction, before `tx.commit()`. `AlreadyExists` then drops the transaction (rollback) and the winner's row is untouched. A commit failure after a successful publish removes the file this call made. The bug text for 07ebee9fd570c738 must be corrected after merge.
2. **`undo_failed_create` removes `full` unconditionally** (`create.rs:~336`). After this change no failed step leaves a file this call made at `full` except the commit-failure case, so the unconditional `remove_file` is a way to delete a concurrent writer's file. It must go, and a test must show an existing file at the path survives a refused create.
3. **`platform::rename_overwrite` is unused and its docs lie**: the Windows body is `remove_file` then `rename`, not `MOVEFILE_REPLACE_EXISTING`. Not touched here. File it as a bug in Task 6.
4. **Out of scope, bug stays open:** the `overwrite: true` shrink-guard window of beae053db1c0a3f6. A rename has no compare-and-swap, so it cannot be closed, only narrowed.
5. **A peer branch** (`ecc-doc-create-cleanup`, commit `561c945d7`) edits `create.rs:547-565` and the test module of the same file. Expect a textual conflict at merge. The merger resolves it; the intent of that commit (best-effort wording of the cleanup error) must survive.

## Global Constraints

- No new `unsafe`. No new dependency. Non-test code uses no `cfg(unix)`-only API without a `cfg(not(unix))` arm.
- `write_new_utf8` runs `config::protected_files::check_write` first, as `atomic_write` does, and emits the same `observe_write` phases (read `atomic_write`, `src/util/fs.rs:~429`, before choosing what a refused create emits).
- The staging file is created with `create_staging_file` (exclusive, never follows a link) and is removed on every failure path. Leftover name: `<file name>.<32 hex>.tmp`.
- A refusal on an occupied path changes nothing: the old bytes, mode and link target stay, and no staging file remains.
- Tests that need symlinks or modes are `#[cfg(all(test, unix))]`. A test of a race window uses a seam, not a sleep.
- Gate: `./scripts/gate.sh --check`. The Windows lane is not covered; say so in the report.

## Task 1: the two helpers in `src/util/fs.rs`

**Files:** `src/util/fs.rs` (functions and tests).

- [ ] `pub fn rename_no_clobber(from: &Path, to: &Path) -> std::io::Result<()>`: `hard_link(from, to)`; on `AlreadyExists` return it unchanged and leave `from` in place; on success `remove_file(from)`; if that remove fails, remove `to` again (roll back) and return the remove error, naming both paths if the rollback also fails; on any other link error use the documented fallback.
- [ ] `pub fn write_new_utf8(path: &Path, content: &str) -> std::io::Result<()>`: `check_write`, `create_dir_all(parent)`, stage, `rename_no_clobber(staging, path)`; on `AlreadyExists` remove the staging file and return `AlreadyExists`. Prefer parametrising `atomic_write_with` over a second copy; take the smaller diff and keep the existing tests green.
- [ ] A `#[cfg(test)]` seam: a thread-local `before_publish` closure called with the target path just before the link, so a test can plant a file in the window. Document it next to the seam.
- [ ] Tests (write each first and watch it fail on the missing function): an occupied path stays byte-identical and returns `AlreadyExists`; no staging file remains after a refusal; a free path gets the content; a file planted by the seam is not replaced; a dangling symlink at the path counts as occupied and stays a link; a failed write leaves no partial file and no staging file; the `hard_link`-unsupported fallback refuses an occupied path and publishes a free one (test it by calling the fallback function directly); `from` is gone and `to` has the bytes after `rename_no_clobber`; a read-only directory gives `PermissionDenied` and leaves `from`.
- [ ] Mutation probe: remove the `AlreadyExists` arm and replace `hard_link` with `rename`; each must fail a named test.

## Task 2: `create_file` (beae053db1c0a3f6)

**Files:** `src/tools/create_file.rs`.

- [ ] For `overwrite == false`, replace `write_utf8` with `write_new_utf8`. On `AlreadyExists`, return the same refusal as the check at `:78`, with its replay handle. Extract that branch into one local function so the two paths cannot drift.
- [ ] `overwrite == true` keeps `write_utf8`.
- [ ] Tests: through the seam, a file planted between the check and the write stays unchanged and the call is refused with the replay handle; the handle's replay (`ack=`) then replaces the file; a normal create still works; `overwrite: true` still replaces.

## Task 3: `doc(move)` (3ca37d8db3572e4e)

**Files:** `src/librarian/tools/mv.rs`.

- [ ] Replace `std::fs::rename(&old_full, &new_full)?` (`:372`) with `rename_no_clobber`. On `AlreadyExists` return the same `destination '…' already exists` refusal as `:287` and leave the source in place and the catalog untouched.
- [ ] Tests: a file planted at the destination through the seam stays unchanged and the move is refused with the source intact and the row unchanged; a normal move still re-keys; moving a symlink still moves the link and not its target (Unix).

## Task 4: `doc(create)` (07ebee9fd570c738)

**Files:** `src/librarian/tools/create.rs`.

- [ ] Move the disk publish (`create_dir_all` and `write_new_utf8`) inside the catalog transaction, after the catalog writes and before `tx.commit()`. Read `ensure_registration` and `upsert_and_mint_slug` first and confirm nothing inside the transaction reads the file.
- [ ] `AlreadyExists` returns `LibrarianRecoverableError` `path exists: …` (the message of `:412`) and the dropped transaction leaves the catalog as it was. Any other write error rolls back and returns the write's own error with the cleanup wording kept from the peer branch.
- [ ] If `tx.commit()` fails after a successful publish, remove the file this call made, then return the commit error.
- [ ] `undo_failed_create` stops removing the file. Delete it or reduce it to the catalog part, whichever leaves no dead code; update its test.
- [ ] Tests: a file planted through the seam stays unchanged, the call is refused, and the catalog row and augmentation of the planted path (seeded by the test as the winner's) are unchanged; a failed write leaves no row, no file and no staging file; a commit failure (a trigger that aborts the insert) removes the published file; a normal create, with and without `augment`, still works.

## Task 5: the two onboarding creates

**Files:** `src/tools/onboarding.rs`, `src/config/protected_files.rs` (module doc only).

- [ ] `write_workspace_config_if_needed` and the `project.toml` create at `:~1842` use `write_new_utf8`. On `AlreadyExists` treat the file as created by someone else: the first returns `Ok(())`, the second leaves `created_config == false`.
- [ ] Confirm `check_write` accepts the `project.toml` create content (no `[security]` table, file absent) and the `observe_write` facts now emitted do no harm; say what you measured.
- [ ] Update the `protected_files.rs` module doc sentence that says onboarding's create uses `std::fs::write`.
- [ ] Tests: an existing `project.toml` and `workspace.toml` planted through the seam are not changed; a normal first onboarding still creates both.

## Task 6: records (controller, after the final review)

- [ ] Update the four bug files (Fix provenance with SHA and patch-id, Tests added, Resume). Correct 07ebee9fd570c738: the fix is the transaction order, not "take the row back". Leave beae053db1c0a3f6 and af3e4091f67e2f67 open for the shrink-guard window and for nothing else.
- [ ] File a bug for the unused, misdocumented `platform::rename_overwrite`.
- [ ] Reconcile the catalog with `merge_worktree` before removing the worktree.
