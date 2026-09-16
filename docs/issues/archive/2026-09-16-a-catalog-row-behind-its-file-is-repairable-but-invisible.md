---
id: 794db556f3cfdf93
kind: bug
status: fixed
title: A catalog row left behind by a failed update is repairable but invisible
tags:
- librarian
- catalog
- architecture
- cluster/selector-narrower-than-its-population
---

# BUG: a catalog row that has fallen behind its file is repairable but invisible

## Summary

`doc(action="update")` writes the file first and the catalog second. When the
catalog half fails, the row keeps the pre-edit `file_sha256` while the file on
disk carries the edit. That state is **reachable**, **repairable by `reindex`**,
and **reported by nothing**.

`librarian(action="doctor")` does not name the artifact. Its catalog-health family
checks path form, `..` segments, ADS colons and `missing_file` — a row whose file
is *gone*. No check compares the stored `file_sha256` against the bytes on disk, so
a row whose file *changed* is outside the population the scan examines, and a clean
report reads as "catalog healthy" rather than "content divergence not looked at".

The consequence is that the recovery is real but untriggered: the window is not
"until the next reindex", it is "until somebody reindexes for an unrelated reason",
which is not a bound.

## Symptom (Effect)

After a failed `update`, `doc(action="find")` and `doc(action="get")` answer from a
row whose `file_sha256`, `updated_at` and any frontmatter-derived column describe
the previous version of a file that has already changed. Nothing errors and
nothing warns.

## Reproduction

`src/librarian/tools/update::tests`, three tests, each ~5 s (the failure only
arrives when `busy_timeout` expires):

- `a_real_catalog_failure_after_the_file_write_leaves_disk_ahead_of_the_catalog`
- `whether_reindex_reconciles_a_catalog_left_behind_by_a_failed_update`
- `doctor_names_a_catalog_row_that_has_fallen_behind_its_file`

Shape: on-disk catalog, second `rusqlite` connection takes `BEGIN IMMEDIATE` and
holds it, `update` with a body patch, observe.

## Environment

codescout `experiments`, 2026-09-16. Gate green all four lanes.

## Root cause

Two independent facts, and the defect is their join.

1. **Ordering.** `src/librarian/tools/update.rs` writes the file at `:688` and
   upserts the row at `:725`. The window between them is real and the code says
   so — `file_written_but_catalog_failed` exists precisely to name both halves to
   the caller. Its sibling `create.rs` orders the other way on purpose (catalog
   first, disk last, BUG-058) so the same failure leaves no file at all. Both
   orderings are defensible; they are opposite.

2. **Observability.** `doctor`'s scan has no predicate over stored-hash versus
   on-disk bytes. `missing_file` is adjacent and does not cover it: a changed file
   is present.

The caller who receives the error is told exactly what happened and can act. The
caller who was never there — a later session reading the catalog — has no signal
at all, and that is the party the recovery depends on.

## Evidence

Measured, with the control that makes the silence a measurement rather than a
dead scan: the same fixture is then broken a second, known-detectable way (the
file is deleted, which `missing_file` owns) and `doctor` **does** name the
artifact. So doctor is live, the question is simply not in its population.

`reindex` reconciles: after the failed update the row's hash catches up to disk,
asserted against a freshly computed `sha_of_bytes` of the file rather than against
a constant.

## Hypotheses tried

1. **Hypothesis:** the divergence is unreachable in practice — the upsert cannot
   realistically fail after the file write. **Test:** real lock contention, a
   second connection holding `BEGIN IMMEDIATE` past the 5 s `busy_timeout`.
   **Verdict:** rejected; it is reachable and the production comment at `:719`
   already predicted that only real contention would exercise it.
2. **Hypothesis:** the divergence is durable, so slice 4 needs rollback
   machinery. **Test:** run `reindex` after releasing the lock. **Verdict:**
   rejected — the row reconciles. The hazard is transient *given a trigger*.
3. **Hypothesis:** `doctor` already reports it, so the trigger exists.
   **Test:** run `doctor` against the diverged state, with the deleted-file
   control. **Verdict:** confirmed as the defect — doctor is silent on the
   divergence and loud on the deletion.

## Fix

Fixed by `6fab2977`, patch-id `1a3ef8e279706c0d701077dcb82d2f2e7b16bce7`.

`doctor` gained `row_behind_file`: it compares each row's stored `file_sha256`
against the bytes on disk and names the rows that differ, with a detail line that
names `reindex` as the repair. The write path is untouched, which was the point —
the recovery already existed and nothing triggered it.

**Reordering `update` to match `create` was the expensive alternative and is still
the wrong one**, for the reason this file gave before the fix: `create` can order
catalog-first because it has nothing to lose if the file never lands, while `update`
has a caller holding a patch it must not blindly re-apply — which is the whole
reason `file_written_but_catalog_failed` says so. Both orderings are defensible and
they are opposite; consolidating them would have destroyed information.

**The `file_mtime` pre-filter this section used to call "the obvious narrowing" was
NOT built, and the measurement is why.** Against the live catalog: hashing all
4,943 files (83.2 MB) costs **115 ms**, against 25 ms for the filtered arm. The
narrowing buys 90 ms for a selector that `git checkout`, `touch -r`,
`rsync --times` and any restore-from-backup defeat, each preserving mtime across a
content change. Its false-negative count in that sample was **0**, and that zero is
deliberately not the argument — citing it would be the population-for-member
substitution. Not building it avoids this file's own cluster rather than managing
it. Re-derive both arms before reintroducing one;
`architecture-boundary-session-log:F-3` holds the method.

**Two things the check may not claim, both decided by measurement.** It is
scope-gated (`ROW_GRAIN_SCOPED_CHECKS`) because divergence is 124 of 4,944 rows
globally but 6 of 1,712 inside codescout — unscoped it publishes a worklist that is
95% another developer's. And it is named for the state rather than for this bug: a
failed `update` and a write that never reaches the catalog at all (`edit_file`,
native `Edit`, `git checkout`) produce the identical row, the corpus is dominated
by the second, and `reindex` repairs both. `failed_update_divergence` would have
published a value correct in one frame under a name asserting another (`IC-24`).
## Tests added

The three named under *Reproduction*, in `src/librarian/tools/update.rs`'s test
module — placed there because the production code carried the comment *"NOT REACHED
BY ANY UNIT TEST … needs real lock contention … which no test in this suite
constructs"*. That comment was false once they existed and was corrected with them.

The third was written asserting the **observed** outcome — that `doctor` was blind —
with a comment saying it would red on the day that changed. It did, on this fix. It
is renamed `doctor_names_a_catalog_row_that_has_fallen_behind_its_file` and inverted
rather than repaired, and it is now the only test that reaches `row_behind_file`
through a **real** failed catalog write rather than a hand-seeded row. It also
asserts the exclusivity the two checks depend on: a row whose file is *gone* is
reported under `missing_file` alone.

Four more in `src/librarian/tools/doctor.rs` cover the predicate itself — the
agreement/divergence pair with its control running first, both abstentions each
with a non-vacuity companion proving the same fixture *can* fire, and a wired
`call()` test asserting the aggregate **and** naming the diverged row specifically
(`by_check == 1` is satisfied equally by "reported the right row" and "reported
some row").

Four mutations, one per guarded site, all **KILLED** via
`scripts/mutation-probe.sh`: inverted predicate (4 tests), deleted empty-hash
abstention (1), missing-file abstention made to fire (2), and
`Check::RowBehindFile` dropped from `ROW_GRAIN_SCOPED_CHECKS` (2).
## Workarounds

Run `librarian(action="reindex")` after any `doc(action="update")` that returned
`doc(action="update") wrote the file but the catalog record failed`. The error
text already tells the caller not to retry the patch blind; reindex is what
repairs the row.

## Resume

Full derivation, including why this argues against slice 4's rollback framing:
`docs/trackers/architecture-boundary-measurement.md` § *Slice 4 — prerequisite
experiment run 2026-09-16*.
