---
id: aa1110786cd2a8a4
kind: bug
status: fixed
title: RESULT_CAP conflates truncating ceilings with suppressing floors, and two structurally identical rows got opposite dispositions
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-03
closed: 2026-10-16
severity: medium
---

## Summary

The `result-cap-marker-gate` branch's `RESULT_CAP` classification vocabulary is used for two mechanically different things: caps that **truncate** (a ceiling — more input than the cap allows, excess is dropped) and caps that **suppress** (a floor — input below a threshold is withheld). A marker means opposite things under each: for a ceiling, the marker is observable exactly when the cap *bites*; for a floor, the marker described in this branch is observable exactly when the cap does *not* bite. This was flagged once for `doctor.exposure_threshold` (left `Deferred`) but a structural twin, `context.attestation_exposure`, was independently classified `Probed` **and** `Mutation::Killed` — inside the branch's own headline "17 mutation-verified" count.

**Re-verified 2026-09-25 (medium-tier sweep, `experiments` @ `fcd451de`) — the inconsistency is live; the
decision is owed.** `doctor.exposure_threshold` is still `Deferred` (`cap_probe.rs:459-475`) while
`context.attestation_exposure` is still `Probed`/`Killed` (`:642-649`); `git grep -hoE 'cap-class: [A-Z_]+' HEAD --
src` returns only `RESULT_CAP` and `NOT_A_CAP`, so no floor/ceiling vocabulary exists. **One correction to this
file's framing:** a below-floor test does exist for context (`the_tap_stays_silent_one_citation_below_the_floor`,
`context.rs` ~1722), and context's floor gates an ADVISORY block while doctor's drops RESULT rows
(`doctor.rs:4311`, `:4457`, `:4586`) — so "structural twin" overstates it, though the opposite classifications
stand. The doctor row's reason text still cites `doctor.rs:2947` / `:11267`, now `:4311` / `:17902`.

## Symptom (Effect)

Two `RESULT_CAP`-annotated constants with the identical `>=` threshold shape receive opposite dispositions in the branch's probe table (`src/tools/core/cap_probe.rs`):

- `doctor.exposure_threshold` (`src/librarian/tools/doctor.rs:2882`, `if exposure < EXPOSURE_THRESHOLD { continue; }`) — `Coverage::Deferred`, reasoned as a floor whose marker is unobservable when the cap bites.
- `context.attestation_exposure` (`src/librarian/tools/context.rs:72`, `if exposure >= ATTESTATION_EXPOSURE_THRESHOLD`) — `Coverage::Probed`, `Mutation::Killed`. Its own doc comment at `context.rs:62` reads *"A floor, not a period"* and explicitly disclaims equivalence to `doctor`'s constant "despite sharing a value today" — yet the row is certified as fully covered.

## Reproduction

1. Check out `result-cap-marker-gate` at `2a32c043` (or later, once merged).
2. Read `src/tools/core/cap_probe.rs`'s rows for `doctor.exposure_threshold` (~`:435-452`) and `context.attestation_exposure` (~`:599-610`).
3. Read `src/librarian/tools/context.rs:58-70` — the constant's own doc comment states the floor property directly.
4. Observe: one row is `Deferred` for being a floor; the other is `Probed`+`Killed` despite being the same shape, cited by a test (`a_load_bearing_statement_arms_the_tap_and_says_what_would_discharge_it`, `context.rs:1661`) that asserts the marker block is **present at the floor** — the opposite event from an `IC-13` regression (a marker silently vanishing below the floor with no field disclosing the drop).

Not independently re-run as a live mutation by this filing; the claim is a whole-branch-review finding (2026-09-03), itself spot-checked at the bytes by the session that filed this bug (confirmed `context.rs:62`'s "floor, not a period" text and the `cap_probe.rs` row classifications).

## Environment

`result-cap-marker-gate` branch/worktree, `src/tools/core/cap_probe.rs`, `src/librarian/tools/doctor.rs`, `src/librarian/tools/context.rs`.

## Root cause

`RESULT_CAP` as a classification carries one implicit contract: "a marker exists that fires when the cap truncates the result." A suppression floor inverts that contract — the marker (or its absence) signals the *opposite* condition. The branch's gate has no vocabulary distinguishing the two, so two rows sharing the shape were classified inconsistently by two different people/passes, and the inconsistency reached the branch's own "mutation-verified" count.

*Read from `cap_probe.rs` and `context.rs` at `result-cap-marker-gate` HEAD `2a32c043` — confirmed at the bytes during the whole-branch review and by this filing's own spot-check, not re-derived independently beyond that.*

## Evidence

`src/librarian/tools/context.rs:62-64`:

```
/// A **floor, not a period**: a Statement appraised at 5 arms again at 10.
///
/// Deliberately NOT `doctor`'s `EXPOSURE_THRESHOLD`, despite sharing a value today.
```

`src/librarian/tools/doctor.rs:2947` (and identical siblings at `:3096`, `:3219`):

```
if exposure < EXPOSURE_THRESHOLD { continue; }
```

`context.rs:355`, `:376`:

```
if exposure >= ATTESTATION_EXPOSURE_THRESHOLD
armed = exposure >= ATTESTATION_EXPOSURE_THRESHOLD && …
```

## Hypotheses tried

1. **Hypothesis:** the two rows' classifications are both individually defensible and the inconsistency is cosmetic. **Test:** re-read `Coverage::Probed`'s own doc contract (`cap_probe.rs:45-73`) — it promises "a behavioural test drives this cap past its bound and asserts the marker arrives." `context.attestation_exposure`'s cited test asserts presence *at* the floor, not absence *below* it, so a marker silently vanishing below the floor would not be caught by anything this row claims as coverage. **Verdict:** confirmed as a real inconsistency, not cosmetic.

## Fix

Chosen with the lead's go-ahead (2026-10-16): none of (a)/(b)/(c) as filed, but the rule they were all missing, which decides each row on the evidence rather than adding vocabulary.

**A floor is a `RESULT_CAP` iff what it suppresses is rows of the result.**

- `doctor.exposure_threshold` (`doctor.rs:4553`, `:4699`, `:4828`: `if exposure < EXPOSURE_THRESHOLD { continue; }`) skips worklist rows, so the caller gets a partial result and cannot tell. It stays a `RESULT_CAP` and is now `Coverage::NoMarker`, with its defect filed in `docs/issues/2026-10-16-doctor-exposure-floor-drops-worklist-rows-without-a-count.md`.
- `context.attestation_exposure` (`context.rs:355`, `:376`) only gates the `verification` advisory block; the neighbourhood is returned whole either way (read at the bytes), exactly the shape `LINE_SOFT_CAP` (`types.rs:38`) records as `NOT_A_CAP`. Its annotation is now `NOT_A_CAP` with that reason and its probe row is removed, so it can no longer inflate the mutation-verified count. The existing tests stay as ordinary coverage of the tap, including the below-floor one the filing overlooked (`the_tap_stays_silent_one_citation_below_the_floor`).
- The mutation-verified tally moves 36 -> 35 (the filing's 18/17 figures were stale): live figure now 85 total, 36 probed, 40 untested, 9 no-marker, 35 mutation-verified. A note in `cap_probe.rs`'s sweep header records the amendment and the sweep population (16 of 17 killed).

## Tests added

`a_floor_is_a_result_cap_only_when_it_drops_rows_of_the_result` in `src/tools/core/cap_probe_tests.rs`: asserts doctor's floor is a `NoMarker` row, that no `context.attestation_exposure` row exists, and (reading `context.rs` as text) that the line above `ATTESTATION_EXPOSURE_THRESHOLD` is `cap-class: NOT_A_CAP`. Seen RED before the row was removed. `tests/result_caps.rs` passes in full (71 tests), including the both-directions annotation/row correspondence that pins the removal.

## Workarounds

A reader triaging the `result-cap-marker-gate` gate's `Probed`/`Killed` count should not treat `context.attestation_exposure` as evidence that a below-floor regression would be caught.

## Resume

Resume step 2 (other `RESULT_CAP` rows whose use site is a threshold) was run 2026-10-16 as a scripted scan of every `RESULT_CAP` const's use sites for a strict `<` against the const (or a `>=` followed by `continue`). It found the two floors named here and one more that the filing did not: `read_file.json_path_route_room` (`JSON_PATH_ROUTE_ROOM_FLOOR`, `read_file.rs:669,685`), a floor on page room that chooses between a literal route and a prose form of `next`. It is `Probed`+`Killed` and was NOT reclassified here: both branches deliver a complete continuation, so by the rule above it looks like `NOT_A_CAP` too, but it is a judgement on a row this issue did not name, so it is left for the owner (and flagged to the lead). The other hits of the scan were ceilings or counters (`agent_build_check.rendered_diagnostics`, `preview.plan_open_next`, `tracker_design.existing_trackers`, `tool_output.largest_array_depth`). The scan reads use sites by pattern; it does not prove no floor is written another way.

## References

- `src/librarian/tools/context.rs:58-70,355,376`, `src/librarian/tools/doctor.rs:2882,2947,3096,3219`, `src/tools/core/cap_probe.rs` (rows for both ids)
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I1
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`) — supersedes/extends the standing owed item there for `doctor.exposure_threshold` alone
