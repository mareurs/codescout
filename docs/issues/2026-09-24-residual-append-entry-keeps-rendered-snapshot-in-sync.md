---
id: a2859e0d13968b04
kind: bug
status: mitigated
title: 'RESIDUAL: Make append_entry/update_entry keep a tracker''s committed rendered snapshot in sync (or refuse), instead of only warning'
tags:
- cluster/unclassified
closed: 2026-10-17
opened: 2026-09-24
owner: marius
related:
- docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md
severity: low
unverified: 'STANDING 2026-10-17. Three table-keeping ledgers stay warning-only (snapshot_missing / snapshot_stale on append_entry, update_entry and doctor; never refused or auto-synced): 2026-08-16-iron-law-gate-firing-audit, windows-platform-support and prompt-hamsa-audit-log. Their body table columns differ from the render_template output, so none can declare a snapshot_anchor without the first update_entry rewriting a hand-authored row into the rendered shape. The other 9 of the 12 snapshot-keeping candidates keep no table and the advisory is already silent for them. See Decision C and Outcome 2026-10-17 in the body.'
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

The doctor check "snapshot-keeping tracker without declared snapshot_anchor" was NOT added: it would fire on all 12 with nothing actionable, because the condition it tests (`body_keeps_snapshot`, a majority of ids line-anchored) is satisfied by heading-anchored ledgers that keep no table. The finding worth a follow-up is the advisory itself: `snapshot_missing` / `snapshot_stale` fire for heading-anchored ledgers on the same heuristic. **[Corrected 2026-10-17: that sentence is false at HEAD. Measured below under *Outcome 2026-10-17*: the advisory is keyed on table ROWS, not headings, since `2026-08-28-body-keeps-snapshot-counts-headings-as-a-table`.]**

### Outcome 2026-10-17: the narrowing the user decided on is already the code's behaviour

The decision was to narrow `snapshot_missing` / `snapshot_stale` so they fire only for ledgers that keep a rendered table. **At HEAD (`299395e5`) that is already what the code does, and it has been since 2026-08-28** (`docs/issues/archive/2026-08-28-body-keeps-snapshot-counts-headings-as-a-table.md`). The closing sentence of the section above (heading-anchored ledgers get the advisory) was false when written; it is marked corrected in place. No production code changed.

**The predicate, as it exists (read at the bytes):** the advisory fires iff `body_keeps_snapshot(claimed, in_body)` (`src/librarian/catalog/augmentation.rs`) where `in_body = snapshot_rows_in_declared_block(doc, PREFIX)`. That set is the `N` of every line matching `^\|[ \t]*[`*\[]*PREFIX-(\d+)\b`, a `|`-anchored row whose first cell is the id, and **never a heading** (`body_snapshot_row_indices`). With a declared `snapshot_anchor` it is narrowed to that block. `body_keeps_snapshot` is true iff `claimed` and `in_body` are non-empty and `|claimed ∩ in_body| * 2 > |claimed|`. So "keeps a table" means: the body carries table rows for a majority of the ledger's ids. Three call sites share it: `append_entry` (`snapshot_missing`), `update_entry` via `snapshot_stale_note` (`snapshot_stale`), and doctor's `scan_snapshot_drift`.

**Measured 2026-10-17 by running the real `snapshot_rows_in_declared_block` and `body_keeps_snapshot` over the real tracker files at `299395e5`** (scratch test, not committed; `claimed` approximated by the ids the body anchors, because the catalog's params are machine-local and were not read):

| tracker | prefix | ids the body anchors | table rows | advisory applies |
|---|---|---|---|---|
| `claim-decay` | DC | 6 | 0 | no |
| `tracker-management-redesign` | TMR | 7 | 0 | no |
| `structural-debt-refactor` | SD | 11 | 0 | no |
| `test-escape-hardening` | I | 11 | 0 | no |
| `codex-pilot-checks-2026-10-13` | T | 12 | 0 | no |
| `fable-tuning-tasks` | FT | 12 | 0 | no |
| `fable-tuning-findings` | FND | 18 | 0 | no |
| `tool-usage-patterns` | T | 34 | 0 | no |
| `system-retrospective-improvements` | SRI | 17 | 2 (`SRI-11`, `SRI-12`, an unrelated findings table) | no, a minority citation |
| `2026-08-16-iron-law-gate-firing-audit` | GF | 8 | 8 | yes |
| `windows-platform-support` | WIN | 35 | 35 | yes |
| `prompt-hamsa-audit-log` | A | 40 | 40 | yes |

So **9 of the 12 are already silent and 3 keep a real table**. Those 3 are the ledgers the user's rule keeps the advisory for, and they are the ones whose table columns differ from the template, so none can declare an anchor (section above). Controls from the same run: `review-catches` 56/56 rows and `open-issue-work-queue` 77/77 rows apply, `provenance-subsystem` 10 of 30 does not (params-canonical).

**What was added.** Three tests, each with an in-test control (the same ids as table rows must still be flagged), because an absence assertion is monotone under removal:
- `update_entry::tests::patching_a_headings_only_ledger_says_no_snapshot_exists_to_be_behind`: `update_entry` on a headings-only ledger carries no `snapshot_stale`. No `update_entry` test covered this shape before.
- `update_entry::tests::patching_a_heading_ledger_that_cites_two_ids_in_a_table_stays_silent`: the `system-retrospective-improvements` shape, on `update_entry`.
- `augmentation::tests::append_entry_stays_silent_for_a_heading_ledger_that_cites_two_ids_in_a_table`: the same shape on `append_entry`, with the table-keeper control (`["F-5","F-6"]`).

Pre-existing pins that already covered the rest: `append_entry_does_not_claim_a_snapshot_when_the_body_is_headings_only`, `append_entry_still_reports_a_missing_row_when_the_body_renders_a_table` (a table and no anchor still gets `snapshot_missing`), `patching_a_rendered_row_without_an_anchor_flags_it_without_asserting_the_cells_moved` (still gets `snapshot_stale`), and the doctor `snapshot_drift_*` set.

**Mutation probes** (`./scripts/mutation-probe.sh`, run against the three new tests alone): heading alternation added to `body_snapshot_row_indices`: KILLED 3 of 3. Majority gate loosened to `count() >= 1`: KILLED 2 of 3 (the headings-only test cannot see it, correctly: no rows at all). Gate raised to `count() * 2 > 2 * claimed.len()`: KILLED 3 of 3 via the controls.

**Deliberately NOT done, and why.** Tightening "a row" further (require a header and separator line, skip fenced code) was weighed and rejected: a scan of every `docs/trackers/**/*.md` found rows outside a table structure or inside a fence in exactly one file, `bug-fix-session-log.md`, a prose ledger that declares `entry_prefix` and has no sidecar under `docs/augmentations/`, so this path never reads it. The change would fix no ledger that exists and would red the many fixtures that write a bare `| T-1 | open |` with no separator. No prompt surface or guide names `snapshot_missing` / `snapshot_stale` (grep over `src/prompts`, `src/**/*.md`), so `ONBOARDING_VERSION` does not move.

**Status.** The proposed transition is `mitigated`, `closed: 2026-10-17`, with `unverified: STANDING` naming the 3 table-keeping ledgers that stay warning-only because their columns differ from the render. It is NOT applied in the frontmatter here: this file is catalog-managed, so the flip has to go through `doc(action="update")`, which writes the catalog, and this session was barred from catalog writes. Note for whoever applies it: the archived parent carries `unverified: TRACKED a2859e0d13968b04`, which `doctor`'s `terminal_status_with_caveat` stops honouring once this file leaves `open`, so the parent will be reported again until it is discharged.

Test commit `8a0ad373` (patch-id `26de8fbc230a2535b0d3f37fa478bbdc051f396b`), on a worktree branch cut from `experiments` at `299395e5`, not yet merged to `experiments`.

## References

- `docs/issues/archive/2026-08-16-append-entry-leaves-the-rendered-snapshot-stale-with-no-signal.md` — parent
