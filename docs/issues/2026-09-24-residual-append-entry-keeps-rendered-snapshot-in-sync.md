---
id: a2859e0d13968b04
kind: bug
status: open
title: 'RESIDUAL: Make append_entry/update_entry keep a tracker''s committed rendered snapshot in sync (or refuse), instead of only warning'
tags:
- cluster/unclassified
closed: null
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
severity: low
---

## Summary

Remaining work split out of `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md`, whose `unverified:` caveat named it and was, until this file, the only surface reporting it. Routed here on 2026-09-24 so the open-bug query reaches it: the parent caveat now opens `TRACKED <this file's id>`, and `doctor`'s `terminal_status_with_caveat` reports the parent again if this file closes or moves without the work.

**The work:** Make append_entry/update_entry keep a tracker's committed rendered snapshot in sync (or refuse), instead of only warning.

## Parent caveat, verbatim

`docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` (status `mitigated`):

> Root cause NOT addressed — `append_entry` still writes catalog-only state, so a tracker's committed snapshot still drifts from its live rows. The 11 tests cover the new REPORTING (doctor, append_entry and update_entry now name rows that never reached the body); nothing prevents the drift, and a caller who ignores the warning reproduces the original silent divergence.

## Fix

Not started. Re-checked at HEAD 2026-10-16: the parent caveat is PARTLY overtaken, and what is left is a design choice that is not mine to make.

**Already done, for a tracker that declares a frontmatter `snapshot_anchor`.** `update_entry` re-renders the row inside the same transaction (`resync_snapshot_row`, `src/librarian/catalog/augmentation.rs`, outcomes `SnapshotRow::Rewritten` and `AlreadyCurrent`), and a params `append_entry` writes the section and the index row in one call (`a_params_append_writes_its_section_and_index_row_in_the_same_call`, `a_declared_snapshot_anchor_places_the_row_at_the_blocks_tail`). So the claim that `append_entry` still writes catalog-only state is false for those.

**What is left is only the undeclared case**, where the call still only warns (`snapshot_missing` / `snapshot_stale`). Measured 2026-10-16 on params-backed trackers under `docs/`: 15 keep a body snapshot (majority of ids line-anchored). 2 declare an anchor (`review-catches`, `open-issue-work-queue`). 12 do not: `claim-decay`, `tracker-management-redesign`, `2026-08-16-iron-law-gate-firing-audit`, `structural-debt-refactor`, `test-escape-hardening`, `codex-pilot-checks-2026-10-13`, `fable-tuning-tasks`, `system-retrospective-improvements`, `fable-tuning-findings`, `tool-usage-patterns`, `windows-platform-support`, `prompt-hamsa-audit-log`. All 12 are in sync today. `provenance-subsystem` sits at 57% and is params-canonical by design.

**Options, none chosen.**

A. Refuse writes on an undeclared snapshot-keeping tracker. It rests on the `body_keeps_snapshot` majority heuristic, which has already misfired twice, and `update_entry` has no `index_row` escape, so those 12 trackers could not be updated until an anchor is declared.

B. Infer the anchor when exactly one table matches. The `update_entry` comments reject a row invented by guess.

C. Declare `snapshot_anchor` on the 12, after checking each table's columns equal its `render_template` output (otherwise the first write rewrites hand-authored rows into another shape), and optionally add a doctor check for a snapshot-keeping tracker with no declared anchor. Same shape as the migration in commit `744ec80d`.

D. Keep the advisory and close this as overtaken by `snapshot_anchor`, with the 12 as the standing risk.

Recommended: C. Awaiting the product decision.

### Decision C applied 2026-10-16: none of the 12 can declare an anchor

The decision was option C: declare `snapshot_anchor` on a tracker only after its table columns equal the `render_template` output. Checked byte for byte, by rendering each tracker's template with its catalog params and comparing each body row with the rendered row (the comparison `resync_snapshot_row` itself makes, `src/librarian/catalog/augmentation.rs`). Result for the 12:

- **No body table holds the ids at all (rendered row exists, no `| ID |` row in the body):** `claim-decay`, `tracker-management-redesign`, `structural-debt-refactor`, `test-escape-hardening`, `codex-pilot-checks-2026-10-13`, `fable-tuning-tasks`, `fable-tuning-findings`, `tool-usage-patterns`. Their ids are anchored by `##`/`###` headings, not table rows. The "15 keep a snapshot" count earlier in this file counted heading lines as well as table rows, so it overstated the table-keeping population.
- **A body table exists but its columns differ from the template:** `2026-08-16-iron-law-gate-firing-audit` (all 8 rows differ: the body keeps a shorter Finding and a different Status/Target than the rendered `| ID | Finding | Sev | Status | Target |`, e.g. GF-1 body `**Incomplete fix** of ...` against rendered `INCOMPLETE FIX of ...`), `windows-platform-support` (body `| id | area | status | summary | ref | since |` against rendered `| id | area | status | since | ref |`; 31 of 35 ids are filtered out of the render), `prompt-hamsa-audit-log` (body `| ID | Date | Artifact | Gap | ... |`; the template renders no row for any of the 40 ids), `system-retrospective-improvements` (rendered `| ID | Task | Status | Notes |` has no matching body table).

Declaring an anchor on any of these would make the first `update_entry` rewrite a hand-authored row into the rendered shape, or no-op, so none was declared. No tracker file was changed. The two that already declare one (`review-catches`: 56 of 56 rows byte-equal; `open-issue-work-queue`: 48 of 77 equal, 29 differ) are unchanged. The 29 differing rows in `open-issue-work-queue` were already known (`resync_snapshot_row` re-renders per row for that reason).

The doctor check "snapshot-keeping tracker without declared snapshot_anchor" was NOT added: it would fire on all 12 with nothing actionable, because the condition it tests (`body_keeps_snapshot`, a majority of ids line-anchored) is satisfied by heading-anchored ledgers that keep no table. The finding worth a follow-up is the advisory itself: `snapshot_missing` / `snapshot_stale` fire for heading-anchored ledgers on the same heuristic.

## References

- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — parent
