---
id: '2d7ef39a4166e5c6'
kind: bug
status: open
title: Librarian reindex writes each chunk row as its own fsynced commit, and no catalog connection sets synchronous
owners:
- marius
tags:
- cluster/unclassified
opened: 2026-10-30
severity: medium
unverified: The fsync count per reindexed artifact and the saving are not measured; the mechanism is read from code.
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

Two separate decisions:
1. Wrap each artifact's upsert + `replace_chunks` in one `unchecked_transaction` (per artifact, so the write lock stays short).
2. `PRAGMA synchronous = NORMAL` under WAL: no corruption risk, but a power loss or OS crash can drop the last commits; augmentations are not in git. **Owner decision required.** It was not tried in `docs/issues/2026-10-29-two-concurrent-catalog-writes-exceed-the-5-s-busy-timeout-when-fsync-takes-3-s.md`.

## Tests added

None yet.

## Workarounds

None.

## Resume

Run the strace count above; decide on (2).

## References

- `docs/issues/2026-10-24-catalog-open-takes-the-write-lock-and-fsyncs-trigger-ddl-on-every-open.md`
