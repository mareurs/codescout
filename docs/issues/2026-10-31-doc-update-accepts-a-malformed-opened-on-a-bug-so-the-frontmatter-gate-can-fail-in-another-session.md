---
id: '3773f11d0aeaada8'
kind: bug
status: open
title: doc(update) accepts a malformed extra.opened on a bug, so the frontmatter gate can fail in another session
tags:
- cluster/unclassified
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

Not observed. Predicted: after `doc(update, patch={extra: {opened: "yesterday"}})` on a bug, `tests/issue_frontmatter.rs` reports `opened` is not YYYY-MM-DD in whichever session next runs the gate.

## Reproduction

NOT run. I did not want to write a bad `opened` into a real record that peers read. A safe run needs a throwaway bug file, one `doc(update)` with a bad `opened`, and the file deleted afterwards.

## Environment

codescout `experiments`, tip after `30721b865`.

## Root cause

Read in `src/librarian/tools/update.rs`: `merge_extra` (lines 133-146) upserts each key, deletes a key on `null`, and checks nothing else. `apply_frontmatter_patch` calls it for all three frontmatter-touching branches. The create-side check, `is_iso_date` in `src/librarian/tools/create.rs`, is private to that file.

## Evidence

Code read only, as above. No run.

## Hypotheses tried

None beyond the code read. Open: whether a `null` `opened` (a delete) should be allowed on a bug, since create now always leaves a stamped value.

## Fix

Not started. Candidate: move `is_iso_date` to a shared place and call it from `merge_extra` for `opened` on a bug. `merge_extra` does not know the kind, so it needs the file's `kind` passed in.

## Tests added

None yet.

## Workarounds

Pass a calendar-valid `YYYY-MM-DD` value.

## Resume

First run the failing case on a throwaway bug to confirm the bug is real. Then decide the `null` question and write the test.

## References

- `src/librarian/tools/update.rs` `merge_extra`
- `src/librarian/tools/create.rs` `is_iso_date`
- `docs/issues/2026-10-31-doc-create-accepts-a-malformed-opened-on-a-bug-so-the-frontmatter-gate-fails-in-another-session.md`
