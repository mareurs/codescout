---
id: '2d7ef39a4166e5c6'
kind: bug
status: fixed
title: Librarian reindex writes each chunk row as its own fsynced commit, and no catalog connection sets synchronous
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-10-30
opened: 2026-10-30
severity: high
unverified: The synchronous=NORMAL decision is open and was not taken. The 400-chunk figures come from one synthetic body on this disk, not from a real reindex of the live catalog.
---

# BUG: Librarian reindex writes each chunk row as its own fsynced commit, and no catalog connection sets synchronous

## Summary

`replace_chunks` opens no transaction and says the caller should (`src/librarian/catalog/chunk.rs:395-410`). `index_repo_sync` never does; its only transaction scopes are savepoints for removals (`src/librarian/indexer.rs:769`, `:790`). So each chunk DELETE, park, INSERT and re-sync UPDATE (`chunk.rs:555-597`), and `upsert_and_mint_slug`, is its own WAL commit. The stamp loop commits once per artifact (`src/librarian/tools/reindex.rs:583-597`). No `synchronous` pragma is set under `src/librarian` (grep 2026-10-30), so SQLite's default `FULL` fsyncs every WAL commit.

## Class fit

`cluster/unclassified`. What the case shows: a function that documents a precondition ("the caller should open a transaction") that no caller meets, so the cost lands silently.

## Symptom (Effect)

Not measured in isolation. Disk fsync probe (2026-10-24 record): p50 15 ms, max 0.7 s. `doc update` avg 38 ms (855 calls), `doc move` avg 273 ms (170 calls).

## Reproduction

`strace -f -c -e fsync,fdatasync` during a reindex that touches one large tracker.

## Environment

Catalog `~/.local/share/librarian/catalog.db`, WAL, `busy_timeout=5000`.

## Root cause

inferred from `src/librarian/catalog/chunk.rs:395-410` and `src/librarian/indexer.rs` — not measured.

## Evidence

Code read 2026-10-30 by the librarian explorer of this session.

## Hypotheses tried

None.

## Fix

Part 1 done (2026-10-30): `embed_queue_items` (`src/librarian/indexer.rs`) calls `replace_chunks` through `in_one_commit`, a SAVEPOINT released on `Ok` and rolled back on `Err`, and so does the backfill rollback that passes `&[]`. A SAVEPOINT rather than `unchecked_transaction()` so it nests inside a caller's transaction. Upsert plus chunks in one commit per artifact was not done: the upsert stays its own commit.

Measured 2026-10-30 on a file-backed WAL catalog, one artifact of 400 chunks (temporary test, removed):

| write | bare | in one commit |
|---|---|---|
| first write, 400 inserts | 4,691 ms | 12 ms |
| rewrite after inserting a section at the top, every chunk shifted | 8,592 ms | 14 ms |

Part 2 (`PRAGMA synchronous = NORMAL`) is still an owner decision and was not taken.

## Tests added

`indexer::tests::a_chunk_write_that_fails_part_way_leaves_the_stored_chunks_as_they_were` (red when `in_one_commit` is bypassed: verified by mutation) and `a_chunk_write_inside_an_open_transaction_nests_and_follows_its_outcome`. No test counts commits; the rusqlite `hooks` feature is not enabled.

## Workarounds

None.

## Resume

Run the strace count above; decide on (2).

## References

- `docs/issues/2026-10-24-catalog-open-takes-the-write-lock-and-fsyncs-trigger-ddl-on-every-open.md`
