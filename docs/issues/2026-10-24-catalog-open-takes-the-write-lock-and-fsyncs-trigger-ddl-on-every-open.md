---
kind: bug
status: fixed
tags:
- cluster/unclassified
closed: 2026-10-29
opened: 2026-10-24
owner: marius
related:
- docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md
severity: low
unverified: 'Not established: that the two 2026-10-18 sightings were this busy_timeout expiring; nothing recorded them. The mechanism was reproduced only with fsync latency injected by strace. After the fix, every open succeeded in the six injected repeats of the two-process cold-start experiment of 2026-10-29 (3 s per fsync); the failures there were at each process''s first write, which is filed as 4012ad02db866d4d and accepted by the operator. The pre-rework test bodies were not re-run under injection against the fixed code, so there is no same-body before/after. What a server does after a failed open was not read. The commits (e2c0b85c3, d7d29171f) are local, not pushed.'
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

Options 1 and the lock-avoiding read-first guards are implemented (status stays `open`: the injected-fsync reproduction still ends in "database is locked", now from the test's own appends, see below).

Three changes make `Catalog::open` of an up-to-date catalog take no write lock:

1. `audit::install_in_txn` (`src/librarian/catalog/audit/mod.rs`, commit d7d29171f) compares the generated `CREATE TRIGGER` text (no trailing `;`, as SQLite stores it in `sqlite_master.sql`) with the stored `sql` and skips the drop+create when all three triggers of a table match. This round the generation moved into `generated_triggers`, and `install` now calls the read-only `triggers_current` BEFORE `BEGIN IMMEDIATE` and returns early, so the write transaction is not even started. The in-transaction re-check is kept for the racing-opener case.
2. `run_migrations` (`src/librarian/catalog/mod.rs`) calls the read-only `migrations_current` first and, when true, skips `BEGIN IMMEDIATE` and only runs `widen_events_kind_check` (itself a read when the CHECK is already wide). `migrations_current` requires: stamps 4, 5, 9, 10, 11, 12, 13; every column the v4-v12 / v6 `add_columns` blocks add; no legacy `repo` / `rel_path` column left; and tables `entry_cite`, `entry_reservation`, `catalog_meta`, `artifact_chunk`, `artifact_vec_v2`. Any miss runs the old path unchanged. Trade-off: the list mirrors `apply_migrations_in_txn` by hand, so a new migration must be added to `migrations_current` (or its stamp to `MIGRATION_VERSIONS`), otherwise an up-to-date-looking catalog would skip it. The test `every_stamped_version_is_known_to_the_up_to_date_check` is the tripwire: a fresh open runs every migration, so a new stamp missing from `BASELINE_VERSIONS` / `MIGRATION_VERSIONS` fails it (checked 2026-10-29 by removing 13 from the list: it failed with "schema_version 13 is stamped but unknown"). It cannot see a new column or table that carries no new stamp.
3. The four `INSERT OR IGNORE INTO schema_version` (v1, v2, v3, v6) are removed from `schema.sql` (an INSERT takes the write lock even when it ignores the row; `execute_batch` runs the whole script on every open). `apply_schema` now runs `SCHEMA_SQL` (all `CREATE ... IF NOT EXISTS`, which on an existing object is a read) and then `stamp_baseline_versions`, which reads each stamp and inserts only a missing one. Used by `open`, `open_in_memory` and `open_with_workspace`.

Verified (2026-10-29):
- `scripts/with-slot.sh cargo test --lib librarian::`: 2468 passed, 0 failed, 2 ignored (was 2466; +2 new tests).
- `cargo clippy --workspace --all-targets --features local-embed -- -D warnings`: clean. `rustfmt --edition 2021 --check` on both changed `.rs` files: clean.
- Each of the three guards is load-bearing for the new test: with `triggers_current`, `migrations_current` or the stamp read-check disabled one at a time, `open_of_an_initialised_catalog_takes_no_write_lock` failed after 5 s with "database is locked" in `installing audit triggers`, `running migrations` and `applying schema` respectively.
- strace fsync injection (3 s on every fsync/fdatasync) against the fixed lib test binary, `append_entry_serializes_across_independent_connections_to_same_file`: still FAILED (132 s), but the panic is `database is locked` raised at `augmentation.rs:5190` from `append_entry` inside the worker thread, not from `Catalog::open`; the opens at lines 5163-5164 happen on the main thread before the race and succeeded. So the original signature (`applying schema` / `Catalog::open`) did not appear, but this run does not show the failure gone: two real appends, each with several 3 s fsyncs, exceed the 5 s `busy_timeout` by themselves. Later measured (see `docs/issues/2026-10-29-two-concurrent-catalog-writes-exceed-the-5-s-busy-timeout-when-fsync-takes-3-s.md`): the test passed under the same injection before this fix, because the opens then recreated the WAL before the race. Now the opens write nothing, so the first append recreates the WAL (3 fsyncs under the write lock). It is a different cost from the open, and this fix exposed it.

Not verified: the second reproduction test (`allocate_entry_id_serializes_across_independent_connections`) was not re-run under injection; the injected run above used the post-rework test, so it is not a before/after of the same test body; no measurement of real lock hold times.

## Tests added

- `src/librarian/catalog/mod.rs`: `open_of_an_initialised_catalog_takes_no_write_lock`. A third connection holds `BEGIN IMMEDIATE` for the whole race; two threads released by a barrier call `Catalog::open` on the already-initialised file; both must return `Ok`. Deterministic: a lock held throughout means any write or write-lock attempt in `open` waits out the 5 s `busy_timeout` and fails, so pass/fail does not depend on timing. The barrier is only there to make the two opens concurrent. Mutation-checked for each of the three guards (see Fix).
- `src/librarian/catalog/mod.rs`: `open_restores_a_missing_baseline_stamp` (stamps 1, 6, 12 deleted, reopen restores them, so removing the inserts from `schema.sql` did not lose the stamps for older catalogs).
- `src/librarian/catalog/audit/mod.rs` (commit d7d29171f): `install_on_up_to_date_triggers_runs_no_ddl` and `install_converges_a_changed_or_missing_trigger`.

A DDL-free open is not asserted by `PRAGMA data_version`; the held-write-lock test was chosen because it was verified to fail on each guard removed.

## Workarounds

None needed for the tests after the rework. For a real pair of servers: not known.

## Resume

0. Closed 2026-10-29 as `fixed` at the operator's request; see `unverified:` for what is not shown.
1. (Done, see 0) Decide whether to close: the open path no longer writes or locks on an up-to-date catalog. The remaining injected-fsync failure is in `append_entry` and `allocate_entry_id`: 8 of 8 runs failed at HEAD `e312bdc20` on 2026-10-29, none inside `Catalog::open`. It is filed as `docs/issues/2026-10-29-two-concurrent-catalog-writes-exceed-the-5-s-busy-timeout-when-fsync-takes-3-s.md`.
2. Optionally re-run both strace reproductions with the pre-rework test bodies for a true before/after.
3. Keep `migrations_current` in step with `apply_migrations_in_txn` when adding a migration.

## References

- `src/librarian/catalog/mod.rs` (`Catalog::open`, `run_migrations`)
- `src/librarian/catalog/audit/mod.rs` (`install`, `install_in_txn`)
- `src/librarian/catalog/schema.sql`
- `docs/issues/2026-10-18-append-entry-concurrency-test-failed-once-with-database-is-locked-in-catalog-open.md`
