---
kind: bug
status: open
title: doctor's exposure floor drops worklist rows below `EXPOSURE_THRESHOLD` and reports no count of them
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
---

## Summary

Three `doctor` checks skip every entry whose citation exposure is below `EXPOSURE_THRESHOLD` (5) and report nothing about the skipped population. The gate itself is deliberate and documented; what is missing is any field saying how many candidates it withheld, so a worklist reads as the complete list of past-due / stale / undeclared entries when it is the list of those *above* the floor. Filed from the `PROBE_ROWS` classification of `doctor.exposure_threshold` as `Coverage::NoMarker`.

## Class fit

A suppression floor rather than a truncating ceiling, and the first floor in the class that withholds ROWS of a result: the caller receives a partial worklist because of a bound and cannot tell. It differs from the ceiling members in that "the cap bit" is not a property of the input size but of how many citers each entry has, and in that the omission is intended (`doctor.rs:4497-4500`: "a conditional nobody is citing is not worth anyone's attention"). Compare `context.attestation_exposure`, which is NOT a member: its floor gates an advisory block and withholds no row (see `docs/issues/archive/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md`).

## Symptom (Effect)

`doctor` output carries `summary.by_check.entry_conditional_past_due` (and the sibling counts) for the rows that survived the floor. No field counts the rows that did not. Reading source: the marker the old `Deferred` reason named counts survivors, so it is non-zero exactly when the floor did NOT bite.

## Reproduction

`src/librarian/tools/doctor.rs`: `if exposure < EXPOSURE_THRESHOLD { continue; }` at `:4553` (`scan_conditional_past_due`), `:4699` (`scan_dated_stale`), `:4828` (`scan_cited_but_undeclared`). The pinning tests (`conditional_past_due_fires_exactly_at_the_exposure_threshold` and neighbours, ~`:18879`) call the scan functions directly and assert the floor is `<`, so they fix the boundary and say nothing about disclosure. Inferred from source at `experiments` f5f1c6f7, not run.

## Root cause

The scans `continue` past a below-floor entry with no counter; the `Vec<Violation>` they return is the only output, and `summary.by_check` is derived from it. Nothing records the population the gate removed.

## Fix

Not designed — needs an owner decision whether the suppressed count is worth a field (e.g. `summary.below_exposure_floor`, per check) or whether the floor is intended to be invisible. If a field is added, `doctor.exposure_threshold` can become `Probed` with a below-floor fixture driving `call()` (the existing boundary tests bypass `call()`, so no `summary.by_check` object exists in their output).

## Tests added

None — filing only.

## Resume

1. Decide: disclose the below-floor count, or record the floor as intentionally silent.
2. If disclosed: add the field in the three scans, add a `call()`-level test, flip the row in `src/tools/core/cap_probe.rs` to `Probed`.
3. Shares `doctor.rs` with the catalog owner — coordinate before editing.

## References

- `src/librarian/tools/doctor.rs:4481-4488,4553,4699,4828`
- `src/tools/core/cap_probe.rs` row `doctor.exposure_threshold`
- `docs/issues/archive/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
