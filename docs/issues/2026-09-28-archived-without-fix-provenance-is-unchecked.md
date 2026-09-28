---
id: f9e51dc0a0af8693
kind: bug
status: open
title: An archive made without Fix provenance is invisible to every check, because doctor skips all archived records
tags:
- librarian
- doctor
- cluster/guard-narrower-than-its-name
opened: 2026-09-28
severity: medium
---

## Summary

`doctor`'s `terminal_status_without_fix_anchor` is the only check for the `## Fix provenance` section that `get_guide("tracker-conventions")` § *Bug files* requires **at archive time**. It skips every `archived` record (`terminal_status_without_fix_anchor_leaves_archived_records_alone`, `src/librarian/tools/doctor.rs`). So a record archived without the section is reported nowhere, and the one moment the rule applies is the moment its check stops looking. The skip covers the whole archive rather than a date range: its doc comment justifies it by the 297 of 355 archived files that predated the rule, and it hides a record archived today just the same.

## Symptom (Effect)

No finding, before or after the move. A `fixed` record without the section reports under `terminal_status_without_fix_anchor`; flip it to `archived` and move it, and the finding disappears whether or not the section was added. The drop in the count reads as the repair.

## Reproduction

1. A `fixed` bug with no `## Fix provenance` section: `librarian(action="doctor")` reports it under `terminal_status_without_fix_anchor`.
2. `doc(action="update", patch={status: "archived"})`, then `doc(action="move")` into `docs/issues/archive/`.
3. `doctor` again: the finding is gone.

Observed 2026-09-28 by session `82cff72e`. It archived `e76b043bbdd97622` and `60fcfdf99e3288c6` without the section, one with the pair in prose and one with it only in the commit message, and no check reported either. It surfaced only when both were grepped while archiving five others (`8dfc251e`), and the doctor drop for that pass, 15 to 7 on this check, would have been identical with or without those five sections.

## Environment

`experiments` at `8dfc251e`; release binary built from `6a6a321e`.

## Root cause

A scope decision made for the backlog and applied to every record: `scan_terminal_status_without_fix_anchor` excludes `archived` by status, never by when the record was archived.

## Evidence

Derived 2026-09-28 on the tree at `8dfc251e`, by parsing every `docs/issues/archive/*.md` whose frontmatter says `kind: bug` (923 files). A file counts as missing when it has neither a `## Fix provenance` heading outside a fenced block nor a non-empty `no_fix_commit:`:

| `closed:` in | archived bug files | missing both |
|---|---|---|
| 2026-09-14 to 2026-09-20 | 59 | 45 |
| 2026-09-21 to 2026-09-27 | 54 | 30 |

Both are upper bounds on the defect. They include records closed `wontfix`, which owe no pointer, and records whose pair sits in prose. The check would count the prose records as missing too, but a reader could still recover the pair from them.

## Hypotheses tried

None needed: the skip is deliberate and named by its own test.

## Fix

Not designed. Two candidates:

- **Scope the skip by date:** report an archived record whose `closed:` postdates the rule, which leaves the pre-rule backlog out of scope and makes every new omission report.
- **Check at the transition:** `doc(action="move")` into `docs/issues/archive/`, on a record with no pointer and no `no_fix_commit:`, warns or refuses.

A sibling on the same check: `5394e9b7bdd83069` (a SHA with no patch-id discharges it).

## Tests added

None yet.

## Workarounds

Before archiving, grep the file for `^## Fix provenance` yourself; do not read the check's count going down as confirmation.

## Resume

Open. Pick a candidate above; either needs a regression test that archives a record without the section and expects a finding.

## References

- `docs/issues/archive/2026-08-19-terminal-bug-file-with-no-recoverable-fix-anchor.md` (the check's origin)
- `docs/trackers/bug-fix-session-log.md` (the session entries for this pass)
