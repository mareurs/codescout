---
id: e620f5f67712182b
kind: bug
status: open
title: 'Lifecycle create checks key uniqueness, then writes: two parallel creates in different collections can share a key'
tags:
- cluster/unclassified
opened: 2026-11-14
severity: low
unverified: Reproduction not run. The race is read from the code by two reviewers; no test shows two parallel creates both passing.
---

# BUG: entry and collection create check key uniqueness, then write: two concurrent creates in different collections can both pass

## Summary

`doc(create)` in entry mode and in collection mode refuses a key that another document already holds, but it checks first and writes after. Two creates that run at the same time, `ledgers/k` and `streams/k`, can both pass the check and write two documents with one key. Found by the Task 8 review and the whole-branch review of lifecycle tools part 1; code is on `integration/identity-lifecycle-20261114` (`18e0ad3c2`), not yet on `experiments`.

## Class fit

No reserved class matches: this is a check-then-act race on a document key, in a new write path, and not a stale claim, a gate gap or a tool-output defect. `cluster/unclassified` is used so the counts that promotion reads are not bent.

## Symptom (Effect)

Two documents with the same key exist after two parallel creates. The doctor can see the duplicate afterwards (duplicate key problem). No file and no row is lost. Nothing refuses the second create at write time.

## Reproduction

Unverified, not run. Reasoning from the code: `check_key` (`src/librarian/tools/create_lifecycle.rs`, called near lines 458-463 in collection mode and in entry mode) reads the key space, and the exclusive create in `create.rs` (`insert_and_create_exclusive_with`) guards only the one path it writes. A second create with the same key in another collection writes a different path, so the exclusive create does not stop it.

## Environment

Branch `integration/identity-lifecycle-20261114` at `18e0ad3c2`. Not on `experiments`.

## Root cause

Unknown beyond the code reading above. The key is unique across the repository (the lowercase file stem), but the only atomic guard is per path.

## Evidence

- Task 8 review (ledger line "key uniqueness is check-then-write").
- Whole-branch review, deferred-minor triage (c): parked, "no file or row is lost".

## Hypotheses tried

None run.

## Fix

Not started. Options: hold the catalog lock across the key check and the write (the exclusive create already holds it from insert to take-back, so the key check may fit inside), or re-check the key after the write and take the new file back when it clashes.

## Tests added

N/A: no fix yet.

## Workarounds

Run the doctor after parallel creates. It reports duplicate keys.

## Resume

Open. Parked in the memory `tracker-lifecycle-parked-items` (§ Parked from tools part 1). Pick up with plan L-B2 part 2 or when Plan D creates entries in bulk.

## References

- docs/trackers/tracker-organization-session-log.md (F-4, F-5, W-5)
- docs/superpowers/plans/2026-11-14-tracker-lifecycle-tools-part-1.md (Task 8)
