---
id: ca68c33bb07c7005
kind: bug
status: open
title: err_family maps the markdown overflow error only for the retired read_markdown, so 146 read_file rows (and six new guard messages) land in NULL
tags:
- cluster/config-propagation-is-additive
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

Not started. Proposed: map the overflow messages for `read_file` (and `doc(get)`, which shares the section reader); add families for the six new guard messages above; bump the fingerprint so the backfill re-classifies history.

## Tests added

None yet.

## Workarounds

Classify by message prefix in ad-hoc queries.

## Resume

`normalize_err_family` in `src/usage/db.rs`; tests `normalize_err_family_maps_the_2026_08_20_unclassified_head` show the pattern.

## References

- `docs/issues/archive/2026-08-20-largest-unclassified-error-is-the-worktree-activate-write-block.md` (the previous NULL-head pass)
