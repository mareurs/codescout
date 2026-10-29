---
id: '4012ad02db866d4d'
kind: bug
status: open
title: 'BUG: two concurrent catalog writes (append_entry, allocate_entry_id) fail with "database is locked" when every fsync takes 3 s'
tags:
- cluster/unclassified
---

## Summary

With every `fsync` and `fdatasync` of the test process delayed by 3 s, two concurrent writers on one catalog file fail with `database is locked`. The failure is in the writers, not in `Catalog::open`. 8 of 8 runs failed at HEAD `e312bdc20` (2026-10-29). Whether a real host produces this is not established.

## Class fit

No existing class names this one: it is a write-contention limit under slow storage, not a wrong value or a stale state. I use the sanctioned `cluster/unclassified` escape hatch rather than force a fit. It differs from the sibling bug on `Catalog::open` because the lock here is taken by a real write that the operation needs.

## Symptom (Effect)

`append_entry` and `allocate_entry_id` return `Err(database is locked)` (SQLite error code 5) from the worker thread. The test then panics at the `.unwrap()` of that call.

## Reproduction

Script: build the lib test binary, then run each test 4 times as

```
strace -f -o /dev/null -e trace=fsync,fdatasync -e inject=fsync,fdatasync:delay_enter=3000000 \
  <lib test binary> librarian::catalog::augmentation::tests::<test> --exact --test-threads=1
```

Measured 2026-10-29 at HEAD `e312bdc20` (the tree that holds `e2c0b85c3`, where an open of an up-to-date catalog takes no write lock):

| test | runs failed | seconds per run | panic site |
|---|---|---|---|
| `append_entry_serializes_across_independent_connections_to_same_file` | 4 of 4 | 124-133 | `augmentation.rs:5177` (2 runs), `5190` (2 runs): the `append_entry(..).unwrap()` of the two threads |
| `allocate_entry_id_serializes_across_independent_connections` | 4 of 4 | 121-130 | `augmentation.rs:5512` (1 run), `5516` (3 runs): the `allocate_entry_id(..).unwrap()` of the two threads |

All 16 `database is locked` lines come from those sites. No run showed `applying schema`.

## Environment

Linux, strace 7.2, SQLite through rusqlite, `busy_timeout = 5000` ms, WAL journal mode.

## Root cause

Not established. What was read: both tests open their two connections on the main thread first, then release the two writers with a `Barrier(2)`. So the failing statement is a write inside the operation. A plausible mechanism is that one writer holds the write lock across several 3 s fsyncs, and the second writer waits longer than 5 s. Nothing here measured the lock hold time or the number of fsyncs per append, so this is a hypothesis.

## Evidence

- Panic sites above, from `@bg_00000015` of this session (script `scratchpad/inject.sh`).
- Contradiction to resolve: `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md` records both reworked tests as passing 4 of 4 under the same 3 s injection. At HEAD they fail 8 of 8. The cause of the difference is not known: the tree, the command or the host may differ.

## Hypotheses tried

None yet.

## Fix

None. Candidates, none tried: a longer `busy_timeout` for appends, a single write transaction per append, or a cross-process lock taken before the write.

## Tests added

N/A: this record reports a reproduction. The two existing tests are the reproduction when run under injection; they pass without it.

## Workarounds

None needed for the normal test run: without injection the tests pass.

## Resume

1. Find why the 2026-10-18 record shows 4 of 4 passes: run the tree at that record's commit under the same injection.
2. Count fsyncs and lock hold time of one `append_entry` (`strace -T`).
3. Decide whether 5 s is a design limit for a catalog on slow storage, and what to do for two real servers.

## References

- `docs/issues/2026-10-24-catalog-open-takes-the-write-lock-and-fsyncs-trigger-ddl-on-every-open.md`
- `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md`
