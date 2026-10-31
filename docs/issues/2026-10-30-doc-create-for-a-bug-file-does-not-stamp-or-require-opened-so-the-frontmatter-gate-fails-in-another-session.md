---
id: f350eba20450a7f4
kind: bug
status: fixed
title: doc(create) for a bug file does not stamp or require opened, so the frontmatter gate fails in another session
tags:
- cluster/unclassified
closed: 2026-10-31
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

Call `doc(action="create", kind="bug", rel_path="docs/issues/<date>-x.md", title=…, body=…)` with no `extra`, then run `scripts/with-slot.sh cargo test -p codescout --test issue_frontmatter`. NOT run in this session as a clean repro: I created one bug that way (`ab99663291a364a0`) and added `opened` by `update` before reading the frontmatter or running the test. The claim rests on the code, below, and on the two peer files.

## Environment

Linux, `experiments` at `327bf1705`.

## Root cause

`grep` over `src/librarian` for `"opened"` finds one hit in the create path: a test in `src/librarian/tools/create.rs` (line 1439) that passes `opened` in `extra`. No create code sets it. Verified 2026-10-30 on `experiments` 0f8705532: `call` builds `Frontmatter` from `a.extra` unchanged; no bug-specific default exists, and the new test failed before the fix.

## Evidence

The grep above, the two failing runs on 2026-10-30, and the failure text: `missing opened (required for files dated 2026-09-24 or later)`.

## Hypotheses tried

None.

## Fix

Done 2026-10-31, commit `1d79382e1`. `doc(create)` for `kind=bug` stamps `extra.opened` with today's date (UTC) when the caller gave none, at any `rel_path`; a caller's own value wins. Other kinds are untouched. Stamping was chosen over a refusal: it puts the field in the creating call with no retry, and the value is always knowable. A malformed supplied value was kept as given until `30721b865`; that was filed separately as `58636f57c701952b` (archived, fixed).

A narrower variant that stamped only under `docs/issues/` (commit `27c47ab19`, written independently in a parallel session) was dropped when this one landed. A generic tool must not hard-code one repo's directory, so the wider rule stands.

Code: `src/librarian/tools/create.rs`, in `call`.

## Tests added

`librarian::tools::create::tests::a_bug_created_without_opened_is_stamped_with_todays_date` (read on both sides of the call, so a run across midnight UTC is not a flake), `a_supplied_opened_on_a_bug_is_kept` and `a_non_bug_kind_is_not_stamped_with_opened`. Per the author, the first was red before the fix, overwriting a supplied value turns the second red, and stamping every kind turns the third red. A mutation that deletes the stamp was not run.

## Workarounds

Pass `extra={"opened": "YYYY-MM-DD"}` on create, or `doc(update, patch={extra:{opened:…}})` afterwards.

## Resume

Run the reproduction above to confirm, then read `create.rs` for a bug-kind branch before choosing between a default and a refusal.

## References

- `tests/issue_frontmatter.rs`
- `docs/issues/archive/2026-10-30-doctor-reports-a-random-host-per-fresh-catalog-so-the-forward-param-probe-may-pass-vacuously-for-doctor-keys.md`
