---
id: '4012ad02db866d4d'
kind: bug
status: mitigated
title: 'BUG: two concurrent catalog writes (append_entry, allocate_entry_id) fail with "database is locked" when every fsync takes 3 s'
tags:
- cluster/unclassified
closed: 2026-10-29
opened: 2026-10-29
unverified: 'Not established: that the cold-start lock hold never fails on a real disk or a real server pair. The failure was reproduced only with 3 s of injected fsync latency. The operator accepted the product behaviour on 2026-10-29; no product code changed. A longer busy_timeout was not tried.'
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

Not established. What was read: both tests open their two connections on the main thread first, then release the two writers with a `Barrier(2)`. So the failing statement is a write inside the operation. Measured 2026-10-29 (fsync targets per thread, `strace -f -y`, no injection; then one mutation run under injection). Both tests drop their only connection at the end of the setup block, and SQLite deletes the WAL file when the last connection closes. The first write after that recreates the WAL. That write costs three fsyncs inside its transaction: `fsync(wal)`, `fsync(<directory>)`, `fsync(wal)`. A write into an existing WAL costs one. At `e34ff991f` the two opens ran trigger DDL on the main thread, so they recreated the WAL before the race; each append then cost one fsync (3 s injected, under the 5 s `busy_timeout`). At `d7d29171f` and later the opens write nothing, so the first append recreates the WAL: 3 fsyncs, 9 s injected, and the second append's 5 s `busy_timeout` expires. Mutation check: at `a3290718c` one added line after the two opens (`art_upsert(&cat1, &sample_art("warm"))`, which writes and so creates the WAL) made the injected test pass (rc=0, 149 s), where the unmutated test fails 8 of 8 on the same tree. That mutation run is a single run. A second finding, same day: warming only `cat1` made `append_entry_serializes_across_independent_connections_to_same_file` pass 2 of 2 but `allocate_entry_id_serializes_across_independent_connections` still failed 2 of 2 (`database is locked` at the `allocate_entry_id` call). The trace of that test showed `fsync(wal)` then `fsync(<directory>)` in the worker thread, the same pair that the second connection's DDL ran on the main thread at `e34ff991f`, so a connection's own first write seems to pay a directory fsync as well. Warming both connections made both tests pass 2 of 2 each under injection (4 of 4 in all). The per-connection directory fsync was read from traces, not from SQLite's source.

Two-process experiment, 2026-10-29 (throwaway `#[ignore]` probe test in a scratch worktree at `4f908aeb8`, not committed; driver `scratchpad/cold-start.sh`). One process opens the catalog, writes one row and exits, which leaves no `-wal` or `-shm` file (checked). Then two fresh processes open the catalog and write one row each, started together. Results: with no injection, 3 of 3 repeats passed for both processes in under a second. With 3 s injected per fsync, 3 of 3 repeats had one process fail with `database is locked` at its first write (`art_upsert`), 15-16 s per repeat, and the other succeed. With a third process holding the catalog open before the writers started (it creates the `-wal` file, checked: 1 file), 3 of 3 repeats still had one process fail the same way, 11 s per repeat. So a long-lived keeper connection did not remove the failure in this setup. The WAL file existed, so the failing cost is not only the WAL recreation; a new connection's own first write is enough, which fits the directory-fsync observation above but was not traced in this experiment. The server holds one connection for its process life (`ToolContext.catalog` behind a global augmentation guard, `src/librarian/adapter.rs`), and each process is a new connection, so this exposure is about processes that start and write at the same moment. Not measured: a real disk, a real server pair, or how often two servers write their first row within the same few seconds. Not measured: the lock hold of a fresh-WAL write on a real host without injection, and whether two real servers ever hit a fresh-WAL write while another waits.

## Evidence

- Panic sites above, from `@bg_00000015` of this session (script `scratchpad/inject.sh`).
- Contradiction to resolve: `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md` records both reworked tests as passing 4 of 4 under the same 3 s injection. At HEAD they fail 8 of 8. Bisect (2026-10-29, same script and injection, `append_entry_serializes_across_independent_connections_to_same_file`): `982a9403e` passed 4 of 4 (159-160 s) and `allocate_entry_id_serializes_across_independent_connections` 4 of 4 (141-142 s); `e34ff991f` passed 1 of 1 (160 s); `d7d29171f` failed 1 of 1 (123 s, `database is locked` at `augmentation.rs:5177`); HEAD `e312bdc20` failed 8 of 8. So the outcome flips at `d7d29171f` (`audit::install_in_txn` skips the trigger reinstall when the stored definitions match). The 2026-10-18 record is consistent with its own tree. The runs at the two boundary commits are single runs. Why the flip happens is not measured. One uninjected run per commit counted fsync/fdatasync calls per thread (`strace -f -tt -e trace=fsync,fdatasync`, `scratchpad/count-fsync.sh`): at `e34ff991f` the main thread made 50 and the two append threads 4 and 2 (6 together); at `d7d29171f` the main thread made 40 and the append threads 5 and 4 (9 together). So the opens got cheaper as intended, and the appends made 3 more fsyncs in total. The split between the two threads varies by run, one run is not a distribution, and which statements make those fsyncs was not read. A passing run is 35 s slower than a failing one, which fits the old opens spending their time in trigger DDL fsyncs, but nothing here shows that this is what protected the appends.

## Hypotheses tried

None yet.

## Fix

Test-side only, 2026-10-29: both tests now write once on each of the two connections (`art_upsert` of a throwaway artifact) before the barrier, so the race measures two appends and not the cold starts. Verified under the 3 s injection: 2 of 2 and 2 of 2 passes on the changed tree, and the full lib suite passed on a clean tree with the change applied (8022 passed, 0 failed). The product side is untouched: a first writer after the last connection closed, or a connection's first write, holds the write lock for two or three fsyncs. Candidates for that: a longer `busy_timeout`, or accepting the cold start. Tried and not effective in the two-process experiment: keeping a connection open so the WAL persists (3 of 3 repeats still failed).

## Tests added

No new test. The two existing tests in `src/librarian/catalog/augmentation.rs` changed (a warm-up write per connection). The injection run is not part of the suite: it needs `strace`, takes about 2.5 minutes per test, and its script lives only in this session's scratchpad. The product behaviour (cold-start lock hold) has no test.

## Workarounds

None needed for the normal test run: without injection the tests pass.

## Resume

0. Decided 2026-10-29 by the operator: accept the product behaviour. The tests were fixed (commit `4f908aeb8`); no product change, no keeper connection (it did not help), no longer `busy_timeout`. Reopen if a real `database is locked` appears at a first write outside injection.
1. (Done, see 0) Decide what the test should do: the flip is explained (fresh-WAL write inside the race), so either warm the WAL in setup, as the mutation did, or state that a first write after the last connection closed is slow by design. Repeat the mutation run for more than one run. The product question stays open: a first writer after an idle period holds the write lock for three fsyncs.
2. Count fsyncs and lock hold time of one `append_entry` (`strace -T`).
3. Decide whether 5 s is a design limit for a catalog on slow storage, and what to do for two real servers.

## References

- `docs/issues/2026-10-24-catalog-open-takes-the-write-lock-and-fsyncs-trigger-ddl-on-every-open.md`
- `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md`
