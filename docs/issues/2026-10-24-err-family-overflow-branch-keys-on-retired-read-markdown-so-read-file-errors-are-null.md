---
id: ca68c33bb07c7005
kind: bug
status: fixed
title: err_family maps the markdown overflow error only for the retired read_markdown, so 146 read_file rows (and six new guard messages) land in NULL
tags:
- cluster/config-propagation-is-additive
closed: 2026-10-27
fix_patch_id: ccc4e87cb187b9404f075ca2ba6de4cfe7ec2a69
fix_sha: 03c125594 (experiments)
follow_up_fix_patch_id: 9fa67bb3eddc08fa1ff915e2d0f0d909c7422d0b
follow_up_fix_sha: 3ba135be5 (experiments)
opened: 2026-10-24
owner: marius
severity: low
---

# BUG: err_family's markdown-overflow branch is scoped to the retired `read_markdown`, so `read_file`'s identical errors land in NULL

## Summary

`normalize_err_family` (`src/usage/db.rs`) maps `"exceeds inline threshold"` to `read_markdown_overflow_threshold` only inside `if tool_name == "read_markdown"`. The comment above that block says those branches "classify HISTORICAL rows on backfill and are dead for new ones", because `read_markdown` was folded into `read_file` on 2026-09-02. But `read_file` now emits the same messages (`read_markdown_single_heading` / `read_markdown_multi_heading` in `src/tools/markdown/read_markdown.rs`), and no branch maps them for `tool_name == "read_file"`. In 2026-10-03..10-24 they are the largest part of the unclassified head: **146 `read_file` rows** (`section "…" spans N lines — exceeds inline threshold` 77, `combined headings span N lines — exceeds inline threshold` 69).

## Class fit

A rename (`read_markdown` → `read_file`) landed in the tool surface, but the classifier kept keying on the old name, so the update propagated and the rename did not. It differs from other IC-4 members because the stale key is a tool name inside telemetry, and the failure mode is silent re-labelling to NULL rather than a broken call.

## Symptom (Effect)

`err_family IS NULL` holds 785 rows (11.2% of all errors) in the window, against an instrument note (`infra/friction-measurement` memory) that says the NULL head was cleared to "4 one-off residue by design". Anyone ranking families or computing coverage reads this as a new uncatalogued population.

## Reproduction

`SELECT tool_name, substr(error_msg,1,60), COUNT(*) FROM tool_calls WHERE err_family IS NULL AND called_at>='2026-10-03' GROUP BY 1,2 ORDER BY 3 DESC;` on the codescout `usage.db`.

## Environment

All usage.db files 2026-10-03..10-24 (40 projects).

## Root cause

The `read_markdown` branch was intentionally left for historical rows; the matching `read_file` branch was never added. The rest of the NULL head is new guard messages with no family yet (measured across all DBs, same window): `read_file` `end_line provided without start_line` 89; `doc(action="move") refused: this fixed bug declares no fix anchor` 78; `edit_code replace(…) was given a complete declaration, but it declares …` 91; `symbols` `pattern looks like a regex` 49; `edit_code` shrink guard 38; `doc` `body_edits[N]: missing required … field` 25.

## Evidence

`src/usage/db.rs` lines ~491-519 (`if tool_name == "read_markdown" { … if msg.contains("exceeds inline threshold") { return Some("read_markdown_overflow_threshold"); } … }`); the 2026-10-24 sweep's NULL breakdown (`docs/usage-reports/2026-10-24-usage-analysis.md`).

## Hypotheses tried

N/A.

## Fix

Two commits on `experiments`.

- `03c125594` (patch-id `ccc4e87cb187b9404f075ca2ba6de4cfe7ec2a69`). `normalize_err_family` returns `read_file_overflow_threshold` for a `read_file` message that starts with `section "` or `combined headings span ` and contains ` lines — exceeds inline threshold`. The rule sits before the substring scans, because the section title inside the message is data: a title such as `index is locked` would otherwise match `lsp_index_locked`.
- `3ba135be5` (patch-id `9fa67bb3eddc08fa1ff915e2d0f0d909c7422d0b`). Four new families and two folded messages, each on a fixed prefix before the substring scans:
  - `shrink_guard_blocked`: `shrink guard: `, `body-shrink guard: ` and `memory-shrink guard: `. The source has five emitters (`edit_code`, `create_file`, `edit_markdown`, `doc(update)`, `memory`). Only `edit_code` rows exist in the data read here.
  - `replace_would_rename`: `edit_code replace('…') was given a complete declaration, but it declares …`. The neighbouring `replace_dropped_sibling` keeps its own text.
  - `unanchored_archive_refused`: both texts of `refuse_unanchored_archive` (no fix anchor, and a SHA without its patch-id), from `doc(move)` and `doc(update)`.
  - `symbols_pattern_is_regex`: tool-scoped to `symbols`.
  - `missing_required_param` now also takes `end_line provided without start_line` and `body_edits[N]: missing required '…'`.

Both commits add families to `ERR_FAMILIES`, which changes the persisted fingerprint, so each `usage.db` re-runs the backfill on its next open. The backfill only fills NULL rows.

**Backfill verified on a real copy.** I copied this project's live `usage.db` with `sqlite3 .backup` and ran the real `open_db` path on it.

- After resetting the 74 overflow rows and rewinding `user_version`, exactly 74 rows changed, all from NULL to `read_file_overflow_threshold`.
- With the four new families, 117 more rows changed, all from NULL to the intended family. None left an existing family. The row count held at 73,632. NULL rows fell from 217 to 100.
- The live DB's `user_version` was `90010721`, which is not the fingerprint of the code at the time. It re-runs the backfill on its next open.

**Corrections to this bug's own text.**

- `doc(get)` does not share the section reader. `grep` for `exceeds inline threshold` finds one emitter, `src/tools/markdown/read_markdown.rs`, and only `ReadFile` calls it. No `doc(get)` family is needed.
- The 49 `symbols` regex errors were not reproduced. A `find` under `/home/marius/work` listed 35 `usage.db` files. The query ran with stderr suppressed, so I did not verify that every file opened. It found 4 rows, in 2 DBs (3 and 1). The shape is `pattern looks like a regex (found regex syntax) — symbols searches symbol names, not text`.

**Still open.** 100 NULL rows remain in this project's DB. They are other messages: the `doc` leaf/ops filter-shape errors, `edit_file` `edits[]` mixing shapes, the `/tmp` rooted-artifact refusal, `both start_line and end_line are required` on `read_file`, and a few one-offs. None is in this bug's list. File a new bug if the ranking needs them.

## Tests added

- `a_read_file_overflow_is_classified_and_stays_tool_scoped` in `src/usage/db.rs`. It covers both live shapes, a hostile title (`index is locked`), the `read_markdown` arm keeping its own family, a non-overflow `read_file` error, and a `run_command` tool-scope check. RED was watched (`left: None`), and a naive `contains` placement failed the hostile-title case.
- `the_six_guard_messages_the_2026_10_24_sweep_found_unclassified_have_a_family` in `src/usage/db.rs`. Shapes are copied from live rows, or from the emitting `format!` where no row exists here. Every rule has a hostile symbol name or hint, and `symbols_pattern_is_regex` has a tool-scope `assert_ne`. RED was watched (`left: None`).
- 56 `usage::db::tests::` tests pass, including `err_families_lists_exactly_what_the_classifier_can_emit`. The full `./scripts/gate.sh` was green for both commits (`FMT=0 CLIPPY=0 LEAN=0 DEFAULT=0`, `PYTHON_LIGHT=0`).

## Workarounds

Classify by message prefix in ad-hoc queries.

## Resume

`normalize_err_family` in `src/usage/db.rs`; tests `normalize_err_family_maps_the_2026_08_20_unclassified_head` show the pattern.

## References

- `docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md` (the previous NULL-head pass)
