---
id: 7c1dceddba3e0d7b
kind: bug
status: fixed
title: doc(update) accepts a malformed extra.opened on a bug, so the frontmatter gate can fail in another session
tags:
- cluster/unclassified
closed: 2026-10-31
opened: 2026-10-31
owner: marius
severity: low
---

# BUG: doc(update) accepts a malformed extra.opened on a bug, so the frontmatter gate can fail in another session

## Summary

`doc(update, patch={extra: {...}})` merges any `extra` value into the frontmatter with no check. On a bug file, a malformed `opened` would reach the file the same way it could on create before `30721b865`.

## Class fit

The same class as the create-side bug: a free-form `extra` map carries a key that a gate test constrains, and one write path validates it while the other does not.

## Symptom (Effect)

Observed by a run on 2026-10-31 (the failing test below): `doc(update, patch={extra: {opened: "yesterday"}})` on a bug was accepted. `tests/issue_frontmatter.rs` would then report `opened` is not YYYY-MM-DD in whichever session next runs the gate.

## Reproduction

Run by the test `a_malformed_opened_on_a_bug_is_refused_by_update_and_leaves_the_file_unchanged` before the fix: it failed with `update accepted opened "yesterday"`. The test creates a bug, then calls `update` with each bad value. I did not run it against the live server, because that would write a bad `opened` into a real record. The rebuilt server did not yet contain this fix.

## Environment

codescout `experiments`, tip after `30721b865`.

## Root cause

Read in `src/librarian/tools/update.rs`: `merge_extra` (lines 133-146) upserts each key, deletes a key on `null`, and checks nothing else. `apply_frontmatter_patch` calls it for all three frontmatter-touching branches. The create-side check, `is_iso_date` in `src/librarian/tools/create.rs`, is private to that file.

## Evidence

The red test run above, and the code read: `merge_extra` upserts every key. Two control tests passed before the fix and after it: a calendar-valid `opened` and another `extra` key on a bug, and a non-date `opened` on a `spec`.

## Hypotheses tried

The open question was whether a `null` `opened` should be allowed on a bug. It is refused: `merge_extra` turns `null` into a delete, and a bug without `opened` fails the same gate (required for files dated 2026-09-24 or later). A `null` on a bug that never had the key is also refused, which is a small over-refusal and harmless.

The kind did not need to be passed into `merge_extra`: `call` already holds `row.kind`, and the check sits in its pre-write block for `patch.extra`.

## Fix

Fixed on `experiments` in `a10e819bf`. In `call` in `src/librarian/tools/update.rs`, the pre-write block for `patch.extra` now refuses a supplied `opened` on a `row.kind == "bug"` unless `create::is_iso_date` accepts it (a string of length 10 that `chrono` parses as a real date). The check runs before any write, so a refusal leaves the file byte-identical. `is_iso_date` in `create.rs` became `pub(super)` so create and update share one rule.


## Fix provenance

- **SHA:** `a10e819bf` (`experiments`)
- **patch-id:** `ebdb3d330e0e550e84245801e7f4cc6092080058`

## Tests added

- `librarian::tools::update::tests::a_malformed_opened_on_a_bug_is_refused_by_update_and_leaves_the_file_unchanged`: six bad values; each refused, and the file is unchanged after each. Red before the fix.
- `librarian::tools::update::tests::a_valid_opened_and_other_extra_keys_are_accepted_by_update_on_a_bug`
- `librarian::tools::update::tests::a_non_date_opened_on_a_non_bug_kind_is_accepted_by_update`

Mutations, each reverted: applying the check to every kind reds the non-bug test; skipping the check for a `null` value reds the first test on `null`. I did not mutate the check to run after the write; the unchanged-file assertion is the guard for that and was not shown red.

## Workarounds

Pass a calendar-valid `YYYY-MM-DD` value.

## Resume

Closed. After the next `/mcp` rebuild, one live call of `doc(update)` with a bad `opened` on a throwaway bug would confirm the shipped binary. The unit tests ran against the compiled library, not the running server.

## References

- `src/librarian/tools/update.rs` `merge_extra`
- `src/librarian/tools/create.rs` `is_iso_date`
- `docs/issues/archive/2026-10-31-doc-create-accepts-a-malformed-opened-on-a-bug-so-the-frontmatter-gate-fails-in-another-session.md`
