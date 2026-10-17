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

- Each of the three scans (`scan_conditional_past_due`, `scan_dated_stale`, `scan_cited_but_undeclared`) applies the floor AFTER its class test, and calls `DoctorScope::note_below_floor(check, id, abs_path)` at the `continue`. Applying it after the class test is what makes the figure a count of CANDIDATES (declared conditional; dated past the horizon; undeclared) rather than of every entry below the floor.
- **The count is split by the active scope** (review of `426feb36`: the first version was catalog-wide beside a project-scoped `summary.by_check`, so a reader in one repo was shown other workspaces' candidates). `note_below_floor` classifies with the new pure `DoctorScope::would_admit` (the predicate `admit` applies: lexical roots, then the relevance exemption; `admit` now calls it too) into `below_floor_in_scope` / `below_floor_out_of_scope`, and never touches `scoped_out`.
- `call()` publishes `summary.below_exposure_floor = { floor, in_scope, out_of_scope, scope }`, each map per check, zeros included so `0` reads as "withheld nothing" and not "not measured". `in_scope` is the figure to act on; `scope` names both halves and that together they are catalog-wide.
- The two scans without a `seen_ids` guard (`scan_conditional_past_due`, `scan_dated_stale`) deduplicate the withheld tally per `(file, entry id)`, so a duplicated heading is one entry; their reported rows are unchanged.
- **The hint is silent unless it can be acted on** (ADR 2026-08-27 clause 2; undeclared entries below the floor exist on essentially every catalog, so a sentence per call would be noise): it names only a check that returned NO rows while the floor withheld IN-SCOPE candidates, since an empty list is the one reading a reader cannot correct from the list itself. It says "at or above" the floor, matching `scope`.
- `src/librarian/tools/doctor/scope.rs`: `below_floor_in_scope`, `below_floor_out_of_scope`, `note_below_floor`, `would_admit`. `src/tools/core/cap_probe.rs`: `doctor.exposure_threshold` moves `NoMarker` to `Probed` (marker `$.summary.below_exposure_floor.in_scope`, mutation `Killed`).

Cost (measured 2026-10-17, release build, a read-only copy of this machine's 450 MB / 6,169-artifact catalog made with `sqlite3 .backup`, rooted at this worktree; old order restored through a temporary switch that was not committed): `declared_section_text` and `parse_validity` now run on every section where only exposure >= 5 paid before. The three scans alone, warm, median of 3 rounds: 822 ms new vs 784 ms old (+38 ms, +4.8%). The whole `doctor` call, median of 4 warm rounds: 9,973 ms new vs 9,959 ms old, a difference inside the run-to-run noise (range 9.5-12.2 s; the first call was 19.6 s cold). Both orders emitted the same 77 rows from the scans and `summary.total` = 2. The scans are dominated by reading every artifact file, not by parsing.

## Tests added

- `call_counts_the_candidates_the_exposure_floor_withheld_and_names_its_scope` (`doctor.rs`): drives `call()` with one below-floor candidate and one above-floor row per check, plus a fresh dated entry and an `invariant` entry that are candidates of no check and must not be counted; asserts the per-check `in_scope` counts, `floor`, a `scope` string naming both halves, zero `out_of_scope`, and that the hint stays silent when every check returned a row. No active project, so everything is in scope.
- `call_splits_the_withheld_count_by_the_active_scope` (`doctor.rs`): rooted in a real project (`ctx_rooted_at`), with one withheld candidate per check under a sibling root. Asserts the sibling's candidates are `out_of_scope` and do not inflate `in_scope`, that `entry_validity_scoped_by_project` stays absent (the tally does not leak into `scoped_out`), and the hint names only the in-scope empty check, with the `at or above` wording.
- `a_duplicated_heading_is_withheld_once_not_once_per_instance` (`doctor.rs`): a heading written twice is one withheld entry in both scans, and the reported rows are still two.
- `the_withheld_count_and_the_reported_rows_split_at_the_same_boundary` (`doctor.rs`): all three scans at exactly `EXPOSURE_THRESHOLD` (reported, not withheld) and one below (withheld, not reported), a fresh `DoctorScope` per level.

Mutations observed red (2026-10-17): deleting `note_below_floor` at each of the three sites, one run each (3 of 3, both original tests); tallying only `would_admit`-admitted candidates in `scan_conditional_past_due` (`call_splits_the_withheld_count_by_the_active_scope`: `entry_conditional_past_due: the sibling's withheld candidate is counted, apart`, red); tallying `scan_dated_stale` only when `scope.admit` passes (same test, `entry_dated_stale` assertion, red); classifying every candidate in scope (`call_splits...`, red); removing the dedup in each of the two scans (`a_duplicated_heading_...`: `{"entry_conditional_past_due": 2, ...}` and `"entry_dated_stale": 2`, red); dropping the `reported == 0` gate on the hint (`call_counts_the_candidates_...`: the hint fired on a catalog where every check had rows, red). A tally moved after `admit` is invisible to any `current_project: None` test (that resolves to `Scope::All`); only the project-rooted test reds it.

## Resume

Nothing outstanding. Not archived on purpose; archive when the fix is on the shipping branch.

## Fix provenance

- **SHA:** `426feb369598cd6efc563f9c31a3b9618cc4f125` (branch `worktree-agent-a4845d73e6020762a`; one commit that also fixes `2026-10-16-doctor-recently-touched-walk-returns-a-bare-set-at-its-cap.md`), patch-id `ab2ff3ec97ecf247f9fa4368d716ae92b4044981` (`git show 426feb36 | git patch-id --stable`)
- **SHA:** `b232271cefaa2a30a602717eeda2cfe529d05c93` (same branch; review fixes: scope split, dedup, hint gating), patch-id `79f2a4ea13ddd63e8222cba2e92c7c5ef1fbc407` (`git show b232271c | git patch-id --stable`)

## References

- `src/librarian/tools/doctor.rs:4481-4488,4553,4699,4828`
- `src/tools/core/cap_probe.rs` row `doctor.exposure_threshold`
- `docs/issues/archive/2026-09-03-result-cap-conflates-truncating-ceilings-with-suppressing-floors.md`
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md`
