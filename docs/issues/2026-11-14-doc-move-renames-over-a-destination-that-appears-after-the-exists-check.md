---
id: '3ca37d8db3572e4e'
kind: bug
status: fixed
title: 'BUG: doc(move) renames over a destination that appears after the exists check, so that document is replaced'
owners:
- marius
tags:
- cluster/unclassified
- librarian
closed: 2026-11-15
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not reproduced. Not established: that any session has hit the window, how wide it is under load, and whether the hard-link fix works on every supported file system. The file that is lost is one a concurrent writer put at the destination. A destination that exists at check time is refused.'
---

# BUG: doc(move) renames over a destination that appears after the exists check, so that document is replaced

## Summary
`doc(action="move")` refuses a destination that exists (`src/librarian/tools/mv.rs:287`). It then calls `std::fs::rename(&old_full, &new_full)` (`src/librarian/tools/mv.rs:372`). A rename replaces an existing target without an error. A file that appears at the destination after the check is replaced, and its bytes are lost unless git holds them.

## Class fit
cluster/unclassified: the move writes a document path outside the atomic write door (`atomic_write` in `src/util/fs.rs`), and it checks and then acts. No existing class names a path replacement that follows a separate existence check. This bug differs from the `doc(create)` sibling (07ebee9fd570c738): the rename itself is atomic, so the source is never torn. Only the destination can be clobbered.

## Symptom (Effect)
A session moves document A to path P. Another session or a person creates a file at P after line 287 and before line 372. The rename succeeds. The file at P is gone. The caller gets `moved: true`, and nothing says that a file was replaced.

## Reproduction
Not reproduced. Read path: `mv::call` → `new_full.exists()` (L287) → read of the source through the read door → archive guard → protected-files guard → `create_dir_all` (L369) → `std::fs::rename` (L372). Those steps sit between the check and the rename.

## Environment
Branch feat/atomic-write-door-20261114 at 346fd5e0b. Read during Task 3 of the atomic catalogued write door plan.

## Root cause
`std::fs::rename` replaces an existing destination on Unix. On Windows `std::fs::rename` also replaces it. The existence check at `mv.rs:287` is a separate call. The code has no no-clobber step at the rename. (`platform::rename_overwrite` in `src/platform/mod.rs` also replaces and has no caller.)

## Fix
Publish the destination with a call that fails when the target exists. For example: `std::fs::hard_link(old, new)`, then `remove_file(old)`. A hard link fails with `AlreadyExists`. On `AlreadyExists`, return the same "destination already exists" refusal and leave the source in place. Fall back to a refusal when the link fails across file systems. Tests: a file placed at the destination between the check and the rename stays unchanged and the move is refused (this needs a test seam between the two steps).

## Fix provenance

Branch `fix/no-clobber-publish-20261115` on `experiments` (tip `1c6ec18e3`), plan `docs/superpowers/plans/2026-11-15-no-clobber-publish.md` (db78f37db84ba413). The fix is `std::fs::rename` replaced by `rename_no_clobber` at the publish step of `src/librarian/tools/mv.rs`.

- **SHA:** `f9ce46574` (`rename_no_clobber` and `write_new_utf8` in `src/util/fs.rs`)
- **patch-id:** `ab73efcee0fd1549ffaec7440948f47e85e81c00`
- **SHA:** `d4037f9d5` (`doc(move)` publishes the destination with `rename_no_clobber`; the private helper `destination_occupied` gives the old check and the new refusal one text)
- **patch-id:** `a2d56cd248171169a6bf9a4f2053f91094a60956`
- **SHA:** `1c6ec18e3` (the contract in the doc comments; the comment at the publish step)
- **patch-id:** `c0754f781d8b9263fc533aa326bf35569fd315c0`

The fallback the fix chose differs from the plan text above: the staged file and the destination share a directory, so a link never crosses a file system. A link that fails for any other reason than `AlreadyExists` (a file system without hard links, a `protected_hardlinks` EPERM) falls back to "refuse if `symlink_metadata(to)` succeeds, else rename". That is the old behaviour and keeps the old race on such a file system.

## Tests added

In `src/librarian/tools/mv.rs` (module `tests`):
- `a_file_planted_after_the_existence_check_is_refused_and_nothing_moves`: the occupant is planted through the test seam; the source, the row (id, path, digest) and the occupant's bytes stay unchanged. It detects a move that does not publish through `rename_no_clobber`; the no-clobber guarantee itself is pinned by `util::fs::no_clobber_tests`.
- `moving_a_symlink_moves_the_link_and_not_its_target` (Unix): `linkat` with no flags links the symlink itself.
- `a_source_that_cannot_be_unlinked_leaves_the_file_at_the_source_only` (Unix, skipped as root): a read-only source directory; the link succeeds, the unlink fails with EACCES, and the rollback takes the new link back. It fails when the rollback is dropped (measured by hand in `src/util/fs.rs`).

In `src/util/fs.rs` (module `no_clobber_tests`) the helper is pinned by 22 tests, among them the planted file, directory and dangling symlink through the seam, and the fallback called directly. Under Wine (`x86_64-pc-windows-gnu`) 16 of them run and pass.

## Behaviour changes and residual windows

Changed: a dangling symlink at the destination is now refused with "destination '…' already exists". The old `rename` replaced it, because `exists()` follows the link and says false. The refusal is pinned only by the `util::fs` tests, not by a `doc(move)` test. On disk the move is now a link and an unlink: the file's ctime changes and its link count is 2 for a moment. The librarian has no file watcher, so nothing depends on rename events.

Residual, all LOW unless stated:
- A crash between the link and the unlink leaves the document at both paths with the same frontmatter `id:` and the catalog row on the old path (LOW to MED). A failed rollback gives the same state, and the error says so; that branch has no test.
- `NotFound` from the unlink (the source was removed or renamed by someone else in the window) makes the rollback delete the new link. If the source was renamed the data survives under the other name; if it was deleted, the delete wins. The caller gets `NotFound` and the row points at a missing path, which `doctor` reports as `missing_file`.
- The rollback and the unlink act by name: a racer that replaces the destination during the rollback, or the source before the unlink, loses its file.
- The Windows `CreateHardLinkW` behaviour on a symlink source is unverified (Wine is not NTFS). If it follows the link, a moved symlink document becomes a hard link to its target. Symlinked documents are rare on Windows.
- The post-publish refusal ignores the half-completed-move discriminator (`old_full` missing): a hand `mv` in a microsecond window could get the plain "delete it first" advice for the artifact itself.

## Resume

N/A: fixed.

## References
- `src/librarian/tools/mv.rs:287`, `src/librarian/tools/mv.rs:372`
- 07ebee9fd570c738 (the `doc(create)` sibling)
- a806db1ec4ea162b (the door itself)
