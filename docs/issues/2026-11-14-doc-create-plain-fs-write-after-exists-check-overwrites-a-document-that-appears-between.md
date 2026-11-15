---
id: '07ebee9fd570c738'
kind: bug
status: open
title: 'BUG: doc(create) writes with a plain fs::write after an exists check, so a document that appears in between is overwritten'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not reproduced. Not established: that two sessions have ever hit the window, how wide the window is under load, and how a Windows target behaves. The file that is lost is one a concurrent writer put at the path. A file that exists at check time is refused, so a document that git holds is not at risk by this path.'
---

# BUG: doc(create) writes with a plain fs::write after an exists check, so a document that appears in between is overwritten

## Summary
`doc(action="create")` refuses a path that exists (`src/librarian/tools/create.rs:412`). It then writes the new document with `std::fs::write` (`src/librarian/tools/create.rs:569`). That call does not go through `atomic_write` or `write_catalogued_file`. A file that appears at the path after the check is truncated and replaced. The write itself is also not atomic.

## Class fit
cluster/unclassified: the writer sits outside the atomic write door that `write_catalogued_file` now uses (`atomic_write` in `src/util/fs.rs`). No existing class names a file writer that bypasses the door or that checks and then writes. This bug differs from a806db1ec4ea162b: that writer used the door and the door was weak. This writer never used the door, and it has a second weakness, a check-then-write race.

## Symptom (Effect)
Two sessions create the same `rel_path` at the same time. Both pass the exists check. The second `fs::write` truncates the first document and writes its own content. The first caller got `ok` and its document is gone. The same happens when a person or a tool writes the file in the window.
A failed write after the truncate leaves a short or empty file. `undo_failed_create` (`create.rs:330-339`) then calls `remove_file` on the path. It removes whatever is at the path, also a file that a concurrent writer put there.

## Reproduction
Not reproduced. Read path: `create::call` → `full.exists()` (L412) → catalog transaction (`upsert_and_mint_slug`, augmentation, registration) → `create_dir_all` + `std::fs::write(&full, &content)` (L567-569). The catalog work between the check and the write widens the window.

## Environment
Branch feat/atomic-write-door-20261114 at 346fd5e0b. Read during Task 3 of the atomic catalogued write door plan.

## Root cause
`create.rs:569` is a plain `std::fs::write`. It opens the path with truncate and without `create_new`. The exists check at `create.rs:412` is a separate call, so the two steps are not one operation. `create.rs:423` and `create.rs:432` refuse links and paths outside the repository, but they do not close the race.

## Fix

Publish the new document with a no-clobber, atomic step, and publish it **inside the open catalog transaction**, before `tx.commit()`.

The first version of this section said: on `AlreadyExists`, take the catalog row back. That is wrong. The artifact id is `sha256(abs_path)` (`ids::artifact_id_from_abs`) and `artifact::upsert` is `ON CONFLICT(id) DO UPDATE`. The loser of a race commits the same row id, which overwrites the winner's row and augmentation. A later `artifact::delete(id)` by the loser would then remove the row the winner relies on. The disk write comes after the commit today, so the catalog is already shared by the time the race is lost.

The corrected design:
- Move `create_dir_all` and `crate::util::fs::write_new_utf8` (a hard-link publish that fails with `AlreadyExists`, and leaves no partial file) inside the transaction, after the catalog writes and before `tx.commit()`. Nothing inside the transaction reads the target file, opens a second connection or takes the catalog lock again (checked by an Opus plan review, 2026-11-15); the catalog is WAL with `busy_timeout = 5000`.
- On `AlreadyExists` (and `symlink_metadata(full).is_ok()`), return the `path exists:` refusal and let the dropped transaction roll back. The winner's row is untouched.
- Any other write error rolls back too. The error text at `create.rs` ("cleanup was attempted, but the catalog row or a partial file may remain") becomes false and must change: nothing was catalogued, and only directories this call created may remain.
- If `tx.commit()` fails after a successful publish, remove the file this call made with the held catalog guard (never `ctx.catalog.lock()` again; the lock is not re-entrant), then return the commit error.
- Delete `undo_failed_create` and its test: it has no caller left, and its unconditional `remove_file(full)` removes a concurrent writer's file.
- Update `failed_disk_write_reports_best_effort_cleanup_without_promising_rollback`: its `delete_fails = true` arm expects one row and gets none.
- Tests: a file planted through the seam (`util::fs::set_before_publish`) stays unchanged and the winner's row, compared field by field (title, `file_sha256`, augmentation prompt), is unchanged; a commit failure removes the published file, built with the deferred foreign-key tripwire in `dual_write.rs:852-863` (a statement-time trigger would fire before the publish and prove nothing); a failed write leaves no row, no file and no staging file.

The helper this needs exists: `write_new_utf8` and `rename_no_clobber` landed on `experiments` (see the `create_file` and `doc(move)` sibling bugs, and the plan `docs/superpowers/plans/2026-11-15-no-clobber-publish.md`, Task 4).

## Resume

Blocked on a base decision, not on design. Task 4 of `docs/superpowers/plans/2026-11-15-no-clobber-publish.md` is written and reviewed, and was left out of that branch on purpose: `integration/identity-lifecycle-20261114` rewrites `src/librarian/tools/create.rs` (+832/−139). On that branch `create::call` delegates to `write_new(…, WriteMode::{Plain, Exclusive})`. `Plain` keeps the old order (commit, `std::fs::write`, `undo_failed_create`). `Exclusive` uses `insert_new`, then `OpenOptions::create_new` and `write_all`, and takes the row back on failure without removing a file, so a partial file can remain. Two ways forward, the operator's choice: (a) land Task 4 after that branch merges into `experiments`, applying the new order inside `write_new`'s `Plain` arm and switching `Exclusive` from `create_new` + `write_all` to `write_new_utf8`; or (b) build it on the integration branch through that line's own landing protocol.

## References
- `src/librarian/tools/create.rs:412`, `src/librarian/tools/create.rs:569`, `src/librarian/tools/create.rs:330`
- `src/librarian/util.rs` (`write_catalogued_file`), `src/util/fs.rs` (`atomic_write`)
- a806db1ec4ea162b (the door itself)
