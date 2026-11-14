---
id: a806db1ec4ea162b
kind: bug
status: fixed
title: 'BUG: write_catalogued_file truncates the ledger in place, so a failed write loses it'
owners:
- marius
tags:
- cluster/unclassified
- librarian
closed: 2026-11-14
opened: 2026-11-13
severity: medium
unverified: 'STANDING: no real write has been seen to fail this way. The tests simulate the failures: a write that fails after the staging file exists, and a read-only directory. They do not fill a disk. The change does not claim power-loss durability (no fsync, plan ruling A-R1).'
---

# BUG: write_catalogued_file truncates the ledger in place, so a failed write loses it

## Summary
`write_catalogued_file` (`src/librarian/util.rs:85-94`) ends in `std::fs::write(path, contents)`. That call truncates the file, then writes. A failure between the two steps (disk full, process kill) leaves a short or empty ledger. Since Plan C1 the file is the primary store for entry writes, so the loss is the only copy until git restores it.

## Class fit
cluster/unclassified: a durability gap in the one door every in-place catalogued write uses, not a logic defect in one tool. No existing class names a non-atomic file write.

## Symptom (Effect)
After a failed write the ledger file is truncated or empty. The catalog row still exists. The next read sees a ledger with no entries.

## Reproduction
Not reproduced. Read path: `append_entry` (file first, since Plan C1) → `write_entry_into_ledger` → `write_catalogued_file` → `std::fs::write`. Fill the target filesystem during the call.

## Environment
experiments at 7db510676.

## Root cause
`std::fs::write` opens with truncate. There is no temp file and no rename.

## Fix

The door replaces the document atomically. `write_catalogued_file` (`src/librarian/util.rs`) keeps the symbolic-link and outside-repository refusals first. It then runs `refuse_unwritable_existing` (`src/util/fs.rs`), so a read-only document stays refused, and writes through `atomic_write`: a staging file in the same directory, then a rename. A write that fails leaves the old bytes in place and no staging file. An `io::Error` from the door now means the document is unchanged, so the partial-write stopgap in the `event_create` and `doc(link)` write errors is gone.

`fsync` is not called. Plan ruling A-R1 declined it: `atomic_write` does not call it, and the edit tools use it the same way. The change closes the failures this bug names, a full disk and a killed process. It does not claim power-loss durability.

The fix commits are listed under `## Fix provenance`. Found by the Plan C1 whole-branch review (finding N-2).

## Fix provenance

- **SHA:** `4bae72783` (`feat/atomic-write-door-20261114`; the door writes atomically and the probe `refuse_unwritable_existing` is added)
- **patch-id:** `57304338dec8643a781ae1a22c071217fb7a498c`
- **SHA:** `346fd5e0b` (`feat/atomic-write-door-20261114`; the partial-write stopgap is removed)
- **patch-id:** `598b5f38063ce40b20979de1495973f03455898d`

## Tests added

In `src/librarian/util.rs` (module `tests`):
- `a_write_replaces_the_document_and_leaves_no_staging_file`
- `a_missing_target_is_created_and_leaves_no_staging_file`
- `a_write_in_a_read_only_directory_is_refused_and_leaves_the_old_bytes`
- `a_read_only_document_is_still_refused_and_unchanged`
- `the_document_keeps_its_exec_bit_and_a_private_mode`
- `a_protected_file_refusal_does_not_carry_the_directory_hint`

In `src/util/fs.rs`: the module `refuse_unwritable_existing_tests`, and `atomic_write_removes_its_tmp_file_when_the_write_itself_fails` (a write that fails after the staging file exists).

## Resume

Done. The door writes atomically and the stopgap is removed. Nothing is left to do for this bug. Open follow-ups are separate bug files: `doc(create)` and `doc(move)` (sibling writers outside the door), `patch_project_table` and `create_file` (writers that check and then write).

## References
- `src/librarian/util.rs:85`
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
