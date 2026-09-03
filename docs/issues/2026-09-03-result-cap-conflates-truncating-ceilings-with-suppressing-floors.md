---
id: aa1110786cd2a8a4
kind: bug
status: open
title: RESULT_CAP conflates truncating ceilings with suppressing floors, and two structurally identical rows got opposite dispositions
owners:
- marius
tags:
- cluster/capped-result-presented-as-complete
opened: 2026-09-03
severity: medium
---

## Summary

The `result-cap-marker-gate` branch's `RESULT_CAP` classification vocabulary is used for two mechanically different things: caps that **truncate** (a ceiling — more input than the cap allows, excess is dropped) and caps that **suppress** (a floor — input below a threshold is withheld). A marker means opposite things under each: for a ceiling, the marker is observable exactly when the cap *bites*; for a floor, the marker described in this branch is observable exactly when the cap does *not* bite. This was flagged once for `doctor.exposure_threshold` (left `Deferred`) but a structural twin, `context.attestation_exposure`, was independently classified `Probed` **and** `Mutation::Killed` — inside the branch's own headline "17 mutation-verified" count.

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

Not designed. Candidate directions: (a) split `RESULT_CAP` into two sub-vocabularies (a truncating-ceiling shape and a suppressing-floor shape) with contract text specific to each; (b) reclassify `context.attestation_exposure` to `Deferred` for consistency with `doctor.exposure_threshold`, which would move the branch's published tally from 18 Probed/17 mutation-verified to 17 Probed/16 mutation-verified; (c) add a companion probe specifically for the below-floor case (does the marker correctly stay absent, and is there a field that discloses "data may exist below the disclosure floor"?).

## Tests added

None yet.

## Workarounds

A reader triaging the `result-cap-marker-gate` gate's `Probed`/`Killed` count should not treat `context.attestation_exposure` as evidence that a below-floor regression would be caught.

## Resume

1. Decide the direction (a/b/c above) once the branch merges and this file's severity can be weighed against the branch's other findings.
2. Grep the merged `cap_probe.rs` for other `RESULT_CAP` rows whose use site is a `>=`/`<` threshold rather than a length/byte/count ceiling — this pair may not be the only instance.
3. If (b) is chosen, update the branch's tally documentation (`IC-13`'s tracker body, `tests/result_caps.rs`'s header) to the corrected numbers.

## References

- `src/librarian/tools/context.rs:58-70,355,376`, `src/librarian/tools/doctor.rs:2882,2947,3096,3219`, `src/tools/core/cap_probe.rs` (rows for both ids)
- Surfaced during `result-cap-marker-gate` branch's whole-branch review (2026-09-03), session ledger `.superpowers/sdd/2026-09-02-result-cap-marker-gate/progress.md`, finding I1
- `docs/trackers/issue-clusters/IC-13-capped-result-presented-as-complete.md` (artifact `8a9dd5a27cd03480`) — supersedes/extends the standing owed item there for `doctor.exposure_threshold` alone

