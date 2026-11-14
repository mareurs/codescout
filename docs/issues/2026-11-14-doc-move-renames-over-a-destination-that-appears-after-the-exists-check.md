---
id: '3ca37d8db3572e4e'
kind: bug
status: open
title: 'BUG: doc(move) renames over a destination that appears after the exists check, so that document is replaced'
owners:
- marius
tags:
- cluster/unclassified
- librarian
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

## Resume
Open. Not started.

## References
- `src/librarian/tools/mv.rs:287`, `src/librarian/tools/mv.rs:372`
- 07ebee9fd570c738 (the `doc(create)` sibling)
- a806db1ec4ea162b (the door itself)
