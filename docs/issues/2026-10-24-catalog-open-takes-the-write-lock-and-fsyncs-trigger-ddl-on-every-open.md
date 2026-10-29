---
status: open
opened: 2026-10-24
closed:
severity: low
owner: marius
related:
- docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md
tags:
- cluster/unclassified
kind: bug
unverified: 'Not established: that the two 2026-10-18 sightings were this busy_timeout expiring. The mechanism was reproduced only with fsync latency injected by strace; no real-process failure of two codescout servers opening one catalog has been observed, and what a server does after a failed open was not read.'
---

# BUG: `Catalog::open` takes the write lock and commits fsync'd trigger DDL on every open, so two concurrent opens of one catalog can fail with "database is locked" when a commit stalls

## Summary

Opening a catalog that is already fully initialised still writes. `Catalog::open` (`src/librarian/catalog/mod.rs:648`) runs the schema script, then the migrations under `BEGIN IMMEDIATE`, then drops and recreates three audit triggers per audited table under another `BEGIN IMMEDIATE`. Two openers of one file therefore queue on SQLite's single write lock, each for the length of the other's commits. The connection waits at most `busy_timeout = 5000` ms. When the holder's commits (each fsync'd) take long enough, the waiting opener gets `applying schema: database is locked (Error code 5)` and `open` fails.

This was found while fixing the two test flakes in `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md`. Those tests were changed to open their connections before the race. The product behaviour below was left as it is, pending an owner decision.

## Class fit

No class on the roster was walked one by one, so the sanctioned `cluster/unclassified` is used. What the case shows: a start-up step that looks like a read of existing state (opening a catalog) does durable writes, so its cost and its failure mode depend on whoever else holds the lock at that moment.

## Symptom (Effect)

```
called `Result::unwrap()` on an `Err` value: applying schema

Caused by:
    0: database is locked
    1: Error code 5: database is locked
```

raised from `Catalog::open`, in the `execute_batch(SCHEMA_SQL)` step, for the second of two openers of the same file.

## Reproduction

Reproduced with injected fsync latency (strace 7.2, every `fsync` and `fdatasync` of the test process delayed by 3 s), using the test binary from this tree before the test change:

```
strace -f -o /dev/null -e trace=fsync,fdatasync -e inject=fsync,fdatasync:delay_enter=3000000 \
  <lib test binary> librarian::catalog::augmentation::tests::append_entry_serializes_across_independent_connections_to_same_file --exact --test-threads=1
```

- `append_entry_serializes_across_independent_connections_to_same_file`: 0 of 4 passed, all 4 with the signature above, panicking at the `Catalog::open(..).unwrap()` inside the spawned threads.
- `allocate_entry_id_serializes_across_independent_connections`: 0 of 4 passed, same signature.
- With 1 s of injected latency the first test passed (1 run). Without injection it passed 360 of 360 runs under about 128 CPU burners and the host's own load. CPU load alone did not reproduce it.
- One uninjected run of that test showed 45 fsyncs for the first open of a fresh file and 5 and 6 fsyncs in the two worker threads (each one an open of the existing file plus one append). The split between open and append was not measured.

## Environment

Linux, `~/.cache/cc-tmp` as TMPDIR, branch `fix/e-20261024` (base `experiments` 30f3e728d), debug test build. A plain fsync probe on that disk, under the host's ambient load, gave p50 15 ms and a maximum of 0.7 s over 200 calls.

## Root cause

Read at the bytes in this tree:

- `src/librarian/catalog/schema.sql:72, 132, 147, 150`: four `INSERT OR IGNORE INTO schema_version` statements, run by `execute_batch(SCHEMA_SQL)` in autocommit, each one needing the write lock.
- `src/librarian/catalog/mod.rs:143`: `run_migrations` starts `BEGIN IMMEDIATE`, so a second opener blocks there too.
- `src/librarian/catalog/audit/mod.rs:189-201` (`install`): `BEGIN IMMEDIATE`, then `install_in_txn` drops and recreates `audit_<table>_insert`, `_update` and `_delete` for every table in `AUDITED_TABLES`, then `COMMIT`. This changes `sqlite_master` on every open, so the commit writes pages and syncs.
- `Catalog::open` sets `foreign_keys`, `journal_mode = WAL` and `busy_timeout = 5000` (`mod.rs:659`). No `synchronous` pragma is set anywhere under `src/librarian`, so SQLite's default (`FULL`) applies and each WAL commit is fsync'd.

So the write-lock hold time of one open includes at least the fsync of the trigger-install commit, and a second opener waits for it. Not measured: how long the real hold is on an uncontended host, and what total host stall produces the failure without injection.

## Evidence

The injection runs in Reproduction. Code read as listed above. The two original sightings (2026-10-18, whole `cargo test --lib` runs of 426 s and 1363 s) match the signature, but nothing was recorded that shows the 5 s timeout expiring in them.

## Hypotheses tried

1. **Hypothesis:** a slow commit by one opener makes the other time out. **Test:** fsync delay injected with strace. **Verdict:** supported for the signature (8 of 8 baseline runs failed with it); not shown to be what happened in the two sightings.
2. **Hypothesis:** CPU load alone is enough. **Test:** 360 runs of the two tests under about 128 CPU burners. **Verdict:** rejected for that load (0 failures).

## Fix

Option 1 is implemented (status stays `open`: the lock is still taken by `schema.sql` and `run_migrations`, and the real failure was not re-tested).

`audit::install_in_txn` (`src/librarian/catalog/audit/mod.rs`) now builds the three `CREATE TRIGGER` texts per audited table (without the trailing `;`, which is how SQLite stores them in `sqlite_master.sql`) and compares each with the stored `sql`. Only when all three are byte-equal does it skip the table; any difference or a missing trigger takes the old drop+create path, so a changed definition, a column add or a lost trigger still converges. An open of an up-to-date catalog therefore runs no trigger DDL.

Not done: the four `INSERT OR IGNORE INTO schema_version` statements in `schema.sql` still run on every open and still take the write lock (no rows written, so no fsync of a data change, but the lock is taken). `run_migrations` still starts `BEGIN IMMEDIATE`. Options 2 and 3 unchanged.

Verified (2026-10-29):
- Test-first: `install_on_up_to_date_triggers_runs_no_ddl` failed on the old code (`PRAGMA schema_version` 64 -> 106 across a second `install`), then passed after the change.
- `scripts/with-slot.sh cargo test --lib librarian::`: 2466 passed, 0 failed, 2 ignored.

Not verified: that this removes the real "database is locked" failure under injected fsync latency (the strace reproduction was not re-run), and the lock hold time of `schema.sql` / `run_migrations` on a real open.

## Tests added

In `src/librarian/catalog/audit/mod.rs` tests: `install_on_up_to_date_triggers_runs_no_ddl` (schema cookie unchanged by a second `install`) and `install_converges_a_changed_or_missing_trigger` (a stale body and a dropped trigger are restored to the generated definitions). No test covers two concurrent `Catalog::open` calls on an initialised file.

## Workarounds

None needed for the tests after the rework. For a real pair of servers: not known.

## Resume

1. Measure the write-lock hold time of one `Catalog::open` on an existing file, uncontended and under host load (for example by timing `BEGIN IMMEDIATE` to `COMMIT` in `audit::install`).
2. Decide between option 1 and leaving the behaviour documented.
3. If option 1: add a test with the strace-style latency or an injected slow commit that fails on the current code.

## References

- `src/librarian/catalog/mod.rs` (`Catalog::open`, `run_migrations`)
- `src/librarian/catalog/audit/mod.rs` (`install`, `install_in_txn`)
- `src/librarian/catalog/schema.sql`
- `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md`
