---
id: '78082c94e2a56ac6'
kind: bug
status: open
title: 'BUG: an adopted append_entry call that writes nothing never records the file on the artifact row'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-14
severity: low
unverified: 'Read from the code, not reproduced. Not established: that a failed commit can leave the section on disk with the row behind it in practice (the commit path restores the file when it still holds the written bytes), nor what doctor reports after the adopting call. No test pins the row hash on the adopted no-write branch.'
---

# BUG: an adopted append_entry call that writes nothing never records the file on the artifact row

## Summary
On the `append_entry` params path, a retry can adopt a section that is already in the file. If the committed mark already covers that id, the call writes no file. In that branch `record_written_file` is never called, so the artifact row keeps the hash it had before the failed first call.

## Class fit
cluster/unclassified: a writer path that skips the step every other writer path takes (refresh the row's `file_sha256` and `file_mtime`). No existing class names a branch that was left out of a shared post-write step.

## Symptom (Effect)
After the adopting call the file holds the adopted section, but `artifact.file_sha256` describes the older bytes. `doctor` compares exactly that column with the file hash, so it should report `row_behind_file` until a `reindex`.

## Reproduction
Not reproduced. Read path in `src/librarian/catalog/augmentation.rs`, `append_entry`:
1. A first call writes the section and the mark in one file write (`write_entry_into_ledger`, which calls `record_written_file` inside the transaction), then the commit fails. The rollback takes the row refresh with it. The section stays on disk when the restore cannot run (`ForeignChange` or `Failed`).
2. The retry finds the section (`find_unrecorded_section`), so `adopted` is true. The mark is already on disk, so `committed_mark.is_none_or(|m| m < next)` is false.
3. The condition at line 1114, `if !adopted || committed_mark.is_none_or(|m| m < next)`, is false, so `write_entry_into_ledger` is not called and nothing else calls `record_written_file`.

The other adopted branch (mark below the adopted id) goes through `write_entry_into_ledger` and does record the file.

## Environment
experiments at fe5370c2f.

## Root cause
`record_written_file` is called from only one place on this path: the tail of `write_entry_into_ledger`. The no-write branch skips that function. The retry's transaction therefore never refreshes the row, even though the file differs from the row's hash because of the earlier failed call.

## Evidence
- `src/librarian/catalog/dual_write.rs` `record_written_file` doc: three writers refresh the row, and a rollback "takes the refresh with it and the row keeps describing whatever a failed commit restores".
- `src/librarian/catalog/augmentation.rs:1105-1126`: the comment says "no write when the mark already covers it".
- The Plan C1 review lists "no byte-level check of the no-write branch" among its deferred findings (Task 5, M7).

## Hypotheses tried
None. Found by the Plan C1 whole-branch review (N-1), read from the code.

## Fix
Not started. Options: call `record_written_file` with the file's current text in the no-write branch (the text is `section_doc`), or refresh the row from the file after the commit. Add a test that adopts with the mark already covering the id and asserts `file_sha256` equals the file hash.

## Tests added
None.

## Workarounds
`librarian(action="reindex")` refreshes the row.

## Resume
Open. Not started.

## References
- `src/librarian/catalog/augmentation.rs` (`append_entry`, line 1114)
- `src/librarian/catalog/dual_write.rs` (`record_written_file`, `write_entry_into_ledger`, `RetryRule::Adopts`)
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
