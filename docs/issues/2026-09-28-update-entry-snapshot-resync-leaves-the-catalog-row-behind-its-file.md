---
id: '49a01cb32b73e7e7'
kind: bug
status: open
title: update_entry's snapshot resync writes the ledger file but leaves the catalog row's hash behind it
tags:
- librarian
- catalog
- cluster/unclassified
opened: 2026-09-28
severity: low
---

## Summary

`update_entry` on a ledger that declares `snapshot_anchor` re-renders the entry's row in the body table (`resync_snapshot_row`, `src/librarian/catalog/augmentation.rs`). It writes the file with `std::fs::write` inside the params transaction, and never refreshes the artifact row's `file_sha256` / `file_mtime`. So every resynced row leaves the catalog row behind its file, and `doctor` reports `row_behind_file` until someone runs `reindex`. `doc(action="update")` does refresh the hash when it writes (`src/librarian/tools/update.rs`, `file_sha256: sha_of_bytes(new_content)` before `artifact::upsert_and_mint_slug`), so this is the one body-writing path that skips the row.

## Reproduction

Observed 2026-09-28 on `docs/trackers/review-catches.md` (artifact `3fc46942aafb886e`, `snapshot_anchor` declared that day), with no net data change:

1. `librarian(action="reindex")`. Afterwards the catalog row and the file both hash to `5531994b33f8`.
2. `doc(action="update_entry", id="3fc46942aafb886e", entry_collection="catches", entry_id="RC-2", fields={"promoted_to": "<value> [repro-probe]"})` returned `row_resynced: true`.
3. `librarian(action="doctor")` then reported, for this artifact only: `row_behind_file` — *catalog holds 5531994b33f8 but the file hashes to 4d5acb0d278b*.
4. Reverting RC-2's field re-rendered the row back, and the file hashed to `5531994b33f8` again.

The same state had followed 54 `update_entry` calls earlier the same day (catalog `a43d697b8d84`, file `5531994b33f8`).

## Impact

Low as observed. `reindex` repairs it and `doctor` names it. The catalog row describes content the file no longer has until then; which readers consume the row's hash or derived state rather than the file was **not checked**. Every snapshot-anchored ledger hits this on every `update_entry` that changes a rendered cell, so the finding recurs in `doctor` output and trains readers to expect it.

## Fix direction

Have `resync_snapshot_row` (or `update_entry` after it) refresh the artifact row's `file_sha256` and `file_mtime` for the bytes it wrote, inside the same transaction, as `update.rs` does. Regression test: after an `update_entry` that resyncs a row, `doctor` reports no `row_behind_file` for that artifact. A control should show the test reds with the refresh removed.

## References

- `src/librarian/catalog/augmentation.rs` (`update_entry`, `resync_snapshot_row`)
- `src/librarian/tools/update.rs` (the hash refresh the body-update path performs)
- `docs/issues/archive/2026-09-16-a-catalog-row-behind-its-file-is-repairable-but-invisible.md` (the check that reports it)
