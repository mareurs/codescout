---
id: '43e0e0c684c0c038'
kind: bug
status: fixed
title: 'BUG: 119 of 1,113 non-success rows in the codescout usage database (10.7%) carry no err_family, after the 2026-10-27 fix for three other shapes'
tags:
- cluster/selector-narrower-than-its-population
closed: 2026-11-02
opened: 2026-11-02
owner: marius
related:
- docs/issues/2026-10-26-three-recurring-error-shapes-carry-no-err-family.md
severity: low
unverified: 'How many of the 119 real rows the new classifier covers was not measured: the backfill runs on the next open of each database. CI has not been read. Shapes with fewer than three rows stay unclassified.'
---

# BUG: 119 of 1,113 non-success rows in the codescout usage database (10.7%) carry no err_family, after the 2026-10-27 fix for three other shapes

## Summary

`err_family` is derived from message prefixes in `ERR_FAMILIES` (`src/usage/db.rs`). The 2026-10-27 fix (`docs/issues/2026-10-26-three-recurring-error-shapes-carry-no-err-family.md`) added families for three shapes, and `friction-probe.py` and any recovery analysis group by family. Since 2026-10-25, 119 of the 1,113 non-success rows in `.codescout/usage.db` still have `err_family IS NULL`, spread over shapes that the table has no family for. Each new recurring shape returns the NULL bucket to a size where a real new error is hard to tell from it.

## Class fit

The class `selector-narrower-than-its-population` fits: a list of message prefixes names the error shapes it knows, and the population of shapes it has to cover keeps growing. The earlier bug on this table sits in the same class. It differs from the other members in that the population here is the set of messages our own tools produce, so it is enumerable at build time.

## Symptom (Effect)

A query for non-success rows grouped by `err_family` shows a `(null)` group of about 10.7% of non-success rows. The largest NULL shapes in the window (tool, message start, rows):

| Tool | Message starts with | Rows |
|---|---|---:|
| `edit_code` | the language server answered textDocument/documentSymbol with null | 20 |
| `doc` | refusing to write an artifact rooted under the system temp dir | 11 |
| `grep` | 'output_mode' is not a parameter of grep — 'count' was not applied | 11 |
| `edit_file` | edits array is mutually exclusive with top-level heading/action | 6 |
| `grep` | 'output_mode' ... 'files' was not applied | 4 |
| `doc` | ops must be an object | 4 |
| `grep` | 'output_mode' ... 'files_with_matches' was not applied | 3 |
| `edit_file` | invalid action "append"; expected one of: replace, insert_before, ... | 3 |
| `doc` | doc(action="update") was given nothing to change | 3 |
| `doc` | start_line/end_line are file lines | 3 |

## Reproduction

```
sqlite3 -readonly .codescout/usage.db "SELECT tool_name, substr(error_msg,1,95), COUNT(*) FROM tool_calls WHERE called_at >= '2026-10-25' AND outcome != 'success' AND err_family IS NULL GROUP BY 1,2 ORDER BY 3 DESC LIMIT 14;"
```

and the count: `SELECT COUNT(*) FROM tool_calls WHERE called_at >= '2026-10-25' AND outcome != 'success' AND err_family IS NULL` gives 119, against 1,113 non-success rows.

## Environment

codescout `experiments` (recorded builds include `d74294616`, `a6d175ee0` and others), the codescout project database only; the other databases were not queried for this.

## Root cause

Not read this session. The families are matched by message prefix, so a message with no entry in the table is NULL by construction. Not checked: whether some of these messages should be `outcome = 'success'` instead. The three `grep` rows say "was not applied" and the call still ran, so they may be corrections counted as errors; that is a separate question that this file does not decide.

## Evidence

The two queries above, run on 2026-11-02, and the table. `docs/usage-reports/2026-11-02-usage-analysis.md` records the window.

## Hypotheses tried

None.

## Fix

Done on `experiments` at `423d812b4`, for the shapes with three or more rows. In `normalize_err_family` (`src/usage/db.rs`): four new families, `lsp_document_symbol_null` (22 rows), `artifact_rooted_in_temp` (11), `param_not_applied` (18) and `invalid_param_shape` (4), the first three at the top because their messages embed data; and four extended ones, `unknown_enum_value` (invalid action), `mutually_exclusive_params` (`edit_file` batch against heading form), `invalid_line_range` (`doc(get)` outside the body) and `missing_required_param` (`doc(update)` with nothing to change). `ERR_FAMILIES` lists the four new names in sorted order, so the backfill fingerprint changes and each database re-classifies its NULL rows on its next open. Not done: shapes with fewer than three rows (`disk I/O error`, `unsupported language`, `@bg_*` refs with `.err`, the undeclared `cluster/` tag and the malformed `extra.opened` refusals of `doc`, `LSP request timed out`, and others); the test pins the first two as `None`. Not decided: whether the `grep` "was not applied" notices are errors; they only have a family now.


## Fix provenance

- **SHA:** `423d812b45767960e9f21ed9abd78197b1e83e63` (`experiments`)
- **patch-id:** `a1a6d9a0afbdddbbaf2d388bd977af990d61a2ae`

## Tests added

`the_shapes_of_the_2026_11_02_null_head_have_a_family` in `src/usage/db.rs`: 14 cases built from the real message prefixes, three with hostile data that holds other rules' keywords (a path and a parameter name), two pinned as `None`. Red before the change (the first case gave `None`), green after. Gate in a clean worktree: FMT, CLIPPY, LEAN and DEFAULT all 0, 8,836 tests; `cargo test --lib usage::` 109 passed. Not measured: how many of the 119 real rows the new classifier covers. The backfill runs on the next open of each database, so that count comes after a rebuild. CI has not been read.

## Workarounds

Read `error_msg` directly for the NULL rows, as the queries above do.

## Resume

Fixed on `experiments` (`423d812b4`) for the shapes with three or more rows. Next: after a rebuild and reconnect, count the NULL rows again (`SELECT COUNT(*) FROM tool_calls WHERE called_at >= '2026-10-25' AND outcome != 'success' AND err_family IS NULL`, against 119 before) and archive this file with that figure.

## References

- `docs/issues/2026-10-26-three-recurring-error-shapes-carry-no-err-family.md` (fixed)
- `docs/issues/2026-10-24-err-family-overflow-branch-keys-on-retired-read-markdown-so-read-file-errors-are-null.md`
- `docs/issues/archive/2026-08-16-backfill-gate-not-derived-from-the-taxonomy.md`
