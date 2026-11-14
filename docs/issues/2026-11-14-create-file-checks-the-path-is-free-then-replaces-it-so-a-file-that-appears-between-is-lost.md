---
id: beae053db1c0a3f6
kind: bug
status: open
title: 'BUG: create_file checks that the path is free and then replaces it, so a file that appears in between is replaced'
owners:
- marius
tags:
- cluster/unclassified
- create_file
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not reproduced. Not established: that two sessions have hit the window, how wide it is under load, and whether hard links work on every supported file system. The file that is lost is one a concurrent writer put at the path. A file that exists at check time is refused, so a file that git holds at that path is not at risk by this window alone.'
---

# BUG: create_file checks that the path is free and then replaces it, so a file that appears in between is replaced

## Summary
`create_file` without `overwrite` refuses a path that exists (`src/tools/create_file.rs:78`). It then writes through `write_utf8` (`src/tools/create_file.rs:139`), which calls `atomic_write`. A rename replaces an existing target without an error. A file that appears at the path after the check is replaced, and `create_file` reports `ok`. The write itself is atomic, so no file is torn. Only the check-then-act race is open.

## Class fit
cluster/unclassified: the tool checks and then acts, in two separate steps, and the second step replaces whatever is at the path. No existing class names a path replacement that follows a separate existence check. This bug differs from the `doc(create)` sibling (07ebee9fd570c738): that writer truncates in place, and this one replaces atomically, so the bytes of the old file are never mixed with the new ones.

## Symptom (Effect)
Two sessions call `create_file` for the same new path at the same time. Both pass the check at L78. The second rename replaces the first file. Both callers get `ok`. The first document is gone, and nothing says that a file was replaced.
A person or a tool that writes the path inside the window loses its file the same way.

## Reproduction
Not reproduced. Read path: `create_file::call` → `resolved.exists()` (L78) → for `overwrite` with an existing file, `read_to_string` and the shrink guard (L110-113) → `write_utf8` (L139). The shrink-guard read sits between the check and the write, and it widens the window. With `overwrite: true`, a change made to the file after that read is replaced by the new content, and the shrink guard judged the old content.

## Environment
Branch feat/atomic-write-door-20261114 at aa29cd410. Found during the final review of the atomic write door.

## Root cause
The check at L78 and the write at L139 are separate calls. `write_utf8` (`src/util/fs.rs:566-571`) is `create_dir_all` plus `atomic_write`. `atomic_write` stages the bytes in a sibling file and renames it onto the path (`src/util/fs.rs:428`). A rename replaces an existing target. `write_utf8` does not call `refuse_unwritable_existing` (`src/util/fs.rs:542`), so it also does not ask whether an existing file is writable. (inferred from `src/tools/create_file.rs:78-139` and `src/util/fs.rs:428-571` - not measured)

## Fix
For the no-overwrite case, publish the staged file with a no-clobber step: `hard_link(staging, path)`, then `remove_file(staging)`. A hard link fails with `AlreadyExists`. On `AlreadyExists`, return the same "file already exists" refusal that L78 returns, and keep the replay handle logic. Fall back to a refusal when the link fails across file systems or on a file system without hard links.
For the overwrite case, the rename is the intended action. The open part is the shrink guard: compare the file against the bytes it read, for example by the size and modification time that `OverwriteGuard` already records, before the rename.
Decide separately whether `create_file(overwrite: true)` may replace a read-only file. Today `atomic_write` replaces it, because a rename needs no permission on the file. That is not part of this bug.
Tests: a file placed at the path between the check and the write stays unchanged and the call is refused (this needs a test seam between the two steps); a failed write leaves no partial file.

## Resume
Open. Not started.

## References
- `src/tools/create_file.rs:78`, `src/tools/create_file.rs:110`, `src/tools/create_file.rs:139`
- `src/util/fs.rs` (`write_utf8`, `atomic_write`, `refuse_unwritable_existing`)
- 07ebee9fd570c738 (the `doc(create)` sibling)
- a806db1ec4ea162b (the door itself)
