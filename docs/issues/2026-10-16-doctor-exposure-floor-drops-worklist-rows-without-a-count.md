---
kind: bug
status: fixed
title: doctor's exposure floor drops worklist rows below `EXPOSURE_THRESHOLD` and reports no count of them
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-10-16
severity: low
closed: 2026-10-17
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

Decision taken: disclose. The floor stays deliberate; `doctor` now says how many candidates it withheld and names the population the count ranges over.

- Each of the three scans (`scan_conditional_past_due`, `scan_dated_stale`, `scan_cited_but_undeclared`) applies the floor AFTER its class test, and calls `DoctorScope::note_below_floor(check)` at the `continue`. Applying it after the class test is what makes the figure a count of CANDIDATES (declared conditional; dated past the horizon; undeclared) rather than of every entry below the floor.
- `call()` publishes `summary.below_exposure_floor = { floor, by_check, scope }`, zeros included so `0` reads as "withheld nothing" and not "not measured". `scope` says the count is catalog-wide: it is tallied before `DoctorScope::admit`, so unlike `summary.by_check` it is not narrowed to the active project.
- A nonzero total also adds a `catalog_health.hint` sentence saying the three lists are the entries above the floor.
- `src/librarian/tools/doctor/scope.rs`: `below_floor` field, `note_below_floor`, `below_floor`. `src/tools/core/cap_probe.rs`: `doctor.exposure_threshold` moves `NoMarker` to `Probed` (marker `$.summary.below_exposure_floor`, mutation `Killed`).

Cost: `parse_validity` now runs on entries below the floor too, where the floor used to skip them before parsing.

## Tests added

- `call_counts_the_candidates_the_exposure_floor_withheld_and_names_its_scope` (`doctor.rs`): drives `call()` with one below-floor candidate and one above-floor row per check, plus a fresh dated entry and an `invariant` entry that are candidates of no check and must not be counted; asserts the per-check counts, `floor`, and a `scope` that says `catalog-wide`.
- `the_withheld_count_and_the_reported_rows_split_at_the_same_boundary` (`doctor.rs`): all three scans at exactly `EXPOSURE_THRESHOLD` (reported, not withheld) and one below (withheld, not reported), a fresh `DoctorScope` per level.

Mutation: deleting `note_below_floor` at each of the three sites, one at a time, reds both tests (3 of 3 killed).

## Resume

Nothing outstanding. Not archived on purpose; archive when the fix is on the shipping branch.

## Fix provenance

- **SHA:** `426feb369598cd6efc563f9c31a3b9618cc4f125` (branch `worktree-agent-a4845d73e6020762a`; one commit that also fixes `2026-10-16-doctor-recently-touched-walk-returns-a-bare-set-at-its-cap.md`)
- **patch-id:** `ab2ff3ec97ecf247f9fa4368d716ae92b4044981` (`git show 426feb36 | git patch-id --stable`)

## References

- `src/librarian/tools/doctor.rs:4481-4488,4553,4699,4828`
- `src/tools/core/cap_probe.rs` row `doctor.exposure_threshold`
- `docs/issues/archive/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
