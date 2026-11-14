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
Publish the new document with a no-clobber, atomic step. Stage the bytes in a temp file in the same directory, as `atomic_write` does. Then publish with a call that fails when the target exists (for example `hard_link` then `remove_file` of the staging file, or `OpenOptions::create_new` as the minimum). On `AlreadyExists`, take the catalog row back and return the same "path exists" refusal. Keep `undo_failed_create`, but make it remove only a file this call made. Tests: a file that appears between the check and the write stays unchanged; a failed write leaves no partial file.

## Resume
Open. Not started.

## References
- `src/librarian/tools/create.rs:412`, `src/librarian/tools/create.rs:569`, `src/librarian/tools/create.rs:330`
- `src/librarian/util.rs` (`write_catalogued_file`), `src/util/fs.rs` (`atomic_write`)
- a806db1ec4ea162b (the door itself)
