---
id: a806db1ec4ea162b
kind: bug
status: open
title: 'BUG: write_catalogued_file truncates the ledger in place, so a failed write loses it'
owners:
- marius
tags:
- cluster/unclassified
- librarian
opened: 2026-11-13
severity: medium
unverified: 'Read from the code, not reproduced. Not established: that any real write has failed this way, or how the dual-write retry rules behave over a truncated file.'
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
Write to a temp file in the same directory, `fsync`, then `rename` over the target. Keep the symlink and outside-repo refusals first. Add a test that a failing write leaves the old bytes. Found by the Plan C1 whole-branch review (finding N-2).

## Resume
Open. Not started.

## References
- `src/librarian/util.rs:85`
- `docs/research/2026-11-13-artifact-identity-plan-c1-execution-state.md`
