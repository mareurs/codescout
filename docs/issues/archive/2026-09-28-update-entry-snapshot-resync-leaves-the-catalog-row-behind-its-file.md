---
id: 60fcfdf99e3288c6
kind: bug
status: archived
title: update_entry's snapshot resync writes the ledger file but leaves the catalog row's hash behind it
tags:
- librarian
- catalog
- cluster/unclassified
closed: 2026-09-28
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

**Fixed 2026-09-28 in `6a6a321e`, patch-id `1fae1b1d51136476730eb4a9233b136c328649e7`, and the scope was wider than filed.** The regression tests were written before the fix, one per writer in `src/librarian/catalog/augmentation.rs`. They showed the same gap at all three sites that write an artifact's file inside a catalog transaction:

- `allocate_entry_id` (the high-water mark, plus a section and its index row);
- `append_entry` (a section);
- `resync_snapshot_row` (one table row).

So a per-site fix to the resync path, the one this file first named, would have left every `append_entry` producing the same finding. The two live `row_behind_file` findings on peer ledgers the same morning (`context-injection-session-log.md`, `reconnaissance-patterns.md`, both last written by appended entries) fit that. The fourth write, `restore_section_after_failed_commit`, restores the original bytes after a rollback, so the row still describes them and it needs no change.

`record_written_file()` updates `file_sha256` and `file_mtime` for the bytes just written, inside the caller's transaction, so a rollback takes the refresh with it. It uses the same derivation as `update.rs`: `sha_of_bytes` over the written bytes, and the file's own mtime in milliseconds.

**Verification:**

- **Red before the fix.** Three tests (`*_leaves_the_row_describing_*`) seed a row whose columns describe no file and assert both columns equal the file on disk afterwards. All three failed on the `file_sha256` assertion.
- **Mutations, via `scripts/mutation-probe.sh` in an isolated worktree, all KILLED.** Removing each call reds only that writer's test, with the other 106 in the module green. Hashing the wrong bytes, and zeroing the mtime, each red all three. The mtime mutation shows that assertion is load-bearing, since the hash assertion runs first and passes under it.
- **Gate:** `FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, with the three tests present by name in the default lane.

The running MCP server predates the fix. It takes effect after a release build (`./scripts/rb.sh`) and `/mcp` in each session; until then live ledgers still drift, and `reindex` repairs them.

## Fix provenance

- **SHA:** `6a6a321e` (`experiments`)
- **patch-id:** `1fae1b1d51136476730eb4a9233b136c328649e7`

Omitted when this record was archived; added the same day. The patch-id was derived from `6a6a321e` with `git show 6a6a321e | git patch-id --stable`.

## References

- `src/librarian/catalog/augmentation.rs` (`update_entry`, `resync_snapshot_row`)
- `src/librarian/tools/update.rs` (the hash refresh the body-update path performs)
- `docs/issues/archive/2026-09-16-a-catalog-row-behind-its-file-is-repairable-but-invisible.md` (the check that reports it)
