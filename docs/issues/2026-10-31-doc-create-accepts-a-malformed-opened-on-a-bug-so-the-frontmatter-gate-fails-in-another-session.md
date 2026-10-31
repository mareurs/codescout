---
id: '325bb3e77164836a'
kind: bug
status: open
title: doc(create) accepts a malformed extra.opened on a bug, so the frontmatter gate fails in another session
tags:
- cluster/unclassified
opened: 2026-10-31
owner: marius
severity: low
---

# BUG: doc(create) accepts a malformed `extra.opened` on a bug, so the issue_frontmatter gate test goes red on the next run

## Summary

`doc(action="create", kind="bug", …, extra={"opened": "yesterday"})` writes the value as given. `tests/issue_frontmatter.rs` requires `opened: YYYY-MM-DD` and refuses anything else, so the error surfaces at gate time in another session, not in the creating call. This is the residue of `f350eba20450a7f4`, which fixed the missing-`opened` case by stamping the date. A supplied value is kept as given, so a malformed one still passes.

## Class fit

`cluster/unclassified`. A required field with a fixed shape that the creating tool accepts in any shape and the gate refuses later, in another session.

## Symptom (Effect)

Not observed in the wild. The shape was found while fixing `f350eba20450a7f4`. The failure text it would produce is `` `opened` is not YYYY-MM-DD: "yesterday" ``, from `tests/issue_frontmatter.rs`.

## Reproduction

NOT run. Call `doc(action="create", kind="bug", rel_path="docs/issues/<date>-x.md", title=…, body=…, extra={"opened": "yesterday"})`, then run `scripts/with-slot.sh cargo test -p codescout --test issue_frontmatter`. The claim rests on reading `call` in `src/librarian/tools/create.rs`: after the reserved-key check, `extra` goes to the frontmatter writer unchanged, except that a missing `opened` is now stamped. Nothing there parses `opened`.

## Environment

Linux, `experiments`, after the stamp fix for `f350eba20450a7f4` (uncommitted when this was filed).

## Root cause

Read, not run. `reject_reserved_extra_keys` checks only for keys the schema already models. No code validates the value of `opened`. The stamp added for `f350eba20450a7f4` uses `entry(..).or_insert_with(..)`, which keeps any supplied value, correctly for a late filing but also for a malformed one.

## Evidence

The code reading above, and the gate test's own rule: `is_date` in `tests/issue_frontmatter.rs` accepts only `YYYY-MM-DD`. Not checked: whether `doc(update)` with `patch={extra:{opened:…}}` validates the value on the update path.

## Hypotheses tried

None.

## Fix

Not started. Candidate: on create for `kind=bug`, refuse a supplied `opened` that is not a calendar-valid `YYYY-MM-DD`, with a recoverable error that names the value and the form. `parse_validity` in `src/librarian/statements.rs` already rejects a calendar-invalid ISO date, so its check can be shared instead of written again.

## Tests added

None yet.

## Workarounds

Pass a well-formed date, or omit `opened` and let the server stamp it.

## Resume

Decide whether the update path needs the same check. Then write a failing create test for `"yesterday"` and for `"2026/10/17"`, and a passing one for a calendar-valid past date.

## References

- `f350eba20450a7f4`: the missing-`opened` bug this one follows.
- `tests/issue_frontmatter.rs`: the gate that refuses the malformed value.
- `docs/adrs/2026-10-16-a-rule-lives-where-the-agent-already-acts.md`: rule 4, the refusal text and the check are one thing.
