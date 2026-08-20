---
id: 5a11ee0211756e71
kind: bug
status: fixed
title: a calendar-invalid dated Statement is invisible to all three validity checks
tags:
- doctor
- statements
- validity
- false-negative
- silent-skip
closed: 2026-08-20
---

# BUG: a calendar-invalid `dated` Statement is invisible to all three validity checks

## Summary

An entry declaring `**Valid:** dated 2026-02-30` parses successfully, reads as "declared and
healthy", and is then reported by **none** of the three `doctor` validity checks. A single
character typo in a date silently removes a Statement from every worklist, with no error,
no warning and no count moving — which is the exact failure mode the validity feature was
built to detect.

## Symptom (Effect)

Verified empirically 2026-08-20 by a scratch probe in `doctor.rs` (since reverted): an entry
seeded with `**Valid:** dated 2026-02-30` at exposure ≥ `EXPOSURE_THRESHOLD`, run against
all three checks directly, returns **empty from all three**.

| Check | Why it does not report |
|---|---|
| `scan_dated_stale` | `chrono` rejects the date, and the code does a silent `continue` |
| `scan_cited_but_undeclared` | excluded — the entry *is* declared, so it is not undeclared |
| `scan_conditional_past_due` | excluded — the class is `Dated`, not `Conditional` |

The three checks partition the space by class, and a value that is well-formed to the
parser but unusable to the consumer falls through the partition.

## Reproduction

```
seed a ledger entry:
  ## R-1 — anything
  **Valid:** dated 2026-02-30
give R-1 cross-file citations ≥ EXPOSURE_THRESHOLD
run scan_dated_stale, scan_cited_but_undeclared, scan_conditional_past_due
→ all three return zero violations
```

Also true for `2025-02-29`, `2026-99-99`, and any other shape-valid, calendar-invalid value.

## Environment

codescout `experiments` at `a2ae4e10`. `src/librarian/statements.rs`,
`src/librarian/tools/doctor.rs`.

## Root cause

Two layers validate differently, and the gap between them has no owner.

- **Parser layer.** `parse_validity` checks the value against `ISO_RE`, a **shape-only**
  regex (`^\d{4}-\d{2}-\d{2}$`). `2026-02-30` matches, so it returns
  `Ok(Some(Validity::Dated("2026-02-30")))`. This is deliberate — Task 2's review recorded
  `dated 2026-99-99` as "accepted (shape-only, spec-compliant)" and deferred it.
- **Check layer.** `scan_dated_stale` converts with `chrono`, which rejects the date, and
  the code `continue`s to the next entry rather than reporting anything.

**The deferral decision was made about the parser and its consequence at the check layer
was never considered.** Task 2's review and Task 6's implementation are six tasks apart;
nothing connected them, and each is locally correct.

Measured 2026-08-20: the scratch probe above, run against all three checks.

## Evidence

Task 6's review reported `2026-02-30` as *"rejected"*; Task 8's implementer reported it as
*"parses successfully, then silently skipped"*. **Both describe the same system at
different layers**, which is why the discrepancy survived two reviews — "rejected" is true
of the conversion inside the check and false of the record's fate. Nothing errors, and
nobody is told.

There is already a test pinning the silent skip as intended behaviour:
`dated_stale_skips_a_shape_valid_but_calendar_invalid_date`. The skip is deliberate; its
invisibility is not.

## Hypotheses tried

1. **Hypothesis:** one of the three checks does report it, and only `scan_dated_stale` is
   silent.
   **Test:** scratch probe running all three checks directly against a seeded fixture.
   **Verdict:** rejected — all three return empty, for three different and individually
   correct reasons.

## Fix

Not attempted. Two directions:

- **Narrow:** `scan_dated_stale` emits its own violation when the declared date fails to
  parse, instead of `continue`. Smallest change, keeps validation where the consumer is,
  and the entry lands in the worklist that already exists. This is the reviewer's
  recommendation.
- **Structural:** `parse_validity` validates the calendar, not just the shape, and returns
  a `RecoverableError` for an impossible date — consistent with how it already refuses a
  bare `conditional` and an unknown class. Catches the typo at write time rather than at
  scan time, but reverses a deliberate deferral and would need Task 2's "shape-only" note
  revisited.

The narrow fix does not preclude the structural one, and the two answer different
questions: *what does the corpus contain* versus *what may be written*.


## Fix Round 3 (2026-08-20)

Shipped both directions from the two listed above, composed as the review's ship-then-fix
verdict required: **Structural** (`parse_validity` now validates the calendar via
`chrono::NaiveDate`, kept alongside the shape regex — see Resume note below) AND a new
fourth check, `scan_validity_unparseable`, that reports any `**Valid:**` line
`parse_validity` refuses (the check all three doc comments already deferred to by name).
Neither alone closed this: tightening the parser alone would have converted the silent
`Ok` into a silently-swallowed `Err`, which is exactly the review's stated reason for
its ship-then-fix verdict rather than a merge block.

**Fix SHA (experiments):** `954c6051` — `feat(doctor): validity_unparseable check + real
calendar date validation`
**Patch-id:** `d72dca2e4deeb75444f014677f3e43ec99b31c31`

Regression tests: `dated_rejects_a_calendar_invalid_date_even_when_shape_valid`,
`dated_still_accepts_every_shape_valid_calendar_valid_date`,
`dated_rejects_non_padded_or_wrong_width_shapes` (`statements.rs`), and
`validity_unparseable_reports_the_calendar_invalid_dates_dated_stale_skips` — the
positive half proving R-8/R-9 (this bug's own repro shape) are now reported rather than
still-invisible (`doctor.rs`). Gate: `cargo fmt` / `cargo clippy -D warnings` / `cargo
test` green, 4365 passed / 45 ignored / 0 failed across 20 binaries.
## Tests added

None yet. A regression test belongs with whichever fix lands, and the existing
`dated_stale_skips_a_shape_valid_but_calendar_invalid_date` will need its name and intent
revised — it currently pins the silence.

## Workarounds

None available to an author, which is the point: the failure is indistinguishable from a
healthy record by inspection of the entry alone.

## Resume

Decide narrow vs structural. If narrow: add a `Violation` in `scan_dated_stale`'s parse-
failure arm, and rename `dated_stale_skips_a_shape_valid_but_calendar_invalid_date` to
describe reporting rather than skipping. If structural: revisit the "shape-only,
spec-compliant" deferral recorded in Task 2's review before changing `ISO_RE`.

## References

- `src/librarian/statements.rs` — `parse_validity`, `ISO_RE`
- `src/librarian/tools/doctor.rs` — `scan_dated_stale`, `scan_cited_but_undeclared`,
  `scan_conditional_past_due`
- `docs/superpowers/specs/2026-08-20-entry-validity-and-attestation-design.md` — the design
  this defeats
- `docs/superpowers/plans/2026-08-20-statement-validity-layers-1-2.md` — the plan that
  shipped all three checks
