---
id: 6fce8a96f2e8d6e1
kind: bug
status: fixed
title: Audit trail records the statement, not the change — 98.5% of rows are empty diffs
tags:
- cluster/gate-keyed-on-unobservable-event
closed: 2026-09-01
opened: 2026-09-01
owner: marius
severity: medium
---

## Summary
The audit UPDATE trigger has **no `WHEN` clause** (`src/librarian/catalog/audit.rs`
`install_in_txn`), so it fires on every `UPDATE` statement — including one that writes
identical values. `update_diff_expr` then folds to the literal `'{}'` and a row is written
anyway. Measured on the live catalog over a **1.74-hour** window: **27,505 of 27,914 rows
(98.5%)** are `commits` updates whose payload is exactly `{}`. They carry no forensic
information, and they dilute `seq` — whose gaps the design names as the tamper signal.

This was flagged as deferred "Minor 5" in the T-1 final review and left unfiled; the number
below is the first measurement of it, taken while scoping T-7 (committed shards).

## Symptom (Effect)
No error. `librarian(action="audit_log")` with no filter returns `commits/update/{}` rows
almost exclusively, so the default view of the trail is noise. `table_total` reads 27,914
where the informative population is ~409. For T-7 the same rows would be exported into a
git-committed shard at roughly **380k rows/day**.

## Reproduction
```
sqlite3 -readonly ~/.local/share/librarian/catalog.db \
  "SELECT payload, count(*) FROM catalog_audit
   WHERE tbl='commits' AND op='update' GROUP BY payload ORDER BY 2 DESC LIMIT 5;"
```
→ `{}|27505` — one distinct payload value, 27,505 rows. Any `librarian(action="reindex")`
adds another full sweep of the `commits` table.

## Environment
codescout `experiments` @ `33fb28c9`; audit trail landed at `10972335` (T-1).

## Root cause
`CREATE TRIGGER audit_{name}_update AFTER UPDATE ON "{name}" BEGIN … END` is unconditional.
SQLite supports `AFTER UPDATE … WHEN <expr>`, and the expression needed is already
constructed for the payload: the same `OLD."c" IS NOT NEW."c"` comparisons, OR-folded.
The recorder observes *a statement executed*, which is trivially available, in place of
*a row changed*, which was equally available — the proxy is not forced by the observation
boundary, it was simply not asked for.

Note on classification: filed under `cluster/gate-keyed-on-unobservable-event` for the
proxy substitution, and it sits in that class's "could have looked and did not" half rather
than its harness-scoped half. A reader who thinks the recorder-vs-gate distinction matters
should move it rather than re-file it.

## Evidence
### Composition of `catalog_audit`, 1.74h window, 27,914 rows
| tbl / op | rows | payload chars |
|---|---:|---:|
| `commits` update (payload `{}`) | 27,505 | 55,010 (2 each) |
| `artifact` update | 147 | 30,574 |
| `events` insert | 73 | 0 |
| `commits` insert | 71 | 0 |
| `events` update | 30 | 1,650 |
| `artifact` insert | 28 | 0 |
| `artifact_augmentation` update | 23 | 786,771 |
| `artifact` delete | 19 | 14,051 |
| `artifact_link` update | 18 | 900 |

Of the 147 `artifact` updates, **107** carry exactly `{updated_at, file_mtime, file_sha256}`
— the reindex-churn key set the T-1 spec already names. So after suppressing no-ops and
churn, the informative population over that window is **~279 rows**, or 1.0%.

## Hypotheses tried
None needed — the payload histogram is decisive.

## Fix
Add a `WHEN` guard to the UPDATE trigger built from the same column list
`update_diff_expr` already walks:

```sql
CREATE TRIGGER audit_{name}_update AFTER UPDATE ON "{name}"
WHEN OLD."c1" IS NOT NEW."c1" OR OLD."c2" IS NOT NEW."c2" OR …
BEGIN … END;
```

`IS NOT` (not `<>`) so NULL transitions still count. Zero-column tables cannot occur —
every audited table has columns — but the generated expression must not be empty; if the
fold produces nothing, emit no `WHEN` rather than a syntax error.

**A guard test must be able to fail in the right direction.** The existing suite asserts
rows *exist*, which is monotone under over-recording and therefore blind to this defect —
it passed through all 27,505. The new test must assert an unchanged-value `UPDATE` writes
**no** row, and be paired with one asserting a real change still does.

## Tests added

Fixed on `experiments` at **`40ab56f6`** (patch-id `d3021d83634be0f6b8d7c69200f241f80f9e5f96`).

A `WHEN` clause on the UPDATE trigger, built by `changed_predicate()` from the same column
list `update_diff_expr` already walks. `IS NOT`, never `<>`.

Two tests, deliberately adjacent and deliberately a pair — the suite's existing update
assertions are monotone under over-recording, which is exactly how 27,505 empty rows passed
a green suite for a day:

- `an_update_that_changes_nothing_writes_no_audit_row` — born RED against the old trigger
  (2 rows where 1 was asserted). Also asserts the `UPDATE` really affected a row, so the
  test cannot pass by failing to run its own mutation.
- `an_update_that_changes_one_column_still_writes_a_row` — the other direction; fails if the
  guard is too aggressive.
- `a_null_to_value_transition_counts_as_a_change` — discriminates `IS NOT` from `<>`. Born
  GREEN by design: it guards the regression the fix newly makes possible, since `<>` would
  drop NULL transitions silently in both directions.
## Workarounds
Filter every query: `librarian(action="audit_log", tbl="artifact")`, or prune periodically.

## Resume

Closed. The reinstall-on-every-open convergence means existing catalogs are repaired by the
next `Catalog::open` — no migration, no backfill. Rows already written stay; prune them with
`librarian(action="audit_log", prune_before_ms=…, confirm=true)` if the historic noise is in
the way.

Re-measure after a rebuild to confirm the composition collapsed: the `commits`/`update` line
in the Evidence table should fall to the count of genuine commit-metadata changes.
## References
- `src/librarian/catalog/audit.rs` — `install_in_txn`, `update_diff_expr`
- `docs/superpowers/specs/2026-09-01-catalog-audit-trail-design.md` § Phase 1 Capture
- `docs/trackers/system-retrospective-improvements.md` T-7 (this blocks the shard export)
- Sibling volume bug: `docs/issues/archive/2026-09-01-audit-growth-concentrates-in-augmentation-params-health-blind-to-bytes.md`
