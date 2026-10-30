---
id: f350eba20450a7f4
kind: bug
status: open
title: doc(create) for a bug file does not stamp or require opened, so the frontmatter gate fails in another session
tags:
- cluster/unclassified
opened: 2026-10-30
owner: marius
severity: low
---

# BUG: doc(create) for a bug file does not stamp or require `opened`, so the issue_frontmatter gate test goes red on the next run

## Summary

`doc(action="create", kind="bug", rel_path="docs/issues/…")` writes a bug file with no `opened:` field unless the caller passes `extra={"opened": …}`. `tests/issue_frontmatter.rs` requires `opened` for files dated 2026-09-24 or later, so the omission surfaces at gate time, in another session, not at creation.

## Class fit

`cluster/unclassified`. A required field the creating tool neither fills nor asks for.

## Symptom (Effect)

On 2026-10-30 `every_tracked_issue_file_has_wellformed_frontmatter` failed twice in one hour on committed peer bug files: `2026-10-30-grep-has-no-wall-clock-budget-…md` (`731747939c0bb764`, repaired in `385d87e5f`) and `2026-10-30-the-mcp-reindex-wrote-each-embedded-chunk-to-qdrant-as-its-own-durable-request.md` (still missing at the time of filing). The failure blocks the whole gate for every session.

## Reproduction

Call `doc(action="create", kind="bug", rel_path="docs/issues/<date>-x.md", title=…, body=…)` with no `extra`, then run `scripts/with-slot.sh cargo test -p codescout --test issue_frontmatter`. NOT run in this session as a clean repro: I created one bug that way (`25d8de9cf59c0e49`) and added `opened` by `update` before reading the frontmatter or running the test. The claim rests on the code, below, and on the two peer files.

## Environment

Linux, `experiments` at `327bf1705`.

## Root cause

`grep` over `src/librarian` for `"opened"` finds one hit in the create path: a test in `src/librarian/tools/create.rs` (line 1439) that passes `opened` in `extra`. No create code sets it. Not read: whether `doc(create)` has a bug-specific default elsewhere, such as in the template path.

## Evidence

The grep above, the two failing runs on 2026-10-30, and the failure text: `missing opened (required for files dated 2026-09-24 or later)`.

## Hypotheses tried

None.

## Fix

Not started. Candidates: default `extra.opened` to today for `kind=bug` under `docs/issues/`; or refuse the create with a recoverable error that names `extra.opened`. Either puts the failure in the creating call, where its author can act on it.

## Tests added

None yet.

## Workarounds

Pass `extra={"opened": "YYYY-MM-DD"}` on create, or `doc(update, patch={extra:{opened:…}})` afterwards.

## Resume

Run the reproduction above to confirm, then read `create.rs` for a bug-kind branch before choosing between a default and a refusal.

## References

- `tests/issue_frontmatter.rs`
- `docs/issues/2026-10-30-doctor-reports-a-random-host-per-fresh-catalog-so-the-forward-param-probe-may-pass-vacuously-for-doctor-keys.md`
