---
id: '325bb3e77164836a'
kind: bug
status: fixed
title: doc(create) accepts a malformed extra.opened on a bug, so the frontmatter gate fails in another session
tags:
- cluster/unclassified
closed: 2026-10-31
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

Fixed in `30721b865` (`src/librarian/tools/create.rs`). On create for `kind=bug`, `call` stamps `opened` when it is absent (the earlier fix, `1d79382e1`) and then refuses a supplied value unless `is_iso_date` accepts it: a JSON string of length 10 that `chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")` parses. The refusal is a `LibrarianRecoverableError` that names the value and the form. It runs before the file and the catalog row are written, so a refusal leaves neither.

The earlier candidate said `parse_validity` in `src/librarian/statements.rs` could be shared. That was wrong: `parse_validity` parses a `**Valid:**` section, and its date check is one inline expression (`iso_re().is_match(d) && chrono::NaiveDate::parse_from_str(..)`), not a function. `is_iso_date` repeats that expression for a `Value`. Extracting one shared predicate is possible and was not done.

`chrono`'s `%Y-%m-%d` alone accepts `2026-1-2`, so the length check is needed.

## Tests added

- `librarian::tools::create::tests::a_malformed_opened_on_a_bug_is_refused_and_writes_no_file`: six bad values (`"yesterday"`, `"2026/10/17"`, `"2026-1-2"`, `"2026-13-45"`, `20261017`, `null`); each must be refused and leave no file. Red before the fix (`opened "yesterday" was accepted`).
- `librarian::tools::create::tests::a_non_date_opened_on_a_non_bug_kind_is_not_refused`: a `spec` with `opened: "soon"` is still created.

Mutations, each reverted: dropping the length check reds the first test on `"2026-1-2"`; replacing the `chrono` parse with a digit-and-dash shape check reds it on `"2026-13-45"`. I did not run a mutation that applies the check to every kind; the non-bug test is the guard for that and was not shown red.

## Workarounds

Pass a well-formed date, or omit `opened` and let the server stamp it.

## Resume

One question stays open. `merge_extra` in `src/librarian/tools/update.rs` upserts any `extra` value with no check, so `doc(update, patch={extra: {opened: "yesterday"}})` on a bug looks like it writes a malformed `opened` the same way. This is a code read only: I did not run it. If it is real, it needs its own bug file and a check shared with create.

## References

- `f350eba20450a7f4`: the missing-`opened` bug this one follows.
- `tests/issue_frontmatter.rs`: the gate that refuses the malformed value.
- `docs/adrs/2026-10-16-a-rule-lives-where-the-agent-already-acts.md`: rule 4, the refusal text and the check are one thing.
