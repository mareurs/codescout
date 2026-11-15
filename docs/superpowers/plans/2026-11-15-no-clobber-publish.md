---
id: db78f37db84ba413
kind: plan
status: active
title: No-clobber publish for create_file, doc create, doc move and the two onboarding creates
tags:
- atomic-write-door
- no-clobber
---

# No-clobber publish — implementation plan

**Status (2026-11-15):** Tasks 1, 2, 3 and 5 are on `experiments` (tip `1c6ec18e3`, branch `fix/no-clobber-publish-20261115`, gate green on the same source tree, Wine run of the `util::fs` tests green). Task 4 is NOT built: it waits for the base decision in Finding 5. The `doc(create)` bug stays open.
**Bugs closed (the no-clobber half):** a2baa3ad8e32cff9 (`create_file`, open only for the `overwrite: true` window), 4b345c816297115b (`doc(move)`, fixed), and the two create-if-absent writes of the onboarding bug (af3e4091f67e2f67, fixed). **Not closed:** 5f7254ff59295050 (`doc(create)`, Task 4).
**Builds on:** `atomic_write` and `refuse_unwritable_existing` (`src/util/fs.rs`). The door landed as `0f26f04fd` (bug a806db1ec4ea162b); the `patch_project_table` half of the onboarding bug landed as `9fb8984bf`..`fb482d04f`.
**Review:** an Opus plan review (2026-11-15) found one Critical and eight Important defects in the first draft; this text is the corrected draft.

## Decision record

**Decision:** `src/util/fs.rs` gets two functions. `rename_no_clobber(from, to)` publishes an existing file at `to` and fails with `AlreadyExists` when anything is at `to`. `write_new_utf8(path, content)` stages the bytes the way `atomic_write` does, then publishes them with `rename_no_clobber`. Four call sites use `write_new_utf8` and one uses `rename_no_clobber`.
**Mechanism:** `std::fs::hard_link(from, to)` then `std::fs::remove_file(from)`. A hard link fails with `AlreadyExists` and never replaces. It needs no new `unsafe` and no per-OS code. On Linux and macOS std calls `linkat` with no flags, which does not follow a symlink at `from` (read in std 1.99.1, `sys/fs/unix.rs`). On Windows it is `CreateHardLinkW`; whether that follows a symlink at `from` is **unverified**.
**Context:** a check-then-write on a separate call is not atomic. `std::fs::rename` replaces on every platform, so a rename cannot be the no-clobber step.
**Alternatives considered:**
- A native no-clobber rename: `renameat2(RENAME_NOREPLACE)` on Linux, `renamex_np(RENAME_EXCL)` on macOS, `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING` on Windows (`windows-sys` is already a dependency). Rejected for now: three `unsafe` bindings and three code paths for a LOW-severity race, where the hard link needs none. Windows can be built here (`x86_64-pc-windows-gnu` is pinned in `rust-toolchain.toml`, recipe in memory `wine-windows-test-suite`); the reason is size of change, not reach. Revisit when the fallback below fires in the field.
- `OpenOptions::create_new` and a direct write. Rejected: a failure mid-write leaves a partial file at the final path, which is the defect a806db1ec4ea162b removed. (The integration branch's `Exclusive` mode does exactly this; see Finding 5.)
- A `flock` on the directory. Rejected: advisory, and no other writer takes it.
**Fallback:** when `hard_link` fails with anything but `AlreadyExists` (a file system without hard links, such as FAT; a `protected_hardlinks` `EPERM`), `rename_no_clobber` falls back to "refuse with `ErrorKind::AlreadyExists` if `symlink_metadata(to)` succeeds, else `rename`". That is today's behaviour, so no file system gets worse, and every other error (read-only directory, `ENOSPC`, `EXDEV`, `EMLINK`) reaches `rename`, which fails or succeeds as before. The fallback is the old race. It logs one `tracing::warn!` when it fires, so a field report can be observed.
**Consequences:** now easier: a concurrent creator can no longer be replaced silently. Now harder: both names exist for a moment between the link and the remove. A crash there leaves a stray `.tmp` for a create, and for `doc(move)` leaves the document at both paths with the same frontmatter `id:`.
**Change scenarios absorbed:** a fourth no-clobber caller needs one call, not a new mechanism; a native no-clobber rename replaces only the body of `rename_no_clobber`.
**Revisit-when:** the fallback's warning shows up in a log, or a Windows CI failure traces to `hard_link`.
**Confidence:** medium. The mechanism is proven on Linux by the tests below. macOS is not run here. Windows is built and the `util::fs` tests are run under Wine (Wine is not NTFS: cite the memory's "what Wine can prove" section); NTFS behaviour stays unverified.

## Findings that shape the tasks (read before dispatch)

1. **`doc(create)` cannot "take the catalog row back" on `AlreadyExists`.** The artifact id is `sha256(abs_path)` (`ids::artifact_id_from_abs`), and `upsert` is `ON CONFLICT(id) DO UPDATE`. The losing creator's commit overwrites the winner's row and augmentation, and its `artifact::delete` then removes them. The fix is an order change: publish the file inside the open catalog transaction, before `tx.commit()`. `AlreadyExists` then drops the transaction (rollback) and the winner's row is untouched. A commit failure after a successful publish removes the file this call made. Verified by the review: nothing inside the transaction reads the target file, opens a second connection or re-takes the catalog lock; the catalog is WAL with `busy_timeout = 5000`.
2. **`undo_failed_create` has one production caller** (`create.rs:572`) **and one test** (`create.rs:1481-1514`), and removes the file at the path unconditionally. Under the new order it has no caller: delete it and its test (a reduced version would be dead code).
3. **`platform::rename_overwrite` is unused and its docs lie** (`platform/mod.rs:124-129`, `windows.rs:334-339`: `remove_file` then `rename`). Not touched; filed as a bug in Task 6.
4. **Out of scope, bug stays open:** the `overwrite: true` shrink-guard window of beae053db1c0a3f6. A rename has no compare-and-swap, so it can be narrowed, not closed.
5. **Task 4 targets a file that another line rewrites.** `integration/identity-lifecycle-20261114` (memory `lifecycle-session-state`; `experiments` receives one merge from it when the operator says all work is done) changes `src/librarian/tools/create.rs` by +832/−139 and touches none of the other four files of this plan. On it, `create::call` delegates to `write_new(…, WriteMode::{Plain, Exclusive})`: `Plain` keeps commit, `std::fs::write`, `undo_failed_create`; `Exclusive` uses `insert_new`, then `create_new` + `write_all`, and takes the row back on failure without removing a file (a partial file can remain). **Ruling:** Tasks 1, 2, 3, 5 and 6 run now on `experiments`. Task 4 is **blocked on the operator's choice of base**: (a) land it after the integration branch is merged, and apply the new order inside `write_new`'s `Plain` arm and switch `Exclusive` from `create_new` + `write_all` to `write_new_utf8`; or (b) build it on the integration branch through that line's own landing protocol. Until then the `doc(create)` bug stays open and gets a pointer to this plan. The wording and test of commit `561c945d7` are already on `experiments` (`create.rs:574`, `:1517`); there is no conflict to resolve with that branch.

## Global Constraints

- No new `unsafe`. No new dependency. Non-test code uses no `cfg(unix)`-only API without a `cfg(not(unix))` arm.
- `write_new_utf8` runs `config::protected_files::check_write` as `atomic_write` does. A create can never replace, so its "before" is "absent": **before `check_write`, return `AlreadyExists` if `symlink_metadata(path)` succeeds**, otherwise a racer's `project.toml` that holds a `[security]` table would make `check_write` refuse with `PermissionDenied` before the no-clobber step and the caller would never see `AlreadyExists`. This check reports the refusal and replaces nothing.
- **`AlreadyExists` means one thing: the target is occupied.** `write_new_utf8` re-kinds every other `AlreadyExists` (from `create_dir_all` when an ancestor is a regular file, std 1.99.1 `fs.rs:3584-3592`; from `create_staging_file` after 16 attempts, `fs.rs:367-376`) to `ErrorKind::Other` with context. Callers also require `symlink_metadata(path).is_ok()` before they map to the "path exists" refusal.
- The staging file is created with `create_staging_file` (exclusive, never follows a link) and is removed on every failure path. Leftover name: `<file name>.<32 hex>.tmp`.
- A refusal on an occupied path changes nothing: the old bytes, mode and link target stay, and no staging file remains.
- Observations (Linux only, `#[cfg(target_os = "linux")]` as in `atomic_write`): a raced refusal records `Attempted` then `Uncertain` (the `WritePhase` values are `Attempted`, `Committed`, `Uncertain`, `observations/server.rs:160-164`), which `observe_call` turns into `error-possible-effects`. The pre-check refusal at `create_file.rs:78` stays `refused`. State this in the doc comment of `write_new_utf8`.
- Tests that need symlinks or modes are `#[cfg(all(test, unix))]`. A test of a race window uses a seam, not a sleep. A test that expects a refusal must reach it through the seam, because a path that is occupied before the call is refused by the old check and passes on the unfixed code.
- Gate: `./scripts/gate.sh --check`, then a Windows build and the `util::fs` no-clobber tests under Wine: `scripts/with-slot.sh cargo test --lib --no-default-features --target x86_64-pc-windows-gnu --no-run`, then run under Wine as memory `wine-windows-test-suite` says. Report what Wine can and cannot prove.

## Task 1: the two helpers in `src/util/fs.rs`

**Files:** `src/util/fs.rs` (functions and tests).

- [ ] `pub fn rename_no_clobber(from: &Path, to: &Path) -> std::io::Result<()>`: `hard_link(from, to)`; on `AlreadyExists` return it unchanged and leave `from` in place; on success `remove_file(from)`; if that remove fails, remove `to` again (roll back) and return the remove error, naming both paths if the rollback also fails; on any other link error use the documented fallback (refuses with `AlreadyExists`, `tracing::warn!` once).
- [ ] `pub fn write_new_utf8(path: &Path, content: &str) -> std::io::Result<()>`: the occupied pre-check, `check_write`, `create_dir_all(parent)`, stage, `rename_no_clobber(staging, path)`; on `AlreadyExists` remove the staging file and return `AlreadyExists`. Replace the final `rename` of `atomic_write_with` (`fs.rs:~509`) with a `publish` callback (the review found this is a small diff; the `inspect_err` cleanup stays; `atomic_write_with` has 5 callers, 1 production and 4 tests).
- [ ] A `#[cfg(test)]` seam: a thread-local `before_publish` closure called with the target path just before the link, so a test can plant a file in the window. Document it next to the seam. It runs on the calling thread; the async tool tests are `#[tokio::test]` current-thread, so it reaches them; a seam closure must not touch a lock the caller holds.
- [ ] Tests (write each first and watch it fail on the missing function): an occupied path stays byte-identical and returns `AlreadyExists`; no staging file remains after a refusal; a free path gets the content; a file planted by the seam is not replaced; a dangling symlink at the path counts as occupied and stays a link; a racer's file with a `[security]` table at a protected `project.toml` path gives `AlreadyExists`, not `PermissionDenied`; **one test per non-publish `AlreadyExists` cause** (an ancestor that is a regular file gives `Other`; an exhausted staging name gives `Other`); a failed write leaves no partial file and no staging file; the fallback function, called directly, refuses an occupied path with `AlreadyExists` and publishes a free one; after `rename_no_clobber` `from` is gone and `to` has the bytes; a read-only directory gives `PermissionDenied` and leaves `from`.
- [ ] Mutation probe: replace `hard_link` with `rename` (the seam test must fail); drop the rollback of the failed `remove_file` (the Task 3 test must fail). The "drop the `AlreadyExists` arm" mutant is **equivalent** (`EEXIST` falls to the fallback, which refuses with the same kind): do not run it.

## Task 2: `create_file` (beae053db1c0a3f6)

**Files:** `src/tools/create_file.rs`.

- [ ] For `overwrite == false`, replace `write_utf8` with `write_new_utf8`. On `AlreadyExists` (with `symlink_metadata(resolved).is_ok()`), return the same refusal as the check at `:78`, with its replay handle, **keeping the exact prefix `file already exists:`** (`src/usage/db.rs:968` classifies on it). Extract that branch into one local function so the two paths cannot drift.
- [ ] `overwrite == true` keeps `write_utf8`.
- [ ] Behaviour change, stated and tested: a dangling symlink at the path was replaced before (`exists()` is false, then the rename). It is now refused as occupied, with no replay handle (`OverwriteGuard::of` uses `metadata`).
- [ ] Tests: through the seam, a file planted between the check and the write stays unchanged and the call is refused with the replay handle; the handle's replay (`ack=`) then replaces it (keep the planted file small: the replay runs the shrink guard); a normal create still works; `overwrite: true` still replaces; the dangling-symlink case above; a blocker file as an ancestor gives the write's own error, not "file already exists".

## Task 3: `doc(move)` (3ca37d8db3572e4e)

**Files:** `src/librarian/tools/mv.rs`.

- [ ] Replace `std::fs::rename(&old_full, &new_full)?` (`:372`) with `rename_no_clobber`. On `AlreadyExists` (with `symlink_metadata(new_full).is_ok()`) return the same `destination '…' already exists` refusal as `:287` and leave the source in place and the catalog untouched.
- [ ] On `cfg(not(unix))` a symlink source uses a plain rename guarded by the existing `exists` check, or the report states that Windows link-following of `hard_link` is unverified. Pick the smaller diff and say which.
- [ ] Tests: a file planted at the destination through the seam stays unchanged and the move is refused with the source intact and the row unchanged; a normal move still re-keys; moving a symlink still moves the link and not its target (Unix); **the rollback**: a source directory `0555` and a writable destination (Unix, skipped for root as at `create.rs:1443`): `linkat` succeeds, `remove_file(from)` fails with `EACCES`; expect the error, the source intact, no destination file and the row unchanged.

## Task 4: `doc(create)` (07ebee9fd570c738) — BLOCKED on the base decision in Finding 5

**Files:** `src/librarian/tools/create.rs`. Do not dispatch until the operator chooses (a) or (b).

- [ ] Move the disk publish (`create_dir_all` and `write_new_utf8`) inside the catalog transaction, after the catalog writes and before `tx.commit()`. The seam closure must not touch `ctx.catalog` (the lock is held).
- [ ] `AlreadyExists` returns `LibrarianRecoverableError` `path exists: …` (the message of `:412`) and the dropped transaction leaves the catalog as it was. Any other write error rolls back and returns the write's own error with a **true** message (the text at `:574`, "cleanup was attempted, but the catalog row or a partial file may remain", becomes false: nothing was catalogued; only directories this call created may remain).
- [ ] If `tx.commit()` fails after a successful publish, remove the file this call made, **with the held `cat`, never `ctx.catalog.lock()`** (the lock is not re-entrant), then return the commit error.
- [ ] Delete `undo_failed_create` and its test (`:1481-1514`). Update `failed_disk_write_reports_best_effort_cleanup_without_promising_rollback` (`:1517`): its `delete_fails = true` arm expects 1 row and now gets 0; delete or rewrite that arm and the message assertion.
- [ ] Update the stale comments: "This write does not go through `atomic_write` … plain `fs::write`" (~`:488`), the `:551-563` comments, and the `protected_files.rs:36-38` doc sentence.
- [ ] Tests: a file planted through the seam stays unchanged, the call is refused, and the winner's row (title, `file_sha256`, augmentation prompt, seeded by the test) is unchanged **field by field** (a "publish after commit" mutant keeps the row and overwrites its fields); a failed write leaves no row, no file and no staging file; **a commit failure removes the published file**, built with the repo's deferred foreign-key tripwire (`dual_write.rs:852-863`: a table with `REFERENCES artifact(id) DEFERRABLE INITIALLY DEFERRED` and an `AFTER INSERT ON artifact` trigger that inserts a dangling id), so that every statement and the publish succeed and only `COMMIT` fails; assert the file is absent, the row count is 0 and the error is the commit error (a statement-time trigger would fire before the publish and prove nothing); a normal create, with and without `augment`, still works.

## Task 5: the two onboarding creates

**Files:** `src/tools/onboarding.rs`, `src/config/protected_files.rs` (module doc only).

- [ ] `write_workspace_config_if_needed` (`:1593-1631`) and the `project.toml` create (`:1802-1849`) use `write_new_utf8`. On `AlreadyExists` with `symlink_metadata` ok, treat the file as created by someone else: the first returns `Ok(())`, the second leaves `created_config == false`. Any other error propagates (a blocker ancestor must not return `Ok`).
- [ ] The review confirmed that `check_write` admits the create content for an absent file (`protected_files.rs:87-149`, `165-179`: absent gives `Ok(None)`). Confirm the `observe_write` facts now emitted do no harm, and say what you measured.
- [ ] Update the `protected_files.rs` module doc sentence that says onboarding's create uses `std::fs::write`.
- [ ] Tests: an existing `project.toml` (and one holding a `[security]` table) and an existing `workspace.toml` planted through the seam are not changed and the call succeeds; the `workspace.toml` fixture needs more than one project (`onboarding.rs:1598`); a normal first onboarding still creates both; a blocker ancestor is an error.

## Task 6: records (controller, after the final review)

- [ ] Update the bug files: Fix provenance with SHA and patch-id, Tests added, Resume. beae053db1c0a3f6 stays open for the shrink-guard window only. The onboarding bug can close (its `patch_project_table` half is fixed, `onboarding.rs:164-170`). 07ebee9fd570c738: correct the fix text (the transaction order, not "take the row back") and point to Task 4 and its blocker.
- [ ] File two bugs: the unused, misdocumented `platform::rename_overwrite`; and `librarian::import_codescout` (`src/librarian/mod.rs:499` `exists()` then `:597` `std::fs::write`, CLI only), which has the same check-then-write pattern.
- [ ] Reconcile the catalog with `merge_worktree` before removing the worktree. The review's branch name for the peer work is `fix/ecc-doc-create-cleanup-20261109`.
