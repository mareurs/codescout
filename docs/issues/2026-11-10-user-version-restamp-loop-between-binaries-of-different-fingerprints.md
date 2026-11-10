---
id: '1a64131278d421ce'
kind: bug
status: mitigated
title: 'BUG: binaries of different err_family fingerprints share one usage.db and re-run the backfill on every flip'
owners:
- marius
tags:
- cluster/unclassified
closed: 2026-11-11
opened: 2026-11-10
unverified: Timings are from a copy of the live usage.db with pages evicted by posix_fadvise, not from a live flip between binaries under contention. The number of flips is unchanged. Whether the cheaper flips remove the dropped rows that coincided with the slow statements (bug f38c0205eaef0829) was not measured.
---

# BUG: binaries of different err_family fingerprints share one usage.db and re-run the backfill on every flip

## Summary
`backfill_legacy_rows` runs when `PRAGMA user_version` differs from the binary's own `err_family_fingerprint()`, then stamps its own value. Several `codescout start` processes on different builds share one `usage.db`, so each flip between two fingerprints re-runs the backfill: a full-table `UPDATE` scan plus a `SELECT` of unclassified rows. The design note says the re-run costs "one pass"; that holds for one binary per file, not for a file shared by builds with different fingerprints.

## Class fit
The gate compares a stored value for equality with a value that every writer computes for itself, so two writers that disagree overwrite each other forever and each overwrite costs a table scan. No existing class names a shared stamp that has more than one owner. Filed unclassified until a second instance shows the pattern.

## Symptom (Effect)
`usage-lock-holds.log` records each backfill that ran, with the value it read and the value it wrote. Twelve lines exist on 2026-11-09 and 2026-11-10 (UTC), all with `to=676504337`:

```
2026-11-09 20:53:04.449  4185695  510500039  backfill  2346  from=1965217055 to=676504337 update_scan_ms=2207
2026-11-09 20:53:04.450  4185695  510500039  backfill  2487  from=1965217055 to=676504337 update_scan_ms=2314
2026-11-09 21:38:44.527  1752622  f34fbc8b3  backfill  6359  from=1965217055 to=676504337 update_scan_ms=3328
2026-11-09 21:44:06.567  1686255  f34fbc8b3  backfill  2758  from=1965217055 to=676504337 update_scan_ms=2646
2026-11-09 21:44:08.799  1719246  f34fbc8b3  backfill   701  from=1965217055 to=676504337 update_scan_ms=506
2026-11-10 10:53:48.976  2722714  fd7f7a029  backfill   429  from=1877607761 to=676504337 update_scan_ms=314
2026-11-10 11:02:49.262  1694535  fd7f7a029  backfill   502  from=1877607761 to=676504337 update_scan_ms=410
2026-11-10 11:03:24.344  1694535  fd7f7a029  backfill   359  from=1877607761 to=676504337 update_scan_ms=97
2026-11-10 11:04:50.607  2829334  5b40efc48  backfill   802  from=1877607761 to=676504337 update_scan_ms=701
2026-11-10 11:04:50.624  2829334  5b40efc48  backfill   416  from=1877607761 to=676504337 update_scan_ms=187
2026-11-10 11:04:50.629  2829334  5b40efc48  backfill   515  from=1877607761 to=676504337 update_scan_ms=401
2026-11-10 11:14:46.189  78238    510500039  backfill  1474  from=1877607761 to=676504337 update_scan_ms=1387
```

Each line is one run on a file whose stamp was not the current build's fingerprint. Durations run from 359 ms to 6359 ms. The 6359 ms run (pid 1752622, 21:38:44) comes six seconds after a dropped row from the same pid at 21:38:38 (bug f38c0205eaef0829).

## Reproduction
Not yet reproducible on demand. Best lead: start two `codescout start` processes on builds whose `ERR_FAMILIES` differ against one project, call any tool on each in turn, and read `PRAGMA user_version` between calls. Each call after a flip should log one `backfill` line. Not run.

## Environment
Linux, about 49 `codescout start` processes sharing `.codescout/usage.db` of this checkout (295 MB), most on deleted binaries. Branch `experiments`. Log clock is UTC; local is UTC+2.

## Root cause
Mechanism, read from `src/usage/db.rs:1193-1235` this session: `backfill_legacy_rows` reads `PRAGMA user_version`, returns at once when it equals `err_family_fingerprint()`, and otherwise runs `UPDATE tool_calls SET project_root = ?1 WHERE project_root IS NULL`, a `SELECT` of rows with `err_family IS NULL AND error_msg IS NOT NULL`, and `PRAGMA user_version = {fingerprint}`. The doc comment at `src/usage/db.rs:1169-1172` states that the gate compares for equality because a fingerprint is not ordered, and that a database with another value "re-runs the backfill once and is then stamped".

Mechanism, inferred and not proven: when binary A stamps its value and binary B (another fingerprint) opens the file afterwards, B finds a value that is not its own and re-runs the backfill. The pair of lines for pid 1694535 fits this: at 11:02:49 and again at 11:03:24 the process read `from=1877607761` after it had stamped 676504337, so another binary wrote 1877607761 in between. Old binaries write no lock-hold log, so their flips leave no line.

Measured 2026-11-10: `PRAGMA user_version` sampled once a minute for 61 minutes on 2026-11-09 gave 52 samples `676504337`, 2 samples `1965217055` and 7 samples `1877607761` (bug f38c0205eaef0829, hypothesis 10). Three fingerprints were alive at once. `676504337` is the fingerprint of the checked-out code on 2026-11-09.

Same-process repetition: pid 2829334 ran three backfills within 22 ms at 11:04:50 and pid 4185695 two within 1 ms at 20:53:04. A process opens the database on every row write, so concurrent writes in one process each pay the gate.

## Evidence
- `.codescout/usage-lock-holds.log`, the twelve `backfill` lines above (`grep` for the tab-separated kind `backfill`).
- `PRAGMA user_version` samples: `docs/issues/2026-11-09-concurrent-pinned-calls-leave-no-usage-row.md`, hypothesis 10.
- `src/usage/db.rs:1169-1172` and `src/usage/db.rs:1193-1235`.

## Hypotheses tried
1. **Hypothesis:** the re-run costs one pass per database, as the design note says. **Verdict:** holds for one binary per file; contradicted by the twelve lines above for a file shared by three fingerprints.

## Fix
Cost mitigated 2026-11-11; the flips are NOT removed. `open_db` (`src/usage/db.rs`) now creates two partial indexes on every open, right before `backfill_legacy_rows`: `idx_tool_calls_unrooted ON tool_calls(id) WHERE project_root IS NULL` and `idx_tool_calls_unclassified ON tool_calls(id) WHERE err_family IS NULL AND error_msg IS NOT NULL`. Each backfill statement now finds "no work" from an empty index and does not read the table. The two statements are named constants (`BACKFILL_FILL_ROOT_SQL`, `BACKFILL_UNCLASSIFIED_SQL`) so the test plans the SQL that runs. A file created by a build without the indexes gets them on its next open, at the price of one scan to build the first.

Measured on a copy of the live `usage.db` (295,079,936 bytes, 120,562 rows, 0 rows with a NULL `project_root`, 172 error rows the classifier cannot classify, stamp 676504337), pages evicted with `posix_fadvise(DONTNEED)`, statements rolled back:

| Statement | Before | After |
|---|---|---|
| `UPDATE ... WHERE project_root IS NULL`, cold | 2.696, 2.753, 2.480 s; plan `SCAN tool_calls` | 0.442, 0.434, 0.471 ms; plan `SCAN tool_calls USING INDEX idx_tool_calls_unrooted` |
| same, warm | 57 and 56 ms | not measured |
| `SELECT ... err_family IS NULL ...`, cold | `SCAN tool_calls` | 0.03 ms, the same 172 rows, plan `USING INDEX idx_tool_calls_unclassified` |
| building the indexes, one time | | 2.4 s for the first (one cold scan), 0.07 s for the second |

The cold `UPDATE` matches the 2.2 to 3.3 s `update_scan_ms` values in `usage-lock-holds.log`, which the 55 ms warm figure does not. So the logged stalls are cold-page reads of the whole file, and the `UPDATE` holds the write lock while it reads.

NOT taken, by choice among the candidates below: a stored set of fingerprints, and a once-per-process guard. They would stop the flips; this change only makes each flip cheap. Any change to the gate must keep the property from the archived fix: a changed taxonomy re-classifies old rows.

Candidates (the first two are not taken):
- Stamp a set of fingerprints that have run, or a taxonomy version that only grows, so a flip back to an older binary does not re-run the backfill.
- Run the backfill once per process before the first write, not on every `open_db` call, so concurrent writes in one process do not each pay it.
- Make the scan cheap when it matches no rows (an index, or a one-row `EXISTS` probe before the `UPDATE`).
Any change to the gate must keep the property from the archived fix: a changed taxonomy re-classifies old rows.

## Tests added
`usage::db::tests::the_backfill_scans_are_served_by_partial_indexes` runs `EXPLAIN QUERY PLAN` on both backfill statements and requires an index in each plan, on a fresh database and again after the two indexes are dropped and the file is re-opened. Watched RED before the indexes (`plan: SCAN tool_calls`), GREEN after. Its second half (drop and re-open) was not run red on its own: the first assertion failed before it was reached. The existing gate tests `backfill_reruns_when_the_stored_marker_does_not_match` and `backfill_fills_project_root_and_err_family_once` pass before and after. `cargo test --lib usage::` 113 passed. The test checks plans, not time.

## Workarounds
Run every server on the same build, so the stamp is written once. Restart the servers that run older binaries.

## Resume
Cost mitigated 2026-11-11 (see Fix); the stamp still flips between builds. Still open, measure not fix: on the next day of work, read `usage-lock-holds.log` for `backfill` lines, count them per day and per pid, and compare with the number of distinct builds in `usage-dropped-rows.log` and `usage-lock-holds.log`. Then pick a candidate above. Cite `src/usage/db.rs` for the gate.

## References
- `docs/issues/2026-11-09-concurrent-pinned-calls-leave-no-usage-row.md` (bug f38c0205eaef0829): the dropped rows that coincide with slow statements on this file.
- `docs/issues/archive/2026-08-16-backfill-gate-not-derived-from-the-taxonomy.md`: the fix that introduced the fingerprint gate and the "one pass" claim.


## Fix provenance

- **SHA:** `e31973932cf0bcb78a2136dd377b9821c7638569` (`experiments`)
- **patch-id:** `84df7758bf66f1d048257619d06ecc6094b9714f`

This commit mitigates the cost of each flip. It does not remove the flips; see Fix.
